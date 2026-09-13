use std::path::{Path, PathBuf};

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner, GitVersion};

/// Where HEAD points.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Head {
    Branch {
        name: String,
    },
    Detached {
        oid: String,
    },
    /// A branch that exists symbolically but has no commit yet — a fresh `git init`.
    Unborn {
        name: String,
    },
}

/// The multi-step operation the repository is in the middle of, detected from the files git
/// leaves in the git dir rather than from porcelain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "snake_case")]
pub enum OpState {
    Clean,
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

/// The paths that identify a repository. Cheap to clone and carried by every engine task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoLocation {
    /// Worktree root. `None` for a bare repository.
    pub workdir: Option<PathBuf>,
    /// This worktree's git dir — `.git/worktrees/<name>` for a linked worktree.
    pub git_dir: PathBuf,
    /// The shared git dir. Differs from `git_dir` only in linked worktrees; refs and objects
    /// live here.
    pub common_dir: PathBuf,
    pub is_bare: bool,
}

/// What `coral open` reports.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "types.ts"))]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub path: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub is_bare: bool,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub git_version: GitVersion,
    pub head: Head,
    pub state: OpState,
    /// Whether a commit-graph is present. Its absence is the single biggest predictor of a
    /// slow first paint on a large repository.
    pub commit_graph: bool,
}

/// Whether a repository is still at `at`.
///
/// Cheap and synchronous, for lists of paths recorded earlier: [`RepoLocation::discover`] runs
/// git and walks upward, which would report the parent of a directory someone deleted. A
/// worktree has `.git`, a bare repository has `HEAD` in the directory itself.
#[must_use]
pub fn present(at: &Path) -> bool {
    at.join(".git").exists() || at.join("HEAD").exists()
}

impl RepoLocation {
    /// Resolves `path` to a repository.
    ///
    /// # Errors
    /// [`CoralError::NotARepository`] if `path` is not inside a git repository.
    pub async fn discover(runner: &GitRunner, path: &Path) -> Result<Self, CoralError> {
        // Somebody pointing at a repository often points at its `.git`, and a file chooser that
        // shows hidden entries puts that one click away. git will not name a work tree from
        // inside one: `--show-toplevel` exits "this operation must be run in a work tree", and
        // that was the whole answer — a tab called `.git` reporting a failure about a directory
        // the user did not mean. The repository is the one holding it. A bare repository is
        // named for itself rather than `.git`, and a linked worktree's git dir is deeper, so
        // neither arrives here.
        let path = match path.file_name() {
            Some(name) if name == ".git" => path.parent().unwrap_or(path),
            _ => path,
        };
        let out = runner
            .output(
                GitCommand::read("rev-parse", path)
                    .args(["rev-parse", "--path-format=absolute"])
                    .args(["--git-dir", "--git-common-dir", "--is-bare-repository"]),
            )
            .await
            .map_err(|e| match e {
                // git exits 128 with "not a git repository"; anything else is a real failure.
                CoralError::GitExit { code: 128, .. } => {
                    CoralError::NotARepository(path.to_path_buf())
                }
                other => other,
            })?;

        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        let git_dir = PathBuf::from(next_line(&mut lines, "git-dir")?);
        let common_dir = PathBuf::from(next_line(&mut lines, "git-common-dir")?);
        let is_bare = next_line(&mut lines, "is-bare-repository")? == "true";

        let workdir = if is_bare {
            None
        } else {
            Some(Self::toplevel(runner, path).await?)
        };
        Ok(Self {
            workdir,
            git_dir,
            common_dir,
            is_bare,
        })
    }

    async fn toplevel(runner: &GitRunner, path: &Path) -> Result<PathBuf, CoralError> {
        let out = runner
            .output(GitCommand::read("rev-parse", path).args([
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
            ]))
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        Ok(PathBuf::from(text.trim_end_matches(['\n', '\r'])))
    }

    /// The directory the UI shows for this repository.
    #[must_use]
    pub fn display_path(&self) -> &Path {
        self.workdir.as_deref().unwrap_or(&self.git_dir)
    }

    /// Resolves a path inside this worktree's git dir.
    #[must_use]
    pub fn git_path(&self, name: &str) -> PathBuf {
        self.git_dir.join(name)
    }

    /// True when the shared object store carries a commit-graph.
    #[must_use]
    pub fn has_commit_graph(&self) -> bool {
        let info = self.common_dir.join("objects").join("info");
        info.join("commit-graph").exists() || info.join("commit-graphs").is_dir()
    }

    /// Reads the in-progress operation from the git dir. Faster and more reliable than
    /// porcelain, which only tells you that files are unmerged.
    /// Whether the stopped operation is `git am` rather than a rebase.
    ///
    /// Both leave `rebase-apply` behind, and git tells them apart by a marker inside it: `am`
    /// writes `applying`, a rebase on the same backend writes `rebasing`. The distinction is
    /// invisible in the window — the two stop the same way and are settled the same way — but
    /// it decides which subcommand continues them, and `git rebase --continue` during an `am`
    /// fails saying no rebase is in progress.
    #[must_use]
    pub fn applying_patches(&self) -> bool {
        self.git_path("rebase-apply/applying").exists()
    }

    #[must_use]
    pub fn op_state(&self) -> OpState {
        // rebase-merge covers interactive and merge-backend rebases; rebase-apply covers the
        // am-backend and `git am` itself.
        if self.git_path("rebase-merge").is_dir() || self.git_path("rebase-apply").is_dir() {
            OpState::Rebase
        } else if self.git_path("MERGE_HEAD").exists() {
            OpState::Merge
        } else if self.git_path("CHERRY_PICK_HEAD").exists() {
            OpState::CherryPick
        } else if self.git_path("REVERT_HEAD").exists() {
            OpState::Revert
        } else if self.git_path("BISECT_LOG").exists() {
            OpState::Bisect
        } else {
            OpState::Clean
        }
    }

    /// Reads HEAD, distinguishing an unborn branch from a detached one.
    ///
    /// # Errors
    /// Propagates unexpected git failures.
    pub async fn head(&self, runner: &GitRunner) -> Result<Head, CoralError> {
        let dir = self.display_path();
        let symbolic = runner
            .output(GitCommand::read("symbolic-ref", dir).args([
                "symbolic-ref",
                "--quiet",
                "--short",
                "HEAD",
            ]))
            .await;

        match symbolic {
            Ok(out) => {
                let name = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if self.resolve_rev(runner, "HEAD").await?.is_some() {
                    Ok(Head::Branch { name })
                } else {
                    Ok(Head::Unborn { name })
                }
            }
            // Exit 1 from symbolic-ref means HEAD is not symbolic, i.e. detached.
            Err(CoralError::GitExit { code: 1, .. }) => {
                let oid = self.resolve_rev(runner, "HEAD").await?.ok_or_else(|| {
                    CoralError::Protocol {
                        label: "symbolic-ref",
                        detail: "HEAD is neither symbolic nor resolvable".to_owned(),
                    }
                })?;
                Ok(Head::Detached { oid })
            }
            Err(other) => Err(other),
        }
    }

    /// Resolves a revision to a full oid, or `None` when it does not exist.
    async fn resolve_rev(
        &self,
        runner: &GitRunner,
        rev: &str,
    ) -> Result<Option<String>, CoralError> {
        let out = runner
            .output(GitCommand::read("rev-parse", self.display_path()).args([
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ]))
            .await;
        match out {
            Ok(o) => Ok(Some(String::from_utf8_lossy(&o.stdout).trim().to_owned())),
            // `--quiet` turns "no such revision" into a silent exit 1.
            Err(CoralError::GitExit { code: 1, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Reads the working tree state.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn status(&self, runner: &GitRunner) -> Result<crate::status::Status, CoralError> {
        let out = runner
            .output(GitCommand::status("status", self.display_path()).args([
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--show-stash",
            ]))
            .await?;
        crate::status::Status::parse(&out.stdout)
    }

    /// Lists every ref, with upstream tracking for local branches.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn refs(&self, runner: &GitRunner) -> Result<Vec<crate::refs::GitRef>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("for-each-ref", self.display_path())
                    .arg("for-each-ref")
                    .arg(format!("--format={}", crate::refs::FORMAT)),
            )
            .await?;
        crate::refs::parse(&out.stdout)
    }

    /// Diffs the worktree, or the index when `staged`, optionally limited to `paths`.
    ///
    /// Three git invocations rather than one: numstat gives unambiguous paths and counts,
    /// name-status distinguishes add from delete from rename, and the patch supplies hunks.
    /// The patch header cannot be trusted for paths — `diff --git a/x b/y` is ambiguous when a
    /// path contains a space, which git does not quote.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn diff(
        &self,
        runner: &GitRunner,
        staged: bool,
        paths: &[&str],
        options: crate::diff::DiffOptions,
    ) -> Result<Vec<crate::diff::FileDiff>, CoralError> {
        let base = |args: &[&str]| {
            let mut c = GitCommand::status("diff", self.display_path())
                .arg("diff")
                .arg("-M");
            if staged {
                c = c.arg("--cached");
            }
            c = c.args(args);
            if paths.is_empty() {
                c
            } else {
                c.arg("--").args(paths)
            }
        };

        let numstat = runner.output(base(&["-z", "--numstat"])).await?;
        let mut files = crate::diff::parse_numstat(&numstat.stdout)?;
        if files.is_empty() {
            return Ok(files);
        }

        let names = runner.output(base(&["-z", "--name-status"])).await?;
        crate::diff::apply_name_status(&mut files, &names.stdout)?;

        let patch = runner.output(base(&options.flags())).await?;
        crate::diff::apply_patch(&mut files, &patch.stdout, options)?;
        crate::diff::recount(&mut files, options);

        // What the repository holds for a path in Git LFS is a pointer of three lines, so its
        // diff is two changed lines of pointer. Staging one of them left the index holding a
        // pointer with no object named in it at all.
        let lfs = self
            .in_lfs(runner, files.iter().map(|f| f.path.as_slice()))
            .await?;
        for file in files
            .iter_mut()
            .filter(|f| lfs.contains(&f.path.to_string()))
        {
            file.binary = true;
            file.added = None;
            file.removed = None;
            file.hunks.clear();
        }
        Ok(files)
    }

    /// The whole of an untracked file, as a diff against nothing.
    ///
    /// `git diff` knows about tracked paths only, so a file git has never seen produces no
    /// diff at all — and the panel said the file had no changes, about a file that is nothing
    /// but change. `--no-index` compares two paths on disk instead of consulting the index,
    /// which is what shows the file added end to end.
    ///
    /// `Ok(None)` for a path git already tracks, or one it is ignoring: the caller cannot know
    /// which it has, and answering with the whole file for a tracked one would claim every
    /// line of it was new.
    ///
    /// # Errors
    /// Propagates git failures. Exit 1 is not one: `--no-index` uses it to say the two paths
    /// differ, which is the reason it was run.
    pub async fn untracked_diff(
        &self,
        runner: &GitRunner,
        path: &str,
        options: crate::diff::DiffOptions,
    ) -> Result<Option<crate::diff::FileDiff>, CoralError> {
        let listed = runner
            .output(
                GitCommand::read("ls-files", self.display_path())
                    .args(["ls-files", "-z", "--others", "--exclude-standard", "--"])
                    .arg(path),
            )
            .await?;
        if listed.stdout.is_empty() {
            return Ok(None);
        }

        // "/dev/null" is a literal git recognises on every platform it builds for, not a path
        // it opens, so this works on Windows too.
        let out = runner
            .output_allowing(
                GitCommand::status("diff", self.display_path())
                    .arg("diff")
                    .arg("--no-index")
                    .args(options.flags())
                    .args(["--", "/dev/null"])
                    .arg(path),
                &[1],
            )
            .await?;

        let binary = bstr::ByteSlice::find(out.stdout.as_slice(), b"\nBinary files ").is_some();
        let mut file = crate::diff::FileDiff {
            path: path.into(),
            old_path: None,
            change: crate::diff::FileChange::Added,
            binary,
            added: None,
            removed: None,
            hunks: Vec::new(),
            mode: None,
            too_large: false,
        };
        crate::diff::apply_patch(std::slice::from_mut(&mut file), &out.stdout, options)?;
        if !binary {
            let added = file
                .hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .filter(|l| l.kind == crate::diff::LineKind::Add)
                .count();
            file.added = Some(u32::try_from(added).unwrap_or(u32::MAX));
            file.removed = Some(0);
        }
        Ok(Some(file))
    }

    /// Diffs one commit against its first parent, optionally limited to `paths`.
    ///
    /// Same three invocations and the same reasoning as [`RepoLocation::diff`], against
    /// `diff-tree` rather than the worktree. `--root` is what makes the initial commit show
    /// its contents instead of nothing.
    ///
    /// The merge flag is `--diff-merges=first-parent`, not `-m --first-parent`: on `diff-tree`
    /// the latter emits a diff against *every* parent one after another, so a merge listed the
    /// union of both sides — every file the branch touched plus every file the mainline
    /// touched since it forked. `--first-parent` is a revision-walking option there and does
    /// not narrow `-m`.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn commit_diff(
        &self,
        runner: &GitRunner,
        rev: &str,
        paths: &[&str],
        options: crate::diff::DiffOptions,
    ) -> Result<Vec<crate::diff::FileDiff>, CoralError> {
        // `--diff-merges=first-parent` and `--root` belong to the one-commit form only: they
        // say how to find something to diff against, and a pair of commits already has one.
        self.tree_diff(
            runner,
            &["--diff-merges=first-parent", "--root"],
            &[rev],
            paths,
            options,
        )
        .await
    }

    /// Diffs one commit against another, optionally limited to `paths`.
    ///
    /// Trees, not the walk between them, so two commits on different branches compare as
    /// readily as two on the same one.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn compare_diff(
        &self,
        runner: &GitRunner,
        from: &str,
        to: &str,
        paths: &[&str],
        options: crate::diff::DiffOptions,
    ) -> Result<Vec<crate::diff::FileDiff>, CoralError> {
        self.tree_diff(runner, &[], &[from, to], paths, options)
            .await
    }

    /// The three invocations both tree diffs are built from.
    async fn tree_diff(
        &self,
        runner: &GitRunner,
        mode: &[&str],
        revs: &[&str],
        paths: &[&str],
        options: crate::diff::DiffOptions,
    ) -> Result<Vec<crate::diff::FileDiff>, CoralError> {
        let base = |args: &[&str]| {
            let c = GitCommand::read("diff-tree", self.display_path())
                .args(["diff-tree", "-r", "-M", "--no-commit-id"])
                .args(mode)
                .args(args)
                .args(revs);
            if paths.is_empty() {
                c
            } else {
                c.arg("--").args(paths)
            }
        };

        let numstat = runner.output(base(&["-z", "--numstat"])).await?;
        let mut files = crate::diff::parse_numstat(&numstat.stdout)?;
        if files.is_empty() {
            return Ok(files);
        }

        let names = runner.output(base(&["-z", "--name-status"])).await?;
        crate::diff::apply_name_status(&mut files, &names.stdout)?;

        let patch = runner.output(base(&options.flags())).await?;
        crate::diff::apply_patch(&mut files, &patch.stdout, options)?;
        crate::diff::recount(&mut files, options);
        Ok(files)
    }

    /// Reads commit history, optionally narrowed by [`crate::history::LogQuery`].
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] if the output does not parse.
    pub async fn log(
        &self,
        runner: &GitRunner,
        q: &crate::history::LogQuery,
    ) -> Result<Vec<crate::commit::Commit>, CoralError> {
        let mut cmd = GitCommand::read("log", self.display_path())
            .arg("log")
            .arg("-z")
            .arg(format!("--format={}", crate::history::FORMAT));

        if let Some(n) = q.limit {
            cmd = cmd.arg(format!("--max-count={n}"));
        }
        if let Some(a) = &q.author {
            cmd = cmd.arg(format!("--author={a}"));
        }
        if let Some(g) = &q.grep {
            cmd = cmd.args(["--grep", g]).arg("--fixed-strings");
        }
        if let Some(s) = &q.pickaxe {
            cmd = cmd.arg(format!("-S{s}"));
        }
        // --follow needs exactly one pathspec and must precede it.
        if q.follow && q.path.is_some() {
            cmd = cmd.arg("--follow");
        }
        cmd = cmd.arg(q.rev.as_deref().unwrap_or("HEAD"));
        if let Some(p) = &q.path {
            cmd = cmd.arg("--").arg(p);
        }

        let out = runner.output(cmd).await?;
        crate::history::parse(&out.stdout)
    }

    /// Commits whose message, author or object id matches `query`.
    ///
    /// Three passes rather than one, because git ANDs `--author` with `--grep` and there is no
    /// flag that ORs them: a search for a name would otherwise find nothing unless the name
    /// were also in the message. An abbreviated object id is resolved first and put at the
    /// front, since someone who pastes a hash means that commit and not a commit that mentions
    /// it.
    ///
    /// Every ref, not just HEAD: the graph shows every branch, so a search that did not would
    /// report nothing for a commit plainly on screen.
    ///
    /// # Errors
    /// Propagates git failures. A query that resolves to nothing is not one.
    pub async fn search_commits(
        &self,
        runner: &GitRunner,
        query: &str,
        limit: u64,
    ) -> Result<Vec<String>, CoralError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Together rather than one after the other. Each pass is a walk of every commit in
        // the repository — eleven seconds on the kernel — and run in sequence the window sat
        // on "searching…" for the sum of them.
        let (by_message, by_author, by_path) = tokio::try_join!(
            self.matching(runner, "--grep", query, limit),
            self.matching(runner, "--author", query, limit),
            self.touching_path(runner, query, limit),
        )?;

        let mut passes = Vec::new();
        if crate::history::looks_like_an_oid(query) {
            passes.push(self.resolve_oid(runner, query).await);
        }
        passes.push(by_message);
        passes.push(by_author);
        passes.push(by_path);

        let mut out = crate::history::merged(&passes);
        out.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(out)
    }

    /// The commit an abbreviation names, or nothing when it names none.
    async fn resolve_oid(&self, runner: &GitRunner, query: &str) -> Vec<String> {
        let out = runner
            .output(
                GitCommand::read("rev-parse", self.display_path())
                    .args(["rev-parse", "--verify", "--quiet"])
                    .arg(format!("{query}^{{commit}}")),
            )
            .await;
        match out {
            // Exit 1 with no output is how `--quiet` says the revision is unknown, which is
            // the ordinary answer for a half-typed hash rather than a failure.
            Err(_) => Vec::new(),
            Ok(found) => String::from_utf8_lossy(&found.stdout)
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        }
    }

    /// Commits that touched a file whose path holds `query`.
    ///
    /// A pathspec rather than `-S`. The two answer different questions and only one of them is
    /// the question a filter is asked: `-S` searches the *content* of every diff for a string,
    /// which on a repository of any size is minutes of work and returns every commit that
    /// happened to add or remove the word. What people mean by searching for `sidebar` is
    /// "which commits touched that file", and a pathspec answers it from the same index the
    /// walk already uses.
    ///
    /// `:(icase)` matches the way the other two passes do, and the `*` on each side means the
    /// query can be any part of the path rather than a whole one. A query carrying pathspec
    /// magic of its own is matched as the pattern it looks like, which is the ordinary risk of
    /// a glob and never worse than finding nothing.
    async fn touching_path(
        &self,
        runner: &GitRunner,
        query: &str,
        limit: u64,
    ) -> Result<Vec<String>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("log", self.display_path())
                    .args(["log", "--all", "--format=%H"])
                    .arg(format!("--max-count={limit}"))
                    .arg("--")
                    .arg(format!(":(icase)*{query}*")),
            )
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect())
    }

    async fn matching(
        &self,
        runner: &GitRunner,
        field: &str,
        query: &str,
        limit: u64,
    ) -> Result<Vec<String>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("log", self.display_path())
                    .args(["log", "--all", "--format=%H", "--fixed-strings", "-i"])
                    .arg(format!("--max-count={limit}"))
                    .arg(format!("{field}={query}")),
            )
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(str::to_owned)
            .collect())
    }

    /// Every file the repository holds at one revision, in git's own order.
    ///
    /// For the panel that lists a commit's files: what changed is the usual question, but
    /// "what was there" is the other one, and it cannot be answered from a diff.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown revision.
    pub async fn tree_files(
        &self,
        runner: &GitRunner,
        rev: &str,
    ) -> Result<Vec<String>, CoralError> {
        let out = runner
            .output(
                GitCommand::read("ls-tree", self.display_path())
                    .args(["ls-tree", "-r", "--name-only", "-z"])
                    .arg(rev),
            )
            .await?;
        // NUL-separated, because a path may hold anything a byte can and git quotes it
        // otherwise.
        Ok(out
            .stdout
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect())
    }

    /// One file's contents at one revision.
    ///
    /// Wanted by the blame view, which has chunks of lines attributed to commits and needs the
    /// lines themselves to put beside them. Read at the same revision the blame was taken at,
    /// or the two would not line up.
    ///
    /// # Errors
    /// Propagates git failures, including an unknown path at that revision.
    pub async fn file_at(
        &self,
        runner: &GitRunner,
        rev: &str,
        path: &str,
        was: Option<&str>,
    ) -> Result<Vec<u8>, CoralError> {
        let named = self.named_at(runner, rev, path, was).await?;
        let out = runner
            .output(
                GitCommand::read("show", self.display_path())
                    .arg("show")
                    .arg(format!("{rev}:{named}")),
            )
            .await?;
        Ok(out.stdout)
    }

    /// Which of the two names the file goes by at `rev`.
    ///
    /// The current one unless the revision predates the rename, which is the case a reader
    /// reaches by stepping back through a file's history.
    async fn named_at<'a>(
        &self,
        runner: &GitRunner,
        rev: &str,
        path: &'a str,
        was: Option<&'a str>,
    ) -> Result<&'a str, CoralError> {
        match was {
            Some(old) if old != path && !self.holds_path(runner, rev, path).await? => Ok(old),
            _ => Ok(path),
        }
    }

    /// Whether `rev` has a blob at `path`.
    ///
    /// `rev-parse --verify --quiet` answers with an exit code and nothing on stdout, and the
    /// runner treats a non-zero exit as an error, so the absence is read from the output being
    /// empty rather than from the failure.
    async fn holds_path(
        &self,
        runner: &GitRunner,
        rev: &str,
        path: &str,
    ) -> Result<bool, CoralError> {
        let out = runner
            .output(
                GitCommand::read("cat-file", self.display_path())
                    .args(["cat-file", "-t"])
                    .arg(format!("{rev}:{path}")),
            )
            .await;
        Ok(out.is_ok())
    }

    /// Attributes each line of a file to the commit that last changed it.
    ///
    /// Streams rather than buffers: `--incremental` emits chunks as it resolves them, which is
    /// what lets the UI paint a long file progressively.
    ///
    /// `was` is the name the file had before a rename, when the caller knows of one. Blame
    /// takes one path and one revision, and at a revision from before the rename the current
    /// name is not in the tree: reading a file's history and stepping back through it answered
    /// "no such path <current name> in <forty characters>" as soon as the reader asked who
    /// wrote a line.
    ///
    /// # Errors
    /// Propagates git failures and [`CoralError::Protocol`] on malformed output.
    pub async fn blame(
        &self,
        runner: &GitRunner,
        rev: &str,
        path: &str,
        was: Option<&str>,
    ) -> Result<crate::blame::Blame, CoralError> {
        let named = self.named_at(runner, rev, path, was).await?;
        let cmd = GitCommand::read("blame", self.display_path())
            .args(["blame", "--porcelain", "--incremental", rev, "--"])
            .arg(named);

        let mut parser = crate::blame::BlameParser::default();
        runner
            .stream(cmd, b'\n', |line| {
                parser.push(line)?;
                Ok(crate::process::Sink::Continue)
            })
            .await?;
        Ok(parser.finish())
    }

    /// Stages whole paths.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn stage(&self, runner: &GitRunner, paths: &[&str]) -> Result<(), CoralError> {
        let cmd = GitCommand::write("add", self.display_path())
            .args(["add", "--all", "--"])
            .args(paths);
        runner.output(cmd).await.map(|_| ())
    }

    /// Unstages whole paths, leaving the worktree alone.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn unstage(&self, runner: &GitRunner, paths: &[&str]) -> Result<(), CoralError> {
        // `restore --staged` needs a source; without a commit there is nothing to restore
        // from, so an unborn HEAD uses `rm --cached` instead.
        let unborn = matches!(self.head(runner).await?, Head::Unborn { .. });
        let cmd = if unborn {
            GitCommand::write("rm", self.display_path())
                .args(["rm", "--cached", "-r", "--quiet", "--"])
                .args(paths)
        } else {
            GitCommand::write("restore", self.display_path())
                .args(["restore", "--staged", "--"])
                .args(paths)
        };
        runner.output(cmd).await.map(|_| ())
    }

    /// Throws away worktree changes to `paths`. Staged content is left in the index.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn discard(&self, runner: &GitRunner, paths: &[&str]) -> Result<(), CoralError> {
        let cmd = GitCommand::write("restore", self.display_path())
            .args(["restore", "--worktree", "--"])
            .args(paths);
        runner.output(cmd).await.map(|_| ())
    }

    /// Puts `paths` back to what HEAD holds, in the index and in the working tree.
    ///
    /// What a client means by "discard", and deliberately not what [`discard`](Self::discard)
    /// does: leaving the index alone would put back a file the user had already staged, and
    /// the panel offers one button rather than two. A path staged as new is not in HEAD at
    /// all, so this unstages it and removes it, which is the same answer.
    ///
    /// Untracked paths must not be passed here. git refuses the whole invocation for one
    /// pathspec it does not know, so a single new file would stop the rest being discarded.
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn restore_from_head(
        &self,
        runner: &GitRunner,
        paths: &[&str],
    ) -> Result<(), CoralError> {
        if paths.is_empty() {
            return Ok(());
        }
        let cmd = GitCommand::write("restore", self.display_path())
            .args(["restore", "--source=HEAD", "--staged", "--worktree", "--"])
            .args(paths);
        runner.output(cmd).await.map(|_| ())
    }

    /// Removes tracked `paths` from the working tree, staging the deletion.
    ///
    /// `git rm -f`, so the file goes and the index is told: anything less leaves a deletion
    /// half-done, sitting in the panel as a change nobody asked for. Untracked paths are not
    /// git's to remove and go through [`remove_untracked`](Self::remove_untracked).
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn delete_tracked(
        &self,
        runner: &GitRunner,
        paths: &[&str],
    ) -> Result<(), CoralError> {
        if paths.is_empty() {
            return Ok(());
        }
        let cmd = GitCommand::write("rm", self.display_path())
            .args(["rm", "--force", "--"])
            .args(paths);
        runner.output(cmd).await.map(|_| ())
    }

    /// Deletes untracked files and directories under `paths`.
    ///
    /// Never `-x`: ignored paths are build output, caches and editor state that the user did
    /// not put there and is not being asked about. Always with explicit paths, so there is no
    /// invocation of this that means "everything".
    ///
    /// # Errors
    /// Propagates git failures.
    pub async fn remove_untracked(
        &self,
        runner: &GitRunner,
        paths: &[&str],
    ) -> Result<(), CoralError> {
        if paths.is_empty() {
            return Ok(());
        }
        let cmd = GitCommand::write("clean", self.display_path())
            .args(["clean", "--force", "-d", "--"])
            .args(paths);
        runner.output(cmd).await.map(|_| ())
    }

    /// Applies a generated patch to the index, for hunk and line staging.
    ///
    /// # Errors
    /// Propagates git failures; a patch that does not apply surfaces as
    /// [`CoralError::GitExit`] carrying git's own explanation.
    pub async fn apply_to_index(
        &self,
        runner: &GitRunner,
        patch: &bstr::BString,
        direction: crate::index::Direction,
    ) -> Result<(), CoralError> {
        if crate::index::is_empty_patch(patch) {
            return Ok(());
        }
        let cmd = GitCommand::write("apply", self.display_path())
            .args(crate::index::apply_args(direction))
            .stdin_bytes(patch.to_vec());
        runner.output(cmd).await.map(|_| ())
    }

    /// Gathers everything `coral open` reports.
    ///
    /// # Errors
    /// Propagates git failures from the HEAD lookup.
    pub async fn info(&self, runner: &GitRunner) -> Result<RepoInfo, CoralError> {
        Ok(RepoInfo {
            path: self.display_path().to_path_buf(),
            git_dir: self.git_dir.clone(),
            common_dir: self.common_dir.clone(),
            is_bare: self.is_bare,
            git_version: runner.version(),
            head: self.head(runner).await?,
            state: self.op_state(),
            commit_graph: self.has_commit_graph(),
        })
    }
}

fn next_line<'a>(
    lines: &mut std::str::Lines<'a>,
    field: &'static str,
) -> Result<&'a str, CoralError> {
    lines.next().ok_or(CoralError::Protocol {
        label: "rev-parse",
        detail: format!("missing {field} in output"),
    })
}
