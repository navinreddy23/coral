use bstr::BString;

/// A person and when they acted. Author and committer are shown separately in the UI, because
/// a rebased or cherry-picked commit has two different answers.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Seconds since the epoch.
    pub time: i64,
}

/// One commit's metadata, without its tree or diff.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: Signature,
    pub committer: Signature,
    /// First line of the message.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub summary: BString,
    /// Everything after the first blank line; empty when there is none.
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub body: BString,
}

impl Commit {
    #[must_use]
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// True when the commit was authored and committed by different people, or at different
    /// times — which is what makes showing both worthwhile.
    #[must_use]
    pub fn was_rewritten(&self) -> bool {
        self.author != self.committer
    }
}
