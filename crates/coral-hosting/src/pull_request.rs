/// Where a change proposal stands.
///
/// GitHub and GitLab spell these differently — GitHub reports `state` plus a separate
/// `merged_at`, GitLab reports one of `opened`/`merged`/`closed`/`locked` — so both are mapped
/// onto this rather than the UI learning either vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Open,
    /// Open, but explicitly marked not ready for review.
    Draft,
    Merged,
    /// Closed without merging.
    Closed,
}

/// A pull request on GitHub, or a merge request on GitLab.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    /// The number people quote: `#42`. Not the provider's internal id.
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub author: String,
    /// Branch being proposed, short — `feature/x`, never `refs/heads/feature/x`.
    pub source_branch: String,
    pub target_branch: String,
    /// Where a person would open it.
    pub web_url: String,
    /// ISO 8601, as the provider gave it. Parsed at the display boundary, not here.
    pub updated_at: String,
}

/// What to open a new one with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPullRequest {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    pub draft: bool,
}
