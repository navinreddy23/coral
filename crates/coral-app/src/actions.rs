use coral_core::ops::MergeMode;
use coral_core::process::GitRunner;
use coral_core::remote::{PullMode, PushOpts};
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// One thing the user asked the repository to do.
///
/// A tagged union rather than a command each: every one of these follows the same
/// snapshot-run-journal path, and splitting them across commands would mean repeating it.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Fetch {
        remote: Option<String>,
    },
    Pull {
        remote: Option<String>,
        mode: PullKind,
    },
    Push {
        remote: Option<String>,
        set_upstream: bool,
    },
    Checkout {
        rev: String,
    },
    BranchCreate {
        name: String,
        at: Option<String>,
        checkout: bool,
    },
    BranchDelete {
        name: String,
        force: bool,
    },
    Merge {
        rev: String,
    },
    Rebase {
        onto: String,
    },
    CherryPick {
        revs: Vec<String>,
    },
    Revert {
        revs: Vec<String>,
    },
    StashPush {
        message: Option<String>,
    },
    StashApply {
        index: usize,
        pop: bool,
    },
    StashDrop {
        index: usize,
    },
    TagCreate {
        name: String,
        at: Option<String>,
        message: Option<String>,
    },
    TagDelete {
        name: String,
    },
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PullKind {
    FfOnly,
    Merge,
    Rebase,
}

impl From<PullKind> for PullMode {
    fn from(k: PullKind) -> Self {
        match k {
            PullKind::FfOnly => Self::FfOnly,
            PullKind::Merge => Self::Merge,
            PullKind::Rebase => Self::Rebase,
        }
    }
}

/// What an action did, phrased for the status line.
// Hand-typed in `ui/src/ipc/commands.ts`, like the other shapes this crate defines: ts-rs
// generates from coral-core, which is where the types both front ends share live.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub what: String,
    /// The operation stopped with conflicts and the worktree needs attention.
    pub conflicted: bool,
}

impl Action {
    /// What the journal should call this, and the label a failure is reported under.
    fn label(&self) -> String {
        match self {
            Self::Fetch { .. } => "fetch".to_owned(),
            Self::Pull { .. } => "pull".to_owned(),
            Self::Push { .. } => "push".to_owned(),
            Self::Checkout { rev } => format!("checkout {rev}"),
            Self::BranchCreate { name, .. } => format!("create branch {name}"),
            Self::BranchDelete { name, .. } => format!("delete branch {name}"),
            Self::Merge { rev } => format!("merge {rev}"),
            Self::Rebase { onto } => format!("rebase onto {onto}"),
            Self::CherryPick { revs } => format!("cherry-pick {}", revs.join(" ")),
            Self::Revert { revs } => format!("revert {}", revs.join(" ")),
            Self::StashPush { .. } => "stash".to_owned(),
            Self::StashApply { pop: true, .. } => "stash pop".to_owned(),
            Self::StashApply { .. } => "stash apply".to_owned(),
            Self::StashDrop { .. } => "stash drop".to_owned(),
            Self::TagCreate { name, .. } => format!("tag {name}"),
            Self::TagDelete { name } => format!("delete tag {name}"),
            Self::Undo => "undo".to_owned(),
            Self::Redo => "redo".to_owned(),
        }
    }

    /// True for the two that move through the journal rather than adding to it.
    const fn steps_journal(&self) -> bool {
        matches!(self, Self::Undo | Self::Redo)
    }
}

/// Runs one action against a repository.
///
/// Every mutation is bracketed by a ref snapshot so undo works without each operation having
/// to describe what it changed. Undo and redo skip that, since they *are* the journal moving.
///
/// # Errors
/// Propagates git failures, including a merge or rebase that stopped on conflicts.
#[tauri::command]
pub async fn repo_action(path: String, action: Action) -> Result<ActionOutcome, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let label = action.label();

    if action.steps_journal() {
        let what = loc
            .undo_step(&runner, matches!(action, Action::Undo))
            .await?;
        return Ok(ActionOutcome {
            what,
            conflicted: false,
        });
    }

    let before = loc.snapshot_refs(&runner).await?;
    let conflicted = run(&loc, &runner, action).await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(&label, before, after)?;

    Ok(ActionOutcome {
        what: label,
        conflicted,
    })
}

/// Performs the action, reporting whether it stopped on conflicts.
async fn run(
    loc: &RepoLocation,
    runner: &GitRunner,
    action: Action,
) -> Result<bool, coral_core::CoralError> {
    match action {
        Action::Fetch { remote } => {
            // Prune: a fetch that leaves deleted remote branches in the sidebar is a fetch
            // that makes the sidebar wrong, which is the thing it was run to correct.
            loc.fetch(runner, remote.as_deref(), true, |_| {}).await?;
        }
        Action::Pull { remote, mode } => {
            let out = loc.pull(runner, remote.as_deref(), mode.into()).await?;
            return Ok(!out.conflicts.is_empty());
        }
        Action::Push {
            remote,
            set_upstream,
        } => {
            let opts = PushOpts {
                remote,
                set_upstream,
                ..PushOpts::default()
            };
            loc.push(runner, &opts, |_| {}).await?;
        }
        Action::Checkout { rev } => loc.checkout(runner, &rev).await?,
        Action::BranchCreate { name, at, checkout } => {
            loc.branch_create(runner, &name, at.as_deref(), checkout)
                .await?;
        }
        Action::BranchDelete { name, force } => loc.branch_delete(runner, &name, force).await?,
        Action::Merge { rev } => {
            let out = loc.merge(runner, &rev, MergeMode::default(), None).await?;
            return Ok(!out.conflicts.is_empty());
        }
        Action::Rebase { onto } => {
            // --update-refs carries any branches pointing inside the rebased range along with
            // it, which is what stops a stack of review branches being left behind.
            let out = loc.rebase(runner, &onto, true).await?;
            return Ok(!out.conflicts.is_empty());
        }
        Action::CherryPick { revs } => {
            let refs: Vec<&str> = revs.iter().map(String::as_str).collect();
            let out = loc.cherry_pick(runner, &refs).await?;
            return Ok(!out.conflicts.is_empty());
        }
        Action::Revert { revs } => {
            let refs: Vec<&str> = revs.iter().map(String::as_str).collect();
            let out = loc.revert(runner, &refs).await?;
            return Ok(!out.conflicts.is_empty());
        }
        Action::StashPush { message } => {
            // Untracked files are included: a stash that leaves them behind is a stash that
            // does not let the branch be switched, which is what it was asked for.
            loc.stash_push(runner, message.as_deref(), true).await?;
        }
        Action::StashApply { index, pop } => loc.stash_apply(runner, index, pop).await?,
        Action::StashDrop { index } => loc.stash_drop(runner, index).await?,
        Action::TagCreate { name, at, message } => {
            loc.tag_create(runner, &name, at.as_deref(), message.as_deref())
                .await?;
        }
        Action::TagDelete { name } => loc.tag_delete(runner, &name).await?,
        Action::Undo | Action::Redo => unreachable!("stepped above"),
    }
    Ok(false)
}

/// The todo list an interactive rebase onto `onto` would start from.
///
/// # Errors
/// Propagates git failures, including an unknown revision.
#[tauri::command]
pub async fn rebase_todo(
    path: String,
    onto: String,
) -> Result<coral_core::sequence::Todo, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    Ok(loc.rebase_todo(&runner, &onto).await?)
}

/// Runs an interactive rebase against a todo the user has decided.
///
/// # Errors
/// Propagates git failures. A rebase that stops on a conflict is an outcome, not an error.
#[tauri::command]
pub async fn rebase_start(
    path: String,
    onto: String,
    todo: coral_core::sequence::Todo,
) -> Result<ActionOutcome, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let binary = std::env::current_exe().map_err(|e| coral_core::CoralError::Protocol {
        label: "rebase",
        detail: format!("could not locate the running binary: {e}"),
    })?;

    let before = loc.snapshot_refs(&runner).await?;
    let outcome = loc
        .rebase_interactive(&runner, &onto, &todo, &binary)
        .await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(&format!("rebase onto {onto}"), before, after)?;

    Ok(ActionOutcome {
        what: format!("rebase onto {onto}"),
        conflicted: !outcome.conflicts.is_empty(),
    })
}
