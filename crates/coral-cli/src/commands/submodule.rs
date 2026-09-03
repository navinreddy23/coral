use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::submodule::Submodule;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleList {
    pub submodules: Vec<Submodule>,
}

/// Lists the repository's submodules.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not in a repository, or any git failure.
pub async fn run(path: &Path) -> Result<SubmoduleList, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    Ok(SubmoduleList {
        submodules: loc.submodules(&runner).await?,
    })
}

impl crate::output::Human for SubmoduleList {
    fn human(&self) -> String {
        if self.submodules.is_empty() {
            return "no submodules".to_owned();
        }
        let mut out = format!("{} submodules", self.submodules.len());
        for s in &self.submodules {
            // The leading character matches `git submodule status`: a space for a submodule
            // that is checked out, a minus for one that has not been initialised.
            let mark = if s.initialised { ' ' } else { '-' };
            let pinned = s.pinned.as_deref().unwrap_or("(not in index)");
            let _ = write!(out, "\n {mark}{pinned:.8}  {:<40}{}", s.path, s.url);
        }
        out
    }
}
