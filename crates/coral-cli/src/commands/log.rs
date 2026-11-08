use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::commit::Commit;
use coral_core::history::LogQuery;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub commits: Vec<Commit>,
}

/// Reads commit history.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path, q: &LogQuery) -> Result<Log, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok(Log {
        commits: loc.log(&runner, q).await?,
    })
}

/// The commits that changed some lines of one file, `FROM,TO` as written on the command line.
///
/// # Errors
/// [`CoralError::Refused`] for a range that does not read as two numbers, and whatever
/// [`RepoLocation::line_history`] refuses.
pub async fn lines(
    path: &Path,
    rev: &str,
    file: &str,
    range: &str,
    limit: u64,
) -> Result<Log, CoralError> {
    let parsed = range
        .split_once(',')
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)));
    let Some(lines) = parsed else {
        return Err(CoralError::Refused {
            label: "read the history of those lines",
            detail: format!("--lines takes FROM,TO, such as 12,20; got {range}"),
        });
    };
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok(Log {
        commits: loc.line_history(&runner, rev, file, lines, limit).await?,
    })
}

impl crate::output::Human for Log {
    fn human(&self) -> String {
        if self.commits.is_empty() {
            return "no commits".to_owned();
        }
        let mut out = format!("{} commits", self.commits.len());
        for c in &self.commits {
            let merge = if c.is_merge() { " (merge)" } else { "" };
            let _ = write!(
                out,
                "\n  {:.8}  {:<20.20}  {}{merge}",
                c.oid, c.author.name, c.summary
            );
        }
        out
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authors {
    pub authors: Vec<coral_core::authors::Person>,
}

/// Who wrote the repository's recent commits, most active first.
///
/// # Errors
/// [`CoralError::NotARepository`], or any git failure.
pub async fn authors(path: &std::path::Path) -> Result<Authors, CoralError> {
    let runner = coral_core::process::GitRunner::discover().await?;
    let loc = coral_core::repo::RepoLocation::discover(&runner, path).await?;
    Ok(Authors {
        authors: loc.recent_authors(&runner).await?,
    })
}

impl crate::output::Human for Authors {
    fn human(&self) -> String {
        self.authors
            .iter()
            .map(|p| format!("{:>5}  {} <{}>", p.commits, p.name, p.email))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
