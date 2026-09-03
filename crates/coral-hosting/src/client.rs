use secrecy::SecretString;

use crate::github;
use crate::gitlab;
use crate::provider::{Host, HostKind, HostingError};
use crate::pull_request::{NewPullRequest, PullRequest};
use crate::token::auth_header;

/// How many proposals to ask for at once.
///
/// One page. A repository with thousands of open requests is not one anybody scrolls through
/// in a sidebar, and paging the rest in would put a second round trip on a panel that is only
/// ever a convenience.
const PER_PAGE: u32 = 50;

/// Talks to a host's REST API.
///
/// Two providers, dispatched on the host rather than behind a trait: they are a closed set,
/// and every difference between them — the API root, the auth header, whether a draft is a
/// field or a title prefix — is one match arm wide.
pub struct Client {
    http: reqwest::Client,
    token: SecretString,
}

impl Client {
    /// Builds a client for one host's token.
    ///
    /// # Errors
    /// [`HostingError::Transport`] if the HTTP stack cannot be constructed.
    pub fn new(token: SecretString) -> Result<Self, HostingError> {
        let http = reqwest::Client::builder()
            // A hosting call is never on the graph's critical path, but it must not hang a
            // panel forever either.
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(concat!("coral/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        Ok(Self { http, token })
    }

    /// Lists the repository's open and recent proposals, newest first.
    ///
    /// # Errors
    /// [`HostingError::Transport`] for a network failure, [`HostingError::Api`] for a refusal,
    /// [`HostingError::Malformed`] if the body is not what the endpoint documents.
    pub async fn pull_requests(&self, host: &Host) -> Result<Vec<PullRequest>, HostingError> {
        let url = match host.kind {
            HostKind::GitHub => github::pulls_url(host, PER_PAGE),
            HostKind::GitLab => gitlab::merge_requests_url(host, PER_PAGE),
        };
        let body = self.get(host, &url).await?;
        match host.kind {
            HostKind::GitHub => github::parse_pulls(&body),
            HostKind::GitLab => gitlab::parse_merge_requests(&body),
        }
    }

    /// Opens a new proposal.
    ///
    /// # Errors
    /// As [`Client::pull_requests`].
    pub async fn create(
        &self,
        host: &Host,
        new: &NewPullRequest,
    ) -> Result<PullRequest, HostingError> {
        let (url, payload) = match host.kind {
            HostKind::GitHub => (github::create_url(host), github::create_body(new)),
            HostKind::GitLab => (gitlab::create_url(host), gitlab::create_body(new)),
        };
        let body = self.post(host, &url, &payload).await?;
        match host.kind {
            HostKind::GitHub => github::parse_one(&body),
            HostKind::GitLab => gitlab::parse_one(&body),
        }
    }

    async fn get(&self, host: &Host, url: &str) -> Result<Vec<u8>, HostingError> {
        let (header, value) = auth_header(host.kind, &self.token);
        let response = self
            .http
            .get(url)
            .header(header, value)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        read(response).await
    }

    async fn post(
        &self,
        host: &Host,
        url: &str,
        payload: &serde_json::Value,
    ) -> Result<Vec<u8>, HostingError> {
        let (header, value) = auth_header(host.kind, &self.token);
        let response = self
            .http
            .post(url)
            .header(header, value)
            .header("Accept", "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        read(response).await
    }
}

async fn read(response: reqwest::Response) -> Result<Vec<u8>, HostingError> {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|e| HostingError::Transport(e.to_string()))?;
    if status.is_success() {
        return Ok(body.to_vec());
    }
    Err(HostingError::Api {
        status: status.as_u16(),
        detail: describe_failure(status.as_u16(), &body),
    })
}

/// Turns a refusal into something worth showing.
///
/// Both providers put a sentence in a `message` field; without it the user is left with a bare
/// status code for what is nearly always an expired or under-scoped token.
#[must_use]
pub fn describe_failure(status: u16, body: &[u8]) -> String {
    let message = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("message")
                .or_else(|| v.get("error"))
                .and_then(|m| m.as_str().map(str::to_owned))
        });

    match (status, message) {
        (401, _) => "the token was rejected; it may have expired".to_owned(),
        (403, Some(m)) if m.contains("rate limit") => m,
        (403, _) => "the token does not have access to this repository".to_owned(),
        (404, _) => "no such repository, or the token cannot see it".to_owned(),
        (_, Some(m)) => m,
        (_, None) => format!("the host answered {status}"),
    }
}
