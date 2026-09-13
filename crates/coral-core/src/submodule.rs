use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bstr::ByteSlice;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;

/// A submodule declared in `.gitmodules`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Submodule {
    /// The `.gitmodules` subsection name, which need not match the path.
    pub name: String,
    /// Slash-separated and relative to the worktree root, as git records it.
    pub path: String,
    pub url: String,
    /// The commit the superproject pins, absent when the path is not a gitlink in the index.
    pub pinned: Option<String>,
    /// True once the submodule has been cloned; only then can it be opened.
    pub initialised: bool,
}

impl RepoLocation {
    /// Lists the repository's submodules.
    ///
    /// Built from `.gitmodules` and the index rather than `git submodule status`, whose output
    /// puts an optional ` (describe)` suffix after an unquoted path and so cannot be parsed
    /// unambiguously for a path containing a space. Both reads here are NUL-delimited.
    ///
    /// # Errors
    /// Propagates a git failure other than the absent-or-empty `.gitmodules` that a repository
    /// without submodules — the overwhelming majority — presents.
    pub async fn submodules(&self, runner: &GitRunner) -> Result<Vec<Submodule>, CoralError> {
        let Some(workdir) = self.workdir.clone() else {
            // A bare repository has nowhere to check a submodule out to.
            return Ok(Vec::new());
        };

        let cmd = GitCommand::read("submodule-config", workdir.clone()).args([
            "config",
            "-f",
            ".gitmodules",
            "-z",
            "--get-regexp",
            r"^submodule\..*\.(path|url)$",
        ]);
        let out = match runner.output(cmd).await {
            Ok(out) => out,
            // `git config` exits 1 when nothing matches and 128 when the file is missing.
            Err(CoralError::GitExit { code: 1 | 128, .. }) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let mut subs = parse_gitmodules(&out.stdout);
        if subs.is_empty() {
            return Ok(Vec::new());
        }

        let pinned = self.pinned_commits(runner, &workdir, &subs).await?;
        for s in &mut subs {
            s.pinned = pinned.get(&s.path).cloned();
            s.initialised = workdir.join(&s.path).join(".git").exists();
        }
        Ok(subs)
    }

    /// The commit each submodule path is pinned to in the index.
    ///
    /// Scoped to the submodule paths: an unrestricted `ls-files` walks the whole index, which
    /// on a worktree the size of the kernel's is 96k entries for at most a handful of answers.
    async fn pinned_commits(
        &self,
        runner: &GitRunner,
        workdir: &Path,
        subs: &[Submodule],
    ) -> Result<BTreeMap<String, String>, CoralError> {
        let mut cmd = GitCommand::read("submodule-pins", workdir.to_path_buf())
            .args(["ls-files", "-z", "--stage", "--"]);
        for s in subs {
            cmd = cmd.arg(&s.path);
        }
        let out = runner.output(cmd).await?;
        Ok(parse_gitlinks(&out.stdout))
    }
}

/// Parses `git config -z --get-regexp` output, whose records are `key\nvalue\0`.
///
/// A subsection name may itself contain dots, so the name is what remains after the known
/// `submodule.` prefix and `.path` or `.url` suffix are removed, not the second dot-field.
#[must_use]
pub fn parse_gitmodules(bytes: &[u8]) -> Vec<Submodule> {
    let mut found: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();

    for record in bytes.split_str("\0") {
        if record.is_empty() {
            continue;
        }
        let Some((key, value)) = record.split_once_str("\n") else {
            continue;
        };
        let (Ok(key), Ok(value)) = (key.to_str(), value.to_str()) else {
            continue;
        };
        let Some(rest) = key.strip_prefix("submodule.") else {
            continue;
        };
        let entry = if let Some(name) = rest.strip_suffix(".path") {
            found.entry(name.to_owned()).or_default().0 = Some(value.to_owned());
            continue;
        } else if let Some(name) = rest.strip_suffix(".url") {
            name
        } else {
            continue;
        };
        found.entry(entry.to_owned()).or_default().1 = Some(value.to_owned());
    }

    found
        .into_iter()
        .filter_map(|(name, (path, url))| {
            // Without a path there is nothing to show or open; the url may legitimately be
            // absent for a submodule the superproject expects to be provided some other way.
            Some(Submodule {
                name,
                path: path?,
                url: url.unwrap_or_default(),
                pinned: None,
                initialised: false,
            })
        })
        .collect()
}

/// Picks the gitlinks out of `git ls-files -z --stage`, whose records are `mode sha stage\tpath`.
#[must_use]
pub fn parse_gitlinks(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for record in bytes.split_str("\0") {
        let Some(rest) = record.strip_prefix(b"160000 ") else {
            continue;
        };
        let Some((sha, tail)) = rest.split_once_str(" ") else {
            continue;
        };
        let Some((_stage, path)) = tail.split_once_str("\t") else {
            continue;
        };
        let (Ok(sha), Ok(path)) = (sha.to_str(), path.to_str()) else {
            continue;
        };
        out.insert(path.to_owned(), sha.to_owned());
    }
    out
}

/// A submodule's recorded commit, read from inside the submodule itself.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleRevision {
    pub oid: String,
    pub summary: String,
    /// Seconds since the epoch, as git reports author time.
    pub time: i64,
    /// True when the working copy sits exactly where the superproject pins it.
    pub in_sync: bool,
}

impl RepoLocation {
    /// The commit a submodule is pinned at, described.
    ///
    /// Read from inside the submodule, because that is the only place the commit's message
    /// lives — the superproject records an object id and nothing else.
    ///
    /// # Errors
    /// Propagates git failures. Returns `Ok(None)` for a submodule with no working copy, which
    /// has no object store to read the message out of.
    pub async fn submodule_revision(
        &self,
        runner: &GitRunner,
        path: &str,
    ) -> Result<Option<SubmoduleRevision>, CoralError> {
        let Some(workdir) = self.workdir.as_ref() else {
            return Ok(None);
        };
        let at = workdir.join(path);
        if !at.join(".git").exists() {
            return Ok(None);
        }

        let out = runner
            .output(GitCommand::read("submodule-log", &at).args([
                "log",
                "-1",
                "--format=%H%x00%s%x00%at",
                "HEAD",
            ]))
            .await?;
        let text = out.stdout.to_str_lossy();
        let mut fields = text.trim().split('\0');
        let oid = fields.next().unwrap_or_default().to_owned();
        if oid.is_empty() {
            return Ok(None);
        }
        let summary = fields.next().unwrap_or_default().to_owned();
        let time = fields
            .next()
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);

        let pinned = self
            .submodules(runner)
            .await?
            .into_iter()
            .find(|s| s.path == path)
            .and_then(|s| s.pinned);
        Ok(Some(SubmoduleRevision {
            in_sync: pinned.as_deref() == Some(oid.as_str()),
            oid,
            summary,
            time,
        }))
    }

    /// Changes where a submodule is cloned from.
    ///
    /// `set-url` writes `.gitmodules`, which is version-controlled, and `sync` copies it into
    /// the working configuration. Without the second step the new URL is committed but the
    /// next fetch still goes to the old one.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown submodule path.
    pub async fn submodule_set_url(
        &self,
        runner: &GitRunner,
        path: &str,
        url: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("submodule", self.display_path())
                    .args(["submodule", "set-url", "--"])
                    .arg(path)
                    .arg(url),
            )
            .await?;
        runner
            .output(
                GitCommand::write("submodule", self.display_path())
                    .args(["submodule", "sync", "--"])
                    .arg(path),
            )
            .await
            .map(|_| ())
    }

    /// Removes a submodule: its working copy, its configuration, and its clone.
    ///
    /// Three steps, and all three are needed. `deinit` empties the working copy and drops the
    /// entry from `.git/config`; `rm` removes the gitlink and the `.gitmodules` stanza; the
    /// clone under `.git/modules` is left by both, and leaving it behind is what makes adding a
    /// submodule back at the same path fail with "already exists in the index".
    ///
    /// # Errors
    /// Propagates git failures, including uncommitted changes inside the submodule without
    /// `force`.
    pub async fn submodule_remove(
        &self,
        runner: &GitRunner,
        path: &str,
        force: bool,
    ) -> Result<(), CoralError> {
        let mut deinit =
            GitCommand::write("submodule", self.display_path()).args(["submodule", "deinit"]);
        if force {
            deinit = deinit.arg("--force");
        }
        runner.output(deinit.arg("--").arg(path)).await?;

        let mut remove = GitCommand::write("rm", self.display_path()).arg("rm");
        if force {
            remove = remove.arg("--force");
        }
        runner.output(remove.arg("--").arg(path)).await?;

        // The clone itself, which neither of the above touches.
        let modules = self.git_path("modules");
        if let Some(module) = module_dir(&modules, path).filter(|m| m.exists()) {
            std::fs::remove_dir_all(&module)?;
        }
        Ok(())
    }

    /// Clones and checks out a submodule's working copy.
    ///
    /// `git submodule update --init` rather than a bare `clone`: the URL, the branch and the
    /// commit to check out all come from the parent's configuration and its gitlink, and
    /// cloning by hand would have to reproduce every one of them.
    ///
    /// A submodule with no path given initialises all of them, which is what the reference
    /// offers on the section itself. `remote` moves it to the tip of its configured branch
    /// rather than to the commit the superproject records.
    ///
    /// # Errors
    /// Propagates git failures, including a URL that cannot be reached.
    pub async fn submodule_init(
        &self,
        runner: &GitRunner,
        path: Option<&str>,
        recursive: bool,
        remote: bool,
    ) -> Result<(), CoralError> {
        let mut cmd = GitCommand::network("submodule", self.display_path()).args([
            "submodule",
            "update",
            "--init",
            "--progress",
        ]);
        if recursive {
            cmd = cmd.arg("--recursive");
        }
        // Without this the submodule is checked out at the commit the superproject records,
        // which is what "update" means most of the time. With it, git fetches the configured
        // branch and moves to its tip — a different operation, and one that changes what the
        // superproject will record next.
        if remote {
            cmd = cmd.arg("--remote");
        }
        if let Some(p) = path {
            // `--` so a submodule whose path starts with a dash is not read as a flag.
            cmd = cmd.arg("--").arg(p);
        }
        runner.output(cmd).await.map(|_| ())
    }
}

/// Where a submodule's own clone lives, or `None` if the path does not name somewhere under
/// `.git/modules`.
///
/// The path is read out of `.gitmodules`, which is content of the repository and so written by
/// whoever it was cloned from. `Path::join` takes an absolute argument as the whole answer and
/// keeps a `..` as a step upwards, so without this the directory about to be deleted outright
/// could be any directory at all. git refuses such a path a step earlier, and this does not
/// depend on it having done so.
fn module_dir(modules: &Path, path: &str) -> Option<PathBuf> {
    let full = modules.join(path);
    full.starts_with(modules)
        .then_some(full)
        .filter(|f| !f.components().any(|c| c == std::path::Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::{Path, PathBuf, module_dir};

    #[test]
    fn a_submodule_clone_lives_under_the_modules_directory() {
        let modules = Path::new("/r/.git/modules");
        assert_eq!(
            module_dir(modules, "vendor/lib"),
            Some(PathBuf::from("/r/.git/modules/vendor/lib"))
        );
    }

    #[test]
    fn a_path_that_climbs_out_names_nowhere() {
        let modules = Path::new("/r/.git/modules");
        // `.gitmodules` is content of the repository, so both of these are things a clone can
        // arrive carrying, and both used to name a directory outside it.
        assert_eq!(module_dir(modules, "../../../etc"), None);
        assert_eq!(module_dir(modules, "/etc"), None);
        assert_eq!(module_dir(modules, "vendor/../../.."), None);
    }
}
