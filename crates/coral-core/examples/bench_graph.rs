//! Times both `CommitStream` implementations against the budgets in docs/ARCHITECTURE.md.
//!
//! `cargo run --release -p coral-core --example bench_graph -- <repo>`

use std::time::Instant;

use coral_core::graph::{
    CommitStream, GixCommitStream, StreamOpts, SubprocessCommitStream, WalkControl,
};
use coral_core::process::GitRunner;

fn time(stream: &dyn CommitStream, label: &str, opts: &StreamOpts) {
    let t = Instant::now();
    let mut commits = 0_u64;
    let mut edges = 0_u64;
    stream
        .walk(opts, &mut |n| {
            commits += 1;
            edges += n.parents.len() as u64;
            WalkControl::Continue
        })
        .expect("walk");
    let ms = t.elapsed().as_millis();
    println!("  {label:<28} {commits:>9} commits {edges:>9} edges  {ms:>6} ms");
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let path = std::path::PathBuf::from(path);

    let runner = GitRunner::discover().await?;
    let gix = GixCommitStream::open(&path)?;
    let sub = SubprocessCommitStream::new(runner, &path);

    println!("first paint (budget 300 ms):");
    time(
        &gix,
        "gix, commit-time, 4096",
        &StreamOpts::first_paint(4096),
    );
    time(
        &sub,
        "rev-list, commit-time, 4096",
        &StreamOpts::first_paint(4096),
    );

    println!("full graph (budget 5000 ms):");
    time(&gix, "gix, topological", &StreamOpts::default());
    time(&sub, "rev-list, topological", &StreamOpts::default());
    Ok(())
}
