//! Placing refs on graph rows. The interesting case is an annotated tag, whose ref points at a
//! tag object that is not in the graph at all — only its peeled commit is.

use coral_core::graph::{GixCommitStream, StreamOpts, Tips, build};
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// Mirrors the body of the `repo_refs` command.
async fn place(repo: &TestRepo) -> Vec<(String, Option<u32>)> {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let refs = loc.refs(&runner).await.unwrap();

    let stream = GixCommitStream::open(repo.path()).unwrap();
    let store = build(&stream, &StreamOpts::default()).unwrap();

    refs.into_iter()
        .map(|r| {
            let row = gix::ObjectId::from_hex(r.commit().as_bytes())
                .ok()
                .and_then(|id| store.row_of(&id));
            (r.short, row)
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn every_branch_and_tag_lands_on_a_row() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("first");
    repo.git(["branch", "feature"]);
    repo.git(["tag", "light"]);
    repo.git(["tag", "-a", "heavy", "-m", "an annotated tag"]);

    let placed = place(&repo).await;
    let find = |n: &str| placed.iter().find(|(s, _)| s == n).map(|(_, r)| *r);

    assert_eq!(find("main"), Some(Some(0)));
    assert_eq!(find("feature"), Some(Some(0)));
    assert_eq!(find("light"), Some(Some(0)));
    assert_eq!(
        find("heavy"),
        Some(Some(0)),
        "an annotated tag points at a tag object, so it must be placed by its peeled commit"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn refs_on_different_commits_land_on_different_rows() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("older");
    repo.git(["branch", "behind"]);
    let repo = repo.write("a.txt", "2\n").commit("newer");

    let placed = place(&repo).await;
    let row = |n: &str| placed.iter().find(|(s, _)| s == n).and_then(|(_, r)| *r);

    assert_eq!(row("main"), Some(0), "the tip is the first row");
    assert_eq!(row("behind"), Some(1));
}

/// A ref whose commit is not in the loaded graph must report no row rather than row zero,
/// which would label the wrong commit.
#[tokio::test(flavor = "multi_thread")]
async fn a_ref_outside_the_loaded_graph_has_no_row() {
    let repo = TestRepo::new().write("a.txt", "1\n").commit("first");
    // An orphan branch is a separate root; walking only main leaves it unplaced.
    repo.git(["checkout", "--quiet", "--orphan", "elsewhere"]);
    repo.git(["rm", "-rf", "--quiet", "--cached", "."]);
    let repo = repo.write("o.txt", "o\n").commit("orphan work");
    repo.git(["checkout", "--quiet", "main"]);

    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    let refs = loc.refs(&runner).await.unwrap();

    // Walk only main, deliberately excluding the orphan.
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let opts = StreamOpts {
        tips: Tips::Only(vec!["refs/heads/main".into()]),
        ..StreamOpts::default()
    };
    let store = build(&stream, &opts).unwrap();

    let orphan = refs
        .iter()
        .find(|r| r.short == "elsewhere")
        .expect("the orphan branch");
    let row = gix::ObjectId::from_hex(orphan.commit().as_bytes())
        .ok()
        .and_then(|id| store.row_of(&id));
    assert_eq!(
        row, None,
        "it must be unplaced, not placed on the wrong commit"
    );
}
