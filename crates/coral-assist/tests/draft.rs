//! A draft asked of a server standing in for the Messages API: what goes out on the wire,
//! and what a draft is made of what comes back.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpListener;

use coral_assist::{Request, draft};
use secrecy::SecretString;

/// Answers one request with `status` and `body`, and hands back what it was sent.
fn serve_once(
    status: &'static str,
    body: &'static str,
) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("[::1]:0").unwrap();
    let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = n.trim().parse().unwrap();
            }
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let mut sent = vec![0; length];
        reader.read_exact(&mut sent).unwrap();
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        format!("{head}{}", String::from_utf8_lossy(&sent))
    });
    (url, handle)
}

#[tokio::test]
async fn a_draft_is_asked_with_the_key_version_and_fallback_and_read_back() {
    let (url, seen) = serve_once(
        "200 OK",
        r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-opus-5-5","stop_reason":"end_turn",
            "content":[{"type":"text","text":"core: say so\n\nWhy."}],"usage":{"input_tokens":10,"output_tokens":5}}"#,
    );
    let request = Request {
        diff: "diff --git a/a.txt b/a.txt\n+one\n".to_owned(),
        recent: vec!["core: before".to_owned()],
        model: String::new(),
    };
    let key = SecretString::from("sk-ant-test-not-real".to_owned());
    let made = draft(&key, &request, Some(&url)).await.unwrap();
    assert_eq!(made.summary, "core: say so");
    assert_eq!(made.body, "Why.");

    let wire = seen.join().unwrap().to_ascii_lowercase();
    assert!(wire.contains("x-api-key: sk-ant-test-not-real"), "{wire}");
    assert!(wire.contains("anthropic-version: 2026-10-21"), "{wire}");
    assert!(
        wire.contains("anthropic-beta: server-side-fallback-2026-07-01"),
        "{wire}"
    );
    assert!(wire.contains("\"fallbacks\":\"default\""), "{wire}");
    assert!(wire.contains("+one"), "the diff went with it");
}

#[tokio::test]
async fn a_refusal_from_the_api_comes_back_in_its_own_words() {
    let (url, seen) = serve_once(
        "400 Bad Request",
        r#"{"type":"error","error":{"type":"invalid_request_error","message":"model: not found"}}"#,
    );
    let request = Request {
        diff: "diff --git a/a b/a\n".to_owned(),
        model: "claude-nonexistent".to_owned(),
        ..Request::default()
    };
    let e = draft(&SecretString::from("k".to_owned()), &request, Some(&url))
        .await
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Anthropic refused the request (400): model: not found"
    );
    assert!(
        seen.join()
            .unwrap()
            .contains("\"model\":\"claude-nonexistent\"")
    );
}
