//! The topological walk over a commit-graph file, which is what the window's first paint and
//! its full graph both come from when a repository has one.

use std::collections::{HashMap, HashSet};

use coral_core::graph::{
    CommitNode, CommitStream, GixCommitStream, StreamOpts, SubprocessCommitStream, WalkControl,
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

/// Branches off old and new points, merges both ways, an octopus and a second root.
fn tangled() -> TestRepo {
    let r = TestRepo::new().write("base.txt", "0\n").commit("base");
    let r = r.write("base.txt", "1\n").commit("main 1");
    r.git(["checkout", "--quiet", "-b", "old-topic", "HEAD~1"]);
    let r = r.write("t.txt", "t1\n").commit("old topic 1");
    let r = r.write("t.txt", "t2\n").commit("old topic 2");
    r.git(["checkout", "--quiet", "main"]);
    let r = r.write("base.txt", "2\n").commit("main 2");
    r.git(["checkout", "--quiet", "-b", "a"]);
    let r = r.write("a.txt", "a\n").commit("a1");
    r.git(["checkout", "--quiet", "-b", "b", "main"]);
    let r = r.write("b.txt", "b\n").commit("b1");
    r.git(["checkout", "--quiet", "main"]);
    r.git(["merge", "--quiet", "--no-ff", "-m", "octopus", "a", "b"]);
    r.git([
        "merge",
        "--quiet",
        "--no-ff",
        "-m",
        "take the old topic",
        "old-topic",
    ]);
    r.git(["checkout", "--quiet", "a"]);
    r.git(["merge", "--quiet", "--no-ff", "-m", "a takes main", "main"]);
    r.git(["checkout", "--quiet", "--orphan", "orphan"]);
    r.git(["rm", "-rf", "--quiet", "--cached", "."]);
    let r = r.write("o.txt", "o\n").commit("orphan root");
    r.git(["checkout", "--quiet", "main"]);
    r.git(["tag", "-a", "-m", "old", "v0", "main~3"]);
    r
}

fn write_graph(repo: &TestRepo) {
    repo.git(["commit-graph", "write", "--reachable", "--no-progress"]);
    assert!(repo.path().join(".git/objects/info/commit-graph").exists());
}

/// Commits made after the commit-graph was written, including a merge of one in it and one not.
fn grow_past_the_graph(repo: TestRepo) -> TestRepo {
    let r = repo.write("late.txt", "1\n").commit("late 1");
    r.git(["checkout", "--quiet", "-b", "late-topic", "b"]);
    let r = r.write("lt.txt", "x\n").commit("late topic");
    r.git(["checkout", "--quiet", "main"]);
    r.git([
        "merge",
        "--quiet",
        "--no-ff",
        "-m",
        "late merge",
        "late-topic",
    ]);
    r
}

async fn streams(path: &std::path::Path) -> (GixCommitStream, SubprocessCommitStream) {
    let runner = GitRunner::discover().await.unwrap();
    (
        GixCommitStream::open(path).unwrap(),
        SubprocessCommitStream::new(runner, path),
    )
}

fn dag(nodes: &[CommitNode]) -> HashMap<gix::ObjectId, Vec<gix::ObjectId>> {
    nodes.iter().map(|n| (n.id, n.parents.to_vec())).collect()
}

fn assert_children_first(nodes: &[CommitNode]) {
    let mut seen = HashSet::new();
    for n in nodes {
        for p in &n.parents {
            assert!(
                !seen.contains(p),
                "parent {p} came before its child {}",
                n.id
            );
        }
        assert!(seen.insert(n.id), "{} came twice", n.id);
    }
}

/// Every prefix the walk can be stopped at is the start of the whole walk, which is what lets
/// the first paint be drawn in the order the full graph will have.
fn assert_prefixes(gix: &GixCommitStream, full: &[CommitNode], first_parent: bool) {
    for k in 1..=full.len() {
        let opts = StreamOpts {
            max_count: Some(k as u64),
            first_parent,
            ..StreamOpts::default()
        };
        let ids: Vec<_> = collect(gix, &opts).into_iter().map(|n| n.id).collect();
        let want: Vec<_> = full[..k].iter().map(|n| n.id).collect();
        assert_eq!(
            ids, want,
            "the walk stopped at {k} is not the start of the full walk"
        );
    }
}

async fn check(repo: &TestRepo) {
    let (gix, sub) = streams(repo.path()).await;
    let full = collect(&gix, &StreamOpts::default());
    assert_eq!(dag(&full), dag(&collect(&sub, &StreamOpts::default())));
    assert_children_first(&full);
    assert_prefixes(&gix, &full, false);
    assert_eq!(
        full,
        collect(&gix, &StreamOpts::default()),
        "the same walk twice"
    );

    let first = StreamOpts {
        first_parent: true,
        ..StreamOpts::default()
    };
    let mainline = collect(&gix, &first);
    let ids: HashSet<_> = mainline.iter().map(|n| n.id).collect();
    let git: HashSet<_> = collect(&sub, &first).into_iter().map(|n| n.id).collect();
    assert_eq!(ids, git, "first-parent reaches what git's does");
    assert_prefixes(&gix, &mainline, true);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_repository_wholly_in_its_commit_graph() {
    let repo = tangled();
    write_graph(&repo);
    check(&repo).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn commits_made_after_the_commit_graph_was_written() {
    let repo = tangled();
    write_graph(&repo);
    let repo = grow_past_the_graph(repo);
    check(&repo).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_merge_shows_the_line_it_brought_in_before_the_one_it_was_made_on() {
    let repo = TestRepo::new().write("a.txt", "0\n").commit("start");
    repo.git(["checkout", "--quiet", "-b", "topic"]);
    let repo = repo.write("t.txt", "t\n").commit("topic work");
    repo.git(["checkout", "--quiet", "main"]);
    let repo = repo.write("a.txt", "1\n").commit("main work");
    repo.git(["merge", "--quiet", "--no-ff", "-m", "merge topic", "topic"]);
    repo.git(["branch", "--quiet", "-D", "topic"]);
    write_graph(&repo);

    let (gix, _) = streams(repo.path()).await;
    let summaries: Vec<String> = collect(&gix, &StreamOpts::default())
        .iter()
        .map(|n| {
            repo.git(["log", "-1", "--format=%s", &n.id.to_string()])
                .trim()
                .to_owned()
        })
        .collect();
    assert_eq!(
        summaries,
        ["merge topic", "topic work", "main work", "start"]
    );
}
