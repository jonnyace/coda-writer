# Coda Writer server

Turns the notes you publish from the desktop app into a blog with an RSS feed. One codebase, two ways to run it:

| | Runs on | Data lives in |
|---|---|---|
| Cloudflare (default) | Workers | D1 |
| Self-hosted | Node 22.5+ or Docker | a SQLite file |

Only notes you publish are sent here. Everything else stays on your machine.

## Settings

| Name | Purpose |
|---|---|
| `CODA_TOKEN` | Secret the desktop app uses to publish. Without it the blog is read-only. |
| `SITE_TITLE` | Name shown at the top of the blog. Default `Notes`. |
| `SITE_DESCRIPTION` | Optional line under the title. |
| `SITE_URL` | Public address, e.g. `https://blog.example.com`. Set it when the server sits behind a proxy. |
| `PORT`, `HOST`, `CODA_DATA_DIR` | Node only. Defaults `8787`, `0.0.0.0`, `./data`. |

Pick a long random token, for example `openssl rand -hex 32`.

## Deploy to Cloudflare

```bash
cd server
npm install
npx wrangler deploy                 # creates the D1 database on first run
npx wrangler secret put CODA_TOKEN
```

Change `SITE_TITLE` and friends in `wrangler.jsonc`. Add a custom domain in the Cloudflare dashboard.

## Self-host with Docker

```bash
docker build -t coda-writer-server server
docker run -d --name coda-writer -p 8787:8787 \
  -e CODA_TOKEN=... -e SITE_TITLE="My Notes" \
  -v coda-writer-data:/data coda-writer-server
```

### On a tailnet

Run the container on any machine in your tailnet, bound to localhost (`-p 127.0.0.1:8787:8787`), then:

```bash
tailscale serve --bg 8787      # private: only your devices can read and publish
tailscale funnel --bg 8787     # public: anyone can read; publishing still needs the token
```

Set `SITE_URL` to the `https://<machine>.<tailnet>.ts.net` address Tailscale prints.

## Connect the desktop app

Add to `~/.config/coda-writer/config.toml`:

```toml
[server]
url = "https://your-blog.example.com"
token = "the CODA_TOKEN value"
```

## API

All `/api` routes need `Authorization: Bearer <CODA_TOKEN>`.

| Route | Does |
|---|---|
| `PUT /api/posts/:id` | Publish or update. JSON: `title`, `body` (markdown), `date` (`YYYY-MM-DD`), `updated` (timestamp), optional `slug`, optional `access` (`public` or `subscribers`). Returns the post's `url`. |
| `DELETE /api/posts/:id` | Unpublish. |
| `GET /api/posts` | List published posts. |

A post's address is fixed when it is first published and does not change if the note is renamed. Posts marked `subscribers` are stored but not shown until reader subscriptions are built.

## Develop

```bash
npm test         # runs the app against an in-memory SQLite database
npm run check    # type-check
```
