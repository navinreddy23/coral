//! A file's bytes as one side of a change had them, for a view that draws rather than reads:
//! an image before and after.

use std::path::{Component, Path};

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// Larger than any image worth putting side by side in a panel, and small enough that holding
/// two of them in the window costs nothing.
pub const LIMIT: u64 = 32 * 1024 * 1024;

/// Where one side of a change lives.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum BlobSide {
    /// As a commit has it.
    Commit { rev: String },
    /// As the commit before `rev` had it: its first parent.
    Parent { rev: String },
    /// As the index has it.
    Index,
    /// As the working tree has it, on disk.
    Worktree,
}

impl RepoLocation {
    /// The bytes of `path` on one side of a change, or `None` where that side has no such file:
    /// before an addition, after a deletion, or before a root commit.
    ///
    /// # Errors
    /// [`CoralError::Refused`] for a file over [`LIMIT`] or a path that leaves the working tree,
    /// and git failures other than the file being absent.
    pub async fn blob(
        &self,
        runner: &GitRunner,
        side: &BlobSide,
        path: &str,
    ) -> Result<Option<Vec<u8>>, CoralError> {
        let spec = match side {
            BlobSide::Commit { rev } => format!("{rev}:{path}"),
            BlobSide::Parent { rev } => format!("{rev}^:{path}"),
            // Stage 0 named, or a path such as `1:x.png` would be read as a stage number.
            BlobSide::Index => format!(":0:{path}"),
            BlobSide::Worktree => return self.worktree_blob(path),
        };
        // `-e` answers whether it exists without a word on stderr, so an absent side is told
        // apart from a failure by its exit code rather than by reading git's prose.
        let exists = runner
            .output_allowing(
                GitCommand::read("cat-file", self.display_path())
                    .args(["cat-file", "-e"])
                    .arg(&spec),
                &[1, 128],
            )
            .await?;
        if exists.code != 0 {
            return Ok(None);
        }
        let measured = runner
            .output(
                GitCommand::read("cat-file", self.display_path())
                    .args(["cat-file", "-s"])
                    .arg(&spec),
            )
            .await?;
        let length: u64 = String::from_utf8_lossy(&measured.stdout)
            .trim()
            .parse()
            .unwrap_or(0);
        too_large(length)?;
        let out = runner
            .output(
                GitCommand::read("cat-file", self.display_path())
                    .args(["cat-file", "blob"])
                    .arg(&spec),
            )
            .await?;
        Ok(Some(out.stdout))
    }

    fn worktree_blob(&self, path: &str) -> Result<Option<Vec<u8>>, CoralError> {
        let Some(root) = &self.workdir else {
            return Ok(None);
        };
        if !stays_inside(Path::new(path)) {
            return Err(CoralError::Refused {
                label: "read the file",
                detail: format!("{path} is outside the working tree"),
            });
        }
        let file = root.join(path);
        match std::fs::metadata(&file) {
            Ok(meta) if meta.is_file() => too_large(meta.len())?,
            Ok(_) => return Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error(path, &e)),
        }
        std::fs::read(&file)
            .map(Some)
            .map_err(|e| io_error(path, &e))
    }
}

/// Whether a repository-relative path names something under the working tree, never above it.
fn stays_inside(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn too_large(size: u64) -> Result<(), CoralError> {
    if size > LIMIT {
        return Err(CoralError::Refused {
            label: "show the file",
            // Rounded up: rounded down, a file just past the limit was said to be the limit.
            detail: format!(
                "it is {} MB, and only files up to {} MB are drawn",
                size.div_ceil(1024 * 1024),
                LIMIT / (1024 * 1024)
            ),
        });
    }
    Ok(())
}

fn io_error(path: &str, e: &std::io::Error) -> CoralError {
    CoralError::Refused {
        label: "read the file",
        detail: format!("{path}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{LIMIT, stays_inside, too_large};
    use std::path::Path;

    #[test]
    fn draws_up_to_the_limit_and_refuses_past_it() {
        assert!(too_large(LIMIT).is_ok());
        let err = too_large(LIMIT + 1).unwrap_err();
        assert_eq!(err.code(), "refused");
        // Just past the limit is said to be past it, not to be it.
        assert!(
            err.to_string()
                .contains("it is 33 MB, and only files up to 32 MB"),
            "{err}"
        );
    }

    #[test]
    fn a_path_may_not_climb_out_of_the_working_tree() {
        assert!(stays_inside(Path::new("img/logo.png")));
        assert!(stays_inside(Path::new("./logo.png")));
        assert!(!stays_inside(Path::new("../secret.png")));
        assert!(!stays_inside(Path::new("img/../../x.png")));
        assert!(!stays_inside(Path::new("/etc/passwd")));
    }
}
