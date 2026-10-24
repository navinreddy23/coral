//! A proposal's review: what people said about it, and saying something back.
//!
//! GitHub keeps three separate lists — the conversation, comments on lines of the diff, and
//! reviews with a verdict — and GitLab one list of notes with approvals among its system
//! notes. Both are folded into one list of remarks in time order, which is how a person reads
//! a review whichever host it is on.

/// What a review concluded, when it concluded anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Approved,
    ChangesRequested,
}

/// One thing somebody said on a proposal.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Remark {
    pub author: String,
    /// Markdown, as written. Empty for a bare approval.
    pub body: String,
    /// ISO 8601, as the provider gave it.
    pub at: String,
    pub verdict: Option<Verdict>,
    /// The file and line of the diff it is about, for a comment left on one.
    pub path: Option<String>,
    pub line: Option<u64>,
}

/// What to say back.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reply {
    Comment {
        body: String,
    },
    Approve {
        body: String,
    },
    /// GitHub only: GitLab has no verdict short of approving for a merge request.
    RequestChanges {
        body: String,
    },
}

/// Puts remarks from several lists into the order they were made.
///
/// By the timestamp text, which sorts correctly because each provider writes every one of its
/// own in the same ISO 8601 form.
#[must_use]
pub fn in_order(mut remarks: Vec<Remark>) -> Vec<Remark> {
    remarks.sort_by(|a, b| a.at.cmp(&b.at));
    remarks
}
