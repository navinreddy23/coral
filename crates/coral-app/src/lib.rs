//! The Tauri command layer, exposed as a library so the commands can be tested without
//! starting a window.

pub mod commands;
pub mod graph;

pub use commands::{commit_staged, open_repo, repo_status, stage_paths};
pub use graph::{
    GraphCache, binary_self_test, commit_detail, graph_frame, repo_refs, row_metadata,
};
