//! The notes folder on disk.
//!
//! Every note is one `.md` file. Folders are directories. The UI addresses
//! notes by their path relative to the root (always with `/` separators).
//! Each note the app touches also gets a stable `id` in its header: paths
//! change when a note is renamed or moved, so the future sync service will
//! key on the id instead.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local, SecondsFormat, Utc};
use serde::Serialize;

use crate::frontmatter::Document;

const TRASH_DIR: &str = ".trash";
const SNIPPET_CHARS: usize = 140;
const SLUG_CHARS: usize = 60;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0} already exists")]
    AlreadyExists(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NoteSummary {
    /// Path relative to the notes root, e.g. `work/meeting.md`.
    pub path: String,
    /// Containing folder relative to the root; empty for the root itself.
    pub folder: String,
    pub id: Option<String>,
    pub title: String,
    pub snippet: String,
    /// Day the note was written, `YYYY-MM-DD`. Shown on the published post.
    pub date: String,
    /// RFC 3339 timestamp of the last edit.
    pub updated: String,
    pub pinned: bool,
    pub published: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    #[serde(flatten)]
    pub summary: NoteSummary,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    /// Open the notes folder, creating it if it does not exist yet.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// All folders below the root, as sorted relative paths.
    pub fn folders(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        self.walk_dirs(&self.root, &mut out)?;
        out.sort_by_key(|f| f.to_lowercase());
        Ok(out)
    }

    /// Notes directly inside `folder` (`""` is the root), or every note in
    /// every folder when `folder` is `None`. Pinned notes come first, then
    /// most recently edited.
    pub fn notes(&self, folder: Option<&str>) -> Result<Vec<NoteSummary>> {
        let mut files = Vec::new();
        match folder {
            Some(folder) => self.collect_notes(&self.resolve_dir(folder)?, false, &mut files)?,
            None => self.collect_notes(&self.root, true, &mut files)?,
        }
        let mut notes: Vec<NoteSummary> = files
            .iter()
            // A file that vanishes or is unreadable mid-scan should not hide
            // every other note, so skip it.
            .filter_map(|file| self.load(file).ok())
            .map(|note| note.summary)
            .collect();
        notes.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated.cmp(&a.updated))
                .then_with(|| a.path.cmp(&b.path))
        });
        Ok(notes)
    }

    /// Notes whose title or text contains every word of `query`, ignoring
    /// case. Notes matching in the title come first, then most recent.
    ///
    /// This reads every note on each call. That is fast enough for a few
    /// thousand notes and needs no index that could go stale; add one only
    /// if a real notes folder proves too slow.
    pub fn search(&self, query: &str) -> Result<Vec<NoteSummary>> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        self.collect_notes(&self.root, true, &mut files)?;

        let mut hits: Vec<(bool, NoteSummary)> = Vec::new();
        for note in files.iter().filter_map(|file| self.load(file).ok()) {
            let title = note.summary.title.to_lowercase();
            let body = note.body.to_lowercase();
            if !terms.iter().all(|t| title.contains(t) || body.contains(t)) {
                continue;
            }
            let in_title = terms.iter().all(|t| title.contains(t));
            let mut summary = note.summary;
            if let Some(context) = match_context(&note.body, &body, &terms) {
                summary.snippet = context;
            }
            hits.push((in_title, summary));
        }
        hits.sort_by(|(a_title, a), (b_title, b)| {
            b_title
                .cmp(a_title)
                .then_with(|| b.updated.cmp(&a.updated))
                .then_with(|| a.path.cmp(&b.path))
        });
        Ok(hits.into_iter().map(|(_, summary)| summary).collect())
    }

    pub fn read(&self, path: &str) -> Result<Note> {
        self.load(&self.resolve_note(path)?)
    }

    /// Create an empty note in `folder` and return it.
    pub fn create(&self, folder: &str) -> Result<NoteSummary> {
        let dir = self.resolve_dir(folder)?;
        let file = unique_path(&dir, "untitled", "md", None);
        let mut doc = Document::default();
        doc.set("id", &new_id());
        doc.set("title", "");
        doc.set("date", &Local::now().format("%Y-%m-%d").to_string());
        doc.set("updated", &now());
        write_atomic(&file, &doc.render())?;
        Ok(self.load(&file)?.summary)
    }

    /// Write a note's title and body. The file is renamed to match a changed
    /// title, so the returned summary may carry a new `path`.
    pub fn save(&self, path: &str, title: &str, body: &str) -> Result<NoteSummary> {
        let file = self.resolve_note(path)?;
        let mut doc = Document::parse(&read_file(&file, path)?);
        let title = title.trim();
        // A note with no `title:` line is titled by its first heading. Compare
        // against that, so editing only the body does not rename the file.
        let old_title = doc.get("title").or_else(|| split_title_heading(&doc.body).map(|(heading, _)| heading));
        let title_changed = old_title.as_deref() != Some(title);

        if !doc.has("id") {
            doc.set("id", &new_id());
        }
        doc.set("title", title);
        if !doc.has("date") {
            doc.set("date", &Local::now().format("%Y-%m-%d").to_string());
        }
        doc.set("updated", &now());
        doc.body = body.to_string();

        let mut target = file.clone();
        if title_changed {
            let slug = slugify(title);
            let stem = if slug.is_empty() { "untitled" } else { &slug };
            if file.file_stem().and_then(|s| s.to_str()) != Some(stem) {
                let dir = file.parent().unwrap_or(&self.root);
                target = unique_path(dir, stem, "md", Some(&file));
            }
        }

        write_atomic(&target, &doc.render())?;
        if target != file {
            fs::remove_file(&file)?;
        }
        Ok(self.load(&target)?.summary)
    }

    /// Set a boolean flag (`pinned`, `published`) without touching the body.
    pub fn set_flag(&self, path: &str, key: &str, value: bool) -> Result<NoteSummary> {
        if !matches!(key, "pinned" | "published") {
            return Err(Error::InvalidPath(format!("unknown flag {key}")));
        }
        let file = self.resolve_note(path)?;
        let mut doc = Document::parse(&read_file(&file, path)?);
        if !doc.has("id") {
            doc.set("id", &new_id());
        }
        doc.set(key, if value { "true" } else { "false" });
        write_atomic(&file, &doc.render())?;
        Ok(self.load(&file)?.summary)
    }

    /// Move a note into `.trash/` inside the notes folder. Nothing is ever
    /// deleted outright, so a mis-click is recoverable. The trash mirrors the
    /// folder layout (`Work/a.md` goes to `.trash/Work/a.md`), which is how
    /// `restore` knows where a note came from without editing the note.
    pub fn trash(&self, path: &str) -> Result<()> {
        let file = self.resolve_note(path)?;
        if !file.is_file() {
            return Err(Error::NotFound(path.to_string()));
        }
        let folder = file.parent().and_then(|d| d.strip_prefix(&self.root).ok()).unwrap_or(Path::new(""));
        let dest = self.trash_dir().join(folder);
        fs::create_dir_all(&dest)?;
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
        fs::rename(&file, unique_path(&dest, stem, "md", None))?;
        Ok(())
    }

    /// Notes in the trash, most recently edited first. Their `path` is
    /// relative to the trash and `folder` is where they would be restored.
    pub fn trashed(&self) -> Result<Vec<NoteSummary>> {
        let trash = self.trash_dir();
        let mut files = Vec::new();
        if trash.is_dir() {
            self.collect_notes(&trash, true, &mut files)?;
        }
        let mut notes: Vec<NoteSummary> = files
            .iter()
            .filter_map(|file| self.load_from(&trash, file).ok())
            .map(|note| note.summary)
            .collect();
        notes.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.path.cmp(&b.path)));
        Ok(notes)
    }

    pub fn read_trashed(&self, path: &str) -> Result<Note> {
        let trash = self.trash_dir();
        self.load_from(&trash, &self.resolve_note_in(&trash, path)?)
    }

    /// Put a trashed note back in the folder it came from, recreating the
    /// folder if it has since been deleted.
    pub fn restore(&self, path: &str) -> Result<NoteSummary> {
        let trash = self.trash_dir();
        let file = self.resolve_note_in(&trash, path)?;
        if !file.is_file() {
            return Err(Error::NotFound(path.to_string()));
        }
        let folder = file.parent().and_then(|d| d.strip_prefix(&trash).ok()).unwrap_or(Path::new(""));
        let dest = self.root.join(folder);
        fs::create_dir_all(&dest)?;
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
        let target = unique_path(&dest, stem, "md", None);
        fs::rename(&file, &target)?;
        if let Some(parent) = file.parent() {
            remove_empty_dirs(parent, &trash);
        }
        Ok(self.load(&target)?.summary)
    }

    /// Move a note to another folder (`""` is the top level).
    pub fn move_note(&self, path: &str, folder: &str) -> Result<NoteSummary> {
        let file = self.resolve_note(path)?;
        if !file.is_file() {
            return Err(Error::NotFound(path.to_string()));
        }
        let dest = self.resolve_dir(folder)?;
        if file.parent() == Some(dest.as_path()) {
            return Ok(self.load(&file)?.summary);
        }
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
        let target = unique_path(&dest, stem, "md", None);
        fs::rename(&file, &target)?;
        Ok(self.load(&target)?.summary)
    }

    /// Give a folder a new name in place. Returns its new path.
    pub fn rename_folder(&self, folder: &str, new_name: &str) -> Result<String> {
        let dir = self.resolve_folder(folder)?;
        let name = clean_relative(new_name.trim())?;
        if name.components().count() != 1 {
            return Err(Error::InvalidPath(format!("\"{new_name}\" is not a folder name")));
        }
        let target = dir.parent().unwrap_or(&self.root).join(&name);
        if target == dir {
            return Ok(folder.to_string());
        }
        if target.exists() {
            return Err(Error::AlreadyExists(name.display().to_string()));
        }
        fs::rename(&dir, &target)?;
        Ok(to_slash(target.strip_prefix(&self.root).unwrap_or(&target)))
    }

    /// Move every note in a folder (and its subfolders) to the trash, then
    /// remove the folder. Returns `false` if the folder had to stay because
    /// it still holds files that are not notes.
    pub fn delete_folder(&self, folder: &str) -> Result<bool> {
        let dir = self.resolve_folder(folder)?;
        let mut files = Vec::new();
        self.collect_notes(&dir, true, &mut files)?;
        for file in &files {
            self.trash(&to_slash(file.strip_prefix(&self.root).unwrap_or(file)))?;
        }
        remove_tree_if_empty(&dir);
        Ok(!dir.exists())
    }

    pub fn create_folder(&self, folder: &str) -> Result<String> {
        let rel = clean_relative(folder)?;
        if rel.as_os_str().is_empty() {
            return Err(Error::InvalidPath("folder name is empty".into()));
        }
        fs::create_dir_all(self.root.join(&rel))?;
        Ok(to_slash(&rel))
    }

    fn trash_dir(&self) -> PathBuf {
        self.root.join(TRASH_DIR)
    }

    fn load(&self, file: &Path) -> Result<Note> {
        self.load_from(&self.root, file)
    }

    /// Read a note, reporting its path relative to `base` (the notes root or
    /// the trash).
    fn load_from(&self, base: &Path, file: &Path) -> Result<Note> {
        let rel = file.strip_prefix(base).unwrap_or(file);
        let rel_str = to_slash(rel);
        let doc = Document::parse(&read_file(file, &rel_str)?);
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // Notes written outside the app often have no header at all. A
        // heading on their first line is the title, and is taken out of the
        // body so the editor does not show it twice. The file itself only
        // changes when the note is next saved, which writes a `title:` line.
        let (heading, body) = match split_title_heading(&doc.body) {
            Some((heading, rest)) if !doc.has("title") => (Some(heading), rest),
            _ => (None, doc.body.clone()),
        };
        let preview = snippet(&body);
        let title = doc.get("title").or(heading).unwrap_or_else(|| stem.to_string());
        let updated = doc
            .get("updated")
            .and_then(|u| DateTime::parse_from_rfc3339(&u).ok())
            .map(|u| u.with_timezone(&Utc))
            .or_else(|| fs::metadata(file).and_then(|m| m.modified()).ok().map(DateTime::<Utc>::from))
            .unwrap_or_else(Utc::now);
        let date = doc
            .get("date")
            .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
            // Hand-written notes may have no date; fall back to the last edit.
            .unwrap_or_else(|| updated.with_timezone(&Local).format("%Y-%m-%d").to_string());

        Ok(Note {
            summary: NoteSummary {
                folder: rel.parent().map(to_slash).unwrap_or_default(),
                path: rel_str,
                id: doc.get("id").filter(|id| !id.is_empty()),
                title,
                snippet: preview,
                date,
                updated: updated.to_rfc3339_opts(SecondsFormat::Secs, true),
                pinned: doc.get_bool("pinned"),
                published: doc.get_bool("published"),
            },
            body,
        })
    }

    fn resolve_dir(&self, folder: &str) -> Result<PathBuf> {
        let dir = self.root.join(clean_relative(folder)?);
        if !dir.is_dir() {
            return Err(Error::NotFound(folder.to_string()));
        }
        Ok(dir)
    }

    /// A real folder below the root; the root itself cannot be renamed or deleted.
    fn resolve_folder(&self, folder: &str) -> Result<PathBuf> {
        if clean_relative(folder)?.as_os_str().is_empty() {
            return Err(Error::InvalidPath("the top-level folder cannot be changed".into()));
        }
        self.resolve_dir(folder)
    }

    fn resolve_note(&self, path: &str) -> Result<PathBuf> {
        self.resolve_note_in(&self.root, path)
    }

    fn resolve_note_in(&self, base: &Path, path: &str) -> Result<PathBuf> {
        let rel = clean_relative(path)?;
        if rel.extension().and_then(|e| e.to_str()) != Some("md") {
            return Err(Error::InvalidPath(format!("{path} is not a .md file")));
        }
        Ok(base.join(rel))
    }

    fn walk_dirs(&self, dir: &Path, out: &mut Vec<String>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if is_hidden(&entry.file_name()) || !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path();
            out.push(to_slash(path.strip_prefix(&self.root).unwrap_or(&path)));
            self.walk_dirs(&path, out)?;
        }
        Ok(())
    }

    fn collect_notes(&self, dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if is_hidden(&entry.file_name()) {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if recursive {
                    self.collect_notes(&path, true, out)?;
                }
            } else if kind.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
        Ok(())
    }
}

/// Validate a path that came from the UI. It must stay inside the notes
/// folder: no absolute paths, no `..`, and no hidden components (which also
/// keeps `.trash` and `.git` out of reach).
fn clean_relative(path: &str) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for part in Path::new(path).components() {
        match part {
            Component::Normal(name) if !is_hidden(name) => out.push(name),
            Component::CurDir => {}
            _ => return Err(Error::InvalidPath(path.to_string())),
        }
    }
    Ok(out)
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

fn to_slash(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn read_file(file: &Path, shown: &str) -> Result<String> {
    match fs::read(file) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound(shown.to_string())),
        Err(e) => Err(e.into()),
    }
}

/// Write to a temp file and rename it into place, so a crash mid-save can
/// never leave a half-written note.
fn write_atomic(file: &Path, contents: &str) -> Result<()> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("note");
    let tmp = dir.join(format!(".{name}.tmp"));
    let mut handle = fs::File::create(&tmp)?;
    handle.write_all(contents.as_bytes())?;
    handle.sync_all()?;
    fs::rename(&tmp, file)?;
    Ok(())
}

/// `stem.ext`, or `stem-2.ext`, `stem-3.ext`… if taken. `own` is the file
/// being renamed, which does not count as a collision with itself.
fn unique_path(dir: &Path, stem: &str, ext: &str, own: Option<&Path>) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 { format!("{stem}.{ext}") } else { format!("{stem}-{n}.{ext}") };
        let candidate = dir.join(name);
        if !candidate.exists() || own == Some(candidate.as_path()) {
            return candidate;
        }
        n += 1;
    }
}

/// Remove `dir` and then each parent while they are empty, stopping at `stop`.
fn remove_empty_dirs(dir: &Path, stop: &Path) {
    let mut dir = dir;
    // `remove_dir` refuses to delete a directory that still has entries.
    while dir != stop && dir.starts_with(stop) && fs::remove_dir(dir).is_ok() {
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
}

/// Remove a directory tree, but only the parts with no files left in them.
fn remove_tree_if_empty(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                remove_tree_if_empty(&entry.path());
            }
        }
    }
    let _ = fs::remove_dir(dir);
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.chars().count() >= SLUG_CHARS {
            break;
        }
    }
    slug.trim_end_matches('-').to_string()
}

/// If the first non-blank line is a `# heading`, return its text and the
/// body without that line (and the blank line after it).
fn split_title_heading(body: &str) -> Option<(String, String)> {
    let start = body.len() - body.trim_start_matches(['\n', '\r']).len();
    let (line, rest) = body[start..].split_once('\n').unwrap_or((&body[start..], ""));
    let heading = line.trim_end_matches('\r').strip_prefix("# ")?.trim();
    if heading.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n')).unwrap_or(rest);
    Some((heading.to_string(), rest.to_string()))
}

/// A one-line plain-text preview of the body for the note list.
fn snippet(body: &str) -> String {
    let mut out = String::new();
    for line in body.lines() {
        let line = line
            .trim()
            .trim_start_matches(['#', '>', '-', '*', '+', ' '])
            .trim_start_matches("[ ]")
            .trim_start_matches("[x]")
            .trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.extend(line.chars().filter(|c| !matches!(c, '*' | '_' | '`')));
        if out.chars().count() >= SNIPPET_CHARS {
            break;
        }
    }
    out.chars().take(SNIPPET_CHARS).collect()
}

/// The text around the first place a search word appears in the body, so
/// the result list shows why the note matched.
fn match_context(body: &str, lowered: &str, terms: &[String]) -> Option<String> {
    const LEAD_CHARS: usize = 30;
    let at = terms.iter().filter_map(|t| lowered.find(t.as_str())).min()?;
    // Count in characters: lowercasing can change byte lengths.
    let start = lowered[..at].chars().count().saturating_sub(LEAD_CHARS);
    // Drop markdown punctuation first so it leaves no double spaces behind.
    let window: String = body
        .chars()
        .skip(start)
        .take(SNIPPET_CHARS * 2)
        .filter(|c| !matches!(c, '*' | '_' | '`' | '#' | '>'))
        .collect();
    let text: String = window.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(SNIPPET_CHARS).collect();
    Some(if start > 0 { format!("…{}", text.trim_start()) } else { text })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, FsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = FsStore::open(dir.path().join("Notes")).unwrap();
        (dir, store)
    }

    #[test]
    fn create_save_read_round_trip() {
        let (_dir, store) = store();
        let created = store.create("").unwrap();
        assert_eq!(created.path, "untitled.md");
        assert_eq!(created.title, "");
        assert!(created.id.is_some());

        let saved = store.save(&created.path, "Grocery List", "- [ ] eggs\n").unwrap();
        assert_eq!(saved.path, "grocery-list.md");
        assert_eq!(saved.id, created.id, "id survives a rename");
        assert_eq!(saved.snippet, "eggs");
        assert!(!store.root().join("untitled.md").exists());

        let note = store.read("grocery-list.md").unwrap();
        assert_eq!(note.summary.title, "Grocery List");
        assert_eq!(note.body, "- [ ] eggs\n");
    }

    #[test]
    fn saving_the_body_alone_keeps_the_filename() {
        let (_dir, store) = store();
        fs::write(store.root().join("my-own-name.md"), "---\ntitle: Something Else\n---\n\nold").unwrap();
        let saved = store.save("my-own-name.md", "Something Else", "new").unwrap();
        assert_eq!(saved.path, "my-own-name.md");
    }

    #[test]
    fn renames_do_not_overwrite_another_note() {
        let (_dir, store) = store();
        let a = store.create("").unwrap();
        store.save(&a.path, "Ideas", "first").unwrap();
        let b = store.create("").unwrap();
        let b = store.save(&b.path, "Ideas", "second").unwrap();
        assert_eq!(b.path, "ideas-2.md");
        assert_eq!(store.read("ideas.md").unwrap().body, "first");
    }

    #[test]
    fn hand_written_notes_are_listed_and_keep_unknown_fields() {
        let (_dir, store) = store();
        fs::write(store.root().join("plain.md"), "# From Vim\n\nhello").unwrap();
        fs::write(
            store.root().join("tagged.md"),
            "---\ntitle: Tagged\ntags:\n  - a\n---\n\nbody",
        )
        .unwrap();

        let notes = store.notes(None).unwrap();
        let plain = notes.iter().find(|n| n.path == "plain.md").unwrap();
        assert_eq!(plain.title, "From Vim");
        assert_eq!(plain.snippet, "hello");
        assert_eq!(store.read("plain.md").unwrap().body, "hello", "the title heading is not repeated in the body");

        // Saving turns the heading into a `title:` line instead of keeping both.
        store.save("plain.md", "From Vim", "hello again").unwrap();
        let text = fs::read_to_string(store.root().join("plain.md")).unwrap();
        assert!(text.contains("title: From Vim\n") && !text.contains("# From Vim"), "{text}");
        assert!(store.root().join("plain.md").exists(), "an unchanged title does not rename the file");

        // A heading further down is content, not the title.
        fs::write(store.root().join("late.md"), "intro\n\n# Section\n").unwrap();
        let late = store.read("late.md").unwrap();
        assert_eq!((late.summary.title.as_str(), late.body.as_str()), ("late", "intro\n\n# Section\n"));
        assert_eq!(plain.id, None);
        assert_eq!(plain.date.len(), 10, "falls back to the edit date: {}", plain.date);

        store.save("tagged.md", "Tagged", "body 2").unwrap();
        let text = fs::read_to_string(store.root().join("tagged.md")).unwrap();
        assert!(text.contains("tags:\n  - a\n"), "{text}");
        assert!(text.contains("\nid: "), "{text}");
    }

    #[test]
    fn the_date_header_is_kept_and_a_bad_one_is_ignored() {
        let (_dir, store) = store();
        let header = "updated: 2025-06-01T12:00:00Z";
        fs::write(store.root().join("dated.md"), format!("---\ndate: 2024-04-02\n{header}\n---\n\n")).unwrap();
        fs::write(store.root().join("odd.md"), format!("---\ndate: last spring\n{header}\n---\n\n")).unwrap();
        assert_eq!(store.read("dated.md").unwrap().summary.date, "2024-04-02");
        assert!(store.read("odd.md").unwrap().summary.date.starts_with("2025-0"));
    }

    #[test]
    fn search_matches_every_word_in_title_or_body_across_folders() {
        let (_dir, store) = store();
        store.create_folder("Work").unwrap();
        let write = |path: &str, title: &str, body: &str| {
            let text = format!("---\ntitle: {title}\nupdated: 2025-01-01T00:00:00Z\n---\n\n{body}");
            fs::write(store.root().join(path), text).unwrap();
        };
        write("soup.md", "Dinner", &format!("{}\n\n> Simmer the **Lentil** soup slowly.", "Filler words. ".repeat(20)));
        write("Work/lentils.md", "Lentil prices", "Quarterly report.");
        write("other.md", "Unrelated", "Nothing to see.");
        fs::create_dir_all(store.root().join(".trash")).unwrap();
        fs::write(store.root().join(".trash/gone.md"), "lentil").unwrap();

        let titles = |q: &str| store.search(q).unwrap().into_iter().map(|n| n.title).collect::<Vec<_>>();
        assert_eq!(titles("LENTIL"), ["Lentil prices", "Dinner"], "title matches first, trash excluded");
        assert_eq!(titles("soup lentil"), ["Dinner"]);
        assert_eq!(titles("lentil report"), ["Lentil prices"], "words may be split between title and body");
        assert!(titles("lentil unicorn").is_empty());
        assert!(titles("   ").is_empty());

        let dinner = &store.search("soup").unwrap()[0];
        assert!(dinner.snippet.starts_with('…'), "{}", dinner.snippet);
        assert!(dinner.snippet.contains("ds. Simmer the Lentil soup"), "{}", dinner.snippet);
    }

    #[test]
    fn folders_scope_the_note_list() {
        let (_dir, store) = store();
        assert_eq!(store.create_folder("Work/Clients").unwrap(), "Work/Clients");
        store.create("").unwrap();
        store.create("Work").unwrap();
        store.create("Work/Clients").unwrap();

        assert_eq!(store.folders().unwrap(), ["Work", "Work/Clients"]);
        assert_eq!(store.notes(None).unwrap().len(), 3);
        assert_eq!(store.notes(Some("")).unwrap().len(), 1);
        let work = store.notes(Some("Work")).unwrap();
        assert_eq!(work.len(), 1);
        assert_eq!(work[0].folder, "Work");
    }

    #[test]
    fn pinned_notes_sort_first() {
        let (_dir, store) = store();
        for (name, updated) in [("old", "2024-01-01T00:00:00Z"), ("new", "2025-01-01T00:00:00Z")] {
            fs::write(
                store.root().join(format!("{name}.md")),
                format!("---\ntitle: {name}\nupdated: {updated}\n---\n\n"),
            )
            .unwrap();
        }
        let order = |s: &FsStore| s.notes(None).unwrap().into_iter().map(|n| n.title).collect::<Vec<_>>();
        assert_eq!(order(&store), ["new", "old"]);
        let pinned = store.set_flag("old.md", "pinned", true).unwrap();
        assert!(pinned.pinned);
        assert_eq!(pinned.updated, "2024-01-01T00:00:00Z", "pinning is not an edit");
        assert_eq!(order(&store), ["old", "new"]);
    }

    #[test]
    fn trash_moves_the_file_and_hides_it() {
        let (_dir, store) = store();
        let note = store.create("").unwrap();
        store.trash(&note.path).unwrap();
        assert!(store.notes(None).unwrap().is_empty());
        assert!(store.folders().unwrap().is_empty());
        assert!(store.root().join(".trash/untitled.md").exists());
        assert!(matches!(store.trash(&note.path), Err(Error::NotFound(_))));
    }

    #[test]
    fn a_trashed_note_can_be_read_and_restored_to_its_folder() {
        let (_dir, store) = store();
        store.create_folder("Work/Clients").unwrap();
        let note = store.create("Work/Clients").unwrap();
        let note = store.save(&note.path, "Acme", "terms").unwrap();
        store.trash(&note.path).unwrap();

        let trashed = store.trashed().unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!((trashed[0].path.as_str(), trashed[0].folder.as_str()), ("Work/Clients/acme.md", "Work/Clients"));
        assert_eq!(store.read_trashed(&trashed[0].path).unwrap().body, "terms");

        // The original folder is gone by the time the note is restored.
        assert!(store.delete_folder("Work").unwrap());
        let restored = store.restore(&trashed[0].path).unwrap();
        assert_eq!(restored.path, "Work/Clients/acme.md");
        assert_eq!(restored.id, note.id);
        assert!(store.trashed().unwrap().is_empty());
        assert!(!store.root().join(".trash/Work").exists(), "empty trash folders are cleaned up");
        assert!(matches!(store.restore("../acme.md"), Err(Error::InvalidPath(_))));
    }

    #[test]
    fn moving_a_note_keeps_its_id_and_never_overwrites() {
        let (_dir, store) = store();
        store.create_folder("Work").unwrap();
        let a = store.create("").unwrap();
        let a = store.save(&a.path, "Ideas", "mine").unwrap();
        let b = store.create("Work").unwrap();
        store.save(&b.path, "Ideas", "theirs").unwrap();

        let moved = store.move_note(&a.path, "Work").unwrap();
        assert_eq!(moved.path, "Work/ideas-2.md");
        assert_eq!(moved.id, a.id);
        assert_eq!(store.read("Work/ideas.md").unwrap().body, "theirs");
        assert_eq!(store.move_note(&moved.path, "Work").unwrap().path, moved.path, "moving to the same folder is a no-op");
        assert_eq!(store.move_note(&moved.path, "").unwrap().path, "ideas-2.md");
        assert!(matches!(store.move_note("ideas-2.md", "Missing"), Err(Error::NotFound(_))));
    }

    #[test]
    fn folders_can_be_renamed_but_not_onto_each_other() {
        let (_dir, store) = store();
        store.create_folder("Work/Clients").unwrap();
        store.create_folder("Home").unwrap();
        store.create("Work/Clients").unwrap();

        assert_eq!(store.rename_folder("Work/Clients", "Customers").unwrap(), "Work/Customers");
        assert_eq!(store.notes(Some("Work/Customers")).unwrap().len(), 1);
        assert!(matches!(store.rename_folder("Work", "Home"), Err(Error::AlreadyExists(_))));
        assert!(matches!(store.rename_folder("Work", "a/b"), Err(Error::InvalidPath(_))));
        assert!(matches!(store.rename_folder("Work", "../x"), Err(Error::InvalidPath(_))));
        assert!(matches!(store.rename_folder("", "x"), Err(Error::InvalidPath(_))));
    }

    #[test]
    fn deleting_a_folder_trashes_its_notes_and_spares_other_files() {
        let (_dir, store) = store();
        store.create_folder("Work/Clients").unwrap();
        store.create_folder("Photos").unwrap();
        store.create("Work").unwrap();
        store.create("Work/Clients").unwrap();
        let keep = store.create("").unwrap();
        fs::write(store.root().join("Photos/cat.jpg"), "x").unwrap();
        store.create("Photos").unwrap();

        assert!(store.delete_folder("Work").unwrap());
        assert!(!store.delete_folder("Photos").unwrap(), "the photo is not a note, so the folder stays");
        assert!(store.root().join("Photos/cat.jpg").exists());
        assert_eq!(store.folders().unwrap(), ["Photos"]);
        assert_eq!(store.notes(None).unwrap().into_iter().map(|n| n.path).collect::<Vec<_>>(), [keep.path]);
        assert_eq!(store.trashed().unwrap().len(), 3);
        assert!(matches!(store.delete_folder(""), Err(Error::InvalidPath(_))));
    }

    #[test]
    fn paths_cannot_escape_the_notes_folder() {
        let (dir, store) = store();
        fs::write(dir.path().join("secret.md"), "x").unwrap();
        for bad in ["../secret.md", "/etc/passwd.md", "a/../../secret.md", ".trash/x.md", ".git/config"] {
            assert!(matches!(store.read(bad), Err(Error::InvalidPath(_))), "{bad}");
            assert!(matches!(store.save(bad, "t", "b"), Err(Error::InvalidPath(_))), "{bad}");
        }
        assert!(matches!(store.read("notes.txt"), Err(Error::InvalidPath(_))));
        assert!(matches!(store.create_folder("../out"), Err(Error::InvalidPath(_))));
    }

    #[test]
    fn slugs_are_filesystem_friendly() {
        assert_eq!(slugify("  Plan: Phase #1!  "), "plan-phase-1");
        assert_eq!(slugify("Café déjà vu"), "café-déjà-vu");
        assert_eq!(slugify("///"), "");
    }
}
