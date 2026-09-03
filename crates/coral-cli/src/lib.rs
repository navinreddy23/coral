pub mod commands;
pub mod output;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    /// List refs.
    Refs {
        #[arg(long, value_enum, default_value_t = commands::refs::Kind::All)]
        kind: commands::refs::Kind,
    },
    /// Walk the commit graph and report rows with their lanes.
    Graph {
        /// Stop after this many commits.
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
        Command::Refs { kind } => output::render(&commands::refs::run(&repo, kind).await),
        Command::Graph {
            limit,
            from,
            first_paint,
        } => output::render(&commands::graph::run(&repo, limit, from, first_paint).await),
        Command::Version => output::render(&commands::version::run().await),
    }
}
