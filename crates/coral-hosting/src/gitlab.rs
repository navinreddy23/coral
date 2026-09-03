use std::fmt::Write as _;

use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};

#[must_use]
pub fn api_base(host: &Host) -> String {
    format!("{}/api/v4", host.origin)
}

/// A project is addressed by its full path, percent-encoded whole.
///
/// The slashes between a group, its subgroups and the project are part of the identifier and
/// have to survive as `%2F`; leaving them raw addresses a different, usually absent, endpoint.
#[must_use]
pub fn project_id(host: &Host) -> String {
    let path = format!("{}/{}", host.owner, host.repo);
    let mut out = String::with_capacity(path.len() + 8);
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}

#[must_use]
pub fn merge_requests_url(host: &Host, per_page: u32) -> String {
    format!(
        "{}/projects/{}/merge_requests?scope=all&order_by=updated_at&sort=desc&per_page={per_page}",
        api_base(host),
        project_id(host),
    )
}

#[must_use]
pub fn create_url(host: &Host) -> String {
    format!(
        "{}/projects/{}/merge_requests",
        api_base(host),
        project_id(host)
    )
}

/// The body `POST /merge_requests` takes.
///
/// A draft is marked by a `Draft:` title prefix rather than a field, which is how GitLab has
/// represented it since it dropped the `WIP:` form.
#[must_use]
pub fn create_body(new: &NewPullRequest) -> serde_json::Value {
    let title = if new.draft {
        format!("Draft: {}", new.title)
    } else {
        new.title.clone()
    };
    serde_json::json!({
        "title": title,
        "description": new.body,
        "source_branch": new.source_branch,
        "target_branch": new.target_branch,
    })
}

#[derive(Deserialize)]
struct Wire {
    iid: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: Option<bool>,
    #[serde(default)]
    work_in_progress: Option<bool>,
    #[serde(default)]
    author: Option<Author>,
    source_branch: String,
    target_branch: String,
    web_url: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct Author {
    username: String,
}

/// Parses a `GET /merge_requests` response.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not the documented array.
pub fn parse_merge_requests(body: &[u8]) -> Result<Vec<PullRequest>, HostingError> {
    let raw: Vec<Wire> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw.into_iter().map(convert).collect())
}

/// Parses the single merge request `POST /merge_requests` answers with.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a merge request.
pub fn parse_one(body: &[u8]) -> Result<PullRequest, HostingError> {
    let raw: Wire =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(convert(raw))
}

fn convert(w: Wire) -> PullRequest {
    // `iid` is the per-project number people quote; `id` is instance-wide and means nothing to
    // anyone reading a branch name.
    let draft = w.draft.or(w.work_in_progress).unwrap_or(false)
        || w.title.starts_with("Draft:")
        || w.title.starts_with("WIP:");

    let state = match w.state.as_str() {
        "merged" => PrState::Merged,
        "closed" | "locked" => PrState::Closed,
        _ if draft => PrState::Draft,
        _ => PrState::Open,
    };

    PullRequest {
        number: w.iid,
        title: w.title,
        state,
        author: w
            .author
            .map_or_else(|| "(unknown)".to_owned(), |a| a.username),
        source_branch: w.source_branch,
        target_branch: w.target_branch,
        web_url: w.web_url,
        updated_at: w.updated_at,
    }
}
