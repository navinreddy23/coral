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

/// Diffs the worktree against the index, the index against HEAD when `staged`, or the worktree
/// against a commit when `since` names one.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(
    path: &Path,
    staged: bool,
    since: Option<&str>,
    paths: &[String],
) -> Result<DiffSet, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let files = match since {
        Some(rev) => {
            loc.diff_since(&runner, rev, &refs, DiffOptions::default())
                .await?
        }
        None => {
            loc.diff(&runner, staged, &refs, DiffOptions::default())
                .await?
        }
    };
    Ok(DiffSet { files })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compared {
    pub from: String,
    pub to: String,
    /// Where the two parted, when the comparison is measured from there.
    pub base: Option<String>,
    pub files: Vec<coral_core::commit::ChangedFile>,
}

/// The files that differ between two revisions: everything between their tips, or only what
/// `to` gained since the two parted when `merge_base`.
///
/// # Errors
/// [`CoralError::Refused`] for a merge-base comparison of histories that never met, or any git
/// failure.
pub async fn compare(
    path: &Path,
    from: &str,
    to: &str,
    merge_base: bool,
) -> Result<Compared, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let base = if merge_base {
        Some(
            loc.merge_base(&runner, from, to)
                .await?
                .ok_or_else(|| CoralError::Refused {
                    label: "compare from where they parted",
                    detail: format!("{from} and {to} share no history"),
                })?,
        )
    } else {
        None
    };
    let files = loc
        .compare(&runner, base.as_deref().unwrap_or(from), to)
        .await?;
    Ok(Compared {
        from: from.to_owned(),
        to: to.to_owned(),
        base,
        files,
    })
}

impl crate::output::Human for Compared {
    fn human(&self) -> String {
        let since = match &self.base {
            Some(base) => format!(" since they parted at {}", base.get(..12).unwrap_or(base)),
            None => String::new(),
        };
        let mut out = format!(
            "{} files differ from {} to {}{since}",
            self.files.len(),
            self.from,
            self.to
        );
        for f in &self.files {
            let _ = write!(
                out,
                "\n  {:<10} {}",
                format!("{:?}", f.change).to_lowercase(),
                f.path
            );
        }
        out
    }
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
