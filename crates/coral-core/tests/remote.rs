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
        .pull(&runner, Some("origin"), PullMode::FfOnly)
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
        loc.pull(&runner, Some("origin"), PullMode::FfOnly)
            .await
            .is_err()
    );
    // Rebasing does integrate them.
    let rebased = loc
        .pull(&runner, Some("origin"), PullMode::Rebase)
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
