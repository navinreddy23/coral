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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub submodule: Submodule,
}

impl crate::output::Human for Added {
    fn human(&self) -> String {
        let s = &self.submodule;
        let pinned = s.pinned.as_deref().unwrap_or("(not in index)");
        format!(
            "added {} at {pinned:.8}\ncommit .gitmodules and {} to record it",
            s.path, s.path
        )
    }
}

/// What to add: where from, where to, and how to reach it.
pub struct NewSubmodule {
    pub url: String,
    /// Derived from the URL by git when absent.
    pub path: Option<String>,
    pub branch: Option<String>,
    /// A private key of its own; absent takes the repository's.
    pub ssh_key: Option<String>,
}

/// Adds a submodule, cloned and staged, with any nested ones cloned too.
///
/// # Errors
/// Refuses a bare repository; propagates git failures, including a URL that cannot be reached.
pub async fn add(path: &Path, new: &NewSubmodule) -> Result<Added, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    let submodule = loc
        .submodule_add(
            &runner,
            &new.url,
            new.path.as_deref(),
            new.branch.as_deref(),
            new.ssh_key.as_deref(),
        )
        .await?;
    Ok(Added { submodule })
}

/// Clones the working copies of the submodules named, or of every one, and of the ones inside.
///
/// `remote` moves each to the tip of its configured branch instead, and then only at the top
/// level: carrying that into the nested ones would move them off the commits their own
/// superproject records, which nobody asked for.
///
/// # Errors
/// Propagates git failures, and refuses a path that declares no submodule.
pub async fn init(
    path: &Path,
    only: &[String],
    remote: bool,
) -> Result<crate::commands::write::Done, CoralError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, path).await?;
    loc.submodule_init(&runner, only, !remote, remote).await?;
    let which = match only {
        [] => "every submodule".to_owned(),
        [one] => one.clone(),
        many => format!("{} submodules", many.len()),
    };
    Ok(crate::commands::write::Done {
        what: if remote {
            format!("updated {which} to its branch tip")
        } else {
            format!("initialised {which}")
        },
        oid: None,
    })
}
