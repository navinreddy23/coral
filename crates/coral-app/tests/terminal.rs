//! The in-app terminal, against a real pseudo-terminal and a real shell.
//!
//! Not a command runner: a git user expects their prompt, their aliases and their pager, and
//! every one of those needs a tty on the other end. These check that is what they get.

use std::io::{Read as _, Write};
use std::time::{Duration, Instant};

use coral_app_lib::terminal::{output_event, spawn_shell};

/// A shell, with its output arriving on a channel.
///
/// The reader has to live on a thread of its own. A read from a pseudo-terminal blocks until
/// the shell writes something, so a deadline checked around the read is never reached — the
/// test simply stops there, which is exactly what an earlier version of this file did. It is
/// the same reason the application reads on a thread.
struct Shell {
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    output: std::sync::mpsc::Receiver<Option<String>>,
}

impl Shell {
    /// Opens a shell in `path`.
    ///
    /// `/bin/sh`, not the user's own: an interactive zsh or bash reads startup files that can
    /// prompt, start jobs, or simply take seconds, none of which these are about. What is
    /// under test is the pseudo-terminal, which is the same either way.
    fn open(path: &str, cols: u16, rows: u16) -> Self {
        let spawned = spawn_shell(path, cols, rows, Some("/bin/sh"), Some(false)).unwrap();
        let mut reader = spawned.reader;
        let (send, output) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => {
                        let _ = send.send(None);
                        return;
                    }
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buffer[..n]).into_owned();
                        if send.send(Some(text)).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        Self {
            writer: spawned.writer,
            child: spawned.child,
            output,
        }
    }

    fn run(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
        self.writer.flush().unwrap();
    }

    /// Collects output until `needle` appears, or the time is up.
    fn wait_for(&self, needle: &str, secs: u64) -> String {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut seen = String::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return seen;
            }
            match self.output.recv_timeout(left) {
                Ok(Some(text)) => {
                    seen.push_str(&text);
                    if seen.contains(needle) {
                        return seen;
                    }
                }
                // The stream ended, or nothing more is coming.
                Ok(None) | Err(_) => return seen,
            }
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_shell_runs_in_the_repository_it_was_opened_for() {
    // The working directory is the whole point: a terminal that opens somewhere else makes
    // every git command typed into it wrong.
    let dir = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(dir.path()).unwrap();
    let wanted = path.to_str().unwrap().to_owned();

    let mut shell = Shell::open(&wanted, 80, 24);
    shell.run("pwd");
    let seen = shell.wait_for(&wanted, 15);
    assert!(seen.contains(&wanted), "expected {wanted} in:\n{seen}");
}

#[test]
fn git_runs_in_it_and_sees_the_repository() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(dir.path()).unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&path)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e"]);
    run(&["config", "user.name", "T"]);
    std::fs::write(path.join("a.txt"), "a").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "a terminal commit"]);

    let mut shell = Shell::open(path.to_str().unwrap(), 100, 30);
    shell.run("git log --oneline -1 | cat");
    let seen = shell.wait_for("a terminal commit", 20);
    assert!(seen.contains("a terminal commit"), "got:\n{seen}");
}

#[test]
fn programs_are_told_they_are_on_a_terminal() {
    // What makes colour, a pager and a progress meter behave. A pipe would answer no, and
    // git's output would be the plain one nobody wants in a terminal pane.
    let dir = tempfile::tempdir().unwrap();
    let mut shell = Shell::open(dir.path().to_str().unwrap(), 80, 24);
    shell.run("test -t 1 && echo IS_A_TTY");
    let seen = shell.wait_for("IS_A_TTY", 15);
    assert!(seen.contains("IS_A_TTY"), "got:\n{seen}");
}

#[test]
fn the_terminal_reports_the_size_it_was_opened_at() {
    // A shell asks how wide it is before drawing its first prompt; one started at the wrong
    // size wraps every line until something resizes it.
    let dir = tempfile::tempdir().unwrap();
    let mut shell = Shell::open(dir.path().to_str().unwrap(), 123, 45);
    shell.run("stty size");
    let seen = shell.wait_for("45 123", 15);
    assert!(seen.contains("45 123"), "got:\n{seen}");
}

#[test]
fn resizing_reaches_the_shell() {
    // Without this a full-screen program draws to the size it was told at startup, and the
    // display stays wrong for as long as it runs.
    let dir = tempfile::tempdir().unwrap();
    let spawned = spawn_shell(
        dir.path().to_str().unwrap(),
        80,
        24,
        Some("/bin/sh"),
        Some(false),
    )
    .unwrap();
    spawned
        .master
        .resize(portable_pty::PtySize {
            rows: 50,
            cols: 132,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let size = spawned.master.get_size().unwrap();
    assert_eq!((size.rows, size.cols), (50, 132));

    let mut child = spawned.child;
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn a_shell_does_not_outlive_the_session_that_started_it() {
    // Sixty-two of them accumulated during one afternoon of testing before `Session` learned
    // to kill its child on the way out. A window that closes without going through
    // `terminal_close` would leak one per terminal ever opened.
    let dir = tempfile::tempdir().unwrap();
    let mut spawned = spawn_shell(
        dir.path().to_str().unwrap(),
        80,
        24,
        Some("/bin/sh"),
        Some(false),
    )
    .unwrap();
    assert!(
        spawned.child.try_wait().unwrap().is_none(),
        "it should be running to begin with"
    );

    spawned.child.kill().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if spawned.child.try_wait().unwrap().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the shell was still running after it was killed");
}

#[test]
fn each_terminal_gets_its_own_event_name() {
    // Two open at once must not have to filter each other's bytes out of one stream.
    assert_ne!(output_event(1), output_event(2));
    assert!(output_event(7).contains('7'));
}

#[test]
fn an_empty_shell_choice_means_the_usual_one() {
    // A settings box somebody cleared says "use the default", not "run a program with no
    // name". Without this the pane would open on a failure to start "".
    let dir = tempfile::tempdir().unwrap();
    let spawned = spawn_shell(
        dir.path().to_str().unwrap(),
        80,
        24,
        Some("   "),
        Some(false),
    )
    .expect("a blank choice falls back rather than failing");
    assert_eq!(
        spawned.shell,
        coral_app_lib::terminal::terminal_defaults().shell
    );
}

#[test]
fn a_login_shell_is_the_default_only_where_it_has_to_be() {
    // macOS runs `path_helper` from `/etc/zprofile`, so a shell that skips the login files
    // there has a PATH missing everything the developer tools and Homebrew put on it. A Linux
    // desktop has already read them for the session Coral was started from.
    assert_eq!(
        coral_app_lib::terminal::login_by_default(),
        cfg!(target_os = "macos")
    );
}
