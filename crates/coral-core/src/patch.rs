//! Patch files, as `git format-patch` writes them.
//!
//! A commit exported as a mailbox-format file, which is what gets attached to a message or fed
//! to `git am` on another machine.

use std::path::{Path, PathBuf};

use bstr::ByteSlice;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

impl RepoLocation {
    /// Writes one commit as a patch file into `directory`, returning what was written.
    ///
    /// `--no-numbered` and `--zero-commit` are deliberately not passed: the file name git
    /// chooses carries the commit's own summary, which is what makes a directory of patches
    /// readable, and the numbering is what keeps a series in order.
    ///
    /// # Errors
    /// Propagates git failures, including a directory that cannot be written and a revision
    /// with no parent to diff against.
    pub async fn format_patch(
        &self,
        runner: &GitRunner,
        rev: &str,
        directory: &Path,
    ) -> Result<Vec<PathBuf>, CoralError> {
        let out = runner
            .output(
                GitCommand::write("format-patch", self.display_path())
                    .args(["format-patch", "-1", "--output-directory"])
                    .arg(directory)
                    .arg(rev),
            )
            .await?;

        // One path per line on stdout, which is the only reliable way to learn the name git
        // built from the summary.
        Ok(out
            .stdout
            .split(|b| *b == b'\n')
            .map(<[u8]>::trim)
            .filter(|l| !l.is_empty())
            .map(|l| PathBuf::from(l.to_str_lossy().into_owned()))
            .collect())
    }
}
