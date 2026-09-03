// The desktop build must not open a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use coral_app_lib::{commands, graph, tabs};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CORAL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    tracing::info!("coral-app starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            graph::commit_detail,
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
