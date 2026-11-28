use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::graph::{GixCommitStream, NO_ROW, Order, StreamOpts, Tips, art, build, flags};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub row: u32,
    pub oid: String,
    pub lane: u16,
    /// `null` for a parent outside the loaded rows, rather than a sentinel integer.
    pub parents: Vec<Option<u32>>,
    pub parent_lanes: Vec<u16>,
    /// Lanes whose line turns into this commit on its row: the branches that left from it.
    pub joins: Vec<u16>,
    /// Lanes with a line entering the row from above.
    pub open: Vec<u16>,
    pub time: i64,
    pub merge: bool,
    pub root: bool,
    pub boundary: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub rows: Vec<Row>,
    /// Rows in the whole walk, which exceeds `rows.len()` when a limit was applied.
    pub total: u32,
    pub lanes: u16,
    /// True when the rows came from a commit-time walk and are not yet topologically sound.
    pub provisional: bool,
}

/// Walks the graph and returns a window of rows.
///
/// # Errors
/// [`CoralError::NotARepository`] when `path` is not a repository, or any walk failure.
pub async fn run(
    path: &Path,
    limit: Option<u64>,
    from: u32,
    first_paint: bool,
    solo: Option<String>,
    hidden: Vec<String>,
) -> Result<Graph, CoralError> {
    let stream = GixCommitStream::open(path)?;
    let opts = StreamOpts {
        tips: tips_from(solo, hidden),
        order: if first_paint {
            stream.first_paint_order()
        } else {
            Order::Topological
        },
        max_count: limit,
        ..StreamOpts::default()
    };

    let store = tokio::task::spawn_blocking(move || build(&stream, &opts))
        .await
        .map_err(|e| CoralError::Protocol {
            label: "graph",
            detail: e.to_string(),
        })??;

    let end = store.len();
    let rows = (from.min(end)..end)
        .map(|row| Row {
            row,
            oid: store.oid(row).map(|o| o.to_string()).unwrap_or_default(),
            lane: store.lane(row).unwrap_or_default(),
            parents: store
                .parents(row)
                .iter()
                .map(|p| (*p != NO_ROW).then_some(*p))
                .collect(),
            parent_lanes: store.parent_lanes(row).to_vec(),
            joins: lanes_in(store.joins(row)),
            open: lanes_in(store.open(row)),
            time: store.time(row).unwrap_or_default(),
            merge: store.flags(row) & flags::MERGE != 0,
            root: store.flags(row) & flags::ROOT != 0,
            boundary: store.flags(row) & flags::BOUNDARY != 0,
        })
        .collect();

    Ok(Graph {
        rows,
        total: end,
        lanes: store.max_lane().saturating_add(1),
        provisional: !store.is_empty() && store.flags(0) & flags::PROVISIONAL != 0,
    })
}

fn lanes_in(mask: u32) -> Vec<u16> {
    (0..32).filter(|lane| mask & (1 << lane) != 0).collect()
}

/// Where the walk starts, from the two flags.
///
/// Solo wins over hiding, as it does in the window: it is the narrower answer, and a walk that
/// tried to honour both would have to say which of the two the user is looking at.
fn tips_from(solo: Option<String>, hidden: Vec<String>) -> Tips {
    match solo {
        Some(name) => Tips::Only(vec![name]),
        None if hidden.is_empty() => Tips::All,
        None => Tips::Except(hidden),
    }
}

impl crate::output::Human for Graph {
    fn human(&self) -> String {
        let mut out = format!(
            "{} rows, {} lanes{}",
            self.total,
            self.lanes,
            if self.provisional {
                " (provisional)"
            } else {
                ""
            }
        );
        let masks: Vec<(u32, u32)> = self
            .rows
            .iter()
            .map(|r| (mask_of(&r.open), mask_of(&r.joins)))
            .collect();
        let rows: Vec<art::Row<'_>> = self
            .rows
            .iter()
            .zip(&masks)
            .map(|(r, &(open, joins))| art::Row {
                lane: r.lane,
                parent_lanes: &r.parent_lanes,
                open,
                joins,
            })
            .collect();
        for (r, drawn) in self.rows.iter().zip(art::draw(&rows)) {
            let _ = write!(out, "\n  {:>7} {}  {:.8}", r.row, drawn.node, r.oid);
            let _ = write!(out, "\n  {:>7} {}", "", drawn.below.trim_end());
        }
        out
    }
}

fn mask_of(lanes: &[u16]) -> u32 {
    lanes
        .iter()
        .filter(|&&lane| lane < 32)
        .fold(0, |mask, &lane| mask | 1 << lane)
}
