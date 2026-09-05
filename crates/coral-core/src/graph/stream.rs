use gix::ObjectId;
use smallvec::SmallVec;

use crate::error::CoralError;

/// One node of the commit DAG, without the strings. Consumed immediately by the row builder
/// and never accumulated, so the 24 bytes of inline parent storage cost nothing at rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitNode {
    pub id: ObjectId,
    pub parents: SmallVec<[ObjectId; 2]>,
    pub commit_time: i64,
}

/// Emission order.
///
/// The distinction is the whole reason first paint is two-phase: [`Order::Topological`] must
/// prepaint indegrees over the entire graph before it can emit one row (1.5 s on the kernel),
/// while [`Order::CommitTime`] emits immediately (6 ms for a screenful) but can place a child
/// before its parent when committer clocks disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Order {
    #[default]
    Topological,
    CommitTime,
}

/// Where a walk starts.
///
/// Ref names rather than object ids, and deliberately so: resolving to an id here would pin the
/// walk to where the branch stood when the user picked it, so a commit made onto a soloed
/// branch would never appear. The names are resolved once per walk instead.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Tips {
    /// Every ref that resolves to a commit, plus HEAD.
    #[default]
    All,
    /// Every one of those but these, by full ref name. HEAD comes too unless it is one of them.
    Except(Vec<String>),
    /// Only these, by full ref name. Empty is an empty graph, and says so rather than quietly
    /// meaning "all" — which is what a bare `Vec` could not distinguish.
    Only(Vec<String>),
}

#[derive(Clone, Debug, Default)]
pub struct StreamOpts {
    pub tips: Tips,
    pub order: Order,
    pub first_parent: bool,
    pub max_count: Option<u64>,
}

impl StreamOpts {
    /// Rows the provisional walk stops at.
    ///
    /// Deliberately not [`super::wire::ROWS_PER_FRAME`], which it once shared: the frame cap
    /// exists to save round trips and the larger it grows the better, while this is on the
    /// 300 ms first-paint budget and every row costs. A few screenfuls is all it has to cover.
    pub const FIRST_PAINT_ROWS: u64 = 4096;

    /// The provisional first screen: fast, all tips, not topologically sound.
    #[must_use]
    pub fn first_paint(rows: u64) -> Self {
        Self {
            order: Order::CommitTime,
            max_count: Some(rows),
            ..Self::default()
        }
    }
}

/// Returned by the sink to stop the walk early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkControl {
    Continue,
    Stop,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WalkStats {
    pub commits: u64,
    pub edges: u64,
}

/// A source of commits in a defined order.
///
/// Deliberately synchronous: the gix implementation is mmap plus inflate, which is CPU-bound,
/// so making this async would box a future per commit to buy nothing. Callers run it on
/// `spawn_blocking`.
pub trait CommitStream: Send + Sync {
    /// Identifies the implementation in benchmarks and error messages.
    fn name(&self) -> &'static str;

    /// Emits commits in `opts.order`. Stopping after *k* emissions must leave a valid prefix
    /// of that order, which is what lets the UI paint before the walk finishes.
    ///
    /// # Errors
    /// Propagates repository read failures.
    fn walk(
        &self,
        opts: &StreamOpts,
        sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    ) -> Result<WalkStats, CoralError>;
}
