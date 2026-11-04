//! Azure DevOps Services: a repository inside a project inside an organisation, every call
//! carrying `api-version`, pull requests by `pullRequestId`, and reviews as threads plus
//! reviewers' votes.
//! <https://learn.microsoft.com/en-us/rest/api/azure/devops/git/pull-requests>

use serde::Deserialize;

use crate::provider::{Host, HostingError};
use crate::pull_request::{NewPullRequest, PrState, PullRequest};
use crate::review::{Remark, Reply, Verdict};

const VERSION: &str = "api-version=7.1";

/// A reviewer's vote for approving, as Azure DevOps counts them.
pub const APPROVE: i64 = 10;
/// "Waiting for author", which is what asking for changes is here.
pub const WAIT_FOR_AUTHOR: i64 = -5;

#[must_use]
pub fn api_base(host: &Host) -> String {
    format!(
        "{}/{}/_apis/git/repositories/{}",
        host.origin, host.owner, host.repo
    )
}

#[must_use]
pub fn pulls_url(host: &Host, per_page: u32) -> String {
    format!(
        "{}/pullrequests?searchCriteria.status=all&$top={per_page}&{VERSION}",
        api_base(host)
    )
}

#[must_use]
pub fn create_url(host: &Host) -> String {
    format!("{}/pullrequests?{VERSION}", api_base(host))
}

#[must_use]
pub fn one_url(host: &Host, id: u64) -> String {
    format!("{}/pullrequests/{id}?{VERSION}", api_base(host))
}

#[must_use]
pub fn statuses_url(host: &Host, id: u64) -> String {
    format!("{}/pullrequests/{id}/statuses?{VERSION}", api_base(host))
}

#[must_use]
pub fn threads_url(host: &Host, id: u64) -> String {
    format!("{}/pullrequests/{id}/threads?{VERSION}", api_base(host))
}

/// Who the token belongs to, which a vote is cast as.
///
/// At the organisation, which is the first part of the owner on `dev.azure.com` and the host
/// itself on a `visualstudio.com` address.
#[must_use]
pub fn connection_url(host: &Host) -> String {
    let org = if host.origin.ends_with(".visualstudio.com") {
        host.origin.clone()
    } else {
        let name = host.owner.split('/').next().unwrap_or_default();
        format!("{}/{name}", host.origin)
    };
    format!("{org}/_apis/connectionData")
}

#[must_use]
pub fn vote_url(host: &Host, id: u64, reviewer: &str) -> String {
    format!(
        "{}/pullrequests/{id}/reviewers/{reviewer}?{VERSION}",
        api_base(host)
    )
}

/// One work item, which is what an issue is on Azure DevOps: `AB#123` in a message.
#[must_use]
pub fn work_item_url(host: &Host, id: u64) -> String {
    format!(
        "{}/{}/_apis/wit/workitems/{id}?{VERSION}",
        host.origin, host.owner
    )
}

#[derive(Deserialize)]
struct WireWorkItem {
    id: u64,
    fields: WorkFields,
}

#[derive(Deserialize)]
struct WorkFields {
    #[serde(rename = "System.Title", default)]
    title: String,
    #[serde(rename = "System.State", default)]
    state: String,
}

/// Parses one work item. Its states are the process's own; done, closed, resolved and removed
/// are the ones that mean nothing is left.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a work item.
pub fn parse_work_item(host: &Host, body: &[u8]) -> Result<crate::issue::Issue, HostingError> {
    let w: WireWorkItem =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    let done = matches!(
        w.fields.state.to_ascii_lowercase().as_str(),
        "done" | "closed" | "resolved" | "removed"
    );
    Ok(crate::issue::Issue {
        number: w.id,
        web_url: format!("{}/{}/_workitems/edit/{}", host.origin, host.owner, w.id),
        title: w.fields.title,
        state: if done {
            crate::issue::IssueState::Closed
        } else {
            crate::issue::IssueState::Open
        },
    })
}

#[must_use]
pub fn create_body(new: &NewPullRequest) -> serde_json::Value {
    serde_json::json!({
        "sourceRefName": format!("refs/heads/{}", new.source_branch),
        "targetRefName": format!("refs/heads/{}", new.target_branch),
        "title": new.title,
        "description": new.body,
        "isDraft": new.draft,
    })
}

/// A new thread holding one comment, which is what a comment on the whole request is.
#[must_use]
pub fn comment_body(body: &str) -> serde_json::Value {
    serde_json::json!({
        "comments": [{ "parentCommentId": 0, "content": body, "commentType": 1 }],
        "status": 1,
    })
}

/// The vote a reply casts, or none for a plain comment.
#[must_use]
pub fn vote_of(reply: &Reply) -> Option<i64> {
    match reply {
        Reply::Comment { .. } => None,
        Reply::Approve { .. } => Some(APPROVE),
        Reply::RequestChanges { .. } => Some(WAIT_FOR_AUTHOR),
    }
}

#[derive(Deserialize)]
struct List<T> {
    #[serde(default = "Vec::new")]
    value: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitRef {
    commit_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reviewer {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    vote: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wire {
    pull_request_id: u64,
    title: String,
    status: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    created_by: Option<Identity>,
    source_ref_name: String,
    target_ref_name: String,
    #[serde(default)]
    last_merge_source_commit: Option<CommitRef>,
    creation_date: String,
    #[serde(default)]
    closed_date: Option<String>,
    #[serde(default)]
    reviewers: Vec<Reviewer>,
}

fn short(name: &str) -> String {
    name.strip_prefix("refs/heads/").unwrap_or(name).to_owned()
}

fn convert(host: &Host, w: Wire) -> PullRequest {
    let state = match w.status.as_str() {
        "completed" => PrState::Merged,
        "abandoned" => PrState::Closed,
        _ if w.is_draft => PrState::Draft,
        _ => PrState::Open,
    };
    PullRequest {
        number: w.pull_request_id,
        web_url: format!(
            "{}/{}/_git/{}/pullrequest/{}",
            host.origin, host.owner, host.repo, w.pull_request_id
        ),
        title: w.title,
        state,
        author: w
            .created_by
            .map_or_else(|| "(unknown)".to_owned(), |c| c.display_name),
        head_sha: w
            .last_merge_source_commit
            .map(|c| c.commit_id)
            .unwrap_or_default(),
        source_branch: short(&w.source_ref_name),
        target_branch: short(&w.target_ref_name),
        // No "updated" date is kept; closing is the last thing that happens to one.
        updated_at: w.closed_date.unwrap_or(w.creation_date),
    }
}

/// Parses the list of pull requests.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not one.
pub fn parse_pulls(host: &Host, body: &[u8]) -> Result<Vec<PullRequest>, HostingError> {
    let list: List<Wire> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(list.value.into_iter().map(|w| convert(host, w)).collect())
}

/// Parses one pull request.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not one.
pub fn parse_one(host: &Host, body: &[u8]) -> Result<PullRequest, HostingError> {
    let raw: Wire =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(convert(host, raw))
}

/// The votes cast on a pull request, as verdicts. Approving with suggestions is approving.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a pull request.
pub fn parse_votes(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let raw: Wire =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(raw
        .reviewers
        .into_iter()
        .filter_map(|r| {
            let verdict = match r.vote {
                v if v > 0 => Verdict::Approved,
                v if v < 0 => Verdict::ChangesRequested,
                _ => return None,
            };
            Some(Remark {
                author: r.display_name,
                body: String::new(),
                at: raw.closed_date.clone().unwrap_or_default(),
                verdict: Some(verdict),
                path: None,
                line: None,
            })
        })
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Position {
    line: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    file_path: String,
    #[serde(default)]
    right_file_start: Option<Position>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireComment {
    #[serde(default)]
    author: Option<Identity>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    published_date: String,
    #[serde(default)]
    comment_type: String,
    #[serde(default)]
    is_deleted: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Thread {
    #[serde(default)]
    comments: Vec<WireComment>,
    #[serde(default, rename = "threadContext")]
    context: Option<Context>,
    #[serde(default)]
    is_deleted: bool,
}

/// Every comment in every thread, leaving out what Azure DevOps writes itself — "Ada voted
/// 10", "updated the pull request" — which is the system's diary, not anybody's review.
///
/// # Errors
/// [`HostingError::Malformed`] when the body is not a list of threads.
pub fn parse_threads(body: &[u8]) -> Result<Vec<Remark>, HostingError> {
    let list: List<Thread> =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    let mut out = Vec::new();
    for thread in list.value.into_iter().filter(|t| !t.is_deleted) {
        let (path, line) = thread.context.map_or((None, None), |c| {
            (Some(c.file_path), c.right_file_start.map(|p| p.line))
        });
        for c in thread.comments {
            if c.is_deleted || c.comment_type == "system" {
                continue;
            }
            out.push(Remark {
                author: c.author.map(|a| a.display_name).unwrap_or_default(),
                body: c.content,
                at: c.published_date,
                verdict: None,
                path: path.clone(),
                line,
            });
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    authenticated_user: Who,
}

#[derive(Deserialize)]
struct Who {
    id: String,
}

/// The id of whoever the token belongs to.
///
/// # Errors
/// [`HostingError::Malformed`] when the body does not say.
pub fn parse_connection(body: &[u8]) -> Result<String, HostingError> {
    let c: Connection =
        serde_json::from_slice(body).map_err(|e| HostingError::Malformed(e.to_string()))?;
    Ok(c.authenticated_user.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        Host::detect_with("https://dev.azure.com/acme/Platform/_git/coral", |_| None).unwrap()
    }

    #[test]
    fn a_list_of_pull_requests_is_read_with_its_web_page_built() {
        let body = br#"{"count":1,"value":[{"pullRequestId":12,"title":"Tidy","status":"active","isDraft":false,
          "createdBy":{"displayName":"Ada"},"sourceRefName":"refs/heads/tidy","targetRefName":"refs/heads/main",
          "lastMergeSourceCommit":{"commitId":"0123456789012345678901234567890123456789"},"creationDate":"2026-11-01T10:00:00Z"}]}"#;
        let pulls = parse_pulls(&host(), body).unwrap();
        assert_eq!(pulls[0].number, 12);
        assert_eq!(pulls[0].source_branch, "tidy");
        assert_eq!(
            pulls[0].web_url,
            "https://dev.azure.com/acme/Platform/_git/coral/pullrequest/12"
        );
        assert_eq!(
            pulls_url(&host(), 50),
            "https://dev.azure.com/acme/Platform/_apis/git/repositories/coral/pullrequests?searchCriteria.status=all&$top=50&api-version=7.1"
        );
        assert_eq!(
            connection_url(&host()),
            "https://dev.azure.com/acme/_apis/connectionData"
        );
    }

    #[test]
    fn threads_leave_out_the_system_diary_and_votes_become_verdicts() {
        let threads = br#"{"value":[
          {"comments":[{"author":{"displayName":"Ada"},"content":"Why?","publishedDate":"2026-11-01T10:00:00Z","commentType":"text"}],
           "threadContext":{"filePath":"/src/a.rs","rightFileStart":{"line":4,"offset":1}}},
          {"comments":[{"author":{"displayName":"Azure"},"content":"Ada voted 10","commentType":"system"}]}]}"#;
        let said = parse_threads(threads).unwrap();
        assert_eq!(said.len(), 1);
        assert_eq!(
            (said[0].path.as_deref(), said[0].line),
            (Some("/src/a.rs"), Some(4))
        );

        let votes = br#"{"pullRequestId":12,"title":"t","status":"active","sourceRefName":"refs/heads/a","targetRefName":"refs/heads/main",
          "creationDate":"x","reviewers":[{"displayName":"Bob","vote":5},{"displayName":"Cy","vote":-10},{"displayName":"Di","vote":0}]}"#;
        let verdicts = parse_votes(votes).unwrap();
        assert_eq!(verdicts.len(), 2);
        assert_eq!(verdicts[1].verdict, Some(Verdict::ChangesRequested));
        assert_eq!(
            vote_of(&Reply::Approve {
                body: String::new()
            }),
            Some(APPROVE)
        );
    }
}
