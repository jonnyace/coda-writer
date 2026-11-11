//! Desktop shell: exposes `coda-core` to the web UI and watches the notes
//! folder so edits made in other programs show up live.

mod config;

use std::path::Path;
use std::time::Duration;

use coda_core::{FsStore, Note, NoteSummary};
use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use tauri::{Emitter, Manager, State};

/// Event sent to the UI when anything in the notes folder changes.
const NOTES_CHANGED: &str = "notes-changed";
const WATCH_DEBOUNCE: Duration = Duration::from_millis(300);

type CommandResult<T> = Result<T, String>;

fn err(e: coda_core::Error) -> String {
    e.to_string()
}

// Commands are `async` so file work runs off the UI thread.

#[tauri::command(async)]
fn notes_root(store: State<'_, FsStore>) -> String {
    store.root().display().to_string()
}

#[tauri::command(async)]
fn list_folders(store: State<'_, FsStore>) -> CommandResult<Vec<String>> {
    store.folders().map_err(err)
}

#[tauri::command(async)]
fn list_notes(store: State<'_, FsStore>, folder: Option<String>) -> CommandResult<Vec<NoteSummary>> {
    store.notes(folder.as_deref()).map_err(err)
}

#[tauri::command(async)]
fn read_note(store: State<'_, FsStore>, path: String) -> CommandResult<Note> {
    store.read(&path).map_err(err)
}

#[tauri::command(async)]
fn create_note(store: State<'_, FsStore>, folder: String) -> CommandResult<NoteSummary> {
    store.create(&folder).map_err(err)
}

#[tauri::command(async)]
fn save_note(store: State<'_, FsStore>, path: String, title: String, body: String) -> CommandResult<NoteSummary> {
    store.save(&path, &title, &body).map_err(err)
}

#[tauri::command(async)]
fn set_note_flag(store: State<'_, FsStore>, path: String, key: String, value: bool) -> CommandResult<NoteSummary> {
    store.set_flag(&path, &key, value).map_err(err)
}

#[tauri::command(async)]
fn trash_note(store: State<'_, FsStore>, path: String) -> CommandResult<()> {
    store.trash(&path).map_err(err)
}

#[tauri::command(async)]
fn create_folder(store: State<'_, FsStore>, name: String) -> CommandResult<String> {
    store.create_folder(&name).map_err(err)
}

/// Hidden files are our own temp files, `.trash/` and things like `.git/`;
/// none of them change what the UI shows.
fn is_visible(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .all(|part| !part.as_os_str().to_string_lossy().starts_with('.'))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let paths = app.path();
            let notes_dir = config::Config::load(&paths.config_dir()?).notes_dir(&paths.home_dir()?);
            let store = FsStore::open(&notes_dir)
                .map_err(|e| format!("cannot open notes folder {}: {e}", notes_dir.display()))?;

            let handle = app.handle().clone();
            let root = store.root().to_path_buf();
            let mut watcher = new_debouncer(WATCH_DEBOUNCE, None, move |result: DebounceEventResult| {
                let Ok(events) = result else { return };
                let relevant = events.iter().any(|event| {
                    !matches!(event.kind, EventKind::Access(_))
                        && event.paths.iter().any(|path| is_visible(&root, path))
                });
                if relevant {
                    let _ = handle.emit(NOTES_CHANGED, ());
                }
            })?;
            watcher.watch(store.root(), RecursiveMode::Recursive)?;

            app.manage(store);
            // Dropping the watcher stops it, so it lives as long as the app.
            app.manage(watcher);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            notes_root,
            list_folders,
            list_notes,
            read_note,
            create_note,
            save_note,
            set_note_flag,
            trash_note,
            create_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running coda-writer");
}
