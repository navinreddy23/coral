use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::{Head, RepoLocation};

/// Where every ref pointed at one moment.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RefSnapshot {
    /// Full ref name to object id.
    pub refs: BTreeMap<String, String>,
    /// The branch HEAD was on, or `None` when detached.
    pub head_branch: Option<String>,
    /// What HEAD resolved to. `None` on an unborn branch.
    pub head_oid: Option<String>,
}

impl RefSnapshot {
    /// Refs that differ between two snapshots, as `(name, before, after)`. A missing side
    /// means the ref did not exist then.
    #[must_use]
    pub fn diff(&self, other: &Self) -> Vec<(String, Option<String>, Option<String>)> {
        let mut names: Vec<&String> = self.refs.keys().chain(other.refs.keys()).collect();
        names.sort_unstable();
        names.dedup();
        names
            .into_iter()
            .filter_map(|n| {
                let (a, b) = (self.refs.get(n), other.refs.get(n));
                (a != b).then(|| (n.clone(), a.cloned(), b.cloned()))
            })
            .collect()
    }
}

/// The first ref the step would move that is no longer where the journal left it.
///
/// A ref that moved outside Coral — a commit in a terminal, another client — makes the whole
/// step unsafe, not just that one ref: restoring the rest would leave a half-undone state.
fn moved_since(now: &RefSnapshot, from: &RefSnapshot, target: &RefSnapshot) -> Option<String> {
    from.diff(target)
        .into_iter()
        .find(|(name, expected, _)| now.refs.get(name) != expected.as_ref())
        .map(|(name, _, _)| name)
}

/// Which copy of the tracked files a comparison is about.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Worktree,
    Index,
}

/// One reversible operation.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JournalEntry {
    /// What the user did, for the undo tooltip.
    pub label: String,
    pub before: RefSnapshot,
    pub after: RefSnapshot,
    /// Seconds since the epoch.
    pub at: i64,
}

/// A bounded, persisted history of ref-changing operations.
///
/// Only refs are journaled, so only what moved a ref can be stepped back. A discard moves no
/// ref and keeps nothing: the window says so before doing it. A worktree that has moved on
/// since the operation makes an undo unsafe, and [`RepoLocation::restore_refs`] refuses rather
/// than overwriting the user's work.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Journal {
    pub entries: Vec<JournalEntry>,
    /// How many entries from the end have been undone and can be redone.
    pub undone: usize,
}

/// Kept small deliberately: this is an undo stack, not an audit log, and it is rewritten on
/// every operation.
const MAX_ENTRIES: usize = 50;

impl Journal {
    /// Where the journal lives for a repository. Inside the git dir, so it travels with the
    /// repository but is never committed.
    #[must_use]
    pub fn path(loc: &RepoLocation) -> PathBuf {
        loc.git_dir.join("coral").join("journal.json")
    }

    /// Reads the journal, treating any unreadable or malformed file as empty.
    ///
    /// A corrupt undo stack must never stop the application from opening a repository.
    #[must_use]
    pub fn load(loc: &RepoLocation) -> Self {
        std::fs::read(Self::path(loc))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Writes the journal.
    ///
    /// # Errors
    /// [`CoralError::Io`] if the file cannot be written.
    pub fn save(&self, loc: &RepoLocation) -> Result<(), CoralError> {
        let path = Self::path(loc);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| CoralError::Protocol {
            label: "journal",
            detail: e.to_string(),
        })?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Records an operation, discarding anything that had been undone.
    ///
    /// Doing something new after undoing abandons the redo stack, which is what every editor
    /// does and what users expect.
    pub fn record(&mut self, entry: JournalEntry) {
        self.entries.truncate(self.entries.len() - self.undone);
        self.undone = 0;
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    /// The entry `undo` would reverse.
    #[must_use]
    pub fn undoable(&self) -> Option<&JournalEntry> {
        self.entries
            .len()
            .checked_sub(self.undone + 1)
            .and_then(|i| self.entries.get(i))
    }

    /// The entry `redo` would replay.
    #[must_use]
    pub fn redoable(&self) -> Option<&JournalEntry> {
        (self.undone > 0)
            .then(|| self.entries.len().checked_sub(self.undone))
            .flatten()
            .and_then(|i| self.entries.get(i))
    }
}

impl RepoLocation {
    /// Captures every ref and where HEAD points.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn snapshot_refs(&self, runner: &GitRunner) -> Result<RefSnapshot, CoralError> {
        let out = runner
            .output(
                GitCommand::read("for-each-ref", self.display_path())
                    .arg("for-each-ref")
                    .arg("--format=%(refname)%00%(objectname)"),
            )
            .await?;

        let mut refs = BTreeMap::new();
        for line in out.stdout.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            let text = String::from_utf8_lossy(line);
            if let Some((name, oid)) = text.split_once('\0') {
                refs.insert(name.to_owned(), oid.to_owned());
            }
        }

        let (head_branch, head_oid) = match self.head(runner).await? {
            Head::Branch { name } => (Some(name), self.rev_parse(runner, "HEAD").await.ok()),
            Head::Detached { oid } => (None, Some(oid)),
            Head::Unborn { name } => (Some(name), None),
        };
        Ok(RefSnapshot {
            refs,
            head_branch,
            head_oid,
        })
    }

    /// Restores refs to a snapshot.
    ///
    /// Refuses when the worktree has changes that the restore would overwrite, because moving
    /// refs underneath uncommitted work silently changes what those changes mean.
    ///
    /// # Errors
    /// [`CoralError::Refused`] if the worktree would lose work; otherwise propagates git
    /// failures.
    pub async fn restore_refs(
        &self,
        runner: &GitRunner,
        target: &RefSnapshot,
        from: &RefSnapshot,
    ) -> Result<(), CoralError> {
        let status = self.status(runner).await?;
        if !status.is_clean() && !self.already_holds(runner, target).await? {
            return Err(CoralError::Refused {
                label: "undo",
                detail: "the worktree has changes; commit or stash them first".to_owned(),
            });
        }

        for (name, before, after) in from.diff(target) {
            // `diff` is called on the current state, so `after` is where we want to end up.
            match after {
                Some(oid) => {
                    let mut cmd = GitCommand::write("update-ref", self.display_path()).args([
                        "update-ref",
                        &name,
                        &oid,
                    ]);
                    // Only move the ref if it is still where we left it.
                    if let Some(expected) = before {
                        cmd = cmd.arg(expected);
                    }
                    runner.output(cmd).await?;
                }
                None => {
                    runner
                        .output(GitCommand::write("update-ref", self.display_path()).args([
                            "update-ref",
                            "-d",
                            &name,
                        ]))
                        .await?;
                }
            }
        }

        // HEAD last, so the checkout lands on refs that are already correct.
        match (&target.head_branch, &target.head_oid) {
            (Some(branch), _) => self.checkout(runner, branch).await?,
            (None, Some(oid)) => self.checkout(runner, oid).await?,
            (None, None) => {}
        }

        // Checking out the branch you are already on is a no-op, so moving its ref underneath
        // leaves the index and worktree describing the old commit. Syncing them is safe here
        // precisely because the guard above established that they hold nothing this would
        // overwrite.
        if let (Some(_), Some(oid)) = (&target.head_branch, &target.head_oid) {
            self.reset(runner, oid, crate::ops::ResetMode::Hard).await?;
        }
        Ok(())
    }

    /// Whether the tracked worktree already holds exactly what `target` points at.
    ///
    /// A soft or mixed reset leaves the worktree dirty by construction: what shows as changed
    /// is the content of the commits it just took off the branch. Undoing one puts those
    /// commits back and syncs the worktree to them, which writes nothing the files do not
    /// already contain — so the dirty-worktree refusal has nothing to protect, and refusing
    /// meant the one action people most want back was the one that could never be taken back.
    ///
    /// Anything the user actually wrote makes this false and the refusal stands. Untracked
    /// files do not enter into it: `diff` does not see them and `reset --hard` does not touch
    /// them.
    async fn already_holds(
        &self,
        runner: &GitRunner,
        target: &RefSnapshot,
    ) -> Result<bool, CoralError> {
        let Some(oid) = target.head_oid.as_deref() else {
            return Ok(false);
        };
        // The worktree has to hold the target already, or the restore overwrites something
        // written by hand.
        if !self.holds(runner, oid, Side::Worktree).await? {
            return Ok(false);
        }
        // And so does the index, with one exception: a mixed reset leaves it holding the tree
        // of the commit HEAD is on now, which is a commit that still exists, so nothing in
        // there is the only copy of anything. Anything else staged is, and this is where work
        // that lives nowhere but the index was being thrown away.
        if self.holds(runner, oid, Side::Index).await? {
            return Ok(true);
        }
        let Ok(head) = self.rev_parse(runner, "HEAD").await else {
            return Ok(false);
        };
        self.holds(runner, &head, Side::Index).await
    }

    /// Whether one side of the repository holds exactly what `oid` points at.
    async fn holds(&self, runner: &GitRunner, oid: &str, side: Side) -> Result<bool, CoralError> {
        // `--name-only` rather than `--quiet`, whose answer is the exit code: a non-zero exit
        // is an error to the runner, and this one is not an error.
        let mut cmd = GitCommand::read("diff", self.display_path()).arg("diff");
        if side == Side::Index {
            cmd = cmd.arg("--cached");
        }
        let out = runner.output(cmd.args(["--name-only", oid, "--"])).await?;
        Ok(out.stdout.is_empty())
    }
}

impl RepoLocation {
    /// Records an entry when an operation moved a ref.
    ///
    /// Snapshots are taken either side of the mutation rather than the operation describing
    /// what it changed, so undo works for operations the journal knows nothing about.
    ///
    /// # Errors
    /// Propagates a failure to write the journal file.
    pub fn journal_change(
        &self,
        label: &str,
        before: RefSnapshot,
        after: RefSnapshot,
    ) -> Result<(), CoralError> {
        if before == after {
            return Ok(());
        }
        let mut journal = Journal::load(self);
        journal.record(JournalEntry {
            label: label.to_owned(),
            before,
            after,
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0)),
        });
        journal.save(self)
    }

    /// Moves one step through the journal, backwards to undo or forwards to redo.
    ///
    /// Returns what it did, phrased for the user.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when there is nothing to step to, or the worktree is dirty.
    pub async fn undo_step(
        &self,
        runner: &GitRunner,
        backwards: bool,
    ) -> Result<String, CoralError> {
        let mut journal = Journal::load(self);
        let picked = if backwards {
            journal.undoable()
        } else {
            journal.redoable()
        };
        let entry = picked.cloned().ok_or_else(|| CoralError::Refused {
            label: if backwards { "undo" } else { "redo" },
            detail: format!("nothing to {}", if backwards { "undo" } else { "redo" }),
        })?;

        let (target, from) = if backwards {
            (&entry.before, &entry.after)
        } else {
            (&entry.after, &entry.before)
        };
        // Checked here rather than left to `update-ref`, which refuses with its own message:
        // "cannot lock ref 'refs/heads/main': is at <sha> but expected <sha>" says nothing
        // about the step being taken, and the user is looking at an Undo button, not at git.
        let now = self.snapshot_refs(runner).await?;
        if let Some(name) = moved_since(&now, from, target) {
            return Err(CoralError::Refused {
                label: if backwards { "undo" } else { "redo" },
                detail: format!(
                    "{name} has moved since {}; there is nothing safe to {} here",
                    entry.label,
                    if backwards { "undo" } else { "redo" }
                ),
            });
        }
        self.restore_refs(runner, target, from).await?;

        if backwards {
            journal.undone += 1;
        } else {
            journal.undone -= 1;
        }
        journal.save(self)?;

        let verb = if backwards { "undid" } else { "redid" };
        Ok(format!("{verb} {}", entry.label))
    }
}
