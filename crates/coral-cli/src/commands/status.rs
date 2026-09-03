use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::status::Status;

/// Reads the working tree state.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path) -> Result<Status, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.status(&runner).await
}

impl crate::output::Human for Status {
    fn human(&self) -> String {
        let mut out = match (&self.branch, &self.upstream) {
            (Some(b), Some(u)) => format!("on {b} (tracking {u})"),
            (Some(b), None) => format!("on {b}"),
            (None, _) => "detached".to_owned(),
        };
        if self.ahead != 0 || self.behind != 0 {
            let _ = write!(out, "  ahead {} behind {}", self.ahead, self.behind.abs());
        }
        if self.stash_count > 0 {
            let _ = write!(out, "  {} stashed", self.stash_count);
        }
        if self.is_clean() {
            out.push_str("\n  clean");
            return out;
        }
        for e in &self.entries {
            let mark = match e.conflict {
                Some(k) => format!("{k:?}"),
                None => format!("{:?}/{:?}", e.index, e.worktree),
            };
            let _ = write!(out, "\n  {mark:<24} {}", e.path);
        }
        out
    }
}
