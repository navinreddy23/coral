use smallvec::SmallVec;

/// No lane assigned.
pub const NO_LANE: u16 = u16::MAX;

/// Where a row sits and where its edges go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowTopology {
    /// The lane the commit's node is drawn in.
    pub lane: u16,
    /// The lane each parent will be drawn in, in parent order.
    pub parent_lanes: SmallVec<[u16; 2]>,
    /// Lanes occupied after this row, i.e. the graph's width here.
    pub width: u16,
    /// Bit `n` set when lane `n` has an edge entering this row from above.
    ///
    /// A renderer that only ever sees a window of rows cannot derive this: an edge spanning a
    /// million rows is owned by a child far off the top of the screen, and without this mask
    /// the lane simply is not drawn for the whole span between its endpoints. Lanes past 31
    /// are omitted; the graph column cannot show them.
    pub open: u32,
}

/// Assigns commits to vertical lanes as the walk emits them.
///
/// The invariant that makes the wire format work: a lane holds at most one reservation, and a
/// reservation is cleared only when its commit is emitted. So a lane reserved for parent `P`
/// at row `c` is continuously occupied from `c` until `P`'s own row, and exactly one commit is
/// ever drawn in it over that span. That is why no "lanes ending here" list has to be sent —
/// a vertical run in lane `L` ends at precisely the row whose own lane is `L`.
///
/// Rendering convention is an immediate join: an edge from `(row c, lane Lc)` to a parent in
/// `Lp != Lc` leaves `Lc` at row `c` and joins `Lp` by row `c+1`, then runs straight down.
/// This keeps `Lc` free rather than holding it open for a possibly million-row span.
///
/// Requires that children are emitted before their parents, which is what
/// [`super::Order::Topological`] guarantees.
/// Keyed by whatever identifies a commit to the caller: an object id in the engine, a plain
/// index in the tests. A parent's row number is not known when its child is emitted, so the
/// key cannot be the row.
pub struct LaneAssigner<K> {
    /// Lane -> the commit it is reserved for, or `None` when free.
    lanes: Vec<Option<K>>,
    /// Reservations still outstanding. Bounded by the graph's width, not its length: an entry
    /// lives only from the child that claims a lane until the parent that consumes it.
    reserved: std::collections::HashMap<K, u16>,
    /// Lanes freed on the current row; never reused until the next one, so a lane does not
    /// appear to teleport across a single row.
    freed_this_row: SmallVec<[u16; 4]>,
    max_width: u16,
}

impl<K: Eq + std::hash::Hash + Clone> Default for LaneAssigner<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + std::hash::Hash + Clone> LaneAssigner<K> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lanes: Vec::new(),
            reserved: std::collections::HashMap::new(),
            freed_this_row: SmallVec::new(),
            max_width: 0,
        }
    }

    /// Widest the graph has been so far.
    #[must_use]
    pub const fn max_width(&self) -> u16 {
        self.max_width
    }

    /// Places one commit and reserves lanes for its parents.
    ///
    /// `index` identifies this commit and `parents` are the indices its parents will be given.
    /// Indices rather than object ids keep the assigner independent of the hash length.
    pub fn push(&mut self, key: &K, parents: &[K]) -> RowTopology {
        self.freed_this_row.clear();
        let open = self.open_mask();

        // A child may already have reserved a lane for me; otherwise I am a branch tip.
        let lane = match self.reserved.remove(key) {
            Some(l) => {
                self.lanes[l as usize] = None;
                l
            }
            None => self.alloc(),
        };

        let mut parent_lanes = SmallVec::new();
        let mut kept_my_lane = false;
        for (i, p) in parents.iter().enumerate() {
            if let Some(&existing) = self.reserved.get(p) {
                // Another child already claimed this parent: a fork joining back, or one side
                // of a criss-cross. Draw a diagonal into that lane rather than opening one.
                parent_lanes.push(existing);
                continue;
            }
            let pl = if i == 0 && !kept_my_lane {
                kept_my_lane = true;
                lane
            } else {
                self.alloc()
            };
            self.lanes[pl as usize] = Some(p.clone());
            self.reserved.insert(p.clone(), pl);
            parent_lanes.push(pl);
        }

        // A root, or a first parent that was already reserved elsewhere: my lane dies here.
        //
        // Unless a later parent was allocated into it. Claiming my lane above left its slot
        // free, so `alloc` is entitled to hand it straight back out; freeing it then would
        // destroy a live reservation and leave the map pointing at a trimmed-away slot.
        if !kept_my_lane && !parent_lanes.contains(&lane) {
            self.free(lane);
        }
        self.trim();

        // The row is drawn in `lane` even when that lane dies here — a fork rejoining takes a
        // lane for one row and diagonals out of it — so width must cover it, not just the
        // lanes still live after the trim.
        let live = u16::try_from(self.lanes.len()).unwrap_or(u16::MAX);
        let width = live.max(lane.saturating_add(1));
        self.max_width = self.max_width.max(width);
        RowTopology {
            lane,
            parent_lanes,
            width,
            open,
        }
    }

    /// Lanes holding a live reservation, as a bitmask over lanes 0..32.
    fn open_mask(&self) -> u32 {
        let mut mask = 0_u32;
        for (i, slot) in self.lanes.iter().take(32).enumerate() {
            if slot.is_some() {
                mask |= 1 << i;
            }
        }
        mask
    }

    /// Leftmost free lane, skipping any freed on this row.
    fn alloc(&mut self) -> u16 {
        for (i, slot) in self.lanes.iter().enumerate() {
            let lane = u16::try_from(i).unwrap_or(u16::MAX);
            if slot.is_none() && !self.freed_this_row.contains(&lane) {
                return lane;
            }
        }
        self.lanes.push(None);
        u16::try_from(self.lanes.len() - 1).unwrap_or(u16::MAX)
    }

    fn free(&mut self, lane: u16) {
        if let Some(slot) = self.lanes.get_mut(lane as usize) {
            *slot = None;
        }
        self.freed_this_row.push(lane);
    }

    /// Drops trailing free lanes so `width` reflects the live graph.
    fn trim(&mut self) {
        while self.lanes.last().is_some_and(Option::is_none) {
            self.lanes.pop();
        }
    }
}
