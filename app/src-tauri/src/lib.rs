//! Desktop shell: exposes `coda-core` to the web UI and watches the notes
//! folder so edits made in other programs show up live.

mod config;
mod publish;

use std::path::Path;
use std::time::Duration;

use coda_core::{FsStore, Note, NoteSummary};
use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use publish::Server;
use serde::Serialize;
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
fn trash_note(store: State<'_, FsStore>, publisher: State<'_, Publisher>, path: String) -> CommandResult<()> {
    // A trashed note must not stay live on the blog. If it cannot be taken
    // down, keep the note so the user can see it is still published.
    let note = store.read(&path).map_err(err)?.summary;
    if let (true, Some(id), Some(server)) = (note.published, note.id, publisher.0.as_ref()) {
        server.remove(&id)?;
        store.set_flag(&path, "published", false).map_err(err)?;
    }
    store.trash(&path).map_err(err)
}

#[tauri::command(async)]
fn create_folder(store: State<'_, FsStore>, name: String) -> CommandResult<String> {
    store.create_folder(&name).map_err(err)
}

/// The blog server from config.toml, if one is set up.
struct Publisher(Option<Server>);

const NO_SERVER: &str =
    "To publish, add a [server] section with url and token to ~/.config/coda-writer/config.toml";

#[derive(Serialize)]
struct PublishResult {
    note: NoteSummary,
    /// Public address of the post; `None` after unpublishing.
    url: Option<String>,
}

/// Publish a note to the blog, or take it down. The `published` flag in the
/// file only changes if the server agreed, so the file never claims a state
/// the blog is not in.
#[tauri::command(async)]
fn set_published(
    store: State<'_, FsStore>,
    publisher: State<'_, Publisher>,
    path: String,
    value: bool,
) -> CommandResult<PublishResult> {
    let server = publisher.0.as_ref().ok_or(NO_SERVER)?;
    if value {
        // Setting the flag also gives a hand-written note its id.
        store.set_flag(&path, "published", true).map_err(err)?;
        let note = store.read(&path).map_err(err)?;
        match server.push(&note) {
            Ok(url) => Ok(PublishResult { note: note.summary, url: Some(url) }),
            Err(e) => {
                store.set_flag(&path, "published", false).map_err(err)?;
                Err(e)
            }
        }
    } else {
        if let Some(id) = store.read(&path).map_err(err)?.summary.id {
            server.remove(&id)?;
        }
        let note = store.set_flag(&path, "published", false).map_err(err)?;
        Ok(PublishResult { note, url: None })
    }
}

/// Send the latest text of an already-published note to the blog. The UI
/// calls this after each save; it does nothing for private notes.
#[tauri::command(async)]
fn push_published(
    store: State<'_, FsStore>,
    publisher: State<'_, Publisher>,
    path: String,
) -> CommandResult<Option<String>> {
    let note = store.read(&path).map_err(err)?;
    if !note.summary.published {
        return Ok(None);
    }
    let server = publisher.0.as_ref().ok_or(NO_SERVER)?;
    server.push(&note).map(Some)
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
            let config = config::Config::load(&paths.config_dir()?);
            let notes_dir = config.notes_dir(&paths.home_dir()?);
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
            app.manage(Publisher(config.server));
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
            set_published,
            push_published,
        ])
        .run(tauri::generate_context!())
        .expect("error while running coda-writer");
}
