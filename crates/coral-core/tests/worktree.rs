//! Linked working trees, and the patch file a commit exports to.

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;
use coral_core::worktree::parse_list;

fn run<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

async fn located(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

// git reports a path resolved and with forward slashes, and a temporary directory differs from
// it in two ways at once. macOS puts them under /var, a symlink to /private/var. Windows hands
// out the 8.3 short name, C:\Users\RUNNER~1, where git says runneradmin. Canonicalising settles
// both, and the extended prefix Windows adds when it resolves has to come back off, because git
// never produces one. Separators are left to Path, which compares by component.
fn resolved(path: &std::path::Path) -> std::path::PathBuf {
    let full = std::fs::canonicalize(path).expect("resolve");
    #[cfg(windows)]
    if let Some(rest) = full.to_string_lossy().strip_prefix(r"\\?\") {
        return std::path::PathBuf::from(rest);
    }
    full
}

// The parent is what gets resolved, because the worktree path does not exist until git makes it.
fn under(dir: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    resolved(dir.path()).join(name)
}

fn two_commits() -> TestRepo {
    TestRepo::new()
        .write("a.txt", "a\n")
        .commit("first commit")
        .write("b.txt", "b\n")
        .commit("second commit")
}

#[test]
fn the_porcelain_listing_survives_a_path_with_spaces_in_it() {
    // Which is the whole reason for using it rather than the plain listing, where the fields
    // are separated by whitespace and a path like this splits into three.
    let out = b"worktree /home/dev/my repo\nHEAD abc123\nbranch refs/heads/main\n\n\
                worktree /tmp/wt\nHEAD def456\ndetached\n\n";
    let list = parse_list(out);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].path, "/home/dev/my repo");
    assert_eq!(list[0].branch.as_deref(), Some("main"));
    assert_eq!(list[1].path, "/tmp/wt");
    assert_eq!(list[1].branch, None, "a detached tree has no branch");
    assert_eq!(list[1].head, "def456");
}

#[test]
fn a_record_without_a_trailing_blank_line_is_still_read() {
    let list = parse_list(b"worktree /a\nHEAD abc\nbranch refs/heads/main");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].branch.as_deref(), Some("main"));
}

#[test]
fn a_new_worktree_checks_a_commit_out_without_moving_the_current_one() {
    let repo = two_commits();
    let first = repo.git(["rev-parse", "HEAD~1"]);
    let elsewhere = tempfile::tempdir().unwrap();
    let at = under(&elsewhere, "older");

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.worktree_add(&runner, &at, &first, None).await.unwrap();

        let list = loc.worktrees(&runner).await.unwrap();
        assert_eq!(list.len(), 2, "{list:?}");
        assert!(list.iter().any(|w| std::path::Path::new(&w.path) == at), "{list:?}");
    });

    // The new tree holds the older commit, and the original is where it was.
    assert!(at.join("a.txt").exists());
    assert!(
        !at.join("b.txt").exists(),
        "the worktree should be at the first commit"
    );
    assert_eq!(repo.git(["rev-parse", "HEAD"]).len(), 40);
    assert!(repo.path().join("b.txt").exists());
}

#[test]
fn a_worktree_can_be_given_a_branch_of_its_own() {
    let repo = two_commits();
    let first = repo.git(["rev-parse", "HEAD~1"]);
    let elsewhere = tempfile::tempdir().unwrap();
    let at = under(&elsewhere, "feature");

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.worktree_add(&runner, &at, &first, Some("from-commit"))
            .await
            .unwrap();
        let list = loc.worktrees(&runner).await.unwrap();
        let made = list
            .iter()
            .find(|w| std::path::Path::new(&w.path) == at)
            .expect("the new worktree is listed");
        assert_eq!(made.branch.as_deref(), Some("from-commit"));
    });
    assert_eq!(repo.git(["rev-parse", "from-commit"]), first);
}

#[test]
fn removing_a_worktree_leaves_the_repository_alone() {
    let repo = two_commits();
    let head = repo.git(["rev-parse", "HEAD"]);
    let elsewhere = tempfile::tempdir().unwrap();
    let at = under(&elsewhere, "scratch");

    run(async {
        let (runner, loc) = located(&repo).await;
        loc.worktree_add(&runner, &at, "HEAD", None).await.unwrap();
        loc.worktree_remove(&runner, &at, false).await.unwrap();
        assert_eq!(loc.worktrees(&runner).await.unwrap().len(), 1);
    });
    assert_eq!(repo.git(["rev-parse", "HEAD"]), head);
}

#[test]
fn a_commit_exports_to_a_patch_file_named_for_its_summary() {
    let repo = two_commits();
    let out = tempfile::tempdir().unwrap();

    let written = run(async {
        let (runner, loc) = located(&repo).await;
        loc.format_patch(&runner, "HEAD", out.path()).await.unwrap()
    });

    assert_eq!(written.len(), 1, "{written:?}");
    let file = &written[0];
    assert!(file.exists(), "{}", file.display());
    assert!(
        file.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("second-commit")),
        "the name carries the summary: {}",
        file.display()
    );

    // It is a mailbox-format patch, which is what `git am` reads on the other end.
    let body = std::fs::read_to_string(file).unwrap();
    assert!(body.starts_with("From "), "{body:.80}");
    assert!(
        body.contains("Subject: [PATCH] second commit"),
        "{body:.400}"
    );
    assert!(body.contains("+++ b/b.txt"), "{body:.600}");
}
