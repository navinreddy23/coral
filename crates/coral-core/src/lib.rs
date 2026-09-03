pub mod bytes;
pub mod diff;
pub mod error;
pub mod graph;
pub mod process;
pub mod refs;
pub mod repo;
pub mod status;

#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

pub use error::CoralError;
pub use refs::{GitRef, RefKind};
pub use repo::{Head, OpState, RepoInfo, RepoLocation};
pub use status::{Change, ConflictKind, Status, StatusEntry};
