use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bstr::ByteSlice;

use crate::config::Scope;
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

/// Which ssh key reaches a submodule's host: the one it is pinned to, and the one it inherits.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleSsh {
    /// The key this submodule is pinned to on its own. `None` takes the superproject's.
    pub key: Option<String>,
    /// The superproject's own key, empty when that is the agent.
    pub inherited: String,
}

/// Where a submodule's own key is recorded, in the superproject's configuration.
///
/// The superproject rather than the submodule, because the clone that needs the key is the one
/// that does not exist yet. Keyed by name rather than path for the same reason git keys
/// `submodule.<name>.url` that way: the name is what a moved submodule keeps.
fn key_setting(name: &str) -> String {
    format!("coral.submodule.{name}.sshKey")
}

/// How the key is handed to the git that clones the submodule, or nothing to hand it.
///
/// The environment, because a submodule's git reads neither the superproject's configuration
/// nor a `-c`, which git carries in `GIT_CONFIG_PARAMETERS` and clears on the way in. Nothing
/// at all when no key is pinned: an empty `GIT_SSH_COMMAND` would shadow what the user set by
/// hand, exactly as an empty `core.sshCommand` does.
fn ssh_env(key: Option<&str>) -> Option<(&'static str, String)> {
    key.map(|k| ("GIT_SSH_COMMAND", crate::ssh::command_for(k)))
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

    /// Which key a submodule is cloned and fetched with, and which one it falls back to.
    ///
    /// # Errors
    /// Propagates git failures, and refuses a path that declares no submodule.
    pub async fn submodule_ssh(
        &self,
        runner: &GitRunner,
        path: &str,
    ) -> Result<SubmoduleSsh, CoralError> {
        let name = self.submodule_name(runner, path).await?;
        Ok(SubmoduleSsh {
            key: self.pinned_key(runner, &name).await?,
            inherited: self.own_key(runner).await?.unwrap_or_default(),
        })
    }

    /// Pins one submodule to a key of its own, or clears that so it takes the repository's.
    ///
    /// # Errors
    /// Propagates git failures, and refuses a path that declares no submodule.
    pub async fn set_submodule_ssh(
        &self,
        runner: &GitRunner,
        path: &str,
        key: Option<&str>,
    ) -> Result<(), CoralError> {
        let name = self.submodule_name(runner, path).await?;
        let key = key.map(str::trim).filter(|k| !k.is_empty());
        self.config_put(runner, Scope::Repository, &key_setting(&name), key)
            .await
    }

    async fn submodule_name(&self, runner: &GitRunner, path: &str) -> Result<String, CoralError> {
        self.submodules(runner)
            .await?
            .into_iter()
            .find(|s| s.path == path)
            .map(|s| s.name)
            .ok_or_else(|| CoralError::Refused {
                label: "submodule",
                detail: format!("{path} is not a submodule of this repository"),
            })
    }

    async fn pinned_key(
        &self,
        runner: &GitRunner,
        name: &str,
    ) -> Result<Option<String>, CoralError> {
        Ok(self
            .config_get(runner, Scope::Repository, &key_setting(name))
            .await?
            .filter(|k| !k.is_empty()))
    }

    /// The key this repository itself is pinned to, which is what a submodule inherits.
    ///
    /// `--local` alone. A key set at the app level needs no help: the submodule's own git reads
    /// the same global configuration. It is this repository's that is lost, because a submodule
    /// is a different repository and reads none of it.
    async fn own_key(&self, runner: &GitRunner) -> Result<Option<String>, CoralError> {
        Ok(self
            .config_get(runner, Scope::Repository, "core.sshCommand")
            .await?
            .as_deref()
            .and_then(crate::ssh::key_in_command))
    }

    /// The key one submodule will actually be reached with.
    async fn key_for(
        &self,
        runner: &GitRunner,
        name: &str,
        inherited: Option<&str>,
    ) -> Result<Option<String>, CoralError> {
        Ok(self
            .pinned_key(runner, name)
            .await?
            .or_else(|| inherited.map(str::to_owned)))
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
    /// **The key goes in the environment.** git runs each submodule's clone and fetch as a
    /// child process in that submodule's own configuration, so the `core.sshCommand` this
    /// repository was cloned with is not read at all, and `-c` is no better: git puts a `-c`
    /// into `GIT_CONFIG_PARAMETERS`, which it clears before entering a submodule. Measured
    /// against a command that records every time it is run: named in the superproject's local
    /// config it was used not once, and the same command in the environment was used for every
    /// attempt. Without this a private submodule is reached as whoever the agent offers first,
    /// and the host answers that the repository does not exist — the failure
    /// `docs/DECISIONS.md` describes, arriving where nothing mentions a key.
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
        let inherited = self.own_key(runner).await?;
        for (only, key) in self.update_runs(runner, path, inherited.as_deref()).await? {
            let cmd = self.update_command(only.as_deref(), recursive, remote, key.as_deref());
            runner.output(cmd).await?;
        }
        self.record_keys(runner, inherited.as_deref()).await;
        Ok(())
    }

    /// One update per key, as `(pathspec, key)`.
    ///
    /// With nothing pinned to a key of its own — which is every repository until somebody says
    /// otherwise — that is the single command git would run anyway, pathspec and all. Only a
    /// submodule given its own key splits them, because one environment cannot carry two.
    async fn update_runs(
        &self,
        runner: &GitRunner,
        path: Option<&str>,
        inherited: Option<&str>,
    ) -> Result<Vec<(Option<String>, Option<String>)>, CoralError> {
        if let Some(p) = path {
            let name = self.submodule_name(runner, p).await?;
            let key = self.key_for(runner, &name, inherited).await?;
            return Ok(vec![(Some(p.to_owned()), key)]);
        }

        let mut runs = Vec::new();
        for s in self.submodules(runner).await? {
            let key = self.key_for(runner, &s.name, inherited).await?;
            runs.push((Some(s.path), key));
        }
        if runs.iter().all(|(_, key)| key.as_deref() == inherited) {
            return Ok(vec![(None, inherited.map(str::to_owned))]);
        }
        Ok(runs)
    }

    fn update_command(
        &self,
        only: Option<&str>,
        recursive: bool,
        remote: bool,
        key: Option<&str>,
    ) -> GitCommand {
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
        if let Some(p) = only {
            // `--` so a submodule whose path starts with a dash is not read as a flag.
            cmd = cmd.arg("--").arg(p);
        }
        if let Some((name, value)) = ssh_env(key) {
            cmd = cmd.env(name, value);
        }
        cmd
    }

    /// Writes the key each submodule was reached with into that submodule's own clone.
    ///
    /// A fetch from inside a submodule — which is what the window does once one is open in a
    /// tab — is a git that reads only the submodule's configuration, so without this the key
    /// would reach the first clone and nothing after it.
    ///
    /// Cleared as well as written. A submodule left holding a key the superproject no longer
    /// uses authenticates as the wrong account, which is the failure the whole arrangement
    /// exists to prevent.
    ///
    /// A failure here leaves a perfectly good working copy, so it is logged and the update
    /// stands — the key can still be set from the submodule's own settings.
    async fn record_keys(&self, runner: &GitRunner, inherited: Option<&str>) {
        let Some(workdir) = self.workdir.clone() else {
            return;
        };
        let Ok(subs) = self.submodules(runner).await else {
            return;
        };
        for s in subs.into_iter().filter(|s| s.initialised) {
            let command = match self.key_for(runner, &s.name, inherited).await {
                Ok(key) => key.as_deref().map(crate::ssh::command_for),
                Err(e) => {
                    tracing::warn!(error = %e, path = s.path, "could not read the submodule's key");
                    continue;
                }
            };
            let at = workdir.join(&s.path);
            // Only `core.sshCommand`, rather than `set_ssh_local`, which would also unset the
            // public key and the credential helper this clone may have been given by hand.
            let wrote = match RepoLocation::discover(runner, &at).await {
                Ok(loc) => {
                    loc.config_put(
                        runner,
                        Scope::Repository,
                        "core.sshCommand",
                        command.as_deref(),
                    )
                    .await
                }
                Err(e) => Err(e),
            };
            if let Err(e) = wrote {
                tracing::warn!(error = %e, path = s.path, "updated, but could not record its key");
            }
        }
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
    use super::{Path, PathBuf, key_setting, module_dir, ssh_env};

    #[test]
    fn the_key_is_handed_over_in_the_environment() {
        // The variable name is the whole fix, and it is the one thing a fixture cannot check:
        // `tests/ssh/run.sh` proves against a real server that this is what a submodule's git
        // reads, and that the superproject's own `core.sshCommand` is not.
        let (name, value) = ssh_env(Some("/home/dev/.ssh/id_work")).expect("a pinned key");
        assert_eq!(name, "GIT_SSH_COMMAND");
        assert_eq!(value, crate::ssh::command_for("/home/dev/.ssh/id_work"));
    }

    #[test]
    fn a_repository_on_the_agent_hands_over_nothing() {
        // An empty value would shadow whatever the user set by hand, which is why the agent
        // case writes nothing rather than something empty.
        assert!(ssh_env(None).is_none());
    }

    #[test]
    fn a_submodules_own_key_is_stored_under_the_name_git_stores_it_by() {
        assert_eq!(
            key_setting("external/dev-scripts"),
            "coral.submodule.external/dev-scripts.sshKey"
        );
        // git splits a key on its first and last dot, so a dotted subsection stays whole.
        assert_eq!(key_setting("a.b.c"), "coral.submodule.a.b.c.sshKey");
    }

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
