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
        stash: false,
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
            left_from: None,
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
        left_from: None,
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
        left_from: None,
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

/// Further than that its line runs a few rows on and stops: on the kernel, held lines to
/// forks thousands of rows down filled every lane of the column.
#[test]
fn a_branch_runs_a_short_tail_towards_a_fork_too_far_down_to_hold_a_line_to() {
    let store = two_branches_off_one_commit(400);
    assert_eq!(store.lane(1), Some(1));
    assert_eq!(store.parent_lanes(1), &[1], "down its own lane");
    let run = (2..401)
        .take_while(|&row| store.open(row) & 0b10 != 0)
        .count();
    assert_eq!(run, 8, "for eight rows");
    assert_eq!(store.joins(401), 0);
}

/// A walk cut off above a commit that two branches leave from draws them as the whole walk
/// does: one line to it, and a short tail under the second branch. Keyed apart, every edge
/// out of the cut held a lane of its own, and the kernel's first paint of 4,096 rows filled the column
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

/// Lays out `nodes` with HEAD on `head`.
fn with_head(nodes: &[CommitNode], head: u32) -> coral_core::graph::RowStore {
    let mut builder = RowStoreBuilder::new(false);
    builder.keep_left(Some(Trunk {
        head: id(head),
        upstream: None,
        left_from: None,
    }));
    for n in nodes {
        builder.push(n);
    }
    builder.finish()
}

/// A HEAD behind the tip of a straight history, as a bisect or an old tag leaves it, is on
/// the one line there is: drawn as one lane. Keeping lane 0 for HEAD alone put the commits
/// above it in lane 1, turning into it, a fork where the history has none.
#[test]
fn a_head_behind_a_straight_line_leaves_it_straight() {
    let nodes: Vec<CommitNode> = (0..10)
        .map(|n| {
            if n < 9 {
                node(n, &[n + 1])
            } else {
                node(n, &[])
            }
        })
        .collect();
    let store = with_head(&nodes, 5);
    for row in 0..10 {
        assert_eq!(store.lane(row), Some(0), "row {row} left the line");
    }
}

/// A fork above HEAD is where the line stops: both branches turn into HEAD's lane, so the
/// lane the user is on is the one running down, whichever branch is newer.
#[test]
fn a_head_with_two_branches_above_it_keeps_lane_0_to_itself() {
    let nodes = [
        node(0, &[1]),
        node(1, &[4]),
        node(2, &[3]),
        node(3, &[4]),
        node(4, &[5]),
        node(5, &[]),
    ];
    let store = with_head(&nodes, 4);
    assert_eq!(store.lane(4), Some(0));
    assert!(
        [0, 1, 2, 3].iter().all(|&row| store.lane(row) != Some(0)),
        "a branch above the fork took lane 0"
    );
}

/// A stash on HEAD, its index parent dropped as the walk drops it, reads as a commit on
/// HEAD: it stays beside HEAD's line rather than becoming its top, and does not stop the line
/// running on above it.
#[test]
fn a_stash_on_head_is_not_taken_for_the_line_running_on() {
    let stash = CommitNode {
        stash: true,
        ..node(0, &[2])
    };
    let nodes = [stash, node(1, &[2]), node(2, &[3]), node(3, &[])];
    let store = with_head(&nodes, 2);
    assert_ne!(store.lane(0), Some(0), "the stash took lane 0");
    for row in 1..4 {
        assert_eq!(store.lane(row), Some(0), "row {row} left the line");
    }
}

/// A main line of merges is still one line. Above a HEAD left on it, the next commit on it is
/// a merge, and a branch off HEAD's commit leaves beside it: the line runs on into the merge,
/// the main line merging a branch back, and the branch turns in from the right.
#[test]
fn a_head_behind_a_main_line_of_merges_leaves_it_straight() {
    let nodes = [node(0, &[2, 1]), node(1, &[2]), node(2, &[3]), node(3, &[])];
    let store = with_head(&nodes, 2);
    assert_eq!(store.lane(0), Some(0), "the merge above HEAD left the line");
    assert_eq!(store.lane(2), Some(0));
    assert_ne!(store.lane(1), Some(0));
}

/// A branch merging HEAD's commit in does not stop HEAD's line: it is not where that line
/// goes, only where it was taken from.
#[test]
fn a_branch_merging_head_in_does_not_stop_the_line() {
    let nodes = [
        node(0, &[2]),
        node(1, &[3, 2]),
        node(2, &[4]),
        node(3, &[4]),
        node(4, &[]),
    ];
    let store = with_head(&nodes, 2);
    assert_eq!(
        store.lane(0),
        Some(0),
        "the commit above HEAD left the line"
    );
    assert_eq!(store.lane(2), Some(0));
}

/// A detached HEAD's line goes on up into the branch it was left from, where the history alone
/// cannot say which way it went: two plain commits leave the commit above HEAD, and the one
/// on the branch is the line.
#[test]
fn a_detached_head_keeps_the_branch_it_was_left_from_in_lane_0() {
    let nodes = [
        node(0, &[1]),
        node(1, &[3]),
        node(2, &[3]),
        node(3, &[4]),
        node(4, &[5]),
        node(5, &[]),
    ];
    let lay_out = |left_from: Option<u32>| {
        let mut builder = RowStoreBuilder::new(false);
        builder.keep_left(Some(Trunk {
            head: id(4),
            upstream: None,
            left_from: left_from.map(id),
        }));
        for n in &nodes {
            builder.push(n);
        }
        builder.finish()
    };
    let store = lay_out(Some(0));
    for row in [0, 1, 3, 4, 5] {
        assert_eq!(store.lane(row), Some(0), "row {row} left the branch's line");
    }
    // A branch that does not lead down to HEAD is no line of HEAD's: the line stops where
    // the history above HEAD forks.
    let store = lay_out(Some(5));
    assert_eq!(store.lane(3), Some(0));
    assert_ne!(
        store.lane(0),
        Some(0),
        "row 0 took lane 0 by way of a branch below HEAD"
    );
}

/// Both walks name the branch a detached HEAD was left from alike: the one a bisect started
/// from, else main.
#[tokio::test(flavor = "multi_thread")]
async fn both_streams_name_the_branch_a_detached_head_was_left_from() {
    let repo = fixture();
    let runner = GitRunner::discover().await.unwrap();
    let rev =
        |name: &str| gix::ObjectId::from_hex(repo.git(["rev-parse", name]).as_bytes()).unwrap();
    let main = rev("main");

    repo.git(["checkout", "--quiet", "--detach", "main~1"]);
    let head = rev("HEAD");
    let gix_stream = GixCommitStream::open(repo.path()).unwrap();
    let sub = SubprocessCommitStream::new(runner.clone(), repo.path());
    let detached = Trunk {
        head,
        upstream: None,
        left_from: Some(main),
    };
    assert_eq!(gix_stream.trunk(), Some(detached));
    assert_eq!(sub.trunk(), Some(detached));

    // A bisect started from a topic branch names it, whatever main is.
    repo.git(["checkout", "--quiet", "-b", "topic", "main"]);
    for n in 0..4 {
        repo.git([
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            &format!("topic {n}"),
        ]);
    }
    let topic = rev("topic");
    repo.git(["bisect", "start", "topic", "main"]);
    let head = rev("HEAD");
    assert_ne!(head, topic, "the bisect left HEAD where it was");
    let gix_stream = GixCommitStream::open(repo.path()).unwrap();
    let bisecting = Trunk {
        head,
        upstream: None,
        left_from: Some(topic),
    };
    assert_eq!(gix_stream.trunk(), Some(bisecting));
    assert_eq!(sub.trunk(), Some(bisecting));
}
