// The desktop build must not open a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use coral_app_lib::{actions, commands, conflicts, graph, hosting, tabs};

fn main() {
    // git invokes the running binary as its sequence editor during an interactive rebase, so
    // this has to be answered before anything else starts: no window, no logging, no Tauri.
    // Handled here rather than by shelling out to the CLI, which a packaged application has no
    // reason to assume is installed.
    if let Some(code) = rebase_editor() {
        std::process::exit(code);
    }
    if let Some(code) = credential_helper() {
        std::process::exit(code);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CORAL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    tracing::info!("coral-app starting");

    // Makes git ask this binary for credentials rather than a terminal the user cannot see.
    // Without a nonce the helper stays off: a guessable one would let any local process ask
    // for the user's tokens, which is worse than not having a helper at all.
    if let (Ok(binary), Some(session)) = (
        std::env::current_exe(),
        coral_core::credential::new_session(),
    ) {
        coral_core::credential::configure(binary, session);
    } else {
        tracing::warn!("no credential helper; git will use whatever the user configured");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(graph::GraphCache::default())
        .setup(|app| {
            use tauri::Manager as _;
            // Beside the app's own config, so it travels with the installation rather than
            // with any one repository.
            let dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            app.manage(tabs::Tabs::load(dir.join("session.json")));

            #[cfg(target_os = "linux")]
            if let Some(window) = app.get_webview_window("main") {
                sharpen_text(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::initial_repo,
            commands::open_repo,
            commands::repo_status,
            commands::stage_paths,
            commands::commit_staged,
            graph::graph_frame,
            graph::row_metadata,
            graph::repo_refs,
            graph::repo_submodules,
            graph::commit_detail,
            graph::file_diff,
            actions::repo_action,
            actions::rebase_todo,
            actions::rebase_start,
            conflicts::repo_operation,
            conflicts::repo_conflicts,
            conflicts::conflict_blocks,
            conflicts::resolve_conflict,
            conflicts::operation_step,
            hosting::hosting_status,
            hosting::hosting_login,
            hosting::hosting_logout,
            hosting::hosting_pull_requests,
            hosting::hosting_create,
            tabs::session_get,
            tabs::tab_open,
            tabs::tab_close,
            tabs::tab_activate,
            tabs::tab_group,
            tabs::tab_ungroup,
            tabs::group_collapse,
            graph::binary_self_test
        ])
        .run(tauri::generate_context!())
        .expect("tauri failed to start");
}

/// Answers git's credential protocol, when invoked as its helper.
///
/// Like the sequence editor, git runs the binary that spawned it, so this is handled here
/// rather than by shelling out to the CLI a packaged application cannot assume is installed.
/// Its stdout is the protocol itself and must carry nothing else.
fn credential_helper() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("credential-helper") {
        return None;
    }
    // git appends the action after whatever `credential.helper` was configured with, so the
    // invocation reads `credential-helper --session <nonce> get`.
    let session = args
        .iter()
        .position(|a| a == "--session")
        .and_then(|at| args.get(at + 1))
        .map(String::as_str);
    let action = args
        .iter()
        .skip(2)
        .find(|a| !a.starts_with("--") && Some(a.as_str()) != session)
        .map_or("get", String::as_str);

    match coral_app_lib::credentials::serve(action, session) {
        Ok(response) => {
            print!("{response}");
            Some(0)
        }
        Err(e) => {
            eprintln!("coral credential-helper: {e}");
            Some(1)
        }
    }
}

/// Installs a prepared rebase todo list, when invoked as git's sequence editor.
///
/// Returns the exit code to use, or `None` when this is an ordinary launch. Writes nothing to
/// stdout: git reads the todo file back, so anything printed would be read as an instruction.
fn rebase_editor() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("rebase-editor") {
        return None;
    }
    let todo = args
        .iter()
        .position(|a| a == "--todo")
        .and_then(|at| args.get(at + 1));
    let target = args.last().filter(|a| *a != "rebase-editor");

    let (Some(todo), Some(target)) = (todo, target) else {
        eprintln!("coral rebase-editor: expected --todo <prepared> <file>");
        return Some(2);
    };
    match std::fs::copy(todo, target) {
        Ok(_) => Some(0),
        Err(e) => {
            eprintln!("coral rebase-editor: {e}");
            Some(1)
        }
    }
}

/// Turns off GPU compositing so text keeps subpixel antialiasing.
///
/// `WebKitGTK` renders text with grayscale antialiasing on any composited layer, and in
/// accelerated mode that is the whole page. The desktop here asks for `rgba` antialiasing with
/// slight hinting, which every other application honours, so Coral's text alone came out
/// visibly softer — the effect people describe as blurry, and the reason it is much harder to
/// see against a dark theme.
///
/// The graph canvas is a couple of hundred pixels wide and repaints on a frame callback, so
/// software rasterisation costs nothing measurable here. `CORAL_GPU=1` puts acceleration back
/// for anyone whose machine disagrees.
#[cfg(target_os = "linux")]
fn sharpen_text(window: &tauri::WebviewWindow) {
    if std::env::var_os("CORAL_GPU").is_some() {
        return;
    }
    let applied = window.with_webview(|webview| {
        use webkit2gtk::{SettingsExt as _, WebViewExt};
        if let Some(settings) = WebViewExt::settings(&webview.inner()) {
            settings
                .set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Never);
        }
    });
    if let Err(e) = applied {
        tracing::warn!(error = %e, "could not reach the webview; text stays grayscale-antialiased");
    }
}
