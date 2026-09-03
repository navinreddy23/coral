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
    /// # Errors
    /// Propagates git failures, including an unknown revision.
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
            });
        }
        Ok(Todo { items })
    }

    /// Runs an interactive rebase against a todo the caller has already decided.
    ///
    /// git normally opens an editor on the todo file. Rather than launch one, the prepared
    /// list is written to a file and `sequence.editor` is pointed at Coral itself, which
    /// copies it into place — the same self-invocation the credential helper uses, and for the
    /// same reason: no shell quoting and nothing interactive on the path.
    ///
    /// `core.editor` is stubbed too. A squash or fixup opens an editor on the combined message
    /// that nobody is there to answer; `true` accepts what git prepared, which is the message
    /// the user was shown.
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

        let path = self.git_path("coral-rebase-todo");
        std::fs::write(&path, todo.render()).map_err(|e| CoralError::Protocol {
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
                    "'{}' rebase-editor --todo '{}'",
                    coral_binary.display(),
                    path.display()
                ),
            )
            .args(["rebase", "--interactive", "--no-autosquash"])
            .arg(onto);

        let result = runner.output(cmd).await;
        // The file has served its purpose either way; leaving it behind would be read as a
        // rebase in progress by anything looking at the git dir.
        let _ = std::fs::remove_file(&path);

        match result {
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                self.op_outcome(runner, msg).await
            }
            Err(e) => {
                let outcome = self
                    .op_outcome(runner, e.stderr().unwrap_or_default().to_owned())
                    .await?;
                if outcome.completed {
                    Err(e)
                } else {
                    Ok(outcome)
                }
            }
        }
    }
}
