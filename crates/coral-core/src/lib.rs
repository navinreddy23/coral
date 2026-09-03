pub mod blame;
pub mod bytes;
pub mod commit;
pub mod diff;
pub mod engine;
pub mod error;
pub mod graph;
pub mod history;
pub mod process;
pub mod refs;
pub mod repo;
pub mod status;
pub mod watch;

#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

pub use commit::{Commit, Signature};
pub use error::CoralError;
pub use refs::{GitRef, RefKind};
pub use repo::{Head, OpState, RepoInfo, RepoLocation};
pub use status::{Change, ConflictKind, Status, StatusEntry};
