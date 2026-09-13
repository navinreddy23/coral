//! Merge conflicts: what is in progress, what is unresolved, and what the choices are.

pub mod blocks;
pub mod resolve;
pub mod state;

pub use blocks::{Block, Blocks, Stages, Take};
pub use resolve::{ConflictedFile, Resolution, Whole};
pub use state::{Operation, Progress, SideLabels};
