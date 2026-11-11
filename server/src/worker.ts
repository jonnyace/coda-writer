// Cloudflare Workers entry point: the same app, stored in D1.
import { createApp } from './app'
import { d1, type D1Like } from './db-d1'

interface Env {
  DB: D1Like
  CODA_TOKEN?: string
  SITE_TITLE?: string
  SITE_DESCRIPTION?: string
  SITE_URL?: string
}

let app: ReturnType<typeof createApp> | undefined

export default {
  fetch(request: Request, env: Env): Response | Promise<Response> {
    app ??= createApp({
      db: d1(env.DB),
      token: env.CODA_TOKEN,
      site: { title: env.SITE_TITLE || 'Notes', description: env.SITE_DESCRIPTION ?? '', url: env.SITE_URL || undefined },
    })
    return app.fetch(request)
  },
}
