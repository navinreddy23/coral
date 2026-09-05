use smallvec::SmallVec;

/// No lane assigned. A parent's edge is not drawn when its lane is this.
pub const NO_LANE: u16 = u16::MAX;

/// How many lanes the graph may be wide.
///
/// Not a tuning knob: [`RowTopology::open`] is a 32-bit mask, so a lane past 31 has no vertical
/// run and cannot be drawn as one, and a column wide enough for even this many is already most
/// of a pane. Left uncapped the assigner handed out lane 89 in the kernel's stable-tree
/// merges, where ninety branches are open at once — every node there was drawn off the side of
/// the column and the graph looked empty.
pub const MAX_LANES: u16 = 32;

/// How many rows a lane may be held open for a parent that has not arrived.
///
/// A merge's second parent can be twenty thousand rows below it in the kernel, and a lane held
/// that long is a line nobody ever sees both ends of — but it costs a whole column for its
/// entire span. Sixty-odd of them are outstanding at once down there, which is more than the
/// column can show, so they crowded every commit into the lane of last resort. Held for eight
/// screenfuls and then given back, the graph stays about as wide as the branches actually in
/// view, which is what makes it readable at all.
const LINE_LIMIT: u64 = 500;

/// The lane a commit is drawn in when every other one is taken.
///
/// Never carries a reservation, and that is the whole point of setting it aside: a run in a
/// lane ends at the row drawn in that lane, so a lane holding both a reservation and an
/// unrelated node would cut the reservation's line short.
const SPILL: u16 = MAX_LANES - 1;

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
    /// Lane -> the commit it is reserved for and when it was claimed, or `None` when free.
    lanes: Vec<Option<(K, u64)>>,
    /// Rows placed so far, which is what "claimed longest ago" is measured in.
    clock: u64,
    /// The earliest row at which any reservation could have gone stale.
    ///
    /// Without it every row scans every lane for one that has expired, which is thirty-one
    /// checks a row and forty-six million over the kernel. This is the row that scan would
    /// first find something at, so until then there is nothing to look for.
    next_sweep: u64,
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
            clock: 0,
            next_sweep: u64::MAX,
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
        self.clock += 1;
        self.retire_stale();
        let open = self.open_mask();

        // A child may already have reserved a lane for me; otherwise I am a branch tip.
        let lane = match self.reserved.remove(key) {
            Some(l) => {
                self.lanes[l as usize] = None;
                l
            }
            None => self.alloc().unwrap_or(SPILL),
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
            let pl = if i == 0 && !kept_my_lane && lane != SPILL {
                kept_my_lane = true;
                Some(lane)
            } else {
                self.alloc()
            };
            // Nowhere to put it. The edge is dropped and the parent becomes a tip when its own
            // row comes: a missing line in a region already too wide to draw is a smaller lie
            // than a line running to the wrong commit.
            let Some(pl) = pl else {
                parent_lanes.push(NO_LANE);
                continue;
            };
            self.lanes[pl as usize] = Some((p.clone(), self.clock));
            self.next_sweep = self.next_sweep.min(self.clock.saturating_add(LINE_LIMIT));
            self.reserved.insert(p.clone(), pl);
            parent_lanes.push(pl);
        }

        // A root, or a first parent that was already reserved elsewhere: my lane dies here.
        //
        // Unless a later parent was allocated into it. Claiming my lane above left its slot
        // free, so `alloc` is entitled to hand it straight back out; freeing it then would
        // destroy a live reservation and leave the map pointing at a trimmed-away slot.
        if !kept_my_lane && !parent_lanes.contains(&lane) && lane != SPILL {
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

    /// Leftmost free lane, skipping any freed on this row and the spill lane.
    ///
    /// When every lane is taken, the one claimed longest ago is taken back. `None` only when
    /// even that is impossible, which is a row that has just claimed every lane itself.
    fn alloc(&mut self) -> Option<u16> {
        for (i, slot) in self.lanes.iter().enumerate() {
            let lane = u16::try_from(i).unwrap_or(u16::MAX);
            if lane >= SPILL {
                break;
            }
            if slot.is_none() && !self.freed_this_row.contains(&lane) {
                return Some(lane);
            }
        }
        if u16::try_from(self.lanes.len()).unwrap_or(u16::MAX) < SPILL {
            self.lanes.push(None);
            return u16::try_from(self.lanes.len() - 1).ok();
        }
        self.evict()
    }

    /// Gives back every lane held longer than a line is worth drawing.
    ///
    /// The lane is closed in this row's `open` mask and not handed out again until the next
    /// row, so the line stops with a gap under it rather than appearing to run on into
    /// whatever is drawn there next.
    fn retire_stale(&mut self) {
        if self.clock < self.next_sweep {
            return;
        }
        let Some(cutoff) = self.clock.checked_sub(LINE_LIMIT) else {
            return;
        };
        let mut oldest = u64::MAX;
        for i in 0..self.lanes.len() {
            let stale = self.lanes[i]
                .as_ref()
                .is_some_and(|(_, at)| *at < cutoff)
                .then(|| self.lanes[i].as_ref().map(|(key, _)| key.clone()))
                .flatten();
            if let Some(key) = stale {
                self.reserved.remove(&key);
                self.lanes[i] = None;
                if let Ok(lane) = u16::try_from(i) {
                    self.freed_this_row.push(lane);
                }
            } else if let Some((_, at)) = &self.lanes[i] {
                oldest = oldest.min(*at);
            }
        }
        self.next_sweep = oldest.saturating_add(LINE_LIMIT);
        self.trim();
    }

    /// Takes back the lane whose reservation has been outstanding longest.
    ///
    /// The kernel keeps more branches open at once than a column can show: the frontier is
    /// sixty-odd lanes deep in its history, so with thirty-one every lane was reserved for a
    /// merge parent tens of thousands of rows below and every commit after row 555 was drawn
    /// in the spill lane — one column of nodes beside thirty lines belonging to nothing on
    /// screen. Recycling the oldest reservation keeps the graph the width it can draw. What is
    /// lost is the line from a merge to a parent so far below that no screen shows both: the
    /// `open` mask closes it at this row, so it stops rather than joining the wrong commit.
    fn evict(&mut self) -> Option<u16> {
        let oldest = self
            .lanes
            .iter()
            .enumerate()
            .take(SPILL as usize)
            .filter_map(|(i, slot)| slot.as_ref().map(|(key, at)| (i, key, *at)))
            .filter(|(i, _, _)| u16::try_from(*i).is_ok_and(|l| !self.freed_this_row.contains(&l)))
            .min_by_key(|(_, _, at)| *at)
            .map(|(i, key, _)| (i, key.clone()));

        let (index, key) = oldest?;
        self.reserved.remove(&key);
        self.lanes[index] = None;
        u16::try_from(index).ok()
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
