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
    /// The lane each parent's line runs down from this row, in parent order.
    ///
    /// The first parent's is always the row's own lane, outside the lane of last resort: a
    /// branch runs straight down to the commit it left from. A later parent's may be another
    /// lane, reached across this row — a merge's corner is on the merge.
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
    /// Bit `n` set when lane `n`'s line ends on this row by turning into its node.
    ///
    /// A branch's line, held in its own lane down to the commit it left from: the corner is
    /// on that commit's row, as `GitKraken` draws it, so the branch is seen to start from it.
    /// Always to the right of [`Self::lane`], because a commit takes the leftmost of the lanes
    /// waiting for it, so every branch turns in from the same side.
    pub joins: u32,
}

/// Assigns commits to vertical lanes as the walk emits them.
///
/// The invariant that makes the wire format work: a lane carries one line at a time, and a
/// line is cleared only when the commit it leads to is emitted. So a lane reserved for parent
/// `P` at row `c` is continuously occupied from `c` until `P`'s own row, and on that row it
/// ends — in `P`'s node when `P` is drawn in it, or turning into `P`'s node when it is one of
/// [`RowTopology::joins`]. That is why no list of where runs end has to be sent beyond the
/// join mask.
///
/// Two shapes of edge, the two `GitKraken` draws. A merge's later parent is reached across the
/// merge's own row and then straight down the parent's lane. A first parent drawn elsewhere is
/// reached straight down the child's lane and then across the parent's row. Several lines may
/// wait for one commit — a branch and the line it left, or every branch off one release — and
/// the commit is drawn in the leftmost of them, so the trunk keeps its lane and every branch
/// joins it from the right.
///
/// Requires that children are emitted before their parents, which is what
/// [`super::Order::Topological`] guarantees.
/// Keyed by whatever identifies a commit to the caller: an object id in the engine, a plain
/// index in the tests. A parent's row number is not known when its child is emitted, so the
/// key cannot be the row.
pub struct LaneAssigner<K> {
    /// Lane -> the commit its line leads to and when it was claimed, or `None` when free.
    lanes: Vec<Option<(K, u64)>>,
    /// Rows placed so far, which is what "claimed longest ago" is measured in.
    clock: u64,
    /// The earliest row at which any reservation could have gone stale.
    ///
    /// Without it every row scans every lane for one that has expired, which is thirty-one
    /// checks a row and forty-six million over the kernel. This is the row that scan would
    /// first find something at, so until then there is nothing to look for.
    next_sweep: u64,
    /// Commit -> every lane whose line leads to it. Bounded by the graph's width, not its
    /// length: an entry lives only from the child that claims a lane until the parent that
    /// consumes it.
    reserved: std::collections::HashMap<K, SmallVec<[u16; 2]>>,
    /// Lanes freed on the current row; never reused until the next one, so a lane does not
    /// appear to teleport across a single row.
    ///
    /// A lane that joined this row's node is not among them: a merge here may carry its own
    /// edge on down that lane, which draws as the one line touching the node on its way past —
    /// `GitKraken`'s shape for a branch merged a second time, and a lane narrower.
    freed_this_row: SmallVec<[u16; 4]>,
    /// The commit lane 0 is kept for until it is placed; see [`Self::keep_left`].
    trunk: Option<K>,
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
            trunk: None,
            max_width: 0,
        }
    }

    /// Keeps lane 0 for `key`: no other commit is placed there before it, and it is placed
    /// there whatever lines are waiting for it, which then turn into it from the right.
    ///
    /// For the line the user is on. Left to the order of the walk, lane 0 goes to whichever
    /// tip is newest, and an unmerged branch forked from far down holds it all the way to its
    /// fork, with the checked-out branch beside it for the whole of that span.
    pub fn keep_left(&mut self, key: K) {
        self.trunk = Some(key);
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

        let (lane, joins) = self.place(key);
        let mut parent_lanes = SmallVec::new();
        for (i, p) in parents.iter().enumerate() {
            let first = i == 0 && lane != SPILL;
            let waiting = self.reserved.get(p).filter(|lanes| !lanes.is_empty());
            let pl = match waiting {
                // A first parent runs straight down this lane. When it already has a line, this
                // is a branch leaving it, or one side of a criss-cross: held down to the
                // parent's own row, where it turns in.
                _ if first => Some(lane),
                // A merge into a line already running there: across this row and down it.
                Some(lanes) => {
                    parent_lanes.push(nearest(lanes, lane));
                    continue;
                }
                // To the right of this commit, so the line bends one way. A free lane to the
                // left is closer, but taking it draws a branch that leaves leftwards and
                // comes back, and a screen of those reads as a knot rather than as a trunk
                // with branches off it.
                None => self.alloc_after(lane),
            };
            // Nowhere to put it. The edge is dropped and the parent becomes a tip when its own
            // row comes: a missing line in a region already too wide to draw is a smaller lie
            // than a line running to the wrong commit.
            let Some(pl) = pl else {
                parent_lanes.push(NO_LANE);
                continue;
            };
            self.claim(pl, p);
            parent_lanes.push(pl);
        }
        self.trim();

        // The row is drawn in `lane` even when that lane dies here — a root — and its joins
        // reach across it, so width must cover both, not just the lanes still live after the
        // trim.
        let live = u16::try_from(self.lanes.len()).unwrap_or(u16::MAX);
        let joined = u16::try_from(32 - joins.leading_zeros()).unwrap_or(0);
        let width = live.max(lane.saturating_add(1)).max(joined);
        self.max_width = self.max_width.max(width);
        RowTopology {
            lane,
            parent_lanes,
            width,
            open,
            joins,
        }
    }

    /// The lane `key` is drawn in, and the lanes whose lines turn into it there.
    ///
    /// The leftmost of the lines waiting for it, so the others all join from its right; a
    /// tip, with nothing waiting, takes the leftmost free lane. Every waiting lane is cleared.
    fn place(&mut self, key: &K) -> (u16, u32) {
        let waiting = self.reserved.remove(key).unwrap_or_default();
        let kept = self.trunk.as_ref() == Some(key);
        if kept {
            self.trunk = None;
            if self.lanes.is_empty() {
                self.lanes.push(None);
            }
        }
        let Some(lane) = (if kept {
            Some(0)
        } else {
            waiting.iter().min().copied()
        }) else {
            return (self.alloc().unwrap_or(SPILL), 0);
        };
        let mut joins = 0_u32;
        for &w in &waiting {
            if let Some(slot) = self.lanes.get_mut(w as usize) {
                *slot = None;
            }
            if w != lane && w < 32 {
                joins |= 1 << w;
            }
        }
        (lane, joins)
    }

    /// Starts the line in `lane` that leads to `key`.
    fn claim(&mut self, lane: u16, key: &K) {
        if let Some(slot) = self.lanes.get_mut(lane as usize) {
            *slot = Some((key.clone(), self.clock));
        }
        self.next_sweep = self.next_sweep.min(self.clock.saturating_add(LINE_LIMIT));
        self.reserved.entry(key.clone()).or_default().push(lane);
    }

    /// Ends the line in `lane` that led to `key` without its commit having come.
    fn release(&mut self, key: &K, lane: u16) {
        if let Some(lanes) = self.reserved.get_mut(key) {
            lanes.retain(|l| *l != lane);
            if lanes.is_empty() {
                self.reserved.remove(key);
            }
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

    /// Leftmost free lane, skipping any freed on this row, the spill lane, and lane 0 while it
    /// is kept for the trunk.
    ///
    /// When every lane is taken, the one claimed longest ago is taken back. `None` only when
    /// even that is impossible, which is a row that has just claimed every lane itself.
    fn alloc(&mut self) -> Option<u16> {
        self.alloc_from(0, true)
    }

    /// Leftmost free lane to the right of `lane`, falling back to anywhere.
    ///
    /// Preferred for a parent so that branch lines only ever bend one way. When the right is
    /// full the left is still better than nothing: a line bending the wrong way beats a
    /// commit with no lane at all.
    fn alloc_after(&mut self, lane: u16) -> Option<u16> {
        self.alloc_from(lane.saturating_add(1), false)
            .or_else(|| self.alloc_from(0, true))
    }

    /// Only the leftward pass may take a lane back; the rightward one gives way instead, so a
    /// preference does not cost somebody else their line.
    fn alloc_from(&mut self, first: u16, may_evict: bool) -> Option<u16> {
        let first = if self.trunk.is_some() {
            first.max(1)
        } else {
            first
        };
        for (i, slot) in self.lanes.iter().enumerate().skip(first as usize) {
            let lane = u16::try_from(i).unwrap_or(u16::MAX);
            if lane >= SPILL {
                break;
            }
            if slot.is_none() && !self.freed_this_row.contains(&lane) {
                return Some(lane);
            }
        }
        while u16::try_from(self.lanes.len()).unwrap_or(u16::MAX) < SPILL {
            self.lanes.push(None);
            let lane = u16::try_from(self.lanes.len() - 1).ok()?;
            if lane >= first {
                return Some(lane);
            }
        }
        if may_evict { self.evict() } else { None }
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
            if let (Some(key), Ok(lane)) = (stale, u16::try_from(i)) {
                self.release(&key, lane);
                self.lanes[i] = None;
                self.freed_this_row.push(lane);
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
        let lane = u16::try_from(index).ok()?;
        self.release(&key, lane);
        self.lanes[index] = None;
        Some(lane)
    }

    /// Drops trailing free lanes so `width` reflects the live graph.
    fn trim(&mut self) {
        while self.lanes.last().is_some_and(Option::is_none) {
            self.lanes.pop();
        }
    }
}

/// Of the lanes leading to one commit, the one a merge in `from` reaches across the fewest
/// lanes to join; to the right on a tie, the way a merge's new lane opens.
fn nearest(lanes: &[u16], from: u16) -> u16 {
    lanes
        .iter()
        .copied()
        .min_by_key(|&l| (l.abs_diff(from), l < from))
        .unwrap_or(NO_LANE)
}
