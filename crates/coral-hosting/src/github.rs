use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};

/// Where the REST API lives for a host.
///
/// github.com serves its API from a separate origin, while GitHub Enterprise serves it from
/// `/api/v3` on the instance itself. Getting this wrong is a 404 against the HTML site, which
/// reads as "no pull requests" rather than as a misconfiguration.
#[must_use]
pub fn api_base(host: &Host) -> String {
    if host.origin.ends_with("//github.com") {
        "https://api.github.com".to_owned()
    } else {
        format!("{}/api/v3", host.origin)
    }
}

#[must_use]
pub fn pulls_url(host: &Host, per_page: u32) -> String {
    format!(
        "{}/repos/{}/{}/pulls?state=all&sort=updated&direction=desc&per_page={per_page}",
        api_base(host),
        host.owner,
        host.repo,
    )
}

#[must_use]
pub fn create_url(host: &Host) -> String {
    format!(
        "{}/repos/{}/{}/pulls",
        api_base(host),
        host.owner,
        host.repo
    )
}

/// The body `POST /pulls` takes.
#[must_use]
pub fn create_body(new: &NewPullRequest) -> serde_json::Value {
    serde_json::json!({
        "title": new.title,
        "body": new.body,
        "head": new.source_branch,
        "base": new.target_branch,
        "draft": new.draft,
    })
}

#[derive(Deserialize)]
struct Wire {
    number: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    user: Option<User>,
    head: Ref,
    base: Ref,
    html_url: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct User {
    login: String,
}

#[derive(Deserialize)]
struct Ref {
    #[serde(rename = "ref")]
    name: String,
}

/// Parses a `GET /pulls` response.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not the array of pull requests documented.
pub fn parse_pulls(body: &[u8]) -> Result<Vec<PullRequest>, HostingError> {
    let raw: Vec<Wire> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw.into_iter().map(convert).collect())
}

/// Parses the single pull request `POST /pulls` answers with.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a pull request.
pub fn parse_one(body: &[u8]) -> Result<PullRequest, HostingError> {
    let raw: Wire =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(convert(raw))
}

fn convert(w: Wire) -> PullRequest {
    // `merged_at` is what distinguishes merged from closed: GitHub reports both as "closed",
    // and calling a merged branch closed is the difference between "landed" and "rejected".
    let state = if w.merged_at.is_some() {
        PrState::Merged
    } else if w.state == "closed" {
        PrState::Closed
    } else if w.draft {
        PrState::Draft
    } else {
        PrState::Open
    };

    PullRequest {
        number: w.number,
        title: w.title,
        state,
        author: w.user.map_or_else(|| "(unknown)".to_owned(), |u| u.login),
        source_branch: w.head.name,
        target_branch: w.base.name,
        web_url: w.html_url,
        updated_at: w.updated_at,
    }
}
