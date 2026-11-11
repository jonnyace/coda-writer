# Plan

Coda Writer is a local-first notes app. The notes folder on disk is the source of truth; everything else (search index, sync, the published blog) is derived from it.

## Decisions

- **One repo.** The app, the storage crate and the blog live together so the blog renderer and the app preview can share code.
- **Publishing is opt-in.** Notes are private. Only notes flagged `published: true` go to the blog.
- **Multi-user is the goal.** The hosted platform sketched in [idea.md](idea.md) is still where this is heading. The desktop app stays local-first and the hosted service syncs the folder; it does not replace it.
- **Stack.** Tauri 2 (Rust + system web view), Svelte, Tiptap with markdown output.

## What multi-user requires of the file format

These are in place from phase 1 so no migration is needed later:

- Every note the app writes has a UUIDv7 `id`. Paths change on rename and move; sync keys on the id.
- Every save stamps `updated` (UTC). Sync uses it to order edits and detect conflicts.
- Unknown header fields round-trip untouched, so the server can add fields (owner, blog, revision) without older clients destroying them.
- Deletes go to `.trash/`, which gives sync a tombstone to propagate.

Still open: which backend hosts sync and publishing (Supabase as in idea.md, or Cloudflare D1 + R2), and how conflicts are shown to the user.

## Phases

1. **Skeleton** (done). Three panes, folders, autosave, pin, trash, live reload of outside edits.
2. **Apple Notes basics.** Full-text search (SQLite FTS5 as a rebuildable cache), move notes between folders, rename and delete folders, a view of the trash with restore, images pasted into `_attachments/`, tags, a command palette.
3. **Omarchy polish.** Follow the active Omarchy theme and re-theme when it changes, quick-capture window with a Hyprland rule and keybinding, AUR package and `.desktop` entry.
4. **Publish.** "Publish to blog" sets `published: true` and copies the note into `site/md/`. Replace the awk converter in `site/build.sh` with a Rust renderer (comrak) shared with the app.
5. **Sync and accounts.** Git autosync of the notes folder first, then the hosted multi-user service.
6. **macOS.** Native menu bar, signed and notarized `.dmg`, global capture hotkey.

## Known gaps after phase 1

- The notes folder can only be changed in the config file, not in the app.
- Notes are sorted by the `updated` header, so a note edited only outside the app keeps its old position until the app saves it.
- Not yet run on a real Omarchy desktop or on macOS; verified by compiling and driving the app in an Arch Linux container.
