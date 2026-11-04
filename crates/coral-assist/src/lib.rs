//! Drafting a commit message with Claude, from the staged change, on the user's own key.
//!
//! Off unless the user turns it on and gives a key. Nothing is sent but what is asked for:
//! the staged diff, the file list, and the subjects of the last few commits so the draft
//! follows the repository's style. Nothing here is on any path the window waits on.

pub mod draft;
pub mod key;

pub use draft::{Draft, DraftError, Request, draft};
