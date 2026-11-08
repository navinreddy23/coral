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
        // Whole paths are discarded by `coral discard`, which restores them outright rather
        // than reversing a patch.
        Direction::Discard => loc.discard(&runner, &refs).await?,
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

/// Stages one hunk as `edited` rewrites its new side, `-` being standard input.
///
/// # Errors
/// [`CoralError::Io`] when the text cannot be read, and whatever
/// [`coral_core::index::build_edited_patch`] or git refuses.
pub async fn edited(
    path: &Path,
    file: &str,
    hunk: usize,
    edited: &Path,
) -> Result<Staged, CoralError> {
    let text = if edited.as_os_str() == "-" {
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read(edited)?
    };
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let files = loc
        .diff(&runner, false, &[file], DiffOptions::default())
        .await?;
    let target = files
        .iter()
        .find(|f| f.path == file)
        .ok_or_else(|| CoralError::Protocol {
            label: "apply",
            detail: format!("{file} has no unstaged changes"),
        })?;
    let generated = coral_core::index::build_edited_patch(target, hunk, &text)?;
    loc.apply_to_index(&runner, &generated, Direction::Stage)
        .await?;
    Ok(Staged {
        paths: vec![file.to_owned()],
        direction: label(Direction::Stage),
    })
}

/// Writes `paths` into the working tree as a commit or a stash entry has them.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure, including an unknown revision.
pub async fn take(
    path: &Path,
    rev: Option<&str>,
    stash: Option<usize>,
    paths: &[String],
) -> Result<Staged, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    for file in paths {
        let source = match (stash, rev) {
            (Some(index), _) => loc.stash_source(&runner, index, file).await?,
            (None, Some(rev)) => rev.to_owned(),
            (None, None) => {
                return Err(CoralError::Refused {
                    label: "take files",
                    detail: "name a commit or a stash to take them from".to_owned(),
                });
            }
        };
        loc.restore_from(&runner, &source, &[file.as_str()]).await?;
    }
    Ok(Staged {
        paths: paths.to_vec(),
        direction: "taken",
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
        Direction::Discard => "discarded",
    }
}
