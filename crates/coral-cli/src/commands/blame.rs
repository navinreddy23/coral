use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::blame::Blame;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;

/// Attributes each line of a file to the commit that last changed it.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path, rev: &str, file: &str) -> Result<Blame, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.blame(&runner, rev, file).await
}

impl crate::output::Human for Blame {
    fn human(&self) -> String {
        let mut out = format!(
            "{} chunks, {} commits",
            self.chunks.len(),
            self.commits.len()
        );
        for c in &self.chunks {
            let who = self.commits.get(&c.oid);
            let name = who.map_or("", |w| w.author.name.as_str());
            let summary = who.map(|w| w.summary.to_string()).unwrap_or_default();
            let _ = write!(
                out,
                "\n  {:>6}-{:<6} {:.8}  {name:<18.18} {summary:.48}",
                c.final_line,
                c.final_line + c.lines - 1,
                c.oid
            );
        }
        out
    }
}
