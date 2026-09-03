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
