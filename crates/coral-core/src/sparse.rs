//! Sparse checkout: a working tree holding only the folders asked for, in cone mode.
//!
//! Cone mode names directories, and each one brings everything beneath it along with the
//! files at the top of the repository. It is the only mode git still recommends, and the only
//! one whose matching stays fast on a tree the size of the kernel's.
//! <https://git-scm.com/docs/git-sparse-checkout>

use bstr::ByteSlice;

use crate::config::{AppConfig, Scope};
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// Whether the working tree is sparse, and what it holds.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Sparse {
    pub enabled: bool,
    /// Patterns rather than folders, which the picker cannot show or keep.
    pub cone: bool,
    /// The folders checked out, besides the top-level files every cone keeps.
    pub folders: Vec<String>,
}

impl RepoLocation {
    /// The sparse checkout as it stands.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn sparse(&self, runner: &GitRunner) -> Result<Sparse, CoralError> {
        let effective = Scope::Effective(AppConfig(None));
        let enabled = self
            .config_get(runner, effective, "core.sparseCheckout")
            .await?
            .as_deref()
            .is_some_and(crate::config::truthy);
        if !enabled {
            return Ok(Sparse {
                enabled: false,
                cone: true,
                folders: Vec::new(),
            });
        }
        let cone = self
            .config_get(runner, effective, "core.sparseCheckoutCone")
            .await?
            .as_deref()
            .is_none_or(crate::config::truthy);
        let out = runner
            .output(
                GitCommand::read("sparse-checkout", self.display_path())
                    .args(["sparse-checkout", "list"]),
            )
            .await?;
        let folders = if cone {
            out.stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_str_lossy().into_owned())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Sparse {
            enabled,
            cone,
            folders,
        })
    }

    /// Checks out these folders and the files at the top, and nothing else.
    ///
    /// No folders at all is a working tree of the top-level files alone, which is where
    /// `clone --sparse` starts. A file with local changes outside the new cone is left on disk
    /// by git rather than deleted.
    ///
    /// # Errors
    /// Propagates git failures, including a folder git will not take as a cone.
    pub async fn set_sparse(
        &self,
        runner: &GitRunner,
        folders: &[String],
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("sparse-checkout", self.display_path())
                    .args(["sparse-checkout", "set", "--cone", "--"])
                    .args(folders),
            )
            .await
            .map(|_| ())
    }

    /// Checks out everything again and stops being sparse.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn disable_sparse(&self, runner: &GitRunner) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("sparse-checkout", self.display_path())
                    .args(["sparse-checkout", "disable"]),
            )
            .await
            .map(|_| ())
    }

    /// The folders directly inside `under` at `rev`, one level at a time, for a picker.
    ///
    /// From the tree rather than the disk, since a sparse working tree has left most of them
    /// out, and one level only: listing the kernel's every directory to show its top twenty
    /// would be reading five thousand to show twenty.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn folders(
        &self,
        runner: &GitRunner,
        rev: &str,
        under: Option<&str>,
    ) -> Result<Vec<String>, CoralError> {
        let mut cmd = GitCommand::read("ls-tree", self.display_path())
            .args(["ls-tree", "-d", "--name-only", "-z", "--full-tree"])
            .arg(rev);
        if let Some(dir) = under
            .map(|d| d.trim_end_matches('/'))
            .filter(|d| !d.is_empty())
        {
            cmd = cmd.arg(format!("{dir}/"));
        }
        let out = runner.output(cmd).await?;
        Ok(out
            .stdout
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| p.to_str_lossy().into_owned())
            .collect())
    }
}
