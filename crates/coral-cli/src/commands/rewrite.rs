use std::path::Path;

use coral_core::CoralError;
use coral_core::ops::OpOutcome;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::sequence::Rewrite;

/// The edits the commit menu offers, as clap sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    /// Remove the commit, replaying its children onto its parent.
    Drop,
    /// Replace the commit's message. Needs --message.
    Reword,
    /// Swap the commit with its child, moving it towards HEAD.
    MoveNewer,
    /// Swap the commit with its parent, moving it away from HEAD.
    MoveOlder,
}

/// Rewrites one commit in place.
///
/// # Errors
/// [`CoralError::Refused`] for a reword with no message, and for the cases
/// [`RepoLocation::rewrite_commit`] refuses. Otherwise propagates git failures.
pub async fn run(
    repo: &Path,
    rev: &str,
    kind: Kind,
    message: Option<String>,
) -> Result<OpOutcome, CoralError> {
    let rewrite = match kind {
        Kind::Drop => Rewrite::Drop,
        Kind::Reword => {
            let Some(m) = message else {
                return Err(CoralError::Refused {
                    label: "rewrite",
                    detail: "a reword needs --message".to_owned(),
                });
            };
            Rewrite::Reword(m)
        }
        Kind::MoveNewer => Rewrite::MoveNewer,
        Kind::MoveOlder => Rewrite::MoveOlder,
    };

    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, repo).await?;
    // git runs this binary as its sequence editor, the same self-invocation the credential
    // helper uses; a packaged application cannot assume coral is on the PATH.
    let binary = std::env::current_exe().map_err(|e| CoralError::Protocol {
        label: "rewrite",
        detail: format!("could not find this executable: {e}"),
    })?;
    loc.rewrite_commit(&runner, rev, &rewrite, &binary).await
}
