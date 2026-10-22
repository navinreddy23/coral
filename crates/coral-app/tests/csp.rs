//! The window's content security policy, held to what it is for.

fn directives() -> Vec<(String, Vec<String>)> {
    let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let csp = conf["app"]["security"]["csp"].as_str().unwrap();
    csp.split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut words = d.split_whitespace().map(str::to_owned);
            (words.next().unwrap(), words.collect())
        })
        .collect()
}

fn sources(name: &str) -> Vec<String> {
    directives()
        .into_iter()
        .find(|(n, _)| n == name)
        .map_or_else(|| panic!("no {name} directive"), |(_, s)| s)
}

/// The image view draws a file's bytes through a blob URL, which only the page's own script
/// can make.
#[test]
fn images_may_come_from_blobs() {
    assert!(sources("img-src").contains(&"blob:".to_owned()));
}

/// A blob is a way to run code nobody shipped when scripts may come from one.
#[test]
fn scripts_come_from_the_bundle_alone() {
    assert_eq!(sources("script-src"), ["'self'"]);
    for (name, srcs) in directives() {
        assert!(
            !srcs.iter().any(|s| s == "'unsafe-eval'"),
            "{name} allows eval"
        );
    }
    assert_eq!(sources("object-src"), ["'none'"]);
}
