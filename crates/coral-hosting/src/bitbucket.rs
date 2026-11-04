//! Bitbucket Cloud: the 2.0 API at `api.bitbucket.org`, a workspace for the owner, and pages
//! of `values` for every list.
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/>

use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};
use crate::review::{Remark, Reply, Verdict};

#[must_use]
pub fn api_base(host: &Host) -> String {
    // A test server stands in for both origins; bitbucket.org itself serves the API elsewhere.
    let api = if host.origin == "https://bitbucket.org" {
        "https://api.bitbucket.org".to_owned()
    } else {
        host.origin.clone()
    };
    format!("{api}/2.0/repositories/{}/{}", host.owner, host.repo)
}

#[must_use]
pub fn pulls_url(host: &Host, per_page: u32) -> String {
    format!(
        "{}/pullrequests?state=OPEN&state=MERGED&state=DECLINED&sort=-updated_on&pagelen={}",
        api_base(host),
        per_page.min(50)
    )
}

#[must_use]
pub fn create_url(host: &Host) -> String {
    format!("{}/pullrequests", api_base(host))
}

#[must_use]
pub fn one_url(host: &Host, id: u64) -> String {
    format!("{}/pullrequests/{id}", api_base(host))
}

#[must_use]
pub fn statuses_url(host: &Host, id: u64) -> String {
    format!("{}/pullrequests/{id}/statuses?pagelen=100", api_base(host))
}

#[must_use]
pub fn comments_url(host: &Host, id: u64) -> String {
    format!("{}/pullrequests/{id}/comments?pagelen=100", api_base(host))
}

#[must_use]
pub fn create_body(new: &NewPullRequest) -> serde_json::Value {
    serde_json::json!({
        "title": new.title,
        "description": new.body,
        "source": { "branch": { "name": new.source_branch } },
        "destination": { "branch": { "name": new.target_branch } },
        "draft": new.draft,
    })
}

/// What a reply posts: a comment, or a verdict and, when there are words, a comment beside it,
/// since Bitbucket's verdicts carry no text of their own.
#[must_use]
pub fn reply_requests(host: &Host, id: u64, reply: &Reply) -> Vec<(String, serde_json::Value)> {
    let comment = |body: &str| {
        (
            format!("{}/pullrequests/{id}/comments", api_base(host)),
            serde_json::json!({ "content": { "raw": body } }),
        )
    };
    let (verdict, body) = match reply {
        Reply::Comment { body } => return vec![comment(body)],
        Reply::Approve { body } => ("approve", body),
        Reply::RequestChanges { body } => ("request-changes", body),
    };
    let mut out = vec![(
        format!("{}/pullrequests/{id}/{verdict}", api_base(host)),
        serde_json::json!({}),
    )];
    if !body.trim().is_empty() {
        out.push(comment(body));
    }
    out
}

#[derive(Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
}

#[derive(Deserialize)]
struct Account {
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
struct Branch {
    name: String,
}

#[derive(Deserialize)]
struct Commit {
    hash: String,
}

#[derive(Deserialize)]
struct End {
    branch: Branch,
    #[serde(default)]
    commit: Option<Commit>,
}

#[derive(Deserialize)]
struct Href {
    href: String,
}

#[derive(Deserialize)]
struct Links {
    html: Href,
}

#[derive(Deserialize)]
struct Wire {
    id: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    author: Option<Account>,
    source: End,
    destination: End,
    links: Links,
    updated_on: String,
}

fn name(account: Option<Account>) -> String {
    account
        .map(|a| a.display_name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "(unknown)".to_owned())
}

fn convert(w: Wire) -> PullRequest {
    let state = match w.state.as_str() {
        "MERGED" => PrState::Merged,
        "DECLINED" | "SUPERSEDED" => PrState::Closed,
        _ if w.draft => PrState::Draft,
        _ => PrState::Open,
    };
    PullRequest {
        number: w.id,
        title: w.title,
        state,
        author: name(w.author),
        // Bitbucket abbreviates it to twelve; checks are read by the request, not the commit.
        head_sha: w.source.commit.map(|c| c.hash).unwrap_or_default(),
        source_branch: w.source.branch.name,
        target_branch: w.destination.branch.name,
        web_url: w.links.html.href,
        updated_at: w.updated_on,
    }
}

/// Parses a page of pull requests.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not one.
pub fn parse_pulls(body: &[u8]) -> Result<Vec<PullRequest>, HostingError> {
    let page: Page<Wire> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(page.values.into_iter().map(convert).collect())
}

/// Parses one pull request.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not one.
pub fn parse_one(body: &[u8]) -> Result<PullRequest, HostingError> {
    let raw: Wire =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(convert(raw))
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    raw: String,
}

#[derive(Deserialize)]
struct Inline {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    to: Option<u64>,
}

#[derive(Deserialize)]
struct WireComment {
    #[serde(default)]
    user: Option<Account>,
    content: Content,
    created_on: String,
    #[serde(default)]
    inline: Option<Inline>,
    #[serde(default)]
    deleted: bool,
}

/// Parses the comments on a pull request, line comments among them.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a page of comments.
pub fn parse_comments(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let page: Page<WireComment> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(page
        .values
        .into_iter()
        .filter(|c| !c.deleted)
        .map(|c| {
            let (path, line) = c.inline.map_or((None, None), |i| (i.path, i.to));
            Remark {
                author: name(c.user),
                body: c.content.raw,
                at: c.created_on,
                verdict: None,
                path,
                line,
            }
        })
        .collect())
}

#[derive(Deserialize)]
struct Participant {
    #[serde(default)]
    user: Option<Account>,
    #[serde(default)]
    approved: bool,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    participated_on: Option<String>,
}

#[derive(Deserialize)]
struct WithParticipants {
    #[serde(default)]
    participants: Vec<Participant>,
}

/// The verdicts on a pull request, which Bitbucket keeps on its participants.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a pull request.
pub fn parse_verdicts(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let raw: WithParticipants =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw
        .participants
        .into_iter()
        .filter_map(|p| {
            let verdict = match p.state.as_deref() {
                Some("changes_requested") => Verdict::ChangesRequested,
                Some("approved") => Verdict::Approved,
                _ if p.approved => Verdict::Approved,
                _ => return None,
            };
            Some(Remark {
                author: name(p.user),
                body: String::new(),
                at: p.participated_on.unwrap_or_default(),
                verdict: Some(verdict),
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
        Host::detect_with("git@bitbucket.org:team/thing.git", |_| None).unwrap()
    }

    #[test]
    fn a_page_of_pull_requests_is_read_and_the_api_is_its_own_origin() {
        let body = br#"{"values":[{"id":7,"title":"Add it","state":"OPEN","draft":true,
          "author":{"display_name":"Ada"},"source":{"branch":{"name":"topic"},"commit":{"hash":"0123456789ab"}},
          "destination":{"branch":{"name":"main"}},"links":{"html":{"href":"https://bitbucket.org/team/thing/pull-requests/7"}},
          "updated_on":"2026-11-01T10:00:00+00:00"}],"pagelen":50}"#;
        let pulls = parse_pulls(body).unwrap();
        assert_eq!(pulls[0].number, 7);
        assert_eq!(pulls[0].state, PrState::Draft);
        assert_eq!(pulls[0].author, "Ada");
        assert!(
            pulls_url(&host(), 50)
                .starts_with("https://api.bitbucket.org/2.0/repositories/team/thing/pullrequests?")
        );
    }

    #[test]
    fn a_verdict_with_words_is_two_requests() {
        let reqs = reply_requests(
            &host(),
            7,
            &Reply::Approve {
                body: "lgtm".into(),
            },
        );
        assert_eq!(reqs.len(), 2);
        assert!(reqs[0].0.ends_with("/pullrequests/7/approve"));
        assert_eq!(reqs[1].1["content"]["raw"], "lgtm");
        assert_eq!(
            reply_requests(
                &host(),
                7,
                &Reply::RequestChanges {
                    body: String::new()
                }
            )
            .len(),
            1
        );
        let verdicts = parse_verdicts(
            br#"{"participants":[{"user":{"display_name":"Bob"},"approved":false,"state":"changes_requested","participated_on":"2026-11-02T00:00:00Z"},
                                 {"user":{"display_name":"Cy"},"approved":false,"state":null}]}"#,
        )
        .unwrap();
        assert_eq!(verdicts.len(), 1);
        assert_eq!(verdicts[0].verdict, Some(Verdict::ChangesRequested));
    }
}
