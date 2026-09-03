use std::collections::HashMap;

use bstr::{BString, ByteSlice};

use crate::commit::Signature;
use crate::error::CoralError;

/// A run of consecutive lines attributed to one commit.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct BlameChunk {
    pub oid: String,
    /// First line in the file as it is now, 1-based.
    pub final_line: u32,
    /// First line in the file as it was at `oid`, 1-based.
    pub orig_line: u32,
    pub lines: u32,
}

/// What a commit contributes to a blame, shared by every chunk that names it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct BlameCommit {
    pub oid: String,
    pub author: Signature,
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub summary: BString,
    /// The path the content had at this commit, which differs after a rename.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub filename: BString,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Blame {
    pub chunks: Vec<BlameChunk>,
    /// Keyed by oid. Git sends a commit's details once and then refers back to it, so this is
    /// far smaller than one entry per chunk on a file with a long history.
    pub commits: HashMap<String, BlameCommit>,
}

impl Blame {
    /// The commit responsible for a 1-based line, if it is covered.
    #[must_use]
    pub fn commit_for_line(&self, line: u32) -> Option<&BlameCommit> {
        let chunk = self
            .chunks
            .iter()
            .find(|c| line >= c.final_line && line < c.final_line + c.lines)?;
        self.commits.get(&chunk.oid)
    }
}

/// Accumulates `git blame --porcelain --incremental` as it streams.
///
/// Incremental output arrives out of order and in bursts, which is the point: the UI paints
/// chunks as they land instead of waiting for a large file to finish.
#[derive(Default)]
pub struct BlameParser {
    blame: Blame,
    pending: Option<Pending>,
}

#[derive(Default)]
struct Pending {
    oid: String,
    orig_line: u32,
    final_line: u32,
    lines: u32,
    name: Option<String>,
    email: Option<String>,
    time: Option<i64>,
    summary: Option<BString>,
    filename: Option<BString>,
}

impl BlameParser {
    /// Feeds one line of porcelain output.
    ///
    /// # Errors
    /// [`CoralError::Protocol`] if a chunk header does not parse.
    pub fn push(&mut self, line: &[u8]) -> Result<(), CoralError> {
        if line.is_empty() {
            return Ok(());
        }
        // A header is "<oid> <orig> <final> <lines>"; every other line is "<key> <value>".
        if let Some(header) = parse_header(line) {
            self.flush();
            self.pending = Some(header);
            return Ok(());
        }

        let (key, value) = split_kv(line);
        let Some(p) = self.pending.as_mut() else {
            return Err(CoralError::Protocol {
                label: "blame",
                detail: "metadata line before any chunk header".to_owned(),
            });
        };
        match key {
            b"author" => p.name = Some(String::from_utf8_lossy(value).into_owned()),
            b"author-mail" => {
                let trimmed = value
                    .trim_start_with(|c| c == '<')
                    .trim_end_with(|c| c == '>');
                p.email = Some(String::from_utf8_lossy(trimmed).into_owned());
            }
            b"author-time" => p.time = value.to_str().ok().and_then(|s| s.parse().ok()),
            b"summary" => p.summary = Some(BString::from(value)),
            b"filename" => p.filename = Some(BString::from(value)),
            _ => {}
        }
        Ok(())
    }

    /// Ends the stream and returns what was gathered.
    #[must_use]
    pub fn finish(mut self) -> Blame {
        self.flush();
        self.blame.chunks.sort_unstable_by_key(|c| c.final_line);
        self.blame
    }

    /// Stores the chunk being built. A commit git has already described arrives with only a
    /// header and a filename, so its details must not be overwritten with blanks.
    fn flush(&mut self) {
        let Some(p) = self.pending.take() else { return };
        self.blame.chunks.push(BlameChunk {
            oid: p.oid.clone(),
            final_line: p.final_line,
            orig_line: p.orig_line,
            lines: p.lines,
        });

        if let (Some(name), Some(email), Some(time), Some(summary)) =
            (p.name, p.email, p.time, p.summary)
        {
            self.blame.commits.insert(
                p.oid.clone(),
                BlameCommit {
                    oid: p.oid,
                    author: Signature { name, email, time },
                    summary,
                    filename: p.filename.unwrap_or_default(),
                },
            );
        }
    }
}

fn parse_header(line: &[u8]) -> Option<Pending> {
    let mut f = line.split(|b| *b == b' ');
    let oid = f.next()?;
    if oid.len() < 40 || !oid.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut num = || {
        f.next()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
    };
    Some(Pending {
        oid: String::from_utf8_lossy(oid).into_owned(),
        orig_line: num()?,
        final_line: num()?,
        lines: num()?,
        ..Pending::default()
    })
}

fn split_kv(line: &[u8]) -> (&[u8], &[u8]) {
    match line.find_byte(b' ') {
        Some(i) => (&line[..i], &line[i + 1..]),
        None => (line, b""),
    }
}
