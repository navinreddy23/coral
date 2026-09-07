//! Remotes, driven against bare repositories on disk. No network is involved, but every code
//! path — refspecs, progress parsing, porcelain results — is the one used against a real host.

use std::sync::{Arc, Mutex};

use coral_core::process::GitRunner;
use coral_core::remote::{PullMode, PushFlag, PushOpts, parse_progress, parse_push};
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// A second working copy of a bare repository, standing in for another person.
///
/// `TestRepo` cannot be used: it initialises its directory, and git refuses to clone into one
/// that is not empty.
struct Clone {
    dir: tempfile::TempDir,
}

impl Clone {
    fn of(origin: &std::path::Path) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let c = Self { dir };
        c.run(&["clone", "--quiet", origin.to_str().unwrap(), "work"]);
        c.git(&["config", "user.name", "Other"]);
        c.git(&["config", "user.email", "other@coral.test"]);
        c
    }

    fn path(&self) -> std::path::PathBuf {
        self.dir.path().join("work")
    }

    fn write(&self, rel: &str, contents: &str) {
        std::fs::write(self.path().join(rel), contents).expect("write");
    }

    fn git(&self, args: &[&str]) {
        let out = std::process::Command::new("git")
            .current_dir(self.path())
            .args(args)
            .envs(hermetic())
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn run(&self, args: &[&str]) {
        let out = std::process::Command::new("git")
            .current_dir(self.dir.path())
            .args(args)
            .envs(hermetic())
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn hermetic() -> Vec<(&'static str, &'static str)> {
    vec![
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_SYSTEM", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", "Other"),
        ("GIT_AUTHOR_EMAIL", "other@coral.test"),
        ("GIT_COMMITTER_NAME", "Other"),
        ("GIT_COMMITTER_EMAIL", "other@coral.test"),
        ("LC_ALL", "C"),
    ]
}

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// A working repository with a bare "origin" that already has `main`.
///
/// The bare repository lives in its own directory, never inside the worktree: `git add --all`
/// would otherwise sweep it into the index, and every later push would dirty the worktree.
/// The returned `TempDir` owns it and must be kept alive.
fn with_origin() -> (TestRepo, tempfile::TempDir, std::path::PathBuf) {
    let repo = TestRepo::new().write("f.txt", "one\n").commit("first");
    let home = tempfile::tempdir().expect("temp dir");
    let origin = home.path().join("origin.git");

    repo.git(["init", "--quiet", "--bare", origin.to_str().unwrap()]);
    // `init --bare` leaves HEAD on master, so cloning a repo that only has main would produce
    // an unborn branch. Hosting providers set this; the fixture must too.
    repo.git([
        "--git-dir",
        origin.to_str().unwrap(),
        "symbolic-ref",
        "HEAD",
        "refs/heads/main",
    ]);
    repo.git(["remote", "add", "origin", origin.to_str().unwrap()]);
    repo.git(["push", "--quiet", "--set-upstream", "origin", "main"]);
    (repo, home, origin)
}

fn collector() -> (
    Arc<Mutex<Vec<coral_core::remote::Progress>>>,
    impl FnMut(coral_core::remote::Progress),
) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = {
        let seen = Arc::clone(&seen);
        move |p| {
            seen.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(p);
        }
    };
    (seen, sink)
}

#[tokio::test]
async fn lists_adds_renames_and_removes_remotes() {
    let repo = TestRepo::new().write("f.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    assert!(loc.remotes(&runner).await.unwrap().is_empty());

    loc.remote_add(&runner, "origin", "https://example.com/a.git")
        .await
        .unwrap();
    let remotes = loc.remotes(&runner).await.unwrap();
    assert_eq!(remotes.len(), 1, "one remote, not one per fetch/push line");
    assert_eq!(remotes[0].name, "origin");
    assert_eq!(remotes[0].fetch_url, "https://example.com/a.git");
    assert_eq!(remotes[0].push_url, remotes[0].fetch_url);

    loc.remote_set_url(&runner, "origin", "https://example.com/b.git")
        .await
        .unwrap();
    assert_eq!(
        loc.remotes(&runner).await.unwrap()[0].fetch_url,
        "https://example.com/b.git"
    );

    loc.remote_rename(&runner, "origin", "upstream")
        .await
        .unwrap();
    assert_eq!(loc.remotes(&runner).await.unwrap()[0].name, "upstream");

    loc.remote_remove(&runner, "upstream").await.unwrap();
    assert!(loc.remotes(&runner).await.unwrap().is_empty());
}

/// A separate push URL must not be reported as a second remote.
#[tokio::test]
async fn a_separate_push_url_is_reported_on_the_same_remote() {
    let repo = TestRepo::new().write("f.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;
    loc.remote_add(&runner, "origin", "https://example.com/read.git")
        .await
        .unwrap();
    repo.git([
        "remote",
        "set-url",
        "--push",
        "origin",
        "ssh://example.com/write.git",
    ]);

    let remotes = loc.remotes(&runner).await.unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].fetch_url, "https://example.com/read.git");
    assert_eq!(remotes[0].push_url, "ssh://example.com/write.git");
}

#[tokio::test]
async fn pushes_a_new_branch_and_reports_it() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;
    loc.branch_create(&runner, "feature", None, true)
        .await
        .unwrap();
    let repo = repo.write("f.txt", "feature work\n").commit("feature");

    let (seen, sink) = collector();
    let results = loc
        .push(
            &runner,
            &PushOpts {
                remote: Some("origin".into()),
                refspec: Some("feature".into()),
                set_upstream: true,
                ..PushOpts::default()
            },
            sink,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].flag, PushFlag::New);
    assert_eq!(results[0].remote, "refs/heads/feature");
    assert!(!results[0].flag.is_failure());

    // The branch really is on the other side.
    let listed = repo.git([
        "--git-dir",
        origin.to_str().unwrap(),
        "branch",
        "--list",
        "feature",
    ]);
    assert!(listed.contains("feature"));
    // Progress may be empty for a tiny push, but must never contain a malformed entry.
    assert!(seen.lock().unwrap().iter().all(|p| p.total > 0));
}

#[tokio::test]
async fn an_up_to_date_push_is_reported_rather_than_failing() {
    let (repo, _home, _origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    let (_, sink) = collector();
    let results = loc
        .push(
            &runner,
            &PushOpts {
                remote: Some("origin".into()),
                refspec: Some("main".into()),
                ..PushOpts::default()
            },
            sink,
        )
        .await
        .unwrap();

    assert_eq!(results[0].flag, PushFlag::UpToDate);
}

/// A rejected ref is an outcome the user acts on, not a crash.
#[tokio::test]
async fn a_non_fast_forward_push_reports_the_rejection() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    // Someone else moves the remote branch on.
    let other = Clone::of(&origin);
    other.write("f.txt", "theirs\n");
    other.git(&["commit", "--quiet", "-am", "their work"]);
    other.git(&["push", "--quiet", "origin", "main"]);

    // We commit on top of the old tip and try to push.
    let repo = repo.write("f.txt", "ours\n").commit("our work");
    let (_, sink) = collector();
    let results = loc
        .push(
            &runner,
            &PushOpts {
                remote: Some("origin".into()),
                refspec: Some("main".into()),
                ..PushOpts::default()
            },
            sink,
        )
        .await
        .unwrap();

    assert!(
        results.iter().any(|r| r.flag == PushFlag::Rejected),
        "{results:?}"
    );
    let _ = repo;
}

/// The whole loop a rejected push puts someone through, end to end.
///
/// Push refused, pull with a rebase that stops on the conflict, resolve it, continue, push
/// again. Every part of this has a test of its own; what this one is about is that they fit
/// together, because in the window they are four buttons pressed in a row.
#[tokio::test]
async fn a_rejected_push_is_settled_by_rebasing_and_pushing_again() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    let other = Clone::of(&origin);
    other.write("f.txt", "theirs\n");
    other.git(&["commit", "--quiet", "-am", "their work"]);
    other.git(&["push", "--quiet", "origin", "main"]);

    let repo = repo.write("f.txt", "ours\n").commit("our work");
    let opts = || PushOpts {
        remote: Some("origin".into()),
        refspec: Some("main".into()),
        ..PushOpts::default()
    };

    let (_, sink) = collector();
    let refused = loc.push(&runner, &opts(), sink).await.unwrap();
    assert!(refused.iter().any(|r| r.flag == PushFlag::Rejected));

    // Pulling with a rebase replays our commit onto theirs, and the same line stops it.
    let stopped = loc
        .pull(&runner, Some("origin"), PullMode::Rebase, |_| {})
        .await
        .unwrap();
    assert!(!stopped.completed, "it stopped on the conflict");
    assert_eq!(stopped.conflicts, vec!["f.txt"]);

    loc.resolve(
        &runner,
        "f.txt",
        &coral_core::conflict::Resolution::Content("theirs\nours\n".into()),
    )
    .await
    .unwrap();
    let done = loc
        .op(&runner, coral_core::ops::OpAction::Continue)
        .await
        .unwrap();
    assert!(done.completed);

    let (_, sink) = collector();
    let accepted = loc.push(&runner, &opts(), sink).await.unwrap();
    assert!(
        accepted.iter().all(|r| r.flag != PushFlag::Rejected),
        "{accepted:?}"
    );
    assert_eq!(
        repo.git(["rev-parse", "HEAD"]),
        repo.git(["rev-parse", "origin/main"]),
        "and the remote is where we are"
    );
}

/// A tag reaches the remote only when it is pushed: `git push` alone sends none.
#[tokio::test]
async fn pushes_one_tag_by_name() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;
    repo.git([
        "tag",
        "--annotate",
        "v1.0",
        "--message",
        "the first release",
    ]);

    let (_, sink) = collector();
    let results = loc
        .push(
            &runner,
            &PushOpts {
                remote: Some("origin".into()),
                refspec: Some("refs/tags/v1.0".into()),
                ..PushOpts::default()
            },
            sink,
        )
        .await
        .unwrap();

    assert!(results.iter().all(|r| !r.flag.is_failure()), "{results:?}");
    let there = std::process::Command::new("git")
        .args(["ls-remote", "--tags", origin.to_str().unwrap()])
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&there.stdout).into_owned();
    assert!(listed.contains("refs/tags/v1.0"), "{listed}");
}

/// The other half of it: every tag at once, which is what the toolbar offers.
#[tokio::test]
async fn pushes_every_tag_at_once() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;
    repo.git(["tag", "v1.0"]);
    repo.git(["tag", "v1.1"]);

    let (_, sink) = collector();
    loc.push(
        &runner,
        &PushOpts {
            remote: Some("origin".into()),
            tags: true,
            ..PushOpts::default()
        },
        sink,
    )
    .await
    .unwrap();

    let there = std::process::Command::new("git")
        .args(["ls-remote", "--tags", origin.to_str().unwrap()])
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&there.stdout).into_owned();
    assert!(listed.contains("refs/tags/v1.0"), "{listed}");
    assert!(listed.contains("refs/tags/v1.1"), "{listed}");
}

#[tokio::test]
async fn fetch_brings_down_new_commits_and_reports_progress() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    // Enough objects that git bothers to report progress.
    let other = Clone::of(&origin);
    for i in 0..40 {
        other.write(&format!("n{i}.txt"), &format!("{i}\n"));
        other.git(&["add", "--all"]);
        other.git(&["commit", "--quiet", "-m", &format!("commit {i}")]);
    }
    other.git(&["push", "--quiet", "origin", "main"]);

    let before = repo.git(["rev-parse", "origin/main"]);
    let (seen, sink) = collector();
    loc.fetch(&runner, Some("origin"), true, sink)
        .await
        .unwrap();
    let after = repo.git(["rev-parse", "origin/main"]);

    assert_ne!(before, after, "the tracking branch moved");
    let progress = seen.lock().unwrap().clone();
    assert!(
        !progress.is_empty(),
        "a 40-commit fetch should report progress"
    );
    assert!(
        progress
            .iter()
            .all(|p| p.current <= p.total && p.percent <= 100)
    );
    assert!(progress.iter().any(|p| p.done), "a phase should finish");
}

#[tokio::test]
async fn fetch_all_and_prune_drop_a_deleted_branch() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    loc.branch_create(&runner, "temp", None, false)
        .await
        .unwrap();
    loc.push(
        &runner,
        &PushOpts {
            remote: Some("origin".into()),
            refspec: Some("temp".into()),
            ..PushOpts::default()
        },
        |_| {},
    )
    .await
    .unwrap();
    loc.fetch(&runner, Some("origin"), false, |_| {})
        .await
        .unwrap();
    assert!(repo.git(["branch", "-r"]).contains("origin/temp"));

    // Delete it on the other side, then prune.
    repo.git([
        "--git-dir",
        origin.to_str().unwrap(),
        "branch",
        "-D",
        "temp",
    ]);
    loc.fetch(&runner, None, true, |_| {}).await.unwrap();
    assert!(
        !repo.git(["branch", "-r"]).contains("origin/temp"),
        "prune removed it"
    );
}

#[tokio::test]
async fn pull_fast_forwards_and_refuses_to_diverge() {
    let (repo, _home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    let other = Clone::of(&origin);
    other.write("new.txt", "theirs\n");
    other.git(&["add", "--all"]);
    other.git(&["commit", "--quiet", "-m", "their work"]);
    other.git(&["push", "--quiet", "origin", "main"]);

    let outcome = loc
        .pull(&runner, Some("origin"), PullMode::FfOnly, |_| {})
        .await
        .unwrap();
    assert!(outcome.completed);
    assert!(
        repo.path().join("new.txt").exists(),
        "the pull brought the file down"
    );

    // Now diverge and try again: fast-forward must refuse.
    let repo = repo.write("f.txt", "ours\n").commit("our work");
    other.write("another.txt", "more\n");
    other.git(&["add", "--all"]);
    other.git(&["commit", "--quiet", "-m", "more of theirs"]);
    other.git(&["push", "--quiet", "origin", "main"]);

    assert!(
        loc.pull(&runner, Some("origin"), PullMode::FfOnly, |_| {})
            .await
            .is_err()
    );
    // Rebasing does integrate them.
    let rebased = loc
        .pull(&runner, Some("origin"), PullMode::Rebase, |_| {})
        .await
        .unwrap();
    assert!(rebased.completed);
    assert!(repo.path().join("another.txt").exists());
}

#[test]
fn parses_progress_lines_git_actually_emits() {
    let p = parse_progress(b"Counting objects:  11% (1/9)").unwrap();
    assert_eq!(p.phase, "Counting objects");
    assert_eq!((p.current, p.total, p.percent), (1, 9, 11));
    assert!(!p.remote && !p.done);

    let p =
        parse_progress(b"Writing objects: 100% (9/9), 585 bytes | 585.00 KiB/s, done.").unwrap();
    assert_eq!(p.phase, "Writing objects");
    assert_eq!((p.current, p.total, p.percent), (9, 9, 100));
    assert!(p.done, "the trailing done. marks the end of a phase");

    // Server-side phases arrive prefixed and padded.
    let p = parse_progress(b"remote: Compressing objects: 100% (2/2), done.        ").unwrap();
    assert!(p.remote);
    assert_eq!(p.phase, "Compressing objects");
}

#[test]
fn ignores_stderr_that_is_not_progress() {
    assert!(parse_progress(b"Delta compression using up to 16 threads").is_none());
    assert!(parse_progress(b"To github.com:o/r.git").is_none());
    assert!(parse_progress(b"").is_none());
    assert!(parse_progress(b"error: failed to push some refs").is_none());
    assert!(parse_progress(b"Total 9 (delta 1), reused 0 (delta 0)").is_none());
}

#[test]
fn parses_every_push_flag() {
    // Built line by line: a string literal's line-continuation escape strips leading
    // whitespace, and a leading space *is* the flag for a plain fast-forward.
    let out = [
        "To ../origin.git",
        "*\trefs/heads/new:refs/heads/new\t[new branch]",
        " \trefs/heads/ff:refs/heads/ff\t",
        "+\trefs/heads/forced:refs/heads/forced\t(forced update)",
        "-\t:refs/heads/gone\t[deleted]",
        "=\trefs/heads/same:refs/heads/same\t[up to date]",
        "!\trefs/heads/no:refs/heads/no\t[rejected] (non-fast-forward)",
        "Done",
    ]
    .join("\n");
    let out = out.as_bytes();

    let results = parse_push(out);

    let flags: Vec<_> = results.iter().map(|r| r.flag).collect();
    assert_eq!(
        flags,
        vec![
            PushFlag::New,
            PushFlag::Ok,
            PushFlag::Forced,
            PushFlag::Deleted,
            PushFlag::UpToDate,
            PushFlag::Rejected,
        ],
        "the To header and trailing Done are not results"
    );
    assert!(results[5].flag.is_failure());
    assert!(results[5].summary.contains("non-fast-forward"));
    assert_eq!(results[3].local, "", "a deletion has no local side");
}

/// The first push of a branch has to say where it goes.
///
/// `--set-upstream` on its own fails with "the current branch has no upstream branch", which is
/// exactly the case the flag is for: git needs the remote and the branch named as well.
#[test]
fn a_first_push_sets_its_own_upstream() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let upstream = TestRepo::new().write("a.txt", "1\n").commit("first");
        let bare = tempfile::tempdir().unwrap();
        upstream.git(["init", "--bare", "--quiet", &bare.path().to_string_lossy()]);

        let repo = TestRepo::new().write("a.txt", "1\n").commit("first");
        repo.git(["remote", "add", "origin", &bare.path().to_string_lossy()]);
        repo.git(["checkout", "--quiet", "-b", "topic"]);
        let repo = repo.write("b.txt", "2\n").commit("on the branch");

        let runner = GitRunner::discover().await.unwrap();
        let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();

        let results = loc
            .push(
                &runner,
                &PushOpts {
                    set_upstream: true,
                    ..PushOpts::default()
                },
                |_| {},
            )
            .await
            .unwrap();
        assert!(!results.is_empty(), "nothing was pushed");

        // The branch is on the remote, and tracking it.
        let refs = repo.git(["ls-remote", "--heads", "origin"]);
        assert!(refs.contains("refs/heads/topic"), "the branch is not there");
        assert_eq!(
            repo.git(["rev-parse", "--abbrev-ref", "topic@{upstream}"]),
            "origin/topic"
        );
    });
}

/// Pulling from a remote that is not the branch's upstream.
///
/// `git pull <remote>` works out which branch to integrate from the current branch's
/// upstream, so naming a second remote answered "there is no tracking information for the
/// current branch" and integrated nothing — while the fetch inside it had already succeeded,
/// which made the failure read as a network problem. That is every pull from a mirror, and
/// pulling from a named remote is the only reason to name one.
#[tokio::test]
async fn a_pull_from_a_remote_that_is_not_the_upstream_still_integrates() {
    let (repo, home, origin) = with_origin();
    let (runner, loc) = open(&repo).await;

    // A second remote holding the same history, and a commit that only it has.
    let mirror = home.path().join("mirror.git");
    repo.git(["init", "--quiet", "--bare", mirror.to_str().unwrap()]);
    repo.git([
        "--git-dir",
        mirror.to_str().unwrap(),
        "symbolic-ref",
        "HEAD",
        "refs/heads/main",
    ]);
    repo.git(["remote", "add", "mirror", mirror.to_str().unwrap()]);
    repo.git(["push", "--quiet", "mirror", "main"]);

    let other = Clone::of(&mirror);
    other.write("f.txt", "from the mirror\n");
    other.git(&["commit", "--quiet", "-am", "the mirror's own commit"]);
    other.git(&["push", "--quiet", "origin", "main"]);

    // The branch still tracks origin, which has not moved.
    let out = loc
        .pull(&runner, Some("mirror"), PullMode::FfOnly, |_| {})
        .await
        .expect("a pull from a named remote is not an error");
    assert!(out.completed, "it should have fast-forwarded");

    assert_eq!(
        std::fs::read_to_string(repo.path().join("f.txt")).expect("read"),
        "from the mirror\n",
        "the mirror's commit is the one that landed"
    );
    let _ = origin;
}

/// Seeding an empty repository from a remote by name.
///
/// A branch with no commit on it has no upstream by definition, so this is the case the
/// naming exists for and the one most likely to be someone's first minute with a repository.
#[tokio::test]
async fn a_pull_into_an_unborn_branch_names_it_too() {
    let (source, home, origin) = with_origin();
    let _ = source;

    // A fresh repository with nothing in it, pointed at the same bare one.
    let empty = TestRepo::new();
    empty.git(["remote", "add", "origin", origin.to_str().unwrap()]);
    let (runner, loc) = open(&empty).await;

    let out = loc
        .pull(&runner, Some("origin"), PullMode::FfOnly, |_| {})
        .await
        .expect("a pull that seeds a repository is not an error");
    assert!(out.completed, "it should have taken the remote's history");
    assert!(
        empty.path().join("f.txt").exists(),
        "the remote's file is in the working tree"
    );
    let _ = home;
}

#[tokio::test]
async fn a_failed_transfer_says_why_rather_than_reciting_its_progress() {
    // git writes progress to stderr with carriage returns, so the last few kilobytes of a
    // failed fetch are almost entirely "Receiving objects: 41% (76/185)". The sentence that
    // explains the failure is the last thing in there, and reporting the lot buries it past
    // anywhere it will be read.
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    repo.git(["remote", "add", "nowhere", "/does/not/exist/anywhere.git"]);

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let failed = loc.fetch(&runner, Some("nowhere"), false, |_| {}).await;

    let message = failed
        .expect_err("a remote that is not there cannot be fetched")
        .to_string();
    assert!(
        !message.contains('\r'),
        "a carriage return means the redraws were kept: {message}"
    );
    assert!(
        message.contains("does not appear to be a git repository")
            || message.contains("Could not read from remote"),
        "the reason survives: {message}"
    );
}
