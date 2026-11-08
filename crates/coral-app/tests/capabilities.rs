//! What the window is allowed to ask of Tauri's plugins, read from the capability it is built
//! with.
//!
//! A command the capability does not grant fails only when it is called, in the window, with an
//! error the window shows as its own — and neither the browser preview nor the unit tests run
//! Tauri at all, so nothing else here can catch one missing.

use std::path::Path;

fn permissions() -> Vec<String> {
    let raw = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json"),
    )
    .expect("capabilities/default.json");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("the capability is json");
    json["permissions"]
        .as_array()
        .expect("a list of permissions")
        .iter()
        .filter_map(|p| p.as_str().map(str::to_owned))
        .collect()
}

/// `allow-open-url` lets the window call the opener and nothing more: the plugin then checks the
/// address against a scope, and with no scope granted every address is refused. Every link the
/// window opened — a request's page, the release page, a token page, the commit on its host —
/// failed with "Not allowed to open url" in the built app, and worked everywhere else.
#[test]
fn the_window_may_open_web_and_mail_links() {
    let granted = permissions();
    for wanted in ["opener:allow-open-url", "opener:allow-default-urls"] {
        assert!(
            granted.iter().any(|p| p == wanted),
            "{wanted} is missing from {granted:?}"
        );
    }
}
