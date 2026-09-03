//! The windowed metadata read. The commit-graph carries neither author nor message, so this is
//! the only way the graph gets them, and it runs on every scroll.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn reads_author_and_summary_for_a_window() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("first commit");
    let repo = repo.write("a.txt", "2\n").commit("second commit");
    let oids: Vec<String> = repo
        .git(["rev-list", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();

    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &oids).await.unwrap();

    assert_eq!(meta.len(), 2);
    assert_eq!(meta[0].summary, "second commit");
    assert_eq!(meta[1].summary, "first commit");
    assert_eq!(meta[0].author, "Coral Fixture");
    assert_eq!(meta[0].email, "fixture@coral.test");
    assert!(meta[0].time > 0);
    assert_eq!(meta[0].oid, oids[0]);
}

/// A merge of an annotated tag embeds a `mergetag` header whose continuation lines are
/// indented and contain blank lines of their own. Treating one of those as the end of the
/// headers would make the tag's body the commit summary.
#[tokio::test]
async fn a_mergetag_header_does_not_become_the_summary() {
    let repo = TestRepo::new().write("a.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("s.txt", "s\n").commit("side work");
    repo.git(["tag", "-a", "v1", "-m", "release one\n\nwith a body\n"]);
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("m.txt", "m\n").commit("main work");
    repo.git([
        "merge",
        "--quiet",
        "--no-ff",
        "--no-edit",
        "-m",
        "merge the tag",
        "v1",
    ]);

    let head = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &[head]).await.unwrap();

    assert_eq!(meta[0].summary, "merge the tag");
    assert_eq!(meta[0].author, "Coral Fixture");
}

#[tokio::test]
async fn an_author_name_containing_spaces_and_the_email_are_separated() {
    let repo = TestRepo::new().write("a.txt", "1\n");
    repo.git(["add", "--all"]);
    // The fixture pins GIT_AUTHOR_NAME, and an environment variable beats `-c user.name`.
    // `--author` beats both.
    repo.git([
        "commit",
        "--quiet",
        "--author=Ada King Lovelace <ada@example.com>",
        "-m",
        "by ada",
    ]);

    let head = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &[head]).await.unwrap();

    assert_eq!(meta[0].author, "Ada King Lovelace");
    assert_eq!(meta[0].email, "ada@example.com");
}

#[tokio::test]
async fn an_empty_message_yields_an_empty_summary_rather_than_a_header() {
    let repo = TestRepo::new().write("a.txt", "1\n");
    repo.git(["add", "--all"]);
    repo.git(["commit", "--quiet", "--allow-empty-message", "-m", ""]);

    let head = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &[head]).await.unwrap();

    assert_eq!(meta.len(), 1);
    assert!(meta[0].summary.is_empty(), "not the author or tree line");
}

/// An id that does not resolve must not abandon the rest of the window.
#[tokio::test]
async fn a_missing_object_is_skipped_rather_than_failing_the_window() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("real commit");
    let head = repo.git(["rev-parse", "HEAD"]);
    let absent = "0".repeat(40);

    let (runner, loc) = open(&repo).await;
    let meta = loc
        .commit_metadata(&runner, &[absent, head.clone()])
        .await
        .unwrap();

    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].oid, head);
}

/// A tree or blob id in the window must not be reported as a commit.
#[tokio::test]
async fn non_commit_objects_are_ignored() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("only commit");
    let tree = repo.git(["rev-parse", "HEAD^{tree}"]);
    let head = repo.git(["rev-parse", "HEAD"]);

    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &[tree, head]).await.unwrap();

    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].summary, "only commit");
}

#[tokio::test]
async fn an_empty_window_costs_nothing() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    assert!(loc.commit_metadata(&runner, &[]).await.unwrap().is_empty());
}

/// The window is fed on stdin while output is read. Doing one before the other deadlocks as
/// soon as the child's output pipe fills, which a screenful of real commits does.
#[tokio::test]
async fn a_large_window_does_not_deadlock() {
    let mut repo = TestRepo::new().write("a.txt", "0\n").commit("commit 0");
    for i in 1..300 {
        repo = repo
            .write("a.txt", &format!("{i}\n"))
            .commit(&format!("commit {i}"));
    }
    let oids: Vec<String> = repo
        .git(["rev-list", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(oids.len(), 300);

    let (runner, loc) = open(&repo).await;
    let meta = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        loc.commit_metadata(&runner, &oids),
    )
    .await
    .expect("timed out, which means the pipes deadlocked")
    .unwrap();

    assert_eq!(meta.len(), 300);
    assert_eq!(meta[0].summary, "commit 299");
}
