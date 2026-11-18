use std::collections::HashMap;

use bstr::{BString, ByteSlice};

use crate::commit::Signature;
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

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
    /// Where this commit's lines were before it, which is where blaming further back starts.
    /// None for a commit that created them.
    pub previous: Option<BlamePrevious>,
    /// Whether the walk stopped here rather than finding who wrote these lines: the root of a
    /// shallow clone, or a commit outside the range asked about. Nothing is before it to blame.
    pub boundary: bool,
}

/// The commit and path a blamed commit's lines came from.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct BlamePrevious {
    pub oid: String,
    /// Different from the blamed commit's own after a rename.
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
    /// Which commits were looked past, so the reader knows a reformat is not who they see.
    pub skipping: Skipping,
}

/// Whether a blame looks past the commits a repository lists in `.git-blame-ignore-revs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub enum ListedCommits {
    Skip,
    Blame,
}

/// Which commits a blame looked past, and why.
///
/// <https://git-scm.com/docs/git-blame#Documentation/git-blame.txt---ignore-revs-fileltfilegt>
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Skipping {
    /// None: every commit is named.
    #[default]
    Nothing,
    /// The repository's `.git-blame-ignore-revs`, which lists `count` commits.
    Listed { count: u32 },
    /// The same list, offered and turned down.
    Offered { count: u32 },
    /// The list `blame.ignoreRevsFile` names in the user's own configuration.
    Configured { file: String },
    /// `blame.ignoreRevsFile` names a file that is not there. git refuses the whole blame over
    /// it, so that file is left out rather than the blame failing.
    Missing { file: String },
}

/// The list a repository keeps of commits blame should look past: reformats, mostly.
const LISTED: &str = ".git-blame-ignore-revs";

impl RepoLocation {
    /// What a blame looks past, and the arguments that make it do so.
    ///
    /// The user's own `blame.ignoreRevsFile` wins, as it would in a terminal. Without one, the
    /// repository's list is used unless the reader has turned it down; git itself only reads
    /// it when configured to, which most clones never are.
    ///
    /// # Errors
    /// Propagates git failures.
    pub(crate) async fn skips(
        &self,
        runner: &GitRunner,
        listed: ListedCommits,
    ) -> Result<(Vec<String>, Skipping), CoralError> {
        let Some(root) = self.workdir.clone() else {
            return Ok((Vec::new(), Skipping::Nothing));
        };
        let out = runner
            .output_allowing(
                GitCommand::read("config", self.display_path()).args([
                    "config",
                    "--type=path",
                    "--get-all",
                    "blame.ignoreRevsFile",
                ]),
                &[1],
            )
            .await?;
        let configured: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        if let Some(first) = configured.first() {
            let (there, gone): (Vec<String>, Vec<String>) = configured
                .iter()
                .cloned()
                .partition(|f| root.join(f).is_file());
            let Some(missing) = gone.into_iter().next() else {
                return Ok((
                    Vec::new(),
                    Skipping::Configured {
                        file: first.clone(),
                    },
                ));
            };
            // `--no-ignore-revs-file` clears what the configuration added, which git reads
            // before the command line; the files that are there are given back after it.
            let mut args = vec!["--no-ignore-revs-file".to_owned()];
            for file in there {
                args.push("--ignore-revs-file".to_owned());
                args.push(file);
            }
            return Ok((args, Skipping::Missing { file: missing }));
        }
        let Ok(list) = std::fs::read_to_string(root.join(LISTED)) else {
            return Ok((Vec::new(), Skipping::Nothing));
        };
        let count = u32::try_from(
            list.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .count(),
        )
        .unwrap_or(u32::MAX);
        Ok(match (count, listed) {
            (0, _) => (Vec::new(), Skipping::Nothing),
            (_, ListedCommits::Skip) => (
                vec!["--ignore-revs-file".to_owned(), LISTED.to_owned()],
                Skipping::Listed { count },
            ),
            (_, ListedCommits::Blame) => (Vec::new(), Skipping::Offered { count }),
        })
    }
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
    previous: Option<BlamePrevious>,
    boundary: bool,
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
            b"boundary" => p.boundary = true,
            // "<oid> <path>", and the path runs to the end of the line, spaces and all.
            b"previous" => {
                let (oid, filename) = split_kv(value);
                p.previous = Some(BlamePrevious {
                    oid: String::from_utf8_lossy(oid).into_owned(),
                    filename: BString::from(filename),
                });
            }
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
                    previous: p.previous,
                    boundary: p.boundary,
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
