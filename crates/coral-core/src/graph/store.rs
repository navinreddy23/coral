use gix::ObjectId;

use super::lanes::LaneAssigner;
use super::stream::{CommitNode, CommitStream, StreamOpts, WalkControl};
use crate::error::CoralError;

/// A parent that is not in the loaded set: a boundary of the walk, or a shallow graft.
pub const NO_ROW: u32 = u32::MAX;

/// Per-row bit flags.
pub mod flags {
    pub const MERGE: u8 = 1 << 0;
    pub const ROOT: u8 = 1 << 1;
    /// At least one parent lies outside the loaded rows.
    pub const BOUNDARY: u8 = 1 << 2;
    /// Placed by a commit-time walk, so its position is provisional until the topological
    /// walk replaces it.
    pub const PROVISIONAL: u8 = 1 << 3;
}

/// The commit graph as a struct of arrays.
///
/// Nothing here is per-row heap-allocated. The naive shape — a `Vec` of structs holding
/// `String` summaries and a `Vec` of parent ids — costs roughly 400 MB on the kernel before
/// allocator overhead, against a 600 MB budget for the whole application.
///
/// Parents are stored CSR rather than as a `SmallVec` per row: `SmallVec<[u32; 2]>` is 24
/// bytes on x86-64, so 1.48M of them would cost 35 MB and scatter across the heap, where the
/// CSR pair costs 12 MB and scans linearly.
pub struct RowStore {
    /// Object ids packed end to end; `hash_len` bytes each, so SHA-256 repositories work
    /// without a second code path.
    oids: Vec<u8>,
    hash_len: usize,
    lane: Vec<u16>,
    open: Vec<u32>,
    flags: Vec<u8>,
    time: Vec<i64>,
    /// CSR offsets into `parent_row`; length is `len + 1`.
    parent_start: Vec<u32>,
    parent_row: Vec<u32>,
    parent_lane: Vec<u16>,
    /// Row numbers ordered by object id, so a lookup is a binary search rather than a hash
    /// map. A `HashMap<ObjectId, u32>` would cost about 60 MB here; this costs 6 MB.
    by_oid: Vec<u32>,
    max_lane: u16,
}

impl RowStore {
    #[must_use]
    pub fn len(&self) -> u32 {
        u32::try_from(self.lane.len()).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lane.is_empty()
    }

    #[must_use]
    pub const fn max_lane(&self) -> u16 {
        self.max_lane
    }

    /// The commit drawn on `row`.
    #[must_use]
    pub fn oid(&self, row: u32) -> Option<ObjectId> {
        let start = (row as usize).checked_mul(self.hash_len)?;
        let bytes = self.oids.get(start..start + self.hash_len)?;
        ObjectId::from_bytes_or_panic(bytes).into()
    }

    #[must_use]
    pub fn lane(&self, row: u32) -> Option<u16> {
        self.lane.get(row as usize).copied()
    }

    /// Lanes with an edge entering `row` from above, as a bitmask over lanes 0..32.
    #[must_use]
    pub fn open(&self, row: u32) -> u32 {
        self.open.get(row as usize).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn time(&self, row: u32) -> Option<i64> {
        self.time.get(row as usize).copied()
    }

    #[must_use]
    pub fn flags(&self, row: u32) -> u8 {
        self.flags.get(row as usize).copied().unwrap_or(0)
    }

    /// Rows of this commit's parents. [`NO_ROW`] marks a parent outside the loaded set.
    #[must_use]
    pub fn parents(&self, row: u32) -> &[u32] {
        self.slice(&self.parent_row, row)
    }

    /// The lane each parent is drawn in, positionally matching [`RowStore::parents`].
    #[must_use]
    pub fn parent_lanes(&self, row: u32) -> &[u16] {
        self.slice(&self.parent_lane, row)
    }

    fn slice<'a, T>(&'a self, flat: &'a [T], row: u32) -> &'a [T] {
        let (Some(&s), Some(&e)) = (
            self.parent_start.get(row as usize),
            self.parent_start.get(row as usize + 1),
        ) else {
            return &[];
        };
        flat.get(s as usize..e as usize).unwrap_or(&[])
    }

    /// The row a commit is drawn on, or `None` if it is not loaded.
    #[must_use]
    pub fn row_of(&self, id: &ObjectId) -> Option<u32> {
        let target = id.as_bytes();
        self.by_oid
            .binary_search_by(|probe| self.oid_bytes(*probe).cmp(target))
            .ok()
            .map(|i| self.by_oid[i])
    }

    fn oid_bytes(&self, row: u32) -> &[u8] {
        let start = row as usize * self.hash_len;
        self.oids.get(start..start + self.hash_len).unwrap_or(&[])
    }

    /// Bytes held by the store itself, which the kernel budget is asserted against.
    #[must_use]
    pub fn bytes_resident(&self) -> usize {
        self.oids.capacity()
            + self.lane.capacity() * 2
            + self.open.capacity() * 4
            + self.flags.capacity()
            + self.time.capacity() * 8
            + self.parent_start.capacity() * 4
            + self.parent_row.capacity() * 4
            + self.parent_lane.capacity() * 2
            + self.by_oid.capacity() * 4
    }
}

/// Accumulates rows as a [`CommitStream`] emits them.
///
/// Parent *rows* cannot be known while walking: topological order guarantees a parent comes
/// after its child, so the child is written before the parent has a row number. Parent object
/// ids are therefore buffered and resolved in one pass at the end.
pub struct RowStoreBuilder {
    oids: Vec<u8>,
    hash_len: usize,
    lane: Vec<u16>,
    open: Vec<u32>,
    flags: Vec<u8>,
    time: Vec<i64>,
    parent_start: Vec<u32>,
    parent_oids: Vec<u8>,
    parent_lane: Vec<u16>,
    assigner: LaneAssigner<ObjectId>,
    provisional: bool,
}

impl RowStoreBuilder {
    #[must_use]
    pub fn new(provisional: bool) -> Self {
        Self {
            oids: Vec::new(),
            hash_len: 0,
            lane: Vec::new(),
            open: Vec::new(),
            flags: Vec::new(),
            time: Vec::new(),
            parent_start: vec![0],
            parent_oids: Vec::new(),
            parent_lane: Vec::new(),
            assigner: LaneAssigner::new(),
            provisional,
        }
    }

    /// Reserves space for a known row count, avoiding repeated reallocation of eight arrays.
    pub fn reserve(&mut self, rows: usize) {
        self.lane.reserve(rows);
        self.open.reserve(rows);
        self.flags.reserve(rows);
        self.time.reserve(rows);
        self.parent_start.reserve(rows + 1);
    }

    pub fn push(&mut self, node: &CommitNode) {
        if self.hash_len == 0 {
            self.hash_len = node.id.as_bytes().len();
        }
        let topo = self.assigner.push(&node.id, &node.parents);

        self.oids.extend_from_slice(node.id.as_bytes());
        self.lane.push(topo.lane);
        self.open.push(topo.open);
        self.time.push(node.commit_time);

        let mut f = 0_u8;
        if node.parents.len() > 1 {
            f |= flags::MERGE;
        }
        if node.parents.is_empty() {
            f |= flags::ROOT;
        }
        if self.provisional {
            f |= flags::PROVISIONAL;
        }
        self.flags.push(f);

        for p in &node.parents {
            self.parent_oids.extend_from_slice(p.as_bytes());
        }
        self.parent_lane.extend_from_slice(&topo.parent_lanes);
        let edges = u32::try_from(self.parent_lane.len()).unwrap_or(u32::MAX);
        self.parent_start.push(edges);
    }

    /// Resolves parent ids to rows and builds the lookup index.
    #[must_use]
    pub fn finish(self) -> RowStore {
        let hash_len = if self.hash_len == 0 {
            20
        } else {
            self.hash_len
        };
        let rows = self.lane.len();

        let mut by_oid: Vec<u32> = (0..u32::try_from(rows).unwrap_or(u32::MAX)).collect();
        by_oid.sort_unstable_by(|a, b| {
            let (a, b) = (*a as usize * hash_len, *b as usize * hash_len);
            self.oids[a..a + hash_len].cmp(&self.oids[b..b + hash_len])
        });

        let lookup = |needle: &[u8]| -> u32 {
            by_oid
                .binary_search_by(|probe| {
                    let s = *probe as usize * hash_len;
                    self.oids[s..s + hash_len].cmp(needle)
                })
                .map_or(NO_ROW, |i| by_oid[i])
        };

        let mut parent_row = vec![NO_ROW; self.parent_oids.len() / hash_len];
        let mut flags = self.flags;
        // Walk rows rather than edges, so the owning row of each edge is known outright.
        for (row, window) in self.parent_start.windows(2).enumerate() {
            let (start, end) = (window[0] as usize, window[1] as usize);
            for (k, slot) in parent_row[start..end].iter_mut().enumerate() {
                let at = (start + k) * hash_len;
                *slot = lookup(&self.parent_oids[at..at + hash_len]);
                if *slot == NO_ROW {
                    flags[row] |= flags::BOUNDARY;
                }
            }
        }

        RowStore {
            oids: self.oids,
            hash_len,
            lane: self.lane,
            open: self.open,
            flags,
            time: self.time,
            parent_start: self.parent_start,
            parent_row,
            parent_lane: self.parent_lane,
            by_oid,
            max_lane: self.assigner.max_width().saturating_sub(1),
        }
    }
}

/// Walks `stream` into a [`RowStore`].
///
/// # Errors
/// Propagates walk failures.
pub fn build(stream: &dyn CommitStream, opts: &StreamOpts) -> Result<RowStore, CoralError> {
    let mut builder = RowStoreBuilder::new(opts.order == super::Order::CommitTime);
    if let Some(n) = opts.max_count {
        builder.reserve(usize::try_from(n).unwrap_or(0));
    }
    stream.walk(opts, &mut |node| {
        builder.push(&node);
        WalkControl::Continue
    })?;
    Ok(builder.finish())
}
