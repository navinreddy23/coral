//! What every drawable layout has to satisfy, shared by the synthetic DAGs in `lanes.rs` and
//! the real repositories in `shapes.rs`.

use coral_core::graph::{MAX_LANES, NO_LANE, RowTopology, art};

/// The lane of last resort, which may never hold a line.
pub const SPILL: u16 = MAX_LANES - 1;

/// Where the line started by an edge out of row `from` in `lane` ends: the first row below
/// drawn in that lane or joining from it.
///
/// The open mask is not consulted, so the mask can be checked against this rather than
/// sharing a bug with it.
fn line_end(rows: &[RowTopology], from: usize, lane: u16) -> Option<usize> {
    for (r, row) in rows.iter().enumerate().skip(from + 1) {
        if row.lane == lane || row.joins & (1 << lane) != 0 {
            return Some(r);
        }
    }
    None
}

/// Everything a drawing of `rows` rests on.
///
/// `strict` for a graph that never filled the column and never held a line long enough to be
/// given back: there, every line must end at exactly the commit it was drawn for, and the open
/// mask must agree with a full scan. A saturated graph is allowed to stop lines short.
pub fn assert_drawable<P: AsRef<[usize]>>(dag: &[P], rows: &[RowTopology], strict: bool) {
    let mut entering = vec![0_u32; rows.len()];
    for (i, row) in rows.iter().enumerate() {
        assert!(row.lane < MAX_LANES, "row {i} is outside the column");
        assert_eq!(row.joins & (1 << row.lane), 0, "row {i} joins its own lane");
        for lane in 0..MAX_LANES {
            if row.joins & (1 << lane) == 0 {
                continue;
            }
            assert!(
                lane > row.lane,
                "row {i}: lane {lane} turns into the node from its left, in lane {}",
                row.lane
            );
            assert_ne!(
                row.open & (1 << lane),
                0,
                "row {i}: lane {lane} joins with no line in it"
            );
        }
        for (k, &lane) in row.parent_lanes.iter().enumerate() {
            if lane == NO_LANE {
                continue;
            }
            assert_ne!(
                lane, SPILL,
                "row {i} holds a line in the lane of last resort"
            );
            if k == 0 && row.lane != SPILL {
                assert_eq!(
                    lane, row.lane,
                    "row {i}: a first parent left its lane on the child's row"
                );
            }
            if !strict {
                continue;
            }
            let parent = dag[i].as_ref()[k];
            let end = line_end(rows, i, lane);
            assert_eq!(
                end,
                Some(parent),
                "row {i}: the line to parent {parent} in lane {lane} ends at {end:?}"
            );
            for slot in &mut entering[i + 1..=parent] {
                *slot |= 1 << lane;
            }
        }
    }
    if strict {
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(
                row.open, entering[i],
                "row {i}: the open mask {:b} disagrees with a full scan {:b}",
                row.open, entering[i]
            );
        }
    }
}

/// A drawing with a label beside each node, and the line under each row kept, so a row that
/// does not run on to the next shows as a gap.
pub fn labelled<'a>(drawn: &[art::Drawn], labels: impl Iterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for (row, label) in drawn.iter().zip(labels) {
        out.push_str(format!("{}  {label}", row.node).trim_end());
        out.push('\n');
        out.push_str(row.below.trim_end());
        out.push('\n');
    }
    out
}
