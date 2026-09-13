use std::collections::HashSet;

use bstr::{BString, ByteSlice};

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
    /// Why there is nothing to pick between, when there is nothing. `None` is the ordinary
    /// text file, settled region by region.
    pub whole: Option<Whole>,
    /// One side deleted the file, so keeping or deleting is the only meaningful choice.
    pub delete_modify: bool,
}

/// Why a conflicted path has no lines of its own to choose between.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum Whole {
    /// A NUL byte in the first 8000 bytes, which is git's own rule.
    Binary,
    /// Git LFS holds it, so the repository has a pointer of a few lines rather than the file.
    Lfs,
    /// A submodule: the two sides are commits.
    Submodule,
    /// A symlink: each side points somewhere, and the file it points at is not the answer.
    Symlink,
}

impl ConflictedFile {
    /// True when the file can be resolved block by block rather than only wholesale.
    #[must_use]
    pub const fn supports_blocks(&self) -> bool {
        self.whole.is_none() && !self.delete_modify
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

/// The mode git gives a submodule's entry: its object is a commit.
const GITLINK: &str = "160000";
/// The mode git gives a symlink: its object is the path pointed at.
const SYMLINK: &[u8] = b"120000";

/// What the index records for one stage of a conflicted path.
struct Staged {
    mode: String,
    oid: String,
}

impl Staged {
    /// Reads one `git ls-files -u -z` record, `<mode> <oid> <stage>\t<path>`, if it is the
    /// stage asked for.
    fn parse(record: &[u8], stage: u8) -> Option<Self> {
        let (meta, _) = record.split_once_str("\t")?;
        let mut parts = meta.split_str(" ");
        let mode = parts.next()?;
        let oid = parts.next()?;
        if parts.next()? != [stage] {
            return None;
        }
        Some(Self {
            mode: String::from_utf8_lossy(mode).into_owned(),
            oid: String::from_utf8_lossy(oid).into_owned(),
        })
    }
}

impl RepoLocation {
    /// Lists the files still needing a decision.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn conflicts(&self, runner: &GitRunner) -> Result<Vec<ConflictedFile>, CoralError> {
        let status = self.status(runner).await?;
        let paths: Vec<String> = status.conflicted().map(|e| e.path.to_string()).collect();
        let lfs = self
            .in_lfs(runner, paths.iter().map(String::as_bytes))
            .await?;
        let symlinks = self.symlinks(runner).await?;

        let mut out = Vec::new();
        for entry in status.conflicted() {
            let path = entry.path.to_string();
            let kind = entry.conflict.unwrap_or(ConflictKind::BothModified);
            let delete_modify = matches!(
                kind,
                ConflictKind::DeletedByUs | ConflictKind::DeletedByThem | ConflictKind::BothDeleted
            );
            let whole = if entry.submodule {
                Some(Whole::Submodule)
            } else if symlinks.contains(&path) {
                Some(Whole::Symlink)
            } else if lfs.contains(&path) {
                Some(Whole::Lfs)
            } else if !delete_modify && self.stage_is_binary(runner, &path).await {
                Some(Whole::Binary)
            } else {
                None
            };
            out.push(ConflictedFile {
                path,
                kind,
                whole,
                delete_modify,
            });
        }
        Ok(out)
    }

    /// The conflicted paths that are symlinks on either side.
    ///
    /// One read of the unmerged index for all of them. What such a path holds is the path it
    /// points at, and writing that out as a file both loses the link and, if the old link is
    /// still there, puts the new target inside whatever the old one pointed at.
    async fn symlinks(&self, runner: &GitRunner) -> Result<HashSet<String>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("ls-files", self.display_path()).args(["ls-files", "-u", "-z"]),
            )
            .await?;
        Ok(out
            .stdout
            .split(|b| *b == 0)
            .filter_map(|record| {
                let (meta, path) = record.split_once_str("\t")?;
                meta.starts_with(SYMLINK)
                    .then(|| String::from_utf8_lossy(path).into_owned())
            })
            .collect())
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
                return self.take_side(runner, path, take).await;
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

    /// Settles a conflicted path on one whole side.
    ///
    /// The index is written first and the file laid down from it, rather than the other way
    /// round. That keeps the mode, so a symlink stays a symlink: writing the stage's bytes
    /// into the worktree made a file out of one, and while the old link was still there it
    /// put the new target inside whatever the old one pointed at — then `git add` recorded the
    /// unchanged link, so the side asked for was not even the side staged.
    async fn take_side(
        &self,
        runner: &GitRunner,
        path: &str,
        take: Take,
    ) -> Result<(), CoralError> {
        let staged = self.staged(runner, path, take).await?;
        if staged.mode == GITLINK {
            return self.take_commit(runner, path, &staged.oid).await;
        }
        runner
            .output(
                GitCommand::write("update-index", self.display_path())
                    .args(["update-index", "--cacheinfo"])
                    .arg(format!("{},{},{path}", staged.mode, staged.oid)),
            )
            .await?;
        self.checkout_form(runner, path).await;
        Ok(())
    }

    /// One stage of a conflicted path as the index itself records it.
    ///
    /// `git checkout --ours` is deliberately not used. During a rebase it means the opposite
    /// of what the user picked, because stage 2 is the branch being rebased onto rather than
    /// their own work; going to the stage directly keeps the caller's choice honest whatever
    /// the operation.
    ///
    /// Read with `ls-files -u` rather than `cat-file`, because the mode is half the answer: a
    /// submodule's stages are commits, and asking for their content gets an error instead of a
    /// file — which is what left a conflicted submodule reporting no content on either side
    /// and offering deletion as the way out of it.
    ///
    /// # Errors
    /// [`CoralError::Refused`] when that side has no stage at all.
    async fn staged(
        &self,
        runner: &GitRunner,
        path: &str,
        take: Take,
    ) -> Result<Staged, CoralError> {
        let wanted = match take {
            Take::Base => b'1',
            Take::Ours => b'2',
            Take::Theirs => b'3',
        };
        let out = runner
            .output(
                GitCommand::read("ls-files", self.display_path())
                    .args(["ls-files", "-u", "-z", "--"])
                    .arg(path),
            )
            .await?;

        out.stdout
            .split(|b| *b == 0)
            .find_map(|record| Staged::parse(record, wanted))
            .ok_or_else(|| CoralError::Refused {
                label: "resolve",
                detail: format!("{path} has no content on that side; delete it instead"),
            })
    }

    /// Settles a conflicted submodule on one of the two commits.
    ///
    /// The submodule's own checkout moves first, so the directory beside the index agrees with
    /// it and `git add` records exactly what is there. Settling only the index would leave the
    /// superproject calling the submodule modified the moment it was resolved.
    async fn take_commit(
        &self,
        runner: &GitRunner,
        path: &str,
        oid: &str,
    ) -> Result<(), CoralError> {
        runner
            .output(
                GitCommand::write("checkout", self.display_path().join(path))
                    .args(["checkout", "--quiet", "--detach"])
                    .arg(oid),
            )
            .await?;
        runner
            .output(
                GitCommand::write("add", self.display_path())
                    .args(["add", "--"])
                    .arg(path),
            )
            .await
            .map(|_| ())
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
        // Taken away rather than written over, because the path may be a symlink and writing
        // would go through it into the file it points at.
        match std::fs::remove_file(&full) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
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
