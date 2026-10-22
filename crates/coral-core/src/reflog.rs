//! Where HEAD has been, newest first, for finding work no branch holds any more.
//!
//! git logs every move of HEAD: a commit, a checkout, a reset, each step of a rebase, an amend.
//! A commit lost to a hard reset, a rebase or a deleted branch is still named there until the
//! log expires it, ninety days by default for a commit nothing else reaches.
//! <https://git-scm.com/docs/git-reflog>

use std::collections::HashSet;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::{Head, RepoLocation};

/// One move of HEAD.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    /// Where HEAD was moved to.
    pub oid: String,
    /// Counted from the newest, as `HEAD@{n}` counts.
    pub index: usize,
    /// What moved it, in git's words: `reset: moving to HEAD~2`.
    pub action: String,
    /// When HEAD moved, in seconds since the epoch, which is not when the commit was made.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub time: i64,
    pub summary: String,
    /// No branch, tag, remote branch or stash reaches it: it is here only because HEAD was.
    pub lost: bool,
}

/// The format [`parse`] expects, read with `--date=unix` so the selector carries the time.
pub const FORMAT: &str = "%H%x1f%gd%x1f%gs%x1f%s";

/// Parses `git log --walk-reflogs --date=unix --format=<FORMAT>`. Nothing is marked lost.
///
/// # Errors
/// [`CoralError::Protocol`] if a record does not have the expected fields.
pub fn parse(input: &[u8]) -> Result<Vec<ReflogEntry>, CoralError> {
    input
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(index, line)| parse_line(index, line))
        .collect()
}

fn parse_line(index: usize, line: &[u8]) -> Result<ReflogEntry, CoralError> {
    let text = String::from_utf8_lossy(line);
    let f: Vec<&str> = text.split('\x1f').collect();
    let [oid, selector, action, summary] = f.as_slice() else {
        return Err(CoralError::Protocol {
            label: "reflog",
            detail: format!("expected 4 fields, got {}", f.len()),
        });
    };
    Ok(ReflogEntry {
        oid: (*oid).to_owned(),
        index,
        action: (*action).to_owned(),
        time: selector_time(selector).unwrap_or_default(),
        summary: (*summary).to_owned(),
        lost: false,
    })
}

/// The `t` in `HEAD@{t}`, which `--date=unix` puts where the position would be.
fn selector_time(selector: &str) -> Option<i64> {
    let (_, rest) = selector.rsplit_once("@{")?;
    rest.strip_suffix('}')?.parse().ok()
}

impl RepoLocation {
    /// HEAD's last `limit` moves, newest first, each marked with whether anything else still
    /// reaches the commit it moved to.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn reflog(
        &self,
        runner: &GitRunner,
        limit: usize,
    ) -> Result<Vec<ReflogEntry>, CoralError> {
        // An unborn HEAD has no log, and `log -g HEAD` calls that an unknown revision.
        if matches!(self.head(runner).await?, Head::Unborn { .. }) {
            return Ok(Vec::new());
        }
        let out = runner
            .output(
                GitCommand::read("reflog", self.display_path())
                    .args(["log", "--walk-reflogs", "--date=unix"])
                    .arg(format!("--format={FORMAT}"))
                    .arg(format!("--max-count={limit}"))
                    .args(["HEAD", "--"]),
            )
            .await?;
        let mut entries = parse(&out.stdout)?;
        let lost = self.unreachable(runner, &entries).await?;
        for entry in &mut entries {
            entry.lost = lost.contains(&entry.oid);
        }
        Ok(entries)
    }

    /// Which of the entries' commits no ref reaches.
    ///
    /// One walk for all of them: everything reachable from the entries and from no ref is
    /// listed, so an entry is lost exactly when it is in the list. `--ignore-missing` because an
    /// entry can outlive its commit when something other than git pruned it.
    async fn unreachable(
        &self,
        runner: &GitRunner,
        entries: &[ReflogEntry],
    ) -> Result<HashSet<String>, CoralError> {
        let oids: HashSet<&str> = entries.iter().map(|e| e.oid.as_str()).collect();
        if oids.is_empty() {
            return Ok(HashSet::new());
        }
        let out = runner
            .output(
                GitCommand::read("reflog-lost", self.display_path())
                    .args(["rev-list", "--ignore-missing"])
                    .args(oids)
                    .args(["--not", "--all"]),
            )
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_when_head_moved_and_what_moved_it() {
        let out = b"aaaa\x1fHEAD@{1761000300}\x1freset: moving to HEAD~1\x1fone\n\
                    bbbb\x1fHEAD@{1761000200}\x1fcommit: two\x1ftwo\n";
        let entries = parse(out).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].oid, "aaaa");
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].action, "reset: moving to HEAD~1");
        assert_eq!(entries[0].time, 1_761_000_300);
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].summary, "two");
    }

    #[test]
    fn a_record_short_of_fields_is_refused() {
        assert!(parse(b"aaaa\x1fHEAD@{1}\n").is_err());
    }
}
