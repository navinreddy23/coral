use std::process::Command;

fn main() {
    tauri_build::build();
    stamp_commit();
}

/// Records the commit this binary was built from, for the About pane to show.
///
/// A version number alone does not identify a build. Between two releases there are hundreds of
/// builds carrying the same one, and a bug report that says "0.1.0" narrows nothing down.
///
/// Absent rather than wrong when there is no commit to name — a source tarball, a build from an
/// unpacked archive, a machine without git. The pane says so instead of inventing one.
fn stamp_commit() {
    // Only when the commit could actually have changed. Without this the crate rebuilds on
    // every `cargo build`, since a build script with no declared inputs is always stale.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-env-changed=CORAL_COMMIT");

    if let Ok(from_env) = std::env::var("CORAL_COMMIT") {
        println!("cargo:rustc-env=CORAL_COMMIT={from_env}");
        return;
    }

    let described = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok());

    if let Some(commit) = described {
        let commit = commit.trim();
        if !commit.is_empty() {
            println!("cargo:rustc-env=CORAL_COMMIT={commit}");
        }
    }
}
