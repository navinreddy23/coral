use coral_core::ops::MergeMode;
use coral_core::process::GitRunner;
use coral_core::remote::{PullMode, PushOpts};
use coral_core::repo::RepoLocation;

use crate::commands::IpcError;

/// One thing the user asked the repository to do.
///
/// A tagged union rather than a command each: every one of these follows the same
/// snapshot-run-journal path, and splitting them across commands would mean repeating it.
// `rename_all` renames the variants; the fields inside them need `rename_all_fields`, and
// without it `setUpstream` arrived as a field serde had never heard of. Push was the only
// action with a field of more than one word, so it was the only one that could not be run:
// "missing field `set_upstream`", from a window that had sent one.
#[derive(Debug, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
        /// One ref to push instead of the current branch, such as `refs/tags/v1.0`.
        #[serde(default)]
        refspec: Option<String>,
        /// Send every tag as well. Tags travel only when they are asked for.
        #[serde(default)]
        tags: bool,
        /// Overwrite what is on the remote, provided nothing has moved it since it was last
        /// fetched. `--force-with-lease`, never a bare force; see `remote.rs`.
        #[serde(default)]
        force_with_lease: bool,
        /// Remove the named ref from the remote instead of updating it.
        #[serde(default)]
        delete: bool,
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
    BranchRename {
        from: String,
        to: String,
    },
    Merge {
        rev: String,
        /// `FfOnly` refuses anything but a fast-forward, which is what "fast-forward to this"
        /// means and what makes it different from a merge.
        #[serde(default)]
        mode: MergeMode,
    },
    Rebase {
        onto: String,
    },
    CherryPick {
        revs: Vec<String>,
        /// Whether to record the result. False leaves it staged, for someone who wants to
        /// change it, split it, or fold it into something else first.
        commit: bool,
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
    /// Write a commit, or a range of them, out as patch files.
    Patch {
        /// The commit to export, or the newer end of a range.
        rev: String,
        /// The older end, exclusive. `None` exports `rev` on its own.
        from: Option<String>,
        /// Directory to write into.
        directory: String,
    },
    /// Apply patch files to the current branch.
    ApplyPatch {
        files: Vec<String>,
        /// Record a commit per patch, keeping its author. False leaves the changes uncommitted.
        commit: bool,
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

/// A ref as people say it: `refs/tags/v1.0` is "v1.0" to everyone but git.
fn bare_ref(name: &str) -> &str {
    name.strip_prefix("refs/tags/")
        .or_else(|| name.strip_prefix("refs/heads/"))
        .unwrap_or(name)
}

/// A revision as a label should read it: an object id shortened, a name left whole.
///
/// Every one of these was formatted with `{rev:.8}`, which is right for the forty characters
/// of an object id and cuts `origin/dummy-branch` down to `origin/d`. What follows the id is
/// kept, because `<oid>~1` is a place people recognise and `~1` alone is not.
fn named(rev: &str) -> std::borrow::Cow<'_, str> {
    let id = rev
        .split(|c: char| !c.is_ascii_hexdigit())
        .next()
        .unwrap_or(rev);
    if id.len() >= 40 {
        format!("{}{}", &id[..8], &rev[id.len()..]).into()
    } else {
        rev.into()
    }
}

/// A list of revisions, each shortened the way [`named`] shortens one.
fn shortened(revs: &[String]) -> String {
    revs.iter()
        .map(|r| named(r).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

impl Action {
    /// What the journal should call this, and the label a failure is reported under.
    fn label(&self) -> String {
        match self {
            Self::Fetch { .. } => "fetch".to_owned(),
            Self::Pull { .. } => "pull".to_owned(),
            // Before the arm below it, which matches any push naming a ref and would
            // otherwise swallow this one and journal a deletion as a push.
            Self::Push {
                delete: true,
                refspec: Some(refspec),
                ..
            } => format!("delete {} from the remote", bare_ref(refspec)),
            Self::Push {
                refspec: Some(refspec),
                ..
            } => format!("push {}", bare_ref(refspec)),
            Self::Push { tags: true, .. } => "push every tag".to_owned(),
            Self::Push {
                force_with_lease: true,
                ..
            } => "force push".to_owned(),
            Self::Push { .. } => "push".to_owned(),
            Self::Checkout { rev } => format!("checkout {}", named(rev)),
            Self::BranchCreate { name, .. } => format!("create branch {name}"),
            Self::BranchDelete { name, .. } => format!("delete branch {name}"),
            Self::BranchRename { from, to } => format!("rename {from} to {to}"),
            Self::Merge { rev, mode } => match mode {
                MergeMode::FfOnly => format!("fast-forward to {}", named(rev)),
                _ => format!("merge {}", named(rev)),
            },
            Self::Rebase { onto } => format!("rebase onto {}", named(onto)),
            Self::CherryPick { revs, commit } => {
                let what = shortened(revs);
                if *commit {
                    format!("cherry-pick {what}")
                } else {
                    format!("cherry-pick {what} without committing")
                }
            }
            Self::Revert { revs } => format!("revert {}", shortened(revs)),
            Self::StashPush { .. } => "stash".to_owned(),
            Self::StashApply { pop: true, .. } => "stash pop".to_owned(),
            Self::StashApply { .. } => "stash apply".to_owned(),
            Self::StashDrop { .. } => "stash drop".to_owned(),
            Self::TagCreate { name, .. } => format!("tag {name}"),
            Self::TagDelete { name } => format!("delete tag {name}"),
            Self::Reset { rev, .. } => format!("reset to {}", named(rev)),
            Self::Rewrite { rev, how, .. } => match how {
                RewriteKind::Drop => format!("drop {}", named(rev)),
                RewriteKind::Reword => format!("reword {}", named(rev)),
                RewriteKind::MoveNewer => format!("move {} up", named(rev)),
                RewriteKind::MoveOlder => format!("move {} down", named(rev)),
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
            Self::Patch { rev, from, .. } => match from {
                Some(from) => format!("patches for {}..{}", named(from), named(rev)),
                None => format!("patch for {}", named(rev)),
            },
            Self::ApplyPatch { files, .. } => match files.len() {
                1 => "apply a patch".to_owned(),
                n => format!("apply {n} patches"),
            },
            Self::Undo => "undo".to_owned(),
            Self::Redo => "redo".to_owned(),
        }
    }

    /// True for the two that move through the journal rather than adding to it.
    const fn steps_journal(&self) -> bool {
        matches!(self, Self::Undo | Self::Redo)
    }

    /// True for the three bounded by somebody else's server rather than by this machine.
    ///
    /// Only these are worth reporting and worth being able to stop: everything else finishes
    /// before a bar could be drawn, and a cancel button on a checkout would be furniture.
    const fn reaches_network(&self) -> bool {
        matches!(
            self,
            Self::Fetch { .. } | Self::Pull { .. } | Self::Push { .. }
        )
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
pub async fn repo_action(
    app: tauri::AppHandle,
    path: String,
    action: Action,
) -> Result<ActionOutcome, IpcError> {
    // Only what reaches the network is watched. A checkout finishes before a progress bar
    // could be drawn, and reporting one would be noise with a cancel button on it.
    if !action.reaches_network() {
        return run_action(&path, action).await;
    }
    let label = action.label();
    // Borrowed by the key and the label as well as by the work, so the block takes references
    // rather than the strings themselves.
    let (at, named) = (path.as_str(), label.as_str());

    // Opened out here and closed out here, rather than inside the watched work. Cancelling
    // drops that work where it stands, so an entry owned by it would be left saying the
    // operation started and never saying what became of it — in the log somebody opens to
    // find out exactly that.
    let entry = crate::activity::started(at, named);
    let outcome = crate::transfer::watched(&app, at, named, |report| async move {
        act(at, action, named, &|p| report.progress(p)).await
    })
    .await;

    match outcome {
        Ok(done) => {
            if done.conflicted {
                entry.stopped();
            } else {
                entry.finished();
            }
            Ok(done)
        }
        Err(e) => {
            entry.failed(&e.message);
            Err(e)
        }
    }
}

/// Runs one action, with nothing reported and nothing to stop it.
///
/// The whole of [`repo_action`] bar the window: a Tauri command needs an app handle and an
/// action does not, so this is the seam the tests drive rather than standing a window up to
/// check what a revert labels itself.
///
/// # Errors
/// Propagates git failures, including an operation that stopped on conflicts.
pub async fn run_action(path: &str, action: Action) -> Result<ActionOutcome, IpcError> {
    let label = action.label();
    logged(path, action, &label, &|_| {}).await
}

/// Runs an action and records what happened to it.
///
/// Both entry points come through here, so the activity log says the same thing whether the
/// window or a test asked for the work.
async fn logged(
    path: &str,
    action: Action,
    label: &str,
    report: &(dyn Fn(&coral_core::remote::Progress) + Sync),
) -> Result<ActionOutcome, IpcError> {
    // Recorded before anything can fail, so a repository that cannot even be discovered still
    // leaves the attempt in the log.
    let entry = crate::activity::started(path, label);
    match act(path, action, label, report).await {
        Ok(outcome) => {
            // `conflicted` is set by a merge or rebase that stopped, and by a push whose refs
            // the remote refused. Both used to be logged as "finished", so the record of a
            // rejected force push read exactly like the record of one that went through.
            if outcome.conflicted {
                entry.stopped();
            } else {
                entry.finished();
            }
            Ok(outcome)
        }
        Err(e) => {
            entry.failed(&e.message);
            Err(e)
        }
    }
}

async fn act(
    path: &str,
    action: Action,
    label: &str,
    report: &(dyn Fn(&coral_core::remote::Progress) + Sync),
) -> Result<ActionOutcome, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;

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
    let done = run(&loc, &runner, action, report).await?;
    let after = loc.snapshot_refs(&runner).await?;
    loc.journal_change(label, before, after, coral_core::undo::Restore::Worktree)?;

    Ok(ActionOutcome {
        what: label.to_owned(),
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
    report: &(dyn Fn(&coral_core::remote::Progress) + Sync),
) -> Result<Done, coral_core::CoralError> {
    match action {
        Action::Fetch { .. } | Action::Pull { .. } | Action::Push { .. } => {
            run_remote(loc, runner, action, report).await
        }
        Action::Checkout { .. }
        | Action::BranchCreate { .. }
        | Action::BranchDelete { .. }
        | Action::BranchRename { .. }
        | Action::Merge { .. }
        | Action::Rebase { .. }
        | Action::CherryPick { .. }
        | Action::Revert { .. }
        | Action::Reset { .. }
        | Action::Rewrite { .. }
        | Action::TagCreate { .. }
        | Action::TagDelete { .. }
        | Action::ApplyPatch { .. } => run_refs(loc, runner, action).await,
        _ => run_tree(loc, runner, action).await,
    }
}

/// The three that reach the network.
async fn run_remote(
    loc: &RepoLocation,
    runner: &GitRunner,
    action: Action,
    report: &(dyn Fn(&coral_core::remote::Progress) + Sync),
) -> Result<Done, coral_core::CoralError> {
    match action {
        Action::Fetch { remote } => {
            // Prune: a fetch that leaves deleted remote branches in the sidebar is a fetch
            // that makes the sidebar wrong, which is the thing it was run to correct.
            loc.fetch(runner, remote.as_deref(), true, |p| report(&p))
                .await?;
            Ok(Done::quiet())
        }
        Action::Pull { remote, mode } => {
            let out = loc
                .pull(runner, remote.as_deref(), mode.into(), |p| report(p))
                .await?;
            Ok(Done::from(&out))
        }
        Action::Push {
            remote,
            set_upstream,
            refspec,
            tags,
            force_with_lease,
            delete,
        } => {
            let opts = PushOpts {
                remote,
                set_upstream,
                refspec,
                tags,
                force_with_lease,
                delete,
            };
            let results = loc.push(runner, &opts, |p| report(&p)).await?;
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
        Action::BranchRename { from, to } => loc.branch_rename(runner, &from, &to).await?,
        Action::Merge { rev, mode } => {
            let out = loc.merge(runner, &rev, mode, None).await?;
            return Ok(Done::from(&out));
        }
        Action::Rebase { onto } => {
            // --update-refs carries any branches pointing inside the rebased range along with
            // it, which is what stops a stack of review branches being left behind.
            let out = loc.rebase(runner, &onto, true).await?;
            return Ok(Done::from(&out));
        }
        Action::CherryPick { revs, commit } => {
            let refs: Vec<&str> = revs.iter().map(String::as_str).collect();
            let out = loc.cherry_pick(runner, &refs, commit).await?;
            return Ok(Done::from(&out));
        }
        Action::Revert { revs } => {
            let refs: Vec<&str> = revs.iter().map(String::as_str).collect();
            let out = loc.revert(runner, &refs).await?;
            return Ok(Done::from(&out));
        }
        Action::Reset { rev, mode } => loc.reset(runner, &rev, mode.into()).await?,
        Action::ApplyPatch { files, commit } => {
            let landing = if commit {
                coral_core::patch::PatchLanding::Commit
            } else {
                coral_core::patch::PatchLanding::WorkingTree
            };
            let paths: Vec<std::path::PathBuf> =
                files.iter().map(std::path::PathBuf::from).collect();
            let out = loc.apply_patches(runner, &paths, landing).await?;
            return Ok(Done::from(&out));
        }
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
        Action::Patch {
            rev,
            from,
            directory,
        } => {
            let into = std::path::Path::new(&directory);
            match from {
                Some(from) => loc.format_patch_range(runner, &from, &rev, into).await?,
                None => loc.format_patch(runner, &rev, into).await?,
            };
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
    // Named the way the menu names it. The journal keeps this text for the life of the entry,
    // so an undo months later read "undid rebase onto <forty characters>~1".
    let what = format!("rebase onto {}", named(&onto));
    loc.journal_change(&what, before, after, coral_core::undo::Restore::Worktree)?;

    Ok(ActionOutcome {
        what,
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

#[cfg(test)]
mod tests {
    use super::{Action, ResetKind, named};

    #[test]
    fn an_object_id_is_shortened_and_a_ref_name_is_not() {
        assert_eq!(named(&"a".repeat(40)), "aaaaaaaa");
        assert_eq!(named("origin/dummy-branch"), "origin/dummy-branch");
        // Long enough to be an id by length, but not hexadecimal.
        assert_eq!(
            named("origin/a-very-long-branch-name-goes-here-x"),
            "origin/a-very-long-branch-name-goes-here-x"
        );
        // What follows the id is kept: an interactive rebase names the parent this way, and
        // the journal read "rebase onto d38081e90dcfd1183977998a03aed3fe8e324949~1".
        assert_eq!(named(&format!("{}~1", "b".repeat(40))), "bbbbbbbb~1");
        assert_eq!(named(&format!("{}^2", "c".repeat(40))), "cccccccc^2");
    }

    /// The journal is read to find out what happened, and "push" and "force push" are not the
    /// same thing to anybody looking for the one that replaced a branch on a remote.
    #[test]
    fn a_forced_push_is_journalled_as_one() {
        let forced = Action::Push {
            remote: None,
            set_upstream: true,
            refspec: None,
            tags: false,
            force_with_lease: true,
            delete: false,
        };
        assert_eq!(forced.label(), "force push");

        let plain = Action::Push {
            remote: None,
            set_upstream: true,
            refspec: None,
            tags: false,
            force_with_lease: false,
            delete: false,
        };
        assert_eq!(plain.label(), "push");
    }

    /// The journal is what somebody reads to find out what removed a branch from a remote.
    /// "push probe" is not that, and the arm that says so has to come before the one that
    /// matches any push naming a ref.
    #[test]
    fn deleting_a_branch_from_a_remote_is_journalled_as_a_deletion() {
        let removed = Action::Push {
            remote: Some("github".to_owned()),
            set_upstream: false,
            refspec: Some("probe/one".to_owned()),
            tags: false,
            force_with_lease: false,
            delete: true,
        };
        assert_eq!(removed.label(), "delete probe/one from the remote");
    }

    #[test]
    fn pushing_a_tag_is_labelled_with_the_tag() {
        let action = Action::Push {
            remote: Some("origin".to_owned()),
            set_upstream: false,
            refspec: Some("refs/tags/v1.0".to_owned()),
            tags: false,
            force_with_lease: false,
            delete: false,
        };
        assert_eq!(action.label(), "push v1.0");

        let all = Action::Push {
            remote: None,
            set_upstream: false,
            refspec: None,
            tags: true,
            force_with_lease: false,
            delete: false,
        };
        assert_eq!(all.label(), "push every tag");
    }

    #[test]
    fn a_reset_to_a_branch_is_labelled_with_the_branch() {
        let action = Action::Reset {
            rev: "origin/dummy-branch".to_owned(),
            mode: ResetKind::Hard,
        };
        assert_eq!(action.label(), "reset to origin/dummy-branch");
    }
}
