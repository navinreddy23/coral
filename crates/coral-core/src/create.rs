//! Making a repository that is not there yet: an empty one, or a copy of somebody else's.
//!
//! Everything else in the engine takes a [`RepoLocation`], which is a repository that already
//! exists. These two are what produce one, so they are free functions over a path rather than
//! methods on a repository, and each answers with the path it made so the caller can open it.

use std::path::{Path, PathBuf};

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};

/// What to create.
#[derive(Clone, Debug)]
pub struct NewRepo {
    /// Where it goes. Created if it is not there; refused if it already holds a repository.
    pub path: PathBuf,
    /// The name of the first branch. `None` leaves git's own default, which the user may have
    /// configured and which Coral has no business overriding.
    pub branch: Option<String>,
    /// Whether to set Large File Storage up in it.
    pub lfs: bool,
}

/// Creates an empty repository and returns where it is.
///
/// # Errors
/// [`CoralError::AlreadyARepository`] if there is one there already, [`CoralError::Io`] if the
/// directory cannot be made, and git's own failure otherwise.
pub async fn init(runner: &GitRunner, new: &NewRepo) -> Result<PathBuf, CoralError> {
    if new.path.join(".git").exists() {
        return Err(CoralError::AlreadyARepository(new.path.clone()));
    }
    std::fs::create_dir_all(&new.path)?;

    let mut cmd = GitCommand::write("init", &new.path).args(["init", "--quiet"]);
    if let Some(branch) = branch_name(new.branch.as_deref()) {
        cmd = cmd.arg(format!("--initial-branch={branch}"));
    }
    runner.output(cmd).await?;

    if new.lfs {
        install_lfs(runner, &new.path).await?;
    }
    Ok(new.path.clone())
}

/// What to clone, and where to put it.
#[derive(Clone, Debug)]
pub struct Cloned {
    pub url: String,
    /// The directory the clone is made *in*; the repository appears under it.
    pub parent: PathBuf,
    /// What to call the directory. `None` uses the name in the URL, as git does.
    pub name: Option<String>,
}

impl Cloned {
    /// Where the repository will be, which is what the caller opens afterwards.
    #[must_use]
    pub fn destination(&self) -> PathBuf {
        self.parent.join(match &self.name {
            Some(name) => name.clone(),
            None => name_from_url(&self.url),
        })
    }
}

/// Clones a repository and returns where it landed.
///
/// # Errors
/// [`CoralError::AlreadyARepository`] if the destination already holds one, and git's own
/// failure otherwise — a URL that does not answer, credentials that were refused.
pub async fn clone(runner: &GitRunner, what: &Cloned) -> Result<PathBuf, CoralError> {
    let into = what.destination();
    if into.join(".git").exists() {
        return Err(CoralError::AlreadyARepository(into));
    }
    std::fs::create_dir_all(&what.parent)?;

    // Network class, so nothing times out: a clone is bounded by the size of the repository and
    // by the user cancelling, not by a clock.
    let cmd = GitCommand::network("clone", &what.parent)
        .args(["clone", "--progress"])
        .arg(&what.url)
        .arg(&into);
    runner.output(cmd).await?;
    Ok(into)
}

/// Whether `git lfs` is installed on this machine.
///
/// Asked before offering it. Large File Storage is a separate program, and a tick box that
/// fails because it is not there is worse than one that is not offered.
pub async fn lfs_available(runner: &GitRunner) -> bool {
    runner
        .output(
            GitCommand::bare("lfs-version", crate::process::GitClass::Read)
                .args(["lfs", "--version"]),
        )
        .await
        .is_ok()
}

async fn install_lfs(runner: &GitRunner, path: &Path) -> Result<(), CoralError> {
    // `--local`, so the setting lands in this repository rather than in the user's own
    // configuration. Creating one repository is not a decision about all of them.
    runner
        .output(GitCommand::write("lfs-install", path).args(["lfs", "install", "--local"]))
        .await
        .map(|_| ())
}

/// The branch name to pass, or `None` to leave git's default alone.
///
/// A name git will not accept is refused by git anyway; what is filtered here is the empty
/// string, which comes from a form field nobody typed in.
fn branch_name(asked: Option<&str>) -> Option<&str> {
    asked.map(str::trim).filter(|name| !name.is_empty())
}

/// The directory name git would choose for a URL.
///
/// git's own rule: the last path segment, without a trailing `.git` and without a trailing
/// slash. `https://host/team/thing.git/` and `git@host:team/thing.git` both give `thing`.
#[must_use]
pub fn name_from_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', ':'])
        .find(|part| !part.is_empty())
        .unwrap_or(trimmed);
    last.strip_suffix(".git").unwrap_or(last).to_owned()
}

#[cfg(test)]
mod tests {
    use super::{branch_name, name_from_url};

    #[test]
    fn takes_the_name_git_would_take() {
        for (url, want) in [
            ("https://github.com/torvalds/linux.git", "linux"),
            ("https://github.com/torvalds/linux", "linux"),
            ("https://github.com/torvalds/linux/", "linux"),
            ("git@gitlab.com:open-source-23/coral.git", "coral"),
            ("ssh://git@host:2222/team/thing.git", "thing"),
            ("/srv/git/bare.git", "bare"),
            ("  https://host/x/y.git  ", "y"),
        ] {
            assert_eq!(name_from_url(url), want, "{url}");
        }
    }

    #[test]
    fn a_url_that_is_only_a_host_still_gives_something_to_call_it() {
        // Nothing usable, but a name that is empty would make the destination the parent
        // directory itself, and cloning into it would be a surprise nobody could undo.
        assert_eq!(name_from_url("https://example.invalid"), "example.invalid");
        assert!(!name_from_url("https://example.invalid/").is_empty());
    }

    #[test]
    fn an_empty_branch_name_is_no_branch_name() {
        // The field is optional and comes back as "" rather than absent, and
        // `--initial-branch=` is an error rather than a default.
        assert_eq!(branch_name(Some("main")), Some("main"));
        assert_eq!(branch_name(Some("  spaced  ")), Some("spaced"));
        assert_eq!(branch_name(Some("   ")), None);
        assert_eq!(branch_name(Some("")), None);
        assert_eq!(branch_name(None), None);
    }
}
