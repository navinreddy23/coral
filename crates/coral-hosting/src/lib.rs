//! GitHub and GitLab integration.
//!
//! Only the shapes the rest of the workspace depends on exist today; the providers land in
//! M8. Nothing here is ever on the graph's critical path — every failure must degrade to
//! plain git rather than block a paint.

pub mod provider;

pub use provider::{Host, HostKind, HostingError};
