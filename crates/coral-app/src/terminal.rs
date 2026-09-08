//! An in-app terminal, one per repository.
//!
//! A real pseudo-terminal running the user's own shell, not a command runner: a git user
//! expects their aliases, their prompt, their completions and their pager to work, and every
//! one of those needs a tty on the other end. Programs that ask whether they are on a terminal
//! — `git log`, `less`, anything with colour — get the answer they would get from a terminal
//! emulator, because that is what this is.
//!
//! Every command here is `async` without awaiting anything: Tauri requires it of a command
//! that takes `State<'_, _>`, because the borrow has to outlive the call. The pseudo-terminal
//! itself is synchronous, and the one part that blocks — reading the shell's output — runs on
//! a thread of its own.
#![allow(clippy::unused_async)]

use std::collections::HashMap;
use std::io::{Read as _, Write};
use std::sync::Mutex;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem as _};
use tauri::Emitter as _;

use crate::commands::IpcError;

/// One running shell.
struct Session {
    /// Kept so the terminal can be resized after it has started.
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl Drop for Session {
    /// Takes the shell with it.
    ///
    /// Without this a window that closes without going through `terminal_close` leaves its
    /// shells running, and they accumulate: one per terminal ever opened, each holding a
    /// pseudo-terminal. Sixty-two of them piled up during one afternoon of testing before this
    /// was here.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Every terminal the window has open, by id.
#[derive(Default)]
pub struct Terminals(Mutex<HashMap<u32, Session>>);

/// What a newly opened terminal reports.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub id: u32,
    /// The shell that was started, for the pane's title.
    pub shell: String,
}

/// The name of the event carrying output for a terminal.
///
/// Per id, so two open terminals do not have to filter each other's bytes out of one stream.
#[must_use]
pub fn output_event(id: u32) -> String {
    format!("terminal://{id}")
}

/// A shell running on a pseudo-terminal.
pub struct Spawned {
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub reader: Box<dyn std::io::Read + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    /// The shell that was started.
    pub shell: String,
}

/// Which shell to open when the caller names none.
///
/// `$SHELL` is a Unix convention and is unset on Windows, where the fallback was `/bin/sh` —
/// a path that does not exist there, so the pane could not open at all. PowerShell is present
/// on every supported Windows and is far more use to a git user than `cmd`, which is the last
/// resort rather than the first.
fn default_shell() -> String {
    #[cfg(windows)]
    {
        for candidate in ["pwsh.exe", "powershell.exe"] {
            if which(candidate) {
                return candidate.to_owned();
            }
        }
        return std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned());
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
    }
}

/// Whether a shell should be started as a login shell.
///
/// macOS says yes and everywhere else says no, which is what every terminal on those platforms
/// does. It is not a style choice there: `/etc/zprofile` runs `path_helper`, so a shell that
/// skips the login files on macOS has a `PATH` missing everything Homebrew and the developer
/// tools put on it, and half of what the user types is not found.
///
/// Linux desktops have already run the login files for the session Coral was started from, so
/// running them again buys nothing and costs the duplicated `PATH` entries they append.
#[must_use]
pub const fn login_by_default() -> bool {
    cfg!(target_os = "macos")
}

/// What the terminal would use if nobody chose anything, so the settings screen can say so
/// rather than showing an empty box.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
    pub shell: String,
    pub login: bool,
}

/// Whether a program can be found on the PATH.
#[cfg(windows)]
fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Starts the user's shell on a pseudo-terminal, in a directory.
///
/// Separate from the command so it can be tested without a window: everything that decides how
/// the shell behaves — the interactive flag, `TERM`, the working directory — is decided here.
///
/// # Errors
/// [`coral_core::CoralError::Protocol`] when a pseudo-terminal cannot be allocated or the
/// shell cannot be started — a missing `$SHELL`, most often.
pub fn spawn_shell(
    path: &str,
    cols: u16,
    rows: u16,
    shell: Option<&str>,
    login: Option<bool>,
) -> Result<Spawned, coral_core::CoralError> {
    let size = PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = NativePtySystem::default()
        .openpty(size)
        .map_err(|e| protocol(format!("could not open a terminal: {e}")))?;

    // An empty choice is no choice: a settings box somebody cleared means "use the usual one",
    // not "run a program with no name".
    let shell = shell
        .map(str::trim)
        .filter(|chosen| !chosen.is_empty())
        .map_or_else(default_shell, str::to_owned);
    let mut command = CommandBuilder::new(&shell);
    command.cwd(path);
    // An interactive shell, so the user's own prompt, aliases and completions are there.
    // Without it the shell reads none of its startup files and behaves like nobody's terminal.
    //
    // Only a Unix shell takes this flag. `powershell -i` is not a switch it has, and cmd reads
    // it as a command to run, so on Windows the pane would open on an error and exit.
    if cfg!(not(windows)) {
        command.arg("-i");
        // A login shell reads a different set of files first: `.zprofile` and `.zlogin` for
        // zsh, `.bash_profile` for bash. On macOS that is where `PATH` comes from; elsewhere
        // the desktop session has already read them.
        if login.unwrap_or_else(login_by_default) {
            command.arg("-l");
        }
    }
    // Everything git's own output decides from the environment. TERM must name a terminal
    // xterm.js can actually render, and COLORTERM is what makes git use 24-bit colour.
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    // The pager would otherwise wait for a keypress the pane cannot deliver until the user
    // knows it is waiting; git's own default already handles a small output, and this keeps
    // the behaviour identical to a terminal.
    command.env(
        "GIT_PAGER",
        std::env::var("GIT_PAGER").unwrap_or_else(|_| "less -FRX".to_owned()),
    );

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| protocol(format!("could not start {shell}: {e}")))?;
    // The slave must be dropped, or the master never sees end-of-file when the shell exits.
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| protocol(format!("could not read from the terminal: {e}")))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| protocol(format!("could not write to the terminal: {e}")))?;

    Ok(Spawned {
        master: pair.master,
        writer,
        reader,
        child,
        shell,
    })
}

/// Starts a shell in a repository and streams its output to the window.
///
/// # Errors
/// As [`spawn_shell`].
#[tauri::command]
pub async fn terminal_open(
    app: tauri::AppHandle,
    terminals: tauri::State<'_, Terminals>,
    path: String,
    cols: u16,
    rows: u16,
    shell: Option<String>,
    login: Option<bool>,
) -> Result<Opened, IpcError> {
    let Spawned {
        master,
        writer,
        mut reader,
        child,
        shell,
    } = spawn_shell(&path, cols, rows, shell.as_deref(), login)?;

    let id = next_id();
    let event = output_event(id);
    let handle = app.clone();
    // A blocking thread rather than a task: this read blocks until the shell writes, which on
    // an idle terminal is most of the time, and holding an async worker for that starves
    // everything else the runtime has to do.
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    // Lossy, because a read can land in the middle of a multi-byte character
                    // and the alternative is holding bytes back until the next one arrives.
                    let text = String::from_utf8_lossy(&buffer[..n]).into_owned();
                    if handle.emit(&event, text).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = handle.emit(&format!("{event}/closed"), ());
    });

    terminals.0.lock().map_err(poisoned)?.insert(
        id,
        Session {
            master,
            writer,
            child,
        },
    );
    Ok(Opened { id, shell })
}

/// Sends keystrokes to a terminal.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] when the terminal has already gone.
#[tauri::command]
pub async fn terminal_write(
    terminals: tauri::State<'_, Terminals>,
    id: u32,
    data: String,
) -> Result<(), IpcError> {
    let mut open = terminals.0.lock().map_err(poisoned)?;
    let session = open.get_mut(&id).ok_or_else(gone)?;
    session
        .writer
        .write_all(data.as_bytes())
        .and_then(|()| session.writer.flush())
        .map_err(|e| protocol(format!("could not write to the terminal: {e}")))?;
    Ok(())
}

/// Tells the shell the pane changed size.
///
/// Without this a full-screen program draws to the size it was told at startup, and the
/// display is wrong for as long as it runs.
///
/// # Errors
/// [`coral_core::CoralError::Refused`] when the terminal has already gone.
#[tauri::command]
pub async fn terminal_resize(
    terminals: tauri::State<'_, Terminals>,
    id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), IpcError> {
    let open = terminals.0.lock().map_err(poisoned)?;
    let session = open.get(&id).ok_or_else(gone)?;
    session
        .master
        .resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| protocol(format!("could not resize the terminal: {e}")))?;
    Ok(())
}

/// Ends a terminal and its shell.
///
/// # Errors
/// Never fails for an id that has already gone; closing twice is not an error.
#[tauri::command]
pub async fn terminal_close(
    terminals: tauri::State<'_, Terminals>,
    id: u32,
) -> Result<(), IpcError> {
    // Dropping the session kills the shell, which is what makes the reader thread see
    // end-of-file and stop.
    drop(terminals.0.lock().map_err(poisoned)?.remove(&id));
    Ok(())
}

fn next_id() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn protocol(detail: String) -> coral_core::CoralError {
    coral_core::CoralError::Protocol {
        label: "terminal",
        detail,
    }
}

fn gone() -> coral_core::CoralError {
    coral_core::CoralError::Refused {
        label: "terminal",
        detail: "that terminal is no longer open".to_owned(),
    }
}

fn poisoned<T>(_: T) -> coral_core::CoralError {
    protocol("the terminal list was left locked by a panic".to_owned())
}

/// The shell and login setting the terminal would use with nothing chosen.
#[tauri::command]
#[must_use]
pub fn terminal_defaults() -> Defaults {
    Defaults {
        shell: default_shell(),
        login: login_by_default(),
    }
}
