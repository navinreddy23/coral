//! Gitea and Forgejo, which share one API: close to GitHub's in shape, at `/api/v1` on the
//! instance, with the token sent as `token …`.
//! <https://gitea.com/api/swagger>

use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};
use crate::review::{Remark, Reply, Verdict};

#[must_use]
pub fn api_base(host: &Host) -> String {
    format!("{}/api/v1/repos/{}/{}", host.origin, host.owner, host.repo)
}

#[must_use]
pub fn pulls_url(host: &Host, per_page: u32) -> String {
    format!(
        "{}/pulls?state=all&sort=recentupdate&limit={per_page}",
        api_base(host)
    )
}

#[must_use]
pub fn create_url(host: &Host) -> String {
    format!("{}/pulls", api_base(host))
}

/// The combined status of one commit, which Gitea Actions and outside CI both report into.
#[must_use]
pub fn status_url(host: &Host, sha: &str) -> String {
    format!("{}/commits/{sha}/status", api_base(host))
}

#[must_use]
pub fn issue_url(host: &Host, number: u64) -> String {
    format!("{}/issues/{number}", api_base(host))
}

#[must_use]
pub fn comments_url(host: &Host, number: u64) -> String {
    format!("{}/issues/{number}/comments", api_base(host))
}

#[must_use]
pub fn reviews_url(host: &Host, number: u64) -> String {
    format!("{}/pulls/{number}/reviews", api_base(host))
}

/// The body `POST /pulls` takes. A draft is a `WIP:` title, which is how Gitea marks one.
#[must_use]
pub fn create_body(new: &NewPullRequest) -> serde_json::Value {
    let title = if new.draft {
        format!("WIP: {}", new.title)
    } else {
        new.title.clone()
    };
    serde_json::json!({
        "title": title,
        "body": new.body,
        "head": new.source_branch,
        "base": new.target_branch,
    })
}

/// Where a reply goes and what is sent: a comment joins the conversation, a verdict is a
/// review.
#[must_use]
pub fn reply_request(host: &Host, number: u64, reply: &Reply) -> (String, serde_json::Value) {
    match reply {
        Reply::Comment { body } => (
            comments_url(host, number),
            serde_json::json!({ "body": body }),
        ),
        Reply::Approve { body } => (
            reviews_url(host, number),
            serde_json::json!({ "event": "APPROVED", "body": body }),
        ),
        Reply::RequestChanges { body } => (
            reviews_url(host, number),
            serde_json::json!({ "event": "REQUEST_CHANGES", "body": body }),
        ),
    }
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

#[derive(Deserialize)]
struct Wire {
    number: u64,
    title: String,
    state: String,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    user: Option<User>,
    head: Ref,
    base: Ref,
    html_url: String,
    updated_at: String,
}

fn login(user: Option<User>) -> String {
    user.map_or_else(|| "(unknown)".to_owned(), |u| u.login)
}

fn convert(w: Wire) -> PullRequest {
    let wip = ["WIP:", "[WIP]", "Draft:"]
        .iter()
        .any(|p| w.title.starts_with(p));
    let state = if w.merged {
        PrState::Merged
    } else if w.state == "closed" {
        PrState::Closed
    } else if w.draft || wip {
        PrState::Draft
    } else {
        PrState::Open
    };
    PullRequest {
        number: w.number,
        title: w.title,
        state,
        author: login(w.user),
        head_sha: w.head.sha,
        source_branch: w.head.name,
        target_branch: w.base.name,
        web_url: w.html_url,
        updated_at: w.updated_at,
    }
}

/// Parses `GET /pulls`.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not the array of pull requests documented.
pub fn parse_pulls(body: &[u8]) -> Result<Vec<PullRequest>, HostingError> {
    let raw: Vec<Wire> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw.into_iter().map(convert).collect())
}

/// Parses the one pull request `POST /pulls` answers with.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a pull request.
pub fn parse_one(body: &[u8]) -> Result<PullRequest, HostingError> {
    let raw: Wire =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(convert(raw))
}

#[derive(Deserialize)]
struct WireComment {
    #[serde(default)]
    user: Option<User>,
    #[serde(default)]
    body: String,
    created_at: String,
}

#[derive(Deserialize)]
struct WireReview {
    #[serde(default)]
    user: Option<User>,
    #[serde(default)]
    body: String,
    state: String,
    #[serde(default)]
    submitted_at: Option<String>,
}

/// The conversation on a pull request, which Gitea files under its issue.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not an array of comments.
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
            path: None,
            line: None,
        })
        .collect())
}

/// The reviews that said something: a verdict, or a comment with words in it. A pending one
/// is a draft only its author sees.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not an array of reviews.
pub fn parse_reviews(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let raw: Vec<WireReview> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw
        .into_iter()
        .filter_map(|r| {
            let verdict = match r.state.as_str() {
                "APPROVED" => Some(Verdict::Approved),
                "REQUEST_CHANGES" => Some(Verdict::ChangesRequested),
                "COMMENT" if !r.body.trim().is_empty() => None,
                _ => return None,
            };
            Some(Remark {
                author: login(r.user),
                body: r.body,
                at: r.submitted_at.unwrap_or_default(),
                verdict,
                path: None,
                line: None,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        Host::detect_with("https://codeberg.org/team/thing.git", |_| None).unwrap()
    }

    #[test]
    fn a_pull_list_is_read_with_merged_and_wip_told_apart() {
        let body = br#"[
          {"number":3,"title":"WIP: half done","state":"open","merged":false,"user":{"login":"ada"},
           "head":{"ref":"topic","sha":"aaaa"},"base":{"ref":"main"},"html_url":"https://codeberg.org/team/thing/pulls/3","updated_at":"2026-11-01T10:00:00Z"},
          {"number":2,"title":"Landed","state":"closed","merged":true,"user":{"login":"bob"},
           "head":{"ref":"done","sha":"bbbb"},"base":{"ref":"main"},"html_url":"https://x/2","updated_at":"2026-10-30T10:00:00Z"}
        ]"#;
        let pulls = parse_pulls(body).unwrap();
        assert_eq!(pulls[0].state, PrState::Draft);
        assert_eq!(pulls[0].head_sha, "aaaa");
        assert_eq!(pulls[1].state, PrState::Merged);
        assert_eq!(
            pulls_url(&host(), 50),
            "https://codeberg.org/api/v1/repos/team/thing/pulls?state=all&sort=recentupdate&limit=50"
        );
    }

    #[test]
    fn reviews_keep_verdicts_and_words_and_drop_drafts() {
        let body = br#"[
          {"user":{"login":"ada"},"body":"","state":"APPROVED","submitted_at":"2026-11-01T10:00:00Z"},
          {"user":{"login":"bob"},"body":"","state":"COMMENT","submitted_at":"2026-11-01T11:00:00Z"},
          {"user":{"login":"cy"},"body":"secret","state":"PENDING"}
        ]"#;
        let said = parse_reviews(body).unwrap();
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].verdict, Some(Verdict::Approved));
        let (url, payload) =
            reply_request(&host(), 3, &Reply::RequestChanges { body: "no".into() });
        assert!(url.ends_with("/pulls/3/reviews"));
        assert_eq!(payload["event"], "REQUEST_CHANGES");
    }
}
