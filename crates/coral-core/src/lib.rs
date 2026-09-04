pub mod blame;
pub mod bytes;
pub mod commit;
pub mod config;
pub mod conflict;
pub mod credential;
pub mod diff;
pub mod engine;
pub mod error;
pub mod graph;
pub mod history;
pub mod index;
pub mod ops;
pub mod patch;
pub mod process;
pub mod refs;
pub mod remote;
pub mod repo;
pub mod sequence;
pub mod signing;
pub mod ssh;
pub mod status;
pub mod submodule;
pub mod undo;
pub mod watch;
pub mod worktree;

#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

pub use commit::{Commit, Signature};
pub use error::CoralError;
pub use refs::{GitRef, RefKind};
pub use repo::{Head, OpState, RepoInfo, RepoLocation};
pub use status::{Change, ConflictKind, Status, StatusEntry};
