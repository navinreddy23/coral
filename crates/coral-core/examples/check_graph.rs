//! Checks every row of a real repository's layout against the rules the lane tests hold
//! generated histories to, and the first paint against the full walk.
//!
//! `cargo run --release -p coral-core --example check_graph -- <repo>`
//!
//! The generated histories are small and the rules fail in crowds: lines given up, lanes taken
//! back, branches thousands of rows from their forks. A history like the kernel's is where
//! those happen, so this walks it as the window does and reads all of it.

use std::time::Instant;

use coral_core::graph::{
    CommitStream, GixCommitStream, MAX_LANES, NO_LANE, NO_ROW, RowStore, StreamOpts, Trunk, build,
};

const SPILL: u16 = MAX_LANES - 1;

#[derive(Default)]
struct Tally {
    edges: u64,
    ended: u64,
    stopped: u64,
    dropped: u64,
    faults: Vec<String>,
}

impl Tally {
    fn fault(&mut self, what: String) {
        if self.faults.len() < 20 {
            self.faults.push(what);
        }
    }
}

fn bit(lane: u16) -> u32 {
    if lane < 32 { 1 << lane } else { 0 }
}

/// The rules one row keeps on its own.
fn check_row(store: &RowStore, row: u32, tally: &mut Tally) {
    let Some(lane) = store.lane(row) else { return };
    let joins = store.joins(row);
    if joins & bit(lane) != 0 {
        tally.fault(format!("row {row}: joins its own lane {lane}"));
    }
    for j in 0..32_u16 {
        if joins & bit(j) == 0 {
            continue;
        }
        if j <= lane {
            tally.fault(format!(
                "row {row}: lane {j} turns in from the left of lane {lane}"
            ));
        }
        if store.open(row) & bit(j) == 0 {
            tally.fault(format!("row {row}: lane {j} joins with no line in it"));
        }
    }
    let lanes = store.parent_lanes(row);
    if lane != SPILL
        && let Some(&first) = lanes.first()
        && first != NO_LANE
        && first != lane
    {
        tally.fault(format!(
            "row {row}: first parent leaves lane {lane} for {first} on its own row"
        ));
    }
    let parents = store.parents(row);
    for (k, &pl) in lanes.iter().enumerate() {
        if pl == NO_LANE {
            tally.dropped += 1;
            continue;
        }
        if pl == SPILL {
            tally.fault(format!("row {row}: a line in the lane of last resort"));
        }
        // git lets a commit name one parent twice, and both are rightly the one line to it.
        let shared = lanes[..k]
            .iter()
            .zip(parents)
            .any(|(&other, &p)| other == pl && Some(&p) != parents.get(k));
        if shared {
            tally.fault(format!("row {row}: two parents down lane {pl}"));
        }
    }
    // A line entering this row came from the row above: it ran on through it, or that row
    // drew an edge down it.
    if row > 0 {
        let above = row - 1;
        let carried = store.open(above) & !store.joins(above) & !store.lane(above).map_or(0, bit);
        let drawn = store.parent_lanes(above).iter().fold(0, |m, &l| m | bit(l));
        let appeared = store.open(row) & !(carried | drawn);
        if appeared != 0 {
            tally.fault(format!("row {row}: lanes {appeared:b} enter from nowhere"));
        }
    }
}

/// Where every edge drawn in `lane` goes, found in one pass up the graph: the next row below
/// that the lane is closed on, or whose node is in it or turns it in.
fn check_lane(store: &RowStore, lane: u16, tally: &mut Tally) {
    let mut next: Option<(u32, bool)> = None;
    for row in (0..store.len()).rev() {
        for (k, &pl) in store.parent_lanes(row).iter().enumerate() {
            if pl != lane {
                continue;
            }
            tally.edges += 1;
            let parent = store.parents(row).get(k).copied().unwrap_or(NO_ROW);
            match next {
                Some((at, true)) if at == parent => tally.ended += 1,
                Some((at, false)) if parent != NO_ROW && at <= parent => tally.stopped += 1,
                // A parent outside the walk: its line may only stop or run off the bottom.
                Some((_, false)) | None if parent == NO_ROW => tally.stopped += 1,
                other => tally.fault(format!(
                    "row {row}: the line to row {parent} in lane {lane} goes to {other:?} \
                     (row, ends there)"
                )),
            }
        }
        let open = store.open(row) & bit(lane) != 0;
        let here = store.lane(row) == Some(lane) || store.joins(row) & bit(lane) != 0;
        if !open {
            next = Some((row, false));
        } else if here {
            next = Some((row, true));
        }
    }
}

fn same_rows(full: &RowStore, first: &RowStore) -> Option<u32> {
    (0..first.len()).find(|&row| {
        full.oid(row) != first.oid(row)
            || full.lane(row) != first.lane(row)
            || full.open(row) != first.open(row)
            || full.joins(row) != first.joins(row)
            || full.parent_lanes(row) != first.parent_lanes(row)
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let stream = GixCommitStream::open(&path)?;
    let trunk: Option<Trunk> = stream.trunk();

    let t = Instant::now();
    let full = build(&stream, &StreamOpts::default())?;
    println!(
        "{} rows laid out in {} ms, {} lanes",
        full.len(),
        t.elapsed().as_millis(),
        full.max_lane() + 1
    );

    if let Some(at) = std::env::var("CHECK_ROW")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    {
        for row in at.saturating_sub(2)..=at + 1 {
            println!(
                "row {row}: lane {:?} parents {:?} lanes {:?} joins {:b} open {:b}",
                full.lane(row),
                full.parents(row),
                full.parent_lanes(row),
                full.joins(row),
                full.open(row)
            );
        }
    }
    let mut tally = Tally::default();
    for row in 0..full.len() {
        check_row(&full, row, &mut tally);
    }
    for lane in 0..SPILL {
        check_lane(&full, lane, &mut tally);
    }
    println!(
        "{} edges: {} end at their commit, {} stop before it, {} dropped in a full column",
        tally.edges, tally.ended, tally.stopped, tally.dropped
    );

    let first = build(
        &stream,
        &StreamOpts::first_paint(StreamOpts::FIRST_PAINT_ROWS, stream.first_paint_order()),
    )?;
    match same_rows(&full, &first) {
        None => println!(
            "first paint: all {} rows as the full walk lays them out",
            first.len()
        ),
        Some(row) => tally.fault(format!(
            "first paint differs from the full walk at row {row}"
        )),
    }
    if let Some(trunk) = trunk
        && let Some(head) = full.row_of(&trunk.head)
    {
        println!("HEAD on row {head}, lane {:?}", full.lane(head));
    }

    if tally.faults.is_empty() {
        println!("no faults");
        Ok(())
    } else {
        for f in &tally.faults {
            println!("FAULT {f}");
        }
        Err("the layout broke a rule".into())
    }
}
