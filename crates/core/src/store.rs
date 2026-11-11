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
        let title_changed = doc.get("title").as_deref() != Some(title);

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
    /// deleted outright, so a mis-click is recoverable from a file manager.
    pub fn trash(&self, path: &str) -> Result<()> {
        let file = self.resolve_note(path)?;
        if !file.is_file() {
            return Err(Error::NotFound(path.to_string()));
        }
        let trash = self.root.join(TRASH_DIR);
        fs::create_dir_all(&trash)?;
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
        fs::rename(&file, unique_path(&trash, stem, "md", None))?;
        Ok(())
    }

    pub fn create_folder(&self, folder: &str) -> Result<String> {
        let rel = clean_relative(folder)?;
        if rel.as_os_str().is_empty() {
            return Err(Error::InvalidPath("folder name is empty".into()));
        }
        fs::create_dir_all(self.root.join(&rel))?;
        Ok(to_slash(&rel))
    }

    fn load(&self, file: &Path) -> Result<Note> {
        let rel = file.strip_prefix(&self.root).unwrap_or(file);
        let rel_str = to_slash(rel);
        let doc = Document::parse(&read_file(file, &rel_str)?);
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let title = match doc.get("title") {
            Some(title) => title,
            // Notes written outside the app often have no header at all.
            None => first_heading(&doc.body).unwrap_or_else(|| stem.to_string()),
        };
        let updated = doc
            .get("updated")
            .and_then(|u| DateTime::parse_from_rfc3339(&u).ok())
            .map(|u| u.with_timezone(&Utc))
            .or_else(|| fs::metadata(file).and_then(|m| m.modified()).ok().map(DateTime::<Utc>::from))
            .unwrap_or_else(Utc::now);

        Ok(Note {
            summary: NoteSummary {
                folder: rel.parent().map(to_slash).unwrap_or_default(),
                path: rel_str,
                id: doc.get("id").filter(|id| !id.is_empty()),
                title,
                snippet: snippet(&doc.body),
                updated: updated.to_rfc3339_opts(SecondsFormat::Secs, true),
                pinned: doc.get_bool("pinned"),
                published: doc.get_bool("published"),
            },
            body: doc.body,
        })
    }

    fn resolve_dir(&self, folder: &str) -> Result<PathBuf> {
        let dir = self.root.join(clean_relative(folder)?);
        if !dir.is_dir() {
            return Err(Error::NotFound(folder.to_string()));
        }
        Ok(dir)
    }

    fn resolve_note(&self, path: &str) -> Result<PathBuf> {
        let rel = clean_relative(path)?;
        if rel.extension().and_then(|e| e.to_str()) != Some("md") {
            return Err(Error::InvalidPath(format!("{path} is not a .md file")));
        }
        Ok(self.root.join(rel))
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

fn first_heading(body: &str) -> Option<String> {
    body.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
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
        assert_eq!(plain.id, None);

        store.save("tagged.md", "Tagged", "body 2").unwrap();
        let text = fs::read_to_string(store.root().join("tagged.md")).unwrap();
        assert!(text.contains("tags:\n  - a\n"), "{text}");
        assert!(text.contains("\nid: "), "{text}");
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
