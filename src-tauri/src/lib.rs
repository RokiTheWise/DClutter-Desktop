//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dclutter_core::{
    DecisionKind, FileCandidate, QueueContext, QueueScorer, ScoreWeights, Step, TriageSession,
};
use base64::Engine;
use dclutter_platform::{
    Destination, DestinationStore, MAX_DESTINATIONS, SessionStore, ThumbnailKind,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// The folder being triaged, with the facts its cards are drawn from.
struct Triage {
    folder: PathBuf,
    session: TriageSession,
    context: QueueContext,
    scores: HashMap<String, f64>,
    sessions: SessionStore,
}

struct AppState {
    triage: Mutex<Option<Triage>>,
    destinations: Mutex<Vec<Destination>>,
    store: DestinationStore,
    sessions: SessionStore,
    /// Recent previews, so going back to a card or undoing doesn't ask the
    /// shell again. Keyed by file and modification time.
    thumbnails: Mutex<HashMap<String, Option<Arc<PreviewImage>>>>,
}

/// How many previews to keep; a few screens' worth either way.
const THUMBNAIL_CACHE: usize = 32;
/// Requested size in pixels. The preview well is at most about 760 wide.
const THUMBNAIL_SIZE: u32 = 768;

/// A shell preview, ready for an <img>.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImage {
    /// "thumbnail" for a picture of the content, "icon" for a type icon.
    kind: &'static str,
    width: u32,
    height: u32,
    src: String,
}

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

/// A destination bin on the shelf.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DestinationItem {
    id: String,
    name: String,
    path: PathBuf,
}

impl From<&Destination> for DestinationItem {
    fn from(d: &Destination) -> Self {
        Self { id: d.id.clone(), name: d.name.clone(), path: d.path.clone() }
    }
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

    /// Saves the session after a change, so quitting never loses decisions
    /// or the ability to undo them. A failed save is logged rather than
    /// shown: the session itself is fine, and it's retried on the next
    /// change.
    fn save(&self) {
        if let Err(e) = self.sessions.save(&self.folder, &self.session.snapshot()) {
            eprintln!("Couldn't save the session for {}: {e}", self.folder.display());
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

    let sessions = state.sessions.clone();
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
        let queue = ranked.into_iter().map(|s| s.candidate).collect();
        // Pick up where the last session on this folder left off.
        let session = match sessions.load(&folder) {
            Some(saved) => TriageSession::restore(queue, saved, |path| path.exists()),
            None => TriageSession::new(queue),
        };
        Ok::<_, String>(Triage { folder, session, context, scores, sessions })
    })
    .await
    .map_err(|e| e.to_string())??;

    // Lets the interface load image previews straight from this folder,
    // and nothing outside it.
    app.asset_protocol_scope()
        .allow_directory(&triage.folder, false)
        .map_err(|e| e.to_string())?;

    let view = triage.view();
    *state.triage.lock().unwrap() = Some(triage);
    Ok(view)
}

#[tauri::command]
fn decide(state: State<'_, AppState>, kind: DecisionKind) -> Result<SessionView, String> {
    let mut guard = state.triage.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    triage.session.decide(kind);
    triage.save();
    Ok(triage.view())
}

// `async` runs these off the main thread: moving a filed file back can
// wait out a lock or copy across drives, and must not freeze the window.
#[tauri::command(async)]
fn undo(state: State<'_, AppState>) -> Result<Change, String> {
    let mut guard = state.triage.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    let step = triage.session.undo();
    if let Err(e) = carry_out(&step) {
        // The file is still in the destination, so put the filing back on
        // record; a card for a file that isn't in the folder would fail at
        // every action.
        triage.session.redo();
        return Err(format!("{e} It's still filed."));
    }
    triage.save();
    Ok(Change { view: triage.view(), kind: step.map(|s| s.kind) })
}

#[tauri::command(async)]
fn redo(state: State<'_, AppState>) -> Result<Change, String> {
    let mut guard = state.triage.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    let step = triage.session.redo();
    if let Err(e) = carry_out(&step) {
        triage.session.undo();
        return Err(e.to_string());
    }
    triage.save();
    Ok(Change { view: triage.view(), kind: step.map(|s| s.kind) })
}

/// Moves a filed file back or forward for an undo or redo. Uses the exact
/// recorded paths and never replaces whatever is there now.
fn carry_out(step: &Option<Step>) -> Result<(), dclutter_platform::MoveError> {
    match step.as_ref().and_then(|s| s.disk.as_ref()) {
        Some(disk) => dclutter_platform::move_back(&disk.from, &disk.to),
        None => Ok(()),
    }
}

/// The files staged for trash, in queue order, for the review list.
#[tauri::command]
fn staged_files(state: State<'_, AppState>) -> Result<Vec<StagedItem>, String> {
    let guard = state.triage.lock().unwrap();
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
        let mut guard = state.triage.lock().unwrap();
        let triage = guard.as_mut().ok_or(NO_FOLDER)?;
        triage.session.unstage(excluded.iter().map(String::as_str));
        triage.save();
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

    let mut guard = state.triage.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    triage.session.commit_trashed(trashed.iter().map(String::as_str));
    triage.save();
    let error = match errors.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        [first, rest @ ..] => Some(format!("{first} ({} more couldn't be moved either.)", rest.len())),
    };
    Ok(CommitOutcome { view: triage.view(), trashed: trashed.len(), error })
}

/// The destination folders, in shelf order.
#[tauri::command]
fn list_destinations(state: State<'_, AppState>) -> Vec<DestinationItem> {
    state.destinations.lock().unwrap().iter().map(DestinationItem::from).collect()
}

/// Asks the user for a folder with the system picker, which can also make
/// a new one, and adds it to the shelf. Returns the new list, unchanged if
/// the picker was cancelled.
#[tauri::command]
async fn add_destination(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<DestinationItem>, String> {
    if state.destinations.lock().unwrap().len() >= MAX_DESTINATIONS {
        return Err(
            "Three folders is the maximum. Highlight one on the shelf and press Delete to free a slot.".into(),
        );
    }

    // The folder you need often doesn't exist yet; you realise it the
    // moment a file demands one. The Windows picker's New folder button
    // covers that without sending anyone to Explorer.
    let picked = app.dialog().file().set_title("Choose a folder to file into").blocking_pick_folder();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(list_destinations(state));
    };

    let open_folder = state.triage.lock().unwrap().as_ref().map(|t| t.folder.clone());
    if open_folder.is_some_and(|folder| Destination::new(folder).is_folder(&path)) {
        return Err("That's the folder you're sorting. Choose somewhere to file things into.".into());
    }

    let mut destinations = state.destinations.lock().unwrap();
    if destinations.iter().any(|d| d.is_folder(&path)) {
        return Err(format!("{} is already on the shelf.", Destination::new(path).name));
    }
    destinations.push(Destination::new(path));
    state.store.save(&destinations).map_err(|e| format!("Couldn't save your folders: {e}"))?;
    Ok(destinations.iter().map(DestinationItem::from).collect())
}

/// Takes a folder off the shelf. The folder itself, and anything already
/// filed into it, are left alone.
#[tauri::command]
fn remove_destination(state: State<'_, AppState>, id: String) -> Result<Vec<DestinationItem>, String> {
    let mut destinations = state.destinations.lock().unwrap();
    destinations.retain(|d| d.id != id);
    state.store.save(&destinations).map_err(|e| format!("Couldn't save your folders: {e}"))?;
    Ok(destinations.iter().map(DestinationItem::from).collect())
}

/// Files the current card's file into the destination `id`, at once.
/// The move is recorded before the file is touched, so it can always be
/// undone; if the move fails, the record is withdrawn.
#[tauri::command]
async fn file_current(state: State<'_, AppState>, id: String) -> Result<SessionView, String> {
    let folder = state
        .destinations
        .lock()
        .unwrap()
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.path.clone())
        .ok_or("That folder is no longer on the shelf.")?;

    let (file_id, path) = {
        let mut guard = state.triage.lock().unwrap();
        let triage = guard.as_mut().ok_or(NO_FOLDER)?;
        let current = triage.session.current().ok_or("Nothing to file")?;
        let planned = folder.join(current.file_name());
        let started = triage.session.begin_file(planned).ok_or("Nothing to file")?;
        // Saved before the file is touched, so even a crash mid-move leaves
        // a record that can undo it.
        triage.save();
        started
    };

    // A move to another drive is a copy, which can take a while for big
    // files; don't hold the session or the main thread meanwhile.
    let moved = {
        let (path, folder) = (path.clone(), folder.clone());
        tauri::async_runtime::spawn_blocking(move || dclutter_platform::move_into_folder(&path, &folder))
            .await
            .map_err(|e| e.to_string())?
    };

    let mut guard = state.triage.lock().unwrap();
    let triage = guard.as_mut().ok_or(NO_FOLDER)?;
    match moved {
        Ok(landed) => {
            triage.session.amend_last_move(&file_id, landed);
            triage.save();
            Ok(triage.view())
        }
        Err(e) => {
            triage.session.withdraw_last_move(&file_id);
            triage.save();
            Err(e.to_string())
        }
    }
}

/// A preview of the file on the card, or of the next one so it's ready in
/// time. Answers only for those two, so the interface can't ask about
/// arbitrary paths. `None` when the shell has nothing for the file.
#[tauri::command]
async fn thumbnail(state: State<'_, AppState>, id: String) -> Result<Option<Arc<PreviewImage>>, String> {
    let (path, key) = {
        let guard = state.triage.lock().unwrap();
        let triage = guard.as_ref().ok_or(NO_FOLDER)?;
        let s = &triage.session;
        let candidate = [s.current(), s.up_next()]
            .into_iter()
            .flatten()
            .find(|c| c.id == id)
            .ok_or("Previews are only made for the current and next file")?;
        (candidate.path.clone(), format!("{}|{:?}", candidate.id, candidate.modified))
    };
    if let Some(cached) = state.thumbnails.lock().unwrap().get(&key) {
        return Ok(cached.clone());
    }

    // The shell can take a moment, especially for a video's first frame.
    let preview = tauri::async_runtime::spawn_blocking(move || {
        dclutter_platform::thumbnail(&path, THUMBNAIL_SIZE).map(|t| {
            Arc::new(PreviewImage {
                kind: match t.kind {
                    ThumbnailKind::Thumbnail => "thumbnail",
                    ThumbnailKind::Icon => "icon",
                },
                width: t.width,
                height: t.height,
                src: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(t.png)),
            })
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    let mut cache = state.thumbnails.lock().unwrap();
    if cache.len() >= THUMBNAIL_CACHE {
        cache.clear();
    }
    cache.insert(key, preview.clone());
    Ok(preview)
}

/// Opens the file on the card in its default app. Takes no path, so the
/// interface can only ever open the file it is showing.
#[tauri::command]
fn open_current(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let path = {
        let guard = state.triage.lock().unwrap();
        let triage = guard.as_ref().ok_or(NO_FOLDER)?;
        triage.session.current().ok_or("Nothing to open")?.path.clone()
    };
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Shrinks the window to fit the screen it opens on, minus the taskbar,
/// and centres it there. The default 716 × 880 suits the triage card, but
/// on a laptop at 125–150% scaling it's taller than the usable area, and
/// the decision buttons would open underneath the taskbar.
fn fit_to_work_area(window: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) else {
        return Ok(());
    };
    let area = monitor.work_area();
    let (outer, inner) = (window.outer_size()?, window.inner_size()?);
    // The title bar and borders, which set_size doesn't include.
    let frame = PhysicalSize::new(outer.width - inner.width, outer.height - inner.height);
    let margin = (12.0 * monitor.scale_factor()).round() as u32;

    let width = outer.width.min(area.size.width.saturating_sub(2 * margin));
    let height = outer.height.min(area.size.height.saturating_sub(2 * margin));
    if (width, height) != (outer.width, outer.height) {
        window.set_size(PhysicalSize::new(width - frame.width, height - frame.height))?;
    }
    window.set_position(PhysicalPosition::new(
        area.position.x + ((area.size.width - width) / 2) as i32,
        area.position.y + ((area.size.height - height) / 2) as i32,
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let store = DestinationStore::new(app.path().app_config_dir()?.join("destinations.json"));
            app.manage(AppState {
                triage: Mutex::new(None),
                destinations: Mutex::new(store.load()),
                store,
                sessions: SessionStore::new(app.path().app_data_dir()?.join("sessions")),
                thumbnails: Mutex::new(HashMap::new()),
            });
            // The window starts hidden so it never shows at the wrong size.
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = fit_to_work_area(&window) {
                    eprintln!("Couldn't fit the window to the screen: {e}");
                }
                window.show()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_queue,
            decide,
            undo,
            redo,
            staged_files,
            commit_trash,
            list_destinations,
            add_destination,
            remove_destination,
            file_current,
            thumbnail,
            open_current
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
