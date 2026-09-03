use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::remote::{PullMode, PushOpts, PushResult, Remote};
use coral_core::repo::RepoLocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Mode {
    /// Refuse unless the local branch can fast-forward.
    FfOnly,
    Merge,
    Rebase,
}

impl From<Mode> for PullMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::FfOnly => Self::FfOnly,
            Mode::Merge => Self::Merge,
            Mode::Rebase => Self::Rebase,
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteList {
    pub remotes: Vec<Remote>,
}

impl crate::output::Human for RemoteList {
    fn human(&self) -> String {
        if self.remotes.is_empty() {
            return "no remotes".to_owned();
        }
        let mut s = format!("{} remotes", self.remotes.len());
        for r in &self.remotes {
            let _ = write!(s, "\n  {:<12} {}", r.name, r.fetch_url);
            if r.push_url != r.fetch_url {
                let _ = write!(s, "\n  {:<12} {} (push)", "", r.push_url);
            }
        }
        s
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub what: String,
    /// Phases git reported, in order, one entry per phase rather than per update.
    pub phases: Vec<String>,
    pub results: Vec<PushResult>,
}

impl crate::output::Human for Transfer {
    fn human(&self) -> String {
        let mut s = self.what.clone();
        for p in &self.phases {
            let _ = write!(s, "\n  {p}");
        }
        for r in &self.results {
            let _ = write!(
                s,
                "\n  {:?} {} -> {} {}",
                r.flag, r.local, r.remote, r.summary
            );
        }
        s
    }
}

async fn open(path: &Path) -> Result<(GitRunner, RepoLocation), CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok((runner, loc))
}

/// Collects one summary line per completed phase, rather than one per progress update.
#[derive(Default)]
struct Phases(Vec<String>);

impl Phases {
    fn record(&mut self, p: &coral_core::remote::Progress) {
        if p.done {
            let where_ = if p.remote { "remote " } else { "" };
            self.0
                .push(format!("{where_}{}: {}/{}", p.phase, p.current, p.total));
        }
    }
}

/// Lists configured remotes.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure.
pub async fn list(path: &Path) -> Result<RemoteList, CoralError> {
    let (runner, loc) = open(path).await?;
    Ok(RemoteList {
        remotes: loc.remotes(&runner).await?,
    })
}

/// Adds, removes, renames or re-points a remote.
///
/// # Errors
/// Propagates git failures.
pub async fn manage(
    path: &Path,
    action: &str,
    name: &str,
    value: Option<&str>,
) -> Result<RemoteList, CoralError> {
    let (runner, loc) = open(path).await?;
    let value = || -> Result<&str, CoralError> {
        value.ok_or_else(|| CoralError::Refused {
            label: "remote",
            detail: format!("{action} needs a value"),
        })
    };
    match action {
        "add" => loc.remote_add(&runner, name, value()?).await?,
        "remove" => loc.remote_remove(&runner, name).await?,
        "rename" => loc.remote_rename(&runner, name, value()?).await?,
        "set-url" => loc.remote_set_url(&runner, name, value()?).await?,
        "prune" => loc.remote_prune(&runner, name).await?,
        other => {
            return Err(CoralError::Refused {
                label: "remote",
                detail: format!("unknown action {other:?}"),
            });
        }
    }
    Ok(RemoteList {
        remotes: loc.remotes(&runner).await?,
    })
}

/// Fetches from one remote, or all of them.
///
/// # Errors
/// Propagates git failures.
pub async fn fetch(
    path: &Path,
    remote: Option<String>,
    prune: bool,
) -> Result<Transfer, CoralError> {
    let (runner, loc) = open(path).await?;
    let mut phases = Phases::default();
    loc.fetch(&runner, remote.as_deref(), prune, |p| phases.record(&p))
        .await?;
    Ok(Transfer {
        what: format!("fetched {}", remote.as_deref().unwrap_or("all remotes")),
        phases: phases.0,
        results: Vec::new(),
    })
}

/// Pushes to a remote.
///
/// # Errors
/// Propagates git failures. A rejected ref is reported in the results, not as an error.
pub async fn push(path: &Path, opts: PushOpts) -> Result<Transfer, CoralError> {
    let (runner, loc) = open(path).await?;
    let mut phases = Phases::default();
    let results = loc.push(&runner, &opts, |p| phases.record(&p)).await?;
    Ok(Transfer {
        what: "pushed".to_owned(),
        phases: phases.0,
        results,
    })
}

/// Fetches and integrates.
///
/// # Errors
/// Propagates git failures; a stopped merge or rebase is an outcome.
pub async fn pull(
    path: &Path,
    remote: Option<String>,
    mode: Mode,
) -> Result<coral_core::ops::OpOutcome, CoralError> {
    let (runner, loc) = open(path).await?;
    loc.pull(&runner, remote.as_deref(), mode.into()).await
}
