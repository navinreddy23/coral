use bstr::{BString, ByteSlice};

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// One region of a conflicted file.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Block {
    /// Both sides agree.
    Common {
        #[serde(serialize_with = "lines_as_strings")]
        #[cfg_attr(feature = "ts", ts(type = "string[]"))]
        lines: Vec<BString>,
    },
    /// The sides disagree. `base` is empty when the file was added on both sides.
    Conflict {
        #[serde(serialize_with = "lines_as_strings")]
        #[cfg_attr(feature = "ts", ts(type = "string[]"))]
        base: Vec<BString>,
        #[serde(serialize_with = "lines_as_strings")]
        #[cfg_attr(feature = "ts", ts(type = "string[]"))]
        ours: Vec<BString>,
        #[serde(serialize_with = "lines_as_strings")]
        #[cfg_attr(feature = "ts", ts(type = "string[]"))]
        theirs: Vec<BString>,
    },
}

fn lines_as_strings<S: serde::Serializer>(v: &[BString], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq as _;
    let mut seq = s.serialize_seq(Some(v.len()))?;
    for line in v {
        seq.serialize_element(&line.to_str_lossy())?;
    }
    seq.end()
}

/// A conflicted file broken into blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Blocks {
    pub blocks: Vec<Block>,
}

impl Blocks {
    /// How many regions actually need a decision.
    #[must_use]
    pub fn conflict_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| matches!(b, Block::Conflict { .. }))
            .count()
    }

    /// Parses `git merge-file --diff3` output.
    ///
    /// The markers are fixed because `--diff3` is passed explicitly, so the user's
    /// `merge.conflictStyle` cannot change what we read.
    ///
    /// # Errors
    /// [`CoralError::Protocol`] if a marker appears out of order, which would mean the output
    /// is not what we asked for.
    pub fn parse(input: &[u8]) -> Result<Self, CoralError> {
        let mut blocks = Vec::new();
        let mut common: Vec<BString> = Vec::new();
        let mut section: Option<(Vec<BString>, Vec<BString>, Vec<BString>)> = None;
        let mut side = Side::Ours;

        for line in split_lines(input) {
            match marker(&line) {
                Some(Marker::Start) => {
                    if section.is_some() {
                        return Err(protocol("nested conflict start"));
                    }
                    if !common.is_empty() {
                        blocks.push(Block::Common {
                            lines: std::mem::take(&mut common),
                        });
                    }
                    section = Some((Vec::new(), Vec::new(), Vec::new()));
                    side = Side::Ours;
                }
                Some(Marker::Base) => {
                    if section.is_none() {
                        return Err(protocol("base marker outside a conflict"));
                    }
                    side = Side::Base;
                }
                Some(Marker::Separator) => {
                    if section.is_none() {
                        return Err(protocol("separator outside a conflict"));
                    }
                    side = Side::Theirs;
                }
                Some(Marker::End) => {
                    let Some((ours, base, theirs)) = section.take() else {
                        return Err(protocol("conflict end without a start"));
                    };
                    blocks.push(Block::Conflict { base, ours, theirs });
                }
                None => match (&mut section, side) {
                    (Some((ours, _, _)), Side::Ours) => ours.push(line),
                    (Some((_, base, _)), Side::Base) => base.push(line),
                    (Some((_, _, theirs)), Side::Theirs) => theirs.push(line),
                    (None, _) => common.push(line),
                },
            }
        }

        if section.is_some() {
            return Err(protocol("unterminated conflict"));
        }
        if !common.is_empty() {
            blocks.push(Block::Common { lines: common });
        }
        Ok(Self { blocks })
    }

    /// Renders the blocks back to a file, taking one side of every conflict.
    #[must_use]
    pub fn render_taking(&self, side: Take) -> BString {
        let mut out: Vec<u8> = Vec::new();
        for block in &self.blocks {
            let lines = match (block, side) {
                (Block::Common { lines }, _) => lines,
                (Block::Conflict { ours, .. }, Take::Ours) => ours,
                (Block::Conflict { theirs, .. }, Take::Theirs) => theirs,
                (Block::Conflict { base, .. }, Take::Base) => base,
            };
            for line in lines {
                out.extend_from_slice(line);
                out.push(b'\n');
            }
        }
        BString::from(out)
    }
}

/// Which side to take when resolving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    Ours,
    Theirs,
    Base,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Ours,
    Base,
    Theirs,
}

enum Marker {
    Start,
    Base,
    Separator,
    End,
}

/// Markers are exactly seven characters, optionally followed by a space and a label.
///
/// A file with CRLF endings keeps its carriage return on every line, and git writes its markers
/// into such a file the same way: the separator arrives as `=======\r`. Read as content, it
/// took the whole incoming side with it — the pane showed nothing on that side, and taking
/// either side wrote a file with the other one's lines missing or a stray marker left in it.
fn marker(line: &BString) -> Option<Marker> {
    let m = |p: &[u8]| {
        line.starts_with(p)
            && match line.get(7) {
                None | Some(&b' ') => true,
                // Only as the end of the line, never inside one.
                Some(&b'\r') => line.len() == 8,
                Some(_) => false,
            }
    };
    if m(b"<<<<<<<") {
        Some(Marker::Start)
    } else if m(b"|||||||") {
        Some(Marker::Base)
    } else if m(b"=======") {
        Some(Marker::Separator)
    } else if m(b">>>>>>>") {
        Some(Marker::End)
    } else {
        None
    }
}

/// Splits on newlines without inventing a trailing empty line for a file that ends in one.
fn split_lines(input: &[u8]) -> Vec<BString> {
    let trimmed = input.strip_suffix(b"\n").unwrap_or(input);
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed.split(|b| *b == b'\n').map(BString::from).collect()
}

fn protocol(detail: &str) -> CoralError {
    CoralError::Protocol {
        label: "merge-file",
        detail: detail.to_owned(),
    }
}

impl RepoLocation {
    /// Rebuilds a conflicted file's blocks from the index stages.
    ///
    /// The worktree file is deliberately not parsed. Its markers depend on the user's
    /// `merge.conflictStyle`, and the default style merges adjacent conflicts into one large
    /// region with no base at all. Regenerating from stages 1, 2 and 3 with an explicit
    /// `--diff3` gives git's own hunk boundaries and a real base, whatever the user configured.
    ///
    /// # Errors
    /// [`CoralError::Refused`] if the path is not conflicted; otherwise propagates git
    /// failures.
    pub async fn conflict_blocks(
        &self,
        runner: &GitRunner,
        path: &str,
    ) -> Result<Blocks, CoralError> {
        let stages = self.stage_blobs(runner, path).await?;
        if stages.ours.is_none() && stages.theirs.is_none() {
            return Err(CoralError::Refused {
                label: "show conflict",
                detail: format!("{path} is not conflicted"),
            });
        }

        let dir = tempfile::tempdir()?;
        let write =
            |name: &str, content: Option<&BString>| -> Result<std::path::PathBuf, CoralError> {
                let p = dir.path().join(name);
                std::fs::write(&p, content.map_or(&[][..], |c| c.as_slice()))?;
                Ok(p)
            };
        let ours = write("ours", stages.ours.as_ref())?;
        let base = write("base", stages.base.as_ref())?;
        let theirs = write("theirs", stages.theirs.as_ref())?;

        let out = runner
            .output(
                GitCommand::read("merge-file", self.display_path())
                    .args([
                        "merge-file",
                        "-p",
                        "--diff3",
                        "-L",
                        "ours",
                        "-L",
                        "base",
                        "-L",
                        "theirs",
                    ])
                    .arg(&ours)
                    .arg(&base)
                    .arg(&theirs),
            )
            .await;

        // merge-file exits with the number of conflicts, so non-zero is the normal case.
        let stdout = match out {
            Ok(o) => o.stdout,
            Err(CoralError::GitExit { code, .. }) if code > 0 => {
                self.merge_file_output(runner, &ours, &base, &theirs)
                    .await?
            }
            Err(e) => return Err(e),
        };
        Blocks::parse(&stdout)
    }

    /// Re-runs merge-file accepting its conflict count as success.
    async fn merge_file_output(
        &self,
        runner: &GitRunner,
        ours: &std::path::Path,
        base: &std::path::Path,
        theirs: &std::path::Path,
    ) -> Result<Vec<u8>, CoralError> {
        let cmd = GitCommand::read("merge-file", self.display_path())
            .args([
                "merge-file",
                "-p",
                "--diff3",
                "-L",
                "ours",
                "-L",
                "base",
                "-L",
                "theirs",
            ])
            .arg(ours)
            .arg(base)
            .arg(theirs);

        let mut buf = Vec::new();
        runner
            .stream(cmd, b'\n', |line| {
                buf.extend_from_slice(line);
                buf.push(b'\n');
                Ok(crate::process::Sink::Continue)
            })
            .await
            .or_else(|e| match e {
                // Still the conflict count, not a failure.
                CoralError::GitExit { code, .. } if code > 0 => Ok(crate::process::GitStream {
                    records: 0,
                    stopped_early: false,
                }),
                other => Err(other),
            })?;
        Ok(buf)
    }
}

/// The three index stages of a conflicted path. Any may be absent: an add/add conflict has no
/// base, and a delete/modify has no content on the deleting side.
#[derive(Clone, Debug, Default)]
pub struct Stages {
    pub base: Option<BString>,
    pub ours: Option<BString>,
    pub theirs: Option<BString>,
}

impl RepoLocation {
    /// Reads stages 1, 2 and 3 of a conflicted path.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn stage_blobs(&self, runner: &GitRunner, path: &str) -> Result<Stages, CoralError> {
        let mut stages = Stages::default();
        for (n, slot) in [
            (1, &mut stages.base),
            (2, &mut stages.ours),
            (3, &mut stages.theirs),
        ] {
            let out = runner
                .output(
                    GitCommand::read("cat-file", self.display_path())
                        .args(["cat-file", "blob"])
                        .arg(format!(":{n}:{path}")),
                )
                .await;
            match out {
                Ok(o) => *slot = Some(BString::from(o.stdout)),
                // A missing stage is normal, not a failure.
                Err(CoralError::GitExit { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(stages)
    }
}
