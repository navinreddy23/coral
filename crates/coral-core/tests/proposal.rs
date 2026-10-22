use coral_core::process::GitRunner;
use coral_core::proposal::{CheckedOut, Proposal};
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// A host with `main`, and proposal 7's tip a commit ahead of it on a branch the host keeps
/// only as `refs/pull/7/head`, the way GitHub keeps a fork's.
fn host() -> TestRepo {
    let host = TestRepo::new().write("a.txt", "a\n").commit("start");
    host.git(["checkout", "--quiet", "-b", "contributor"]);
    let host = host.write("b.txt", "b\n").commit("the contribution");
    let tip = host.git(["rev-parse", "HEAD"]);
    host.git(["update-ref", "refs/pull/7/head", tip.trim()]);
    host.git(["update-ref", "refs/merge-requests/7/head", tip.trim()]);
    host.git(["checkout", "--quiet", "main"]);
    host.git(["branch", "--quiet", "-D", "contributor"]);
    host
}

/// A clone of `host` as `origin`, on `main`.
fn local(host: &TestRepo) -> TestRepo {
    let local = TestRepo::new();
    local.git(["remote", "add", "origin", host.path().to_str().unwrap()]);
    local.git(["fetch", "--quiet", "origin"]);
    local.git(["checkout", "--quiet", "-B", "main", "origin/main"]);
    local
}

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

fn head(repo: &TestRepo) -> (String, String) {
    (
        repo.git(["symbolic-ref", "--short", "HEAD"])
            .trim()
            .to_owned(),
        repo.git(["log", "-1", "--format=%s"]).trim().to_owned(),
    )
}

/// Moves proposal 7 on the host to a new commit on top of where it was.
fn push_to_proposal(host: &TestRepo, file: &str) {
    let tip = host.git(["rev-parse", "refs/pull/7/head"]);
    host.git(["checkout", "--quiet", "--detach", tip.trim()]);
    std::fs::write(host.path().join(file), "more\n").unwrap();
    host.git(["add", "--all"]);
    host.git(["commit", "--quiet", "-m", "a fix after review"]);
    let new = host.git(["rev-parse", "HEAD"]);
    host.git(["update-ref", "refs/pull/7/head", new.trim()]);
    host.git(["checkout", "--quiet", "main"]);
}

#[tokio::test]
async fn checks_a_pull_request_out_as_its_own_branch_and_again_changes_nothing() {
    let host = host();
    let local = local(&host);
    let (runner, loc) = open(&local).await;

    let did = loc
        .checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap();
    assert_eq!(did, CheckedOut::Created);
    assert_eq!(head(&local), ("pr/7".into(), "the contribution".into()));
    // Kept beside the remote's own branches, so the graph shows where the request is.
    assert!(local.git(["rev-parse", "origin/pr/7"]).trim().len() >= 40);

    let again = loc
        .checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap();
    assert_eq!(again, CheckedOut::Unchanged);
}

#[tokio::test]
async fn brings_the_branch_up_to_a_request_that_moved_on() {
    let host = host();
    let local = local(&host);
    let (runner, loc) = open(&local).await;
    loc.checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap();

    // Checked out: fast-forwarded where it stands.
    push_to_proposal(&host, "c.txt");
    let did = loc
        .checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap();
    assert_eq!(did, CheckedOut::Updated);
    assert_eq!(head(&local), ("pr/7".into(), "a fix after review".into()));

    // Not checked out: moved, then checked out.
    local.git(["checkout", "--quiet", "main"]);
    push_to_proposal(&host, "d.txt");
    let did = loc
        .checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap();
    assert_eq!(did, CheckedOut::Updated);
    assert_eq!(head(&local).0, "pr/7");
    assert!(local.path().join("d.txt").exists());
}

#[tokio::test]
async fn leaves_alone_a_branch_that_would_lose_commits() {
    let host = host();
    let local = local(&host);
    let (runner, loc) = open(&local).await;
    loc.checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap();
    // A commit made here, on the request's branch.
    std::fs::write(local.path().join("mine.txt"), "mine\n").unwrap();
    local.git(["add", "--all"]);
    local.git(["commit", "--quiet", "-m", "my own change"]);
    // And the request force-pushed to something else entirely.
    let main = host.git(["rev-parse", "main"]);
    host.git(["update-ref", "refs/pull/7/head", main.trim()]);

    let err = loc
        .checkout_proposal(&runner, "origin", Proposal::Pull(7), |_| {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), "refused");
    assert!(err.to_string().contains("force-pushed"), "{err}");
    assert_eq!(
        head(&local),
        ("pr/7".into(), "my own change".into()),
        "untouched"
    );
}

#[tokio::test]
async fn checks_a_gitlab_merge_request_out_by_its_own_name() {
    let host = host();
    let local = local(&host);
    let (runner, loc) = open(&local).await;
    loc.checkout_proposal(&runner, "origin", Proposal::MergeRequest(7), |_| {})
        .await
        .unwrap();
    assert_eq!(head(&local), ("mr/7".into(), "the contribution".into()));
}

#[tokio::test]
async fn a_request_the_host_does_not_have_is_an_error_and_moves_nothing() {
    let host = host();
    let local = local(&host);
    let (runner, loc) = open(&local).await;
    assert!(
        loc.checkout_proposal(&runner, "origin", Proposal::Pull(99), |_| {})
            .await
            .is_err()
    );
    assert_eq!(head(&local).0, "main");
}
