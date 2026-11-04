use secrecy::SecretString;

use crate::checks::{self, Checks};
use crate::provider::{Host, HostKind, HostingError};
use crate::pull_request::{NewPullRequest, PullRequest};
use crate::review::{self, Remark, Reply};
use crate::token::{Auth, auth};
use crate::{azure, bitbucket, gitea, github, gitlab};

/// How many proposals to ask for at once.
///
/// One page. A repository with thousands of open requests is not one anybody scrolls through
/// in a sidebar, and paging the rest in would put a second round trip on a panel that is only
/// ever a convenience.
const PER_PAGE: u32 = 50;

/// Talks to a host's REST API.
///
/// Five providers, dispatched on the host rather than behind a trait: they are a closed set,
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
            HostKind::Gitea => gitea::pulls_url(host, PER_PAGE),
            HostKind::Bitbucket => bitbucket::pulls_url(host, PER_PAGE),
            HostKind::Azure => azure::pulls_url(host, PER_PAGE),
        };
        let body = self.get(host, &url).await?;
        match host.kind {
            HostKind::GitHub => github::parse_pulls(&body),
            HostKind::GitLab => gitlab::parse_merge_requests(&body),
            HostKind::Gitea => gitea::parse_pulls(&body),
            HostKind::Bitbucket => bitbucket::parse_pulls(&body),
            HostKind::Azure => azure::parse_pulls(host, &body),
        }
    }

    /// Whether the host will answer for this repository with this token.
    ///
    /// For a caller about to keep the token: a secret stored without being tried is one the
    /// window then reports as working, and the sign-in dialog said "Signed in" about a string
    /// the host had never seen. Listing the proposals is the check, because that is the whole
    /// of what the token is for here.
    ///
    /// Only a refusal from the host counts against a token. A network that is down is no
    /// evidence about the secret, and refusing it would mean nobody can sign in until they are
    /// online.
    ///
    /// # Errors
    /// [`HostingError::Api`] when the host refuses the request.
    pub async fn check(&self, host: &Host) -> Result<(), HostingError> {
        match self.pull_requests(host).await {
            Err(e) if e.is_refusal() => Err(e),
            Ok(_) | Err(_) => Ok(()),
        }
    }

    /// Whether a proposal's tip commit passes its checks: GitHub's and Gitea's by the commit,
    /// the rest by the request — GitLab's pipelines are not always run on the tip itself, and
    /// Bitbucket and Azure DevOps keep their statuses on the request.
    ///
    /// # Errors
    /// [`HostingError::Malformed`] for a `sha` that is not a commit id where it would go in a
    /// URL; otherwise as [`Client::pull_requests`].
    pub async fn checks(
        &self,
        host: &Host,
        number: u64,
        sha: &str,
    ) -> Result<Checks, HostingError> {
        let by_commit = matches!(host.kind, HostKind::GitHub | HostKind::Gitea);
        if by_commit && !checks::is_commit_id(sha) {
            return Err(HostingError::Malformed(format!("{sha} is not a commit id")));
        }
        match host.kind {
            HostKind::GitHub => {
                let runs = self.get(host, &github::check_runs_url(host, sha)).await?;
                let statuses = self.get(host, &github::statuses_url(host, sha)).await?;
                checks::parse_github(&runs, &statuses)
            }
            HostKind::GitLab => {
                let body = self.get(host, &gitlab::pipelines_url(host, number)).await?;
                checks::parse_gitlab(&body)
            }
            HostKind::Gitea => {
                checks::parse_gitea(&self.get(host, &gitea::status_url(host, sha)).await?)
            }
            HostKind::Bitbucket => checks::parse_bitbucket(
                &self
                    .get(host, &bitbucket::statuses_url(host, number))
                    .await?,
            ),
            HostKind::Azure => {
                checks::parse_azure(&self.get(host, &azure::statuses_url(host, number)).await?)
            }
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
            HostKind::Gitea => (gitea::create_url(host), gitea::create_body(new)),
            HostKind::Bitbucket => (bitbucket::create_url(host), bitbucket::create_body(new)),
            HostKind::Azure => (azure::create_url(host), azure::create_body(new)),
        };
        let body = self.post(host, &url, &payload).await?;
        match host.kind {
            HostKind::GitHub => github::parse_one(&body),
            HostKind::GitLab => gitlab::parse_one(&body),
            HostKind::Gitea => gitea::parse_one(&body),
            HostKind::Bitbucket => bitbucket::parse_one(&body),
            HostKind::Azure => azure::parse_one(host, &body),
        }
    }

    /// Everything said on a proposal, oldest first.
    ///
    /// # Errors
    /// As [`Client::pull_requests`].
    pub async fn review(&self, host: &Host, number: u64) -> Result<Vec<Remark>, HostingError> {
        let remarks = match host.kind {
            HostKind::GitHub => {
                let talk = self
                    .get(host, &github::conversation_url(host, number))
                    .await?;
                let lines = self
                    .get(host, &github::line_comments_url(host, number))
                    .await?;
                let verdicts = self.get(host, &github::reviews_url(host, number)).await?;
                let mut all = github::parse_comments(&talk)?;
                all.extend(github::parse_comments(&lines)?);
                all.extend(github::parse_reviews(&verdicts)?);
                all
            }
            HostKind::GitLab => {
                let notes = self.get(host, &gitlab::notes_url(host, number)).await?;
                gitlab::parse_notes(&notes)?
            }
            HostKind::Gitea => {
                let talk = self.get(host, &gitea::comments_url(host, number)).await?;
                let reviews = self.get(host, &gitea::reviews_url(host, number)).await?;
                let mut all = gitea::parse_comments(&talk)?;
                all.extend(gitea::parse_reviews(&reviews)?);
                all
            }
            HostKind::Bitbucket => {
                let talk = self
                    .get(host, &bitbucket::comments_url(host, number))
                    .await?;
                let one = self.get(host, &bitbucket::one_url(host, number)).await?;
                let mut all = bitbucket::parse_comments(&talk)?;
                all.extend(bitbucket::parse_verdicts(&one)?);
                all
            }
            HostKind::Azure => {
                let threads = self.get(host, &azure::threads_url(host, number)).await?;
                let one = self.get(host, &azure::one_url(host, number)).await?;
                let mut all = azure::parse_threads(&threads)?;
                all.extend(azure::parse_votes(&one)?);
                all
            }
        };
        Ok(review::in_order(remarks))
    }

    /// Comments on a proposal, approves it, or asks for changes.
    ///
    /// # Errors
    /// [`HostingError::Unsupported`] for asking GitLab for changes; otherwise as
    /// [`Client::pull_requests`].
    pub async fn reply(&self, host: &Host, number: u64, reply: &Reply) -> Result<(), HostingError> {
        let requests = match host.kind {
            HostKind::GitHub => vec![github::reply_request(host, number, reply)],
            HostKind::GitLab => gitlab::reply_requests(host, number, reply)?,
            HostKind::Gitea => vec![gitea::reply_request(host, number, reply)],
            HostKind::Bitbucket => bitbucket::reply_requests(host, number, reply),
            HostKind::Azure => return self.reply_on_azure(host, number, reply).await,
        };
        for (url, payload) in requests {
            self.post(host, &url, &payload).await?;
        }
        Ok(())
    }

    /// A vote, cast as whoever the token belongs to, and the words in a thread of their own,
    /// since a vote on Azure DevOps carries none.
    async fn reply_on_azure(
        &self,
        host: &Host,
        number: u64,
        reply: &Reply,
    ) -> Result<(), HostingError> {
        if let Some(vote) = azure::vote_of(reply) {
            let me = azure::parse_connection(&self.get(host, &azure::connection_url(host)).await?)?;
            self.put(
                host,
                &azure::vote_url(host, number, &me),
                &serde_json::json!({ "vote": vote }),
            )
            .await?;
        }
        let (Reply::Comment { body } | Reply::Approve { body } | Reply::RequestChanges { body }) =
            reply;
        if matches!(reply, Reply::Comment { .. }) || !body.trim().is_empty() {
            self.post(
                host,
                &azure::threads_url(host, number),
                &azure::comment_body(body),
            )
            .await?;
        }
        Ok(())
    }

    async fn get(&self, host: &Host, url: &str) -> Result<Vec<u8>, HostingError> {
        let response = signed(self.http.get(url), host, &self.token)
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
        let response = signed(self.http.post(url), host, &self.token)
            .header("Accept", "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|e| HostingError::Transport(e.to_string()))?;
        read(response).await
    }
}

/// A request carrying the token the way the host wants it.
fn signed(
    request: reqwest::RequestBuilder,
    host: &Host,
    token: &SecretString,
) -> reqwest::RequestBuilder {
    match auth(host.kind, token) {
        Auth::Header(name, value) => request.header(name, value),
        Auth::Basic { user, password } => request.basic_auth(user, Some(password)),
    }
}

impl Client {
    async fn put(
        &self,
        host: &Host,
        url: &str,
        payload: &serde_json::Value,
    ) -> Result<Vec<u8>, HostingError> {
        let response = signed(self.http.put(url), host, &self.token)
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
    // `message` on GitHub, GitLab, Gitea and Azure DevOps; Bitbucket nests it under `error`.
    let message = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("message")
                .or_else(|| v.pointer("/error/message"))
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
