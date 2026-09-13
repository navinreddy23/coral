use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::conflict::{Blocks, ConflictedFile, Operation, Resolution, Whole};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictList {
    pub operation: Operation,
    pub files: Vec<ConflictedFile>,
}

impl crate::output::Human for ConflictList {
    fn human(&self) -> String {
        let l = &self.operation.labels;
        let mut s = format!("{:?}: {} vs {}", self.operation.state, l.ours, l.theirs);
        if l.swapped {
            s.push_str("\n  note: during a rebase \"ours\" is the branch you are rebasing onto");
        }
        if let Some(p) = self.operation.progress {
            let _ = write!(s, "\n  progress: {}/{}", p.current, p.total);
        }
        if self.files.is_empty() {
            s.push_str("\n  no conflicts");
        }
        for f in &self.files {
            let extra = match f.whole {
                Some(Whole::Binary) => " [binary]",
                Some(Whole::Lfs) => " [in git lfs]",
                Some(Whole::Submodule) => " [submodule]",
                Some(Whole::Symlink) => " [a link]",
                Some(Whole::TooLarge) => " [too large to show as lines]",
                None if f.delete_modify => " [deleted on one side]",
                None => "",
            };
            let _ = write!(s, "\n  {:<20?} {}{extra}", f.kind, f.path);
        }
        s
    }
}

impl crate::output::Human for Blocks {
    fn human(&self) -> String {
        use coral_core::conflict::Block;
        let mut s = format!("{} conflicts", self.conflict_count());
        let mut n = 0;
        for block in &self.blocks {
            match block {
                Block::Common { lines } => {
                    let _ = write!(s, "\n  common: {} lines", lines.len());
                }
                Block::Conflict { base, ours, theirs } => {
                    let _ = write!(
                        s,
                        "\n  conflict {n}: ours {} / base {} / theirs {} lines",
                        ours.len(),
                        base.len(),
                        theirs.len()
                    );
                    n += 1;
                }
            }
        }
        s
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    pub path: String,
    pub how: String,
    pub remaining: usize,
}

impl crate::output::Human for Resolved {
    fn human(&self) -> String {
        format!(
            "{} resolved by {}, {} left",
            self.path, self.how, self.remaining
        )
    }
}

/// How to resolve a file from the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum How {
    /// Take the side labelled "ours" — during a rebase, the branch being rebased onto.
    Ours,
    /// Take the side labelled "theirs" — during a rebase, your own replayed commit.
    Theirs,
    /// Remove the file.
    Delete,
    /// Keep the file as it already is on disk and stage it.
    Edited,
}

async fn open(path: &Path) -> Result<(GitRunner, RepoLocation), CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok((runner, loc))
}

/// Lists the files still needing a decision, with what the sides mean.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure.
pub async fn list(path: &Path) -> Result<ConflictList, CoralError> {
    let (runner, loc) = open(path).await?;
    Ok(ConflictList {
        operation: loc.operation(&runner).await?,
        files: loc.conflicts(&runner).await?,
    })
}

/// Shows one file's conflict blocks, rebuilt from the index stages.
///
/// # Errors
/// [`CoralError::Refused`] if the path is not conflicted.
pub async fn show(path: &Path, file: &str) -> Result<Blocks, CoralError> {
    let (runner, loc) = open(path).await?;
    loc.conflict_blocks(&runner, file).await
}

/// Resolves one file.
///
/// # Errors
/// [`CoralError::Refused`] if the choice is impossible for that file.
pub async fn resolve(path: &Path, file: &str, how: How) -> Result<Resolved, CoralError> {
    let (runner, loc) = open(path).await?;
    match how {
        How::Ours => loc.resolve(&runner, file, &Resolution::TakeOurs).await?,
        How::Theirs => loc.resolve(&runner, file, &Resolution::TakeTheirs).await?,
        How::Delete => loc.resolve(&runner, file, &Resolution::Delete).await?,
        How::Edited => loc.mark_resolved(&runner, file).await?,
    }
    Ok(Resolved {
        path: file.to_owned(),
        how: format!("{how:?}").to_lowercase(),
        remaining: loc.conflicts(&runner).await?.len(),
    })
}
