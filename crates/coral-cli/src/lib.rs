pub mod commands;
pub mod output;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use coral_core::index::Direction as D;

#[derive(Parser, Debug)]
#[command(
    name = "coral",
    version,
    about = "A fast Git client engine, driven from the terminal"
)]
pub struct Cli {
    /// Repository to operate on. Defaults to the current directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub repo: Option<PathBuf>,

    /// Emit the JSON envelope instead of human-readable output.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Validate a repository and report git version, HEAD, operation state and commit-graph.
    Open,
    /// Create an empty repository.
    Init {
        /// Where it goes. Created if it is not there.
        path: PathBuf,
        /// The name of the first branch. Left to git's own default when absent.
        #[arg(long, value_name = "NAME")]
        branch: Option<String>,
        /// Set Large File Storage up in it.
        #[arg(long)]
        lfs: bool,
    },
    /// Clone a repository.
    Clone {
        url: String,
        /// The directory to clone *into*; the repository appears under it. Defaults to `--repo`.
        #[arg(long, value_name = "DIR")]
        into: Option<PathBuf>,
        /// What to call the directory. Uses the name in the URL, as git does, when absent.
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
        /// The private ssh key to authenticate with, recorded in the repository afterwards.
        #[arg(long, value_name = "PATH")]
        ssh_key: Option<String>,
        /// Fetch only this many commits of history, and only the branch being cloned.
        #[arg(long, value_name = "N")]
        depth: Option<u32>,
        /// Leave file contents on the server until something reads one. History stays whole.
        #[arg(long)]
        blobless: bool,
        /// Do not draw progress on stderr.
        #[arg(long)]
        quiet: bool,
    },
    /// Report the working tree state.
    Status,
    /// Record a commit from the staged changes.
    Commit {
        #[arg(short, long)]
        message: String,
        /// Replace the previous commit rather than adding one.
        #[arg(long)]
        amend: bool,
        /// Append a Signed-off-by trailer.
        #[arg(long)]
        signoff: bool,
        /// Override the author, as "Name <email>".
        #[arg(long)]
        author: Option<String>,
        /// Commit even with nothing staged.
        #[arg(long)]
        allow_empty: bool,
    },
    /// Merge a revision into the current branch.
    Merge {
        rev: String,
        #[arg(long, value_enum, default_value_t = commands::write::Mode::Auto)]
        mode: commands::write::Mode,
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Replay the current branch onto another.
    Rebase {
        onto: String,
        /// Move branches that point into the replayed range.
        #[arg(long)]
        update_refs: bool,
    },
    /// Apply commits onto the current branch.
    CherryPick {
        #[arg(required = true)]
        revs: Vec<String>,
    },
    /// Record commits that undo others.
    Revert {
        #[arg(required = true)]
        revs: Vec<String>,
    },
    /// Move the current branch, and optionally the index and worktree.
    Reset {
        rev: String,
        #[arg(long, value_enum, default_value_t = commands::write::Reset::Mixed)]
        mode: commands::write::Reset,
    },
    /// Switch to a revision.
    Checkout { rev: String },
    /// Continue, abort or skip the operation in progress.
    Op {
        #[arg(value_enum)]
        action: commands::write::Action,
    },
    /// Create a branch.
    BranchCreate {
        name: String,
        /// Start the branch here instead of at HEAD.
        #[arg(long)]
        at: Option<String>,
        /// Check it out after creating it.
        #[arg(long)]
        checkout: bool,
    },
    /// Delete a branch.
    BranchDelete {
        name: String,
        /// Delete even when the branch is not merged.
        #[arg(long)]
        force: bool,
    },
    /// Rename a branch.
    BranchRename { from: String, to: String },
    /// Create a tag. With a message it is annotated.
    TagCreate {
        name: String,
        #[arg(long)]
        at: Option<String>,
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Delete a tag.
    TagDelete { name: String },
    /// Push, apply, pop or drop a stash entry.
    Stash {
        #[arg(value_enum)]
        action: commands::write::StashAction,
        /// Which entry to act on.
        #[arg(long, default_value_t = 0)]
        index: usize,
        #[arg(short, long)]
        message: Option<String>,
        /// Include untracked files when pushing.
        #[arg(long)]
        include_untracked: bool,
    },
    /// List configured remotes.
    Remotes,
    /// Add, remove, rename, re-point or prune a remote.
    Remote {
        /// One of: add, remove, rename, set-url, prune.
        action: String,
        name: String,
        /// The URL for add and set-url, or the new name for rename.
        value: Option<String>,
    },
    /// Fetch from a remote, or from all of them.
    Fetch {
        remote: Option<String>,
        /// Drop tracking branches whose remote counterparts are gone.
        #[arg(long)]
        prune: bool,
    },
    /// Push to a remote.
    Push {
        remote: Option<String>,
        refspec: Option<String>,
        #[arg(long)]
        set_upstream: bool,
        /// Overwrite the remote branch, but only if it is where we last saw it. There is no
        /// bare --force: it silently destroys work pushed by someone else.
        #[arg(long)]
        force_with_lease: bool,
        #[arg(long)]
        tags: bool,
        /// Delete the remote branch rather than updating it.
        #[arg(long)]
        delete: bool,
    },
    /// Fetch and integrate.
    Pull {
        remote: Option<String>,
        #[arg(long, value_enum, default_value_t = commands::remote::Mode::FfOnly)]
        mode: commands::remote::Mode,
    },
    /// Answer a `git credential` request. Git runs this; people do not.
    CredentialHelper {
        /// One of: get, store, erase.
        action: String,
        /// The nonce the running application shares with the git children it spawns.
        #[arg(long)]
        session: Option<String>,
    },
    /// List the files still needing a decision.
    Conflicts,
    /// Show one file's conflict blocks, rebuilt from the index stages.
    ConflictShow { file: String },
    /// Resolve one conflicted file.
    ConflictResolve {
        file: String,
        #[arg(long, value_enum)]
        how: commands::conflicts::How,
    },
    /// Reverse the most recent operation.
    Undo,
    /// Replay the most recently undone operation.
    Redo,
    /// Show the undo stack.
    Journal,
    /// Stage changes.
    Stage {
        /// Paths to stage whole.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
        /// Stage one hunk of a file instead. Use with --file.
        #[arg(long, requires = "file")]
        hunk: Option<usize>,
        /// The file a --hunk or --lines selection applies to.
        #[arg(long)]
        file: Option<String>,
        /// Stage only these line indices within --hunk.
        #[arg(long, requires = "hunk", value_delimiter = ',')]
        lines: Option<Vec<usize>>,
    },
    /// Unstage changes, leaving the worktree alone.
    Unstage {
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
        #[arg(long, requires = "file")]
        hunk: Option<usize>,
        #[arg(long)]
        file: Option<String>,
        #[arg(long, requires = "hunk", value_delimiter = ',')]
        lines: Option<Vec<usize>>,
    },
    /// Throw away worktree changes. This cannot be undone from the index.
    Discard {
        #[arg(value_name = "PATH", required = true)]
        paths: Vec<String>,
    },
    /// Show changes to the worktree, or to the index with --staged.
    Diff {
        /// Diff the index against HEAD rather than the worktree against the index.
        #[arg(long)]
        staged: bool,
        /// Limit to these paths.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },
    /// Read commit history.
    Log {
        /// Start from this revision.
        #[arg(long, default_value = "HEAD")]
        rev: String,
        /// Limit to commits touching this path.
        #[arg(long)]
        path: Option<String>,
        /// Follow the path across renames. Requires --path.
        #[arg(long, requires = "path")]
        follow: bool,
        #[arg(long)]
        author: Option<String>,
        /// Match the commit message.
        #[arg(long)]
        grep: Option<String>,
        /// Match added or removed content.
        #[arg(long)]
        search: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u64,
    },
    /// Attribute each line of a file to the commit that last changed it.
    Blame {
        /// Revision to blame at.
        #[arg(long, default_value = "HEAD")]
        rev: String,
        /// File to blame.
        file: String,
    },
    /// List refs.
    Refs {
        #[arg(long, value_enum, default_value_t = commands::refs::Kind::All)]
        kind: commands::refs::Kind,
    },
    /// List submodules.
    Submodules,
    /// Report commit signing at every level for this repository.
    Signing,
    /// List the keys that could sign.
    SigningKeys,
    /// Override commit signing for this repository.
    SigningSet {
        /// The key to sign with.
        #[arg(long)]
        key: Option<String>,
        /// One of: openpgp, x509, ssh.
        #[arg(long)]
        format: Option<String>,
        /// Whether commits are signed here.
        #[arg(long)]
        sign_commits: Option<bool>,
        /// Drop every override so the app-level settings apply again.
        #[arg(long)]
        inherit: bool,
    },
    /// List the repository's working trees.
    Worktrees,
    /// Check a revision out into a new working tree.
    WorktreeAdd {
        /// Where the new working tree goes.
        path: PathBuf,
        /// What to check out there. Defaults to HEAD.
        #[arg(long, default_value = "HEAD")]
        rev: String,
        /// Create this branch there rather than detaching.
        #[arg(long)]
        branch: Option<String>,
    },
    /// Remove a working tree.
    WorktreeRemove {
        path: PathBuf,
        /// Remove it even with uncommitted changes.
        #[arg(long)]
        force: bool,
    },
    /// Write a commit out as a patch file.
    Patch {
        /// The commit to export. Defaults to HEAD.
        #[arg(long, default_value = "HEAD")]
        rev: String,
        /// Export everything after this commit up to --rev, rather than --rev alone.
        #[arg(long)]
        from: Option<String>,
        /// Directory to write into.
        #[arg(long, default_value = ".")]
        out: PathBuf,
    },
    /// Apply patch files to the current branch.
    ApplyPatch {
        /// A patch file. Repeat for a series; they are applied in the order given.
        #[arg(long = "file", required = true, value_name = "PATH")]
        files: Vec<PathBuf>,
        /// Leave the changes in the worktree instead of recording a commit for each patch.
        #[arg(long)]
        no_commit: bool,
    },
    /// Drop, reword or reorder one commit, replaying everything above it.
    Rewrite {
        /// The commit to change.
        rev: String,
        #[arg(long, value_enum)]
        kind: commands::rewrite::Kind,
        /// The replacement message, for --kind reword.
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Show the todo list an interactive rebase onto a revision would start from.
    RebaseTodo {
        /// The commit to rebase onto.
        onto: String,
    },
    /// Replace a rebase todo file with a prepared one. Git invokes this, not a person.
    #[command(hide = true)]
    RebaseEditor {
        /// The prepared list to install.
        #[arg(long)]
        todo: String,
        /// The file git wants edited.
        file: String,
    },
    /// Report the hosting provider behind the repository's remote.
    Host,
    /// Store an API token for the repository's host, read from stdin.
    HostLogin,
    /// Forget the stored API token for the repository's host.
    HostLogout,
    /// List the repository's pull or merge requests.
    PullRequests,
    /// Walk the commit graph and report rows with their lanes.
    Graph {
        /// Stop after this many commits. Note that this does not make a topological walk
        /// cheap: the order requires prepainting the whole graph before the first row, so a
        /// small limit costs about as much as no limit. Use --first-paint for a fast screen.
        #[arg(long)]
        limit: Option<u64>,
        /// First row to print; the walk still covers everything before it.
        #[arg(long, default_value_t = 0)]
        from: u32,
        /// Use the fast commit-time order the UI paints first, rather than topological order.
        #[arg(long)]
        first_paint: bool,
        /// Walk only this ref, by full name. The window calls this soloing a branch.
        #[arg(long)]
        solo: Option<String>,
        /// Leave this ref out of the walk, by full name. Repeatable. Its commits still appear
        /// when a ref that is walked reaches them, which is what git itself would say.
        #[arg(long = "hide")]
        hidden: Vec<String>,
    },
    /// Report the git binary coral will drive.
    Version,
}

/// Runs one CLI invocation and returns the envelope plus the exit code.
///
/// Returning rather than printing is what lets the snapshot tests run in-process instead of
/// spawning a binary per case.
///
/// # Errors
/// Never returns `Err`; failures are encoded in the returned envelope. The `Result` is the
/// clap parse outcome only.
pub async fn run(argv: Vec<OsString>) -> output::Rendered {
    let cli = match Cli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) => {
            let value = serde_json::json!({
                "schema": output::SCHEMA,
                "ok": false,
                "error": { "code": "usage", "message": e.to_string() }
            });
            return output::Rendered {
                json: value,
                text: e.to_string(),
                code: ExitCode::from(output::exit::USAGE),
            };
        }
    };

    // Makes git ask this binary for credentials rather than a terminal it cannot prompt on.
    // Not done when serving as the helper itself: that invocation spawns no git and needs no
    // configuration, and generating a nonce there would only slow it down.
    if !matches!(
        cli.command,
        Command::CredentialHelper { .. } | Command::RebaseEditor { .. }
    ) && let (Ok(binary), Some(session)) = (
        std::env::current_exe(),
        coral_core::credential::new_session(),
    ) {
        coral_core::credential::configure(binary, session);
    }

    // The sequence editor answers git, not a person. It writes nothing at all: git reads the
    // todo file back, so anything on stdout is noise and a JSON envelope would be read as a
    // rebase instruction.
    if let Command::RebaseEditor { todo, file } = &cli.command {
        return match std::fs::copy(todo, file) {
            Ok(_) => output::Rendered {
                json: serde_json::Value::Null,
                text: String::new(),
                code: ExitCode::SUCCESS,
            },
            Err(e) => output::Rendered {
                json: serde_json::Value::Null,
                text: format!("coral rebase-editor: {e}"),
                code: ExitCode::from(1),
            },
        };
    }

    // The credential helper answers git, not a person: its output is the protocol itself, so
    // it must not be wrapped in the JSON envelope.
    if let Command::CredentialHelper { action, session } = &cli.command {
        return match commands::credential::serve(action, session.as_deref()) {
            Ok(response) => output::Rendered {
                json: serde_json::Value::Null,
                text: response,
                code: ExitCode::SUCCESS,
            },
            // A helper that fails aborts the whole git operation, so even an error answers
            // empty; the message goes to stderr for a human reading the logs.
            Err(e) => {
                eprintln!("coral credential-helper: {e}");
                output::Rendered {
                    json: serde_json::Value::Null,
                    text: String::new(),
                    code: ExitCode::SUCCESS,
                }
            }
        };
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let repo = cli.repo.unwrap_or(cwd);
    dispatch(cli.command, &repo).await
}

/// Routes a parsed command. Split from `run` only to keep each half readable.
async fn dispatch(command: Command, repo: &std::path::Path) -> output::Rendered {
    match command {
        Command::Open => output::render(&commands::open::run(repo).await),
        // Neither of these operates on an existing repository, so `--repo` is only where a
        // relative destination is resolved from.
        Command::Init { path, branch, lfs } => {
            output::render(&commands::create::init(&repo.join(path), branch, lfs).await)
        }
        Command::Clone {
            url,
            into,
            name,
            ssh_key,
            depth,
            blobless,
            quiet,
        } => {
            let into = into.map_or_else(|| repo.to_path_buf(), |dir| repo.join(dir));
            let name = name.filter(|n| !n.trim().is_empty());
            let key = ssh_key.filter(|k| !k.trim().is_empty());
            output::render(
                &commands::create::clone(url, &into, name, key, depth, blobless, quiet).await,
            )
        }
        Command::Status => output::render(&commands::status::run(repo).await),
        Command::Diff { staged, paths } => {
            output::render(&commands::diff::run(repo, staged, &paths).await)
        }
        Command::Log {
            rev,
            path,
            follow,
            author,
            grep,
            search,
            limit,
        } => {
            let q = coral_core::history::LogQuery {
                rev: Some(rev),
                path,
                follow,
                author,
                grep,
                pickaxe: search,
                limit: Some(limit),
            };
            output::render(&commands::log::run(repo, &q).await)
        }
        Command::Blame { rev, file } => {
            output::render(&commands::blame::run(repo, &rev, &file).await)
        }
        Command::Refs { kind } => output::render(&commands::refs::run(repo, kind).await),
        Command::Submodules => output::render(&commands::submodule::run(repo).await),
        Command::Worktrees => output::render(&commands::worktree::list(repo).await),
        Command::Signing => output::render(&commands::signing::show(repo).await),
        Command::SigningKeys => output::render(&commands::signing::keys(repo).await),
        Command::SigningSet {
            key,
            format,
            sign_commits,
            inherit,
        } => {
            output::render(&commands::signing::set(repo, key, format, sign_commits, inherit).await)
        }
        Command::RebaseTodo { onto } => output::render(&commands::rebase::todo(repo, &onto).await),
        Command::Host => output::render(&commands::hosting::detect(repo).await),
        Command::HostLogin => {
            // Read from stdin rather than an argument: a token on a command line is in the
            // shell history and in every `ps` listing until the process exits.
            let mut token = String::new();
            if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut token) {
                return output::render::<commands::hosting::Done>(&Err(
                    coral_core::CoralError::Protocol {
                        label: "stdin",
                        detail: e.to_string(),
                    },
                ));
            }
            let secret = secrecy::SecretString::from(token.trim().to_owned());
            output::render(&commands::hosting::login(repo, secret).await)
        }
        Command::HostLogout => output::render(&commands::hosting::logout(repo).await),
        Command::PullRequests => output::render(&commands::hosting::list(repo).await),
        Command::Graph {
            limit,
            from,
            first_paint,
            solo,
            hidden,
        } => output::render(
            &commands::graph::run(repo, limit, from, first_paint, solo, hidden).await,
        ),
        Command::Version => output::render(&commands::version::run().await),
        other => dispatch_write(other, repo).await,
    }
}

/// Remote-facing commands, split out so each dispatcher stays readable.
async fn dispatch_remote(command: Command, repo: &std::path::Path) -> output::Rendered {
    match command {
        Command::Remotes => output::render(&commands::remote::list(repo).await),
        Command::Remote {
            action,
            name,
            value,
        } => {
            output::render(&commands::remote::manage(repo, &action, &name, value.as_deref()).await)
        }
        Command::Fetch { remote, prune } => {
            output::render(&commands::remote::fetch(repo, remote, prune).await)
        }
        Command::Push {
            remote,
            refspec,
            set_upstream,
            force_with_lease,
            tags,
            delete,
        } => {
            let opts = coral_core::remote::PushOpts {
                remote,
                refspec,
                set_upstream,
                force_with_lease,
                tags,
                delete,
            };
            output::render(&commands::remote::push(repo, opts).await)
        }
        Command::Pull { remote, mode } => {
            output::render_op(&commands::remote::pull(repo, remote, mode).await)
        }
        // Every other variant is handled before reaching here.
        _ => unreachable!("non-remote command routed to the remote dispatcher"),
    }
}

async fn dispatch_write(command: Command, repo: &std::path::Path) -> output::Rendered {
    match command {
        Command::Commit {
            message,
            amend,
            signoff,
            author,
            allow_empty,
        } => {
            let opts = coral_core::ops::CommitOpts {
                message,
                amend,
                signoff,
                author,
                allow_empty,
            };
            output::render(&commands::write::commit(repo, opts).await)
        }
        Command::Merge { rev, mode, message } => {
            output::render_op(&commands::write::merge(repo, rev, mode, message).await)
        }
        Command::Rebase { onto, update_refs } => {
            output::render_op(&commands::write::rebase(repo, onto, update_refs).await)
        }
        Command::CherryPick { revs } => {
            output::render_op(&commands::write::cherry_pick(repo, revs).await)
        }
        Command::Revert { revs } => output::render_op(&commands::write::revert(repo, revs).await),
        Command::Reset { rev, mode } => {
            output::render(&commands::write::reset(repo, rev, mode).await)
        }
        Command::Checkout { rev } => output::render(&commands::write::checkout(repo, rev).await),
        Command::Op { action } => output::render_op(&commands::write::op(repo, action).await),
        Command::BranchCreate { name, at, checkout } => {
            output::render(&commands::write::branch_create(repo, name, at, checkout).await)
        }
        Command::BranchDelete { name, force } => {
            output::render(&commands::write::branch_delete(repo, name, force).await)
        }
        Command::BranchRename { from, to } => {
            output::render(&commands::write::branch_rename(repo, from, to).await)
        }
        Command::TagCreate { name, at, message } => {
            output::render(&commands::write::tag_create(repo, name, at, message).await)
        }
        Command::TagDelete { name } => {
            output::render(&commands::write::tag_delete(repo, name).await)
        }
        Command::Stash {
            action,
            index,
            message,
            include_untracked,
        } => output::render(
            &commands::write::stash(repo, action, index, message, include_untracked).await,
        ),
        Command::Conflicts => output::render(&commands::conflicts::list(repo).await),
        Command::ConflictShow { file } => {
            output::render(&commands::conflicts::show(repo, &file).await)
        }
        Command::ConflictResolve { file, how } => {
            output::render(&commands::conflicts::resolve(repo, &file, how).await)
        }
        // Everything that touches the working tree, a linked one, or a file on disk.
        other => dispatch_tree(other, repo).await,
    }
}

/// The second half of the write dispatcher.
///
/// Split for the line count, and along the seam the commands already have: what moves refs
/// is above, what touches the working tree or a file on disk is here.
async fn dispatch_tree(command: Command, repo: &std::path::Path) -> output::Rendered {
    match command {
        Command::WorktreeAdd { path, rev, branch } => {
            output::render(&commands::worktree::add(repo, &path, &rev, branch).await)
        }
        Command::WorktreeRemove { path, force } => {
            output::render(&commands::worktree::remove(repo, &path, force).await)
        }
        Command::Patch { rev, from, out } => {
            output::render(&commands::patch::write(repo, &rev, from.as_deref(), &out).await)
        }
        Command::ApplyPatch { files, no_commit } => {
            output::render_op(&commands::patch::apply(repo, &files, !no_commit).await)
        }
        Command::Rewrite { rev, kind, message } => {
            output::render_op(&commands::rewrite::run(repo, &rev, kind, message).await)
        }
        Command::Undo => output::render(&commands::write::undo(repo).await),
        Command::Redo => output::render(&commands::write::redo(repo).await),
        Command::Journal => output::render(&commands::write::journal(repo).await),
        Command::Stage {
            paths,
            hunk,
            file,
            lines,
        } => output::render(&stage_or_unstage(repo, paths, hunk, file, lines, D::Stage).await),
        Command::Unstage {
            paths,
            hunk,
            file,
            lines,
        } => output::render(&stage_or_unstage(repo, paths, hunk, file, lines, D::Unstage).await),
        Command::Discard { paths } => output::render(&commands::stage::discard(repo, &paths).await),
        Command::Remotes
        | Command::Remote { .. }
        | Command::Fetch { .. }
        | Command::Push { .. }
        | Command::Pull { .. } => dispatch_remote(command, repo).await,
        // Every read variant is handled by `dispatch` before reaching here.
        _ => unreachable!("read command routed to the write dispatcher"),
    }
}

/// Routes a stage or unstage to the whole-path or partial form.
async fn stage_or_unstage(
    repo: &std::path::Path,
    paths: Vec<String>,
    hunk: Option<usize>,
    file: Option<String>,
    lines: Option<Vec<usize>>,
    direction: D,
) -> Result<commands::stage::Staged, coral_core::CoralError> {
    match (hunk, file) {
        (Some(h), Some(f)) => commands::stage::partial(repo, &f, h, lines, direction).await,
        _ => commands::stage::whole(repo, &paths, direction).await,
    }
}
