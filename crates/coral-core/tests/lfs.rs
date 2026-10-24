//! Tracking a file type with LFS, and locking files on a fake LFS server.
//!
//! Skipped where git-lfs or python3 is not installed: both are the machine's, not Coral's.

#![cfg(unix)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use coral_core::lfs::LfsLock;
use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::testutil::TestRepo;

/// `lfs_server.py`, beside this file, stopped when dropped.
struct Server {
    child: Child,
    url: String,
    _dir: tempfile::TempDir,
}

impl Server {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let port_file = dir.path().join("port");
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lfs_server.py");
        let child = Command::new("python3")
            .arg(script)
            .arg(&port_file)
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        let port = loop {
            if let Ok(text) = std::fs::read_to_string(&port_file)
                && !text.is_empty()
            {
                break text;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "the LFS server did not start"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        Self {
            child,
            url: format!("http://127.0.0.1:{port}/"),
            _dir: dir,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn present(program: &str, arg: &str) -> bool {
    Command::new(program)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn lfs_present() -> bool {
    present("git", "lfs") && present("python3", "--version")
}

async fn open(repo: &TestRepo) -> (GitRunner, RepoLocation) {
    let runner = GitRunner::discover().await.unwrap();
    let loc = RepoLocation::discover(&runner, repo.path()).await.unwrap();
    (runner, loc)
}

#[tokio::test]
async fn tracking_a_type_sets_lfs_up_and_stores_the_next_one_as_a_pointer() {
    if !lfs_present() {
        return;
    }
    let repo = TestRepo::new().write("a.txt", "1\n").commit("start");
    let (runner, loc) = open(&repo).await;

    loc.lfs_track(&runner, "*.psd").await.unwrap();
    let attributes = std::fs::read_to_string(repo.path().join(".gitattributes")).unwrap();
    assert!(
        attributes.contains("*.psd filter=lfs diff=lfs merge=lfs -text"),
        "{attributes}"
    );

    // Staged through Coral, as the window stages it: `repo.git` runs with no global
    // configuration, so it would not see a git-lfs set up for the user rather than here.
    let repo = repo.write("art.psd", "a big picture\n");
    loc.stage(&runner, &[".gitattributes", "art.psd"])
        .await
        .unwrap();
    let stored = repo.git(["cat-file", "-p", ":art.psd"]);
    assert!(
        stored.starts_with("version https://git-lfs.github.com/spec/v1"),
        "{stored}"
    );
}

#[tokio::test]
async fn locks_lists_and_unlocks_and_says_when_someone_else_holds_one() {
    if !lfs_present() {
        return;
    }
    let server = Server::start();
    let repo = TestRepo::new().write("a.txt", "1\n").commit("start");
    repo.git(["lfs", "install", "--local"]);
    repo.git(["config", "lfs.url", &server.url]);
    let (runner, loc) = open(&repo).await;

    loc.lfs_lock(&runner, "hero.psd").await.unwrap();
    assert_eq!(
        loc.lfs_locks(&runner).await.unwrap(),
        [LfsLock {
            id: "1".into(),
            path: "hero.psd".into(),
            owner: "Fixture User".into(),
            ours: true
        }]
    );
    let refused = loc.lfs_lock(&runner, "hero.psd").await.unwrap_err();
    assert!(
        refused.to_string().contains("already created lock"),
        "{refused}"
    );

    loc.lfs_unlock(&runner, "hero.psd", false).await.unwrap();
    assert_eq!(loc.lfs_locks(&runner).await.unwrap(), []);

    // Bob locks a file; this repository speaks as Bob through the header the server reads.
    repo.git(["config", "http.extraHeader", "X-Lock-Owner: Bob"]);
    loc.lfs_lock(&runner, "level.psd").await.unwrap();
    repo.git(["config", "--unset", "http.extraHeader"]);
    let held = loc.lfs_locks(&runner).await.unwrap();
    assert_eq!(held[0].owner, "Bob");
    assert!(!held[0].ours, "Bob's lock read as ours");

    let refused = loc
        .lfs_unlock(&runner, "level.psd", false)
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("owned by Bob"), "{refused}");
    loc.lfs_unlock(&runner, "level.psd", true).await.unwrap();
    assert_eq!(loc.lfs_locks(&runner).await.unwrap(), []);
}
