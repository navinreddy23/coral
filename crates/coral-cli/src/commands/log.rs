use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::commit::Commit;
use coral_core::history::LogQuery;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub commits: Vec<Commit>,
}

/// Reads commit history.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path, q: &LogQuery) -> Result<Log, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok(Log {
        commits: loc.log(&runner, q).await?,
    })
}

impl crate::output::Human for Log {
    fn human(&self) -> String {
        if self.commits.is_empty() {
            return "no commits".to_owned();
        }
        let mut out = format!("{} commits", self.commits.len());
        for c in &self.commits {
            let merge = if c.is_merge() { " (merge)" } else { "" };
            let _ = write!(
                out,
                "\n  {:.8}  {:<20.20}  {}{merge}",
                c.oid, c.author.name, c.summary
            );
        }
        out
    }
}
