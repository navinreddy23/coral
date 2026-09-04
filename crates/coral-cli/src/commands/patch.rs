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
pub async fn write(repo: &Path, rev: &str, directory: &Path) -> Result<Written, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, repo).await?;
    let files = loc.format_patch(&runner, rev, directory).await?;
    Ok(Written {
        files: files.iter().map(|p| p.display().to_string()).collect(),
    })
}
