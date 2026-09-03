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
    pub time: i64,
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub summary: BString,
}

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
    }
    CommitMeta {
        oid,
        author,
        email,
        time,
        summary,
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
