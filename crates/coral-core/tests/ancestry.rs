//! Where a revision stands relative to HEAD, against real git.
//!
//! This is what decides the direction of every operation a ref menu offers, so getting it
//! backwards offers a fast-forward that cannot happen and hides the one that can.

use coral_core::process::GitRunner;
use coral_core::refs::Ancestry;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

/// master ahead of a tag and a branch, with one branch ahead of it and one diverged.
fn spread() -> TestRepo {
    let repo = TestRepo::new().write("f.txt", "1\n").commit("first");
    repo.git(["tag", "v1.0.0"]);
    let repo = repo.write("f.txt", "2\n").commit("second");
    repo.git(["branch", "behind", "HEAD~1"]);
    repo.git(["checkout", "--quiet", "-b", "sideways", "HEAD~1"]);
    let repo = repo.write("g.txt", "s\n").commit("a diverged commit");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("f.txt", "3\n").commit("third");
    repo.git(["checkout", "--quiet", "-b", "ahead"]);
    let repo = repo.write("f.txt", "4\n").commit("fourth");
    repo.git(["tag", "-a", "-m", "later", "v2.0.0"]);
    repo.git(["checkout", "--quiet", "main"]);
    repo
}

#[tokio::test]
async fn a_tag_the_branch_has_passed_is_behind() {
    // The reported bug: right-clicking v1.0.0 on a master that already contains it offered to
    // fast-forward master back to it, which git cannot do.
    let repo = spread();
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.ancestry(&runner, "v1.0.0").await.unwrap(),
        Ancestry::Behind
    );
    assert_eq!(
        loc.ancestry(&runner, "behind").await.unwrap(),
        Ancestry::Behind
    );
}

#[tokio::test]
async fn a_branch_with_commits_this_one_lacks_is_ahead() {
    let repo = spread();
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.ancestry(&runner, "ahead").await.unwrap(),
        Ancestry::Ahead
    );
}

#[tokio::test]
async fn an_annotated_tag_is_read_by_the_commit_it_points_at() {
    // Without `^{commit}` the comparison is against the tag object, which is an ancestor of
    // nothing, so every annotated tag came back diverged.
    let repo = spread();
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.ancestry(&runner, "v2.0.0").await.unwrap(),
        Ancestry::Ahead
    );
}

#[tokio::test]
async fn neither_containing_the_other_is_diverged() {
    let repo = spread();
    let (runner, loc) = open(&repo).await;
    assert_eq!(
        loc.ancestry(&runner, "sideways").await.unwrap(),
        Ancestry::Diverged
    );
}

#[tokio::test]
async fn the_commit_head_is_on_is_neither_ahead_nor_behind() {
    // `--is-ancestor` answers yes both ways for the same commit, which would read as ahead or
    // behind depending on which question was asked first.
    let repo = spread();
    let (runner, loc) = open(&repo).await;
    assert_eq!(loc.ancestry(&runner, "main").await.unwrap(), Ancestry::Same);
    assert_eq!(loc.ancestry(&runner, "HEAD").await.unwrap(), Ancestry::Same);
}

#[tokio::test]
async fn an_unknown_revision_is_an_error_rather_than_a_guess() {
    let repo = spread();
    let (runner, loc) = open(&repo).await;
    assert!(loc.ancestry(&runner, "no-such-ref").await.is_err());
}
