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
    /// Commands of the user's own, offered in the palette and the menus.
    pub actions: Vec<CustomAction>,
}

/// A command line of the user's own, run in the repository with what it is about filled in.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAction {
    /// Stable across edits and reorders, so the window asks for one by it rather than by the
    /// command line it holds.
    pub id: String,
    pub name: String,
    /// `{repo}`, `{commit}`, `{short}`, `{branch}`, `{file}` and `{path}` are filled in.
    pub command: String,
    pub on: ActionTarget,
}

/// What an action is about, which decides the menus it appears in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionTarget {
    /// The repository as a whole: the palette.
    Repository,
    /// One commit: its menu, and the palette for the selected one.
    Commit,
    /// One file: the file menus.
    File,
}

/// What an action is being run on, as the window knows it.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ActionContext {
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub file: Option<String>,
}

/// How a run went, for the window to say.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRun {
    pub success: bool,
    /// The last of what it printed, both streams; the whole of it is in the activity log.
    pub output: String,
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

// git names files with `/` on every platform; joined whole, that left `C:\repo\src/a.rs` for
// the editor on Windows.
fn on_disk(repo: &Path, file: &str) -> String {
    file.split('/')
        .fold(repo.to_path_buf(), |path, part| path.join(part))
        .display()
        .to_string()
}

/// The editor's argv for one file, with the placeholders filled in. A command that names no
/// file gets the file's path appended, which is what nearly every editor expects.
#[must_use]
pub fn editor_argv(template: &str, repo: &Path, file: &str, line: Option<u32>) -> Vec<String> {
    let path = on_disk(repo, file);
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

/// An action's argv with the context filled in: each placeholder inside one word, so a file
/// name with spaces stays one argument. A placeholder the context has no value for is refused
/// rather than run as an empty argument.
///
/// # Errors
/// A message naming the placeholder that has nothing to fill it.
pub fn action_argv(
    command: &str,
    repo: &Path,
    context: &ActionContext,
) -> Result<Vec<String>, String> {
    let short = context
        .commit
        .as_deref()
        .map(|c| c.chars().take(8).collect::<String>());
    let path = context.file.as_deref().map(|f| on_disk(repo, f));
    let fills: [(&str, Option<&str>); 6] = [
        ("{repo}", Some(&*repo.to_string_lossy())),
        ("{commit}", context.commit.as_deref()),
        ("{short}", short.as_deref()),
        ("{branch}", context.branch.as_deref()),
        ("{file}", context.file.as_deref()),
        ("{path}", path.as_deref()),
    ];
    words(command)
        .into_iter()
        .map(|mut word| {
            for (name, value) in &fills {
                if word.contains(name) {
                    let Some(value) = value else {
                        return Err(format!("{name} has nothing to stand for here"));
                    };
                    word = word.replace(name, value);
                }
            }
            Ok(word)
        })
        .collect()
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

/// Replaces the custom actions, as the Tools pane edits them.
#[must_use]
#[tauri::command]
pub fn tools_set_actions(
    tools: tauri::State<'_, Tools>,
    actions: Vec<CustomAction>,
) -> ToolSettings {
    let mut next = tools.read();
    next.actions = actions
        .into_iter()
        .filter(|a| !a.name.trim().is_empty() && !a.command.trim().is_empty())
        .collect();
    tools.write(next);
    tools.read()
}

/// How much of an action's output comes back to the window.
const OUTPUT_TAIL: usize = 4000;

/// Runs one custom action, by its id, in the repository, and answers when it has finished.
///
/// By id, so the window never sends a command line to run: what runs is what the user saved.
/// Started directly rather than through a shell; an action that wants a pipe says `sh -c`.
///
/// # Errors
/// An unknown id, a placeholder with nothing to fill it, or a program that cannot be started.
/// A program that runs and fails is an answer, not an error.
#[tauri::command]
pub async fn action_run(
    tools: tauri::State<'_, Tools>,
    path: String,
    id: String,
    context: ActionContext,
) -> Result<ActionRun, IpcError> {
    let refused = |detail: String| {
        IpcError::from(coral_core::CoralError::Refused {
            label: "run that action",
            detail,
        })
    };
    let Some(action) = tools.read().actions.into_iter().find(|a| a.id == id) else {
        return Err(refused("it is no longer in Preferences, Tools".to_owned()));
    };
    let (_, loc) = located(&path).await?;
    let root = loc.workdir.clone().unwrap_or_else(|| loc.git_dir.clone());
    let argv = action_argv(&action.command, &root, &context).map_err(refused)?;
    let Some((program, rest)) = argv.split_first() else {
        return Err(refused("its command is empty".to_owned()));
    };
    let logged = crate::activity::started(&path, &action.name);
    let out = tokio::process::Command::new(program)
        .args(rest)
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| refused(format!("{program}: {e}")))?;
    let mut said = String::from_utf8_lossy(&out.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&out.stderr));
    if !said.trim().is_empty() {
        crate::activity::note(
            &path,
            crate::activity::Level::Info,
            said.trim_end().to_owned(),
        );
    }
    if out.status.success() {
        logged.finished();
    } else {
        logged.failed(&format!("exited with {}", out.status));
    }
    let cut = said.len().saturating_sub(OUTPUT_TAIL);
    let tail = said
        .char_indices()
        .find(|(i, _)| *i >= cut)
        .map_or("", |(i, _)| &said[i..]);
    Ok(ActionRun {
        success: out.status.success(),
        output: tail.trim().to_owned(),
    })
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
    use super::{ActionContext, action_argv, editor_argv, words};
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

    // What the platform calls `parts` under `/r`, so these hold on Windows too.
    fn under_r(parts: &[&str]) -> String {
        parts
            .iter()
            .fold(Path::new("/r").to_path_buf(), |p, part| p.join(part))
            .display()
            .to_string()
    }

    #[test]
    fn the_file_is_filled_in_or_added_at_the_end() {
        let repo = Path::new("/r");
        assert_eq!(
            editor_argv("code --goto {path}:{line}", repo, "src/a b.rs", Some(7)),
            [
                "code",
                "--goto",
                &format!("{}:7", under_r(&["src", "a b.rs"]))
            ]
        );
        assert_eq!(
            editor_argv("gedit", repo, "x.txt", None),
            ["gedit", &under_r(&["x.txt"])]
        );
        // A name that looks like an option is still only ever one argument.
        assert_eq!(
            editor_argv("subl", repo, "--help", None),
            ["subl", &under_r(&["--help"])]
        );
    }

    #[test]
    fn an_action_is_filled_in_word_by_word_and_refuses_what_it_cannot_fill() {
        let repo = Path::new("/r");
        let on_commit = ActionContext {
            commit: Some("0123456789abcdef".to_owned()),
            branch: Some("main".to_owned()),
            file: None,
        };
        assert_eq!(
            action_argv("git log -1 {commit} --format=%s", repo, &on_commit).unwrap(),
            ["git", "log", "-1", "0123456789abcdef", "--format=%s"]
        );
        assert_eq!(
            action_argv("echo {short}@{branch} in {repo}", repo, &on_commit).unwrap(),
            ["echo", "01234567@main", "in", "/r"]
        );
        let on_file = ActionContext {
            file: Some("docs/a b.md".to_owned()),
            ..ActionContext::default()
        };
        assert_eq!(
            action_argv("wc -l {path}", repo, &on_file).unwrap(),
            ["wc", "-l", &under_r(&["docs", "a b.md"])]
        );
        let e = action_argv("git show {commit}", repo, &on_file).unwrap_err();
        assert!(e.contains("{commit}"), "{e}");
    }
}
