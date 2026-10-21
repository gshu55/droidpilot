use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct UpdateService {
    // Held throughout each operation: competing checks/installs fail immediately.
    pending: Mutex<Option<Update>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    version: String,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
    attempt: u8,
}

#[derive(Debug, Serialize)]
pub struct UpdateError {
    code: &'static str,
    detail: String,
}

impl UpdateError {
    fn new(code: &'static str, detail: impl ToString) -> Self {
        Self {
            code,
            detail: detail.to_string(),
        }
    }
}

fn retryable(error: &tauri_plugin_updater::Error) -> bool {
    match error {
        tauri_plugin_updater::Error::Reqwest(error) => {
            error.is_timeout() || error.is_connect() || error.is_body()
        }
        tauri_plugin_updater::Error::Network(message) => ["408", "429", "500", "502", "503", "504"]
            .iter()
            .any(|code| message.contains(code)),
        _ => false,
    }
}

fn download_error(error: tauri_plugin_updater::Error) -> UpdateError {
    let code = match &error {
        tauri_plugin_updater::Error::Minisign(_)
        | tauri_plugin_updater::Error::Base64(_)
        | tauri_plugin_updater::Error::SignatureUtf8(_) => "signature",
        _ => "download",
    };
    UpdateError::new(code, error)
}

#[tauri::command]
pub async fn app_update_check(
    app: AppHandle,
    service: State<'_, UpdateService>,
) -> Result<Option<UpdateMetadata>, UpdateError> {
    let mut pending = service
        .pending
        .try_lock()
        .map_err(|_| UpdateError::new("busy", "Update in progress"))?;
    *pending = None;
    // Same split as Portiva: short manifest timeout, longer package download timeout.
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(30 * 60))
        .build()
        .map_err(|error| UpdateError::new("check", error))?;
    let update = tokio::time::timeout(Duration::from_secs(60), updater.check())
        .await
        .map_err(|_| UpdateError::new("timeout", "Update check timed out"))?
        .map_err(|error| UpdateError::new("check", error))?;
    let metadata = update.as_ref().map(|update| UpdateMetadata {
        version: update.version.clone(),
        notes: update.body.clone(),
    });
    *pending = update;
    Ok(metadata)
}

#[tauri::command]
pub async fn app_update_install(
    app: AppHandle,
    service: State<'_, UpdateService>,
    progress: Channel<UpdateProgress>,
) -> Result<(), UpdateError> {
    let mut pending = service
        .pending
        .try_lock()
        .map_err(|_| UpdateError::new("busy", "Update in progress"))?;
    let update = pending
        .as_ref()
        .ok_or_else(|| UpdateError::new("missing", "Check for updates first"))?;
    let mut attempt = 1;
    let bytes = loop {
        let _ = progress.send(UpdateProgress {
            phase: "downloading",
            downloaded: 0,
            total: None,
            attempt,
        });
        let mut downloaded = 0u64;
        let mut last_sent = Instant::now();
        let result = update
            .download(
                |chunk, total| {
                    downloaded += chunk as u64;
                    if last_sent.elapsed() >= Duration::from_millis(100)
                        || total == Some(downloaded)
                    {
                        let _ = progress.send(UpdateProgress {
                            phase: "downloading",
                            downloaded,
                            total,
                            attempt,
                        });
                        last_sent = Instant::now();
                    }
                },
                || {},
            )
            .await;
        match result {
            Ok(bytes) => break bytes, // Signature verification is included in download().
            Err(error) if attempt < 3 && retryable(&error) => {
                tokio::time::sleep(Duration::from_secs(u64::from(attempt))).await;
                attempt += 1;
            }
            Err(error) => return Err(download_error(error)),
        }
    };
    let _ = progress.send(UpdateProgress {
        phase: "installing",
        downloaded: bytes.len() as u64,
        total: Some(bytes.len() as u64),
        attempt,
    });
    let installer = update.clone();
    tauri::async_runtime::spawn_blocking(move || installer.install(&bytes))
        .await
        .map_err(|error| UpdateError::new("install", error))?
        .map_err(|error| UpdateError::new("install", error))?;
    *pending = None;
    // Windows installers terminate the running app; other platforms need a restart.
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_retry_bad_signatures_or_missing_assets() {
        let signature = tauri_plugin_updater::Error::SignatureUtf8("invalid".into());
        assert!(!retryable(&signature));
        assert_eq!(download_error(signature).code, "signature");
        assert!(!retryable(&tauri_plugin_updater::Error::Network(
            "404 Not Found".into()
        )));
        assert!(retryable(&tauri_plugin_updater::Error::Network(
            "503 Service Unavailable".into()
        )));
    }
}
