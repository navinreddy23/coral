use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::sequence::Todo;

/// The todo list an interactive rebase onto `onto` would start from.
///
/// # Errors
/// Propagates git failures, including an unknown revision.
pub async fn todo(path: &Path, onto: &str) -> Result<Todo, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.rebase_todo(&runner, onto).await
}

impl crate::output::Human for Todo {
    fn human(&self) -> String {
        if self.items.is_empty() {
            return "nothing to rebase".to_owned();
        }
        let mut out = format!("{} commits", self.items.len());
        for item in &self.items {
            let _ = write!(
                out,
                "\n  {:<7}{:.8}  {}",
                item.step.verb(),
                item.oid,
                item.summary
            );
        }
        out
    }
}
