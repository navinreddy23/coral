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
    /// Report the working tree state.
    Status,
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

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let repo = cli.repo.unwrap_or(cwd);

    match cli.command {
        Command::Open => output::render(&commands::open::run(&repo).await),
        Command::Status => output::render(&commands::status::run(&repo).await),
        Command::Stage {
            paths,
            hunk,
            file,
            lines,
        } => output::render(&stage_or_unstage(&repo, paths, hunk, file, lines, D::Stage).await),
        Command::Unstage {
            paths,
            hunk,
            file,
            lines,
        } => output::render(&stage_or_unstage(&repo, paths, hunk, file, lines, D::Unstage).await),
        Command::Discard { paths } => {
            output::render(&commands::stage::discard(&repo, &paths).await)
        }
        Command::Diff { staged, paths } => {
            output::render(&commands::diff::run(&repo, staged, &paths).await)
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
            output::render(&commands::log::run(&repo, &q).await)
        }
        Command::Blame { rev, file } => {
            output::render(&commands::blame::run(&repo, &rev, &file).await)
        }
        Command::Refs { kind } => output::render(&commands::refs::run(&repo, kind).await),
        Command::Graph {
            limit,
            from,
            first_paint,
        } => output::render(&commands::graph::run(&repo, limit, from, first_paint).await),
        Command::Version => output::render(&commands::version::run().await),
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
