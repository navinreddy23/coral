//! The Tauri command layer, exposed as a library so the commands can be tested without
//! starting a window.

pub mod actions;
pub mod activity;
pub mod commands;
pub mod conflicts;
pub mod credentials;
pub mod graph;
pub mod hosting;
pub mod remotes;
pub mod session;
pub mod signing;
pub mod ssh;
pub mod tabs;
pub mod terminal;
pub mod watcher;

pub use commands::{commit_staged, discard_paths, open_repo, repo_status, stage_paths};
pub use graph::{
    GraphCache, binary_self_test, commit_detail, graph_frame, graph_row_of, repo_refs, row_metadata,
};
