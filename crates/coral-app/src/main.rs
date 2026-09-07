// The desktop build must not open a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use coral_app_lib::{
    actions, activity, commands, conflicts, experimental, graph, hosting, profile, recent, remotes,
    scope, signing, ssh, tabs, terminal, transfer, version, watcher,
};

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

    // Two destinations for one set of events: the terminal, for whoever started the binary
    // from one, and the in-memory activity log the window shows.
    //
    // Both layers carry their own filter, and the log's is not optional. A layer with no
    // filter tells the registry it is interested in everything, which turns on every `trace!`
    // in every dependency for the whole process — gix walking a million objects, rustls on
    // every byte — and the window comes up and never paints.
    {
        use tracing_subscriber::Layer as _;
        use tracing_subscriber::layer::SubscriberExt as _;
        use tracing_subscriber::util::SubscriberInitExt as _;

        let terminal = tracing_subscriber::EnvFilter::try_from_env("CORAL_LOG")
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
        // Coral's own crates at info, everyone else's warnings only: the log answers "what has
        // this application been doing", and a dependency's debug output is not that.
        let mine = tracing_subscriber::filter::Targets::new()
            .with_target("coral_app_lib", tracing::Level::INFO)
            .with_target("coral_core", tracing::Level::INFO)
            .with_target("coral_hosting", tracing::Level::INFO)
            .with_default(tracing::Level::WARN);

        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().with_filter(terminal))
            .with(activity::Capture.with_filter(mine))
            .init();
    }

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

    window();
}

/// Reads what is kept between launches: the tabs, the recents, the settings.
///
/// Separate from the builder because the command list below it is long and growing, and
/// clippy's line count is a fair warning that the two are different jobs.
fn load_state(app: &tauri::App) {
    use tauri::Manager as _;

    // Beside the app's own config, so it travels with the installation rather than with any
    // one repository.
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    // Before the two it decides the location of. The workspace belongs to a profile; the graph
    // scopes below do not, because a hidden branch is a fact about the repository rather than
    // about whoever is looking at it.
    let profiles = profile::Profiles::load(dir.clone());
    let current = profiles.read().current;
    app.manage(tabs::Tabs::load(profiles.session_path(&current)));
    app.manage(recent::Recents::load(profiles.recent_path(&current)));
    app.manage(profiles);
    app.manage(scope::Scopes::load(dir.join("scope.json")));

    // Before anything can run git, since this is what decides which git that is.
    let settings = experimental::Experimental::load(dir.join("settings.json"));
    settings.apply();
    app.manage(settings);
}

/// Builds and runs the window.
///
/// Split from `main` because the command list is long and growing: the startup work above it —
/// the sequence editor, the credential helper, logging — has nothing to do with the window and
/// reads better on its own.
/// Every command the window may call.
///
/// Its own function because the list is the long part of the builder and grows with every
/// feature; leaving it inline put `window` over the line count for a reason that says
/// nothing about the window.
// A list of names, one per line. Splitting it in two to satisfy a line count would make a
// command harder to find, which is the only thing this function is read for.
#[allow(clippy::too_many_lines)]
fn handlers() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::initial_repo,
        commands::open_repo,
        commands::repo_status,
        commands::stage_paths,
        commands::commit_staged,
        commands::discard_paths,
        commands::delete_paths,
        commands::repo_init,
        commands::repo_clone,
        commands::lfs_available,
        recent::recent_repos,
        recent::forget_recent,
        recent::forget_all_recents,
        graph::graph_frame,
        graph::row_metadata,
        graph::repo_refs,
        graph::repo_stashes,
        graph::graph_row_of,
        graph::graph_rewalk,
        scope::graph_scope,
        scope::set_graph_scope,
        profile::profile_list,
        profile::profile_create,
        profile::profile_rename,
        profile::profile_recolour,
        profile::profile_set_settings,
        profile::profile_switch,
        profile::profile_delete,
        profile::profile_apply_here,
        profile::repo_identity,
        activity::activity_log,
        activity::activity_clear,
        experimental::experimental_git,
        experimental::experimental_set_git,
        graph::repo_submodules,
        graph::commit_detail,
        graph::patch_range_size,
        graph::file_diff,
        graph::worktree_diff,
        graph::file_blame,
        graph::file_history,
        graph::file_text,
        graph::search_commits,
        graph::compare_commits,
        graph::compare_file_diff,
        graph::commit_tree,
        graph::apply_part,
        actions::repo_action,
        remotes::remote_list,
        remotes::remote_edit,
        remotes::commit_url,
        watcher::watch_repo,
        watcher::unwatch_repo,
        transfer::cancel_transfer,
        transfer::running_transfers,
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
        signing::signing_read,
        signing::signing_set_app,
        signing::signing_set_repo,
        signing::signing_keys,
        signing::signing_generate,
        ssh::ssh_read,
        ssh::ssh_set_app,
        ssh::ssh_set_repo,
        ssh::ssh_keys,
        ssh::ssh_generate,
        ssh::ssh_public_key,
        terminal::terminal_open,
        terminal::terminal_write,
        terminal::terminal_resize,
        terminal::terminal_close,
        tabs::session_get,
        tabs::tab_open,
        tabs::tab_close,
        tabs::tab_activate,
        tabs::tab_group,
        tabs::tab_move,
        tabs::tab_ungroup,
        tabs::group_collapse,
        tabs::tab_enter_submodule,
        tabs::tab_leave_submodule,
        tabs::tab_icon,
        tabs::group_rename,
        tabs::group_recolour,
        tabs::group_dissolve,
        tabs::group_close,
        version::app_version,
        graph::binary_self_test
    ]
}

fn window() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(graph::GraphCache::default())
        .manage(terminal::Terminals::default())
        .manage(watcher::Watchers::default())
        .manage(transfer::Transfers::default())
        .setup(|app| {
            load_state(app);
            Ok(())
        })
        .invoke_handler(handlers())
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
