//! What `.gitattributes` says about a path.
//!
//! One question so far, and it is the one that matters to anything reading a file's content:
//! whether Git LFS stands between the repository and the worktree.

use std::collections::HashSet;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// The filter driver `git lfs track` writes, and the name git-lfs installs itself under.
const LFS: &[u8] = b"lfs";

impl RepoLocation {
    /// Which of these paths Git LFS keeps outside the repository.
    ///
    /// What is stored for such a path is a pointer of three lines naming an object; the asset
    /// itself lives elsewhere. Anything that treats those three lines as the file's content —
    /// a conflict settled region by region, a diff staged a line at a time — builds a pointer
    /// that names nothing, and the next clone has no file there at all.
    ///
    /// Only this one driver, deliberately. A filter is not by itself a reason to stop showing
    /// a file's lines: `nbstripout` and `git-crypt` both leave text that merges and stages
    /// exactly as it reads. LFS is different because its three lines are one record.
    ///
    /// One process for the whole list. A merge can conflict in hundreds of files, and asking
    /// per path would spawn a git for each.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn in_lfs(
        &self,
        runner: &GitRunner,
        paths: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<HashSet<String>, CoralError> {
        let mut stdin = Vec::new();
        for path in paths {
            stdin.extend_from_slice(path.as_ref());
            stdin.push(0);
        }
        if stdin.is_empty() {
            return Ok(HashSet::new());
        }
        let out = runner
            .output(
                GitCommand::read("check-attr", self.display_path())
                    .args(["check-attr", "-z", "--stdin", "filter"])
                    .stdin_bytes(stdin),
            )
            .await?;
        Ok(tracked(&out.stdout))
    }
}

/// Reads `git check-attr -z`: `<path>\0<attribute>\0<value>\0` for each path asked about.
fn tracked(out: &[u8]) -> HashSet<String> {
    let mut fields = out.split(|b| *b == 0);
    let mut named = HashSet::new();
    while let (Some(path), Some(_attr), Some(value)) = (fields.next(), fields.next(), fields.next())
    {
        if value == LFS {
            named.insert(String::from_utf8_lossy(path).into_owned());
        }
    }
    named
}

#[cfg(test)]
mod tests {
    use super::tracked;

    #[test]
    fn names_the_paths_git_lfs_holds_and_leaves_the_rest() {
        let out = b"logo.png\0filter\0lfs\0notes.txt\0filter\0unspecified\0nb.ipynb\0filter\0nbstripout\0";
        let named = tracked(out);

        assert!(named.contains("logo.png"));
        // A filter is not by itself a reason to stop showing a file's lines.
        assert_eq!(named.len(), 1);
    }

    #[test]
    fn an_answer_cut_short_names_nothing_rather_than_guessing() {
        assert!(tracked(b"logo.png\0filter\0").is_empty());
    }
}
