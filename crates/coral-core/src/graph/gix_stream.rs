use gix::ObjectId;
use smallvec::SmallVec;

use super::stream::{CommitNode, CommitStream, Order, StreamOpts, WalkControl, WalkStats};
use crate::error::CoralError;

/// Reads the DAG in-process through gix, using the commit-graph file when one is present.
///
/// The commit-graph stores parents as positions within the file rather than as object ids, so
/// the topology walk is a pointer chase over an mmap with no hashing and no zlib. That is what
/// makes the full kernel walk 2.7 s instead of tens of seconds.
pub struct GixCommitStream {
    repo: gix::ThreadSafeRepository,
}

impl GixCommitStream {
    /// Opens `path` for graph reads.
    ///
    /// # Errors
    /// [`CoralError::NotARepository`] if gix cannot open it.
    pub fn open(path: &std::path::Path) -> Result<Self, CoralError> {
        let repo = gix::open(path)
            .map_err(|_| CoralError::NotARepository(path.to_path_buf()))?
            .into_sync();
        Ok(Self { repo })
    }

    /// Every ref that resolves to a commit, plus HEAD.
    ///
    /// Refs are filtered by object kind because the kernel carries tags that peel to trees and
    /// blobs, and handing one to the walk is a hard error rather than a skip.
    fn all_tips(repo: &gix::Repository) -> Result<Vec<ObjectId>, CoralError> {
        let mut tips: Vec<ObjectId> = repo
            .references()
            .map_err(graph_err)?
            .all()
            .map_err(graph_err)?
            .filter_map(Result::ok)
            .filter_map(|mut r| r.peel_to_id().ok().map(gix::Id::detach))
            .filter(|id| {
                repo.find_header(*id)
                    .is_ok_and(|h| h.kind() == gix::object::Kind::Commit)
            })
            .collect();

        if let Ok(head) = repo.head_id() {
            tips.push(head.detach());
        }
        tips.sort_unstable();
        tips.dedup();
        Ok(tips)
    }

    fn walk_topological(
        repo: &gix::Repository,
        tips: Vec<ObjectId>,
        opts: &StreamOpts,
        sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    ) -> Result<WalkStats, CoralError> {
        use gix::traverse::commit::{Parents, topo};

        let commit_graph = repo.commit_graph_if_enabled().ok().flatten();
        let walk = topo::Builder::from_iters(&repo.objects, tips, None::<Vec<ObjectId>>)
            .with_commit_graph(commit_graph)
            .sorting(topo::Sorting::TopoOrder)
            .parents(if opts.first_parent {
                Parents::First
            } else {
                Parents::All
            })
            .build()
            .map_err(graph_err)?;

        let mut stats = WalkStats::default();
        for info in walk {
            let info = info.map_err(graph_err)?;
            if !emit(
                &mut stats,
                opts,
                sink,
                info.id,
                &info.parent_ids,
                info.commit_time.unwrap_or_default(),
            ) {
                break;
            }
        }
        Ok(stats)
    }

    fn walk_by_commit_time(
        repo: &gix::Repository,
        tips: Vec<ObjectId>,
        opts: &StreamOpts,
        sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    ) -> Result<WalkStats, CoralError> {
        let mut platform = repo
            .rev_walk(tips)
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ));
        if opts.first_parent {
            platform = platform.first_parent_only();
        }

        let mut stats = WalkStats::default();
        for info in platform.all().map_err(graph_err)? {
            let info = info.map_err(graph_err)?;
            let time = info.commit_time.unwrap_or_default();
            if !emit(&mut stats, opts, sink, info.id, &info.parent_ids, time) {
                break;
            }
        }
        Ok(stats)
    }
}

/// Feeds one node to the sink. Returns false when the walk should stop.
fn emit(
    stats: &mut WalkStats,
    opts: &StreamOpts,
    sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    id: ObjectId,
    parents: &[ObjectId],
    commit_time: i64,
) -> bool {
    // Both gix and git report every parent even when only the first is traversed; keeping the
    // rest would leave edges pointing at rows the walk never emits.
    let parents: SmallVec<[ObjectId; 2]> = if opts.first_parent {
        SmallVec::from_slice(&parents[..parents.len().min(1)])
    } else {
        SmallVec::from_slice(parents)
    };
    stats.commits += 1;
    stats.edges += parents.len() as u64;
    let node = CommitNode {
        id,
        parents,
        commit_time,
    };
    if sink(node) == WalkControl::Stop {
        return false;
    }
    opts.max_count.is_none_or(|m| stats.commits < m)
}

fn graph_err(e: impl std::fmt::Display) -> CoralError {
    CoralError::Protocol {
        label: "gix-walk",
        detail: e.to_string(),
    }
}

impl CommitStream for GixCommitStream {
    fn name(&self) -> &'static str {
        "gix"
    }

    fn walk(
        &self,
        opts: &StreamOpts,
        sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    ) -> Result<WalkStats, CoralError> {
        let repo = self.repo.to_thread_local();
        let tips = if opts.tips.is_empty() {
            Self::all_tips(&repo)?
        } else {
            opts.tips.clone()
        };
        if tips.is_empty() {
            return Ok(WalkStats::default());
        }
        match opts.order {
            Order::Topological => Self::walk_topological(&repo, tips, opts, sink),
            Order::CommitTime => Self::walk_by_commit_time(&repo, tips, opts, sink),
        }
    }
}
