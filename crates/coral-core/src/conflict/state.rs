use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::{OpState, RepoLocation};

/// What the two sides of a conflict should be called in the interface.
///
/// Never "ours" and "theirs". During a rebase those words are actively misleading: git replays
/// your commits on top of the target, so stage 2 ("ours") is the branch you are rebasing
/// *onto* and stage 3 ("theirs") is your own work. Presenting the raw words is how every git
/// client confuses its users.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SideLabels {
    /// What stage 2 is.
    pub ours: String,
    /// What stage 3 is.
    pub theirs: String,
    /// True during a rebase, where the sides read backwards from what a user expects. The UI
    /// states this once rather than leaving people to work it out.
    pub swapped: bool,
}

/// How far through a multi-step operation git has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub current: u32,
    pub total: u32,
}

/// Everything known about the operation in progress.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub state: OpState,
    pub labels: SideLabels,
    /// Present for a rebase, which replays many commits.
    pub progress: Option<Progress>,
    /// The branch being rebased, as a short name.
    pub head_name: Option<String>,
    /// The commit the operation stopped on.
    pub stopped_at: Option<String>,
    /// True for `rebase -i`.
    pub interactive: bool,
    /// Whether git has an operation it can be told to continue, abort or skip.
    ///
    /// False for a conflicted index with no marker file: `merge --no-commit`, `cherry-pick
    /// --no-commit`, and a stash that would not apply all leave one. The state is reported as
    /// a merge because that is what git's own status calls it, but there is nothing to
    /// continue — the files are resolved, staged and committed like any other change, and
    /// `git merge --continue` answers that no merge is in progress.
    pub resumable: bool,

    /// True when this is `git am` rather than a rebase.
    ///
    /// The two leave the same files behind and are settled the same way, so the state is
    /// `Rebase` for both. What differs is what to call it and which sides it has: applying a
    /// patch does not reverse them, and telling somebody who just opened a patch file that a
    /// rebase is in progress with its sides reversed is two pieces of wrong information.
    pub applying: bool,

    /// The message git wrote for the commit that will finish this, if there is one.
    ///
    /// git leaves it in `MERGE_MSG` and reads it back when it opens an editor. Coral does not
    /// open one, so without this the message was simply dropped: a merge or a cherry-pick
    /// asked for without committing leaves the changes staged and the commit box empty, and
    /// the subject that git had already written had to be typed out again from the row above.
    pub prepared: Option<String>,
}

impl RepoLocation {
    /// Reads the in-progress operation from the files git leaves in the git dir.
    ///
    /// Faster and more reliable than porcelain, which only reports that files are unmerged.
    ///
    /// # Errors
    /// Propagates git failures from resolving ref names.
    pub async fn operation(&self, runner: &GitRunner) -> Result<Operation, CoralError> {
        let state = self.op_state();
        match state {
            OpState::Rebase => self.rebase_operation(runner).await,
            OpState::Merge => self.merge_operation(runner).await,
            OpState::CherryPick | OpState::Revert => {
                let head = if state == OpState::CherryPick {
                    "CHERRY_PICK_HEAD"
                } else {
                    "REVERT_HEAD"
                };
                let stopped = self.read_head_file(head);
                // Named the way git names it in its own conflict messages, because the raw
                // object id is what was here before: a forty-character label on a button, on
                // a column header, and in every sentence the merge tool writes.
                //
                // A revert applies the commit backwards, so the side coming in is the state
                // *before* it, and naming it with the commit alone told the reader that taking
                // that side gave them the commit — the opposite of what it does. git writes
                // "parent of <id> (<subject>)" in its own markers here, and so does this.
                let named = match &stopped {
                    Some(oid) => self.name_of(runner, oid).await,
                    None => None,
                };
                let theirs = match (state, named) {
                    (OpState::Revert, Some(name)) => Some(format!("parent of {name}")),
                    (_, named) => named,
                };
                Ok(Operation {
                    state,
                    labels: SideLabels {
                        ours: self.current_branch(runner).await,
                        theirs: theirs.unwrap_or_else(|| "incoming".to_owned()),
                        swapped: false,
                    },
                    progress: None,
                    head_name: None,
                    stopped_at: stopped,
                    interactive: false,
                    resumable: true,
                    applying: false,
                    prepared: self.prepared_message(),
                })
            }
            OpState::Clean if self.has_unmerged(runner).await? => {
                // A conflicted index with nothing to mark it. `cherry-pick --no-commit` and
                // `merge --no-commit` both leave one: the files are unmerged and there is no
                // CHERRY_PICK_HEAD or MERGE_HEAD to find, so reading the git dir alone said
                // nothing was happening and the window offered no way to resolve them.
                //
                // Reported as a merge, which is what git's own status calls it, and finished
                // by committing rather than by continuing, since there is no operation to
                // continue.
                Ok(Operation {
                    state: OpState::Merge,
                    labels: SideLabels {
                        ours: self.current_branch(runner).await,
                        theirs: "the incoming change".to_owned(),
                        swapped: false,
                    },
                    progress: None,
                    head_name: None,
                    stopped_at: None,
                    interactive: false,
                    resumable: false,
                    applying: false,
                    prepared: self.prepared_message(),
                })
            }
            OpState::Bisect | OpState::Clean => Ok(Operation {
                state,
                labels: SideLabels {
                    ours: self.current_branch(runner).await,
                    theirs: String::new(),
                    swapped: false,
                },
                progress: None,
                head_name: None,
                stopped_at: None,
                interactive: false,
                resumable: false,
                applying: false,
                prepared: self.prepared_message(),
            }),
        }
    }

    /// How git itself writes a commit in a conflict message: `3755eae (test conflicts)`.
    ///
    /// `None` when the commit cannot be read, which is the caller's cue to say something
    /// vaguer rather than print an object id.
    async fn name_of(&self, runner: &GitRunner, oid: &str) -> Option<String> {
        let out = runner
            .output(GitCommand::read("show", self.display_path()).args([
                "show",
                "--no-patch",
                "--format=%h (%s)",
                oid,
            ]))
            .await
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    /// Whether any path is left unmerged in the index.
    ///
    /// Asked only when the git dir shows nothing in progress, so the cost falls on the case
    /// that would otherwise be reported as "nothing is happening" while files sit conflicted.
    async fn has_unmerged(&self, runner: &GitRunner) -> Result<bool, CoralError> {
        let out = runner
            .output(GitCommand::read("diff", self.display_path()).args([
                "diff",
                "--name-only",
                "--diff-filter=U",
            ]))
            .await?;
        Ok(!out.stdout.is_empty())
    }

    async fn rebase_operation(&self, runner: &GitRunner) -> Result<Operation, CoralError> {
        let dir = if self.git_path("rebase-merge").is_dir() {
            "rebase-merge"
        } else {
            "rebase-apply"
        };
        let read = |name: &str| self.read_rebase_file(dir, name);

        // rebase-apply uses next/last where rebase-merge uses msgnum/end.
        let current = read("msgnum")
            .or_else(|| read("next"))
            .and_then(|v| v.parse().ok());
        let total = read("end")
            .or_else(|| read("last"))
            .and_then(|v| v.parse().ok());
        let head_name = read("head-name").map(|n| short_ref(&n));
        let onto = read("onto");

        let applying = self.applying_patches();

        // `git am` keeps HEAD on the branch and puts the patch on the other side, which is the
        // ordinary orientation. A rebase is the one that reverses them.
        let labels = if applying {
            SideLabels {
                ours: self.current_branch(runner).await,
                theirs: read("final-commit")
                    .and_then(|m| m.lines().next().map(str::to_owned))
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| "the patch".to_owned()),
                swapped: false,
            }
        } else {
            let onto_label = match &onto {
                Some(oid) => self.describe(runner, oid).await,
                None => "the target branch".to_owned(),
            };
            SideLabels {
                // Stage 2 during a rebase is the target, not the user's work.
                ours: onto_label,
                theirs: head_name
                    .clone()
                    .unwrap_or_else(|| "your commits".to_owned()),
                swapped: true,
            }
        };

        Ok(Operation {
            state: OpState::Rebase,
            labels,
            progress: current
                .zip(total)
                .map(|(current, total)| Progress { current, total }),
            head_name,
            stopped_at: read("stopped-sha"),
            interactive: self.git_path(dir).join("interactive").exists(),
            resumable: true,
            applying,
            prepared: self.prepared_message(),
        })
    }

    async fn merge_operation(&self, runner: &GitRunner) -> Result<Operation, CoralError> {
        let incoming = self.read_head_file("MERGE_HEAD");
        let theirs = match &incoming {
            Some(oid) => self.describe(runner, oid).await,
            None => "the merged branch".to_owned(),
        };
        Ok(Operation {
            state: OpState::Merge,
            labels: SideLabels {
                ours: self.current_branch(runner).await,
                theirs,
                swapped: false,
            },
            progress: None,
            head_name: None,
            stopped_at: incoming,
            interactive: false,
            resumable: true,
            applying: false,
            prepared: self.prepared_message(),
        })
    }

    /// The message git prepared for the commit that ends the operation in progress.
    ///
    /// `MERGE_MSG` is written by merge, cherry-pick, revert and squash alike, and stays until
    /// the commit is made. Its comment lines are git's own and never part of the message: git
    /// strips them itself when it reads the file back out of the editor.
    fn prepared_message(&self) -> Option<String> {
        let raw = std::fs::read_to_string(self.git_path("MERGE_MSG")).ok()?;
        let said: Vec<&str> = raw.lines().filter(|line| !line.starts_with('#')).collect();
        let text = said.join("\n").trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    fn read_rebase_file(&self, dir: &str, name: &str) -> Option<String> {
        std::fs::read_to_string(self.git_path(dir).join(name))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    /// Reads a single-line file from the git dir, treating blank or absent as none.
    fn read_head_file(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.git_path(name))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    /// A human-readable name for a commit: a ref pointing at it, else an abbreviated id.
    async fn describe(&self, runner: &GitRunner, rev: &str) -> String {
        let named = runner
            .output(
                GitCommand::read("name-rev", self.display_path())
                    .args([
                        "name-rev",
                        "--name-only",
                        "--refs=refs/heads/*",
                        "--refs=refs/remotes/*",
                        // Merging a release tag should say "v7.2", not an object id.
                        "--refs=refs/tags/*",
                    ])
                    .arg(rev),
            )
            .await
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|n| !n.is_empty() && n != "undefined")
            // name-rev qualifies tags as "tags/v7.2" and remote branches as
            // "remotes/origin/main". The bare names are what people call them, and the merge
            // tool puts these in a column heading where every character counts.
            .map(|n| n.strip_prefix("tags/").unwrap_or(&n).to_owned())
            .map(|n| n.strip_prefix("remotes/").unwrap_or(&n).to_owned());

        named.unwrap_or_else(|| rev.chars().take(8).collect())
    }

    async fn current_branch(&self, runner: &GitRunner) -> String {
        match self.head(runner).await {
            Ok(crate::repo::Head::Branch { name } | crate::repo::Head::Unborn { name }) => name,
            Ok(crate::repo::Head::Detached { oid }) => oid.chars().take(8).collect(),
            Err(_) => "HEAD".to_owned(),
        }
    }
}

/// `refs/heads/topic` to `topic`.
fn short_ref(name: &str) -> String {
    name.strip_prefix("refs/heads/").unwrap_or(name).to_owned()
}
