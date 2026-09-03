//! Measures the candidates for the 300 ms open-to-first-paint budget, which the full
//! topological walk cannot meet: topo order must prepaint indegrees before it can emit a
//! single row.
//!
//! Run with: cargo run --release -p coral-core --example first_paint_spike -- <repo>

use std::time::Instant;

const FIRST_SCREEN: usize = 4096;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let repo = gix::open(&path)?;
    let head = repo.head_id()?.detach();

    // Candidate A: first-parent from HEAD. This is the mainline the user sees on open.
    let t = Instant::now();
    let walk = repo
        .rev_walk([head])
        .first_parent_only()
        .sorting(gix::revision::walk::Sorting::BreadthFirst)
        .all()?;
    let n = walk.take(FIRST_SCREEN).count();
    println!(
        "first-parent from HEAD, {n} rows: {} ms",
        t.elapsed().as_millis()
    );

    // Candidate B: commit-time order across all tips, which emits without a prepaint pass.
    let tips: Vec<gix::ObjectId> = repo
        .references()?
        .all()?
        .filter_map(Result::ok)
        .filter_map(|mut r| r.peel_to_id_in_place().ok().map(|id| id.detach()))
        .filter(|id| {
            repo.find_header(*id)
                .is_ok_and(|h| h.kind() == gix::object::Kind::Commit)
        })
        .collect();

    let t = Instant::now();
    let walk = repo
        .rev_walk(tips.clone())
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            Default::default(),
        ))
        .all()?;
    let n = walk.take(FIRST_SCREEN).count();
    println!(
        "commit-time, all tips, {n} rows:  {} ms",
        t.elapsed().as_millis()
    );

    // Candidate C: the full topological walk, for comparison at the same row count.
    let graph = repo.commit_graph_if_enabled()?;
    let t = Instant::now();
    let walk = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        tips,
        None::<Vec<gix::ObjectId>>,
    )
    .with_commit_graph(graph)
    .sorting(gix::traverse::commit::topo::Sorting::TopoOrder)
    .parents(gix::traverse::commit::Parents::All)
    .build()?;
    let n = walk.take(FIRST_SCREEN).count();
    println!(
        "topo, all tips, {n} rows:         {} ms",
        t.elapsed().as_millis()
    );
    Ok(())
}
