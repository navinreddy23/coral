use bstr::{BString, ByteSlice};

use crate::diff::{FileDiff, Hunk, LineKind};
use crate::error::CoralError;

/// Which lines of a hunk to apply. Line numbers are indices into [`Hunk::lines`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Every changed line in the hunk.
    WholeHunk,
    /// Only these lines; every other changed line is treated as unchanged.
    Lines(Vec<usize>),
}

impl Selection {
    fn includes(&self, i: usize) -> bool {
        match self {
            Self::WholeHunk => true,
            Self::Lines(v) => v.contains(&i),
        }
    }
}

/// Which direction a generated patch runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Worktree into the index.
    Stage,
    /// Index back out, which git applies in reverse.
    Unstage,
    /// Out of the working tree altogether, which is the one that cannot be undone.
    Discard,
}

/// Builds a patch containing only the selected hunks of one file, suitable for
/// `git apply --cached`.
///
/// The hard part is not filtering lines but recomputing the header. A `+` line that is not
/// selected disappears entirely, while a `-` line that is not selected becomes context — it
/// still exists on both sides. Getting that backwards produces a patch that applies cleanly
/// and silently stages the wrong thing.
///
/// # Errors
/// [`CoralError::Protocol`] if a requested hunk does not exist.
pub fn build_patch(
    file: &FileDiff,
    hunks: &[(usize, Selection)],
    direction: Direction,
) -> Result<BString, CoralError> {
    let mut out = BString::from(Vec::new());
    let old_path = file.old_path.as_ref().unwrap_or(&file.path);

    out.extend_from_slice(b"diff --git ");
    side(b'a', old_path, &mut out);
    out.push(b' ');
    side(b'b', &file.path, &mut out);
    out.push(b'\n');
    out.extend_from_slice(b"--- ");
    side(b'a', old_path, &mut out);
    out.push(b'\n');
    out.extend_from_slice(b"+++ ");
    side(b'b', &file.path, &mut out);
    out.push(b'\n');

    // Each retained hunk shifts the lines after it on the side being rebuilt.
    let mut drift: i64 = 0;
    for (index, selection) in hunks {
        let hunk = file.hunks.get(*index).ok_or_else(|| CoralError::Protocol {
            label: "apply",
            detail: format!("hunk {index} does not exist"),
        })?;
        let rendered = render_hunk(hunk, selection, direction, &mut drift);
        out.extend_from_slice(&rendered);
    }
    Ok(out)
}

/// Writes one side of a patch header — `a/<path>` — quoted the way git quotes it.
///
/// git splits `diff --git a/x b/y` on a space and reads `--- ` up to a tab or the end of the
/// line, so a path holding a tab, a newline or a quote makes a header that no longer says what
/// it means: `git apply` answered "diff header lacks filename information" and staged nothing.
///
/// Spaces are deliberately left alone. git does not quote them either, and resolves the
/// ambiguity from the `---` and `+++` lines instead; quoting them here would differ from every
/// patch git writes for no gain. Bytes above 0x7f are left alone for the same reason: `core.
/// quotePath` decides whether git escapes them, and its reader accepts either.
fn side(prefix: u8, path: &[u8], out: &mut BString) {
    let mut token = vec![prefix, b'/'];
    token.extend_from_slice(path);
    if !token.iter().any(|b| needs_quoting(*b)) {
        out.extend_from_slice(&token);
        return;
    }
    out.push(b'"');
    for byte in token {
        escape(byte, out);
    }
    out.push(b'"');
}

const fn needs_quoting(byte: u8) -> bool {
    byte == b'"' || byte == b'\\' || byte < 0x20 || byte == 0x7f
}

/// One byte in C-quoted form, as `quote_c_style` writes it.
fn escape(byte: u8, out: &mut BString) {
    let named: &[u8] = match byte {
        b'"' => b"\\\"",
        b'\\' => b"\\\\",
        0x07 => b"\\a",
        0x08 => b"\\b",
        0x09 => b"\\t",
        0x0a => b"\\n",
        0x0b => b"\\v",
        0x0c => b"\\f",
        0x0d => b"\\r",
        b if b < 0x20 || b == 0x7f => {
            out.extend_from_slice(format!("\\{b:03o}").as_bytes());
            return;
        }
        b => {
            out.push(b);
            return;
        }
    };
    out.extend_from_slice(named);
}

/// Emits one hunk with only `selection` applied.
fn render_hunk(
    hunk: &Hunk,
    selection: &Selection,
    direction: Direction,
    drift: &mut i64,
) -> BString {
    let mut body: Vec<u8> = Vec::new();
    let mut old_count: u32 = 0;
    let mut new_count: u32 = 0;

    let reversed = direction != Direction::Stage;
    for (i, line) in hunk.lines.iter().enumerate() {
        let selected = selection.includes(i);
        let (marker, counts_old, counts_new) = match (line.kind, selected) {
            (LineKind::Add, true) => (b'+', false, true),
            (LineKind::Remove, true) => (b'-', true, false),
            // An unselected line belongs in the patch only when the side git will match it
            // against already holds it, and which side that is depends on the direction.
            //
            // Staging applies forwards, against the index: a removal is still in it and is
            // context, an addition never reached it and is left out. Unstaging and discarding
            // apply in reverse, against the side the diff calls new — the index and the working
            // tree — and the two swap over. Emitted the other way round, the patch describes a
            // file that does not exist, and git refused every line-by-line unstage and discard
            // with "patch does not apply".
            (LineKind::Add, false) if !reversed => continue,
            (LineKind::Remove, false) if reversed => continue,
            (LineKind::Context, _) | (LineKind::Add | LineKind::Remove, false) => {
                (b' ', true, true)
            }
        };

        body.push(marker);
        body.extend_from_slice(&line.text);
        body.push(b'\n');
        if line.no_newline {
            body.extend_from_slice(b"\\ No newline at end of file\n");
        }
        old_count += u32::from(counts_old);
        new_count += u32::from(counts_new);
    }

    // Nothing changed once the selection was applied; emitting it would be a no-op hunk that
    // `git apply` rejects as empty.
    if old_count == new_count && !body.iter().any(|b| *b == b'+' || *b == b'-') {
        return BString::from(Vec::new());
    }

    // Applying in reverse means git reads the header the other way round, so the start we must
    // shift is the one that side is rebuilding.
    let (old_start, new_start) = match direction {
        Direction::Stage => (
            hunk.old_start,
            u32::try_from(i64::from(hunk.old_start) + *drift).unwrap_or(hunk.new_start),
        ),
        // Both read the header as git will when applying in reverse: the side being rebuilt
        // is the one the patch already describes.
        Direction::Unstage | Direction::Discard => (hunk.old_start, hunk.new_start),
    };
    *drift += i64::from(new_count) - i64::from(old_count);

    let header = format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@\n");
    let mut out = BString::from(header.into_bytes());
    out.extend_from_slice(&body);
    out
}

/// The arguments `git apply` needs for a generated patch.
///
/// `--unidiff-zero` is required because a selection can leave a hunk with no context lines at
/// all, which git otherwise refuses.
///
/// Discarding is the one that leaves the index alone: it takes the change back out of the
/// working tree, which is why it has no `--cached` and why it is the only one of the three
/// that cannot be undone.
#[must_use]
pub fn apply_args(direction: Direction) -> Vec<&'static str> {
    let mut v = vec!["apply", "--unidiff-zero", "--whitespace=nowarn"];
    if direction != Direction::Discard {
        v.insert(1, "--cached");
    }
    if direction != Direction::Stage {
        v.push("--reverse");
    }
    v
}

/// True when the patch would do nothing, so the caller can skip spawning git.
#[must_use]
pub fn is_empty_patch(patch: &BString) -> bool {
    !patch.lines().any(|l| l.starts_with(b"@@"))
}
