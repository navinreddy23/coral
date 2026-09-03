use std::fmt::Write as _;
use std::path::Path;

use coral_core::CoralError;
use coral_core::graph::{GixCommitStream, NO_ROW, Order, StreamOpts, build, flags};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub row: u32,
    pub oid: String,
    pub lane: u16,
    /// `null` for a parent outside the loaded rows, rather than a sentinel integer.
    pub parents: Vec<Option<u32>>,
    pub parent_lanes: Vec<u16>,
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
) -> Result<Graph, CoralError> {
    let stream = GixCommitStream::open(path)?;
    let opts = if first_paint {
        StreamOpts {
            order: Order::CommitTime,
            max_count: limit,
            ..StreamOpts::default()
        }
    } else {
        StreamOpts {
            max_count: limit,
            ..StreamOpts::default()
        }
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
        provisional: first_paint,
    })
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
        for r in &self.rows {
            let mark = if r.merge {
                "M"
            } else if r.root {
                "R"
            } else {
                " "
            };
            let _ = write!(
                out,
                "\n  {:>7} {mark} lane {:<4} {:.8}",
                r.row, r.lane, r.oid
            );
        }
        out
    }
}
