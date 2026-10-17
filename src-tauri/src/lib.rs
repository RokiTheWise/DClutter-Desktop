//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::path::PathBuf;

use dclutter_core::{FileCandidate, QueueContext, QueueScorer, ScoreWeights};
use serde::Serialize;
use tauri::{AppHandle, Manager};

/// A folder's contents in triage order, as the interface shows them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Queue {
    folder: PathBuf,
    items: Vec<QueueItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueueItem {
    #[serde(flatten)]
    candidate: FileCandidate,
    name: String,
    score: f64,
    duplicate_count: usize,
    redundant_copy: bool,
    extracted_archive: bool,
}

/// Scans `folder`, or the user's Downloads folder when none is given, and
/// returns its items ranked highest score first.
#[tauri::command]
async fn scan_folder(app: AppHandle, folder: Option<PathBuf>) -> Result<Queue, String> {
    let folder = match folder {
        Some(folder) => folder,
        None => app.path().download_dir().map_err(|e| e.to_string())?,
    };

    tauri::async_runtime::spawn_blocking(move || {
        let candidates = dclutter_platform::candidates_in(&folder)
            .map_err(|e| format!("Couldn't read {}: {e}", folder.display()))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64();

        let ctx = QueueContext::new(&candidates);
        let items = QueueScorer::rank(candidates, now, &ScoreWeights::default())
            .into_iter()
            .map(|scored| QueueItem {
                name: scored.candidate.file_name().to_owned(),
                duplicate_count: ctx.duplicate_count(&scored.candidate),
                redundant_copy: ctx.is_redundant_copy(&scored.candidate),
                extracted_archive: ctx.is_extracted_archive(&scored.candidate),
                score: scored.score,
                candidate: scored.candidate,
            })
            .collect();

        Ok(Queue { folder, items })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![scan_folder])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
