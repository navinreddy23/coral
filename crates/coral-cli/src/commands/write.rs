use std::path::Path;

use coral_core::CoralError;
use coral_core::ops::{CommitOpts, MergeMode, OpAction, OpOutcome, ResetMode};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::undo::Journal;

/// CLI mirrors of the engine enums. The engine deliberately does not depend on clap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Mode {
    Auto,
    NoFf,
    FfOnly,
    Squash,
}

impl From<Mode> for MergeMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Auto => Self::Auto,
            Mode::NoFf => Self::NoFf,
            Mode::FfOnly => Self::FfOnly,
            Mode::Squash => Self::Squash,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Reset {
    Soft,
    Mixed,
    Hard,
}

impl From<Reset> for ResetMode {
    fn from(m: Reset) -> Self {
        match m {
            Reset::Soft => Self::Soft,
            Reset::Mixed => Self::Mixed,
            Reset::Hard => Self::Hard,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Action {
    Continue,
    Abort,
    Skip,
}

impl From<Action> for OpAction {
    fn from(a: Action) -> Self {
        match a {
            Action::Continue => Self::Continue,
            Action::Abort => Self::Abort,
            Action::Skip => Self::Skip,
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    pub what: String,
    /// Set when the operation produced a commit.
    pub oid: Option<String>,
}

impl crate::output::Human for Done {
    fn human(&self) -> String {
        match &self.oid {
            Some(o) => format!("{} {:.8}", self.what, o),
            None => self.what.clone(),
        }
    }
}

impl crate::output::Human for OpOutcome {
    fn human(&self) -> String {
        // On success the message is git's own chatter, except for a patch that was already in:
        // "completed" alone would read as though it had just been applied.
        if self.completed && self.message == coral_core::patch::ALREADY_APPLIED {
            return self.message.clone();
        }
        if self.completed {
            return format!("completed ({:?})", self.state);
        }
        let mut s = format!(
            "stopped in {:?} with {} conflicts",
            self.state,
            self.conflicts.len()
        );
        for c in &self.conflicts {
            s.push_str("\n  ");
            s.push_str(c);
        }
        // git's own explanation. A stop with no conflicts — an empty commit, or an `edit` step
        // the user asked for — says nothing at all without it, and that is exactly the case
        // where the reason is not obvious from the repository.
        for line in self.message.lines().filter(|l| !l.trim().is_empty()) {
            s.push_str("\n  ");
            s.push_str(line.trim_end());
        }
        s
    }
}

/// Opens a repository and journals whatever `f` changes about its refs.
///
/// The snapshot is taken before and after every mutation, so undo works for operations the
/// journal has no special knowledge of.
async fn journaled<T, F, Fut>(path: &Path, label: &str, f: F) -> Result<T, CoralError>
where
    F: FnOnce(GitRunner, RepoLocation) -> Fut,
    Fut: Future<Output = Result<T, CoralError>>,
{
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let before = loc.snapshot_refs(&runner).await?;

    let result = f(runner.clone(), loc.clone()).await?;

    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(label, before, after, coral_core::undo::Restore::Worktree)?;
    Ok(result)
}

/// Records a commit.
///
/// # Errors
/// Propagates git failures.
pub async fn commit(path: &Path, opts: CommitOpts) -> Result<Done, CoralError> {
    let oid = journaled(
        path,
        "commit",
        |r, l| async move { l.commit(&r, &opts).await },
    )
    .await?;
    Ok(Done {
        what: "committed".into(),
        oid: Some(oid),
    })
}

/// Merges a revision into the current branch.
///
/// # Errors
/// Propagates git failures that left no conflict behind.
pub async fn merge(
    path: &Path,
    rev: String,
    mode: Mode,
    message: Option<String>,
) -> Result<OpOutcome, CoralError> {
    journaled(path, &format!("merge {rev}"), |r, l| async move {
        l.merge(&r, &rev, mode.into(), message.as_deref()).await
    })
    .await
}

/// Replays the current branch onto another.
///
/// # Errors
/// Propagates git failures that left no conflict behind.
pub async fn rebase(
    path: &Path,
    onto: String,
    from: Option<String>,
    update_refs: bool,
) -> Result<OpOutcome, CoralError> {
    let label = match &from {
        None => format!("rebase onto {onto}"),
        Some(from) => format!("move the commits after {from} onto {onto}"),
    };
    journaled(path, &label, |r, l| async move {
        match from {
            None => l.rebase(&r, &onto, update_refs).await,
            Some(from) => l.rebase_onto(&r, &onto, &from, update_refs).await,
        }
    })
    .await
}

/// Applies commits onto the current branch.
///
/// # Errors
/// Propagates git failures that left no conflict behind.
pub async fn cherry_pick(
    path: &Path,
    revs: Vec<String>,
    mainline: Option<u32>,
) -> Result<OpOutcome, CoralError> {
    journaled(path, "cherry-pick", |r, l| async move {
        let picks: Vec<&str> = revs.iter().map(String::as_str).collect();
        l.cherry_pick(&r, &picks, true, mainline).await
    })
    .await
}

/// Records commits that undo others.
///
/// # Errors
/// Propagates git failures that left no conflict behind.
pub async fn revert(
    path: &Path,
    revs: Vec<String>,
    mainline: Option<u32>,
) -> Result<OpOutcome, CoralError> {
    journaled(path, "revert", |r, l| async move {
        let targets: Vec<&str> = revs.iter().map(String::as_str).collect();
        l.revert(&r, &targets, mainline).await
    })
    .await
}

/// Continues, aborts or skips the operation in progress.
///
/// # Errors
/// [`CoralError::Protocol`] when nothing is in progress.
pub async fn op(path: &Path, action: Action) -> Result<OpOutcome, CoralError> {
    // Label the entry with what was in progress, not with "op": the undo tooltip names the
    // operation, and "continue" on its own tells the user nothing.
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let label = format!("{:?} {:?}", loc.op_state(), action).to_lowercase();

    journaled(
        path,
        &label,
        |r, l| async move { l.op(&r, action.into()).await },
    )
    .await
}

/// Moves the current branch.
///
/// # Errors
/// Propagates git failures.
pub async fn reset(path: &Path, rev: String, mode: Reset) -> Result<Done, CoralError> {
    // Journaled with its mode rather than through `journaled`, so a redo repeats it in the
    // same mode instead of as a hard reset.
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let before = loc.snapshot_refs(&runner).await?;
    loc.reset(&runner, &rev, mode.into()).await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_reset(&format!("reset to {rev}"), before, after, mode.into())?;
    Ok(Done {
        what: "reset".into(),
        oid: None,
    })
}

/// Checks out a revision.
///
/// # Errors
/// Propagates git failures.
pub async fn checkout(path: &Path, rev: String) -> Result<Done, CoralError> {
    journaled(path, &format!("checkout {rev}"), |r, l| async move {
        l.checkout(&r, &rev).await
    })
    .await?;
    Ok(Done {
        what: "checked out".into(),
        oid: None,
    })
}

/// Reverses the most recent journalled operation.
///
/// # Errors
/// [`CoralError::Protocol`] when there is nothing to undo, or the worktree is dirty.
pub async fn undo(path: &Path) -> Result<Done, CoralError> {
    step(path, true).await
}

/// Replays the most recently undone operation.
///
/// # Errors
/// [`CoralError::Protocol`] when there is nothing to redo, or the worktree is dirty.
pub async fn redo(path: &Path) -> Result<Done, CoralError> {
    step(path, false).await
}

async fn step(path: &Path, backwards: bool) -> Result<Done, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok(Done {
        what: loc.undo_step(&runner, backwards).await?,
        oid: None,
    })
}

/// What undo and redo would do next.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalView {
    pub undoable: Option<String>,
    pub redoable: Option<String>,
    pub entries: Vec<String>,
}

impl crate::output::Human for JournalView {
    fn human(&self) -> String {
        let mut s = format!(
            "undo: {}\nredo: {}",
            self.undoable.as_deref().unwrap_or("nothing"),
            self.redoable.as_deref().unwrap_or("nothing")
        );
        for e in &self.entries {
            s.push_str("\n  ");
            s.push_str(e);
        }
        s
    }
}

/// Reports the undo stack.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository.
pub async fn journal(path: &Path) -> Result<JournalView, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let j = Journal::load(&loc);
    Ok(JournalView {
        undoable: j.undoable().map(|e| e.label.clone()),
        redoable: j.redoable().map(|e| e.label.clone()),
        entries: j.entries.iter().map(|e| e.label.clone()).collect(),
    })
}

/// Creates a branch, optionally checking it out.
///
/// # Errors
/// Propagates git failures, including an existing branch of the same name.
pub async fn branch_create(
    path: &Path,
    name: String,
    at: Option<String>,
    checkout: bool,
) -> Result<Done, CoralError> {
    journaled(path, &format!("create branch {name}"), |r, l| async move {
        l.branch_create(&r, &name, at.as_deref(), checkout).await
    })
    .await?;
    Ok(Done {
        what: "branch created".into(),
        oid: None,
    })
}

/// Deletes a branch.
///
/// # Errors
/// Propagates git failures; git refuses an unmerged branch without `force`.
pub async fn branch_delete(path: &Path, name: String, force: bool) -> Result<Done, CoralError> {
    journaled(path, &format!("delete branch {name}"), |r, l| async move {
        l.branch_delete(&r, &name, force).await
    })
    .await?;
    Ok(Done {
        what: "branch deleted".into(),
        oid: None,
    })
}

/// Renames a branch.
///
/// # Errors
/// Propagates git failures.
pub async fn branch_rename(path: &Path, from: String, to: String) -> Result<Done, CoralError> {
    journaled(path, &format!("rename branch {from}"), |r, l| async move {
        l.branch_rename(&r, &from, &to).await
    })
    .await?;
    Ok(Done {
        what: "branch renamed".into(),
        oid: None,
    })
}

/// Creates a tag. A message makes it annotated.
///
/// # Errors
/// Propagates git failures.
pub async fn tag_create(
    path: &Path,
    name: String,
    at: Option<String>,
    message: Option<String>,
) -> Result<Done, CoralError> {
    journaled(path, &format!("create tag {name}"), |r, l| async move {
        l.tag_create(&r, &name, at.as_deref(), message.as_deref())
            .await
    })
    .await?;
    Ok(Done {
        what: "tag created".into(),
        oid: None,
    })
}

/// Deletes a tag.
///
/// # Errors
/// Propagates git failures.
pub async fn tag_delete(path: &Path, name: String) -> Result<Done, CoralError> {
    journaled(path, &format!("delete tag {name}"), |r, l| async move {
        l.tag_delete(&r, &name).await
    })
    .await?;
    Ok(Done {
        what: "tag deleted".into(),
        oid: None,
    })
}

/// What to do with the stash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum StashAction {
    Push,
    Apply,
    Pop,
    Drop,
}

/// Pushes, applies, pops or drops a stash entry.
///
/// # Errors
/// Propagates git failures that left no conflict behind. A stash that lands on conflicts is
/// reported as a stop, the way a merge is, rather than as a failure.
pub async fn stash(
    path: &Path,
    action: StashAction,
    index: usize,
    message: Option<String>,
    include_untracked: bool,
) -> Result<OpOutcome, CoralError> {
    let label = format!("stash {action:?}").to_lowercase();
    journaled(path, &label, |r, l| async move {
        match action {
            // Neither of these can stop half-done, so both answer with the outcome the
            // repository is in once they have run: a completed one.
            StashAction::Push => {
                let scope = coral_core::ops::StashScope::Everything { include_untracked };
                l.stash_push(&r, message.as_deref(), &scope).await?;
                l.op_outcome(&r, String::new()).await
            }
            StashAction::Drop => {
                l.stash_drop(&r, index).await?;
                l.op_outcome(&r, String::new()).await
            }
            StashAction::Apply => l.stash_apply(&r, index, false).await,
            StashAction::Pop => l.stash_apply(&r, index, true).await,
        }
    })
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drafted {
    #[serde(flatten)]
    pub draft: coral_assist::Draft,
}

/// Drafts a message for what is staged, on the key the app keeps in the keyring.
///
/// # Errors
/// [`CoralError::Refused`] with no key kept, nothing staged, too much staged, or the API's
/// own refusal; and git failures.
pub async fn draft_message(path: &Path, model: Option<String>) -> Result<Drafted, CoralError> {
    let refused = |e: coral_assist::DraftError| CoralError::Refused {
        label: "draft a message",
        detail: e.to_string(),
    };
    let key = coral_assist::key::load()
        .map_err(refused)?
        .ok_or_else(|| CoralError::Refused {
            label: "draft a message",
            detail: "no API key is kept; add one in the app's Preferences, Experimental".to_owned(),
        })?;
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let recent = loc
        .log(
            &runner,
            &coral_core::history::LogQuery {
                limit: Some(12),
                ..Default::default()
            },
        )
        .await
        .map(|c| c.into_iter().map(|c| c.summary.to_string()).collect())
        .unwrap_or_default();
    let request = coral_assist::Request {
        diff: loc.staged_patch(&runner).await?,
        recent,
        model: model.unwrap_or_default(),
    };
    let endpoint = std::env::var("CORAL_ASSIST_URL").ok();
    let draft = coral_assist::draft(&key, &request, endpoint.as_deref())
        .await
        .map_err(refused)?;
    Ok(Drafted { draft })
}

impl crate::output::Human for Drafted {
    fn human(&self) -> String {
        if self.draft.body.is_empty() {
            self.draft.summary.clone()
        } else {
            format!("{}\n\n{}", self.draft.summary, self.draft.body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Human;

    fn outcome(message: &str) -> OpOutcome {
        OpOutcome {
            completed: true,
            state: coral_core::repo::OpState::Clean,
            conflicts: Vec::new(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn a_patch_already_in_says_so_rather_than_completed() {
        assert_eq!(
            outcome(coral_core::patch::ALREADY_APPLIED).human(),
            coral_core::patch::ALREADY_APPLIED
        );
        assert_eq!(outcome("Applying: readme").human(), "completed (Clean)");
    }
}
