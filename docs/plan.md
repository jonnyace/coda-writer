# Plan

Coda Writer is a local-first notes app. The notes folder on disk is the source of truth; everything else (search index, sync, the published blog) is derived from it.

## Decisions

- **One repo.** The app, the storage crate and the blog live together so the blog renderer and the app preview can share code.
- **Publishing is opt-in.** Notes are private. Only notes flagged `published: true` go to the blog.
- **Multi-user is the goal.** One server deployment should be able to host many writers. The desktop app stays local-first: the server receives what you publish and, later, syncs your folder. It never replaces the folder.
- **Deploy anywhere, Cloudflare by default.** The server in `server/` is one TypeScript codebase (Hono) that needs only a SQLite-style database. It runs as a Cloudflare Worker with D1, or under Node with a SQLite file (a Docker image, a VPS, a machine on your tailnet). This replaces the Supabase design in [idea.md](idea.md).
- **Private notes stay on your machine.** Until sync exists, the server only ever receives notes flagged `published`.
- **Writers can sell subscriptions.** A post can be marked for subscribers only. See "Subscriptions" below.
- **Stack.** Desktop: Tauri 2 (Rust + system web view), Svelte, Tiptap with markdown output. Server: Hono, D1 or SQLite, markdown-it.

## Deployment targets

| Target | Database | Notes |
|---|---|---|
| Cloudflare (default) | Worker + D1 | Deploys into the writer's own account. Base for the hosted multi-user product. |
| Self-hosted | Node + SQLite file | Docker image. On a tailnet, `tailscale serve` keeps it private and can supply the user's identity; `tailscale funnel` can expose only the blog. |
| No server | none | Notes folder synced with git or Syncthing; blog exported as static files from `site/`. |

The app only needs a server address and a token, so it does not care which target is behind it.

## Subscriptions

Readers subscribe to a writer's blog. Posts with `access: subscribers` show a preview to everyone else. The `access` column already exists on the server so no migration is needed.

- **Self-hosted or own Cloudflare account:** the writer adds their own Stripe keys. Stripe Checkout takes the payment, the customer portal handles cancellation, and a webhook records who is subscribed. Money goes straight to the writer.
- **Hosted multi-user service:** Stripe Connect, so the platform can onboard writers and take a fee. The exact Connect setup is decided when that phase starts.
- Readers sign in with an emailed link, so the server needs an email sender (Resend or Cloudflare Email Service) before this ships.

## What multi-user requires of the file format

These are in place from phase 1 so no migration is needed later:

- Every note the app writes has a UUIDv7 `id`. Paths change on rename and move; sync keys on the id.
- Every save stamps `updated` (UTC). Sync uses it to order edits and detect conflicts.
- Unknown header fields round-trip untouched, so the server can add fields (owner, blog, revision) without older clients destroying them.
- Deletes go to `.trash/`, which gives sync a tombstone to propagate.

Still open: how sync conflicts are shown to the user, and how one deployment routes many writers (subdomains or paths).

## Phases

Built in this order:

1. **Skeleton** (done). Three panes, folders, autosave, pin, trash, live reload of outside edits.
2. **Server and publish** (done). The deploy-anywhere server renders published notes as a blog with an RSS feed. The app gets a Publish button that pushes the note to the server and keeps it updated on save.
3. **Apple Notes basics** (in progress). Done: search across all notes, move notes between folders, rename and delete folders, a trash view with restore. Search reads the files directly rather than keeping a SQLite index; add an index only if a real folder proves slow. Still to do: images pasted into `_attachments/`, tags, a command palette, emptying the trash from the app.
4. **Omarchy polish.** Follow the active Omarchy theme and re-theme when it changes, quick-capture window with a Hyprland rule and keybinding, AUR package and `.desktop` entry.
5. **Subscriptions.** Reader sign-in, Stripe Checkout with the writer's own keys, subscriber-only posts.
6. **Sync and accounts.** Sync the whole notes folder through the server, writer accounts, many writers per deployment, Stripe Connect for the hosted service.
7. **macOS.** Native menu bar, signed and notarized `.dmg`, global capture hotkey.

## Known gaps

- The notes folder can only be changed in the config file, not in the app.
- Notes are sorted by the `updated` header, so a note edited only outside the app keeps its old position until the app saves it.
- The server token sits in plain text in `config.toml`; it should move to the system keyring.
- If the blog is unreachable when a published note is saved, the update is retried only on the next save.
- The published address is shown as text; it does not open in the browser yet.
- The server has not been deployed to a real Cloudflare account, only run against wrangler's local D1.
- Not yet run on a real Omarchy desktop or on macOS; verified by compiling and driving the app in an Arch Linux container.
