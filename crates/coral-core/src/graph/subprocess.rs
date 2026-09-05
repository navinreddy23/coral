use std::path::{Path, PathBuf};

use gix::ObjectId;
use smallvec::SmallVec;

use super::stream::{CommitNode, CommitStream, Order, StreamOpts, Tips, WalkControl, WalkStats};
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner, Sink};

/// Reads the DAG by streaming `git rev-list`.
///
/// This is the oracle the gix implementation is tested against, and the fallback if gix ever
/// regresses or lacks a feature. It is the one place a non-`-z` format is parsed, which is
/// safe because every field is fixed-width hex or a decimal integer: no field can contain the
/// space that separates them.
pub struct SubprocessCommitStream {
    runner: GitRunner,
    workdir: PathBuf,
}

impl SubprocessCommitStream {
    /// Binds a runner to a repository.
    #[must_use]
    pub fn new(runner: GitRunner, workdir: &Path) -> Self {
        Self {
            runner,
            workdir: workdir.to_path_buf(),
        }
    }

    /// Whether HEAD is on no branch.
    ///
    /// Only asked when something is hidden, and only because `--all` cannot answer it. `--all`
    /// means "every ref, and HEAD", and `--exclude` filters the refs but not the HEAD it adds,
    /// so hiding the checked-out branch left every one of its commits on screen. `--glob=refs/*`
    /// is the same set without that HEAD, which makes the exclusion work — and then leaves a
    /// detached HEAD, reachable from no ref at all, as the one tip that has to be named again.
    fn head_detached(&self) -> bool {
        let cmd = GitCommand::read("rev-parse", &self.workdir)
            .arg("rev-parse")
            .arg("--symbolic-full-name")
            .arg("HEAD");
        let mut detached = false;
        let read = self.runner.stream_blocking(&cmd, b'\n', |line| {
            detached = line == b"HEAD";
            Ok(Sink::Stop)
        });
        read.is_ok() && detached
    }

    fn command(&self, opts: &StreamOpts) -> GitCommand {
        let mut cmd = GitCommand::read("rev-list", &self.workdir)
            .arg("rev-list")
            .arg("--parents")
            .arg("--timestamp");

        cmd = match opts.order {
            Order::Topological => cmd.arg("--topo-order"),
            Order::CommitTime => cmd.arg("--date-order"),
        };
        if opts.first_parent {
            cmd = cmd.arg("--first-parent");
        }
        if let Some(n) = opts.max_count {
            cmd = cmd.arg(format!("--max-count={n}"));
        }
        // git's own vocabulary for the same thing, which is what keeps this an oracle rather
        // than a second implementation: `--exclude` applies to the `--all` that follows it, so
        // the order of these two arguments is the whole of their meaning.
        match &opts.tips {
            Tips::All => cmd.arg("--all"),
            Tips::Except(names) => {
                for name in names {
                    cmd = cmd.arg(format!("--exclude={name}"));
                }
                cmd = cmd.arg("--glob=refs/*");
                if self.head_detached() {
                    cmd = cmd.arg("HEAD");
                }
                cmd
            }
            // A name the repository no longer has is skipped, not refused: these come from a
            // choice the user made before the branch was deleted, and rev-list's default is to
            // exit 128 on one. The gix walk skips it, so without this the two disagree.
            Tips::Only(names) => cmd
                .arg("--ignore-missing")
                .args(names.iter().map(String::as_str)),
        }
    }
}

/// Parses one `<timestamp> <oid> <parent-oid>...` line.
fn parse_line(line: &[u8]) -> Result<CommitNode, CoralError> {
    let text =
        std::str::from_utf8(line).map_err(|_| protocol("rev-list emitted a non-UTF-8 line"))?;
    let mut fields = text.split(' ');

    let commit_time: i64 = fields
        .next()
        .and_then(|f| f.parse().ok())
        .ok_or_else(|| protocol("missing or malformed timestamp"))?;
    let id = fields
        .next()
        .and_then(|f| ObjectId::from_hex(f.as_bytes()).ok())
        .ok_or_else(|| protocol("missing or malformed commit id"))?;

    let mut parents = SmallVec::new();
    for f in fields {
        parents
            .push(ObjectId::from_hex(f.as_bytes()).map_err(|_| protocol("malformed parent id"))?);
    }
    Ok(CommitNode {
        id,
        parents,
        commit_time,
    })
}

fn protocol(detail: &str) -> CoralError {
    CoralError::Protocol {
        label: "rev-list",
        detail: detail.to_owned(),
    }
}

impl CommitStream for SubprocessCommitStream {
    fn name(&self) -> &'static str {
        "rev-list"
    }

    fn walk(
        &self,
        opts: &StreamOpts,
        sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    ) -> Result<WalkStats, CoralError> {
        // `rev-list` with no revision argument is a usage error, not an empty answer, and
        // soloing nothing is a legitimate state to pass through on the way to soloing something.
        if opts.tips == Tips::Only(Vec::new()) {
            return Ok(WalkStats::default());
        }
        let cmd = self.command(opts);
        let mut stats = WalkStats::default();

        self.runner.stream_blocking(&cmd, b'\n', |line| {
            if line.is_empty() {
                return Ok(Sink::Continue);
            }
            let mut node = parse_line(line)?;
            // git prints every parent even under --first-parent, but the walk only visits the
            // first, so keeping the rest would leave edges pointing at rows that never arrive.
            if opts.first_parent {
                node.parents.truncate(1);
            }
            stats.commits += 1;
            stats.edges += node.parents.len() as u64;
            Ok(if sink(node) == WalkControl::Stop {
                Sink::Stop
            } else {
                Sink::Continue
            })
        })?;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_merge_a_normal_commit_and_a_root() {
        let merge = parse_line(b"1788390122 940de590b839f71d6dc846160534bf202401b8b7 89a312991dc6e638a36adc43ccb91dbc25504c04 2625480a1bf79c62ffb09aafdf61778e682da492").unwrap();
        assert_eq!(merge.commit_time, 1_788_390_122);
        assert_eq!(merge.parents.len(), 2);

        let normal = parse_line(b"1788303695 2625480a1bf79c62ffb09aafdf61778e682da492 cee9395acd8043be0644b25c34bfa86623f2b935").unwrap();
        assert_eq!(normal.parents.len(), 1);

        let root = parse_line(b"1456236215 a101ad945113be3d7f283a181810d76897f0a0d6").unwrap();
        assert!(root.parents.is_empty());
        assert_eq!(
            root.id.to_string(),
            "a101ad945113be3d7f283a181810d76897f0a0d6"
        );
    }

    #[test]
    fn rejects_malformed_lines_rather_than_guessing() {
        assert!(parse_line(b"").is_err());
        assert!(parse_line(b"notanumber 940de590b839f71d6dc846160534bf202401b8b7").is_err());
        assert!(parse_line(b"1788390122 nothex").is_err());
        assert!(parse_line(b"1788390122 940de590b839f71d6dc846160534bf202401b8b7 zzz").is_err());
    }
}
