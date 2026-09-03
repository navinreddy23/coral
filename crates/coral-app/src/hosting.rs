use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_hosting::pull_request::NewPullRequest;
use coral_hosting::{Client, Host, PullRequest, token};

use crate::commands::IpcError;

/// What Coral knows about the host behind a repository's remote.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostView {
    pub host: Option<Host>,
    /// Why no host was identified, when none was. Not an error: plain git still works, and
    /// nothing here is allowed to stop it.
    pub detail: Option<String>,
    pub signed_in: bool,
}

async fn host_of(path: &str) -> Result<HostView, IpcError> {
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
            signed_in: false,
        });
    };

    Ok(match Host::detect(&remote.fetch_url) {
        Ok(host) => {
            let signed_in = token::load(&host).ok().flatten().is_some();
            HostView {
                host: Some(host),
                detail: None,
                signed_in,
            }
        }
        Err(e) => HostView {
            host: None,
            detail: Some(e.to_string()),
            signed_in: false,
        },
    })
}

/// The host behind the repository's remote, and whether a token is stored for it.
///
/// # Errors
/// Propagates git failures. An unrecognised host is reported in the result, not raised.
#[tauri::command]
pub async fn hosting_status(path: String) -> Result<HostView, IpcError> {
    host_of(&path).await
}

/// Stores an API token for the repository's host.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] when there is no recognised host, or the keyring
/// refuses the write.
#[tauri::command]
pub async fn hosting_login(path: String, token_value: String) -> Result<HostView, IpcError> {
    let view = host_of(&path).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    token::store(host, &secrecy::SecretString::from(token_value)).map_err(|e| refused(&e))?;
    host_of(&path).await
}

/// Forgets the stored token for the repository's host.
///
/// # Errors
/// As [`hosting_login`].
#[tauri::command]
pub async fn hosting_logout(path: String) -> Result<HostView, IpcError> {
    let view = host_of(&path).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    token::forget(host).map_err(|e| refused(&e))?;
    host_of(&path).await
}

/// The repository's pull or merge requests.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] with the provider's own explanation for a refusal, so
/// an expired token says so rather than showing an empty list.
#[tauri::command]
pub async fn hosting_pull_requests(path: String) -> Result<Vec<PullRequest>, IpcError> {
    let view = host_of(&path).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    let secret = token::load(host).map_err(|e| refused(&e))?.ok_or_else(|| {
        coral_core::CoralError::Refused {
            label: "hosting",
            detail: format!("no token stored for {}", host.origin),
        }
    })?;
    let client = Client::new(secret).map_err(|e| refused(&e))?;
    Ok(client.pull_requests(host).await.map_err(|e| refused(&e))?)
}

/// Opens a new pull or merge request.
///
/// # Errors
/// As [`hosting_pull_requests`].
#[tauri::command]
pub async fn hosting_create(
    path: String,
    title: String,
    body: String,
    source: String,
    target: String,
    draft: bool,
) -> Result<PullRequest, IpcError> {
    let view = host_of(&path).await?;
    let host = view.host.as_ref().ok_or_else(|| no_host(&view))?;
    let secret = token::load(host).map_err(|e| refused(&e))?.ok_or_else(|| {
        coral_core::CoralError::Refused {
            label: "hosting",
            detail: format!("no token stored for {}", host.origin),
        }
    })?;
    let client = Client::new(secret).map_err(|e| refused(&e))?;
    let new = NewPullRequest {
        title,
        body,
        source_branch: source,
        target_branch: target,
        draft,
    };
    Ok(client.create(host, &new).await.map_err(|e| refused(&e))?)
}

fn no_host(view: &HostView) -> coral_core::CoralError {
    coral_core::CoralError::Refused {
        label: "hosting",
        detail: view
            .detail
            .clone()
            .unwrap_or_else(|| "no recognised host".to_owned()),
    }
}

fn refused(e: &coral_hosting::HostingError) -> coral_core::CoralError {
    coral_core::CoralError::Refused {
        label: "hosting",
        detail: e.to_string(),
    }
}
