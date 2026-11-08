//! The people who have written in a repository lately, for naming a co-author.

use bstr::ByteSlice;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// How far back to look. Enough to find everyone working on a repository now, and a bounded
/// read on one with a million commits, where asking for every author ever takes seconds.
const LATELY: usize = 2000;

/// Somebody who wrote commits here, as `.mailmap` names them.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub name: String,
    pub email: String,
    /// How many of the recent commits are theirs, which is the order they are offered in.
    pub commits: u32,
}

impl RepoLocation {
    /// Everyone who wrote one of the last few thousand commits on any branch, most active first.
    ///
    /// `%aN` and `%aE` are the names after `.mailmap`, so somebody who has committed under two
    /// addresses is offered once, as the project knows them.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn recent_authors(&self, runner: &GitRunner) -> Result<Vec<Person>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("log", self.display_path())
                    .args(["log", "--all", "--no-merges", "-z", "--format=%aN%x00%aE"])
                    .arg(format!("--max-count={LATELY}")),
            )
            .await?;
        Ok(tally(&out.stdout))
    }
}

fn tally(raw: &[u8]) -> Vec<Person> {
    let fields: Vec<&[u8]> = raw.split(|b| *b == 0).collect();
    let mut people: Vec<Person> = Vec::new();
    for [name, email] in fields.as_chunks::<2>().0 {
        let (name, email) = (name.to_str_lossy(), email.to_str_lossy());
        let (name, email) = (name.trim(), email.trim());
        if email.is_empty() {
            continue;
        }
        match people
            .iter_mut()
            .find(|p| p.email.eq_ignore_ascii_case(email))
        {
            Some(known) => known.commits += 1,
            None => people.push(Person {
                name: name.to_owned(),
                email: email.to_owned(),
                commits: 1,
            }),
        }
    }
    people.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.name.cmp(&b.name)));
    people
}

#[cfg(test)]
mod tests {
    use super::tally;

    #[test]
    fn counts_each_address_once_whatever_its_case_and_ranks_by_commits() {
        let raw = b"Ada\0ada@x.org\0Grace\0grace@x.org\0Ada\0ADA@x.org\0\0\0";
        let people = tally(raw);
        assert_eq!(people.len(), 2);
        assert_eq!((people[0].name.as_str(), people[0].commits), ("Ada", 2));
        assert_eq!((people[1].name.as_str(), people[1].commits), ("Grace", 1));
    }
}
