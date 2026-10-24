use std::fmt::Write as _;

use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};
use crate::review::{Remark, Reply, Verdict};

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

/// The newest pipeline run for a merge request.
///
/// By the request rather than by its tip commit: a merged-results pipeline runs on a commit
/// GitLab makes for the purpose, and a fork's runs in the fork's project, so asking by the tip
/// finds neither and calls a request with passing checks one with none.
/// <https://docs.gitlab.com/api/merge_requests/#list-merge-request-pipelines>
#[must_use]
pub fn pipelines_url(host: &Host, iid: u64) -> String {
    format!(
        "{}/projects/{}/merge_requests/{iid}/pipelines?per_page=1",
        api_base(host),
        project_id(host),
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
    #[serde(default)]
    sha: Option<String>,
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
        head_sha: w.sha.unwrap_or_default(),
    }
}

/// Every note on a merge request, oldest first: comments, comments on lines of the diff, and
/// the system notes that record approvals.
#[must_use]
pub fn notes_url(host: &Host, iid: u64) -> String {
    format!(
        "{}/projects/{}/merge_requests/{iid}/notes?sort=asc&order_by=created_at&per_page=100",
        api_base(host),
        project_id(host),
    )
}

/// The requests a reply takes: a note for anything said, and the approval endpoint for an
/// approval, which records it where GitLab counts approvals rather than only in the text.
///
/// # Errors
/// [`HostingError::Unsupported`] for requesting changes, which GitLab has no verdict for on a
/// merge request.
pub fn reply_requests(
    host: &Host,
    iid: u64,
    reply: &Reply,
) -> Result<Vec<(String, serde_json::Value)>, HostingError> {
    let base = format!(
        "{}/projects/{}/merge_requests/{iid}",
        api_base(host),
        project_id(host)
    );
    let note = |body: &str| (format!("{base}/notes"), serde_json::json!({ "body": body }));
    match reply {
        Reply::Comment { body } => Ok(vec![note(body)]),
        Reply::Approve { body } => {
            let mut out = vec![(format!("{base}/approve"), serde_json::json!({}))];
            if !body.trim().is_empty() {
                out.push(note(body));
            }
            Ok(out)
        }
        Reply::RequestChanges { .. } => Err(HostingError::Unsupported(
            "GitLab has no way to request changes on a merge request; say what needs changing \
             in a comment"
                .to_owned(),
        )),
    }
}

#[derive(Deserialize)]
struct WireNote {
    #[serde(default)]
    author: Option<Author>,
    #[serde(default)]
    body: String,
    created_at: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    position: Option<WirePosition>,
}

#[derive(Deserialize)]
struct WirePosition {
    #[serde(default)]
    new_path: Option<String>,
    #[serde(default)]
    new_line: Option<u64>,
    #[serde(default)]
    old_path: Option<String>,
    #[serde(default)]
    old_line: Option<u64>,
}

/// Parses a merge request's notes.
///
/// System notes are GitLab's record of events — pushes, label changes, assignments — and
/// only an approval among them is something a reviewer said. A line comment on a removed line
/// has only the old side of its position.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not the documented array.
pub fn parse_notes(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let raw: Vec<WireNote> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw
        .into_iter()
        .filter_map(|n| {
            let author = n
                .author
                .map_or_else(|| "(unknown)".to_owned(), |a| a.username);
            if n.system {
                return (n.body.trim() == "approved this merge request").then(|| Remark {
                    author,
                    body: String::new(),
                    at: n.created_at,
                    verdict: Some(Verdict::Approved),
                    path: None,
                    line: None,
                });
            }
            let (path, line) = n.position.map_or((None, None), |p| {
                (p.new_path.or(p.old_path), p.new_line.or(p.old_line))
            });
            Some(Remark {
                author,
                body: n.body,
                at: n.created_at,
                verdict: None,
                path,
                line,
            })
        })
        .collect())
}
