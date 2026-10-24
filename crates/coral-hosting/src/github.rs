use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};
use crate::review::{Remark, Reply, Verdict};

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

/// The check runs on one commit. A hundred is the most a page holds and more than any
/// pipeline here has.
#[must_use]
pub fn check_runs_url(host: &Host, sha: &str) -> String {
    format!(
        "{}/repos/{}/{}/commits/{sha}/check-runs?per_page=100",
        api_base(host),
        host.owner,
        host.repo
    )
}

/// The commit statuses on one commit, the older mechanism some integrations still use.
#[must_use]
pub fn statuses_url(host: &Host, sha: &str) -> String {
    format!(
        "{}/repos/{}/{}/commits/{sha}/status",
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
    #[serde(default)]
    sha: String,
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
        head_sha: w.head.sha,
        source_branch: w.head.name,
        target_branch: w.base.name,
        web_url: w.html_url,
        updated_at: w.updated_at,
    }
}

/// The conversation on a pull request: comments not tied to a line, which GitHub files under
/// the pull request's issue.
#[must_use]
pub fn conversation_url(host: &Host, number: u64) -> String {
    format!(
        "{}/repos/{}/{}/issues/{number}/comments?per_page=100",
        api_base(host),
        host.owner,
        host.repo
    )
}

/// Comments left on lines of the diff.
#[must_use]
pub fn line_comments_url(host: &Host, number: u64) -> String {
    format!(
        "{}/repos/{}/{}/pulls/{number}/comments?per_page=100",
        api_base(host),
        host.owner,
        host.repo
    )
}

/// Reviews, each with its verdict.
#[must_use]
pub fn reviews_url(host: &Host, number: u64) -> String {
    format!(
        "{}/repos/{}/{}/pulls/{number}/reviews?per_page=100",
        api_base(host),
        host.owner,
        host.repo
    )
}

/// Where a reply goes and what is sent: a plain comment joins the conversation, a verdict is
/// a review.
#[must_use]
pub fn reply_request(host: &Host, number: u64, reply: &Reply) -> (String, serde_json::Value) {
    let reviews = format!(
        "{}/repos/{}/{}/pulls/{number}/reviews",
        api_base(host),
        host.owner,
        host.repo
    );
    match reply {
        Reply::Comment { body } => (
            format!(
                "{}/repos/{}/{}/issues/{number}/comments",
                api_base(host),
                host.owner,
                host.repo
            ),
            serde_json::json!({ "body": body }),
        ),
        Reply::Approve { body } => (
            reviews,
            serde_json::json!({ "event": "APPROVE", "body": body }),
        ),
        Reply::RequestChanges { body } => (
            reviews,
            serde_json::json!({ "event": "REQUEST_CHANGES", "body": body }),
        ),
    }
}

#[derive(Deserialize)]
struct WireComment {
    #[serde(default)]
    user: Option<User>,
    #[serde(default)]
    body: String,
    created_at: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    line: Option<u64>,
    /// Where the comment was made, for one whose line has since been changed away.
    #[serde(default)]
    original_line: Option<u64>,
}

#[derive(Deserialize)]
struct WireReview {
    #[serde(default)]
    user: Option<User>,
    #[serde(default)]
    body: Option<String>,
    state: String,
    #[serde(default)]
    submitted_at: Option<String>,
}

fn login(user: Option<User>) -> String {
    user.map_or_else(|| "(unknown)".to_owned(), |u| u.login)
}

/// Parses the conversation or the line comments; both are arrays of the same comment shape.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not that array.
pub fn parse_comments(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let raw: Vec<WireComment> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw
        .into_iter()
        .map(|c| Remark {
            author: login(c.user),
            body: c.body,
            at: c.created_at,
            verdict: None,
            line: c.path.as_ref().and(c.line.or(c.original_line)),
            path: c.path,
        })
        .collect())
}

/// Parses the reviews, keeping those that said something.
///
/// A review left as "Comment" with no text of its own is only the envelope GitHub puts line
/// comments in, and those are listed in their own right; a pending one is still a draft only
/// its author can see; a dismissed one no longer stands.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not the array of reviews.
pub fn parse_reviews(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let raw: Vec<WireReview> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw
        .into_iter()
        .filter_map(|r| {
            let body = r.body.unwrap_or_default();
            let verdict = match r.state.as_str() {
                "APPROVED" => Some(Verdict::Approved),
                "CHANGES_REQUESTED" => Some(Verdict::ChangesRequested),
                "COMMENTED" if !body.trim().is_empty() => None,
                _ => return None,
            };
            Some(Remark {
                author: login(r.user),
                body,
                at: r.submitted_at.unwrap_or_default(),
                verdict,
                path: None,
                line: None,
            })
        })
        .collect())
}
