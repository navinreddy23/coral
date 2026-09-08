//! Making a repository that is not there yet, from the terminal.
//!
//! The one thing the window could do and the CLI could not. That mattered beyond tidiness:
//! cloning had no headless test surface, so the only way to exercise it was to drive the
//! window at a real host.

use std::path::{Path, PathBuf};

use coral_core::CoralError;
use coral_core::create::{Cloned, NewRepo};
use coral_core::process::GitRunner;
use serde::Serialize;

/// Where a repository ended up, which is what the caller opens next.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Made {
    pub path: PathBuf,
}

impl crate::output::Human for Made {
    fn human(&self) -> String {
        self.path.display().to_string()
    }
}

/// Creates an empty repository.
///
/// # Errors
/// [`CoralError::AlreadyARepository`] if there is one there already, and git's own failure
/// otherwise.
pub async fn init(path: &Path, branch: Option<String>, lfs: bool) -> Result<Made, CoralError> {
    let runner = GitRunner::discover().await?;
    let made = coral_core::create::init(
        &runner,
        &NewRepo {
            path: path.to_path_buf(),
            branch,
            lfs,
        },
    )
    .await?;
    Ok(Made { path: made })
}

/// Clones a repository, reporting progress as git reports it.
///
/// # Errors
/// [`CoralError::AlreadyARepository`] if the destination already holds one, and git's own
/// failure otherwise — a URL that does not answer, a key that authenticates as somebody else.
pub async fn clone(
    url: String,
    into: &Path,
    name: Option<String>,
    ssh_key: Option<String>,
    depth: Option<u32>,
    blobless: bool,
    quiet: bool,
) -> Result<Made, CoralError> {
    let runner = GitRunner::discover().await?;
    let what = Cloned {
        url,
        parent: into.to_path_buf(),
        name,
        ssh_key,
        depth,
        blobless,
    };
    // Written to stderr, since stdout carries the envelope and a progress line is not part of
    // it. Carriage returns, so a terminal rewrites one line rather than scrolling.
    let made = coral_core::create::clone(&runner, &what, |p| {
        if !quiet {
            // Carriage return and a clear-to-end, so a terminal rewrites one line rather than
            // scrolling a thousand of them past.
            eprint!(
                "\r{} {}% ({}/{})\x1b[K",
                p.phase, p.percent, p.current, p.total
            );
        }
    })
    .await?;
    if !quiet {
        eprintln!();
    }
    Ok(Made { path: made })
}
