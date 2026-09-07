use std::path::PathBuf;

use crate::process::GitVersion;

/// Everything `coral-core` can fail with. Both the CLI JSON envelope and the Tauri IPC layer
/// serialize these, so `code()` is a stable part of the public contract.
#[derive(Debug, thiserror::Error)]
pub enum CoralError {
    #[error("`git` was not found on PATH")]
    GitMissing,

    #[error("git {found} is too old; coral needs {required} or newer")]
    GitTooOld {
        found: GitVersion,
        required: GitVersion,
    },

    #[error("could not parse `git --version` output: {raw}")]
    GitVersionUnparsable { raw: String },

    #[error("git {label} exited with {code}: {stderr}")]
    GitExit {
        label: &'static str,
        code: i32,
        argv: Vec<String>,
        stderr: String,
    },

    #[error("git {label} was killed by signal {signal}")]
    GitSignal {
        label: &'static str,
        signal: i32,
        argv: Vec<String>,
    },

    #[error("failed to spawn git {label}")]
    GitSpawn {
        label: &'static str,
        argv: Vec<String>,
        #[source]
        source: std::io::Error,
    },

    #[error("not a git repository: {0}")]
    NotARepository(PathBuf),

    /// Asked to make a repository where one already is. Refused rather than reported as git's
    /// "reinitialized existing repository", which is a success message for something the user
    /// did not ask for.
    #[error("there is already a git repository at {0}")]
    AlreadyARepository(PathBuf),

    #[error("malformed git {label} output: {detail}")]
    Protocol { label: &'static str, detail: String },

    /// The request is well-formed but cannot be carried out safely right now — undoing over a
    /// dirty worktree, continuing when nothing is in progress. Distinct from a protocol error,
    /// which means we failed to understand git.
    #[error("cannot {label}: {detail}")]
    Refused { label: &'static str, detail: String },

    /// The user stopped it. Not a failure of the operation, and never reported as one: a
    /// cancelled fetch has not gone wrong, it has been called off.
    #[error("{label} was cancelled")]
    Cancelled { label: &'static str },

    #[error("io error")]
    Io(#[from] std::io::Error),
}

impl CoralError {
    /// Stable machine-readable tag. Never change an existing string without bumping the
    /// envelope schema; the UI and the CLI's exit-code map both switch on it.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::GitMissing => "git_missing",
            Self::GitTooOld { .. } => "git_too_old",
            Self::GitVersionUnparsable { .. } => "git_version_unparsable",
            Self::GitExit { .. } | Self::GitSignal { .. } => "git_error",
            Self::GitSpawn { .. } => "git_spawn_failed",
            Self::NotARepository(_) => "not_a_repository",
            Self::AlreadyARepository(_) => "already_a_repository",
            Self::Protocol { .. } => "protocol_error",
            Self::Refused { .. } => "refused",
            Self::Cancelled { .. } => "cancelled",
            Self::Io(_) => "io_error",
        }
    }

    /// The redacted argv, when the failure came from a git child. Safe to log and serialize.
    #[must_use]
    pub fn argv(&self) -> Option<&[String]> {
        match self {
            Self::GitExit { argv, .. }
            | Self::GitSignal { argv, .. }
            | Self::GitSpawn { argv, .. } => Some(argv),
            _ => None,
        }
    }

    /// git's own stderr, when there is any.
    #[must_use]
    pub fn stderr(&self) -> Option<&str> {
        match self {
            Self::GitExit { stderr, .. } => Some(stderr),
            _ => None,
        }
    }
}
