use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::grep::{GrepQuery, GrepResult};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

/// Searches the files of one revision, as the window's code search does.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure,
/// which includes a pattern git cannot read.
pub async fn run(path: &Path, rev: &str, query: &GrepQuery) -> Result<GrepResult, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.grep(&runner, rev, query).await
}

impl crate::output::Human for GrepResult {
    fn human(&self) -> String {
        if self.matches.is_empty() {
            return "no matches".to_owned();
        }
        let more = if self.more { ", and more" } else { "" };
        let mut out = format!("{} lines{more}", self.matches.len());
        for m in &self.matches {
            let _ = write!(out, "\n  {}:{}:{}: {}", m.path, m.line, m.column, m.text);
        }
        out
    }
}
