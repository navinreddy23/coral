use bstr::{BString, ByteSlice};

use crate::error::CoralError;

/// How much unchanged text a patch carries around each change.
///
/// A hunk view wants the three lines either side that git gives by default. A side-by-side
/// view of a whole file wants all of it, so the reader can see the change where it sits rather
/// than in a window cut out of the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Context {
    #[default]
    Hunks,
    WholeFile,
}

impl Context {
    /// The `-U` git wants. Whole-file is a count larger than any file, which is how git is
    /// asked for all of it; there is no flag that says so.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Hunks => "-U3",
            Self::WholeFile => "-U1000000000",
        }
    }
}

/// What happened to a file between two trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Context,
    Add,
    Remove,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub kind: LineKind,
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub text: BString,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    /// The file does not end with a newline, and this is its last line.
    pub no_newline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    /// The `@@ ... @@` line verbatim, including any trailing section heading.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub header: BString,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub path: BString,
    #[serde(serialize_with = "crate::bytes::as_str_opt")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub old_path: Option<BString>,
    pub change: FileChange,
    pub binary: bool,
    /// Lines added and removed. `None` for a binary file, where git reports no counts.
    pub added: Option<u32>,
    pub removed: Option<u32>,
    /// Empty for a binary file, a pure rename, or a mode-only change.
    pub hunks: Vec<Hunk>,
    /// Set when the file was not read because it exceeds the size guard.
    pub too_large: bool,
}

impl FileDiff {
    #[must_use]
    pub fn is_renamed(&self) -> bool {
        matches!(self.change, FileChange::Renamed | FileChange::Copied)
    }
}

/// Files over this many bytes of patch text are reported without hunks; the UI offers an
/// explicit load. A single kernel-sized generated file can otherwise stall a paint.
pub const LARGE_PATCH_BYTES: usize = 5 * 1024 * 1024;

/// Parses `git diff -z --numstat -M`.
///
/// This, not the patch header, is the authoritative source of paths: `diff --git a/x b/y` is
/// ambiguous when a path contains a space, and git only quotes for tabs and control
/// characters, not spaces. The NUL-delimited numstat has no such problem.
///
/// # Errors
/// [`CoralError::Protocol`] on a record that does not match the documented grammar.
pub fn parse_numstat(input: &[u8]) -> Result<Vec<FileDiff>, CoralError> {
    let mut out = Vec::new();
    let mut records = input.split(|b| *b == 0).filter(|r| !r.is_empty());

    while let Some(record) = records.next() {
        let mut parts = record.splitn(3, |b| *b == b'\t');
        let added = parts.next().unwrap_or_default();
        let removed = parts
            .next()
            .ok_or_else(|| protocol("numstat record has no delete count"))?;
        let path = parts.next().unwrap_or_default();

        // Git reports "-" for both counts on a binary file.
        let binary = added == b"-";
        let (added, removed) = if binary {
            (None, None)
        } else {
            (Some(count(added)?), Some(count(removed)?))
        };

        // A rename or copy leaves the path field empty and follows with old then new.
        let (path, old_path) = if path.is_empty() {
            let old = records
                .next()
                .ok_or_else(|| protocol("rename has no source path"))?;
            let new = records
                .next()
                .ok_or_else(|| protocol("rename has no destination path"))?;
            (BString::from(new), Some(BString::from(old)))
        } else {
            (BString::from(path), None)
        };

        out.push(FileDiff {
            path,
            old_path,
            // numstat cannot distinguish added from modified; `apply_name_status` refines it.
            change: FileChange::Modified,
            binary,
            added,
            removed,
            hunks: Vec::new(),
            too_large: false,
        });
    }
    Ok(out)
}

/// Applies `git diff -z --name-status -M`, which knows add from delete from rename.
///
/// # Errors
/// [`CoralError::Protocol`] if a status letter is unknown.
pub fn apply_name_status(files: &mut [FileDiff], input: &[u8]) -> Result<(), CoralError> {
    let mut records = input.split(|b| *b == 0).filter(|r| !r.is_empty());
    let mut i = 0;

    while let Some(status) = records.next() {
        let letter = *status
            .first()
            .ok_or_else(|| protocol("empty status record"))?;
        let change = match letter {
            b'A' => FileChange::Added,
            b'D' => FileChange::Deleted,
            b'M' | b'T' => FileChange::Modified,
            b'R' => FileChange::Renamed,
            b'C' => FileChange::Copied,
            _ => return Err(protocol("unknown name-status letter")),
        };
        // The path follows as its own record, and a rename or copy has two.
        let extra = usize::from(matches!(change, FileChange::Renamed | FileChange::Copied));
        for _ in 0..=extra {
            records.next();
        }
        if let Some(f) = files.get_mut(i) {
            f.change = change;
        }
        i += 1;
    }
    Ok(())
}

/// Splits a `-p` patch into per-file sections and parses their hunks.
///
/// Sections are matched to `files` positionally, because git emits both listings in the same
/// order. Paths are never read from the patch, for the reason given on [`parse_numstat`].
///
/// # Errors
/// [`CoralError::Protocol`] if a hunk header does not parse, or if the patch has more file
/// sections than the authoritative listing.
pub fn apply_patch(files: &mut [FileDiff], patch: &[u8]) -> Result<(), CoralError> {
    if patch.len() > LARGE_PATCH_BYTES {
        for f in files.iter_mut() {
            f.too_large = true;
        }
        return Ok(());
    }

    for (i, section) in split_sections(patch).enumerate() {
        let Some(file) = files.get_mut(i) else {
            return Err(protocol(
                "patch has more file sections than the numstat listing",
            ));
        };
        file.hunks = parse_hunks(section)?;
    }
    Ok(())
}

/// Yields each `diff --git ...` section as a slice of lines.
fn split_sections(patch: &[u8]) -> impl Iterator<Item = Vec<&[u8]>> {
    let mut sections: Vec<Vec<&[u8]>> = Vec::new();
    for line in patch.split(|b| *b == b'\n') {
        if line.starts_with(b"diff --git ") {
            sections.push(Vec::new());
        } else if let Some(current) = sections.last_mut() {
            current.push(line);
        }
    }
    sections.into_iter()
}

/// Parses every hunk in one file's section. A binary file, a pure rename, or a mode-only
/// change has none.
fn parse_hunks(section: Vec<&[u8]>) -> Result<Vec<Hunk>, CoralError> {
    let mut hunks = Vec::new();
    let mut lines = section.into_iter().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with(b"@@") {
            let mut hunk = parse_hunk_header(line)?;
            read_hunk_body(&mut hunk, &mut lines);
            hunks.push(hunk);
        }
    }
    Ok(hunks)
}

/// `@@ -old_start,old_lines +new_start,new_lines @@ optional heading`
fn parse_hunk_header(line: &[u8]) -> Result<Hunk, CoralError> {
    let text = line.to_str_lossy();
    let body = text
        .strip_prefix("@@ ")
        .and_then(|r| r.split_once(" @@"))
        .ok_or_else(|| protocol("malformed hunk header"))?
        .0;
    let (old, new) = body
        .split_once(' ')
        .ok_or_else(|| protocol("hunk header has one range"))?;

    let (old_start, old_lines) = range(old.trim_start_matches('-'))?;
    let (new_start, new_lines) = range(new.trim_start_matches('+'))?;
    Ok(Hunk {
        header: BString::from(line),
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: Vec::new(),
    })
}

/// `12,4` or bare `12`, which means a single line.
fn range(spec: &str) -> Result<(u32, u32), CoralError> {
    match spec.split_once(',') {
        Some((s, n)) => Ok((count(s.as_bytes())?, count(n.as_bytes())?)),
        None => Ok((count(spec.as_bytes())?, 1)),
    }
}

/// Reads a hunk's lines, bounded by the counts its header declares.
///
/// The bound is not an optimisation. Splitting a patch on newlines leaves a trailing empty
/// element after the final line, and an empty element is indistinguishable from a context line
/// whose single space was stripped. Consuming until the prefix stops matching therefore
/// appended a phantom context line to the last file of every patch, producing a hunk one line
/// longer than the file — which `git apply` rejects.
fn read_hunk_body<'a, I>(hunk: &mut Hunk, lines: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = &'a [u8]>,
{
    let mut old_no = hunk.old_start;
    let mut new_no = hunk.new_start;
    let mut old_seen = 0;
    let mut new_seen = 0;

    while old_seen < hunk.old_lines || new_seen < hunk.new_lines {
        let Some(line) = lines.peek() else { break };
        let kind = match line.first() {
            Some(b' ') => LineKind::Context,
            Some(b'+') => LineKind::Add,
            Some(b'-') => LineKind::Remove,
            // git's marker for a file with no trailing newline; it annotates the line above
            // rather than being a line of its own, and does not count towards either side.
            Some(b'\\') => {
                lines.next();
                if let Some(last) = hunk.lines.last_mut() {
                    last.no_newline = true;
                }
                continue;
            }
            _ => break,
        };
        let line = lines.next().unwrap_or_default();
        let text = BString::from(line.get(1..).unwrap_or_default());

        let (o, n) = match kind {
            LineKind::Context => {
                let v = (Some(old_no), Some(new_no));
                old_no += 1;
                new_no += 1;
                old_seen += 1;
                new_seen += 1;
                v
            }
            LineKind::Add => {
                let v = (None, Some(new_no));
                new_no += 1;
                new_seen += 1;
                v
            }
            LineKind::Remove => {
                let v = (Some(old_no), None);
                old_no += 1;
                old_seen += 1;
                v
            }
        };
        hunk.lines.push(Line {
            kind,
            text,
            old_no: o,
            new_no: n,
            no_newline: false,
        });
    }

    // The counts are satisfied by the last content line, but a "no newline" marker can still
    // follow it and belongs to it.
    if lines.peek().is_some_and(|l| l.starts_with(b"\\")) {
        lines.next();
        if let Some(last) = hunk.lines.last_mut() {
            last.no_newline = true;
        }
    }
}

fn count(field: &[u8]) -> Result<u32, CoralError> {
    field
        .to_str()
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| protocol("expected a number"))
}

fn protocol(detail: &str) -> CoralError {
    CoralError::Protocol {
        label: "diff",
        detail: detail.to_owned(),
    }
}
