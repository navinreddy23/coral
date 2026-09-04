use std::collections::{HashMap, HashSet};

use coral_core::graph::{
    CommitNode, CommitStream, GixCommitStream, Order, StreamOpts, SubprocessCommitStream,
    WalkControl,
};
use coral_core::process::GitRunner;
use coral_core::testutil::TestRepo;

fn collect(stream: &dyn CommitStream, opts: &StreamOpts) -> Vec<CommitNode> {
    let mut out = Vec::new();
    stream
        .walk(opts, &mut |n| {
            out.push(n);
            WalkControl::Continue
        })
        .unwrap_or_else(|e| panic!("{} walk failed: {e}", stream.name()));
    out
}

/// A DAG with a merge, an octopus, a criss-cross, and two roots — the shapes lane assignment
/// and topological ordering both have to survive.
fn gnarly_repo() -> TestRepo {
    let r = TestRepo::new().write("base.txt", "base\n").commit("base");

    r.git(["checkout", "--quiet", "-b", "a"]);
    let r = r.write("a.txt", "a\n").commit("a1");
    r.git(["checkout", "--quiet", "main"]);
    r.git(["checkout", "--quiet", "-b", "b"]);
    let r = r.write("b.txt", "b\n").commit("b1");
    r.git(["checkout", "--quiet", "main"]);
    r.git(["checkout", "--quiet", "-b", "c"]);
    let r = r.write("c.txt", "c\n").commit("c1");

    // Octopus: main takes a, b and c at once.
    r.git(["checkout", "--quiet", "main"]);
    r.git([
        "merge", "--quiet", "--no-ff", "-m", "octopus", "a", "b", "c",
    ]);

    // Criss-cross: two branches that each merge the other's tip.
    r.git(["checkout", "--quiet", "-b", "x", "a"]);
    let r = r.write("x.txt", "x\n").commit("x1");
    r.git(["checkout", "--quiet", "-b", "y", "b"]);
    let r = r.write("y.txt", "y\n").commit("y1");
    r.git(["merge", "--quiet", "--no-ff", "-m", "y-takes-x", "x"]);
    r.git(["checkout", "--quiet", "x"]);
    r.git(["merge", "--quiet", "--no-ff", "-m", "x-takes-b", "b"]);

    // A second root, unrelated to everything above.
    r.git(["checkout", "--quiet", "--orphan", "orphan"]);
    r.git(["rm", "-rf", "--quiet", "--cached", "."]);
    let r = r.write("orphan.txt", "o\n").commit("orphan root");
    r.git(["checkout", "--quiet", "main"]);
    r
}

async fn streams(path: &std::path::Path) -> (GixCommitStream, SubprocessCommitStream) {
    let runner = GitRunner::discover().await.unwrap();
    (
        GixCommitStream::open(path).unwrap(),
        SubprocessCommitStream::new(runner, path),
    )
}

/// The two implementations need not agree on tie-breaking, so comparing sequences would be
/// permanently flaky. What must match is the set of commits and each one's parents.
#[tokio::test(flavor = "multi_thread")]
async fn gix_and_rev_list_see_the_same_dag() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;
    let opts = StreamOpts::default();

    let a = collect(&gix, &opts);
    let b = collect(&sub, &opts);

    assert_eq!(a.len(), b.len(), "commit count differs");
    assert!(
        a.len() >= 10,
        "fixture should be non-trivial, got {}",
        a.len()
    );

    let as_map = |v: &[CommitNode]| -> HashMap<_, _> {
        v.iter().map(|n| (n.id, n.parents.to_vec())).collect()
    };
    assert_eq!(as_map(&a), as_map(&b), "commit set or parent lists differ");
}

/// Stopping after k emissions must leave a valid topological prefix; that property is what
/// lets the UI paint before the walk finishes.
#[tokio::test(flavor = "multi_thread")]
async fn topological_order_never_emits_a_parent_before_its_child() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        let nodes = collect(stream, &StreamOpts::default());
        let mut seen: HashSet<gix::ObjectId> = HashSet::new();
        for n in &nodes {
            for p in &n.parents {
                assert!(
                    !seen.contains(p),
                    "{}: parent {p} was emitted before its child {}",
                    stream.name(),
                    n.id
                );
            }
            seen.insert(n.id);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn max_count_bounds_the_walk_for_both_implementations() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;
    let opts = StreamOpts {
        max_count: Some(3),
        ..StreamOpts::default()
    };

    assert_eq!(collect(&gix, &opts).len(), 3);
    assert_eq!(collect(&sub, &opts).len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopping_sink_ends_the_walk_early() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        let mut count = 0;
        stream
            .walk(&StreamOpts::default(), &mut |_| {
                count += 1;
                if count == 2 {
                    WalkControl::Stop
                } else {
                    WalkControl::Continue
                }
            })
            .unwrap();
        assert_eq!(count, 2, "{} kept walking after Stop", stream.name());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn first_paint_order_returns_the_same_commit_set_as_topological() {
    let repo = gnarly_repo();
    let (gix, _) = streams(repo.path()).await;

    let topo: HashSet<_> = collect(&gix, &StreamOpts::default())
        .into_iter()
        .map(|n| n.id)
        .collect();
    let by_time: HashSet<_> = collect(
        &gix,
        &StreamOpts {
            order: Order::CommitTime,
            ..StreamOpts::default()
        },
    )
    .into_iter()
    .map(|n| n.id)
    .collect();

    assert_eq!(
        topo, by_time,
        "the provisional first paint must not lose or invent commits"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn first_parent_follows_only_the_mainline() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;
    let opts = StreamOpts {
        first_parent: true,
        tips: vec![],
        ..StreamOpts::default()
    };

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        let nodes = collect(stream, &opts);
        assert!(
            nodes.iter().all(|n| n.parents.len() <= 1),
            "{} emitted a node with multiple parents under first_parent",
            stream.name()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_repository_walks_to_nothing() {
    let repo = TestRepo::new();
    let (gix, sub) = streams(repo.path()).await;

    assert!(collect(&gix, &StreamOpts::default()).is_empty());
    assert!(collect(&sub, &StreamOpts::default()).is_empty());
}

#[test]
fn a_ref_pointing_at_an_object_that_cannot_be_read_is_refused() {
    // Skipping it produces a graph missing whole branches with nothing anywhere to say so.
    // The only symptom is a commit count that quietly disagrees with git's, which is not
    // something anyone notices without another client open beside it.
    let repo = TestRepo::new()
        .write("a.txt", "a\n")
        .commit("first")
        .write("b.txt", "b\n")
        .commit("second");

    // A ref naming an object the repository has never had.
    let absent = "0123456789abcdef0123456789abcdef01234567";
    std::fs::write(
        repo.path().join(".git/refs/heads/broken"),
        format!("{absent}\n"),
    )
    .unwrap();

    let stream = coral_core::graph::GixCommitStream::open(repo.path()).unwrap();
    let mut seen = 0_usize;
    let error = stream
        .walk(&coral_core::graph::StreamOpts::default(), &mut |_| {
            seen += 1;
            coral_core::graph::WalkControl::Continue
        })
        .unwrap_err();

    assert_eq!(error.code(), "protocol_error", "{error}");
    assert!(
        error.to_string().contains("broken"),
        "it names the ref: {error}"
    );
    assert_eq!(
        seen, 0,
        "nothing is emitted from a walk that cannot be complete"
    );
}

#[test]
fn a_tag_that_peels_to_a_tree_is_skipped_rather_than_refused() {
    // The kernel carries these. They are not commits and never were, so they are not a sign
    // that anything is wrong.
    let repo = TestRepo::new().write("a.txt", "a\n").commit("first");
    let tree = repo.git(["rev-parse", "HEAD^{tree}"]);
    repo.git(["tag", "a-tree-tag", &tree]);

    let stream = coral_core::graph::GixCommitStream::open(repo.path()).unwrap();
    let mut seen = 0_usize;
    stream
        .walk(&coral_core::graph::StreamOpts::default(), &mut |_| {
            seen += 1;
            coral_core::graph::WalkControl::Continue
        })
        .unwrap();
    assert_eq!(seen, 1, "the one commit is still walked");
}
