//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use dclutter_core::{
    DecisionKind, FileCandidate, QueueContext, QueueScorer, ScoreWeights, TriageSession,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// The folder being triaged, with the facts its cards are drawn from.
struct Triage {
    folder: PathBuf,
    session: TriageSession,
    context: QueueContext,
    scores: HashMap<String, f64>,
}

#[derive(Default)]
struct AppState(Mutex<Option<Triage>>);

/// Everything the triage screen shows, sent after every change.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionView {
    folder: PathBuf,
    current: Option<CardItem>,
    /// Sent ahead so the interface can preload its preview.
    up_next: Option<CardItem>,
    remaining: usize,
    total: usize,
    sorted: usize,
    staged: usize,
    trashed: usize,
    can_undo: bool,
    can_redo: bool,
}

/// A staged file as the review-trash list shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StagedItem {
    id: String,
    name: String,
    bytes: u64,
    is_directory: bool,
}

/// What happened when the user confirmed the review-trash list.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitOutcome {
    view: SessionView,
    trashed: usize,
    /// Set when one or more files couldn't be recycled. They stay staged,
    /// so the list still shows them.
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardItem {
    #[serde(flatten)]
    candidate: FileCandidate,
    name: String,
    extension: String,
    score: f64,
    duplicate_count: usize,
    extracted_archive: bool,
}

/// A view plus the decision an undo or redo just reversed or re-applied,
/// so the card can return from the side it left by.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Change {
    view: SessionView,
    kind: Option<DecisionKind>,
}

impl Triage {
    fn card(&self, candidate: &FileCandidate) -> CardItem {
        CardItem {
            name: candidate.file_name().to_owned(),
            extension: candidate.extension().to_lowercase(),
            score: self.scores.get(&candidate.id).copied().unwrap_or_default(),
            duplicate_count: self.context.duplicate_count(candidate),
            extracted_archive: self.context.is_extracted_archive(candidate),
            candidate: candidate.clone(),
        }
    }

    fn view(&self) -> SessionView {
        let s = &self.session;
        SessionView {
            folder: self.folder.clone(),
            current: s.current().map(|c| self.card(c)),
            up_next: s.up_next().map(|c| self.card(c)),
            remaining: s.remaining_count(),
            total: s.total_count(),
            sorted: s.sorted_count(),
            staged: s.staged().len(),
            trashed: s.trashed_count(),
            can_undo: s.can_undo(),
            can_redo: s.can_redo(),
        }
    }
}

const NO_FOLDER: &str = "No folder is open";

/// Scans `folder`, or the user's Downloads folder when none is given, and
/// starts a fresh triage session on it.
#[tauri::command]
async fn open_queue(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: Option<PathBuf>,
) -> Result<SessionView, String> {
    let folder = match folder {
        Some(folder) => folder,
        None => app.path().download_dir().map_err(|e| e.to_string())?,
    };

    let triage = tauri::async_runtime::spawn_blocking(move || {
        let candidates = dclutter_platform::candidates_in(&folder)
            .map_err(|e| format!("Couldn't read {}: {e}", folder.display()))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64();

        let context = QueueContext::new(&candidates);
        let ranked = QueueScorer::rank(candidates, now, &ScoreWeights::default());
        let scores = ranked.iter().map(|s| (s.candidate.id.clone(), s.score)).collect();
        let session = TriageSession::new(ranked.into_iter().map(|s| s.candidate).collect());
        Ok::<_, String>(Triage { folder, session, context, scores })
    })
    .await
    .map_err(|e| e.to_string())??;

    // Lets the interface load image previews straight from this folder,
    // and nothing outside it.
    app.asset_protocol_scope()
        .allow_directory(&triage.folder, false)
        .map_err(|e| e.to_string())?;

    let view = triage.view();
    *state.0.lock().unwrap() = Some(triage);
    Ok(view)
}

#[tauri::command]
fn decide(state: State<'_, AppState>, kind: DecisionKind) -> Result<SessionView, String> {
    let mut guard = state.0.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    triage.session.decide(kind);
    Ok(triage.view())
}

#[tauri::command]
fn undo(state: State<'_, AppState>) -> Result<Change, String> {
    let mut guard = state.0.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    let kind = triage.session.undo();
    Ok(Change { view: triage.view(), kind })
}

#[tauri::command]
fn redo(state: State<'_, AppState>) -> Result<Change, String> {
    let mut guard = state.0.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    let kind = triage.session.redo();
    Ok(Change { view: triage.view(), kind })
}

/// The files staged for trash, in queue order, for the review list.
#[tauri::command]
fn staged_files(state: State<'_, AppState>) -> Result<Vec<StagedItem>, String> {
    let guard = state.0.lock().unwrap();
    let triage = guard.as_ref().ok_or(NO_FOLDER)?;
    Ok(triage
        .session
        .staged()
        .into_iter()
        .map(|c| StagedItem {
            id: c.id.clone(),
            name: c.file_name().to_owned(),
            bytes: c.bytes,
            is_directory: c.is_directory,
        })
        .collect())
}

/// Keeps the files the user unticked, then moves every other staged file
/// to the Recycle Bin. Only ever acts on files the session has staged, so
/// the interface can't name an arbitrary path to delete.
#[tauri::command]
async fn commit_trash(
    state: State<'_, AppState>,
    excluded: Vec<String>,
) -> Result<CommitOutcome, String> {
    let to_trash: Vec<(String, PathBuf)> = {
        let mut guard = state.0.lock().unwrap();
        let triage = guard.as_mut().ok_or(NO_FOLDER)?;
        triage.session.unstage(excluded.iter().map(String::as_str));
        triage.session.staged().into_iter().map(|c| (c.id.clone(), c.path.clone())).collect()
    };

    // Recycling can take a moment per file; keep it off the main thread
    // and don't hold the session lock while the shell works.
    let (trashed, errors) = tauri::async_runtime::spawn_blocking(move || {
        let mut trashed = Vec::new();
        let mut errors = Vec::new();
        for (id, path) in to_trash {
            match dclutter_platform::move_to_recycle_bin(&path) {
                Ok(()) => trashed.push(id),
                Err(e) => errors.push(e.to_string()),
            }
        }
        (trashed, errors)
    })
    .await
    .map_err(|e| e.to_string())?;

    let mut guard = state.0.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    triage.session.commit_trashed(trashed.iter().map(String::as_str));
    let error = match errors.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        [first, rest @ ..] => Some(format!("{first} ({} more couldn't be moved either.)", rest.len())),
    };
    Ok(CommitOutcome { view: triage.view(), trashed: trashed.len(), error })
}

/// Opens the file on the card in its default app. Takes no path, so the
/// interface can only ever open the file it is showing.
#[tauri::command]
fn open_current(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let path = {
        let guard = state.0.lock().unwrap();
        let triage = guard.as_ref().ok_or(NO_FOLDER)?;
        triage.session.current().ok_or("Nothing to open")?.path.clone()
    };
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_queue,
            decide,
            undo,
            redo,
            staged_files,
            commit_trash,
            open_current
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
