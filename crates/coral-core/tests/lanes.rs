//! Lane assignment, drawn as box art and snapshotted.
//!
//! A lane bug is otherwise very hard to read from raw indices; as a picture it is a one-line
//! diff. Each DAG is written parent-last, matching the order a topological walk emits, and
//! every picture is checked against the DAG it came from before it is compared: each line has
//! to end at the commit it was drawn for.

mod common;

use common::{SPILL, assert_branches_hold, assert_drawable};
use coral_core::graph::{LaneAssigner, MAX_LANES, NO_LANE, RowTopology, art};

/// Lays out a DAG. `dag[i]` lists the parent indices of commit `i`, children first.
fn lay_out<P: AsRef<[usize]>>(dag: &[P]) -> (Vec<RowTopology>, LaneAssigner<usize>) {
    let mut assigner = LaneAssigner::<usize>::new();
    let rows = dag
        .iter()
        .enumerate()
        .map(|(i, p)| assigner.push(&i, p.as_ref()))
        .collect();
    (rows, assigner)
}

/// The layout of a DAG as a picture, one row per line with the lanes running on under it,
/// after checking it can be drawn.
fn picture(dag: &[&[usize]], labels: &[&str]) -> String {
    let (rows, _) = lay_out(dag);
    assert_drawable(dag, &rows, true);
    assert_branches_hold(&rows);
    let drawn: Vec<art::Row<'_>> = rows.iter().map(art::Row::from).collect();
    common::labelled(&art::draw(&drawn), labels.iter().copied())
}

#[test]
fn linear_history_stays_in_one_lane() {
    let dag: &[&[usize]] = &[&[1], &[2], &[3], &[]];
    insta::assert_snapshot!(picture(dag, &["d", "c", "b", "a (root)"]));
}

#[test]
fn a_merge_opens_a_lane_for_the_second_parent() {
    // m takes both a and b; they share the root r.
    let dag: &[&[usize]] = &[&[1, 2], &[3], &[3], &[]];
    insta::assert_snapshot!(picture(dag, &["m (merge)", "a", "b", "r (root)"]));
}

#[test]
fn a_branch_runs_down_its_own_lane_to_the_commit_it_left_from() {
    // Two children of one parent: the second keeps its lane to the parent's row and turns in.
    let dag: &[&[usize]] = &[&[2], &[2], &[]];
    insta::assert_snapshot!(picture(dag, &["child-1", "child-2", "shared parent"]));
}

#[test]
fn a_feature_merged_back_is_one_lane_from_merge_to_fork() {
    // The order git's topological walk gives: the merged line first, then the mainline.
    let dag: &[&[usize]] = &[&[3, 1], &[2], &[4], &[4], &[]];
    insta::assert_snapshot!(picture(
        dag,
        &["merge", "feature-2", "feature-1", "main", "base"]
    ));
}

#[test]
fn five_branches_from_one_commit_all_turn_in_from_the_right() {
    let dag: &[&[usize]] = &[&[5], &[5], &[5], &[5], &[5], &[]];
    insta::assert_snapshot!(picture(dag, &["a", "b", "c", "d", "e", "base"]));
}

#[test]
fn a_branch_off_a_branch_turns_into_the_branch() {
    let dag: &[&[usize]] = &[&[5], &[2], &[4], &[4], &[5], &[]];
    insta::assert_snapshot!(picture(
        dag,
        &["main", "sub", "sub-1", "feature", "feature-1", "base"]
    ));
}

#[test]
fn main_merged_into_a_feature_joins_mains_line() {
    let dag: &[&[usize]] = &[&[2], &[3, 2], &[4], &[4], &[]];
    insta::assert_snapshot!(picture(
        dag,
        &["main", "merge main", "main-1", "feature-1", "base"]
    ));
}

#[test]
fn a_branch_merged_twice_runs_on_through_the_first_merge() {
    let dag: &[&[usize]] = &[&[2, 1], &[3], &[4, 3], &[5], &[5], &[]];
    insta::assert_snapshot!(picture(
        dag,
        &[
            "merge-2",
            "feature-2",
            "merge-1",
            "feature-1",
            "main",
            "base"
        ]
    ));
}

#[test]
fn an_octopus_opens_a_lane_per_extra_parent() {
    let dag: &[&[usize]] = &[&[1, 2, 3, 4], &[5], &[5], &[5], &[5], &[]];
    insta::assert_snapshot!(picture(dag, &["octopus", "a", "b", "c", "d", "r (root)"]));
}

#[test]
fn a_criss_cross_joins_the_lines_already_running() {
    // Two merges that each reach both shared parents.
    let dag: &[&[usize]] = &[&[2, 3], &[2, 3], &[4], &[4], &[]];
    insta::assert_snapshot!(picture(dag, &["merge-x", "merge-y", "a", "b", "r (root)"]));
}

#[test]
fn a_second_root_starts_a_line_of_its_own() {
    let dag: &[&[usize]] = &[&[1], &[], &[3], &[]];
    insta::assert_snapshot!(picture(dag, &["a", "root-1", "b (orphan)", "root-2"]));
}

#[test]
fn two_orphan_branches_side_by_side_end_at_their_own_roots() {
    let dag: &[&[usize]] = &[&[2], &[3], &[4], &[5], &[], &[]];
    insta::assert_snapshot!(picture(
        dag,
        &[
            "main",
            "pages",
            "main-1",
            "pages-1",
            "main root",
            "pages root"
        ]
    ));
}

#[test]
fn an_unrelated_history_merged_in_ends_at_its_root() {
    let dag: &[&[usize]] = &[&[2, 1], &[], &[3], &[]];
    insta::assert_snapshot!(picture(dag, &["merge", "other root", "main", "base"]));
}

#[test]
fn a_branch_newer_than_main_keeps_the_left_lane() {
    // Whichever tip comes first takes lane 0; the other turns into the fork from the right.
    let dag: &[&[usize]] = &[&[3], &[2], &[3], &[]];
    insta::assert_snapshot!(picture(dag, &["feature", "main", "main-1", "base"]));
}

#[test]
fn the_line_kept_left_stays_in_lane_0_however_new_the_other_tips_are() {
    // The same history as above, with main's tip kept left: the newer branch takes lane 1,
    // and turns into the fork from the right.
    let dag: &[&[usize]] = &[&[3], &[2], &[3], &[]];
    let mut assigner = LaneAssigner::<usize>::new();
    assigner.keep_left(1);
    let rows: Vec<RowTopology> = dag
        .iter()
        .enumerate()
        .map(|(i, p)| assigner.push(&i, p))
        .collect();
    assert_drawable(dag, &rows, true);
    let drawn: Vec<art::Row<'_>> = rows.iter().map(art::Row::from).collect();
    insta::assert_snapshot!(common::labelled(
        &art::draw(&drawn),
        ["feature", "main (kept left)", "main-1", "base"].into_iter()
    ));
}

#[test]
fn a_line_kept_left_takes_lane_0_even_with_lines_already_waiting_for_it() {
    // A merge above the kept commit already holds a line for it in lane 1.
    let dag: &[&[usize]] = &[&[2, 1], &[3], &[3], &[]];
    let mut assigner = LaneAssigner::<usize>::new();
    assigner.keep_left(1);
    let rows: Vec<RowTopology> = dag
        .iter()
        .enumerate()
        .map(|(i, p)| assigner.push(&i, p))
        .collect();
    assert_drawable(dag, &rows, true);
    assert_eq!(rows[1].lane, 0);
    assert_ne!(
        rows[0].lane, 0,
        "nothing else is placed in lane 0 before it"
    );
}

#[test]
fn width_never_exceeds_the_number_of_live_branches() {
    let dag: &[&[usize]] = &[&[1, 2], &[3], &[3], &[]];
    let (_, assigner) = lay_out(dag);
    assert_eq!(
        assigner.max_width(),
        2,
        "a two-sided merge needs exactly two lanes"
    );
}

/// Regression: a row whose first parent is already reserved does not keep its own lane, but
/// `alloc` may hand that same lane to a *later* parent. Freeing it afterwards destroyed the
/// slot while the reservation was still live, and the assigner then indexed past the end when
/// that parent was finally emitted. Only reproduced on the kernel, at 382 lanes.
#[test]
fn a_lane_handed_to_a_later_parent_is_not_freed_from_under_it() {
    let dag: &[&[usize]] = &[&[2, 3], &[2, 4], &[], &[], &[]];
    let (rows, _) = lay_out(dag);
    assert_drawable(dag, &rows, true);
}

/// A small deterministic generator, so a failure names a seed that reproduces it.
struct Dice(u64);

impl Dice {
    fn roll(&mut self, sides: usize) -> usize {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let value = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33;
        usize::try_from(value).unwrap_or(0) % sides.max(1)
    }
}

/// A history the way repositories grow one: branches open off a commit, take commits, and
/// merge back; now and then one merges several, forks from deep in the past, or starts over
/// from no parent at all. Returned children first, as a topological walk would emit it.
fn history(dice: &mut Dice, commits: usize, branches: usize) -> Vec<Vec<usize>> {
    let mut parents: Vec<Vec<usize>> = Vec::with_capacity(commits);
    let mut heads: Vec<usize> = Vec::new();
    for c in 0..commits {
        let roll = dice.roll(100);
        let mut mine = Vec::new();
        if c > 0 && roll >= 3 {
            let first = if heads.is_empty() || dice.roll(10) == 0 {
                dice.roll(c)
            } else {
                heads[dice.roll(heads.len())]
            };
            mine.push(first);
            // Now and then an octopus as wide as the kernel's, which can need more lanes than
            // the column has on one row.
            let extra = match roll {
                3..=14 => 1,
                15..=16 => 2 + dice.roll(3),
                17 => 5 + dice.roll(40),
                _ => 0,
            };
            for _ in 0..extra {
                let other = dice.roll(c);
                if !mine.contains(&other) {
                    mine.push(other);
                }
            }
        }
        for p in &mine {
            heads.retain(|h| h != p);
        }
        // A head is dropped now and then even when nothing builds on it, which is a branch
        // left behind; otherwise a new head replaces the oldest once there are enough.
        heads.push(c);
        if heads.len() > branches {
            heads.remove(dice.roll(heads.len()));
        }
        parents.push(mine);
    }
    // Reversed: the newest commit first, and every parent index rebased to match.
    let last = commits - 1;
    parents
        .into_iter()
        .rev()
        .map(|ps| ps.into_iter().map(|p| last - p).collect())
        .collect()
}

#[test]
fn every_line_ends_at_the_commit_it_was_drawn_for() {
    let mut widths = std::collections::BTreeMap::new();
    for seed in 1..=3000_u64 {
        let mut dice = Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let commits = 2 + dice.roll(150);
        let branches = 1 + dice.roll(12);
        let dag = history(&mut dice, commits, branches);
        // Every other history keeps some commit's line left, as the window keeps HEAD's.
        let kept = (seed % 2 == 0).then(|| dice.roll(commits));
        let mut assigner = LaneAssigner::<usize>::new();
        if let Some(k) = kept {
            assigner.keep_left(k);
        }
        let rows: Vec<RowTopology> = dag
            .iter()
            .enumerate()
            .map(|(i, p)| assigner.push(&i, p))
            .collect();
        *widths.entry(assigner.max_width()).or_insert(0) += 1;
        assert_drawable(&dag, &rows, assigner.max_width() < SPILL);
        assert_branches_hold(&rows);
        if let Some(k) = kept {
            assert_eq!(rows[k].lane, 0, "seed {seed}: the kept commit left lane 0");
            assert!(
                rows[..k].iter().all(|r| r.lane != 0),
                "seed {seed}: a commit above the kept one took lane 0"
            );
        }
    }
    eprintln!("widths: {widths:?}");
}

/// A branch whose fork is too far down to hold a line to runs a few rows on down its own lane
/// and stops: no corner on its own row, so every corner in the graph still turns one way.
#[test]
fn a_far_fork_runs_a_short_tail_down_its_own_lane() {
    // Rows 0 and 1 leave from row 30; row 1 is laid out as one whose fork is far below, and
    // rows 2.. are an unrelated line beside them.
    let mut dag: Vec<Vec<usize>> = vec![vec![30], vec![30]];
    for i in 2..30 {
        dag.push(if i < 29 { vec![i + 1] } else { vec![] });
    }
    dag.push(vec![]);
    let mut assigner = LaneAssigner::<usize>::new();
    let rows: Vec<RowTopology> = dag
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 1 {
                assigner.push_far(&i, p)
            } else {
                assigner.push(&i, p)
            }
        })
        .collect();
    assert_drawable(&dag, &rows, false);
    assert_branches_hold(&rows);

    let tail = rows[1].lane;
    assert_eq!(
        rows[1].parent_lanes.as_slice(),
        &[tail],
        "down its own lane"
    );
    let run = rows[2..]
        .iter()
        .take_while(|row| row.open & (1 << tail) != 0)
        .count();
    assert_eq!(run, 8, "the tail runs eight rows under the commit");
    assert_eq!(
        rows[30].joins & (1 << tail),
        0,
        "and never reaches the fork"
    );
}

#[test]
fn far_forks_mixed_in_never_end_a_line_at_another_commit() {
    for seed in 1..=1000_u64 {
        let mut dice = Dice(seed.wrapping_mul(0x94d0_49bb_1331_11eb) | 1);
        let commits = 2 + dice.roll(120);
        let branches = 1 + dice.roll(10);
        let dag = history(&mut dice, commits, branches);
        let mut assigner = LaneAssigner::<usize>::new();
        let rows: Vec<RowTopology> = dag
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if dice.roll(3) == 0 {
                    assigner.push_far(&i, p)
                } else {
                    assigner.push(&i, p)
                }
            })
            .collect();
        // A tail stops short of its commit by design.
        assert_drawable(&dag, &rows, false);
        assert_branches_hold(&rows);
    }
}

#[test]
fn a_saturated_graph_still_draws_inside_the_column() {
    for seed in 1..=200_u64 {
        let mut dice = Dice(seed.wrapping_mul(0xd1b5_4a32_d192_ed03) | 1);
        let dag = history(&mut dice, 400, 60);
        let (rows, _) = lay_out(&dag);
        assert_drawable(&dag, &rows, false);
    }
}

/// Tails are the lines due to be given up soonest, so a full column takes them back first,
/// but never one drawn on the row that needs the lane: in the kernel a merge's first parent
/// got a tail and its second parent the same lane, both drawn down it to the second.
#[test]
fn a_saturated_graph_with_far_forks_never_ends_a_line_at_another_commit() {
    for seed in 1..=200_u64 {
        let mut dice = Dice(seed.wrapping_mul(0xa076_1d64_78bd_642f) | 1);
        let dag = history(&mut dice, 400, 60);
        let mut assigner = LaneAssigner::<usize>::new();
        let rows: Vec<RowTopology> = dag
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if dice.roll(2) == 0 {
                    assigner.push_far(&i, p)
                } else {
                    assigner.push(&i, p)
                }
            })
            .collect();
        assert_drawable(&dag, &rows, false);
        for (i, row) in rows.iter().enumerate() {
            let drawn: Vec<u16> = row
                .parent_lanes
                .iter()
                .copied()
                .filter(|&l| l != NO_LANE)
                .collect();
            let mut unique = drawn.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(
                unique.len(),
                drawn.len(),
                "seed {seed}, row {i}: two parents down one lane {drawn:?}"
            );
        }
    }
}

/// A merge joining lines already running to some of its parents uses those lanes as much as
/// the ones it opens: a full column must not take them back for its later parents. The
/// kernel's 66-parent octopus came to a full column, joined seven lines running to its parents,
/// and then handed those seven lanes to seven more of its parents, whose lines all ended at
/// the wrong commits.
#[test]
fn an_octopus_in_a_full_column_never_hands_a_lane_it_joined_to_another_parent() {
    let mut assigner = LaneAssigner::<u32>::new();
    // Thirty-one tips with a line each, filling the column; the ten newest lead to commits the
    // octopus has for parents too, and it comes to them before the others.
    for tip in 0..31_u32 {
        assigner.push(&tip, &[100 + tip]);
    }
    let parents: Vec<u32> = std::iter::once(300)
        .chain(121..131)
        .chain(301..331)
        .collect();
    let row = assigner.push(&50, &parents);
    for (k, &lane) in row.parent_lanes.iter().enumerate() {
        if lane == NO_LANE {
            continue;
        }
        let twice = row.parent_lanes[..k].contains(&lane);
        assert!(
            !twice,
            "parent {k} drawn down lane {lane}, which an earlier parent has"
        );
    }
    assert!(
        row.parent_lanes.contains(&NO_LANE),
        "the column never ran out, so nothing was tested"
    );
}

#[test]
fn the_open_mask_names_every_lane_entering_a_row() {
    // A renderer holding only a window of rows relies on this mask alone to know a lane is
    // live, so it has to agree with a full scan for every shape the assigner can produce.
    let dags: &[&[&[usize]]] = &[
        &[&[1], &[2], &[3], &[]],
        &[&[1, 2], &[3], &[3], &[]],
        &[&[2], &[2], &[]],
        &[&[1, 2, 3, 4], &[5], &[5], &[5], &[5], &[]],
        &[&[2, 3], &[2, 3], &[4], &[4], &[]],
        &[&[2, 1], &[3], &[4, 3], &[5], &[5], &[]],
    ];
    for dag in dags {
        let (rows, _) = lay_out(dag);
        assert_drawable(dag, &rows, true);
    }
}

#[test]
fn a_lane_stays_open_across_a_long_run() {
    // The kernel's first-parent chain runs for hundreds of thousands of rows between merges.
    // Every row in between must report the lane as open, or the renderer draws nothing for it.
    let mut dag: Vec<Vec<usize>> = vec![vec![1, 200]];
    for i in 1..200 {
        dag.push(vec![i + 1]);
    }
    dag.push(vec![]);
    let refs: Vec<&[usize]> = dag.iter().map(Vec::as_slice).collect();

    let mut assigner = LaneAssigner::<usize>::new();
    let rows: Vec<_> = refs
        .iter()
        .enumerate()
        .map(|(i, p)| assigner.push(&i, p))
        .collect();

    // Row 0's second parent is the final root, so its lane is held open the whole way down.
    let held = rows[0].parent_lanes[1];
    for (i, row) in rows.iter().enumerate().skip(1) {
        assert!(
            row.open & (1 << held) != 0,
            "row {i} lost the lane reserved at row 0"
        );
    }
}

/// A line another commit has joined is as new as that commit.
///
/// Lines are given up a few hundred rows after they were drawn, but at kernel row 25302 a
/// merge joined a line drawn long before, which was then given up on the very next row: the
/// merge's line went out to lane 13 and stopped, and its parent, right under it, was drawn as
/// a tip in lane 9.
#[test]
fn a_line_joined_late_runs_on_to_its_commit() {
    // Row 0 is a merge whose second parent is 600; row 400 merges 600 again, and the chain
    // from row 0 leads down to 600 too.
    let mut dag: Vec<Vec<usize>> = vec![vec![1, 600]];
    for i in 1..600 {
        dag.push(if i == 400 {
            vec![i + 1, 600]
        } else {
            vec![i + 1]
        });
    }
    dag.push(vec![]);
    let (rows, _) = lay_out(&dag);
    assert_drawable(&dag, &rows, false);

    let joined = rows[400].parent_lanes[1];
    assert_eq!(
        joined, rows[0].parent_lanes[1],
        "row 400 joins row 0's line"
    );
    assert_eq!(rows[600].lane, 0);
    assert_ne!(
        rows[600].joins & (1 << joined),
        0,
        "the line row 400 joined never reached its commit"
    );
}

/// A graph wider than the column can show is a graph nobody can read.
///
/// The kernel's stable-tree merges have ninety branches open at once, and the assigner handed
/// out lane 89 for them: every node was drawn off the side of the column and the graph looked
/// empty. Capped, every commit lands somewhere that can be drawn.
#[test]
fn the_graph_is_never_wider_than_it_can_be_drawn() {
    let mut assigner: LaneAssigner<u32> = LaneAssigner::new();

    // One commit opening two lanes, a hundred times over: every parent is off in the future,
    // so nothing is ever freed and the width only grows.
    let mut next = 1_000_u32;
    for child in 0..100_u32 {
        let parents = [next, next + 1];
        next += 2;
        let topo = assigner.push(&child, &parents);
        assert!(topo.lane < MAX_LANES, "a node outside the drawable column");
        assert!(topo.width <= MAX_LANES, "wider than the column can hold");
        for lane in &topo.parent_lanes {
            assert!(
                *lane == NO_LANE || *lane < MAX_LANES,
                "an edge to a lane nothing is drawn in"
            );
        }
    }
    assert!(assigner.max_width() <= MAX_LANES);
}

/// The spill lane must never hold a reservation, or it would cut another line short.
///
/// A run in a lane ends at the row drawn in that lane. Two unrelated commits sharing the spill
/// lane is fine; a reservation living there while another commit is drawn in it is not.
#[test]
fn nothing_is_ever_reserved_in_the_lane_of_last_resort() {
    let mut assigner: LaneAssigner<u32> = LaneAssigner::new();
    let mut next = 1_000_u32;
    for child in 0..200_u32 {
        let parents = [next, next + 1];
        next += 2;
        let topo = assigner.push(&child, &parents);
        assert!(
            !topo.parent_lanes.contains(&(MAX_LANES - 1)),
            "a parent was reserved in the spill lane"
        );
        // The open mask covers exactly the drawable lanes, so nothing it reports is unshowable.
        assert_eq!(topo.open & (1 << (MAX_LANES - 1)), 0);
    }
}

/// Saturation must not push every commit into one column.
///
/// The kernel keeps sixty-odd branches open at once deep in its history. With thirty-one
/// lanes every one was reserved for a merge parent tens of thousands of rows below, so from
/// row 555 on every commit was drawn in the lane of last resort: one column of nodes beside
/// thirty lines belonging to nothing on screen. The oldest reservation is recycled instead.
#[test]
fn a_full_graph_recycles_its_oldest_lane_rather_than_stacking_up() {
    let mut assigner: LaneAssigner<u32> = LaneAssigner::new();

    // Forty merges, each opening a lane for a parent that never arrives: more than the column
    // holds, so the early ones have to give their lanes back.
    let mut next = 10_000_u32;
    let mut lanes = Vec::new();
    for child in 0..40_u32 {
        let parents = [next, next + 1];
        next += 2;
        lanes.push(assigner.push(&child, &parents).lane);
    }

    let spill = MAX_LANES - 1;
    assert!(
        !lanes[5..].contains(&spill),
        "commits are still being stacked in the lane of last resort: {lanes:?}"
    );
    assert!(
        lanes.iter().collect::<std::collections::HashSet<_>>().len() > 1,
        "every commit landed in the same lane: {lanes:?}"
    );
}

/// What eviction costs, stated once so it cannot change by accident.
///
/// The line from a merge to a parent far below stops rather than joining the wrong commit:
/// the lane is closed in the `open` mask from the row it was taken back.
#[test]
fn a_recycled_lane_is_closed_before_it_is_handed_out_again() {
    let mut assigner: LaneAssigner<u32> = LaneAssigner::new();

    let mut next = 10_000_u32;
    let mut rows = Vec::new();
    for child in 0..60_u32 {
        let parents = [next, next + 1];
        next += 2;
        rows.push(assigner.push(&child, &parents));
    }

    // Every row here is a tip with two parents that never arrive, so every lane it is drawn in
    // or draws down is new, and a line entering one of them from above would run on into this
    // row's node or edge. Taken back, the old line has to stop above the row.
    let mut recycled = 0;
    for (i, row) in rows.iter().enumerate() {
        assert!(row.lane < MAX_LANES, "row {i} is outside the column");
        for &lane in row.parent_lanes.iter().chain([&row.lane]) {
            if lane == NO_LANE || lane == SPILL {
                continue;
            }
            assert!(
                lane < MAX_LANES,
                "row {i} points at a lane nothing is drawn in"
            );
            assert_eq!(
                row.open & (1 << lane),
                0,
                "row {i}: the line in lane {lane} runs on into a commit it was not drawn for"
            );
            if rows[..i].iter().any(|r| r.parent_lanes.contains(&lane)) {
                recycled += 1;
            }
        }
    }
    assert!(
        recycled > 0,
        "nothing was taken back, so nothing was tested"
    );
}

/// Branch lines bend one way, which is what makes a graph read as a trunk with branches.
///
/// A lane to the left of the merge is nearer, but taking it draws a branch that leaves
/// leftwards and comes back, and a screen of those reads as a knot.
#[test]
fn a_merge_opens_its_extra_lane_to_the_right() {
    let mut assigner: LaneAssigner<u32> = LaneAssigner::new();

    // Three tips, so lanes 0, 1 and 2 are taken; then lane 0 falls free.
    assigner.push(&0, &[10]);
    assigner.push(&1, &[11]);
    assigner.push(&2, &[12]);
    assigner.push(&10, &[]); // lane 0 dies here

    // A merge sitting in lane 1 must open lane 3, not the free lane 0 to its left.
    let merge = assigner.push(&11, &[20, 21]);
    assert_eq!(merge.lane, 1);
    let opened = merge.parent_lanes[1];
    assert!(
        opened > merge.lane,
        "the branch bends left, into lane {opened}, from lane {}",
        merge.lane
    );
}

/// Every rule at once, over far more histories than the suite runs: near and far forks mixed,
/// a line kept left, graphs from a handful of commits to past the column's width. Too slow for
/// every gate; `CORAL_FUZZ_SEEDS` sets how many, 20,000 when unset.
///
/// `cargo test -p coral-core --all-features --test lanes -- --ignored`
#[test]
#[ignore = "a long run; see the doc comment"]
fn deep_fuzz_every_rule_at_once() {
    let seeds: u64 = std::env::var("CORAL_FUZZ_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    let mut saturated = 0;
    for seed in 1..=seeds {
        let mut dice = Dice(seed.wrapping_mul(0xbf58_476d_1ce4_e5b9) | 1);
        let commits = 2 + dice.roll(if seed % 10 == 0 { 600 } else { 150 });
        let branches = 1 + dice.roll(if seed % 7 == 0 { 70 } else { 12 });
        let dag = history(&mut dice, commits, branches);
        let far_odds = dice.roll(4);
        let kept = (seed % 3 == 0).then(|| dice.roll(commits));
        let mut assigner = LaneAssigner::<usize>::new();
        if let Some(k) = kept {
            assigner.keep_left(k);
        }
        let mut any_far = false;
        let rows: Vec<RowTopology> = dag
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if far_odds > 0 && dice.roll(far_odds + 1) == 0 {
                    any_far = true;
                    assigner.push_far(&i, p)
                } else {
                    assigner.push(&i, p)
                }
            })
            .collect();
        let full = assigner.max_width() >= SPILL;
        saturated += usize::from(full);
        assert_drawable(&dag, &rows, !full && !any_far);
        assert_branches_hold(&rows);
        for (i, row) in rows.iter().enumerate() {
            let drawn: Vec<u16> = row
                .parent_lanes
                .iter()
                .copied()
                .filter(|&l| l != NO_LANE)
                .collect();
            let mut unique = drawn.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(
                unique.len(),
                drawn.len(),
                "seed {seed}, row {i}: two parents down one lane {drawn:?}"
            );
        }
        if let Some(k) = kept {
            assert_eq!(rows[k].lane, 0, "seed {seed}: the kept commit left lane 0");
        }
    }
    eprintln!("{seeds} histories, {saturated} of them filling the column");
}
