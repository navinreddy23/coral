use std::process::ExitCode;

use coral_core::CoralError;
use serde::Serialize;

/// Envelope version. Every consumer switches on this before reading anything else, so a
/// change here is a breaking change and must be a reviewed, deliberate commit.
pub const SCHEMA: u32 = 1;

/// Process exit codes. Callers — including the kernel scenario scripts — branch on these, so
/// they are as much a part of the contract as the JSON.
pub mod exit {
    /// Command succeeded.
    pub const OK: u8 = 0;
    /// git itself failed.
    pub const GIT: u8 = 1;
    /// Bad invocation.
    pub const USAGE: u8 = 2;
    /// The operation stopped on conflicts and needs resolution.
    pub const CONFLICTS: u8 = 3;
    /// Credentials are required.
    pub const AUTH: u8 = 4;
    /// Cancelled by the user.
    pub const CANCELLED: u8 = 130;
}

#[derive(Serialize)]
struct Ok_<T> {
    schema: u32,
    ok: bool,
    result: T,
}

#[derive(Serialize)]
struct Err_ {
    schema: u32,
    ok: bool,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitBody>,
}

#[derive(Serialize)]
struct GitBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    exit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    argv: Vec<String>,
}

/// A command's result in both renderings, plus the exit code.
pub struct Rendered {
    pub json: serde_json::Value,
    pub text: String,
    pub code: ExitCode,
}

/// How a successful result reads for a person. Errors are rendered centrally.
pub trait Human {
    fn human(&self) -> String;
}

/// Renders a result as the envelope and returns the matching exit code.
///
/// Both the JSON and the human rendering go through here so the two can never disagree about
/// whether the command succeeded.
pub fn render<T: Serialize + Human>(result: &Result<T, CoralError>) -> Rendered {
    let (json, code) = envelope(result);
    let text = match result {
        Ok(v) => v.human(),
        Err(e) => format!("error [{}]: {e}", e.code()),
    };
    Rendered { json, text, code }
}

fn envelope<T: Serialize>(result: &Result<T, CoralError>) -> (serde_json::Value, ExitCode) {
    match result {
        Ok(value) => {
            let body = Ok_ {
                schema: SCHEMA,
                ok: true,
                result: value,
            };
            (
                serde_json::to_value(body).unwrap_or_else(|e| fallback(&e)),
                ExitCode::from(exit::OK),
            )
        }
        Err(e) => {
            let body = Err_ {
                schema: SCHEMA,
                ok: false,
                error: ErrorBody {
                    code: e.code(),
                    message: e.to_string(),
                    git: git_body(e),
                },
            };
            let value = serde_json::to_value(body).unwrap_or_else(|e| fallback(&e));
            (value, ExitCode::from(exit_code_for(e)))
        }
    }
}

fn git_body(e: &CoralError) -> Option<GitBody> {
    let argv = e.argv()?;
    Some(GitBody {
        exit: match e {
            CoralError::GitExit { code, .. } => Some(*code),
            _ => None,
        },
        stderr: e.stderr().map(str::to_owned),
        argv: argv.to_vec(),
    })
}

/// Maps an error onto its exit code. One place, so no command module invents its own.
fn exit_code_for(e: &CoralError) -> u8 {
    match e {
        CoralError::GitMissing
        | CoralError::GitTooOld { .. }
        | CoralError::GitVersionUnparsable { .. }
        | CoralError::NotARepository(_) => exit::USAGE,
        _ => exit::GIT,
    }
}

/// Serialization of our own types cannot realistically fail, but panicking in the output
/// path would lose the real error, so degrade to a valid envelope instead.
fn fallback(e: &serde_json::Error) -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA,
        "ok": false,
        "error": { "code": "serialization_failed", "message": e.to_string() }
    })
}
