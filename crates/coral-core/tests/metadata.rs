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

#[tokio::test]
async fn commit_detail_reports_the_message_and_the_files_it_changed() {
    let repo = TestRepo::new()
        .write("keep.txt", "1\n")
        .write("gone.txt", "x\n")
        .commit("base");
    let repo = repo.write("keep.txt", "2\n").write("added.txt", "new\n");
    repo.git(["rm", "--quiet", "gone.txt"]);
    repo.git(["add", "--all"]);
    repo.git(["commit", "--quiet", "-m", "a change", "-m", "with a body"]);

    let (runner, loc) = open(&repo).await;
    let detail = loc.commit_detail(&runner, "HEAD").await.unwrap();

    assert_eq!(detail.commit.summary, "a change");
    assert_eq!(detail.commit.body, "with a body");
    assert_eq!(detail.commit.parents.len(), 1);

    let mut paths: Vec<String> = detail.files.iter().map(|f| f.path.to_string()).collect();
    paths.sort();
    assert_eq!(paths, vec!["added.txt", "gone.txt", "keep.txt"]);

    let find = |p: &str| detail.files.iter().find(|f| f.path == p).unwrap();
    assert_eq!(
        find("added.txt").change,
        coral_core::diff::FileChange::Added
    );
    assert_eq!(
        find("gone.txt").change,
        coral_core::diff::FileChange::Deleted
    );
    assert_eq!(
        find("keep.txt").change,
        coral_core::diff::FileChange::Modified
    );
}

/// `diff-tree` reports nothing at all for a merge without `-m`, so a merge would look like it
/// changed no files — the most common commit in a busy repository showing an empty panel.
#[tokio::test]
async fn a_merge_reports_its_first_parent_changes_rather_than_nothing() {
    let repo = TestRepo::new().write("base.txt", "base\n").commit("base");
    repo.git(["checkout", "--quiet", "-b", "side"]);
    let repo = repo.write("from-side.txt", "s\n").commit("side work");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("from-main.txt", "m\n").commit("main work");
    repo.git([
        "merge",
        "--quiet",
        "--no-ff",
        "--no-edit",
        "-m",
        "the merge",
        "side",
    ]);

    let (runner, loc) = open(&repo).await;
    let detail = loc.commit_detail(&runner, "HEAD").await.unwrap();

    assert_eq!(detail.commit.parents.len(), 2);
    assert!(detail.commit.is_merge());
    let paths: Vec<String> = detail.files.iter().map(|f| f.path.to_string()).collect();
    assert!(paths.contains(&"from-side.txt".to_owned()), "got {paths:?}");
}

/// The initial commit has no parent to diff against, so without `--root` it reports nothing.
#[tokio::test]
async fn the_root_commit_lists_every_file_it_introduced() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .write("b.txt", "2\n")
        .commit("first");

    let (runner, loc) = open(&repo).await;
    let detail = loc.commit_detail(&runner, "HEAD").await.unwrap();

    assert!(detail.commit.parents.is_empty());
    let mut paths: Vec<String> = detail.files.iter().map(|f| f.path.to_string()).collect();
    paths.sort();
    assert_eq!(paths, vec!["a.txt", "b.txt"]);
    assert!(
        detail
            .files
            .iter()
            .all(|f| f.change == coral_core::diff::FileChange::Added)
    );
}

#[tokio::test]
async fn a_rename_carries_both_paths() {
    let repo = TestRepo::new()
        .write("old.txt", "content that stays the same\n")
        .commit("base");
    repo.git(["mv", "old.txt", "new.txt"]);
    repo.git(["commit", "--quiet", "-m", "rename it"]);

    let (runner, loc) = open(&repo).await;
    let detail = loc.commit_detail(&runner, "HEAD").await.unwrap();

    let f = &detail.files[0];
    assert_eq!(f.change, coral_core::diff::FileChange::Renamed);
    assert_eq!(f.path, "new.txt");
    assert_eq!(
        f.old_path.as_ref().map(ToString::to_string).as_deref(),
        Some("old.txt")
    );
}

#[tokio::test]
async fn an_unknown_revision_is_refused() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("base");
    let (runner, loc) = open(&repo).await;

    assert!(
        loc.commit_detail(&runner, "0000000000000000000000000000000000000000")
            .await
            .is_err()
    );
}

/// The graph shows the body dimmed after the summary, so a window carries a preview of it.
#[tokio::test]
async fn the_body_preview_follows_the_summary() {
    let repo = TestRepo::new().write("a.txt", "1\n");
    repo.git(["add", "--all"]);
    repo.git([
        "commit",
        "--quiet",
        "-m",
        "the subject",
        "-m",
        "first line\nsecond line",
    ]);

    let head = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &[head]).await.unwrap();

    assert_eq!(meta[0].summary, "the subject");
    assert!(meta[0].body.to_string().contains("first line"));
    assert!(meta[0].body.to_string().contains("second line"));
}

#[tokio::test]
async fn a_commit_with_no_body_previews_nothing() {
    let repo = TestRepo::new()
        .write("a.txt", "1\n")
        .commit("just a subject");
    let head = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;

    let meta = loc.commit_metadata(&runner, &[head]).await.unwrap();
    assert_eq!(meta[0].summary, "just a subject");
    assert!(meta[0].body.to_string().trim().is_empty());
}

/// A long body is truncated, and never in the middle of a character — a split multi-byte
/// character renders as a replacement glyph in the row.
#[tokio::test]
async fn a_long_body_is_cut_on_a_character_boundary() {
    let long = "é".repeat(400);
    let repo = TestRepo::new().write("a.txt", "1\n");
    repo.git(["add", "--all"]);
    repo.git(["commit", "--quiet", "-m", "subject", "-m", &long]);

    let head = repo.git(["rev-parse", "HEAD"]);
    let (runner, loc) = open(&repo).await;
    let meta = loc.commit_metadata(&runner, &[head]).await.unwrap();

    assert!(meta[0].body.len() <= 300, "the preview is bounded");
    assert!(
        std::str::from_utf8(&meta[0].body).is_ok(),
        "and is still valid UTF-8, so it renders as text rather than as U+FFFD"
    );
}
