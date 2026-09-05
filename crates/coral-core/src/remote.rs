use bstr::ByteSlice;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner, Sink};
use crate::repo::RepoLocation;

/// A configured remote.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    /// Differs from `fetch_url` when the user configured a separate push URL.
    pub push_url: String,
}

/// One phase of a transfer, as git reports it on stderr.
///
/// Not exported to `types.ts`: the window never sees a transfer phase, and this shares a name
/// with the rebase progress that it does see. Two types exporting under one name meant
/// whichever test ran last decided what `Progress` was, and the window's own type was the one
/// that lost.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// "Counting objects", "Receiving objects", and so on.
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub percent: u8,
    /// True when the phase is running on the server rather than locally.
    pub remote: bool,
    /// True for the final line of a phase.
    pub done: bool,
}

/// Parses one `--progress` line.
///
/// Lines look like `Counting objects:  11% (1/9)` and may be prefixed `remote: ` when the
/// server is reporting, suffixed `, done.`, and padded with trailing spaces. Anything that is
/// not a percentage report — "Delta compression using up to 16 threads" — yields `None`.
#[must_use]
pub fn parse_progress(line: &[u8]) -> Option<Progress> {
    let text = line.to_str().ok()?.trim();
    let (remote, text) = match text.strip_prefix("remote: ") {
        Some(rest) => (true, rest.trim()),
        None => (false, text),
    };

    let (phase, rest) = text.split_once(':')?;
    let rest = rest.trim();
    let done = rest.ends_with("done.");

    let percent_end = rest.find('%')?;
    let percent: u8 = rest[..percent_end].trim().parse().ok()?;

    // The counts are the first parenthesised pair after the percentage.
    let open = rest.find('(')?;
    let close = rest[open..].find(')')? + open;
    let (current, total) = rest[open + 1..close].split_once('/')?;

    Some(Progress {
        phase: phase.trim().to_owned(),
        current: current.trim().parse().ok()?,
        total: total.trim().parse().ok()?,
        percent,
        remote,
        done,
    })
}

/// What happened to one ref during a push.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum PushFlag {
    /// Fast-forwarded.
    Ok,
    /// Updated, but not a fast-forward.
    Forced,
    Deleted,
    New,
    Rejected,
    UpToDate,
}

impl PushFlag {
    const fn from_char(c: u8) -> Option<Self> {
        Some(match c {
            b' ' => Self::Ok,
            b'+' => Self::Forced,
            b'-' => Self::Deleted,
            b'*' => Self::New,
            b'!' => Self::Rejected,
            b'=' => Self::UpToDate,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn is_failure(self) -> bool {
        matches!(self, Self::Rejected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub flag: PushFlag,
    pub local: String,
    pub remote: String,
    /// git's own words, e.g. "[new branch]" or the reason for a rejection.
    pub summary: String,
}

/// Parses `git push --porcelain` output.
///
/// # Errors
/// Never; unrecognised lines are skipped, because git also prints a `To <url>` header and a
/// trailing `Done`.
#[must_use]
pub fn parse_push(stdout: &[u8]) -> Vec<PushResult> {
    stdout
        .split(|b| *b == b'\n')
        .filter_map(|line| {
            let flag = PushFlag::from_char(*line.first()?)?;
            // The flag is one character followed by a tab, so the remainder starts with the
            // separator; splitting without dropping it yields an empty first field.
            let rest = line.get(1..)?.to_str().ok()?.strip_prefix('\t')?;
            let mut fields = rest.splitn(2, '\t');
            let refs = fields.next()?;
            let (local, remote) = refs.split_once(':')?;
            Some(PushResult {
                flag,
                local: local.to_owned(),
                remote: remote.to_owned(),
                summary: fields.next().unwrap_or_default().trim().to_owned(),
            })
        })
        .collect()
}

/// How a pull should integrate the fetched commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PullMode {
    /// Refuse unless the local branch can fast-forward.
    #[default]
    FfOnly,
    Merge,
    Rebase,
}

/// What to push.
// Independent switches that map one-to-one onto git's own flags; grouping them into enums
// would obscure rather than clarify what is passed through.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
pub struct PushOpts {
    pub remote: Option<String>,
    /// Defaults to the current branch.
    pub refspec: Option<String>,
    pub set_upstream: bool,
    /// Overwrite the remote branch, but only if it is where we last saw it.
    pub force_with_lease: bool,
    pub tags: bool,
    /// Remove the remote branch instead of updating it.
    pub delete: bool,
}

impl RepoLocation {
    /// Lists configured remotes.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn remotes(&self, runner: &GitRunner) -> Result<Vec<Remote>, CoralError> {
        let out = runner
            .output(GitCommand::read("remote", self.display_path()).args(["remote", "--verbose"]))
            .await?;

        // Two lines per remote: "<name>\t<url> (fetch)" then "(push)".
        let mut remotes: Vec<Remote> = Vec::new();
        for line in out.stdout.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let text = line.to_str_lossy();
            let Some((name, rest)) = text.split_once('\t') else {
                continue;
            };
            let Some((url, kind)) = rest.rsplit_once(' ') else {
                continue;
            };

            match remotes.iter_mut().find(|r| r.name == name) {
                Some(existing) if kind == "(push)" => url.clone_into(&mut existing.push_url),
                Some(_) => {}
                None => remotes.push(Remote {
                    name: name.to_owned(),
                    fetch_url: url.to_owned(),
                    push_url: url.to_owned(),
                }),
            }
        }
        Ok(remotes)
    }

    /// Adds a remote.
    ///
    /// # Errors
    /// Propagates git failures, including a name already in use.
    pub async fn remote_add(
        &self,
        runner: &GitRunner,
        name: &str,
        url: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("remote", self.display_path()).args(["remote", "add", name, url]),
            )
            .await
            .map(|_| ())
    }

    /// Removes a remote and its tracking branches.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn remote_remove(&self, runner: &GitRunner, name: &str) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("remote", self.display_path()).args(["remote", "remove", name]),
            )
            .await
            .map(|_| ())
    }

    /// Renames a remote, moving its tracking branches with it.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn remote_rename(
        &self,
        runner: &GitRunner,
        from: &str,
        to: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("remote", self.display_path())
                    .args(["remote", "rename", from, to]),
            )
            .await
            .map(|_| ())
    }

    /// Changes a remote's URL.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn remote_set_url(
        &self,
        runner: &GitRunner,
        name: &str,
        url: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("remote", self.display_path())
                    .args(["remote", "set-url", name, url]),
            )
            .await
            .map(|_| ())
    }

    /// Drops tracking branches whose remote counterparts are gone.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn remote_prune(&self, runner: &GitRunner, name: &str) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::network("remote", self.display_path()).args(["remote", "prune", name]),
            )
            .await
            .map(|_| ())
    }

    /// Fetches from one remote, or from all of them.
    ///
    /// Fetching everything is one `git fetch --all`, not one process per remote: git shares
    /// the object store and the connection setup between them.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn fetch<F>(
        &self,
        runner: &GitRunner,
        remote: Option<&str>,
        prune: bool,
        mut on_progress: F,
    ) -> Result<(), CoralError>
    where
        F: FnMut(Progress),
    {
        let mut cmd =
            GitCommand::network("fetch", self.display_path()).args(["fetch", "--progress"]);
        if prune {
            cmd = cmd.arg("--prune");
        }
        cmd = match remote {
            Some(name) => cmd.arg(name),
            None => cmd.arg("--all"),
        };

        // Progress is written to stderr with carriage returns, so it is read as records
        // delimited by CR rather than by newline.
        runner
            .stream_err(cmd, |line| {
                if let Some(p) = parse_progress(line) {
                    on_progress(p);
                }
                Ok(Sink::Continue)
            })
            .await
    }

    /// Pushes to a remote.
    ///
    /// Forcing always uses `--force-with-lease`, never a bare `--force`: the lease refuses when
    /// the remote branch has moved since we last saw it, which is the case where a plain force
    /// silently destroys someone else's work.
    ///
    /// # Errors
    /// Propagates git failures. A rejected ref is reported in the results rather than as an
    /// error, so the caller can show which refs failed and why.
    pub async fn push<F>(
        &self,
        runner: &GitRunner,
        opts: &PushOpts,
        mut on_progress: F,
    ) -> Result<Vec<PushResult>, CoralError>
    where
        F: FnMut(Progress),
    {
        let mut cmd = GitCommand::network("push", self.display_path()).args([
            "push",
            "--porcelain",
            "--progress",
        ]);
        if opts.set_upstream {
            cmd = cmd.arg("--set-upstream");
        }
        if opts.force_with_lease {
            cmd = cmd.arg("--force-with-lease");
        }
        if opts.tags {
            cmd = cmd.arg("--tags");
        }
        if opts.delete {
            cmd = cmd.arg("--delete");
        }
        // `--set-upstream` alone is not enough: with no upstream configured git does not know
        // which remote to set, and answers "the current branch has no upstream branch" — which
        // is the case the flag exists for. Naming the remote and the branch is what
        // `git push --set-upstream origin main` does, and what this has to do for itself.
        let chosen = match (&opts.remote, &opts.refspec) {
            (None, None) if opts.set_upstream => self.first_push_target(runner).await?,
            _ => None,
        };
        if let Some(remote) = opts
            .remote
            .as_deref()
            .or(chosen.as_ref().map(|c| c.0.as_str()))
        {
            cmd = cmd.arg(remote);
        }
        if let Some(refspec) = opts
            .refspec
            .as_deref()
            .or(chosen.as_ref().map(|c| c.1.as_str()))
        {
            cmd = cmd.arg(refspec);
        }

        let mut stdout = Vec::new();
        let outcome = runner
            .stream_both(
                cmd,
                |line| {
                    stdout.extend_from_slice(line);
                    stdout.push(b'\n');
                    Ok(Sink::Continue)
                },
                |line| {
                    if let Some(p) = parse_progress(line) {
                        on_progress(p);
                    }
                    Ok(Sink::Continue)
                },
            )
            .await;

        let results = parse_push(&stdout);
        match outcome {
            Ok(()) => Ok(results),
            // A rejection is reported per-ref; only report an error if nothing explains it.
            Err(e) if results.iter().any(|r| r.flag.is_failure()) => {
                let _ = e;
                Ok(results)
            }
            Err(e) => Err(e),
        }
    }

    /// Where a branch with no upstream should be pushed the first time.
    ///
    /// `origin` when there is one, since that is what a clone makes and what everything else
    /// assumes; otherwise the only remote, because with exactly one there is no choice to make.
    /// `None` when the repository has no remotes or several and none called `origin` — git's own
    /// error is clearer than a guess would be.
    async fn first_push_target(
        &self,
        runner: &GitRunner,
    ) -> Result<Option<(String, String)>, CoralError> {
        let crate::repo::Head::Branch { name } = self.head(runner).await? else {
            // A detached HEAD has no branch to set an upstream for.
            return Ok(None);
        };

        let remotes = self.remotes(runner).await?;
        let picked = remotes.iter().find(|r| r.name == "origin").or_else(|| {
            if remotes.len() == 1 {
                remotes.first()
            } else {
                None
            }
        });
        Ok(picked.map(|r| (r.name.clone(), name)))
    }

    /// Fetches and integrates, in the way the caller asked for.
    ///
    /// # Errors
    /// Propagates git failures; a stopped merge or rebase is reported as an outcome.
    pub async fn pull(
        &self,
        runner: &GitRunner,
        remote: Option<&str>,
        mode: PullMode,
    ) -> Result<crate::ops::OpOutcome, CoralError> {
        let mut cmd = GitCommand::network("pull", self.display_path()).args([
            "pull",
            "--progress",
            "--no-edit",
        ]);
        cmd = match mode {
            PullMode::FfOnly => cmd.arg("--ff-only"),
            PullMode::Merge => cmd.arg("--no-rebase"),
            PullMode::Rebase => cmd.arg("--rebase"),
        };
        if let Some(name) = remote {
            cmd = cmd.arg(name);
        }

        self.run_stoppable(runner, cmd).await
    }
}
