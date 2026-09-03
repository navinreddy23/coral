use std::path::Path;

use coral_core::CoralError;
use coral_core::ops::{CommitOpts, MergeMode, OpAction, OpOutcome, ResetMode};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::undo::{Journal, JournalEntry};

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
    if before != after {
        let mut journal = Journal::load(&loc);
        journal.record(JournalEntry {
            label: label.to_owned(),
            before,
            after,
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0)),
        });
        journal.save(&loc)?;
    }
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
pub async fn rebase(path: &Path, onto: String, update_refs: bool) -> Result<OpOutcome, CoralError> {
    journaled(path, &format!("rebase onto {onto}"), |r, l| async move {
        l.rebase(&r, &onto, update_refs).await
    })
    .await
}

/// Applies commits onto the current branch.
///
/// # Errors
/// Propagates git failures that left no conflict behind.
pub async fn cherry_pick(path: &Path, revs: Vec<String>) -> Result<OpOutcome, CoralError> {
    journaled(path, "cherry-pick", |r, l| async move {
        let picks: Vec<&str> = revs.iter().map(String::as_str).collect();
        l.cherry_pick(&r, &picks).await
    })
    .await
}

/// Records commits that undo others.
///
/// # Errors
/// Propagates git failures that left no conflict behind.
pub async fn revert(path: &Path, revs: Vec<String>) -> Result<OpOutcome, CoralError> {
    journaled(path, "revert", |r, l| async move {
        let targets: Vec<&str> = revs.iter().map(String::as_str).collect();
        l.revert(&r, &targets).await
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
    journaled(path, &format!("reset to {rev}"), |r, l| async move {
        l.reset(&r, &rev, mode.into()).await
    })
    .await?;
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
    let mut journal = Journal::load(&loc);

    let picked = if backwards {
        journal.undoable()
    } else {
        journal.redoable()
    };
    let entry = picked.cloned().ok_or_else(|| CoralError::Refused {
        label: if backwards { "undo" } else { "redo" },
        detail: format!("nothing to {}", if backwards { "undo" } else { "redo" }),
    })?;

    let (target, from) = if backwards {
        (&entry.before, &entry.after)
    } else {
        (&entry.after, &entry.before)
    };
    loc.restore_refs(&runner, target, from).await?;

    if backwards {
        journal.undone += 1;
    } else {
        journal.undone -= 1;
    }
    journal.save(&loc)?;

    let verb = if backwards { "undid" } else { "redid" };
    Ok(Done {
        what: format!("{verb} {}", entry.label),
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
