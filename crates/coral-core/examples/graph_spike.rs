//! Decides the graph architecture: does an in-process gix topological walk meet the 5 s
//! budget on the kernel, or do we stream `git rev-list --topo-order` instead?
//!
//! Run with: cargo run --release -p coral-core --example graph_spike -- <repo>

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());

    let t0 = std::time::Instant::now();
    let repo = gix::open(&path)?;
    let open_ms = t0.elapsed().as_millis();

    let graph = repo.commit_graph_if_enabled()?;
    println!("open: {open_ms} ms, commit-graph: {}", graph.is_some());

    // `--all` semantics: every ref, peeled, keeping only those that land on a commit. The
    // kernel carries tags that peel to trees and blobs, and feeding one to the walk is a hard
    // error rather than a skip.
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
    println!("tips: {}", tips.len());

    let t1 = std::time::Instant::now();
    let walk = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        tips,
        None::<Vec<gix::ObjectId>>,
    )
    .with_commit_graph(graph)
    .sorting(gix::traverse::commit::topo::Sorting::TopoOrder)
    .parents(gix::traverse::commit::Parents::All)
    .build()?;

    let mut count: u64 = 0;
    let mut edges: u64 = 0;
    let mut first_4096_ms = 0;
    for info in walk {
        let info = info?;
        edges += info.parent_ids.len() as u64;
        count += 1;
        if count == 4096 {
            first_4096_ms = t1.elapsed().as_millis();
        }
    }
    let total_ms = t1.elapsed().as_millis();

    println!("first 4096 rows: {first_4096_ms} ms");
    println!("walk: {count} commits, {edges} edges in {total_ms} ms");
    #[allow(clippy::cast_precision_loss)]
    let rate = count as f64 / (total_ms as f64 / 1000.0);
    println!("rate: {rate:.0} commits/s");
    Ok(())
}
