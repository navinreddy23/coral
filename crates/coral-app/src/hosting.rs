// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_hosting::pull_request::NewPullRequest;
use coral_hosting::token::Account;
use coral_hosting::{Client, Host, PullRequest, token};

use crate::commands::IpcError;

/// Which stored token a host is reached with.
///
/// Three states rather than a pair of booleans, because "signed in with nothing" and "using a
/// profile's token and the shared one at once" are not things that can happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenSource {
    /// Neither this profile nor the shared account has one.
    None,
    /// This profile's own, which no other profile can see.
    Profile,
    /// The one every profile falls back to, and the only one `coral hosting login` writes.
    Shared,
}

/// What Coral knows about the host behind a repository's remote.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostView {
    pub host: Option<Host>,
    /// Why no host was identified, when none was. Not an error: plain git still works, and
    /// nothing here is allowed to stop it.
    pub detail: Option<String>,
    pub token: TokenSource,
}

/// Which token would be used for `host` from `profile`.
///
/// The profile's own first, then the shared one. A profile that has never been signed in
/// anywhere still works, which is what anybody upgrading from before profiles expects; a
/// profile that has been signed in is answering for itself and the fallback is not consulted.
fn source_for(host: &Host, profile: &str) -> TokenSource {
    if token::load(host, &Account::of_profile(profile))
        .ok()
        .flatten()
        .is_some()
    {
        TokenSource::Profile
    } else if token::load(host, &Account::shared())
        .ok()
        .flatten()
        .is_some()
    {
        TokenSource::Shared
    } else {
        TokenSource::None
    }
}

/// The account a view's token came from, for a command about to write or delete it.
fn account_of(view: &HostView, profile: &str) -> Account {
    match view.token {
        TokenSource::Shared => Account::shared(),
        TokenSource::None | TokenSource::Profile => Account::of_profile(profile),
    }
}

async fn host_of(path: &str, profile: &str) -> Result<HostView, IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(path)).await?;
    let remotes = loc.remotes(&runner).await?;

    let Some(remote) = remotes
        .iter()
        .find(|r| r.name == "origin")
        .or_else(|| remotes.first())
    else {
        return Ok(HostView {
            host: None,
            detail: Some("the repository has no remotes".to_owned()),
            token: TokenSource::None,
        });
    };

    Ok(match Host::detect(&remote.fetch_url) {
        Ok(host) => {
            let token = source_for(&host, profile);
            HostView {
                host: Some(host),
                detail: None,
                token,
            }
        }
        Err(e) => HostView {
            host: None,
            detail: Some(e.to_string()),
            token: TokenSource::None,
        },
    })
}

/// The host behind the repository's remote, and which stored token reaches it.
///
/// # Errors
/// Propagates git failures. An unrecognised host is reported in the result, not raised.
#[tauri::command]
pub async fn hosting_status(
    profiles: tauri::State<'_, crate::profile::Profiles>,
    path: String,
) -> Result<HostView, IpcError> {
    tracing::info!(path, "hosting_status");
    host_of(&path, &current(&profiles)).await
}

/// Stores an API token for the repository's host, under the profile in use.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] when there is no recognised host, or the keyring
/// refuses the write.
#[tauri::command]
pub async fn hosting_login(
    profiles: tauri::State<'_, crate::profile::Profiles>,
    path: String,
    token_value: String,
) -> Result<HostView, IpcError> {
    let profile = current(&profiles);
    let view = host_of(&path, &profile).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    // Tried before it is kept. Stored first and asked after, the window said "Signed in as
    // Personal" about a string the host had never seen, with the rejection arriving a moment
    // later as a second line contradicting the first — and a token the host refuses sitting in
    // the keyring in place of one that worked.
    Client::new(secrecy::SecretString::from(token_value.clone()))
        .map_err(|e| refused("sign in", &e))?
        .check(host)
        .await
        .map_err(|e| refused("sign in", &e))?;
    // Always the profile's own, never the shared one: signing in here is this profile saying
    // who it is, and overwriting a token the other profiles were falling back to would sign
    // them in as somebody they never chose.
    token::store(
        host,
        &Account::of_profile(&profile),
        &secrecy::SecretString::from(token_value),
    )
    .map_err(|e| refused("sign in", &e))?;
    host_of(&path, &profile).await
}

/// Forgets whichever token this profile was reaching the host with.
///
/// The profile's own if it has one, so the other profiles keep theirs. The shared one if that
/// was what it was falling back to, which does sign every other profile out — the window says
/// as much before asking, because there is nothing else it could mean.
///
/// # Errors
/// As [`hosting_login`].
#[tauri::command]
pub async fn hosting_logout(
    profiles: tauri::State<'_, crate::profile::Profiles>,
    path: String,
) -> Result<HostView, IpcError> {
    let profile = current(&profiles);
    let view = host_of(&path, &profile).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    token::forget(host, &account_of(&view, &profile)).map_err(|e| refused("sign out", &e))?;
    host_of(&path, &profile).await
}

/// The repository's pull or merge requests.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] with the provider's own explanation for a refusal, so
/// an expired token says so rather than showing an empty list.
#[tauri::command]
pub async fn hosting_pull_requests(
    profiles: tauri::State<'_, crate::profile::Profiles>,
    path: String,
) -> Result<Vec<PullRequest>, IpcError> {
    tracing::info!(path, "hosting_pull_requests");
    let profile = current(&profiles);
    let view = host_of(&path, &profile).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    let listing = verb(&view, "list pull requests", "list merge requests");
    let secret = secret_for(&view, host, &profile, listing)?;
    let client = Client::new(secret).map_err(|e| refused(listing, &e))?;
    Ok(client
        .pull_requests(host)
        .await
        .map_err(|e| refused(listing, &e))?)
}

/// Opens a new pull or merge request.
///
/// # Errors
/// As [`hosting_pull_requests`].
#[tauri::command]
pub async fn hosting_create(
    profiles: tauri::State<'_, crate::profile::Profiles>,
    path: String,
    title: String,
    body: String,
    source: String,
    target: String,
    draft: bool,
) -> Result<PullRequest, IpcError> {
    let profile = current(&profiles);
    let view = host_of(&path, &profile).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    let opening = verb(&view, "open a pull request", "open a merge request");
    let secret = secret_for(&view, host, &profile, opening)?;
    let client = Client::new(secret).map_err(|e| refused(opening, &e))?;
    let new = NewPullRequest {
        title,
        body,
        source_branch: source,
        target_branch: target,
        draft,
    };
    Ok(client
        .create(host, &new)
        .await
        .map_err(|e| refused(opening, &e))?)
}

/// The id of the profile at the window, which is what a token is filed under.
fn current(profiles: &crate::profile::Profiles) -> String {
    profiles.read().current
}

/// The token the view says is in use, or a refusal naming the host it is missing for.
fn secret_for(
    view: &HostView,
    host: &Host,
    profile: &str,
    label: &'static str,
) -> Result<secrecy::SecretString, IpcError> {
    let account = account_of(view, profile);
    token::load(host, &account)
        .map_err(|e| refused(label, &e))?
        .ok_or_else(|| {
            coral_core::CoralError::Refused {
                label,
                detail: format!("no token stored for {}", host.origin),
            }
            .into()
        })
}

/// The provider's own word for a request, so a GitLab user is not told about pull requests.
fn verb(view: &HostView, github: &'static str, gitlab: &'static str) -> &'static str {
    match view.host.as_ref().map(|h| h.kind) {
        Some(coral_hosting::HostKind::GitLab) => gitlab,
        _ => github,
    }
}

fn no_host(view: &HostView) -> coral_core::CoralError {
    coral_core::CoralError::Refused {
        label: "reach the host",
        detail: view
            .detail
            .clone()
            .unwrap_or_else(|| "no recognised host".to_owned()),
    }
}

// The label is a verb: `CoralError::Refused` renders as "cannot {label}: {detail}", and a noun
// there reads as "cannot hosting", which is what the window was showing.
fn refused(label: &'static str, e: &coral_hosting::HostingError) -> coral_core::CoralError {
    coral_core::CoralError::Refused {
        label,
        detail: e.to_string(),
    }
}
