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
    /// The private ssh key to authenticate with. `None` leaves it to the agent and
    /// `~/.ssh/config`, which is what git does on its own.
    pub ssh_key: Option<String>,
    /// How many commits of history to fetch. `None` is all of it.
    ///
    /// A shallow clone is a real repository with its history cut off at a boundary, and the
    /// graph walker grafts it there — see `docs/ARCHITECTURE.md`. It is not a lesser clone;
    /// it is the difference between waiting five minutes for the kernel and waiting five
    /// seconds when all somebody wants is to read the current tree.
    pub depth: Option<u32>,
    /// Fetch file contents on demand rather than up front.
    ///
    /// `--filter=blob:none` takes every commit and every tree and leaves the blobs on the
    /// server until something asks for one. History stays complete, which is what separates
    /// it from a shallow clone, and the price is that reading an old file needs the network.
    pub blobless: bool,
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

/// Removes a half-made clone when the work is dropped.
///
/// A cancelled clone is killed where it stands, so nothing after the await runs and the
/// partial repository would be left behind for the user to find and wonder about.
///
/// It arms only when the destination did not exist beforehand. A clone into a directory that
/// is already there fails without creating anything, and deleting it would take work that was
/// never ours to take.
struct RemoveOnDrop {
    at: PathBuf,
    armed: bool,
}

impl RemoveOnDrop {
    fn new(at: &Path) -> Self {
        Self {
            at: at.to_path_buf(),
            armed: !at.exists(),
        }
    }

    fn keep(mut self) {
        self.armed = false;
    }
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if self.armed && self.at.exists() {
            let _ = std::fs::remove_dir_all(&self.at);
        }
    }
}

/// The prefixes git puts on a line it means somebody to read.
const NOTEWORTHY: [&str; 4] = ["warning:", "error:", "hint:", "fatal:"];

/// Where a clone landed, and anything git said about it that was not progress.
///
/// A clone can exit 0 and still not check anything out. `notes` is how that reaches the person
/// who asked for it rather than being thrown away with the progress records.
#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CloneOutcome {
    pub at: PathBuf,
    pub notes: Vec<String>,
}

/// Clones a repository, reporting progress as git reports it.
///
/// Streamed rather than buffered, so a clone of anything large is not a silent wait. git
/// writes progress to stderr with carriage returns as separators, which is what
/// [`crate::remote::parse_progress`] reads.
///
/// Cancelling is dropping the future: the runner kills the child on drop, and the guard above
/// takes the partial repository with it.
///
/// # Errors
/// [`CoralError::AlreadyARepository`] if the destination already holds one, and git's own
/// failure otherwise — a URL that does not answer, credentials that were refused.
pub async fn clone<F>(
    runner: &GitRunner,
    what: &Cloned,
    mut on_progress: F,
) -> Result<CloneOutcome, CoralError>
where
    F: FnMut(crate::remote::Progress),
{
    let into = what.destination();
    if into.join(".git").exists() {
        return Err(CoralError::AlreadyARepository(into));
    }
    std::fs::create_dir_all(&what.parent)?;
    let partial = RemoveOnDrop::new(&into);

    // Network class, so nothing times out: a clone is bounded by the size of the repository and
    // by the user cancelling, not by a clock.
    let mut cmd = GitCommand::network("clone", &what.parent).args(["clone", "--progress"]);
    if let Some(depth) = what.depth.filter(|d| *d > 0) {
        cmd = cmd.arg(format!("--depth={depth}"));
        // git makes a depth-limited clone single-branch on its own, and says so only in the
        // manual. Saying it here means the repository that arrives is the one the caller
        // asked for rather than the one git inferred.
        cmd = cmd.arg("--single-branch");
    }
    if what.blobless {
        cmd = cmd.arg("--filter=blob:none");
    }
    cmd = cmd.arg(&what.url).arg(&into);
    // The environment rather than `-c core.sshCommand=`, because a `-c` is not inherited by
    // the repository git creates: it would authenticate this one fetch and leave the clone
    // with nothing. The key is written into the new repository below, which is what makes
    // every later fetch and push use it too.
    if let Some(key) = chosen_key(what) {
        cmd = cmd.env("GIT_SSH_COMMAND", crate::ssh::command_for(key));
    }
    // Progress is most of what git writes here, but not all of it: "remote HEAD refers to
    // nonexistent ref, unable to checkout" is a warning, git exits 0, and what arrives is a
    // repository with no files in it. Dropping every line that is not progress made that clone
    // look exactly like one that worked.
    let mut notes = Vec::new();
    runner
        .stream_err(cmd, |line| {
            if let Some(p) = crate::remote::parse_progress(line) {
                on_progress(p);
            } else {
                let text = String::from_utf8_lossy(line).trim().to_owned();
                // Only what git marks as worth reading. The rest of this stream is "Cloning
                // into '…'" and "done.", which say nothing the window does not already show,
                // and which would push the one line that matters out of a clamped toast.
                if NOTEWORTHY.iter().any(|p| text.starts_with(p)) {
                    notes.push(text);
                }
            }
            Ok(crate::process::Sink::Continue)
        })
        .await?;

    if let Some(key) = chosen_key(what) {
        pin_key(runner, &into, key).await?;
    }
    partial.keep();
    Ok(CloneOutcome { at: into, notes })
}

/// The key to authenticate with, ignoring a field somebody left blank.
fn chosen_key(what: &Cloned) -> Option<&str> {
    what.ssh_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
}

/// Records the key in the repository that was just made.
///
/// Failing here would leave a perfectly good clone reported as a failure, so it is logged and
/// the clone stands: the repository is there, and the key can be set from its settings.
async fn pin_key(runner: &GitRunner, into: &Path, key: &str) -> Result<(), CoralError> {
    let loc = crate::repo::RepoLocation::discover(runner, into).await?;
    let overrides = crate::ssh::SshOverrides {
        private_key: Some(key.to_owned()),
        ..crate::ssh::SshOverrides::default()
    };
    if let Err(e) = loc.set_ssh_local(runner, &overrides).await {
        tracing::warn!(error = %e, key, "cloned, but could not record the ssh key");
    }
    Ok(())
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
