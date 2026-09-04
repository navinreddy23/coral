use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::diff::{DiffOptions, FileDiff};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSet {
    pub files: Vec<FileDiff>,
}

/// Diffs the worktree, or the index when `staged`.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path, staged: bool, paths: &[String]) -> Result<DiffSet, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    Ok(DiffSet {
        files: loc
            .diff(&runner, staged, &refs, DiffOptions::default())
            .await?,
    })
}

impl crate::output::Human for DiffSet {
    fn human(&self) -> String {
        if self.files.is_empty() {
            return "no changes".to_owned();
        }
        let mut out = format!("{} files", self.files.len());
        for f in &self.files {
            let counts = match (f.added, f.removed) {
                (Some(a), Some(r)) => format!("+{a} -{r}"),
                _ => "binary".to_owned(),
            };
            let rename = match &f.old_path {
                Some(o) => format!(" (from {o})"),
                None => String::new(),
            };
            let large = if f.too_large { "  [too large]" } else { "" };
            let _ = write!(
                out,
                "\n  {:<10} {counts:<12} {}{rename}{large}",
                format!("{:?}", f.change).to_lowercase(),
                f.path
            );
        }
        out
    }
}
