use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_hosting::token::Account;
use coral_hosting::{Client, Host, PullRequest, token};

/// The CLI has no profile, so it reads and writes the account every profile falls back to.
///
/// A token stored here works in the window until that profile signs in with one of its own,
/// and a token the window stored for a profile is not visible from here. Both are what the
/// two tools can honestly say: nothing on the command line names which profile is meant.
fn account() -> Account {
    Account::shared()
}

/// What Coral knows about the host behind a repository's remote.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostView {
    pub host: Option<Host>,
    /// Why no host was identified, when none was.
    pub detail: Option<String>,
    /// Whether a token is stored for it.
    pub signed_in: bool,
}

/// Identifies the host behind `origin`, or the first remote if there is no `origin`.
///
/// # Errors
/// Propagates git failures. A remote on an unrecognised host is reported rather than an error:
/// plain git still works, and nothing here is allowed to block that.
pub async fn detect(path: &Path) -> Result<HostView, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let remotes = loc.remotes(&runner).await?;

    let Some(remote) = remotes
        .iter()
        .find(|r| r.name == "origin")
        .or_else(|| remotes.first())
    else {
        return Ok(HostView {
            host: None,
            detail: Some("the repository has no remotes".to_owned()),
            signed_in: false,
        });
    };

    match Host::detect(&remote.fetch_url) {
        Ok(host) => {
            let signed_in = token::load(&host, &account()).ok().flatten().is_some();
            Ok(HostView {
                host: Some(host),
                detail: None,
                signed_in,
            })
        }
        Err(e) => Ok(HostView {
            host: None,
            detail: Some(e.to_string()),
            signed_in: false,
        }),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequests {
    pub pull_requests: Vec<PullRequest>,
}

/// Lists the repository's pull or merge requests.
///
/// # Errors
/// [`CoralError::Refused`] when there is no recognised host or no stored token, and for any
/// refusal from the provider.
pub async fn list(path: &Path) -> Result<PullRequests, CoralError> {
    let view = detect(path).await?;
    let host = view.host.ok_or_else(|| CoralError::Refused {
        label: "reach the host",
        detail: view.detail.unwrap_or_else(|| "no host".to_owned()),
    })?;

    let secret = token::load(&host, &account())
        .map_err(|e| refused("list pull requests", &e))?
        .ok_or_else(|| CoralError::Refused {
            label: "list pull requests",
            detail: format!(
                "no token stored for {}; run `coral host-login`",
                host.origin
            ),
        })?;

    let client = Client::new(secret).map_err(|e| refused("list pull requests", &e))?;
    let pull_requests = client
        .pull_requests(&host)
        .await
        .map_err(|e| refused("list pull requests", &e))?;
    Ok(PullRequests { pull_requests })
}

/// Stores a token for the repository's host, read from stdin.
///
/// # Errors
/// [`CoralError::Refused`] when there is no recognised host, or the keyring refuses.
pub async fn login(path: &Path, secret: secrecy::SecretString) -> Result<Done, CoralError> {
    let view = detect(path).await?;
    let host = view.host.ok_or_else(|| CoralError::Refused {
        label: "sign in",
        detail: view.detail.unwrap_or_else(|| "no host".to_owned()),
    })?;
    // Tried before it is kept, so a typo or an expired token does not replace one that works.
    Client::new(secret.clone())
        .map_err(|e| refused("sign in", &e))?
        .check(&host)
        .await
        .map_err(|e| refused("sign in", &e))?;
    token::store(&host, &account(), &secret).map_err(|e| refused("sign in", &e))?;
    Ok(Done {
        what: format!("stored a token for {}, for every profile", host.origin),
    })
}

/// Forgets the token for the repository's host.
///
/// # Errors
/// As [`login`].
pub async fn logout(path: &Path) -> Result<Done, CoralError> {
    let view = detect(path).await?;
    let host = view.host.ok_or_else(|| CoralError::Refused {
        label: "sign out",
        detail: view.detail.unwrap_or_else(|| "no host".to_owned()),
    })?;
    token::forget(&host, &account()).map_err(|e| refused("sign out", &e))?;
    Ok(Done {
        what: format!("forgot the shared token for {}", host.origin),
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    pub what: String,
}

// The label is a verb: `CoralError::Refused` renders as "cannot {label}: {detail}", and a noun
// there reads as "cannot hosting".
fn refused(label: &'static str, e: &coral_hosting::HostingError) -> CoralError {
    CoralError::Refused {
        label,
        detail: e.to_string(),
    }
}

impl crate::output::Human for HostView {
    fn human(&self) -> String {
        match &self.host {
            Some(h) => format!(
                "{:?} at {} — {}/{}{}",
                h.kind,
                h.origin,
                h.owner,
                h.repo,
                if self.signed_in {
                    ", signed in"
                } else {
                    ", no token stored"
                }
            ),
            None => self.detail.clone().unwrap_or_else(|| "no host".to_owned()),
        }
    }
}

impl crate::output::Human for PullRequests {
    fn human(&self) -> String {
        if self.pull_requests.is_empty() {
            return "no pull requests".to_owned();
        }
        let mut out = format!("{} pull requests", self.pull_requests.len());
        for pr in &self.pull_requests {
            let _ = write!(
                out,
                "\n  #{:<5} {:<10?} {:<28} {} → {}",
                pr.number,
                pr.state,
                truncate(&pr.title, 28),
                pr.source_branch,
                pr.target_branch
            );
        }
        out
    }
}

impl crate::output::Human for Done {
    fn human(&self) -> String {
        self.what.clone()
    }
}

fn truncate(s: &str, at: usize) -> String {
    if s.chars().count() <= at {
        return s.to_owned();
    }
    s.chars().take(at.saturating_sub(1)).collect::<String>() + "…"
}
