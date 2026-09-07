use coral_core::conflict::{Blocks, ConflictedFile, Operation, Resolution};
use coral_core::ops::OpAction;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

async fn located(path: &str) -> Result<(GitRunner, RepoLocation), IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    Ok((runner, loc))
}

/// The operation in progress, and what its two sides are called.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_operation(path: String) -> Result<Operation, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.operation(&runner).await?)
}

/// Files still needing a decision.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn repo_conflicts(path: String) -> Result<Vec<ConflictedFile>, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.conflicts(&runner).await?)
}

/// One conflicted file broken into blocks.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn conflict_blocks(path: String, file: String) -> Result<Blocks, IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.conflict_blocks(&runner, &file).await?)
}

/// How the user chose to settle one file.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Choice {
    Ours,
    Theirs,
    Delete,
    /// The exact text the merge tool produced, block decisions already applied.
    Content {
        text: String,
    },
}

/// Applies a resolution and stages the result.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] if the path is not conflicted.
#[tauri::command]
pub async fn resolve_conflict(path: String, file: String, choice: Choice) -> Result<(), IpcError> {
    let (runner, loc) = located(&path).await?;
    let resolution = match choice {
        Choice::Ours => Resolution::TakeOurs,
        Choice::Theirs => Resolution::TakeTheirs,
        Choice::Delete => Resolution::Delete,
        Choice::Content { text } => Resolution::Content(text.into()),
    };
    Ok(loc.resolve(&runner, &file, &resolution).await?)
}

/// Continues, aborts, or skips the operation in progress.
///
/// Bracketed by a ref snapshot like every other mutation, because this is where a conflicted
/// merge or rebase actually lands its commit. Without it the operation left nothing in the
/// journal, and the next Undo reached past it to an older entry whose refs had moved — which
/// git refused, in git's words, under a button that says Undo.
///
/// # Errors
/// Propagates git failures, including a continue that hits the next conflict.
#[tauri::command]
pub async fn operation_step(
    path: String,
    step: String,
) -> Result<coral_core::ops::OpOutcome, IpcError> {
    let (runner, loc) = located(&path).await?;
    let action = match step.as_str() {
        "abort" => OpAction::Abort,
        "skip" => OpAction::Skip,
        _ => OpAction::Continue,
    };
    // Read before the step, since finishing it is what clears the state.
    let label = journal_label(loc.operation(&runner).await?.state, &step);
    let before = loc.snapshot_refs(&runner).await?;
    let outcome = loc.op(&runner, action).await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(&label, before, after, coral_core::undo::Restore::Worktree)?;
    Ok(outcome)
}

/// What the journal, and so the undo tooltip, should call this step.
fn journal_label(state: coral_core::repo::OpState, step: &str) -> String {
    use coral_core::repo::OpState;
    let what = match state {
        OpState::Merge => "merge",
        OpState::Rebase => "rebase",
        OpState::CherryPick => "cherry-pick",
        OpState::Revert => "revert",
        OpState::Bisect => "bisect",
        OpState::Clean => "operation",
    };
    match step {
        "abort" => format!("abort the {what}"),
        "skip" => format!("skip a commit in the {what}"),
        _ => format!("finish the {what}"),
    }
}
