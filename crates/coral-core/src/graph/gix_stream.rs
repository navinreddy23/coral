use std::collections::HashSet;

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

    /// The commits git keeps for a stash that are not the stash itself.
    ///
    /// `git stash` writes three commits: the stash, a commit holding the index, and one
    /// holding the untracked files. Only the first is a thing the user did; the other two are
    /// its second and third parents, and drawing them put three rows and two extra lanes in
    /// the graph for every stash, captioned "index on main" and "untracked files on main".
    ///
    /// Returned as a pair: the stashes themselves, whose extra parents are dropped, and the
    /// bookkeeping commits, which are not drawn at all.
    fn stash_plumbing(repo: &gix::Repository) -> (HashSet<ObjectId>, HashSet<ObjectId>) {
        let mut stashes = HashSet::new();
        let mut hidden = HashSet::new();
        let Ok(Some(reference)) = repo.try_find_reference("refs/stash") else {
            return (stashes, hidden);
        };
        // Every stash, not only the newest: the reflog is where the older ones live, and a
        // repository can be left with a ref pointing at one.
        let mut ids: Vec<ObjectId> = reference
            .log_iter()
            .all()
            .ok()
            .flatten()
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|line| ObjectId::from_hex(line.new_oid).ok())
                    .collect()
            })
            .unwrap_or_default();
        if let Ok(id) = reference.clone().into_fully_peeled_id() {
            ids.push(id.detach());
        }

        for id in ids {
            let Ok(commit) = repo.find_commit(id) else {
                continue;
            };
            let extra: Vec<ObjectId> = commit.parent_ids().skip(1).map(gix::Id::detach).collect();
            if extra.is_empty() {
                continue;
            }
            stashes.insert(id);
            hidden.extend(extra);
        }
        (stashes, hidden)
    }

    /// Every ref that resolves to a commit, plus HEAD.
    ///
    /// Refs are filtered by object kind because the kernel carries tags that peel to trees and
    /// blobs, and handing one to the walk is a hard error rather than a skip.
    ///
    /// A ref whose object cannot be read at all is a different matter, and is refused rather
    /// than skipped: dropping it silently produces a graph that is missing whole branches with
    /// nothing anywhere to say so, and the only symptom is a commit count that quietly
    /// disagrees with git's. That is exactly how it presents, and it took a screenshot from
    /// another client beside it to notice.
    fn all_tips(repo: &gix::Repository) -> Result<Vec<ObjectId>, CoralError> {
        let mut tips: Vec<ObjectId> = Vec::new();
        let mut unreadable: Vec<String> = Vec::new();

        for reference in repo
            .references()
            .map_err(graph_err)?
            .all()
            .map_err(graph_err)?
        {
            let Ok(mut reference) = reference else {
                continue;
            };
            let name = reference.name().as_bstr().to_string();
            let Ok(id) = reference.peel_to_id() else {
                unreadable.push(name);
                continue;
            };
            let id = id.detach();
            match repo.find_header(id) {
                // A tag that peels to a tree or a blob is not a mistake; the kernel has them.
                Ok(header) if header.kind() != gix::object::Kind::Commit => {}
                Ok(_) => tips.push(id),
                Err(_) => unreadable.push(name),
            }
        }

        if !unreadable.is_empty() {
            unreadable.sort_unstable();
            return Err(CoralError::Protocol {
                label: "graph",
                detail: format!(
                    "{} ref(s) point at objects this repository cannot read, so the graph \
                     would be missing whole branches: {}",
                    unreadable.len(),
                    unreadable.join(", ")
                ),
            });
        }

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
        plumbing: &(HashSet<ObjectId>, HashSet<ObjectId>),
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
                plumbing,
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
        plumbing: &(HashSet<ObjectId>, HashSet<ObjectId>),
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
            if !emit(
                &mut stats,
                opts,
                plumbing,
                sink,
                info.id,
                &info.parent_ids,
                time,
            ) {
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
    plumbing: &(HashSet<ObjectId>, HashSet<ObjectId>),
    sink: &mut dyn FnMut(CommitNode) -> WalkControl,
    id: ObjectId,
    parents: &[ObjectId],
    commit_time: i64,
) -> bool {
    let (stashes, hidden) = plumbing;
    // git's own bookkeeping, not anything the user did. Skipped rather than drawn, and the
    // walk goes on: their own parent is the commit the stash was taken from, which is
    // reachable anyway.
    if hidden.contains(&id) {
        return opts.max_count.is_none_or(|m| stats.commits < m);
    }
    // Both gix and git report every parent even when only the first is traversed; keeping the
    // rest would leave edges pointing at rows the walk never emits.
    let first_only = opts.first_parent || stashes.contains(&id);
    let parents: SmallVec<[ObjectId; 2]> = if first_only {
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
        let plumbing = Self::stash_plumbing(&repo);
        match opts.order {
            Order::Topological => Self::walk_topological(&repo, tips, opts, &plumbing, sink),
            Order::CommitTime => Self::walk_by_commit_time(&repo, tips, opts, &plumbing, sink),
        }
    }
}
