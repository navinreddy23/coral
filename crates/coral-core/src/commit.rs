use bstr::BString;

/// A person and when they acted. Author and committer are shown separately in the UI, because
/// a rebased or cherry-picked commit has two different answers.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Seconds since the epoch.
    ///
    /// Declared as a number rather than ts-rs's default `bigint` for `i64`: serde writes it as
    /// a JSON number, so `bigint` would describe something the wire never carries. Seconds are
    /// exact in a double until well past the year 200,000.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub time: i64,
}

/// One commit's metadata, without its tree or diff.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: Signature,
    pub committer: Signature,
    /// First line of the message.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub summary: BString,
    /// Everything after the first blank line; empty when there is none.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub body: BString,
}

impl Commit {
    #[must_use]
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// True when the commit was authored and committed by different people, or at different
    /// times — which is what makes showing both worthwhile.
    #[must_use]
    pub fn was_rewritten(&self) -> bool {
        self.author != self.committer
    }
}

/// What the graph shows next to a row: who wrote it and its first line.
///
/// Deliberately smaller than [`Commit`]. The graph needs this for a screenful of rows at a
/// time; carrying full parent lists and bodies would undo the row store's compactness.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CommitMeta {
    pub oid: String,
    pub author: String,
    pub email: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub time: i64,
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub summary: BString,
    /// The start of the message body, for the dimmed continuation the graph shows after the
    /// summary. Truncated because a screenful of full bodies is far more than the row needs.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub body: BString,
}

/// How much of a body is worth sending for a row that will show one line of it.
const BODY_PREVIEW_BYTES: usize = 300;

impl crate::repo::RepoLocation {
    /// Reads author and summary for a window of commits.
    ///
    /// One `cat-file --batch` for the whole window rather than a process per commit: on the
    /// kernel a screenful costs about 60 ms this way. The commit-graph carries neither field,
    /// so this is the only way to get them without inflating every object up front.
    ///
    /// # Errors
    /// Propagates git failures. An object id that does not resolve is skipped rather than
    /// failing the window.
    pub async fn commit_metadata(
        &self,
        runner: &crate::process::GitRunner,
        oids: &[String],
    ) -> Result<Vec<CommitMeta>, crate::error::CoralError> {
        if oids.is_empty() {
            return Ok(Vec::new());
        }
        let mut stdin = Vec::with_capacity(oids.len() * 41);
        for oid in oids {
            stdin.extend_from_slice(oid.as_bytes());
            stdin.push(b'\n');
        }

        let out = runner
            .output(
                crate::process::GitCommand::read("cat-file", self.display_path())
                    .args(["cat-file", "--batch"])
                    .stdin_bytes(stdin),
            )
            .await?;
        Ok(parse_batch(&out.stdout))
    }
}

/// Parses `git cat-file --batch`: `<oid> <type> <size>\n<object>` repeated.
fn parse_batch(input: &[u8]) -> Vec<CommitMeta> {
    let mut out = Vec::new();
    let mut rest = input;

    while let Some(nl) = rest.iter().position(|b| *b == b'\n') {
        let header = &rest[..nl];
        rest = &rest[nl + 1..];

        let mut fields = header.split(|b| *b == b' ');
        let Some(oid) = fields.next() else { break };
        let kind = fields.next().unwrap_or_default();
        let Some(size) = fields.next().and_then(parse_size) else {
            // "<oid> missing" has no size; there is no body to skip.
            continue;
        };
        if size > rest.len() {
            break;
        }
        let (body, tail) = rest.split_at(size);
        // Each object is followed by a newline the size does not count.
        rest = tail.strip_prefix(b"\n").unwrap_or(tail);

        if kind == b"commit" {
            out.push(parse_commit_object(
                String::from_utf8_lossy(oid).into_owned(),
                body,
            ));
        }
    }
    out
}

fn parse_size(field: &[u8]) -> Option<usize> {
    std::str::from_utf8(field).ok()?.trim().parse().ok()
}

/// Reads the author line and the first line of the message from a raw commit object.
fn parse_commit_object(oid: String, body: &[u8]) -> CommitMeta {
    let mut author = String::new();
    let mut email = String::new();
    let mut time = 0_i64;
    let mut summary = BString::from(Vec::new());
    let mut preview = BString::from(Vec::new());

    let mut lines = body.split(|b| *b == b'\n');
    for line in lines.by_ref() {
        // A blank line ends the headers; the message follows.
        if line.is_empty() {
            break;
        }
        // A leading space continues the previous header, as `mergetag` does over many lines.
        if line.first() == Some(&b' ') {
            continue;
        }
        if let Some(rest) = line.strip_prefix(b"author ") {
            (author, email, time) = parse_identity(rest);
        }
    }
    if let Some(first) = lines.next() {
        summary = BString::from(first);

        // Truncated on a character boundary, so a preview cannot split a multi-byte character
        // and render as a replacement glyph.
        let remainder: Vec<u8> = lines.collect::<Vec<_>>().join(&b'\n');
        let cut = floor_char_boundary(&remainder, BODY_PREVIEW_BYTES);
        preview = BString::from(&remainder[..cut]);
    }
    CommitMeta {
        oid,
        author,
        email,
        time,
        summary,
        body: preview,
    }
}

/// `Name <email> 1788390122 -0700`. A name may contain spaces, so the angle brackets are the
/// only reliable delimiter.
fn parse_identity(field: &[u8]) -> (String, String, i64) {
    let text = String::from_utf8_lossy(field);
    let (name, rest) = match text.split_once(" <") {
        Some((n, r)) => (n.trim().to_owned(), r),
        None => (text.trim().to_owned(), ""),
    };
    let (email, rest) = match rest.split_once('>') {
        Some((e, r)) => (e.to_owned(), r),
        None => (String::new(), rest),
    };
    let time = rest
        .split_whitespace()
        .next()
        .and_then(|t| t.parse().ok())
        .unwrap_or(0);
    (name, email, time)
}

/// A file a commit changed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub path: BString,
    #[serde(serialize_with = "crate::bytes::as_str_opt")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub old_path: Option<BString>,
    pub change: crate::diff::FileChange,
}

/// Everything the details panel shows for one commit.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub commit: Commit,
    pub files: Vec<ChangedFile>,
}

impl crate::repo::RepoLocation {
    /// Reads one commit and the files it changed.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn commit_detail(
        &self,
        runner: &crate::process::GitRunner,
        rev: &str,
    ) -> Result<CommitDetail, crate::error::CoralError> {
        let commits = self
            .log(
                runner,
                &crate::history::LogQuery {
                    rev: Some(rev.to_owned()),
                    limit: Some(1),
                    ..crate::history::LogQuery::default()
                },
            )
            .await?;
        let commit =
            commits
                .into_iter()
                .next()
                .ok_or_else(|| crate::error::CoralError::Refused {
                    label: "show commit",
                    detail: format!("{rev} is not a commit"),
                })?;

        Ok(CommitDetail {
            files: self.changed_files(runner, rev).await?,
            commit,
        })
    }

    /// The files one commit changed, against its first parent.
    ///
    /// `-m --first-parent` is needed for both shapes: without `-m` a merge reports nothing at
    /// all, and `--first-parent` is what makes that report the mainline change rather than one
    /// entry per parent. `--root` covers the initial commit, which has no parent to diff.
    async fn changed_files(
        &self,
        runner: &crate::process::GitRunner,
        rev: &str,
    ) -> Result<Vec<ChangedFile>, crate::error::CoralError> {
        let out = runner
            .output(
                crate::process::GitCommand::read("diff-tree", self.display_path())
                    .args([
                        "diff-tree",
                        "-r",
                        "-z",
                        "--name-status",
                        "-M",
                        "--no-commit-id",
                    ])
                    .args(["-m", "--first-parent", "--root"])
                    .arg(rev),
            )
            .await?;
        Ok(parse_name_status(&out.stdout))
    }
}

/// Parses `--name-status -z`: a status field, then one path, or two for a rename or copy.
fn parse_name_status(input: &[u8]) -> Vec<ChangedFile> {
    use crate::diff::FileChange;

    let mut out = Vec::new();
    let mut records = input.split(|b| *b == 0).filter(|r| !r.is_empty());

    while let Some(status) = records.next() {
        let Some(letter) = status.first() else {
            continue;
        };
        let change = match letter {
            b'A' => FileChange::Added,
            b'D' => FileChange::Deleted,
            b'M' | b'T' => FileChange::Modified,
            b'R' => FileChange::Renamed,
            b'C' => FileChange::Copied,
            _ => continue,
        };
        let renamed = matches!(change, FileChange::Renamed | FileChange::Copied);
        let Some(first) = records.next() else { break };
        let (path, old_path) = if renamed {
            let Some(new) = records.next() else { break };
            (BString::from(new), Some(BString::from(first)))
        } else {
            (BString::from(first), None)
        };
        out.push(ChangedFile {
            path,
            old_path,
            change,
        });
    }
    out
}

/// The largest index at or below `at` that does not split a UTF-8 character.
fn floor_char_boundary(bytes: &[u8], at: usize) -> usize {
    if at >= bytes.len() {
        return bytes.len();
    }
    let mut i = at;
    // Continuation bytes are 0b10xxxxxx; step back over them to the leading byte.
    while i > 0 && (bytes[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    i
}
