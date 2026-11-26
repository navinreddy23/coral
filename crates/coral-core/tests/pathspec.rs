//! A path the window names means that one file, never a pattern.
//!
//! git reads a path argument as a glob, so `pages/[id].tsx` — a route file in any Next.js or
//! `SvelteKit` project — also stands for `pages/d.tsx` and `pages/i.tsx`. Each case puts the
//! file beside a sibling the glob would reach, acts on the file, and checks the sibling.

use coral_core::conflict::Resolution;
use coral_core::history::LogQuery;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

const ROUTE: &str = "pages/[id].tsx";
const SIBLING: &str = "pages/d.tsx";

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

fn routes() -> TestRepo {
    TestRepo::new()
        .write(ROUTE, "route\n")
        .write(SIBLING, "sibling\n")
        .commit("base")
}

fn read(repo: &TestRepo, path: &str) -> String {
    std::fs::read_to_string(repo.path().join(path)).unwrap()
}

#[tokio::test]
async fn discarding_a_route_file_keeps_the_changes_in_its_sibling() {
    let repo = routes().write(ROUTE, "edited\n").write(SIBLING, "mine\n");
    let (runner, loc) = open(&repo).await;

    loc.discard(&runner, &[ROUTE]).await.unwrap();

    assert_eq!(read(&repo, ROUTE), "route\n");
    assert_eq!(
        read(&repo, SIBLING),
        "mine\n",
        "an unrelated file lost its changes"
    );
}

#[tokio::test]
async fn putting_a_route_file_back_to_head_keeps_its_sibling() {
    let repo = routes().write(ROUTE, "edited\n").write(SIBLING, "mine\n");
    let (runner, loc) = open(&repo).await;

    loc.restore_from_head(&runner, &[ROUTE]).await.unwrap();

    assert_eq!(read(&repo, SIBLING), "mine\n");
}

#[tokio::test]
async fn taking_an_older_route_file_leaves_its_sibling() {
    let repo = routes()
        .write(ROUTE, "second\n")
        .write(SIBLING, "second\n")
        .commit("second")
        .write(SIBLING, "mine\n");
    let (runner, loc) = open(&repo).await;

    loc.restore_from(&runner, "HEAD~1", &[ROUTE]).await.unwrap();

    assert_eq!(read(&repo, ROUTE), "route\n");
    assert_eq!(read(&repo, SIBLING), "mine\n");
}

#[tokio::test]
async fn staging_and_unstaging_a_route_file_moves_only_that_file() {
    let repo = routes().write(ROUTE, "edited\n").write(SIBLING, "mine\n");
    let (runner, loc) = open(&repo).await;

    loc.stage(&runner, &[ROUTE]).await.unwrap();
    assert_eq!(repo.git(["diff", "--cached", "--name-only"]).trim(), ROUTE);

    loc.stage(&runner, &[SIBLING]).await.unwrap();
    loc.unstage(&runner, &[ROUTE]).await.unwrap();
    assert_eq!(
        repo.git(["diff", "--cached", "--name-only"]).trim(),
        SIBLING
    );
}

#[tokio::test]
async fn untracking_a_route_folder_leaves_the_folder_its_name_matches() {
    let repo = TestRepo::new()
        .write("pages/[id]/page.tsx", "route\n")
        .write("pages/i/page.tsx", "other\n")
        .commit("base");
    let (runner, loc) = open(&repo).await;

    loc.untrack(&runner, &["pages/[id]"]).await.unwrap();

    assert_eq!(
        repo.git(["ls-files", "pages"]).trim(),
        "pages/i/page.tsx",
        "only the named folder leaves the index"
    );
}

#[tokio::test]
async fn deleting_new_files_by_name_deletes_nothing_else() {
    let repo = routes()
        .write("pages/[slug].tsx", "new\n")
        .write("pages/s.tsx", "also new, and nobody asked\n")
        .write("pages/[u]/f.txt", "new\n")
        .write("pages/u/f.txt", "also new\n");
    let (runner, loc) = open(&repo).await;

    loc.remove_untracked(&runner, &["pages/[slug].tsx", "pages/[u]/"])
        .await
        .unwrap();

    assert!(!repo.path().join("pages/[slug].tsx").exists());
    assert!(!repo.path().join("pages/[u]").exists());
    assert!(
        repo.path().join("pages/s.tsx").exists(),
        "an untracked file nobody named was deleted"
    );
    assert!(repo.path().join("pages/u/f.txt").exists());
}

#[tokio::test]
async fn deleting_a_tracked_route_file_leaves_its_sibling() {
    let repo = routes();
    let (runner, loc) = open(&repo).await;

    loc.delete_tracked(&runner, &[ROUTE]).await.unwrap();

    assert!(!repo.path().join(ROUTE).exists());
    assert_eq!(read(&repo, SIBLING), "sibling\n");
}

#[tokio::test]
async fn a_route_files_diff_and_history_are_its_own() {
    let repo = routes()
        .write(SIBLING, "only the sibling\n")
        .commit("sibling only")
        .write(ROUTE, "edited\n")
        .write(SIBLING, "mine\n");
    let (runner, loc) = open(&repo).await;

    let diff = loc
        .diff(
            &runner,
            false,
            &[ROUTE],
            coral_core::diff::DiffOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(diff.len(), 1, "the sibling's change came with it");

    let history = loc
        .log(
            &runner,
            &LogQuery {
                path: Some(ROUTE.to_owned()),
                follow: true,
                ..LogQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        history.len(),
        1,
        "a commit that touched only the sibling is in its history"
    );
}

#[tokio::test]
async fn resolving_a_route_file_leaves_its_conflicted_sibling_unresolved() {
    let repo = routes();
    repo.git(["checkout", "-q", "-b", "side"]);
    let repo = repo
        .write(ROUTE, "side\n")
        .write(SIBLING, "side\n")
        .commit("side");
    repo.git(["checkout", "-q", "main"]);
    let repo = repo
        .write(ROUTE, "main\n")
        .write(SIBLING, "main\n")
        .commit("main");
    let _ = repo.command(["merge", "side"]).output().unwrap();
    let (runner, loc) = open(&repo).await;

    loc.resolve(&runner, ROUTE, &Resolution::TakeOurs)
        .await
        .unwrap();

    let unmerged = repo.git(["diff", "--name-only", "--diff-filter=U"]);
    assert_eq!(
        unmerged.trim(),
        SIBLING,
        "the sibling was marked resolved with its markers in it"
    );
}

#[tokio::test]
async fn marking_a_route_file_resolved_leaves_its_conflicted_sibling() {
    let repo = routes();
    repo.git(["checkout", "-q", "-b", "side"]);
    let repo = repo
        .write(ROUTE, "side\n")
        .write(SIBLING, "side\n")
        .commit("side");
    repo.git(["checkout", "-q", "main"]);
    let repo = repo
        .write(ROUTE, "main\n")
        .write(SIBLING, "main\n")
        .commit("main");
    let _ = repo.command(["merge", "side"]).output().unwrap();
    let repo = repo.write(ROUTE, "settled by hand\n");
    let (runner, loc) = open(&repo).await;

    loc.mark_resolved(&runner, ROUTE).await.unwrap();

    let unmerged = repo.git(["diff", "--name-only", "--diff-filter=U"]);
    assert_eq!(unmerged.trim(), SIBLING);
}
