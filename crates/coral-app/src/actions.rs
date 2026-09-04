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
    /// Move the current branch, and optionally the index and worktree, to a commit.
    Reset {
        rev: String,
        mode: ResetKind,
    },
    /// Drop, reword or reorder one commit, replaying everything above it.
    Rewrite {
        rev: String,
        how: RewriteKind,
        /// The replacement message, for a reword.
        message: Option<String>,
    },
    /// Check a commit out into a working tree of its own.
    WorktreeAdd {
        /// Where the new working tree goes.
        path: String,
        rev: String,
        /// Create this branch there rather than detaching.
        branch: Option<String>,
    },
    /// Clone and check out a submodule's working copy, or move it to its branch tip.
    SubmoduleInit {
        /// The submodule's path within the repository. All of them when absent.
        path: Option<String>,
        recursive: bool,
        /// Move it to the tip of its configured branch rather than to the recorded commit.
        remote: bool,
    },
    /// Change where a submodule is cloned from.
    SubmoduleSetUrl {
        path: String,
        url: String,
    },
    /// Remove a submodule: its working copy, its configuration, and its clone.
    SubmoduleRemove {
        path: String,
        force: bool,
    },
    /// Write a commit out as a patch file.
    Patch {
        rev: String,
        /// Directory to write into.
        directory: String,
    },
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetKind {
    Soft,
    Mixed,
    Hard,
}

impl From<ResetKind> for coral_core::ops::ResetMode {
    fn from(k: ResetKind) -> Self {
        match k {
            ResetKind::Soft => Self::Soft,
            ResetKind::Mixed => Self::Mixed,
            ResetKind::Hard => Self::Hard,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RewriteKind {
    Drop,
    Reword,
    MoveNewer,
    MoveOlder,
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
    /// git's own words about what happened, when there are any.
    ///
    /// Carried because "Already up to date." is a different answer from a pull that brought
    /// commits, and without it both arrive as the same success.
    pub message: String,
}

/// What running one action produced, before it is phrased for the window.
struct Done {
    conflicted: bool,
    message: String,
}

impl Done {
    fn quiet() -> Self {
        Self {
            conflicted: false,
            message: String::new(),
        }
    }

    fn from(outcome: &coral_core::ops::OpOutcome) -> Self {
        Self {
            conflicted: !outcome.conflicts.is_empty(),
            message: outcome.message.clone(),
        }
    }
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
            Self::Reset { rev, .. } => format!("reset to {rev:.8}"),
            Self::Rewrite { rev, how, .. } => match how {
                RewriteKind::Drop => format!("drop {rev:.8}"),
                RewriteKind::Reword => format!("reword {rev:.8}"),
                RewriteKind::MoveNewer => format!("move {rev:.8} up"),
                RewriteKind::MoveOlder => format!("move {rev:.8} down"),
            },
            Self::WorktreeAdd { path, .. } => format!("worktree at {path}"),
            Self::SubmoduleInit {
                path: Some(p),
                remote: true,
                ..
            } => format!("update {p} to its branch tip"),
            Self::SubmoduleInit { path: Some(p), .. } => format!("update {p}"),
            Self::SubmoduleInit { path: None, .. } => "update the submodules".to_owned(),
            Self::SubmoduleSetUrl { path, .. } => format!("re-point {path}"),
            Self::SubmoduleRemove { path, .. } => format!("remove {path}"),
            Self::Patch { rev, .. } => format!("patch for {rev:.8}"),
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
            message: String::new(),
        });
    }

    let before = loc.snapshot_refs(&runner).await?;
    let done = run(&loc, &runner, action).await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(&label, before, after)?;

    Ok(ActionOutcome {
        what: label,
        conflicted: done.conflicted,
        message: done.message,
    })
}

/// Performs the action, reporting whether it stopped and what git said.
///
/// Split by what the action is about rather than by size: the three groups below touch the
/// network, the refs, and the working tree respectively, and nothing crosses between them.
async fn run(
    loc: &RepoLocation,
    runner: &GitRunner,
    action: Action,
) -> Result<Done, coral_core::CoralError> {
    match action {
        Action::Fetch { .. } | Action::Pull { .. } | Action::Push { .. } => {
            run_remote(loc, runner, action).await
        }
        Action::Checkout { .. }
        | Action::BranchCreate { .. }
        | Action::BranchDelete { .. }
        | Action::Merge { .. }
        | Action::Rebase { .. }
        | Action::CherryPick { .. }
        | Action::Revert { .. }
        | Action::Reset { .. }
        | Action::Rewrite { .. }
        | Action::TagCreate { .. }
        | Action::TagDelete { .. } => run_refs(loc, runner, action).await,
        _ => run_tree(loc, runner, action).await,
    }
}

/// The three that reach the network.
async fn run_remote(
    loc: &RepoLocation,
    runner: &GitRunner,
    action: Action,
) -> Result<Done, coral_core::CoralError> {
    match action {
        Action::Fetch { remote } => {
            // Prune: a fetch that leaves deleted remote branches in the sidebar is a fetch
            // that makes the sidebar wrong, which is the thing it was run to correct.
            loc.fetch(runner, remote.as_deref(), true, |_| {}).await?;
            Ok(Done::quiet())
        }
        Action::Pull { remote, mode } => {
            let out = loc.pull(runner, remote.as_deref(), mode.into()).await?;
            Ok(Done::from(&out))
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
            let results = loc.push(runner, &opts, |_| {}).await?;
            // git's per-ref answers, which is the only place "Everything up-to-date" and a
            // rejection are told apart.
            let message = results
                .iter()
                .map(|r| format!("{} -> {} {}", r.local, r.remote, r.summary))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(Done {
                conflicted: results.iter().any(|r| r.flag.is_failure()),
                message,
            })
        }
        _ => unreachable!("routed by `run`"),
    }
}

/// Everything that moves a ref, including the three that rewrite history.
async fn run_refs(
    loc: &RepoLocation,
    runner: &GitRunner,
    action: Action,
) -> Result<Done, coral_core::CoralError> {
    match action {
        Action::Checkout { rev } => loc.checkout(runner, &rev).await?,
        Action::BranchCreate { name, at, checkout } => {
            loc.branch_create(runner, &name, at.as_deref(), checkout)
                .await?;
        }
        Action::BranchDelete { name, force } => loc.branch_delete(runner, &name, force).await?,
        Action::Merge { rev } => {
            let out = loc.merge(runner, &rev, MergeMode::default(), None).await?;
            return Ok(Done::from(&out));
        }
        Action::Rebase { onto } => {
            // --update-refs carries any branches pointing inside the rebased range along with
            // it, which is what stops a stack of review branches being left behind.
            let out = loc.rebase(runner, &onto, true).await?;
            return Ok(Done::from(&out));
        }
        Action::CherryPick { revs } => {
            let refs: Vec<&str> = revs.iter().map(String::as_str).collect();
            let out = loc.cherry_pick(runner, &refs).await?;
            return Ok(Done::from(&out));
        }
        Action::Revert { revs } => {
            let refs: Vec<&str> = revs.iter().map(String::as_str).collect();
            let out = loc.revert(runner, &refs).await?;
            return Ok(Done::from(&out));
        }
        Action::Reset { rev, mode } => loc.reset(runner, &rev, mode.into()).await?,
        Action::Rewrite { rev, how, message } => {
            let rewrite = match how {
                RewriteKind::Drop => coral_core::sequence::Rewrite::Drop,
                RewriteKind::Reword => {
                    coral_core::sequence::Rewrite::Reword(message.unwrap_or_default())
                }
                RewriteKind::MoveNewer => coral_core::sequence::Rewrite::MoveNewer,
                RewriteKind::MoveOlder => coral_core::sequence::Rewrite::MoveOlder,
            };
            let out = loc
                .rewrite_commit(runner, &rev, &rewrite, &coral_binary()?)
                .await?;
            return Ok(Done::from(&out));
        }
        Action::TagCreate { name, at, message } => {
            loc.tag_create(runner, &name, at.as_deref(), message.as_deref())
                .await?;
        }
        Action::TagDelete { name } => loc.tag_delete(runner, &name).await?,
        _ => unreachable!("routed by `run`"),
    }
    Ok(Done::quiet())
}

/// The stash, the submodules, a linked worktree, and a patch file.
async fn run_tree(
    loc: &RepoLocation,
    runner: &GitRunner,
    action: Action,
) -> Result<Done, coral_core::CoralError> {
    match action {
        Action::StashPush { message } => {
            // Untracked files are included: a stash that leaves them behind is a stash that
            // does not let the branch be switched, which is what it was asked for.
            loc.stash_push(runner, message.as_deref(), true).await?;
        }
        Action::StashApply { index, pop } => loc.stash_apply(runner, index, pop).await?,
        Action::StashDrop { index } => loc.stash_drop(runner, index).await?,
        Action::WorktreeAdd { path, rev, branch } => {
            loc.worktree_add(runner, std::path::Path::new(&path), &rev, branch.as_deref())
                .await?;
        }
        Action::SubmoduleInit {
            path,
            recursive,
            remote,
        } => {
            loc.submodule_init(runner, path.as_deref(), recursive, remote)
                .await?;
        }
        Action::SubmoduleSetUrl { path, url } => {
            loc.submodule_set_url(runner, &path, &url).await?;
        }
        Action::SubmoduleRemove { path, force } => {
            loc.submodule_remove(runner, &path, force).await?;
        }
        Action::Patch { rev, directory } => {
            loc.format_patch(runner, &rev, std::path::Path::new(&directory))
                .await?;
        }
        Action::Undo | Action::Redo => unreachable!("stepped above"),
        _ => unreachable!("routed by `run`"),
    }
    Ok(Done::quiet())
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
    let binary = coral_binary()?;

    let before = loc.snapshot_refs(&runner).await?;
    let outcome = loc
        .rebase_interactive(&runner, &onto, &todo, &binary)
        .await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(&format!("rebase onto {onto}"), before, after)?;

    Ok(ActionOutcome {
        what: format!("rebase onto {onto}"),
        conflicted: !outcome.conflicts.is_empty(),
        message: outcome.message,
    })
}

/// Where this application is, so git can be pointed back at it as its sequence editor.
///
/// The same self-invocation the credential helper uses: a packaged application cannot assume
/// the CLI is installed, let alone on the PATH.
fn coral_binary() -> Result<std::path::PathBuf, coral_core::CoralError> {
    std::env::current_exe().map_err(|e| coral_core::CoralError::Protocol {
        label: "rebase",
        detail: format!("could not locate the running binary: {e}"),
    })
}
