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
        /// Check out only the files at the top; pick folders afterwards with `coral sparse`.
        #[arg(long)]
        sparse: bool,
        /// Do not draw progress on stderr.
        #[arg(long)]
        quiet: bool,
    },
    /// Report the working tree state.
    Status,
    /// Record a commit from the staged changes.
    Commit {
        /// Required, except with --fixup, whose message git writes.
        #[arg(short, long, default_value = "", required_unless_present_any = ["fixup", "squash"])]
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
        /// Record it as a fixup of this commit, to be folded in by the next autosquash.
        #[arg(long, value_name = "REV", conflicts_with_all = ["amend", "squash"])]
        fixup: Option<String>,
        /// Record it as a squash into this commit; --message becomes the extra text.
        #[arg(long, value_name = "REV", conflicts_with = "amend")]
        squash: Option<String>,
        /// Someone else who wrote it, as "Name <email>". Give it once for each.
        #[arg(long = "co-author", value_name = "WHO")]
        co_authors: Vec<String>,
    },
    /// List who wrote the repository's recent commits, most active first.
    Authors,
    /// List the local branches a revision already has every commit of.
    Merged {
        /// The branch they are merged into.
        #[arg(default_value = "HEAD")]
        into: String,
    },
    /// Stop a branch following an upstream.
    UnsetUpstream { branch: String },
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
        /// Replay only the commits after this one, leaving it and what is below it behind
        /// (`git rebase --onto ONTO FROM`).
        #[arg(long, value_name = "REV")]
        from: Option<String>,
        /// Move branches that point into the replayed range.
        #[arg(long)]
        update_refs: bool,
    },
    /// Apply commits onto the current branch.
    CherryPick {
        #[arg(required = true)]
        revs: Vec<String>,
        /// Which parent of a merge to measure the change against, counting from one. A merge
        /// needs one; git refuses a number the commit has no parent for.
        #[arg(long)]
        mainline: Option<u32>,
    },
    /// Record commits that undo others.
    Revert {
        #[arg(required = true)]
        revs: Vec<String>,
        /// Which parent of a merge to keep, counting from one. A merge needs one; git refuses
        /// a number the commit has no parent for.
        #[arg(long)]
        mainline: Option<u32>,
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
        /// The branch to make, for `branch`.
        #[arg(long, required_if_eq("action", "branch"))]
        name: Option<String>,
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
        #[arg(long, conflicts_with_all = ["deepen", "unshallow"])]
        prune: bool,
        /// In a shallow clone, fetch this many more commits behind where history stops.
        #[arg(long, value_name = "COMMITS", conflicts_with = "unshallow")]
        deepen: Option<u32>,
        /// In a shallow clone, fetch the rest of the history.
        #[arg(long)]
        unshallow: bool,
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
        /// Stage --hunk as this file's text reads instead of as the worktree has it: the
        /// hunk's new side, context and added lines, rewritten. `-` reads standard input.
        #[arg(long, value_name = "PATH", requires = "hunk", conflicts_with = "lines")]
        edited: Option<std::path::PathBuf>,
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
    /// Write files into the working tree as a commit or a stash has them, staging nothing.
    Take {
        /// The commit to take them from.
        #[arg(required_unless_present = "stash", conflicts_with = "stash")]
        rev: Option<String>,
        /// A stash entry to take them from instead.
        #[arg(long)]
        stash: Option<usize>,
        #[arg(long = "path", value_name = "PATH", required = true)]
        paths: Vec<String>,
    },
    Discard {
        #[arg(value_name = "PATH", required_unless_present = "file")]
        paths: Vec<String>,
        /// Throw away one hunk of a file instead. Use with --file.
        #[arg(long, requires = "file")]
        hunk: Option<usize>,
        /// The file a --hunk or --lines selection applies to.
        #[arg(long)]
        file: Option<String>,
        /// Throw away only these line indices within --hunk.
        #[arg(long, requires = "hunk", value_delimiter = ',')]
        lines: Option<Vec<usize>>,
    },
    /// Show changes to the worktree, or to the index with --staged.
    Diff {
        /// Diff the index against HEAD rather than the worktree against the index.
        #[arg(long, conflicts_with = "since")]
        staged: bool,
        /// Diff the worktree against this commit: everything since it, staged or not.
        #[arg(long, value_name = "REV")]
        since: Option<String>,
        /// Limit to these paths.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },
    /// List the files that differ between two revisions.
    Compare {
        from: String,
        to: String,
        /// Only what `to` gained since the two parted, rather than everything between the tips.
        #[arg(long)]
        merge_base: bool,
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
        /// Only commits that changed these lines of --path, as `FROM,TO` at --rev. Follows the
        /// lines across renames, so --follow is not needed.
        #[arg(long, value_name = "FROM,TO", requires = "path", conflicts_with_all = ["follow", "author", "grep", "search"])]
        lines: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u64,
    },
    /// Find commits the way the window's find box does: message, author, path and object id,
    /// narrowed by `code:text` (added or removed it), `code:/regex/`, `after:` and `before:`.
    Find {
        query: String,
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
    /// Show the diff and merge tools git has configured here.
    Tools,
    /// Open one file's change in git's configured diff tool. Returns once it has started.
    Difftool {
        file: String,
        /// The staged change rather than the unstaged one.
        #[arg(long, conflicts_with = "rev")]
        staged: bool,
        /// What this commit did to the file.
        #[arg(long)]
        rev: Option<String>,
    },
    /// Open a conflicted file in git's configured merge tool.
    Mergetool { file: String },
    /// Show how often the window fetches this repository on its own, or change it.
    AutoFetch {
        /// Fetch every this many minutes, between 1 and 1440.
        #[arg(long, value_name = "MINUTES", conflicts_with_all = ["off", "default"])]
        every: Option<u16>,
        /// Never fetch this repository automatically.
        #[arg(long, conflicts_with = "default")]
        off: bool,
        /// Use the window's own interval again.
        #[arg(long)]
        default: bool,
    },
    /// Show whether git reuses recorded conflict resolutions here, or turn it on or off.
    Rerere {
        #[arg(long, conflicts_with = "off")]
        on: bool,
        #[arg(long)]
        off: bool,
    },
    /// Show which folders a sparse working tree holds, or change them.
    Sparse {
        /// Check out only these folders, and the files at the top. None leaves just those.
        #[arg(long, num_args = 0.., value_name = "FOLDER", conflicts_with = "off")]
        set: Option<Vec<String>>,
        /// Check out everything again.
        #[arg(long)]
        off: bool,
    },
    /// List the folders directly inside one, as the sparse picker does.
    Folders {
        under: Option<String>,
        #[arg(long, default_value = "HEAD")]
        rev: String,
    },
    /// Show the caches git keeps to answer a status quickly, or switch them here.
    StatusCache {
        /// The untracked cache: `on` or `off`.
        #[arg(long, value_parser = ["on", "off"])]
        untracked: Option<String>,
        /// git's own filesystem monitor: `on` or `off`. Refused by a git built without one.
        #[arg(long, value_parser = ["on", "off"])]
        fsmonitor: Option<String>,
    },
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
    /// Store an API token for the repository's host, read from stdin. Every profile in the
    /// app can use it, until one signs in with a token of its own.
    HostLogin,
    /// Forget the stored API token for the repository's host. A profile that signed in for
    /// itself in the app keeps its own.
    HostLogout,
    /// List the repository's pull or merge requests.
    PullRequests,
    /// Draft a commit message for what is staged, with Claude, on the API key the app keeps.
    /// Prints the draft; nothing is committed.
    DraftMessage {
        /// The model to ask, instead of the default.
        #[arg(long)]
        model: Option<String>,
    },
    /// Show one issue on the repository's host (a work item on Azure DevOps), and the branch
    /// it would be worked on in.
    Issue { number: u64 },
    /// Say what kind of host a self-hosted server is, for one whose name does not: a Gitea or
    /// Forgejo instance can be called anything. `none` forgets it.
    HostKind {
        /// The server's name, as it appears in remote URLs: `git.example.com`.
        hostname: String,
        #[arg(value_parser = ["gitea", "github", "gitlab", "bitbucket", "azure", "none"])]
        kind: String,
    },
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
            sparse,
            quiet,
        } => {
            let what = coral_core::create::Cloned {
                url,
                parent: into.map_or_else(|| repo.to_path_buf(), |dir| repo.join(dir)),
                name: name.filter(|n| !n.trim().is_empty()),
                ssh_key: ssh_key.filter(|k| !k.trim().is_empty()),
                depth,
                blobless,
                sparse,
            };
            output::render(&commands::create::clone(&what, quiet).await)
        }
        Command::Status => output::render(&commands::status::run(repo).await),
        command @ (Command::Diff { .. } | Command::Compare { .. }) => {
            dispatch_diff(command, repo).await
        }
        command @ Command::Log { .. } => dispatch_log(command, repo).await,
        Command::Find { query, limit } => {
            output::render(&commands::find::run(repo, &query, limit).await)
        }
        Command::Blame { rev, file } => {
            output::render(&commands::blame::run(repo, &rev, &file).await)
        }
        Command::Refs { kind } => output::render(&commands::refs::run(repo, kind).await),
        Command::Submodules => output::render(&commands::submodule::run(repo).await),
        Command::Worktrees => output::render(&commands::worktree::list(repo).await),
        Command::Authors => output::render(&commands::log::authors(repo).await),
        Command::Merged { into } => output::render(&commands::refs::merged(repo, &into).await),
        command @ (Command::Rerere { .. }
        | Command::AutoFetch { .. }
        | Command::Tools
        | Command::Difftool { .. }
        | Command::Mergetool { .. }
        | Command::Sparse { .. }
        | Command::Folders { .. }
        | Command::StatusCache { .. }) => dispatch_config(command, repo).await,
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
        Command::DraftMessage { model } => {
            output::render(&commands::write::draft_message(repo, model).await)
        }
        Command::Issue { number } => output::render(&commands::hosting::issue(repo, number).await),
        Command::HostKind { hostname, kind } => {
            output::render(&commands::hosting::declare(&hostname, &kind))
        }
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
/// The repository's own git switches: rerere, the status caches, the sparse checkout.
async fn dispatch_config(command: Command, repo: &std::path::Path) -> output::Rendered {
    match command {
        Command::Tools => output::render(&commands::tool::show(repo).await),
        Command::Difftool { file, staged, rev } => {
            output::render(&commands::tool::difftool(repo, &file, staged, rev).await)
        }
        Command::Mergetool { file } => {
            output::render(&commands::tool::mergetool(repo, &file).await)
        }
        Command::AutoFetch {
            every,
            off,
            default,
        } => {
            use commands::remote::AutoFetchChange;
            use coral_core::autofetch::AutoFetch;
            let change = match (every, off, default) {
                (Some(minutes), _, _) => AutoFetchChange::Set(Some(AutoFetch::Every { minutes })),
                (None, true, _) => AutoFetchChange::Set(Some(AutoFetch::Off)),
                (None, false, true) => AutoFetchChange::Set(None),
                (None, false, false) => AutoFetchChange::Show,
            };
            output::render(&commands::remote::auto_fetch(repo, change).await)
        }
        Command::Rerere { on, off } => {
            let set = if on {
                Some(true)
            } else if off {
                Some(false)
            } else {
                None
            };
            output::render(&commands::conflicts::rerere(repo, set).await)
        }
        Command::Sparse { set, off } => {
            output::render(&commands::status::sparse(repo, set, off).await)
        }
        Command::Folders { under, rev } => {
            output::render(&commands::status::folders(repo, &rev, under.as_deref()).await)
        }
        Command::StatusCache {
            untracked,
            fsmonitor,
        } => output::render(
            &commands::status::caches(
                repo,
                untracked.map(|v| v == "on"),
                fsmonitor.map(|v| v == "on"),
            )
            .await,
        ),
        _ => unreachable!("routed by `dispatch`"),
    }
}

/// What differs: the working tree, the index, or two revisions.
async fn dispatch_diff(command: Command, repo: &std::path::Path) -> output::Rendered {
    match command {
        Command::Diff {
            staged,
            since,
            paths,
        } => output::render(&commands::diff::run(repo, staged, since.as_deref(), &paths).await),
        Command::Compare {
            from,
            to,
            merge_base,
        } => output::render(&commands::diff::compare(repo, &from, &to, merge_base).await),
        _ => unreachable!("routed by `dispatch`"),
    }
}

async fn dispatch_log(command: Command, repo: &std::path::Path) -> output::Rendered {
    let Command::Log {
        rev,
        path,
        follow,
        author,
        grep,
        search,
        lines,
        limit,
    } = command
    else {
        unreachable!("routed by `dispatch`")
    };
    if let Some(lines) = lines {
        let file = path.unwrap_or_default();
        return output::render(&commands::log::lines(repo, &rev, &file, &lines, limit).await);
    }
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
        Command::Fetch {
            remote,
            prune,
            deepen,
            unshallow,
        } => {
            use coral_core::remote::Deepen;
            let how = match (deepen, unshallow) {
                (Some(commits), _) => Some(Deepen::By { commits }),
                (None, true) => Some(Deepen::All),
                (None, false) => None,
            };
            match how {
                Some(how) => output::render(&commands::remote::deepen(repo, remote, how).await),
                None => output::render(&commands::remote::fetch(repo, remote, prune).await),
            }
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
            fixup,
            squash,
            co_authors,
        } => {
            let fold = match (fixup, squash) {
                (Some(target), _) => Some(coral_core::ops::FoldInto {
                    how: coral_core::ops::Fold::Fixup,
                    target,
                }),
                (None, Some(target)) => Some(coral_core::ops::FoldInto {
                    how: coral_core::ops::Fold::Squash,
                    target,
                }),
                (None, None) => None,
            };
            let opts = coral_core::ops::CommitOpts {
                message,
                amend,
                signoff,
                author,
                allow_empty,
                fold,
                co_authors,
            };
            output::render(&commands::write::commit(repo, opts).await)
        }
        Command::Merge { rev, mode, message } => {
            output::render_op(&commands::write::merge(repo, rev, mode, message).await)
        }
        Command::Rebase {
            onto,
            from,
            update_refs,
        } => output::render_op(&commands::write::rebase(repo, onto, from, update_refs).await),
        Command::CherryPick { revs, mainline } => {
            output::render_op(&commands::write::cherry_pick(repo, revs, mainline).await)
        }
        Command::Revert { revs, mainline } => {
            output::render_op(&commands::write::revert(repo, revs, mainline).await)
        }
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
        Command::UnsetUpstream { branch } => {
            output::render(&commands::write::unset_upstream(repo, &branch).await)
        }
        Command::TagDelete { name } => {
            output::render(&commands::write::tag_delete(repo, name).await)
        }
        Command::Stash {
            action,
            index,
            message,
            include_untracked,
            name,
        } => output::render(
            &commands::write::stash(repo, action, index, message, include_untracked, name).await,
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
            hunk: Some(hunk),
            file: Some(file),
            edited: Some(edited),
            ..
        } => output::render(&commands::stage::edited(repo, &file, hunk, &edited).await),
        Command::Stage {
            paths,
            hunk,
            file,
            lines,
            ..
        } => output::render(&stage_or_unstage(repo, paths, hunk, file, lines, D::Stage).await),
        Command::Unstage {
            paths,
            hunk,
            file,
            lines,
        } => output::render(&stage_or_unstage(repo, paths, hunk, file, lines, D::Unstage).await),
        Command::Take { rev, stash, paths } => {
            output::render(&commands::stage::take(repo, rev.as_deref(), stash, &paths).await)
        }
        Command::Discard {
            paths,
            hunk,
            file,
            lines,
        } => output::render(&discard(repo, paths, hunk, file, lines).await),
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
/// Throws away whole paths, or one hunk of a file.
///
/// A whole path is restored from the index rather than by reversing a patch, which is what
/// `discard` has always meant here. A selection has no such shortcut and goes through the same
/// patch machinery staging does, in reverse.
async fn discard(
    repo: &std::path::Path,
    paths: Vec<String>,
    hunk: Option<usize>,
    file: Option<String>,
    lines: Option<Vec<usize>>,
) -> Result<commands::stage::Staged, coral_core::CoralError> {
    match (hunk, file) {
        (Some(h), Some(f)) => commands::stage::partial(repo, &f, h, lines, D::Discard).await,
        _ => commands::stage::discard(repo, &paths).await,
    }
}

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
