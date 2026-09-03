use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::{Head, RepoInfo, RepoLocation};

/// Validates a repository and gathers what the UI needs before it can show anything.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path) -> Result<RepoInfo, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.info(&runner).await
}

impl crate::output::Human for RepoInfo {
    fn human(&self) -> String {
        let head = match &self.head {
            Head::Branch { name } => format!("on branch {name}"),
            Head::Detached { oid } => format!("detached at {}", &oid[..oid.len().min(8)]),
            Head::Unborn { name } => format!("on unborn branch {name}"),
        };
        let graph = if self.commit_graph {
            "present"
        } else {
            "absent"
        };
        format!(
            "{}\n  {head}\n  state: {:?}\n  git: {}\n  commit-graph: {graph}",
            self.path.display(),
            self.state,
            self.git_version,
        )
    }
}
