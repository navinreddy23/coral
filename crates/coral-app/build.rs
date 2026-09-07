use std::path::Path;
use std::process::Command;

fn main() {
    tauri_build::build();
    watch_frontend(Path::new("../../ui/dist"));
    stamp_commit();
}

/// Declares the built interface as an input, so rebuilding it rebuilds the binary.
///
/// `generate_context!` reads `ui/dist` while the crate compiles, and nothing tells cargo that.
/// Neither `tauri_build` nor the default package scan covers a directory two levels up, so
/// `npm run build` followed by `cargo build --release` was a no-op that finished in a quarter of
/// a second and left the previous interface embedded — a binary that is a release build of a
/// commit it was never built from. It cost an afternoon: a fix was verified in the window,
/// `just check` rebuilt the interface, and the binary run afterwards was still the broken one.
///
/// Every file, not just the directory. A directory's own timestamp moves when a name is added or
/// removed, and `index.html` keeps its name across every build.
fn watch_frontend(dist: &Path) {
    println!("cargo:rerun-if-changed={}", dist.display());
    let Ok(entries) = std::fs::read_dir(dist) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            watch_frontend(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
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
