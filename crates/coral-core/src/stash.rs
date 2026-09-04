//! The stash stack, as a list rather than as one ref.
//!
//! `refs/stash` is the top of the stack and the only stash git keeps a ref for; everything
//! below it lives in that ref's reflog. Listing refs therefore finds exactly one stash however
//! many there are, and calls it "stash" — which is also what it calls the next one.

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// One entry on the stash stack.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    /// Position on the stack, which is what `stash@{n}` means and what apply and drop take.
    pub index: usize,
    pub oid: String,
    /// The branch it was made on, when git recorded one.
    pub branch: Option<String>,
    /// What git wrote, with the "WIP on <branch>: " it puts in front removed.
    pub message: String,
    /// Seconds since the epoch.
    pub time: i64,
}

impl StashEntry {
    /// A name no other entry on the stack can have.
    ///
    /// git's own subject is not one: stashing twice from the same commit writes "WIP on
    /// master: 1a2b3c4 the message" both times, and a list of identical names is a list nobody
    /// can act on. The stash's own object id is the thing that differs.
    #[must_use]
    pub fn name(&self) -> String {
        let short: String = self.oid.chars().take(7).collect();
        match &self.branch {
            Some(branch) => format!("{branch}@{short}"),
            None => short,
        }
    }
}

/// The format [`parse`] expects: object id, selector, subject, commit time.
pub const FORMAT: &str = "%H%x00%gd%x00%gs%x00%ct";

/// Parses `git stash list --format=<FORMAT>`.
///
/// # Errors
/// [`CoralError::Protocol`] if a record does not have the expected fields, or if its selector
/// is not one this can find again. A stash whose position is unknown must not be listed:
/// dropping the wrong one destroys work that has no other copy.
pub fn parse(input: &[u8]) -> Result<Vec<StashEntry>, CoralError> {
    input
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(parse_line)
        .collect()
}

fn parse_line(line: &[u8]) -> Result<StashEntry, CoralError> {
    let text = String::from_utf8_lossy(line);
    let f: Vec<&str> = text.split('\0').collect();
    if f.len() < 4 {
        return Err(CoralError::Protocol {
            label: "stash list",
            detail: format!("expected 4 fields, got {}", f.len()),
        });
    }

    let index = selector_index(f[1]).ok_or_else(|| CoralError::Protocol {
        label: "stash list",
        detail: format!("cannot read a position from {:?}", f[1]),
    })?;
    let (branch, message) = split_subject(f[2]);

    Ok(StashEntry {
        index,
        oid: f[0].to_owned(),
        branch,
        message,
        time: f[3].parse().unwrap_or_default(),
    })
}

/// The `n` in `stash@{n}`.
fn selector_index(selector: &str) -> Option<usize> {
    selector
        .strip_prefix("stash@{")?
        .strip_suffix('}')?
        .parse()
        .ok()
}

/// Splits git's subject into the branch it names and what is left.
///
/// Two shapes, and they are git's: `WIP on <branch>: <commit> <subject>` for a stash made with
/// no message, `On <branch>: <message>` for one made with. Anything else is kept whole rather
/// than guessed at.
fn split_subject(subject: &str) -> (Option<String>, String) {
    for prefix in ["WIP on ", "On "] {
        if let Some(rest) = subject.strip_prefix(prefix)
            && let Some((branch, message)) = rest.split_once(": ")
        {
            return (Some(branch.to_owned()), message.to_owned());
        }
    }
    (None, subject.to_owned())
}

impl RepoLocation {
    /// Every entry on the stash stack, newest first, as git orders them.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn stashes(&self, runner: &GitRunner) -> Result<Vec<StashEntry>, CoralError> {
        let out = runner
            .output(GitCommand::read("stash-list", self.display_path()).args([
                "stash",
                "list",
                &format!("--format={FORMAT}"),
            ]))
            .await?;
        parse(&out.stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::{StashEntry, parse};

    fn record(oid: &str, selector: &str, subject: &str, time: &str) -> Vec<u8> {
        format!("{oid}\0{selector}\0{subject}\0{time}\n").into_bytes()
    }

    #[test]
    fn reads_the_branch_out_of_both_subjects_git_writes() {
        let mut input = record("aaa", "stash@{0}", "WIP on master: 1a2b3c4 a commit", "10");
        input.extend(record(
            "bbb",
            "stash@{1}",
            "On topic: something I typed",
            "20",
        ));

        let entries = parse(&input).unwrap();
        assert_eq!(entries[0].branch.as_deref(), Some("master"));
        assert_eq!(entries[0].message, "1a2b3c4 a commit");
        assert_eq!(entries[1].branch.as_deref(), Some("topic"));
        assert_eq!(entries[1].message, "something I typed");
    }

    #[test]
    fn keeps_a_subject_it_does_not_recognise_whole() {
        // Guessing at a shape git did not write would put half a message in the branch column.
        let input = record("aaa", "stash@{0}", "something else entirely", "10");
        let entries = parse(&input).unwrap();
        assert_eq!(entries[0].branch, None);
        assert_eq!(entries[0].message, "something else entirely");
    }

    #[test]
    fn names_two_stashes_from_the_same_commit_apart() {
        // The case that made the list unusable: stashing twice from one commit writes the same
        // subject both times, so the subject cannot be the name.
        let subject = "WIP on master: 1a2b3c4 the same commit";
        let mut input = record(
            "78d0a2dd1a00b8e06286b02bac7ca3811a7041fc",
            "stash@{0}",
            subject,
            "20",
        );
        input.extend(record(
            "21b55ebccffb3a1b09c67b16ef4b009cce430e5e",
            "stash@{1}",
            subject,
            "10",
        ));

        let entries = parse(&input).unwrap();
        assert_eq!(entries[0].name(), "master@78d0a2d");
        assert_eq!(entries[1].name(), "master@21b55eb");
        assert_ne!(entries[0].name(), entries[1].name());
    }

    #[test]
    fn a_stash_with_no_branch_is_named_by_its_object_alone() {
        let entry = StashEntry {
            index: 0,
            oid: "abcdef1234567890".to_owned(),
            branch: None,
            message: String::new(),
            time: 0,
        };
        assert_eq!(entry.name(), "abcdef1");
    }

    #[test]
    fn refuses_a_record_whose_position_it_cannot_read() {
        // The position is what drop and apply are given. Listing an entry whose position is a
        // guess would mean destroying work that has no other copy.
        for selector in ["stash@{}", "stash@0", "", "refs/stash"] {
            let input = record("aaa", selector, "WIP on master: x", "10");
            assert!(parse(&input).is_err(), "{selector:?}");
        }
    }

    #[test]
    fn reads_the_position_from_the_selector_rather_than_the_order() {
        let mut input = record("aaa", "stash@{3}", "WIP on master: x", "10");
        input.extend(record("bbb", "stash@{7}", "WIP on master: y", "20"));
        let entries = parse(&input).unwrap();
        assert_eq!(entries[0].index, 3);
        assert_eq!(entries[1].index, 7);
    }

    #[test]
    fn an_empty_stack_is_an_empty_list() {
        assert!(parse(b"").unwrap().is_empty());
    }
}
