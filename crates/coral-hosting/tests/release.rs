//! Reading GitHub's answer about Coral's newest release, and deciding whether it is newer.
//!
//! The body is a trimmed copy of what `GET /repos/{owner}/{repo}/releases/latest` returns.
//! No network: what would go wrong in the field is the reading and the comparison.

use coral_hosting::release::{is_newer, parse_latest};

#[test]
fn reads_the_version_and_the_page_from_the_latest_release() {
    let body = br#"{
      "url": "https://api.github.com/repos/navinreddy23/coral/releases/1",
      "html_url": "https://github.com/navinreddy23/coral/releases/tag/v1.2.4",
      "tag_name": "v1.2.4", "name": "Coral 1.2.4", "draft": false, "prerelease": false,
      "assets": [{"name": "SHA256SUMS.txt"}]
    }"#;
    let release = parse_latest(body).unwrap();
    assert_eq!(release.version, "1.2.4");
    assert_eq!(
        release.url,
        "https://github.com/navinreddy23/coral/releases/tag/v1.2.4"
    );
}

#[test]
fn takes_a_tag_with_or_without_its_v() {
    let body = br#"{"html_url": "https://example.invalid/r", "tag_name": "1.3.0"}"#;
    assert_eq!(parse_latest(body).unwrap().version, "1.3.0");
}

#[test]
fn refuses_a_body_that_is_not_a_release() {
    // What GitHub answers when a repository has no published release at all.
    assert!(parse_latest(br#"{"message": "Not Found", "status": "404"}"#).is_err());
    assert!(parse_latest(b"<html>").is_err());
}

#[test]
fn compares_versions_by_number_not_by_text() {
    assert!(is_newer("1.2.3", "1.2.4"));
    assert!(is_newer("1.2.3", "1.3.0"));
    assert!(is_newer("1.2.3", "2.0.0"));
    // As text, "1.10.0" sorts before "1.9.0".
    assert!(is_newer("1.9.0", "1.10.0"));
    assert!(!is_newer("1.10.0", "1.9.0"));
}

#[test]
fn the_same_version_or_an_older_one_is_not_newer() {
    assert!(!is_newer("1.2.3", "1.2.3"));
    assert!(!is_newer("1.2.3", "1.2.2"));
    assert!(!is_newer("1.2.3", "0.9.9"));
}

#[test]
fn a_pre_release_is_older_than_its_release() {
    assert!(!is_newer("1.3.0", "1.3.0-rc.1"));
    assert!(is_newer("1.2.3", "1.3.0-rc.1"));
    assert!(is_newer("1.3.0-rc.1", "1.3.0"));
}

#[test]
fn a_version_it_cannot_read_is_never_offered_as_an_update() {
    assert!(!is_newer("1.2.3", "nightly"));
    assert!(!is_newer("1.2.3", ""));
    assert!(!is_newer("1.2.3", "1.2"));
}
