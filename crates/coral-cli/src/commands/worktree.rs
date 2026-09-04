use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::worktree::Worktree;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktrees {
    pub worktrees: Vec<Worktree>,
}

impl crate::output::Human for Worktrees {
    fn human(&self) -> String {
        if self.worktrees.is_empty() {
            return "no worktrees".to_owned();
        }
        let mut s = format!("{} worktrees", self.worktrees.len());
        for w in &self.worktrees {
            let what = w.branch.as_deref().unwrap_or("detached");
            let _ = write!(s, "\n  {:<10} {}", what, w.path);
            if w.locked {
                s.push_str(" (locked)");
            }
        }
        s
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub path: String,
    pub rev: String,
    pub branch: Option<String>,
}

impl crate::output::Human for Added {
    fn human(&self) -> String {
        match &self.branch {
            Some(b) => format!("worktree {} on new branch {b}", self.path),
            None => format!("worktree {} detached at {:.8}", self.path, self.rev),
        }
    }
}

/// Lists the repository's working trees.
///
/// # Errors
/// Propagates git failures.
pub async fn list(path: &Path) -> Result<Worktrees, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok(Worktrees {
        worktrees: loc.worktrees(&runner).await?,
    })
}

/// Checks a revision out into a new working tree.
///
/// # Errors
/// Propagates git failures.
pub async fn add(
    repo: &Path,
    at: &Path,
    rev: &str,
    branch: Option<String>,
) -> Result<Added, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, repo).await?;
    let created = loc
        .worktree_add(&runner, at, rev, branch.as_deref())
        .await?;
    Ok(Added {
        path: display(&created),
        rev: rev.to_owned(),
        branch,
    })
}

/// Removes a working tree.
///
/// # Errors
/// Propagates git failures, including uncommitted changes without `force`.
pub async fn remove(
    repo: &Path,
    at: &Path,
    force: bool,
) -> Result<crate::commands::write::Done, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, repo).await?;
    loc.worktree_remove(&runner, at, force).await?;
    Ok(crate::commands::write::Done {
        what: format!("removed worktree {}", display(at)),
        oid: None,
    })
}

fn display(p: &Path) -> String {
    p.display().to_string()
}
