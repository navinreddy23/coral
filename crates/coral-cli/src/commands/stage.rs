use std::path::Path;

use coral_core::CoralError;
use coral_core::diff::DiffOptions;
use coral_core::index::{Direction, Selection, build_patch};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Staged {
    /// Paths the operation touched.
    pub paths: Vec<String>,
    pub direction: &'static str,
}

impl crate::output::Human for Staged {
    fn human(&self) -> String {
        format!("{} {} paths", self.direction, self.paths.len())
    }
}

/// Stages or unstages whole paths.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure.
pub async fn whole(
    path: &Path,
    paths: &[String],
    direction: Direction,
) -> Result<Staged, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();

    match direction {
        Direction::Stage => loc.stage(&runner, &refs).await?,
        Direction::Unstage => loc.unstage(&runner, &refs).await?,
    }
    Ok(Staged {
        paths: paths.to_vec(),
        direction: label(direction),
    })
}

/// Stages or unstages one hunk, or selected lines within it.
///
/// # Errors
/// [`CoralError::Protocol`] if the file or hunk does not exist in the current diff.
pub async fn partial(
    path: &Path,
    file: &str,
    hunk: usize,
    lines: Option<Vec<usize>>,
    direction: Direction,
) -> Result<Staged, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;

    // Staging reads the worktree diff; unstaging reads what is already in the index.
    let staged_side = direction == Direction::Unstage;
    let files = loc
        .diff(&runner, staged_side, &[file], DiffOptions::default())
        .await?;
    let target = files
        .iter()
        .find(|f| f.path == file)
        .ok_or_else(|| CoralError::Protocol {
            label: "apply",
            detail: format!("{file} has no changes on that side"),
        })?;

    let selection = lines.map_or(Selection::WholeHunk, Selection::Lines);
    let generated = build_patch(target, &[(hunk, selection)], direction)?;
    loc.apply_to_index(&runner, &generated, direction).await?;

    Ok(Staged {
        paths: vec![file.to_owned()],
        direction: label(direction),
    })
}

/// Throws away worktree changes.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure.
pub async fn discard(path: &Path, paths: &[String]) -> Result<Staged, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    loc.discard(&runner, &refs).await?;
    Ok(Staged {
        paths: paths.to_vec(),
        direction: "discarded",
    })
}

const fn label(d: Direction) -> &'static str {
    match d {
        Direction::Stage => "staged",
        Direction::Unstage => "unstaged",
    }
}
