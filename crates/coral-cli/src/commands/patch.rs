use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    pub files: Vec<String>,
}

impl crate::output::Human for Written {
    fn human(&self) -> String {
        if self.files.is_empty() {
            return "no patch written".to_owned();
        }
        let mut s = format!("{} patch files", self.files.len());
        for f in &self.files {
            let _ = write!(s, "\n  {f}");
        }
        s
    }
}

/// Writes one commit as a patch file.
///
/// # Errors
/// Propagates git failures, including an unwritable directory.
pub async fn write(
    repo: &Path,
    rev: &str,
    from: Option<&str>,
    directory: &Path,
) -> Result<Written, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, repo).await?;
    let files = match from {
        Some(from) => {
            loc.format_patch_range(&runner, from, rev, directory)
                .await?
        }
        None => loc.format_patch(&runner, rev, directory).await?,
    };
    Ok(Written {
        files: files.iter().map(|p| p.display().to_string()).collect(),
    })
}

/// Applies patch files to the current branch.
///
/// # Errors
/// Propagates git failures. A patch that conflicts is an outcome rather than a failure, and
/// comes back as a stop.
pub async fn apply(
    repo: &Path,
    files: &[std::path::PathBuf],
    commit: bool,
) -> Result<coral_core::ops::OpOutcome, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, repo).await?;
    let landing = if commit {
        coral_core::patch::PatchLanding::Commit
    } else {
        coral_core::patch::PatchLanding::WorkingTree
    };
    loc.apply_patches(&runner, files, landing).await
}
