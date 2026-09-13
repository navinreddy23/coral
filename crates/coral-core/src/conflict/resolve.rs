use std::collections::HashSet;

use bstr::BString;

use super::blocks::Take;
use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner};
use crate::repo::RepoLocation;
use crate::status::ConflictKind;

/// One file needing a decision.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ConflictedFile {
    pub path: String,
    pub kind: ConflictKind,
    /// Binary files offer only whole-file choices; there are no blocks to pick between.
    pub binary: bool,
    /// One side deleted the file, so keeping or deleting is the only meaningful choice.
    pub delete_modify: bool,
    /// The repository stores this path through a filter driver, so what the index holds is
    /// not the file. Whole-file choices only, for the same reason a binary file gets them.
    pub filtered: bool,
}

impl ConflictedFile {
    /// True when the file can be resolved block by block rather than only wholesale.
    #[must_use]
    pub const fn supports_blocks(&self) -> bool {
        !self.binary && !self.delete_modify && !self.filtered
    }
}

/// How to resolve one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Take stage 2 wholesale.
    TakeOurs,
    /// Take stage 3 wholesale.
    TakeTheirs,
    /// Remove the file.
    Delete,
    /// Write this exact content, which is what the merge tool's editable output produces.
    Content(BString),
}

/// Reads `git check-attr -z`: `<path>\0<attribute>\0<value>\0` for each path asked about.
///
/// Any value but git's own two words for "no driver here" names one.
fn driven(out: &[u8]) -> HashSet<String> {
    let mut fields = out.split(|b| *b == 0);
    let mut named = HashSet::new();
    while let (Some(path), Some(_attr), Some(value)) = (fields.next(), fields.next(), fields.next())
    {
        if !matches!(value, b"unspecified" | b"unset") {
            named.insert(String::from_utf8_lossy(path).into_owned());
        }
    }
    named
}

impl RepoLocation {
    /// Lists the files still needing a decision.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn conflicts(&self, runner: &GitRunner) -> Result<Vec<ConflictedFile>, CoralError> {
        let status = self.status(runner).await?;
        let mut out = Vec::new();

        for entry in status.conflicted() {
            let path = entry.path.to_string();
            let kind = entry.conflict.unwrap_or(ConflictKind::BothModified);
            let delete_modify = matches!(
                kind,
                ConflictKind::DeletedByUs | ConflictKind::DeletedByThem | ConflictKind::BothDeleted
            );
            let binary = if delete_modify {
                false
            } else {
                self.stage_is_binary(runner, &path).await
            };
            out.push(ConflictedFile {
                path,
                kind,
                binary,
                delete_modify,
                filtered: false,
            });
        }

        let filtered = self.filtered(runner, &out).await?;
        for file in &mut out {
            file.filtered = filtered.contains(&file.path);
        }
        Ok(out)
    }

    /// Which of these paths git converts through a filter driver on its way to the worktree.
    ///
    /// Git LFS is the one that matters. It keeps a pointer of three lines in the repository
    /// and the asset in the worktree, so offering those three lines as a conflict invites a
    /// resolution that takes one side's object and the other's size. That names nothing: the
    /// commit is made, it is pushed, and every clone after it has no file there at all.
    async fn filtered(
        &self,
        runner: &GitRunner,
        files: &[ConflictedFile],
    ) -> Result<HashSet<String>, CoralError> {
        if files.is_empty() {
            return Ok(HashSet::new());
        }
        let mut paths = Vec::new();
        for file in files {
            paths.extend_from_slice(file.path.as_bytes());
            paths.push(0);
        }
        let out = runner
            .output(
                GitCommand::read("check-attr", self.display_path())
                    .args(["check-attr", "-z", "--stdin", "filter"])
                    .stdin_bytes(paths),
            )
            .await?;
        Ok(driven(&out.stdout))
    }

    /// Whether a conflicted path's content is binary, judged by git rather than by us.
    async fn stage_is_binary(&self, runner: &GitRunner, path: &str) -> bool {
        let Ok(stages) = self.stage_blobs(runner, path).await else {
            return false;
        };
        // git's own rule: a NUL byte in the first 8000 bytes means binary.
        [stages.ours, stages.theirs, stages.base]
            .into_iter()
            .flatten()
            .any(|b| b.iter().take(8000).any(|c| *c == 0))
    }

    /// Applies a resolution and stages the result.
    ///
    /// # Errors
    /// [`CoralError::Refused`] if the path is not conflicted; otherwise propagates git
    /// failures.
    pub async fn resolve(
        &self,
        runner: &GitRunner,
        path: &str,
        resolution: &Resolution,
    ) -> Result<(), CoralError> {
        match resolution {
            Resolution::Delete => {
                runner
                    .output(
                        GitCommand::write("rm", self.display_path())
                            .args(["rm", "--quiet", "-f", "--"])
                            .arg(path),
                    )
                    .await?;
                return Ok(());
            }
            Resolution::TakeOurs | Resolution::TakeTheirs => {
                let take = if *resolution == Resolution::TakeOurs {
                    Take::Ours
                } else {
                    Take::Theirs
                };
                let content = self.side(runner, path, take).await?;
                self.write_worktree(path, &content)?;
            }
            Resolution::Content(content) => {
                self.write_worktree(path, content)?;
            }
        }

        runner
            .output(
                GitCommand::write("add", self.display_path())
                    .args(["add", "--"])
                    .arg(path),
            )
            .await?;
        self.checkout_form(runner, path).await;
        Ok(())
    }

    /// One whole side of a conflict, as the index holds it.
    ///
    /// `git checkout --ours` is deliberately not used. During a rebase it means the opposite of
    /// what the user picked, because stage 2 is the branch being rebased onto rather than their
    /// own work; reading the stage blob directly keeps the caller's choice honest whatever the
    /// operation.
    async fn side(
        &self,
        runner: &GitRunner,
        path: &str,
        take: Take,
    ) -> Result<BString, CoralError> {
        let stages = self.stage_blobs(runner, path).await?;
        let content = match take {
            Take::Ours => stages.ours,
            Take::Theirs => stages.theirs,
            Take::Base => stages.base,
        };
        content.ok_or_else(|| CoralError::Refused {
            label: "resolve",
            detail: format!("{path} has no content on that side; delete it instead"),
        })
    }

    /// Leaves the worktree copy as a checkout would have written it.
    ///
    /// The bytes just staged are the repository's own: no line endings converted, no smudge
    /// filter run over them. Written out as they are they leave an LF file in a
    /// `core.autocrlf` checkout, and under Git LFS a pointer of a few lines where the asset
    /// should be. Neither is ever reported, because cleaning those bytes again gives back
    /// exactly what the index holds, so git calls the worktree clean and the file stays wrong
    /// until something else checks it out.
    ///
    /// The file is removed first because `checkout-index` returns without doing anything when
    /// the index's stat information already matches what is on disk, which it does: `git add`
    /// recorded it a moment ago. `-u` has git record the new stat itself, and without it every
    /// resolved file is reported modified with an empty diff from then on.
    async fn checkout_form(&self, runner: &GitRunner, path: &str) {
        let full = self.display_path().join(path);
        if let Err(e) = std::fs::remove_file(&full) {
            tracing::warn!(error = %e, path, "could not replace the resolved file");
            return;
        }
        if let Err(e) = runner
            .output(
                GitCommand::write("checkout-index", self.display_path())
                    .args(["checkout-index", "-u", "-f", "--"])
                    .arg(path),
            )
            .await
        {
            // A smudge filter that fails is git's own business: it warns and writes the blob
            // unconverted. Reaching here means git could not write the file at all, and
            // nothing is there now, so put back what the index holds rather than a hole.
            tracing::warn!(error = %e, path, "git would not write the resolved file back");
            self.write_index_form(runner, path, &full).await;
        }
    }

    async fn write_index_form(&self, runner: &GitRunner, path: &str, full: &std::path::Path) {
        let read = runner
            .output(
                GitCommand::read("cat-file", self.display_path())
                    .args(["cat-file", "blob"])
                    .arg(format!(":0:{path}")),
            )
            .await;
        let put_back = read.and_then(|o| std::fs::write(full, o.stdout).map_err(CoralError::from));
        if let Err(e) = put_back {
            tracing::warn!(error = %e, path, "could not restore the resolved file");
        }
    }

    fn write_worktree(&self, path: &str, content: &BString) -> Result<(), CoralError> {
        let full = self.display_path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, content)?;
        Ok(())
    }

    /// Stages a file the user has already edited, without changing its content.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn mark_resolved(&self, runner: &GitRunner, path: &str) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("add", self.display_path())
                    .args(["add", "--"])
                    .arg(path),
            )
            .await
            .map(|_| ())
    }
}
