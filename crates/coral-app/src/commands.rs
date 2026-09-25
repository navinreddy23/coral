use coral_core::process::GitRunner;
use coral_core::repo::{RepoInfo, RepoLocation};

/// Errors cross the IPC boundary as the same stable `code` the CLI envelope uses, so the UI
/// has one error vocabulary regardless of which front end it is talking to.
#[derive(Debug, serde::Serialize)]
pub struct IpcError {
    code: &'static str,
    pub message: String,
    /// What the failed operation is called, when it belongs to one the journal names.
    ///
    /// A red toast titled "Something went wrong" says nothing its colour has not. Actions
    /// already carry a name — `tag v1.0`, `push main`, `fetch` — and a failure titled with it
    /// reads the way a success does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
}

impl IpcError {
    /// Whether this is somebody stopping the work rather than the work going wrong.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.code == "cancelled"
    }

    /// Names the operation this failure belongs to.
    #[must_use]
    pub fn during(mut self, what: &str) -> Self {
        self.what = Some(what.to_owned());
        self
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<coral_core::CoralError> for IpcError {
    fn from(e: coral_core::CoralError) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
            what: None,
        }
    }
}

/// Which repository the window should open with.
///
/// `CORAL_REPO` exists so the app can be pointed at a repository without a file dialog, which
/// is what makes benchmarking against a large clone possible. M5 replaces this with the tab
/// session.
#[tauri::command]
#[must_use]
pub fn initial_repo() -> String {
    std::env::var("CORAL_REPO").unwrap_or_else(|_| ".".to_owned())
}

/// # Errors
/// [`coral_core::CoralError::NotARepository`], or any git failure.
#[tauri::command]
pub async fn open_repo(path: String) -> Result<RepoInfo, IpcError> {
    tracing::info!(path, "open_repo");
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.info(&runner).await?)
}

/// The working tree, for the WIP row and the staging panel.
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_status(path: String) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.status(&runner).await?)
}

/// Stages or unstages whole paths, then reports the resulting status.
///
/// Returning the new status rather than nothing means the panel cannot drift from the
/// repository: there is no separate refresh to miss.
/// # Errors
/// Propagates git failures, including a path that does not exist.
#[tauri::command]
pub async fn stage_paths(
    path: String,
    paths: Vec<String>,
    stage: bool,
) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();

    let logged = crate::activity::started(&path, &staging_label(&paths, stage));
    let done = if stage {
        loc.stage(&runner, &refs).await
    } else {
        loc.unstage(&runner, &refs).await
    };
    match done {
        Ok(()) => logged.finished(),
        Err(e) => {
            logged.failed(&e.to_string());
            return Err(e.into());
        }
    }
    Ok(loc.status(&runner).await?)
}

/// What the log calls a staging change: the file when there is one, a count when there are
/// several, since a hundred paths on one line is not a log entry anyone reads.
fn staging_label(paths: &[String], stage: bool) -> String {
    let verb = if stage { "Stage" } else { "Unstage" };
    match paths {
        [] => format!("{verb} everything"),
        [one] => format!("{verb} {one}"),
        many => format!("{verb} {} files", many.len()),
    }
}

/// Creates an empty repository and answers with where it is.
///
/// # Errors
/// [`coral_core::CoralError::AlreadyARepository`] if there is one there already, and git's own
/// failure otherwise.
#[tauri::command]
pub async fn repo_init(
    profiles: tauri::State<'_, crate::profile::Profiles>,
    path: String,
    branch: Option<String>,
    lfs: bool,
) -> Result<String, IpcError> {
    let settings = profiles.read().current().settings.clone();
    let runner = coral_core::process::GitRunner::discover().await?;
    let made = coral_core::create::init(
        &runner,
        &coral_core::create::NewRepo {
            path: std::path::PathBuf::from(&path),
            branch,
            lfs,
        },
    )
    .await?;
    // A repository that has just been made has nothing to overwrite, which is why the profile
    // is applied here without asking and nowhere else without a button.
    crate::profile::stamp_new_repository(&settings, &made).await;
    Ok(made.display().to_string())
}

/// Everything a clone is asked for.
///
/// One argument rather than six, because a command signature that long stops saying which
/// value is which — and the window has to build the same shape either way.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    pub url: String,
    /// The directory the clone is made *in*; the repository appears under it.
    pub parent: String,
    /// What to call it. `None` uses the name in the URL, as git does.
    pub name: Option<String>,
    /// The private key to authenticate with, or `None` to leave it to the agent.
    pub ssh_key: Option<String>,
    /// How many commits of history to take. `None` is all of them.
    pub depth: Option<u32>,
    /// Leave file contents on the server until something reads one.
    #[serde(default)]
    pub blobless: bool,
}

/// Clones a repository and answers with where it landed.
///
/// # Errors
/// [`coral_core::CoralError::AlreadyARepository`] if the destination already holds one, and
/// git's own failure otherwise.
#[tauri::command]
pub async fn repo_clone(
    app: tauri::AppHandle,
    profiles: tauri::State<'_, crate::profile::Profiles>,
    request: CloneRequest,
) -> Result<coral_core::create::CloneOutcome, IpcError> {
    let CloneRequest {
        url,
        parent,
        name,
        ssh_key,
        depth,
        blobless,
    } = request;
    let mut settings = profiles.read().current().settings.clone();
    let runner = coral_core::process::GitRunner::discover().await?;
    // The form's own choice wins over the profile's, since it was made about this clone; the
    // profile is the default the form was filled in with.
    let key = ssh_key
        .filter(|k| !k.trim().is_empty())
        .or_else(|| settings.ssh.private_key.clone());
    // And it is the key the stamp below should write, for the same reason.
    settings.ssh = ssh_for_clone(&settings.ssh, key.as_deref());
    let what = coral_core::create::Cloned {
        url,
        parent: std::path::PathBuf::from(&parent),
        name: name.filter(|n| !n.trim().is_empty()),
        ssh_key: key,
        depth: depth.filter(|d| *d > 0),
        blobless,
    };
    let logged = crate::activity::started(&parent, &format!("Clone {}", what.url));
    // Keyed by where it will land, since there is no repository to name yet and that is the
    // one string the window already has: it is what the form says the clone will be at.
    let key = what.destination().display().to_string();
    let cloning = crate::transfer::watched(&app, &key, "Clone", move |report| async move {
        coral_core::create::clone(&runner, &what, |p| report.progress(&p)).await
    });
    match cloning.await {
        Ok(made) => {
            logged.finished();
            // Anything git said that was not progress. A clone can exit 0 and check nothing
            // out, and the window has no other way to learn that.
            for note in &made.notes {
                crate::activity::note(&parent, crate::activity::Level::Warn, note);
            }
            crate::profile::stamp_new_repository(&settings, &made.at).await;
            Ok(made)
        }
        Err(e) if matches!(e, coral_core::CoralError::Cancelled { .. }) => {
            logged.cancelled();
            Err(e.into())
        }
        Err(e) => {
            logged.failed(&e.to_string());
            Err(e.into())
        }
    }
}

/// What to stamp a clone's ssh settings with, given the profile's and the key it used.
///
/// The profile is only the default the form was filled in with, so a key chosen there replaces
/// it rather than being overwritten by it. Both halves go together: the public one is what
/// gets pasted into a host, and the profile's names a key this repository does not sign in
/// with. The credential helper is about the host rather than the key, so it stays.
fn ssh_for_clone(
    profile: &coral_core::ssh::SshOverrides,
    key: Option<&str>,
) -> coral_core::ssh::SshOverrides {
    let key = key.map(str::to_owned);
    if key == profile.private_key {
        return profile.clone();
    }
    coral_core::ssh::SshOverrides {
        public_key: key.as_deref().and_then(public_half),
        private_key: key,
        credential_helper: profile.credential_helper.clone(),
    }
}

/// The public half beside a private key, when it is there.
///
/// The same pairing the key list uses: a private key is the file whose `.pub` sits next to it.
/// git never reads this half; it is what the settings pane offers for pasting into a host.
fn public_half(private: &str) -> Option<String> {
    let beside = format!("{private}.pub");
    std::path::Path::new(&beside).is_file().then_some(beside)
}

/// Whether `git lfs` is on this machine.
///
/// Asked before offering it: Large File Storage is a separate program, and a tick box that
/// fails because it is not installed is worse than one that is not shown.
///
/// # Errors
/// Propagates the failure to find git at all. Not finding `git lfs` is an answer, not a
/// failure.
#[tauri::command]
pub async fn lfs_available() -> Result<bool, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    Ok(coral_core::create::lfs_available(&runner).await)
}

/// Throws away working-tree changes, then reports the resulting status.
///
/// Two lists, and the caller says which path goes in which. `restore` goes back to what HEAD
/// holds; `remove` is deleted outright. The split is not inferred here from each path's status
/// on purpose: deleting a file git has never seen destroys the only copy of it, so the window
/// has to have asked about those files by name and to say which ones it asked about.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn discard_paths(
    path: String,
    restore: Vec<String>,
    remove: Vec<String>,
) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let put_back: Vec<&str> = restore.iter().map(String::as_str).collect();
    let delete: Vec<&str> = remove.iter().map(String::as_str).collect();

    let logged = crate::activity::started(&path, &discard_label(&restore, &remove));
    let done = async {
        loc.restore_from_head(&runner, &put_back).await?;
        loc.remove_untracked(&runner, &delete).await
    }
    .await;
    match done {
        Ok(()) => logged.finished(),
        Err(e) => {
            logged.failed(&e.to_string());
            return Err(e.into());
        }
    }
    Ok(loc.status(&runner).await?)
}

/// Deletes files outright: gone from the working tree, and for tracked ones staged as removed.
///
/// Separate from discarding, which puts a file back to what HEAD holds. This is the one that
/// leaves nothing behind, which is why the window asks before calling it.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn delete_paths(
    path: String,
    tracked: Vec<String>,
    untracked: Vec<String>,
) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let known: Vec<&str> = tracked.iter().map(String::as_str).collect();
    let new: Vec<&str> = untracked.iter().map(String::as_str).collect();

    let count = tracked.len() + untracked.len();
    let logged = crate::activity::started(&path, &format!("delete {count} file(s)"));
    let done = async {
        loc.delete_tracked(&runner, &known).await?;
        loc.remove_untracked(&runner, &new).await
    }
    .await;
    match done {
        Ok(()) => logged.finished(),
        Err(e) => {
            logged.failed(&e.to_string());
            return Err(e.into());
        }
    }
    Ok(loc.status(&runner).await?)
}

/// What the log calls a discard. It names the deletions separately, because that is the half
/// nothing can bring back and the half worth being able to find afterwards.
fn discard_label(restore: &[String], remove: &[String]) -> String {
    match (restore.len(), remove.len()) {
        (0, 0) => "Discard nothing".to_owned(),
        (n, 0) => format!("Discard changes to {n} file(s)"),
        (0, m) => format!("Delete {m} untracked file(s)"),
        (n, m) => format!("Discard changes to {n} file(s) and delete {m} untracked"),
    }
}

/// Records a commit from what is staged, then reports the resulting status.
/// # Errors
/// Propagates git failures, including a rejecting hook.
#[tauri::command]
pub async fn commit_staged(
    path: String,
    message: String,
    amend: bool,
) -> Result<coral_core::Status, IpcError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc =
        coral_core::repo::RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let opts = coral_core::ops::CommitOpts {
        message,
        amend,
        ..coral_core::ops::CommitOpts::default()
    };

    // Lowercase, like every other journal label: undo reads them back as "undid <label>", so
    // a capitalised noun lands in the middle of a sentence.
    let label = if amend { "amend the commit" } else { "commit" };
    let logged = crate::activity::started(&path, label);
    // Journalled like any other operation that moves a ref, so Undo reaches the commonest one
    // of all. It comes back staged rather than discarded: the refs go where they were and the
    // files stay exactly where the user left them a moment ago.
    let before = loc.snapshot_refs(&runner).await?;
    match loc.commit(&runner, &opts).await {
        Ok(_) => logged.finished(),
        Err(e) => {
            logged.failed(&e.to_string());
            return Err(e.into());
        }
    }
    let after = loc.snapshot_refs(&runner).await?;
    if let Err(e) = loc.journal_change(label, before, after, coral_core::undo::Restore::KeepChanges)
    {
        // The commit is made and correct. Losing the ability to undo it is worth reporting and
        // not worth failing over.
        tracing::warn!(error = %e, "committed, but could not journal it for undo");
    }
    Ok(loc.status(&runner).await?)
}

#[cfg(test)]
mod tests {
    use super::ssh_for_clone;
    use coral_core::ssh::SshOverrides;

    fn profile(key: &str) -> SshOverrides {
        SshOverrides {
            private_key: Some(key.to_owned()),
            public_key: Some(format!("{key}.pub")),
            credential_helper: Some("manager".to_owned()),
        }
    }

    #[test]
    fn a_clone_that_used_the_profiles_key_is_stamped_with_the_profiles_settings() {
        let settings = profile("/keys/work");
        assert_eq!(
            ssh_for_clone(&settings, Some("/keys/work")),
            settings,
            "the public half it names is the right one already"
        );
    }

    #[test]
    fn a_key_chosen_on_the_form_takes_its_own_public_half_rather_than_the_profiles() {
        // Not the profile's: the pane offers that half for pasting into a host, and it would
        // name a key this repository does not sign in with.
        let stamped = ssh_for_clone(&profile("/keys/work"), Some("/keys/personal"));
        assert_eq!(stamped.private_key.as_deref(), Some("/keys/personal"));
        assert_eq!(
            stamped.public_key, None,
            "there is no /keys/personal.pub on this machine to name"
        );
        assert_eq!(
            stamped.credential_helper.as_deref(),
            Some("manager"),
            "the helper is about the host, not the key"
        );
    }

    #[test]
    fn a_key_chosen_on_the_form_is_paired_with_the_public_half_beside_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let key = dir.path().join("id_chosen");
        std::fs::write(&key, "x").expect("write");
        std::fs::write(key.with_extension("pub"), "x").expect("write");
        let key = key.display().to_string();

        let stamped = ssh_for_clone(&profile("/keys/work"), Some(&key));

        assert_eq!(stamped.private_key.as_deref(), Some(key.as_str()));
        assert_eq!(stamped.public_key, Some(format!("{key}.pub")));
    }

    #[test]
    fn a_clone_with_no_key_anywhere_is_stamped_with_nothing() {
        let stamped = ssh_for_clone(&SshOverrides::default(), None);
        assert!(stamped.is_empty(), "the agent stays the default");
    }
}
