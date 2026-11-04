//! The hosts a repository can live on: GitHub, GitLab, Gitea and Forgejo, Bitbucket Cloud and
//! Azure DevOps. Nothing here is ever on the graph's critical path — every failure must degrade
//! to plain git rather than block a paint.

pub mod avatar;
pub mod azure;
pub mod bitbucket;
pub mod checks;
pub mod client;
pub mod declared;
pub mod gitea;
pub mod github;
pub mod gitlab;
pub mod provider;
pub mod pull_request;
pub mod review;
pub mod token;

pub use checks::{CheckState, Checks};
pub use client::Client;
pub use provider::{Host, HostKind, HostingError};
pub use pull_request::{NewPullRequest, PrState, PullRequest};
