//! Notes: text attached to a commit after it was made, without changing the commit.
//!
//! Kept in `refs/notes/commits`, the ref `git log` shows by default (see `git help notes`).
//! A note can be added, changed or removed at any time, and the commit's id stays the same.

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

impl RepoLocation {
    /// The note on `rev`, or `None` when it has none.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn note(&self, runner: &GitRunner, rev: &str) -> Result<Option<String>, CoralError> {
        // `notes show` exits 1, saying "no note found", for a commit without one.
        let out = runner
            .output_allowing(
                GitCommand::read("notes", self.display_path()).args(["notes", "show", rev]),
                &[1],
            )
            .await?;
        if out.code != 0 {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&out.stdout).trim_end().to_owned();
        Ok((!text.is_empty()).then_some(text))
    }

    /// Sets the note on `rev`, replacing any there was. Empty text removes it.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn set_note(
        &self,
        runner: &GitRunner,
        rev: &str,
        text: &str,
    ) -> Result<(), CoralError> {
        let cmd = if text.trim().is_empty() {
            GitCommand::write("notes", self.display_path()).args([
                "notes",
                "remove",
                "--ignore-missing",
                rev,
            ])
        } else {
            // From stdin rather than `-m`, so a note that starts with a dash or spans lines
            // reaches git as written.
            GitCommand::write("notes", self.display_path())
                .args(["notes", "add", "--force", "--file=-", rev])
                .stdin_bytes(text.as_bytes().to_vec())
        };
        runner.output(cmd).await.map(|_| ())
    }
}
