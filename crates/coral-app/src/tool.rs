//! The programs outside Coral that it starts: an editor for a file, and git's own diff and
//! merge tools.
//!
//! The editor is a command line the user writes, kept beside the app's other settings, with
//! `{path}`, `{file}`, `{line}` and `{repo}` filled in. Left empty it is whatever the desktop
//! opens the file with. It is started directly, never through a shell, so a file name is one
//! argument whatever it holds.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use coral_core::tool::{Compared, ExternalTools};

use crate::commands::IpcError;

/// What `tools.json` holds.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolSettings {
    /// The editor's command line; empty for the desktop's own choice.
    pub editor: String,
}

/// The stored settings and where they are kept.
pub struct Tools {
    inner: Mutex<ToolSettings>,
    path: PathBuf,
}

impl Tools {
    /// The settings stored at `path`, or the defaults.
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        let stored = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(stored),
            path,
        }
    }

    #[must_use]
    pub fn read(&self) -> ToolSettings {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn write(&self, next: ToolSettings) {
        let mut held = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *held = next;
        let saved = self
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| Ok(serde_json::to_vec_pretty(&*held)?))
            .and_then(|json| std::fs::write(&self.path, json));
        if let Err(e) = saved {
            tracing::warn!(error = %e, path = %self.path.display(), "could not save the tools");
        }
    }
}

/// Words of a command line, split the way a shell splits them for quotes and backslashes and
/// for nothing else: no variables, no globbing, no pipes.
#[must_use]
pub fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => word.extend(chars.next()),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, '\\') => {
                word.extend(chars.next());
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                started = false;
            }
            (_, c) => word.push(c),
        }
    }
    if started || !word.is_empty() {
        out.push(word);
    }
    out
}

/// The editor's argv for one file, with the placeholders filled in. A command that names no
/// file gets the file's path appended, which is what nearly every editor expects.
#[must_use]
pub fn editor_argv(template: &str, repo: &Path, file: &str, line: Option<u32>) -> Vec<String> {
    let path = repo.join(file).display().to_string();
    let line = line.unwrap_or(1).to_string();
    let mut named = false;
    let mut argv: Vec<String> = words(template)
        .into_iter()
        .map(|w| {
            named |= w.contains("{path}") || w.contains("{file}");
            w.replace("{path}", &path)
                .replace("{file}", file)
                .replace("{line}", &line)
                .replace("{repo}", &repo.display().to_string())
        })
        .collect();
    if !named {
        argv.push(path);
    }
    argv
}

/// What the Tools pane shows.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsView {
    pub settings: ToolSettings,
    /// What git has configured in the repository asked about, or nothing without one.
    pub external: ExternalTools,
}

async fn located(path: &str) -> Result<(GitRunner, RepoLocation), IpcError> {
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, Path::new(path)).await?;
    Ok((runner, loc))
}

/// The stored tools, and the ones git has configured for `path` when one is given.
///
/// # Errors
/// Propagates git failures.
#[tauri::command]
pub async fn tools_read(
    tools: tauri::State<'_, Tools>,
    path: Option<String>,
) -> Result<ToolsView, IpcError> {
    let external = match path.filter(|p| !p.is_empty()) {
        Some(path) => {
            let (runner, loc) = located(&path).await?;
            loc.external_tools(&runner).await?
        }
        None => ExternalTools::default(),
    };
    Ok(ToolsView {
        settings: tools.read(),
        external,
    })
}

/// Stores the editor's command line.
#[must_use]
#[tauri::command]
pub fn tools_set_editor(tools: tauri::State<'_, Tools>, editor: String) -> ToolSettings {
    let mut next = tools.read();
    editor.trim().clone_into(&mut next.editor);
    tools.write(next);
    tools.read()
}

/// Opens one file of a repository in the editor, at a line when one is given.
///
/// # Errors
/// A command that cannot be started, or a desktop that has nothing to open the file with.
#[tauri::command]
pub async fn open_in_editor(
    app: tauri::AppHandle,
    tools: tauri::State<'_, Tools>,
    path: String,
    file: String,
    line: Option<u32>,
) -> Result<(), IpcError> {
    let (_, loc) = located(&path).await?;
    let root = loc.workdir.clone().unwrap_or_else(|| loc.git_dir.clone());
    let template = tools.read().editor;
    if template.is_empty() {
        use tauri_plugin_opener::OpenerExt as _;
        let full = root.join(&file);
        return app
            .opener()
            .open_path(full.display().to_string(), None::<&str>)
            .map_err(|e| {
                IpcError::from(coral_core::CoralError::Refused {
                    label: "open the file",
                    detail: e.to_string(),
                })
            });
    }
    let argv = editor_argv(&template, &root, &file, line);
    let Some((program, rest)) = argv.split_first() else {
        return Ok(());
    };
    let mut child = std::process::Command::new(program)
        .args(rest)
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            IpcError::from(coral_core::CoralError::Refused {
                label: "start the editor",
                detail: format!("{program}: {e}"),
            })
        })?;
    // Reaped beside the window rather than waited on: an editor runs for as long as it runs.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// What to compare in the diff tool, as the window names it.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiffOf {
    Unstaged,
    Staged,
    Commit { rev: String },
}

/// Opens one file's change in git's configured diff tool.
///
/// # Errors
/// Refuses where no diff tool is configured; otherwise a failure to start git.
#[tauri::command]
pub async fn open_difftool(path: String, file: String, of: DiffOf) -> Result<(), IpcError> {
    let (runner, loc) = located(&path).await?;
    let compared = match of {
        DiffOf::Unstaged => Compared::Unstaged,
        DiffOf::Staged => Compared::Staged,
        DiffOf::Commit { rev } => Compared::Commit(rev),
    };
    Ok(loc.open_difftool(&runner, &file, &compared).await?)
}

/// Opens a conflicted file in git's configured merge tool.
///
/// # Errors
/// Refuses where no merge tool is configured; otherwise a failure to start git.
#[tauri::command]
pub async fn open_mergetool(path: String, file: String) -> Result<(), IpcError> {
    let (runner, loc) = located(&path).await?;
    Ok(loc.open_mergetool(&runner, &file).await?)
}

#[cfg(test)]
mod tests {
    use super::{editor_argv, words};
    use std::path::Path;

    #[test]
    fn a_command_line_splits_on_quotes_and_nothing_else() {
        assert_eq!(
            words("code --goto '{path}:{line}'"),
            ["code", "--goto", "{path}:{line}"]
        );
        assert_eq!(
            words(r#"my\ editor "a b" '' $HOME"#),
            ["my editor", "a b", "", "$HOME"]
        );
        assert_eq!(words("  "), Vec::<String>::new());
    }

    #[test]
    fn the_file_is_filled_in_or_added_at_the_end() {
        let repo = Path::new("/r");
        assert_eq!(
            editor_argv("code --goto {path}:{line}", repo, "src/a b.rs", Some(7)),
            ["code", "--goto", "/r/src/a b.rs:7"]
        );
        assert_eq!(
            editor_argv("gedit", repo, "x.txt", None),
            ["gedit", "/r/x.txt"]
        );
        // A name that looks like an option is still only ever one argument.
        assert_eq!(
            editor_argv("subl", repo, "--help", None),
            ["subl", "/r/--help"]
        );
    }
}
