//! The policy the window is served under, read from the configuration it is built from.
//!
//! Only a packaged build has a policy at all — under `devUrl` the page is plain http and none of
//! this applies — so nothing that runs against the dev server can catch a mistake here.

use std::path::Path;

fn config() -> serde_json::Value {
    let raw =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"))
            .expect("tauri.conf.json");
    serde_json::from_str(&raw).expect("tauri.conf.json is json")
}

fn security() -> serde_json::Value {
    config()["app"]["security"].clone()
}

/// Tauri appends a nonce to any directive it is allowed to modify, and a nonce in a directive
/// makes `'unsafe-inline'` inert — that is the CSP rule, not a Tauri quirk. The served policy
/// read `style-src 'self' 'unsafe-inline' 'nonce-5069828581219600622'`, so every stylesheet the
/// page built at runtime was refused: `el.sheet` came back null and the rules never existed.
///
/// xterm styles the terminal that way, with one stylesheet it writes from the theme and the font
/// it was given. Blocked, it drew in the page's proportional face with no colour at all, while
/// the rest of the window — styled from the bundled file, which `'self'` allows — looked right.
/// The terminal was the only thing on screen that could show the fault.
#[test]
fn style_src_keeps_the_inline_permission_it_declares() {
    let security = security();
    let csp = security["csp"].as_str().expect("a csp is configured");
    assert!(
        csp.contains("style-src 'self' 'unsafe-inline'"),
        "style-src must permit the stylesheets xterm writes at runtime: {csp}"
    );

    let exempt = security["dangerousDisableAssetCspModification"]
        .as_array()
        .expect("a list of directives Tauri must not touch");
    assert!(
        exempt.iter().any(|d| d == "style-src"),
        "style-src must be exempt, or Tauri's nonce turns 'unsafe-inline' off: {exempt:?}"
    );
}

/// The exemption is per directive and must stay that way. `true` would exempt all of them,
/// including `script-src`, whose hash is the thing actually holding the window shut.
#[test]
fn nothing_else_is_exempt() {
    let security = security();
    let exempt = security["dangerousDisableAssetCspModification"].clone();
    assert!(
        !exempt.is_boolean(),
        "a blanket exemption would drop the script-src hash too"
    );
    let names: Vec<&str> = exempt
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert_eq!(names, ["style-src"]);
}

#[test]
fn scripts_are_still_held_to_the_origin() {
    let csp = security();
    let csp = csp["csp"].as_str().expect("a csp is configured");
    assert!(csp.contains("script-src 'self'"), "{csp}");
    assert!(!csp.contains("script-src 'self' 'unsafe-inline'"), "{csp}");
    assert!(csp.contains("object-src 'none'"), "{csp}");
    assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
}
