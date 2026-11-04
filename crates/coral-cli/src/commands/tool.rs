use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::tool::{Compared, ExternalTools};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    #[serde(flatten)]
    pub tools: ExternalTools,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub tool: String,
    pub file: String,
}

async fn open(path: &Path) -> Result<(GitRunner, RepoLocation), CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok((runner, loc))
}

/// The diff and merge tools git has configured here.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure.
pub async fn show(path: &Path) -> Result<Tools, CoralError> {
    let (runner, loc) = open(path).await?;
    Ok(Tools {
        tools: loc.external_tools(&runner).await?,
    })
}

/// Starts the configured diff tool on one file and returns once it is running.
///
/// # Errors
/// A repository with no diff tool configured, or a git that cannot be started.
pub async fn difftool(
    path: &Path,
    file: &str,
    staged: bool,
    rev: Option<String>,
) -> Result<Started, CoralError> {
    let (runner, loc) = open(path).await?;
    let compared = match (rev, staged) {
        (Some(rev), _) => Compared::Commit(rev),
        (None, true) => Compared::Staged,
        (None, false) => Compared::Unstaged,
    };
    loc.open_difftool(&runner, file, &compared).await?;
    Ok(Started {
        tool: "diff".to_owned(),
        file: file.to_owned(),
    })
}

/// Starts the configured merge tool on one conflicted file.
///
/// # Errors
/// A repository with no merge tool configured, or a git that cannot be started.
pub async fn mergetool(path: &Path, file: &str) -> Result<Started, CoralError> {
    let (runner, loc) = open(path).await?;
    loc.open_mergetool(&runner, file).await?;
    Ok(Started {
        tool: "merge".to_owned(),
        file: file.to_owned(),
    })
}

impl crate::output::Human for Tools {
    fn human(&self) -> String {
        let name = |t: &Option<String>| t.clone().unwrap_or_else(|| "none set".to_owned());
        format!(
            "diff tool: {}\nmerge tool: {}",
            name(&self.tools.diff),
            name(&self.tools.merge)
        )
    }
}

impl crate::output::Human for Started {
    fn human(&self) -> String {
        format!("started the {} tool on {}", self.tool, self.file)
    }
}
