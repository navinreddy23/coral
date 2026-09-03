// The desktop build must not open a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use coral_app_lib::{actions, commands, graph, tabs};

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
