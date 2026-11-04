use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::commit::CommitMeta;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub commits: Vec<CommitMeta>,
}

/// Answers a query the way the window's find box does, qualifiers included.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, a qualifier that cannot
/// be read, or any git failure.
pub async fn run(path: &Path, query: &str, limit: u64) -> Result<Found, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let oids = loc.search_commits(&runner, query, limit).await?;
    let mut commits = loc.commit_metadata(&runner, &oids).await?;
    // The window ranks by where a match sits in the graph; here the engine's own order, which
    // puts a pasted hash first, is kept rather than whatever order the batch read returned.
    commits.sort_by_key(|c| oids.iter().position(|o| *o == c.oid));
    Ok(Found { commits })
}

impl crate::output::Human for Found {
    fn human(&self) -> String {
        if self.commits.is_empty() {
            return "no matches".to_owned();
        }
        let mut out = format!("{} matches", self.commits.len());
        for c in &self.commits {
            let _ = write!(out, "\n  {:.8}  {:<20.20}  {}", c.oid, c.author, c.summary);
        }
        out
    }
}
