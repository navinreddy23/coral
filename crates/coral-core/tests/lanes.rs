//! Lane assignment, rendered as ASCII and snapshotted.
//!
//! A lane bug is otherwise very hard to read from raw indices; as a picture it is a one-line
//! diff. Each DAG is written parent-last, matching the order a topological walk emits.

use std::fmt::Write as _;

use coral_core::graph::LaneAssigner;

/// Renders a DAG as lane art. `dag[i]` is the list of parent indices for commit `i`, and
/// commits are listed children-first as a topological walk would emit them.
fn render(dag: &[&[usize]], labels: &[&str]) -> String {
    let mut assigner = LaneAssigner::<usize>::new();
    let rows: Vec<_> = dag
        .iter()
        .enumerate()
        .map(|(i, p)| assigner.push(&i, p))
        .collect();
    let width = usize::from(assigner.max_width()).max(1);

    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let mut line: Vec<char> = vec![' '; width * 2];
        // Lanes that carry a vertical run through this row: reserved before it, resolved after.
        for lane in 0..width {
            if carries_through(&rows, i, lane) {
                line[lane * 2] = '|';
            }
        }
        line[usize::from(row.lane) * 2] = '*';
        let art: String = line.into_iter().collect();

        let edges: Vec<String> = row.parent_lanes.iter().map(ToString::to_string).collect();
        let label = labels.get(i).copied().unwrap_or("");
        let _ = writeln!(
            out,
            "{art}  {label:<12} lane {:<2} -> [{}]",
            row.lane,
            edges.join(",")
        );
    }
    out
}

/// True when `lane` is occupied across row `i` by a reservation made above and consumed below.
fn carries_through(rows: &[coral_core::graph::RowTopology], i: usize, lane: usize) -> bool {
    let lane = u16::try_from(lane).unwrap_or(u16::MAX);
    rows[..i].iter().enumerate().any(|(j, r)| {
        r.parent_lanes.contains(&lane) && rows[j + 1..=i].iter().all(|k| k.lane != lane)
    })
}

#[test]
fn linear_history_stays_in_one_lane() {
    let dag: &[&[usize]] = &[&[1], &[2], &[3], &[]];
    insta::assert_snapshot!(render(dag, &["d", "c", "b", "a (root)"]));
}

#[test]
fn a_merge_opens_a_lane_for_the_second_parent() {
    // m takes both a and b; they share the root r.
    let dag: &[&[usize]] = &[&[1, 2], &[3], &[3], &[]];
    insta::assert_snapshot!(render(dag, &["m (merge)", "a", "b", "r (root)"]));
}

#[test]
fn a_fork_rejoins_without_opening_a_lane() {
    // Two children of the same parent: the second draws a diagonal into the first's lane.
    let dag: &[&[usize]] = &[&[2], &[2], &[]];
    insta::assert_snapshot!(render(dag, &["child-1", "child-2", "shared parent"]));
}

#[test]
fn an_octopus_opens_a_lane_per_extra_parent() {
    let dag: &[&[usize]] = &[&[1, 2, 3, 4], &[5], &[5], &[5], &[5], &[]];
    insta::assert_snapshot!(render(dag, &["octopus", "a", "b", "c", "d", "r (root)"]));
}

#[test]
fn a_criss_cross_draws_diagonals_rather_than_new_lanes() {
    // Two merges that each reach both shared parents.
    let dag: &[&[usize]] = &[&[2, 3], &[2, 3], &[4], &[4], &[]];
    insta::assert_snapshot!(render(dag, &["merge-x", "merge-y", "a", "b", "r (root)"]));
}

#[test]
fn a_second_root_gets_its_own_lane() {
    let dag: &[&[usize]] = &[&[1], &[], &[3], &[]];
    insta::assert_snapshot!(render(dag, &["a", "root-1", "b (orphan)", "root-2"]));
}

#[test]
fn width_never_exceeds_the_number_of_live_branches() {
    let dag: &[&[usize]] = &[&[1, 2], &[3], &[3], &[]];
    let mut a = LaneAssigner::<usize>::new();
    for (i, p) in dag.iter().enumerate() {
        a.push(&i, p);
    }
    assert_eq!(
        a.max_width(),
        2,
        "a two-sided merge needs exactly two lanes"
    );
}

/// Every lane a row points at must be reachable: some later row must actually occupy it.
#[test]
fn every_reserved_lane_is_eventually_occupied() {
    let dag: &[&[usize]] = &[&[1, 2, 3, 4], &[5], &[5], &[5], &[5], &[]];
    let mut a = LaneAssigner::<usize>::new();
    let rows: Vec<_> = dag.iter().enumerate().map(|(i, p)| a.push(&i, p)).collect();

    for (i, row) in rows.iter().enumerate() {
        for (k, lane) in row.parent_lanes.iter().enumerate() {
            let parent = dag[i][k];
            assert_eq!(
                rows[parent].lane, *lane,
                "row {i} parent {parent} lane mismatch"
            );
        }
    }
}

/// Regression: a row whose first parent is already reserved does not keep its own lane, but
/// `alloc` may hand that same lane to a *later* parent. Freeing it afterwards destroyed the
/// slot while the reservation was still live, and the assigner then indexed past the end when
/// that parent was finally emitted. Only reproduced on the kernel, at 382 lanes.
#[test]
fn a_lane_handed_to_a_later_parent_is_not_freed_from_under_it() {
    let dag: &[&[usize]] = &[&[2, 3], &[2, 4], &[], &[], &[]];
    let mut a = LaneAssigner::<usize>::new();
    let rows: Vec<_> = dag.iter().enumerate().map(|(i, p)| a.push(&i, p)).collect();

    for (i, row) in rows.iter().enumerate() {
        for (k, lane) in row.parent_lanes.iter().enumerate() {
            let parent = dag[i][k];
            assert_eq!(
                rows[parent].lane, *lane,
                "row {i} reserved lane {lane} for parent {parent}, which was drawn in {}",
                rows[parent].lane
            );
        }
    }
}

/// True when `lane` holds a reservation made above row `i` that row `i` has not yet consumed.
///
/// The definition the wire format's open mask must satisfy, derived here by scanning the whole
/// history so it cannot share a bug with the incremental version the assigner keeps.
fn reserved_entering(rows: &[coral_core::graph::RowTopology], i: usize, lane: u16) -> bool {
    rows[..i].iter().enumerate().any(|(j, r)| {
        r.parent_lanes.contains(&lane) && rows[j + 1..i].iter().all(|k| k.lane != lane)
    })
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
    ];

    for (d, dag) in dags.iter().enumerate() {
        let mut assigner = LaneAssigner::<usize>::new();
        let rows: Vec<_> = dag
            .iter()
            .enumerate()
            .map(|(i, p)| assigner.push(&i, p))
            .collect();

        for (i, row) in rows.iter().enumerate() {
            for lane in 0..assigner.max_width() {
                let expected = reserved_entering(&rows, i, lane);
                let actual = row.open & (1 << lane) != 0;
                assert_eq!(
                    actual, expected,
                    "dag {d}, row {i}, lane {lane}: open mask disagrees with a full scan"
                );
            }
        }
    }
}

#[test]
fn a_lane_stays_open_across_a_long_run() {
    // The kernel's first-parent chain runs for hundreds of thousands of rows between merges.
    // Every row in between must report the lane as open, or the renderer draws nothing for it.
    let mut dag: Vec<Vec<usize>> = vec![vec![1, 2]];
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
