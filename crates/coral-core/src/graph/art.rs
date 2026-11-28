//! The graph as box-drawing text: for a terminal, and for tests to read a layout as a picture.
//!
//! Drawn from the same four facts the window draws from — a row's lane, its parents' lanes,
//! the lanes entering it and the lanes joining it — and with the same two corners: a merge
//! leaves its node across its own row and turns down (`●─╮`), a branch runs down its own lane
//! and turns into the commit it left from on that commit's row (`●─╯`).

use super::lanes::{NO_LANE, RowTopology};

/// One row, as much of it as a drawing needs.
#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
    pub lane: u16,
    pub parent_lanes: &'a [u16],
    /// Lanes with a line entering the row from above, bit `n` for lane `n`.
    pub open: u32,
    /// Lanes whose line ends on the row by turning into its node.
    pub joins: u32,
}

impl<'a> From<&'a RowTopology> for Row<'a> {
    fn from(t: &'a RowTopology) -> Self {
        Self {
            lane: t.lane,
            parent_lanes: &t.parent_lanes,
            open: t.open,
            joins: t.joins,
        }
    }
}

const UP: u8 = 1;
const DOWN: u8 = 2;
const RIGHT: u8 = 4;
const LEFT: u8 = 8;

/// A row drawn: the line with its node on it, and the line under it.
///
/// The second says which lanes run on to the row below. Without it two commits stacked in one
/// lane look the same whether one is the other's parent or the upper is a root with an
/// unrelated history starting under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drawn {
    pub node: String,
    pub below: String,
}

/// Each row drawn, lane `n` in column `2n`, every line the same width.
///
/// `rows` may be a window of a longer graph: the lines the first row's mask says are already
/// running come in from the top.
#[must_use]
pub fn draw(rows: &[Row<'_>]) -> Vec<Drawn> {
    let width = rows.iter().map(widest).max().map_or(0, |w| w + 1);
    let mut grid = Grid {
        cells: vec![vec![0; width]; rows.len()],
        links: vec![vec![false; width.saturating_sub(1)]; rows.len()],
    };

    if let Some(first) = rows.first() {
        for lane in lanes_in(first.open) {
            grid.trace(rows, 0, lane);
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let node = usize::from(row.lane);
        for &parent in row.parent_lanes {
            if parent == NO_LANE {
                continue;
            }
            let parent = usize::from(parent);
            if parent != node {
                grid.across(i, node, parent);
            }
            grid.cells[i][parent] |= DOWN;
            grid.trace(rows, i + 1, parent);
        }
        for lane in lanes_in(row.joins) {
            grid.across(i, node, lane);
        }
    }

    rows.iter()
        .enumerate()
        .map(|(i, row)| Drawn {
            node: grid.line(i, usize::from(row.lane)),
            below: grid.below(i),
        })
        .collect()
}

struct Grid {
    cells: Vec<Vec<u8>>,
    /// Whether a horizontal runs between lane `n` and lane `n + 1` on a row.
    links: Vec<Vec<bool>>,
}

impl Grid {
    /// Follows a line in `lane` down from `start` to where it ends: the row drawn in its lane,
    /// the row it joins, the row whose mask no longer has it, or the bottom.
    fn trace(&mut self, rows: &[Row<'_>], start: usize, lane: usize) {
        for (r, row) in rows.iter().enumerate().skip(start) {
            if row.open & bit(lane) == 0 {
                return;
            }
            self.cells[r][lane] |= UP;
            if usize::from(row.lane) == lane || row.joins & bit(lane) != 0 {
                return;
            }
            self.cells[r][lane] |= DOWN;
        }
    }

    /// A horizontal on row `r` from the node in `node` to `lane`.
    fn across(&mut self, r: usize, node: usize, lane: usize) {
        let (lo, hi) = (node.min(lane), node.max(lane));
        for link in &mut self.links[r][lo..hi] {
            *link = true;
        }
        for cell in &mut self.cells[r][lo + 1..hi] {
            *cell |= LEFT | RIGHT;
        }
        self.cells[r][lane] |= if lane > node { LEFT } else { RIGHT };
    }

    fn below(&self, r: usize) -> String {
        let lanes: Vec<String> = self.cells[r]
            .iter()
            .map(|&cell| if cell & DOWN == 0 { " " } else { "│" }.to_owned())
            .collect();
        lanes.join(" ")
    }

    fn line(&self, r: usize, node: usize) -> String {
        let mut out = String::new();
        for (lane, &cell) in self.cells[r].iter().enumerate() {
            out.push(if lane == node { '●' } else { glyph(cell) });
            if let Some(&link) = self.links[r].get(lane) {
                out.push(if link { '─' } else { ' ' });
            }
        }
        out
    }
}

fn glyph(cell: u8) -> char {
    match cell {
        0 => ' ',
        c if c == UP | DOWN => '│',
        c if c == LEFT | RIGHT => '─',
        c if c == UP | DOWN | LEFT | RIGHT => '┼',
        c if c == DOWN | LEFT => '╮',
        c if c == DOWN | RIGHT => '╭',
        c if c == UP | LEFT => '╯',
        c if c == UP | RIGHT => '╰',
        c if c == UP | DOWN | LEFT => '┤',
        c if c == UP | DOWN | RIGHT => '├',
        c if c == LEFT | RIGHT | DOWN => '┬',
        c if c == LEFT | RIGHT | UP => '┴',
        c if c == UP => '╵',
        c if c == DOWN => '╷',
        _ => '?',
    }
}

fn widest(row: &Row<'_>) -> usize {
    let mut widest = usize::from(row.lane);
    for &lane in row.parent_lanes {
        if lane != NO_LANE {
            widest = widest.max(usize::from(lane));
        }
    }
    for mask in [row.open, row.joins] {
        if mask != 0 {
            widest = widest.max(31 - mask.leading_zeros() as usize);
        }
    }
    widest
}

fn lanes_in(mask: u32) -> impl Iterator<Item = usize> {
    (0..32).filter(move |lane| mask & bit(*lane) != 0)
}

const fn bit(lane: usize) -> u32 {
    if lane < 32 { 1 << lane } else { 0 }
}
