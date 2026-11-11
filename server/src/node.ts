// Node entry point: the same app, stored in a SQLite file. Use this to
// self-host in Docker, on a VPS, or on a machine in your tailnet.
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import { serve } from '@hono/node-server'
import { createApp } from './app'
import { sqlite } from './db-sqlite'

const dataDir = process.env.CODA_DATA_DIR || './data'
const port = Number(process.env.PORT || 8787)
const hostname = process.env.HOST || '0.0.0.0'

mkdirSync(dataDir, { recursive: true })

const app = createApp({
  db: sqlite(join(dataDir, 'coda-writer.db')),
  token: process.env.CODA_TOKEN,
  site: { title: process.env.SITE_TITLE || 'Notes', description: process.env.SITE_DESCRIPTION ?? '', url: process.env.SITE_URL || undefined },
})

if (!process.env.CODA_TOKEN) console.warn('CODA_TOKEN is not set: the blog is served but nothing can be published.')

serve({ fetch: app.fetch, port, hostname }, () => console.log(`coda-writer server on http://${hostname}:${port}`))
