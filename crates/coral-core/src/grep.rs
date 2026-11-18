//! Searching the files of one revision, through `git grep`.

use bstr::{BString, ByteSlice};

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner, Sink};
use crate::repo::RepoLocation;

/// The most lines one search answers with. A common word in the kernel is on twenty-five
/// thousand lines, and nobody reads past the first few hundred of them.
pub const MOST: usize = 5_000;

/// How the pattern is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub enum GrepPattern {
    /// As it is written, every character itself.
    Fixed,
    /// As an extended regular expression, `git grep -E`.
    Regex,
}

/// What to look for, and where.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct GrepQuery {
    pub pattern: String,
    pub how: GrepPattern,
    pub ignore_case: bool,
    /// Pathspecs to look under; empty for the whole tree.
    pub paths: Vec<String>,
}

/// One line a search found.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct GrepMatch {
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub path: BString,
    pub line: u32,
    /// Where the first match on the line starts, in bytes from one.
    pub column: u32,
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub text: BString,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct GrepResult {
    pub matches: Vec<GrepMatch>,
    /// Whether it stopped at [`MOST`] with more still to find.
    pub more: bool,
}

impl RepoLocation {
    /// The lines of the files at `rev` that match, in path order, up to [`MOST`].
    ///
    /// Binary files are left out (`-I`), as nobody wants the line a search found in an image.
    ///
    /// # Errors
    /// Propagates git failures, which include a pattern git cannot read.
    pub async fn grep(
        &self,
        runner: &GitRunner,
        rev: &str,
        query: &GrepQuery,
    ) -> Result<GrepResult, CoralError> {
        let mut out = GrepResult::default();
        if query.pattern.is_empty() {
            return Ok(out);
        }
        let mut cmd = GitCommand::read("grep", self.display_path())
            .args(["grep", "-n", "-z", "--column", "-I", "--no-color"])
            .arg(match query.how {
                GrepPattern::Fixed => "-F",
                GrepPattern::Regex => "-E",
            });
        if query.ignore_case {
            cmd = cmd.arg("-i");
        }
        let cmd = cmd
            .arg("-e")
            .arg(&query.pattern)
            .arg(rev)
            .arg("--")
            .args(&query.paths);

        // Every line names the revision first: `<rev>:<path>\0<line>\0<column>\0<text>`.
        let prefix = format!("{rev}:");
        let streamed = runner
            .stream(cmd, b'\n', |record| {
                if out.matches.len() == MOST {
                    out.more = true;
                    return Ok(Sink::Stop);
                }
                if let Some(found) = parse(record, prefix.as_bytes()) {
                    out.matches.push(found);
                }
                Ok(Sink::Continue)
            })
            .await;
        match streamed {
            Ok(_) => Ok(out),
            // git's answer for nothing found, and an answer rather than a failure.
            Err(CoralError::GitExit {
                code: 1,
                ref stderr,
                ..
            }) if stderr.trim().is_empty() => Ok(out),
            Err(e) => Err(e),
        }
    }
}

fn parse(record: &[u8], prefix: &[u8]) -> Option<GrepMatch> {
    let mut fields = record.splitn(4, |b| *b == 0);
    let path = fields.next()?;
    let line = fields.next()?.to_str().ok()?.parse().ok()?;
    let column = fields.next()?.to_str().ok()?.parse().ok()?;
    let text = fields.next()?;
    Some(GrepMatch {
        path: BString::from(path.strip_prefix(prefix).unwrap_or(path)),
        line,
        column,
        text: BString::from(text),
    })
}
