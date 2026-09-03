use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::refs::{GitRef, RefKind};
use coral_core::repo::RepoLocation;

/// Which refs to list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    All,
    Local,
    Remote,
    Tag,
    Stash,
}

impl Kind {
    fn matches(self, r: &GitRef) -> bool {
        match self {
            Self::All => true,
            Self::Local => matches!(r.kind, RefKind::LocalBranch),
            Self::Remote => matches!(r.kind, RefKind::RemoteBranch { .. }),
            Self::Tag => matches!(r.kind, RefKind::Tag { .. }),
            Self::Stash => matches!(r.kind, RefKind::Stash),
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefList {
    pub refs: Vec<GitRef>,
}

/// Lists refs, optionally filtered by kind.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path, kind: Kind) -> Result<RefList, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let refs = loc
        .refs(&runner)
        .await?
        .into_iter()
        .filter(|r| kind.matches(r))
        .collect();
    Ok(RefList { refs })
}

impl crate::output::Human for RefList {
    fn human(&self) -> String {
        if self.refs.is_empty() {
            return "no refs".to_owned();
        }
        let mut out = format!("{} refs", self.refs.len());
        for r in &self.refs {
            let tracking = match (&r.upstream, r.ahead, r.behind) {
                (Some(u), 0, 0) => format!("  -> {u}"),
                (Some(u), a, b) => format!("  -> {u} (+{a} -{b})"),
                (None, _, _) => String::new(),
            };
            let _ = write!(out, "\n  {:.8}  {:<40}{tracking}", r.commit(), r.short);
        }
        out
    }
}
