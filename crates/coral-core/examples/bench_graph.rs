//! Times the graph pipeline against the budgets in docs/ARCHITECTURE.md.
//!
//! `cargo run --release -p coral-core --example bench_graph -- <repo>`

use std::time::Instant;

use coral_core::graph::{
    CommitStream, GixCommitStream, StreamOpts, SubprocessCommitStream, WalkControl, build,
};
use coral_core::process::GitRunner;

fn time_walk(stream: &dyn CommitStream, label: &str, opts: &StreamOpts) {
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
    println!("  {label:<30} {commits:>9} commits {edges:>9} edges  {ms:>6} ms");
}

#[allow(
    clippy::cast_precision_loss,
    reason = "benchmark reporting, not a computation"
)]
fn time_build(stream: &dyn CommitStream, label: &str, opts: &StreamOpts) {
    let t = Instant::now();
    let store = build(stream, opts).expect("build");
    let ms = t.elapsed().as_millis();
    let mb = store.bytes_resident() as f64 / (1024.0 * 1024.0);
    let per_row = if store.is_empty() {
        0.0
    } else {
        store.bytes_resident() as f64 / f64::from(store.len())
    };
    println!(
        "  {label:<30} {:>9} rows  lanes {:<4} {ms:>6} ms  {mb:>7.1} MB  {per_row:.1} B/row",
        store.len(),
        store.max_lane() + 1,
    );
}

/// Resident set from /proc, to show how much of the build peak was transient walk state.
fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|p| p.parse::<u64>().ok())
        })
        .map_or(0, |pages| pages * 4 / 1024)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let gix = GixCommitStream::open(&path)?;

    // `build-only` isolates the peak RSS of one full graph build, which is the number the
    // memory budget is really about; running every case in one process conflates them.
    if std::env::args().any(|a| a == "build-only") {
        time_build(&gix, "gix, full graph", &StreamOpts::default());
        println!("  rss after the walk state is dropped: {} MB", rss_mb());
        return Ok(());
    }

    let runner = GitRunner::discover().await?;
    let sub = SubprocessCommitStream::new(runner, &path);

    println!("walk, first paint (budget 300 ms):");
    time_walk(
        &gix,
        "gix, commit-time, 4096",
        &StreamOpts::first_paint(4096),
    );
    time_walk(
        &sub,
        "rev-list, commit-time, 4096",
        &StreamOpts::first_paint(4096),
    );

    println!("walk, full graph (budget 5000 ms):");
    time_walk(&gix, "gix, topological", &StreamOpts::default());
    time_walk(&sub, "rev-list, topological", &StreamOpts::default());

    println!("build into the row store (budget 5000 ms, < 600 MB):");
    time_build(&gix, "gix, first paint", &StreamOpts::first_paint(4096));
    time_build(&gix, "gix, full graph", &StreamOpts::default());
    Ok(())
}
