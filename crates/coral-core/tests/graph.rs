use std::collections::{HashMap, HashSet};

use coral_core::graph::{
    CommitNode, CommitStream, GixCommitStream, Order, StreamOpts, SubprocessCommitStream, Tips,
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
    // On a repository with no stash. The gix walk hides the two commits git writes for one,
    // which `rev-list` has no reason to; the fixture below has no stash, so they agree.
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

/// A stash is one thing the user did, not three.
///
/// `git stash` writes a commit for the stash, one for the index and one for the untracked
/// files, and the last two are the first one's extra parents. Drawn like any other commit,
/// they put three rows and two extra lanes in the graph for every stash, captioned "index on
/// main" and "untracked files on main".
#[tokio::test]
async fn a_stash_is_one_row_and_not_its_bookkeeping() {
    let repo = TestRepo::new().write("a.txt", "one\n").commit("first");
    std::fs::write(repo.path().join("a.txt"), "changed\n").unwrap();
    std::fs::write(repo.path().join("new.txt"), "untracked\n").unwrap();
    repo.git(["stash", "push", "--include-untracked", "--message", "wip"]);

    let (gix, _) = streams(repo.path()).await;
    let rows = collect(&gix, &StreamOpts::default());

    let messages: Vec<String> = rows
        .iter()
        .map(|node| repo.git(["log", "--format=%s", "-1", &node.id.to_string()]))
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.starts_with("On main: wip") || m.starts_with("WIP on main")),
        "the stash itself is missing: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.starts_with("index on")),
        "the index commit is drawn: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.starts_with("untracked files on")),
        "the untracked commit is drawn: {messages:?}"
    );

    // And the stash's own row keeps one parent, so no lane is opened for what is not drawn.
    let stash = rows
        .iter()
        .find(|node| {
            let m = repo.git(["log", "--format=%s", "-1", &node.id.to_string()]);
            m.starts_with("On main: wip") || m.starts_with("WIP on main")
        })
        .expect("the stash row");
    assert_eq!(
        stash.parents.len(),
        1,
        "the stash still points at its plumbing"
    );
}

/// The set of commits a walk emitted, for comparing selections against each other.
fn ids(stream: &dyn CommitStream, tips: Tips) -> HashSet<gix::ObjectId> {
    let opts = StreamOpts {
        tips,
        ..StreamOpts::default()
    };
    collect(stream, &opts).into_iter().map(|n| n.id).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn soloing_one_ref_walks_only_what_that_ref_reaches() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;
    let only = Tips::Only(vec!["refs/heads/a".to_owned()]);

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        let soloed = ids(stream, only.clone());
        let everything = ids(stream, Tips::All);

        // `a` is one commit on top of the base, and nothing else in this repository is below it.
        assert_eq!(soloed.len(), 2, "{} soloed the wrong number", stream.name());
        assert!(
            soloed.is_subset(&everything) && soloed.len() < everything.len(),
            "{} soloed a set that is not a proper part of the whole",
            stream.name()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hiding_a_ref_drops_only_the_commits_nothing_else_reaches() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        let everything = ids(stream, Tips::All);
        let without = ids(stream, Tips::Except(vec!["refs/heads/orphan".to_owned()]));
        let orphan = ids(stream, Tips::Only(vec!["refs/heads/orphan".to_owned()]));

        // The orphan root is reachable from nothing else, so hiding its branch is the one case
        // where hiding actually removes commits.
        assert!(
            without.is_subset(&everything),
            "{} invented commits when hiding",
            stream.name()
        );
        assert!(
            orphan.iter().all(|id| !without.contains(id)),
            "{} still drew the hidden orphan",
            stream.name()
        );
        assert_eq!(
            without.len() + orphan.len(),
            everything.len(),
            "{} lost commits that another branch still reaches",
            stream.name()
        );
    }
}

/// Hiding a branch whose commits another branch also reaches must change nothing. This is the
/// honest answer, and the one git gives: `a` is an ancestor of the octopus merge on `main`.
#[tokio::test(flavor = "multi_thread")]
async fn hiding_a_merged_branch_leaves_its_commits_where_they_are() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        assert_eq!(
            ids(stream, Tips::Except(vec!["refs/heads/a".to_owned()])),
            ids(stream, Tips::All),
            "{} dropped commits that were still reachable",
            stream.name()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn soloing_nothing_is_an_empty_graph_rather_than_every_branch() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        assert!(
            ids(stream, Tips::Only(Vec::new())).is_empty(),
            "{} read an empty selection as every ref",
            stream.name()
        );
    }
}

/// A name from a choice made before the branch was deleted. It must leave a graph that draws.
#[tokio::test(flavor = "multi_thread")]
async fn a_selection_naming_a_ref_that_is_gone_is_skipped_rather_than_refused() {
    let repo = gnarly_repo();
    let (gix, sub) = streams(repo.path()).await;
    let tips = Tips::Only(vec![
        "refs/heads/a".to_owned(),
        "refs/heads/deleted-yesterday".to_owned(),
    ]);

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        assert_eq!(
            ids(stream, tips.clone()),
            ids(stream, Tips::Only(vec!["refs/heads/a".to_owned()])),
            "{} did not skip the missing ref",
            stream.name()
        );
    }
}

/// Hiding the branch HEAD is on has to hide it. HEAD resolves to the same commit, so leaving it
/// in the tip set would put the branch straight back with nothing on screen to explain it.
#[tokio::test(flavor = "multi_thread")]
async fn hiding_the_checked_out_branch_hides_it() {
    let repo = gnarly_repo();
    let tip = repo.git(["rev-parse", "refs/heads/main"]);
    let tip = gix::ObjectId::from_hex(tip.trim().as_bytes()).unwrap();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        assert!(
            ids(stream, Tips::All).contains(&tip),
            "{} did not draw the checked-out branch to begin with",
            stream.name()
        );
        // The octopus merge is on main and on nothing else, so it is the commit that says
        // whether hiding main took effect at all.
        assert!(
            !ids(stream, Tips::Except(vec!["refs/heads/main".to_owned()])).contains(&tip),
            "{} kept the checked-out branch through HEAD",
            stream.name()
        );
    }
}

/// A detached HEAD is a tip no ref reaches, so hiding something unrelated must not take it with
/// it. This is the case `--glob=refs/*` drops and the explicit HEAD puts back.
#[tokio::test(flavor = "multi_thread")]
async fn hiding_a_branch_keeps_a_detached_head() {
    let repo = gnarly_repo();
    repo.git(["checkout", "--quiet", "--detach"]);
    let repo = repo.write("loose.txt", "loose\n").commit("on no branch");
    let loose = repo.git(["rev-parse", "HEAD"]);
    let loose = gix::ObjectId::from_hex(loose.trim().as_bytes()).unwrap();
    let (gix, sub) = streams(repo.path()).await;

    for stream in [&gix as &dyn CommitStream, &sub as &dyn CommitStream] {
        assert!(
            ids(stream, Tips::Except(vec!["refs/heads/orphan".to_owned()])).contains(&loose),
            "{} lost the detached HEAD while hiding another branch",
            stream.name()
        );
    }
}

/// A shallow clone of `source`, one commit deep, which is how `west` fetches a module.
fn shallow_clone_of(source: &std::path::Path, into: &std::path::Path) {
    let url = format!("file://{}", source.display());
    let status = std::process::Command::new("git")
        .args(["clone", "--quiet", "--depth", "1", &url])
        .arg(into)
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .expect("git clone runs");
    assert!(status.success(), "shallow clone of {url} failed");
}

/// A shallow clone's boundary commit names parents the clone never fetched. Walking into one
/// used to stop the graph with "an object … could not be found", which is what opening a west
/// workspace's modules did; the boundary is grafted now, exactly as git grafts it.
#[test]
fn a_shallow_clone_walks_to_its_boundary_and_stops() {
    let source = TestRepo::new().write("a.txt", "one\n").commit("first");
    let source = source.write("a.txt", "two\n").commit("second");
    let source = source.write("a.txt", "three\n").commit("third");

    let dir = tempfile::tempdir().unwrap();
    let clone = dir.path().join("shallow");
    shallow_clone_of(source.path(), &clone);
    assert!(
        clone.join(".git/shallow").is_file(),
        "the clone has to be shallow for this test to mean anything"
    );

    let stream = GixCommitStream::open(&clone).unwrap();
    let nodes = collect(&stream, &StreamOpts::default());
    assert_eq!(nodes.len(), 1, "one commit deep");
    assert!(
        nodes[0].parents.is_empty(),
        "the boundary commit is a root as far as this clone is concerned"
    );
}

/// The same by commit time, since the two orders take different paths through gix.
#[test]
fn a_shallow_clone_walks_by_commit_time_too() {
    let source = TestRepo::new().write("a.txt", "one\n").commit("first");
    let source = source.write("a.txt", "two\n").commit("second");

    let dir = tempfile::tempdir().unwrap();
    let clone = dir.path().join("shallow");
    shallow_clone_of(source.path(), &clone);

    let stream = GixCommitStream::open(&clone).unwrap();
    let opts = StreamOpts {
        order: Order::CommitTime,
        ..StreamOpts::default()
    };
    let nodes = collect(&stream, &opts);
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].parents.is_empty());
}

/// Grafting must not touch a whole clone: a repository with no `shallow` file keeps every
/// parent it has, and keeps its commit-graph.
#[test]
fn an_ordinary_clone_is_left_alone() {
    let r = TestRepo::new().write("a.txt", "one\n").commit("first");
    let r = r.write("a.txt", "two\n").commit("second");
    let stream = GixCommitStream::open(r.path()).unwrap();
    let nodes = collect(&stream, &StreamOpts::default());
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].parents.len(), 1, "the tip still knows its parent");
}
