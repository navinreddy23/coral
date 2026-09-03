use coral_core::graph::{
    GixCommitStream, NO_ROW, Order, StreamOpts, SubprocessCommitStream, build, flags,
};
use coral_core::process::GitRunner;
use coral_core::testutil::TestRepo;

/// A merge, a fork, a second root: enough shapes that a mistake in the CSR offsets or the
/// parent resolution shows up.
fn fixture() -> TestRepo {
    let r = TestRepo::new().write("base.txt", "base\n").commit("base");
    r.git(["checkout", "--quiet", "-b", "side"]);
    let r = r.write("side.txt", "s\n").commit("side");
    r.git(["checkout", "--quiet", "main"]);
    let r = r.write("main.txt", "m\n").commit("main");
    r.git(["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);
    r.git(["checkout", "--quiet", "--orphan", "orphan"]);
    r.git(["rm", "-rf", "--quiet", "--cached", "."]);
    let r = r.write("o.txt", "o\n").commit("orphan root");
    r.git(["checkout", "--quiet", "main"]);
    r
}

#[tokio::test(flavor = "multi_thread")]
async fn every_parent_resolves_to_a_real_row() {
    let repo = fixture();
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let store = build(&stream, &StreamOpts::default()).unwrap();

    assert!(store.len() >= 5);
    for row in 0..store.len() {
        for (k, &p) in store.parents(row).iter().enumerate() {
            assert_ne!(p, NO_ROW, "row {row} parent {k} did not resolve");
            assert!(
                p > row,
                "a parent must come after its child in topological order"
            );
        }
        assert_eq!(
            store.parents(row).len(),
            store.parent_lanes(row).len(),
            "row {row}: parent rows and parent lanes must line up"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn round_trips_object_ids_to_rows_and_back() {
    let repo = fixture();
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let store = build(&stream, &StreamOpts::default()).unwrap();

    for row in 0..store.len() {
        let oid = store.oid(row).expect("row has an oid");
        assert_eq!(
            store.row_of(&oid),
            Some(row),
            "oid -> row -> oid disagreed at {row}"
        );
    }
    let absent = gix::ObjectId::from_hex(b"0000000000000000000000000000000000000001").unwrap();
    assert_eq!(store.row_of(&absent), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn flags_mark_merges_roots_and_provisional_rows() {
    let repo = fixture();
    let stream = GixCommitStream::open(repo.path()).unwrap();

    let store = build(&stream, &StreamOpts::default()).unwrap();
    let merges = (0..store.len())
        .filter(|r| store.flags(*r) & flags::MERGE != 0)
        .count();
    let roots = (0..store.len())
        .filter(|r| store.flags(*r) & flags::ROOT != 0)
        .count();
    assert_eq!(merges, 1, "the fixture has exactly one merge");
    assert_eq!(roots, 2, "the fixture has two roots");
    assert!(
        (0..store.len()).all(|r| store.flags(r) & flags::PROVISIONAL == 0),
        "a topological build is not provisional"
    );

    let quick = build(&stream, &StreamOpts::first_paint(4096)).unwrap();
    assert!(
        (0..quick.len()).all(|r| quick.flags(r) & flags::PROVISIONAL != 0),
        "a commit-time build must mark every row provisional"
    );
}

/// A walk cut short leaves parents outside the loaded set; those rows must say so rather than
/// point at a wrong row.
#[tokio::test(flavor = "multi_thread")]
async fn a_truncated_walk_marks_its_boundary() {
    let repo = fixture();
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let opts = StreamOpts {
        max_count: Some(2),
        ..StreamOpts::default()
    };
    let store = build(&stream, &opts).unwrap();

    assert_eq!(store.len(), 2);
    let boundary = (0..store.len())
        .filter(|r| store.flags(*r) & flags::BOUNDARY != 0)
        .count();
    assert!(
        boundary >= 1,
        "a cut-off walk must flag at least one boundary row"
    );

    for row in 0..store.len() {
        for &p in store.parents(row) {
            assert!(p == NO_ROW || p < store.len(), "parent row out of range");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn both_streams_build_identical_stores() {
    let repo = fixture();
    let runner = GitRunner::discover().await.unwrap();
    let gix = build(
        &GixCommitStream::open(repo.path()).unwrap(),
        &StreamOpts::default(),
    )
    .unwrap();
    let sub = build(
        &SubprocessCommitStream::new(runner, repo.path()),
        &StreamOpts::default(),
    )
    .unwrap();

    assert_eq!(gix.len(), sub.len());
    // Row order may differ in tie-breaking, so compare each commit's parent set by oid.
    for row in 0..gix.len() {
        let oid = gix.oid(row).unwrap();
        let other = sub
            .row_of(&oid)
            .expect("commit missing from the rev-list store");
        let mut a: Vec<_> = gix.parents(row).iter().map(|r| gix.oid(*r)).collect();
        let mut b: Vec<_> = sub.parents(other).iter().map(|r| sub.oid(*r)).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "parent sets differ for {oid}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_repository_builds_an_empty_store() {
    let repo = TestRepo::new();
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let store = build(&stream, &StreamOpts::default()).unwrap();

    assert!(store.is_empty());
    assert_eq!(
        store.row_of(&gix::ObjectId::empty_tree(gix::hash::Kind::Sha1)),
        None
    );
    assert_eq!(store.parents(0), &[] as &[u32]);
}

#[tokio::test(flavor = "multi_thread")]
async fn commit_time_and_topological_builds_hold_the_same_commits() {
    let repo = fixture();
    let stream = GixCommitStream::open(repo.path()).unwrap();
    let topo = build(&stream, &StreamOpts::default()).unwrap();
    let time = build(
        &stream,
        &StreamOpts {
            order: Order::CommitTime,
            ..StreamOpts::default()
        },
    )
    .unwrap();

    assert_eq!(topo.len(), time.len());
    for row in 0..topo.len() {
        let oid = topo.oid(row).unwrap();
        assert!(
            time.row_of(&oid).is_some(),
            "{oid} missing from the commit-time store"
        );
    }
}
