# Coda Writer

A notes app in the spirit of Apple Notes, built for [Omarchy](https://omarchy.org) (Linux first, macOS later). Every note is a plain markdown file in a folder you own. Notes you flag can be published to the blog in `site/`.

```
app/          Desktop app: Svelte UI in src/, Tauri shell in src-tauri/
crates/core/  Note storage (markdown files, folders, trash). No GUI code.
site/         The blog: build.sh turns site/md/*.md into static HTML
docs/         Plan and design notes
```

See [docs/plan.md](docs/plan.md) for the roadmap and [site/README.md](site/README.md) for the blog.

## Run the app

On Arch / Omarchy:

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel rust nodejs npm
cd app
npm install
npm run tauri dev      # develop
npm run tauri build    # release binary in ../target/release/coda-writer
```

If the window is blank on an NVIDIA GPU, start it with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Where notes live

Notes are read from `~/Notes`. To use another folder, create `~/.config/coda-writer/config.toml`:

```toml
notes_dir = "~/Documents/notes"
```

Each note is a `.md` file with a short header:

```markdown
---
id: 0193a1c9-2c99-70b6-bfdc-917a872e6e2e
title: Groceries
date: 2026-11-08
updated: 2026-11-08T15:00:00Z
pinned: true
---

- [ ] eggs
```

- The file is named after the title and renamed when the title changes. The `id` never changes.
- Subfolders are folders in the app. Hidden files and folders are ignored.
- Deleting a note moves it to `.trash/` inside the notes folder.
- You can edit the files in any other editor while the app is open; it picks up the changes.
- Header lines the app does not know (tags, aliases, anything else) are left exactly as they are.

## Test

```bash
cargo test -p coda-core        # storage tests, no GUI libraries needed
cd app && npm run check        # type-check the UI
```
