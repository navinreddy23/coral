// The desktop build must not open a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use coral_app_lib::{commands, graph};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CORAL_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    tracing::info!("coral-app starting");

    tauri::Builder::default()
        .manage(graph::GraphCache::default())
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
            graph::binary_self_test
        ])
        .run(tauri::generate_context!())
        .expect("tauri failed to start");
}
