//! One request to the Messages API: the staged change in, a commit message out.
//! <https://platform.claude.com/docs/en/api/messages>

use secrecy::{ExposeSecret as _, SecretString};

/// The model asked when the user names none.
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

/// The most of a diff that is sent. A change past this is refused with a sentence saying so
/// rather than cut short, since a message written about half a change describes the wrong one.
pub const MAX_DIFF_BYTES: usize = 400_000;

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const VERSION: &str = "2026-10-21";
/// The safety classifiers on the newest models can decline a request; this lets the API run a
/// declined one again on the model it recommends for the reason, rather than answer nothing.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

const SYSTEM: &str = "You write git commit messages. Reply with the message alone: no preface, \
no code fences, no quotation marks around it.\n\nThe first line is a summary of at most 72 \
characters in the imperative mood, in the style of the repository's recent subjects when they \
show one (a prefix such as `ui:` or `fix:`, capitalisation, tense). Then a blank line, then a \
body wrapped at 72 columns that says why the change was made and anything a reviewer could not \
see from the diff; leave the body out for a change that needs none. Describe what the diff \
actually does, not what a filename suggests.";

/// What a draft is written from.
#[derive(Clone, Debug, Default)]
pub struct Request {
    /// `git diff --cached`, as git printed it.
    pub diff: String,
    /// The subjects of the last few commits, newest first.
    pub recent: Vec<String>,
    /// The model to ask; empty for [`DEFAULT_MODEL`].
    pub model: String,
}

/// A drafted message, split as the commit box holds it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub summary: String,
    pub body: String,
    /// Which model wrote it, which a fallback can make different from the one asked.
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    #[error("nothing is staged to write a message about")]
    NothingStaged,
    #[error("the staged change is {0} KB, more than the {max} KB a draft is written from; stage less of it, or write this one", max = MAX_DIFF_BYTES / 1000)]
    TooLarge(usize),
    #[error("could not reach Anthropic: {0}")]
    Transport(String),
    #[error("Anthropic refused the request ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("the model declined to write this one")]
    Declined,
    #[error("the answer was not a message: {0}")]
    Malformed(String),
    #[error("the keyring could not be reached: {0}")]
    Keyring(String),
}

/// The JSON body of the request.
#[must_use]
pub fn body(request: &Request) -> serde_json::Value {
    let model = if request.model.trim().is_empty() {
        DEFAULT_MODEL
    } else {
        request.model.trim()
    };
    let recent = if request.recent.is_empty() {
        "(none yet)".to_owned()
    } else {
        request.recent.join("\n")
    };
    let user = format!(
        "Recent commit subjects in this repository, newest first:\n{recent}\n\nThe staged \
         change:\n\n{}",
        request.diff
    );
    serde_json::json!({
        "model": model,
        "max_tokens": 16000,
        // A short, well-specified task: low effort is enough and keeps it quick.
        "output_config": { "effort": "low" },
        "fallbacks": "default",
        "system": SYSTEM,
        "messages": [{ "role": "user", "content": user }],
    })
}

#[derive(serde::Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(serde::Deserialize)]
struct Reply {
    model: String,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    content: Vec<Block>,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(serde::Deserialize)]
struct ErrorDetail {
    message: String,
}

/// Reads a successful answer into a draft: the text blocks only, checked for a decline first.
///
/// # Errors
/// [`DraftError::Declined`] for a refusal, [`DraftError::Malformed`] for a body that is not a
/// message or a message with no text in it.
pub fn parse(body: &[u8]) -> Result<Draft, DraftError> {
    let reply: Reply =
        serde_json::from_slice(body).map_err(|e| DraftError::Malformed(e.to_string()))?;
    if reply.stop_reason.as_deref() == Some("refusal") {
        return Err(DraftError::Declined);
    }
    let text: String = reply
        .content
        .iter()
        .filter(|b| b.kind == "text")
        .map(|b| b.text.as_str())
        .collect();
    let text = text
        .trim()
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let (summary, body) = text.split_once('\n').unwrap_or((text, ""));
    let summary = summary.trim().to_owned();
    if summary.is_empty() {
        return Err(DraftError::Malformed(
            "the answer had no text in it".to_owned(),
        ));
    }
    Ok(Draft {
        summary,
        body: body.trim().to_owned(),
        model: reply.model,
    })
}

/// A refusal from the API, with its own sentence when it gave one.
fn refused(status: u16, body: &[u8]) -> DraftError {
    let message = serde_json::from_slice::<ErrorBody>(body).map_or_else(
        |_| String::from_utf8_lossy(body).chars().take(200).collect(),
        |e| e.error.message,
    );
    let message = match status {
        401 => "the API key was not accepted".to_owned(),
        429 => format!("too many requests just now; try again in a moment ({message})"),
        _ => message,
    };
    DraftError::Api { status, message }
}

/// Asks for a draft.
///
/// `endpoint` is the Messages API, or a server standing in for it in tests.
///
/// # Errors
/// [`DraftError::NothingStaged`] and [`DraftError::TooLarge`] before anything is sent;
/// otherwise the transport's, the API's or the answer's own failure.
pub async fn draft(
    key: &SecretString,
    request: &Request,
    endpoint: Option<&str>,
) -> Result<Draft, DraftError> {
    if request.diff.trim().is_empty() {
        return Err(DraftError::NothingStaged);
    }
    if request.diff.len() > MAX_DIFF_BYTES {
        return Err(DraftError::TooLarge(request.diff.len() / 1000));
    }
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent(concat!("coral/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DraftError::Transport(e.to_string()))?;
    let response = http
        .post(endpoint.unwrap_or(ENDPOINT))
        .header("x-api-key", key.expose_secret())
        .header("anthropic-version", VERSION)
        .header("anthropic-beta", FALLBACK_BETA)
        .json(&body(request))
        .send()
        .await
        .map_err(|e| DraftError::Transport(e.to_string()))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| DraftError::Transport(e.to_string()))?;
    if !status.is_success() {
        return Err(refused(status.as_u16(), &bytes));
    }
    parse(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_the_model_effort_and_fallback_and_carries_the_change() {
        let b = body(&Request {
            diff: "diff --git a/x b/x".to_owned(),
            recent: vec!["ui: one".to_owned(), "core: two".to_owned()],
            model: String::new(),
        });
        assert_eq!(b["model"], DEFAULT_MODEL);
        assert_eq!(b["output_config"]["effort"], "low");
        assert_eq!(b["fallbacks"], "default");
        let said = b["messages"][0]["content"].as_str().unwrap();
        assert!(said.contains("ui: one\ncore: two"));
        assert!(said.ends_with("diff --git a/x b/x"));
        assert!(
            b.get("thinking").is_none(),
            "thinking cannot be turned off on this model"
        );
    }

    #[test]
    fn an_answer_is_split_into_summary_and_body_and_a_decline_is_said() {
        let d = parse(
            br#"{"model":"claude-opus-5-5","stop_reason":"end_turn","content":[
                {"type":"thinking","thinking":""},
                {"type":"text","text":"ui: fold the panel\n\nBecause the lists grew."}]}"#,
        )
        .unwrap();
        assert_eq!(d.summary, "ui: fold the panel");
        assert_eq!(d.body, "Because the lists grew.");
        assert!(matches!(
            parse(br#"{"model":"m","stop_reason":"refusal","content":[]}"#),
            Err(DraftError::Declined)
        ));
        let fenced =
            parse(br#"{"model":"m","content":[{"type":"text","text":"```\nfix: x\n```"}]}"#)
                .unwrap();
        assert_eq!(fenced.summary, "fix: x");
    }

    #[test]
    fn an_api_refusal_keeps_its_own_words() {
        let e = refused(
            400,
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"bad model"}}"#,
        );
        assert_eq!(
            e.to_string(),
            "Anthropic refused the request (400): bad model"
        );
        assert!(refused(401, b"{}").to_string().contains("not accepted"));
    }

    #[tokio::test]
    async fn nothing_is_sent_for_an_empty_or_oversized_change() {
        let key = SecretString::from("k".to_owned());
        let empty = Request::default();
        assert!(matches!(
            draft(&key, &empty, Some("http://[::1]:9")).await,
            Err(DraftError::NothingStaged)
        ));
        let big = Request {
            diff: "x".repeat(MAX_DIFF_BYTES + 1),
            ..Request::default()
        };
        assert!(matches!(
            draft(&key, &big, Some("http://[::1]:9")).await,
            Err(DraftError::TooLarge(_))
        ));
    }
}
