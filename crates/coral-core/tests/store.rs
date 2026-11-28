use coral_core::graph::{
    CommitNode, CommitStream, GixCommitStream, NO_ROW, Order, RowStoreBuilder, StreamOpts,
    SubprocessCommitStream, Trunk, build, flags,
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

    let quick = build(&stream, &StreamOpts::first_paint(4096, Order::CommitTime)).unwrap();
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

/// An object id made from a number, for histories written out by hand.
fn id(n: u32) -> gix::ObjectId {
    let mut bytes = [0_u8; 20];
    bytes[..4].copy_from_slice(&n.to_be_bytes());
    bytes[19] = 1;
    gix::ObjectId::from_bytes_or_panic(&bytes)
}

fn node(n: u32, parents: &[u32]) -> CommitNode {
    CommitNode {
        id: id(n),
        parents: parents.iter().map(|&p| id(p)).collect(),
        commit_time: 0,
    }
}

/// A first screen and the full walk lay the first screen's rows out alike, whether HEAD is on
/// it or further down. Otherwise every line on screen would move when the full walk landed.
#[test]
fn a_first_screen_lays_out_its_rows_as_the_full_walk_does() {
    let screen = u32::try_from(StreamOpts::FIRST_PAINT_ROWS).unwrap();
    for head_row in [10, screen + 10] {
        // A newer branch tip first, then one long line with HEAD somewhere down it.
        let rows = screen + 40;
        let root = rows - 1;
        let mut nodes = vec![node(0, &[root])];
        nodes.extend((1..root).map(|n| node(n, &[n + 1])));
        nodes.push(node(root, &[]));
        let trunk = Trunk {
            head: id(head_row),
            upstream: None,
        };

        let lay_out = |count: usize| {
            let mut builder = RowStoreBuilder::new(false);
            builder.keep_left(Some(trunk));
            for n in &nodes[..count] {
                builder.push(n);
            }
            builder.finish()
        };
        let full = lay_out(nodes.len());
        let first = lay_out(screen as usize);

        for row in 0..screen {
            assert_eq!(
                (first.lane(row), first.joins(row), first.open(row)),
                (full.lane(row), full.joins(row), full.open(row)),
                "HEAD on row {head_row}: row {row} is laid out differently by the first screen"
            );
        }
        let kept = head_row < screen;
        assert_eq!(
            full.lane(head_row) == Some(0),
            kept,
            "HEAD on row {head_row} is kept in lane 0 only when the first screen reaches it"
        );
    }
}

/// HEAD's line is kept in lane 0 by both walks, so they must agree on what it is.
#[tokio::test(flavor = "multi_thread")]
async fn both_streams_name_the_same_trunk() {
    let repo = fixture();
    let gix_stream = GixCommitStream::open(repo.path()).unwrap();
    let runner = GitRunner::discover().await.unwrap();
    let sub = SubprocessCommitStream::new(runner, repo.path());
    let head = gix::ObjectId::from_hex(repo.git(["rev-parse", "HEAD"]).as_bytes()).unwrap();

    let alone = Trunk {
        head,
        upstream: None,
    };
    assert_eq!(gix_stream.trunk(), Some(alone));
    assert_eq!(sub.trunk(), Some(alone));

    // With main tracking a remote branch that is one ahead of it.
    repo.git(["update-ref", "refs/remotes/origin/main", "side"]);
    for (key, value) in [
        ("remote.origin.url", "/nowhere"),
        ("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"),
        ("branch.main.remote", "origin"),
        ("branch.main.merge", "refs/heads/main"),
    ] {
        repo.git(["config", key, value]);
    }
    let side = gix::ObjectId::from_hex(repo.git(["rev-parse", "side"]).as_bytes()).unwrap();
    let tracking = Trunk {
        head,
        upstream: Some(side),
    };
    // Reopened, as the window opens one for every walk: gix reads the config when it opens.
    let gix_stream = GixCommitStream::open(repo.path()).unwrap();
    assert_eq!(gix_stream.trunk(), Some(tracking));
    assert_eq!(sub.trunk(), Some(tracking));
}

/// Two tips leaving one commit, the second `gap` rows above it: the line to it held or not.
fn two_branches_off_one_commit(gap: u32) -> coral_core::graph::RowStore {
    // Row 0 leads to the fork first; row 1 is the branch that may or may not hold a lane down
    // to it; rows 2.. are an unrelated line that puts the fork `gap` rows below row 1.
    let fork = gap + 1;
    let mut builder = RowStoreBuilder::new(false);
    builder.push(&node(0, &[fork]));
    builder.push(&node(1, &[fork]));
    for n in 2..fork {
        let parents: &[u32] = if n + 1 < fork { &[n + 1] } else { &[] };
        builder.push(&node(n, parents));
    }
    builder.push(&node(fork, &[]));
    builder.finish()
}

/// A branch keeps its lane down to a fork a few screens below, as `GitKraken` draws it.
#[test]
fn a_branch_holds_its_lane_to_a_fork_a_few_screens_down() {
    let store = two_branches_off_one_commit(60);
    assert_eq!(store.parent_lanes(1), &[store.lane(1).unwrap()]);
    assert_ne!(
        store.joins(61) & (1 << store.lane(1).unwrap()),
        0,
        "it turns into the fork"
    );
}

/// Further than that it joins the line already running to its fork, on its own row: on the
/// kernel, held lines to forks thousands of rows down filled every lane of the column.
#[test]
fn a_branch_joins_the_line_to_a_fork_too_far_down_to_hold_one() {
    let store = two_branches_off_one_commit(400);
    assert_eq!(store.lane(1), Some(1));
    assert_eq!(
        store.parent_lanes(1),
        &[0],
        "across its own row into the line to the fork"
    );
    assert_eq!(store.joins(401), 0);
}

/// A walk cut off above a commit that two branches leave from draws them as the whole walk
/// does: one line to it, which the second branch joins. Keyed apart, every edge out of the
/// cut opened a lane of its own, and the kernel's first paint of 4,096 rows filled the column
/// from row 76 down, until the full walk landed and every line below moved.
#[test]
fn a_walk_cut_short_shares_the_line_to_a_commit_it_does_not_reach() {
    let full = two_branches_off_one_commit(400);
    let mut builder = RowStoreBuilder::new(false);
    builder.push(&node(0, &[401]));
    builder.push(&node(1, &[401]));
    for n in 2..300 {
        builder.push(&node(n, &[n + 1]));
    }
    let cut = builder.finish();
    for row in 0..300 {
        assert_eq!(
            (
                cut.lane(row),
                cut.parent_lanes(row),
                cut.open(row),
                cut.joins(row)
            ),
            (
                full.lane(row),
                full.parent_lanes(row),
                full.open(row),
                full.joins(row)
            ),
            "row {row} is laid out differently by the walk cut short"
        );
    }
}
