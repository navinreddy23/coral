use bstr::{BString, ByteSlice};

use crate::error::CoralError;

/// What to do with one commit during an interactive rebase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Pick,
    Reword,
    Edit,
    /// Combine into the previous commit, keeping both messages.
    Squash,
    /// Combine into the previous commit, discarding this message.
    Fixup,
    Drop,
}

impl Step {
    /// The verb git writes in the todo file. The long form is used deliberately: the file is
    /// shown to the user in the merge tool and abbreviations are not self-explanatory.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Reword => "reword",
            Self::Edit => "edit",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }

    /// Accepts both the long and short forms git itself writes.
    #[must_use]
    pub fn parse(word: &[u8]) -> Option<Self> {
        Some(match word {
            b"pick" | b"p" => Self::Pick,
            b"reword" | b"r" => Self::Reword,
            b"edit" | b"e" => Self::Edit,
            b"squash" | b"s" => Self::Squash,
            b"fixup" | b"f" => Self::Fixup,
            b"drop" | b"d" => Self::Drop,
            _ => return None,
        })
    }
}

/// One line of a rebase todo list.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub step: Step,
    pub oid: String,
    #[serde(serialize_with = "crate::bytes::as_str")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub summary: BString,
    /// The replacement message for a [`Step::Reword`]. Never written to the todo file.
    #[serde(default)]
    pub message: Option<String>,
}

/// A rebase todo list, in the order the commits will be replayed.
///
/// Note that this is the reverse of how the graph shows them: git replays oldest first, while
/// the UI lists newest first. The conversion happens once, here, rather than in every caller.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
pub struct Todo {
    pub items: Vec<TodoItem>,
}

impl Todo {
    /// Parses a `git-rebase-todo` file, ignoring comments and blank lines.
    ///
    /// # Errors
    /// [`CoralError::Protocol`] on a line that is neither a comment nor a recognised step.
    pub fn parse(input: &[u8]) -> Result<Self, CoralError> {
        let mut items = Vec::new();
        for line in input.split(|b| *b == b'\n') {
            let line = line.trim();
            if line.is_empty() || line.starts_with(b"#") {
                continue;
            }
            let mut parts = line.splitn(3, |b| *b == b' ');
            let verb = parts.next().unwrap_or_default();
            let Some(step) = Step::parse(verb) else {
                // `label`, `merge`, `reset` and friends appear with --rebase-merges, which is
                // out of scope; refusing is safer than silently dropping them.
                return Err(CoralError::Protocol {
                    label: "rebase-todo",
                    detail: format!("unsupported step {:?}", verb.to_str_lossy()),
                });
            };
            let oid = parts.next().unwrap_or_default();
            items.push(TodoItem {
                step,
                oid: oid.to_str_lossy().into_owned(),
                summary: BString::from(parts.next().unwrap_or_default()),
                // The todo file has no field for it; a parsed list is one git wrote.
                message: None,
            });
        }
        Ok(Self { items })
    }

    /// Renders the todo file git will replay.
    ///
    /// Dropped commits are written as explicit `drop` lines rather than omitted, so the file
    /// still reads as a record of the user's decisions.
    #[must_use]
    pub fn render(&self) -> BString {
        let mut out = BString::from(Vec::new());
        for item in &self.items {
            out.extend_from_slice(item.step.verb().as_bytes());
            out.push(b' ');
            out.extend_from_slice(item.oid.as_bytes());
            if !item.summary.is_empty() {
                out.push(b' ');
                out.extend_from_slice(&item.summary);
            }
            out.push(b'\n');
        }
        out
    }

    /// Moves the item at `from` to `to`, shifting the rest.
    ///
    /// # Errors
    /// [`CoralError::Refused`] if either index is out of range.
    pub fn reorder(&mut self, from: usize, to: usize) -> Result<(), CoralError> {
        if from >= self.items.len() || to >= self.items.len() {
            return Err(CoralError::Refused {
                label: "reorder",
                detail: format!("index out of range for {} items", self.items.len()),
            });
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        Ok(())
    }

    /// The first item cannot be squashed or fixed up: there is nothing before it to fold into.
    #[must_use]
    pub fn first_step_is_valid(&self) -> bool {
        !matches!(
            self.items.first().map(|i| i.step),
            Some(Step::Squash | Step::Fixup)
        )
    }
}

impl crate::repo::RepoLocation {
    /// The todo list `git rebase -i <onto>` would open, without opening anything.
    ///
    /// Built from `rev-list` rather than by starting a rebase and reading the file git writes:
    /// starting one leaves the repository in a rebase that has to be aborted if the user
    /// changes their mind, and this is what the picker is populated from before they have
    /// decided anything.
    ///
    /// A range holding a merge is refused here rather than by each caller. The list is built
    /// with `--no-merges`, so such a range comes back a commit short and says nothing about it;
    /// a picker showing "4 of 4 commits kept" over a range of five is worse than no picker.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when the range holds a merge. Otherwise propagates git failures,
    /// including an unknown revision.
    pub async fn rebase_todo(
        &self,
        runner: &crate::process::GitRunner,
        onto: &str,
    ) -> Result<Todo, CoralError> {
        // Oldest first, which is replay order and the order the todo file is written in.
        let out = runner
            .output(
                crate::process::GitCommand::read("rev-list", self.display_path())
                    .args(["rev-list", "--reverse", "--no-merges", "--format=%H %s"])
                    .arg(format!("{onto}..HEAD")),
            )
            .await?;

        let mut items = Vec::new();
        for line in out.stdout.split(|b| *b == b'\n') {
            // `--format` prefixes each entry with a `commit <oid>` line of its own.
            if line.is_empty() || line.starts_with(b"commit ") {
                continue;
            }
            let (oid, summary) = match line.iter().position(|b| *b == b' ') {
                Some(at) => (&line[..at], &line[at + 1..]),
                None => (line, &b""[..]),
            };
            let Ok(oid) = oid.to_str() else { continue };
            items.push(TodoItem {
                step: Step::Pick,
                oid: oid.to_owned(),
                summary: BString::from(summary),
                message: None,
            });
        }
        self.refuse_merges_in(runner, onto, items.len()).await?;
        Ok(Todo { items })
    }

    /// Runs an interactive rebase against a todo the caller has already decided.
    ///
    /// git normally opens an editor on the todo file. Rather than launch one, the prepared
    /// list is written to a file and `sequence.editor` is pointed at Coral itself, which
    /// copies it into place — the same self-invocation the credential helper uses, and for the
    /// same reason: no shell quoting and nothing interactive on the path.
    ///
    /// A reword is run as an `edit` and amended here rather than through git's own `reword`,
    /// which opens an editor on the message with no way to say which commit is being asked
    /// about. Stopping instead gives an exact answer: git records the commit it stopped on, so
    /// the right message goes to the right commit even after everything above it has been
    /// rewritten. A stop with no message waiting is a stop the user asked for, and is handed
    /// back to them.
    ///
    /// # Errors
    /// Propagates git failures. A rebase that stops on a conflict or an `edit` is reported
    /// through [`crate::ops::OpOutcome`], not as an error.
    pub async fn rebase_interactive(
        &self,
        runner: &crate::process::GitRunner,
        onto: &str,
        todo: &Todo,
        coral_binary: &std::path::Path,
    ) -> Result<crate::ops::OpOutcome, CoralError> {
        if !todo.first_step_is_valid() {
            return Err(CoralError::Refused {
                label: "rebase",
                detail: "the first commit cannot be squashed or fixed up into its parent"
                    .to_owned(),
            });
        }

        // Reword becomes edit, and the message is kept against the commit it belongs to.
        //
        // Written to the git dir rather than held here, because this call does not always see
        // the rebase through: a conflict hands it to the merge tool, and what continues it from
        // there is `op`. A message that lived only in this function was dropped the moment the
        // commit being reworded was also the one that conflicted.
        let mut messages: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        let mut plan = todo.clone();
        for item in &mut plan.items {
            if item.step == Step::Reword {
                if let Some(message) = item.message.clone() {
                    messages.insert(item.oid.clone(), message);
                    item.step = Step::Edit;
                } else {
                    // Nothing to reword it to; leaving it as a reword would open an editor.
                    item.step = Step::Pick;
                }
            }
        }
        self.remember_rewords(&messages);

        let path = self.git_path("coral-rebase-todo");
        std::fs::write(&path, plan.render()).map_err(|e| CoralError::Protocol {
            label: "rebase",
            detail: format!("could not write the todo list: {e}"),
        })?;

        // Through the environment, not `-c sequence.editor`: the runner pins both editor
        // variables to a no-op so nothing can hang waiting for one, and an environment
        // variable beats config, so a `-c` here is silently ignored and git replays its own
        // unedited todo — a rebase that reports success and changes nothing.
        //
        // No `!` prefix either: that is special to aliases and credential helpers. This value
        // goes straight to the shell, where a leading `!` is part of the command name.
        //
        // `GIT_EDITOR` keeps its no-op: a squash or fixup opens an editor on the combined
        // message that nobody is there to answer, and accepting what git prepared is exactly
        // the message the user was shown.
        let cmd = crate::process::GitCommand::write("rebase", self.display_path())
            .env(
                "GIT_SEQUENCE_EDITOR",
                format!(
                    "{} rebase-editor --todo {}",
                    crate::process::shell_word(&coral_binary.display().to_string()),
                    crate::process::shell_word(&path.display().to_string()),
                ),
            )
            .args(["rebase", "--interactive", "--no-autosquash"])
            .arg(onto);

        let result = runner.output(cmd).await;
        // The file has served its purpose either way; leaving it behind would be read as a
        // rebase in progress by anything looking at the git dir.
        let _ = std::fs::remove_file(&path);

        let mut outcome = match result {
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                self.op_outcome(runner, msg).await?
            }
            Err(e) => {
                let stopped = self
                    .op_outcome(runner, e.stderr().unwrap_or_default().to_owned())
                    .await?;
                if stopped.completed {
                    return Err(e);
                }
                stopped
            }
        };

        // Each pass either amends one commit and carries on, or hands back. Bounded by the
        // list, because a rebase cannot stop more times than it has steps and a misread stop
        // must not be able to spin.
        for _ in 0..=plan.items.len() {
            if outcome.completed || !outcome.conflicts.is_empty() {
                break;
            }
            let stopped = self.operation(runner).await?.stopped_at;
            let Some(message) = stopped.as_deref().and_then(|oid| messages.get(oid)) else {
                // A stop the user asked for, or one nothing here can answer.
                break;
            };
            self.amend_message(runner, message).await?;
            outcome = self.op(runner, crate::ops::OpAction::Continue).await?;
        }
        if outcome.completed {
            self.forget_rewords();
        }
        Ok(outcome)
    }

    /// Where the messages a rebase still owes are kept.
    fn rewords_path(&self) -> std::path::PathBuf {
        self.git_path("coral-rebase-rewords")
    }

    /// Puts the messages an interactive rebase still owes where a later continue can find them.
    pub(crate) fn remember_rewords(&self, messages: &std::collections::BTreeMap<String, String>) {
        if messages.is_empty() {
            self.forget_rewords();
            return;
        }
        if let Ok(text) = serde_json::to_string(messages) {
            let _ = std::fs::write(self.rewords_path(), text);
        }
    }

    /// The messages a rebase still owes, keyed by the commit each belongs to.
    ///
    /// Unreadable or malformed is the same as none: a rebase that cannot find them replays the
    /// commits with the messages they already have, which is what git would have done anyway.
    #[must_use]
    pub(crate) fn rewords(&self) -> std::collections::BTreeMap<String, String> {
        std::fs::read_to_string(self.rewords_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Forgets them, for a rebase that finished or was abandoned.
    pub(crate) fn forget_rewords(&self) {
        let _ = std::fs::remove_file(self.rewords_path());
    }

    /// Puts the message a rebase owes the commit it is stopped on where git will use it.
    ///
    /// Which of the two places depends on whether that commit has been recorded yet, and git
    /// keeps the answer itself: it writes `amend` into the rebase directory when it stops on a
    /// commit it has already made. Before that, `message` is the text `--continue` will commit
    /// with; after it, the commit exists and has to be amended.
    pub(crate) async fn settle_reword(
        &self,
        runner: &crate::process::GitRunner,
    ) -> Result<(), CoralError> {
        let Some(oid) = self.operation(runner).await?.stopped_at else {
            return Ok(());
        };
        let waiting = self.rewords();
        let Some(message) = waiting.get(&oid) else {
            return Ok(());
        };
        let dir = self.git_path("rebase-merge");
        if dir.join("amend").exists() {
            self.amend_message(runner, message).await?;
        } else {
            let _ = std::fs::write(dir.join("message"), message);
        }
        Ok(())
    }

    /// Replaces the message of the commit a rebase has stopped on.
    pub(crate) async fn amend_message(
        &self,
        runner: &crate::process::GitRunner,
        message: &str,
    ) -> Result<(), CoralError> {
        // `--no-verify`, because a rebase replays commits that were accepted once already and
        // a hook rejecting one halfway through leaves the rebase stopped with nothing to say.
        runner
            .output(
                crate::process::GitCommand::write("commit", self.display_path())
                    .args(["commit", "--amend", "--no-verify", "--allow-empty", "-m"])
                    .arg(message),
            )
            .await
            .map(|_| ())
    }
}

/// A change to one named commit, expressed as the interactive rebase it becomes.
///
/// Each of these is a menu item in the reference's commit menu. They are one type rather than
/// four methods because the work either side of the edit — resolving the commit, refusing an
/// unsafe range, running the rebase — is the same for all of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rewrite {
    /// Remove the commit, replaying its children onto its parent.
    Drop,
    /// Replace the commit's message.
    Reword(String),
    /// Swap the commit with its child, moving it towards HEAD.
    MoveNewer,
    /// Swap the commit with its parent, moving it away from HEAD.
    MoveOlder,
}

impl Rewrite {
    /// How far back the rebase has to start for this edit to have both commits it needs.
    const fn depth(&self) -> u32 {
        match self {
            // Only the commit itself is touched, so its parent is enough.
            Self::Drop | Self::Reword(_) | Self::MoveNewer => 1,
            // The commit below has to be in the list too, so the base is one further back.
            Self::MoveOlder => 2,
        }
    }
}

impl crate::repo::RepoLocation {
    /// Rewrites one commit in place, replaying everything above it.
    ///
    /// Every one of these rewrites history, so it is refused for a commit that is not an
    /// ancestor of HEAD: replaying a commit that is not on this branch would either do nothing
    /// or silently take unrelated work with it.
    ///
    /// A range containing a merge is refused as well. The todo list is built with
    /// `--no-merges`, which is what an interactive rebase does, and replaying such a range
    /// flattens the merge out of the history without saying so.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when the commit is not on HEAD, when the range holds a merge, or
    /// when there is nothing to swap with. Otherwise propagates git failures; a rebase that
    /// stops is reported through [`crate::ops::OpOutcome`].
    pub async fn rewrite_commit(
        &self,
        runner: &crate::process::GitRunner,
        rev: &str,
        rewrite: &Rewrite,
        coral_binary: &std::path::Path,
    ) -> Result<crate::ops::OpOutcome, CoralError> {
        let oid = self.rev_parse(runner, rev).await?;
        if !self.is_ancestor_of_head(runner, &oid).await? {
            return Err(CoralError::Refused {
                label: "rewrite",
                detail: format!("{oid:.8} is not on the current branch"),
            });
        }

        let base = format!("{oid}~{}", rewrite.depth());
        let base = self.rev_parse(runner, &base).await?;
        let mut todo = self.rebase_todo(runner, &base).await?;

        let at = todo
            .items
            .iter()
            .position(|i| i.oid == oid)
            .ok_or_else(|| CoralError::Refused {
                label: "rewrite",
                detail: format!("{oid:.8} is not in the range being replayed"),
            })?;
        apply_rewrite(&mut todo, at, rewrite)?;

        self.rebase_interactive(runner, &base, &todo, coral_binary)
            .await
    }

    /// True when `oid` is reachable from HEAD, which is what makes it safe to replay.
    async fn is_ancestor_of_head(
        &self,
        runner: &crate::process::GitRunner,
        oid: &str,
    ) -> Result<bool, CoralError> {
        // `--is-ancestor` answers by exit status, so a false answer arrives as a git failure
        // rather than as output.
        let result = runner
            .output(
                crate::process::GitCommand::read("merge-base", self.display_path())
                    .args(["merge-base", "--is-ancestor"])
                    .arg(oid)
                    .arg("HEAD"),
            )
            .await;
        match result {
            Ok(_) => Ok(true),
            Err(CoralError::GitExit { code: 1, .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Refuses when the range holds a merge the todo list would flatten away.
    async fn refuse_merges_in(
        &self,
        runner: &crate::process::GitRunner,
        base: &str,
        without_merges: usize,
    ) -> Result<(), CoralError> {
        let out = runner
            .output(
                crate::process::GitCommand::read("rev-list", self.display_path())
                    .args(["rev-list", "--count"])
                    .arg(format!("{base}..HEAD")),
            )
            .await?;
        let total: usize =
            out.stdout
                .to_str_lossy()
                .trim()
                .parse()
                .map_err(|_| CoralError::Protocol {
                    label: "rev-list",
                    detail: "could not read the commit count".to_owned(),
                })?;
        if total == without_merges {
            return Ok(());
        }
        Err(CoralError::Refused {
            label: "rebase",
            detail: "there is a merge between this commit and HEAD; replaying the range would \
                     flatten it"
                .to_owned(),
        })
    }
}

/// Edits the todo list in place. Split out so it can be tested without a repository.
///
/// # Errors
/// [`CoralError::Refused`] when a move has nothing to swap with.
pub fn apply_rewrite(todo: &mut Todo, at: usize, rewrite: &Rewrite) -> Result<(), CoralError> {
    let Some(item) = todo.items.get_mut(at) else {
        return Err(CoralError::Refused {
            label: "rewrite",
            detail: format!("no commit at position {at}"),
        });
    };
    match rewrite {
        Rewrite::Drop => item.step = Step::Drop,
        Rewrite::Reword(message) => {
            item.step = Step::Reword;
            item.message = Some(message.clone());
        }
        // The list is oldest first and the graph is newest first, so moving a commit towards
        // HEAD moves it later in the list. This is the one place that conversion matters.
        Rewrite::MoveNewer => {
            if at + 1 >= todo.items.len() {
                return Err(CoralError::Refused {
                    label: "rewrite",
                    detail: "this is the newest commit; there is nothing above it".to_owned(),
                });
            }
            todo.items.swap(at, at + 1);
        }
        Rewrite::MoveOlder => {
            if at == 0 {
                return Err(CoralError::Refused {
                    label: "rewrite",
                    detail: "this is the oldest commit being replayed; there is nothing below it"
                        .to_owned(),
                });
            }
            todo.items.swap(at, at - 1);
        }
    }
    Ok(())
}
