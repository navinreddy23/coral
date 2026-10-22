//! A topological walk that can stop early for the cost of what it emitted.
//!
//! git's incremental topological walk, and gix's port of it, count every commit's children
//! down to the generation of the *lowest* tip before emitting anything. On the kernel that is
//! the whole history: its oldest tags sit a few commits above the root, so the first row costs
//! as much as all 1.5 million (`count_indegree_walked` equals the commit count in
//! `GIT_TRACE2_PERF`). That is why the window used to paint a commit-time walk first and
//! re-order every row once the real one landed.
//!
//! Here the bound is per commit instead. A commit's children all have a higher generation than
//! it does, so once discovery has gone below a commit's generation its count of children is
//! final, and it can be emitted as soon as they have been. The first screenful then costs only
//! the generations above it, and a walk stopped after *k* rows is exactly the first *k* rows of
//! the full one, so the first paint is already in the final order.
//! <https://git-scm.com/docs/commit-graph#_generation_numbers>

use std::collections::{BinaryHeap, HashMap};

use gix::ObjectId;
use gix::commitgraph::{Graph, Position};
use smallvec::SmallVec;

use crate::error::CoralError;

/// The generation given to commits newer than the commit-graph file. Their children are newer
/// still, so ordering them above everything in the file keeps the rule that a child's
/// generation is higher than its parent's.
const ABOVE_GRAPH: u32 = u32::MAX;

const DISCOVERED: u8 = 1;
const EXPLORED: u8 = 2;
const EMITTED: u8 = 4;

/// A commit the commit-graph does not have, read from the object database instead.
struct Loose {
    id: ObjectId,
    parents: SmallVec<[ObjectId; 2]>,
    time: i64,
}

/// What [`walk`] hands each commit to: its id, every parent, and its commit time. Returns false
/// to stop the walk.
pub type Visit<'v> = dyn FnMut(ObjectId, &[ObjectId], i64) -> bool + 'v;

/// Emits every commit reachable from `tips` with no parent before any of its children.
///
/// Between branches, a merge's second parent's line comes before its first parent's, and a
/// line is followed down until it reaches a commit that something else still leads to — the
/// order `git log --topo-order` draws.
///
/// # Errors
/// [`CoralError::Protocol`] for a commit-graph or a commit that cannot be read.
pub fn walk(
    repo: &gix::Repository,
    graph: &Graph,
    tips: &[ObjectId],
    first_parent: bool,
    visit: &mut Visit<'_>,
) -> Result<(), CoralError> {
    let mut w = Walk::new(repo, graph, first_parent);
    let mut starts = Vec::with_capacity(tips.len());
    for tip in tips {
        let node = w.node_of(*tip)?;
        w.discover(node);
        starts.push((w.time(node), node));
    }
    // Newest last, so the newest is the first one taken off the stack.
    starts.sort_unstable();
    w.stack = starts.into_iter().map(|(_, node)| node).collect();
    w.run(visit)
}

/// How many of the commits reachable from `tips` are not in `hidden`: the row count a full
/// [`walk`] of them will have, for a fraction of its cost, so the first screen can already
/// say how long the history is and scroll as far as it goes.
///
/// # Errors
/// [`CoralError::Protocol`] for a commit-graph or a commit that cannot be read.
pub fn count<S: std::hash::BuildHasher>(
    repo: &gix::Repository,
    graph: &Graph,
    tips: &[ObjectId],
    first_parent: bool,
    hidden: &std::collections::HashSet<ObjectId, S>,
) -> Result<u64, CoralError> {
    let mut w = Walk::new(repo, graph, first_parent);
    let mut todo = Vec::with_capacity(tips.len());
    for tip in tips {
        let node = w.node_of(*tip)?;
        if w.state[node as usize] & DISCOVERED == 0 {
            w.state[node as usize] |= DISCOVERED;
            todo.push(node);
        }
    }
    let mut reached = 0_u64;
    while let Some(node) = todo.pop() {
        reached += 1;
        for p in w.walked_parents(node)? {
            if w.state[p as usize] & DISCOVERED == 0 {
                w.state[p as usize] |= DISCOVERED;
                todo.push(p);
            }
        }
    }
    let mut skipped = 0_u64;
    for id in hidden {
        let node = match graph.lookup(id) {
            Some(Position(pos)) => Some(pos),
            None => w.loose_nodes.get(id).copied(),
        };
        if node.is_some_and(|n| w.state[n as usize] & DISCOVERED != 0) {
            skipped += 1;
        }
    }
    Ok(reached - skipped)
}

struct Walk<'a> {
    repo: &'a gix::Repository,
    graph: &'a Graph,
    first_parent: bool,
    /// Indexed by node: commit-graph positions first, then the loose commits in the order met.
    state: Vec<u8>,
    /// Children discovered and not yet emitted.
    pending: Vec<u32>,
    loose: Vec<Loose>,
    loose_nodes: HashMap<ObjectId, u32>,
    /// Discovered and not yet explored, highest generation first.
    frontier: BinaryHeap<(u32, u32)>,
    /// Ready to emit, as far as was known when each was pushed.
    stack: Vec<u32>,
}

impl<'a> Walk<'a> {
    fn new(repo: &'a gix::Repository, graph: &'a Graph, first_parent: bool) -> Self {
        let n = graph.num_commits() as usize;
        Self {
            repo,
            graph,
            first_parent,
            state: vec![0; n],
            pending: vec![0; n],
            loose: Vec::new(),
            loose_nodes: HashMap::new(),
            frontier: BinaryHeap::new(),
            stack: Vec::new(),
        }
    }

    fn run(&mut self, visit: &mut Visit<'_>) -> Result<(), CoralError> {
        let mut parents: SmallVec<[ObjectId; 2]> = SmallVec::new();
        while let Some(node) = self.stack.pop() {
            if self.state[node as usize] & EMITTED != 0 {
                continue;
            }
            self.settle(self.generation(node))?;
            // A child turned up below it after it was pushed; that child will push it again.
            if self.pending[node as usize] > 0 {
                continue;
            }
            if self.state[node as usize] & EXPLORED == 0 {
                self.explore(node)?;
            }
            self.state[node as usize] |= EMITTED;

            parents.clear();
            self.all_parents(node, &mut parents)?;
            if !visit(self.id(node), &parents, self.time(node)) {
                return Ok(());
            }
            // Pushed in order, so the last parent is taken first: a merge shows the line it
            // brought in before the line it was made on.
            for p in self.walked_parents(node)? {
                self.pending[p as usize] -= 1;
                if self.pending[p as usize] == 0 {
                    self.stack.push(p);
                }
            }
        }
        Ok(())
    }

    /// Explores everything above `generation`, which makes the child count of every commit at
    /// that generation final.
    fn settle(&mut self, generation: u32) -> Result<(), CoralError> {
        while let Some(&(top, node)) = self.frontier.peek() {
            if top <= generation && top != ABOVE_GRAPH {
                break;
            }
            self.frontier.pop();
            if self.state[node as usize] & EXPLORED == 0 {
                self.explore(node)?;
            }
        }
        Ok(())
    }

    /// Counts `node` as a child of each parent it walks to, discovering the ones not yet met.
    fn explore(&mut self, node: u32) -> Result<(), CoralError> {
        self.state[node as usize] |= EXPLORED;
        for p in self.walked_parents(node)? {
            self.pending[p as usize] += 1;
            self.discover(p);
        }
        Ok(())
    }

    fn discover(&mut self, node: u32) {
        if self.state[node as usize] & DISCOVERED == 0 {
            self.state[node as usize] |= DISCOVERED;
            self.frontier.push((self.generation(node), node));
        }
    }

    fn node_of(&mut self, id: ObjectId) -> Result<u32, CoralError> {
        if let Some(Position(pos)) = self.graph.lookup(id) {
            return Ok(pos);
        }
        if let Some(node) = self.loose_nodes.get(&id) {
            return Ok(*node);
        }
        let commit = self.repo.find_commit(id).map_err(walk_err)?;
        let time = commit.time().map_err(walk_err)?.seconds;
        let parents = commit.parent_ids().map(gix::Id::detach).collect();
        let node = u32::try_from(self.state.len()).map_err(walk_err)?;
        self.loose.push(Loose { id, parents, time });
        self.loose_nodes.insert(id, node);
        self.state.push(0);
        self.pending.push(0);
        Ok(node)
    }

    fn in_graph(&self, node: u32) -> bool {
        node < self.graph.num_commits()
    }

    fn generation(&self, node: u32) -> u32 {
        if self.in_graph(node) {
            self.graph.commit_at(Position(node)).generation()
        } else {
            ABOVE_GRAPH
        }
    }

    fn time(&self, node: u32) -> i64 {
        if self.in_graph(node) {
            let seconds = self.graph.commit_at(Position(node)).committer_timestamp();
            i64::try_from(seconds).unwrap_or(i64::MAX)
        } else {
            self.loose_of(node).time
        }
    }

    fn id(&self, node: u32) -> ObjectId {
        if self.in_graph(node) {
            self.graph.id_at(Position(node)).to_owned()
        } else {
            self.loose_of(node).id
        }
    }

    /// The parents the walk goes on to: all of them, or the first alone.
    fn walked_parents(&mut self, node: u32) -> Result<SmallVec<[u32; 2]>, CoralError> {
        let mut out = SmallVec::new();
        if self.in_graph(node) {
            for parent in self.graph.commit_at(Position(node)).iter_parents() {
                out.push(parent.map_err(walk_err)?.0);
                if self.first_parent {
                    break;
                }
            }
            return Ok(out);
        }
        let ids = self.loose_of(node).parents.clone();
        for id in ids
            .iter()
            .take(if self.first_parent { 1 } else { ids.len() })
        {
            out.push(self.node_of(*id)?);
        }
        Ok(out)
    }

    fn all_parents(&self, node: u32, out: &mut SmallVec<[ObjectId; 2]>) -> Result<(), CoralError> {
        if !self.in_graph(node) {
            out.extend_from_slice(&self.loose_of(node).parents);
            return Ok(());
        }
        for parent in self.graph.commit_at(Position(node)).iter_parents() {
            out.push(self.graph.id_at(parent.map_err(walk_err)?).to_owned());
        }
        Ok(())
    }

    fn loose_of(&self, node: u32) -> &Loose {
        &self.loose[(node - self.graph.num_commits()) as usize]
    }
}

fn walk_err(e: impl std::fmt::Display) -> CoralError {
    CoralError::Protocol {
        label: "topo-walk",
        detail: e.to_string(),
    }
}
