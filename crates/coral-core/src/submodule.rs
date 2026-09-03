use std::collections::BTreeMap;
use std::path::Path;

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
