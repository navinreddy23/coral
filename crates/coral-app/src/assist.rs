//! Drafting a commit message with Claude: the setting, the key, and the one request.
//!
//! Off until the user turns it on and gives a key of their own. What is sent is the staged
//! diff and the subjects of the last few commits, to Anthropic, and only when asked.

// Tauri requires `State` by value in a command signature; it is a handle, not the data.
#![allow(clippy::needless_pass_by_value)]

use coral_core::process::GitRunner;
use coral_core::repo::RepoLocation;
use secrecy::SecretString;

use crate::commands::IpcError;
use crate::experimental::{Assist, Experimental, Settings};

/// What the Experimental pane shows. The key itself never leaves the keyring.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistView {
    pub enabled: bool,
    pub model: String,
    pub has_key: bool,
    /// The model asked when none is named.
    pub default_model: &'static str,
}

fn refused(label: &'static str, e: &coral_assist::DraftError) -> IpcError {
    coral_core::CoralError::Refused {
        label,
        detail: e.to_string(),
    }
    .into()
}

fn view(settings: &Experimental) -> AssistView {
    let assist = settings.read().assist;
    AssistView {
        enabled: assist.enabled,
        model: assist.model,
        has_key: coral_assist::key::load().ok().flatten().is_some(),
        default_model: coral_assist::draft::DEFAULT_MODEL,
    }
}

/// Whether drafting is on, with which model, and whether a key is kept.
#[tauri::command]
#[must_use]
pub fn assist_read(settings: tauri::State<'_, Experimental>) -> AssistView {
    view(&settings)
}

/// Turns drafting on or off and names the model.
#[tauri::command]
#[must_use]
pub fn assist_set(
    settings: tauri::State<'_, Experimental>,
    enabled: bool,
    model: String,
) -> AssistView {
    settings.write(Settings {
        assist: Assist {
            enabled,
            model: model.trim().to_owned(),
        },
        ..settings.read()
    });
    view(&settings)
}

/// Keeps an API key in the keyring, or forgets it with `None`.
///
/// # Errors
/// A keyring that refuses.
#[tauri::command]
pub fn assist_set_key(
    settings: tauri::State<'_, Experimental>,
    key: Option<String>,
) -> Result<AssistView, IpcError> {
    let key = key
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty())
        .map(SecretString::from);
    coral_assist::key::store(key.as_ref()).map_err(|e| refused("keep the key", &e))?;
    Ok(view(&settings))
}

/// Drafts a message for what is staged in the repository at `path`.
///
/// `CORAL_ASSIST_URL` stands a server in for the Messages API, for the tests.
///
/// # Errors
/// Drafting turned off or no key kept; nothing staged or too much of it; and whatever the API
/// answers.
#[tauri::command]
pub async fn assist_draft(
    settings: tauri::State<'_, Experimental>,
    path: String,
) -> Result<coral_assist::Draft, IpcError> {
    let label = "draft a message";
    let assist = settings.read().assist;
    if !assist.enabled {
        return Err(coral_core::CoralError::Refused {
            label,
            detail: "drafting is off; turn it on in Preferences, Experimental".to_owned(),
        }
        .into());
    }
    let key = coral_assist::key::load()
        .map_err(|e| refused(label, &e))?
        .ok_or_else(|| coral_core::CoralError::Refused {
            label,
            detail: "no API key is kept; add one in Preferences, Experimental".to_owned(),
        })?;
    let runner = GitRunner::discover().await?;
    let loc = RepoLocation::discover(&runner, std::path::Path::new(&path)).await?;
    let request = coral_assist::Request {
        diff: loc.staged_patch(&runner).await?,
        recent: recent_subjects(&runner, &loc).await,
        model: assist.model,
    };
    let endpoint = std::env::var("CORAL_ASSIST_URL").ok();
    let logged = crate::activity::started(&path, label);
    match coral_assist::draft(&key, &request, endpoint.as_deref()).await {
        Ok(draft) => {
            logged.finished();
            Ok(draft)
        }
        Err(e) => {
            logged.failed(&e.to_string());
            Err(refused(label, &e))
        }
    }
}

/// The last few subjects, for the draft to follow their style. None on an unborn branch.
async fn recent_subjects(runner: &GitRunner, loc: &RepoLocation) -> Vec<String> {
    let query = coral_core::history::LogQuery {
        limit: Some(12),
        ..Default::default()
    };
    loc.log(runner, &query).await.map_or_else(
        |_| Vec::new(),
        |commits| commits.into_iter().map(|c| c.summary.to_string()).collect(),
    )
}
