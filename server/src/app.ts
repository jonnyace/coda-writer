import { Hono } from 'hono'
import { SCHEMA, type Db } from './db'
import { STYLES, feed, indexPage, notFoundPage, postPage, type Post, type Site } from './pages'
import { slugify } from './render'

export interface Options {
  db: Db
  /** Secret the desktop app sends to publish. Publishing is off without it. */
  token: string | undefined
  site: Site
}

const SITE_ID = 'main'
const MAX_BODY_BYTES = 1_000_000
const FEED_POSTS = 20
// Paths the server uses itself; a post may not take them.
const RESERVED_SLUGS = new Set(['api', 'feed.xml', 'styles.css', 'favicon.ico', 'robots.txt'])
const PAGE_CACHE = 'public, max-age=60'

const ID_PATTERN = /^[0-9a-zA-Z-]{8,64}$/
const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/

async function sha256(text: string): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text)))
}

/** Compare digests so the time taken does not reveal how much of the token matched. */
async function sameSecret(a: string, b: string): Promise<boolean> {
  const [x, y] = await Promise.all([sha256(a), sha256(b)])
  let difference = 0
  for (let i = 0; i < x.length; i++) difference |= x[i]! ^ y[i]!
  return difference === 0
}

interface PostInput {
  slug: string
  title: string
  body: string
  date: string
  updated: string
  access: 'public' | 'subscribers'
}

function parsePost(input: unknown): PostInput | string {
  if (typeof input !== 'object' || input === null) return 'body must be a JSON object'
  const { slug, title, body, date, updated, access = 'public' } = input as Record<string, unknown>
  if (typeof title !== 'string' || title.length > 300) return 'title must be text of at most 300 characters'
  if (typeof body !== 'string' || body.length > MAX_BODY_BYTES) return 'body must be text of at most 1 MB'
  if (typeof date !== 'string' || !DATE_PATTERN.test(date)) return 'date must look like 2026-01-31'
  if (typeof updated !== 'string' || Number.isNaN(Date.parse(updated))) return 'updated must be a timestamp'
  if (access !== 'public' && access !== 'subscribers') return 'access must be public or subscribers'
  if (slug !== undefined && typeof slug !== 'string') return 'slug must be text'
  return { slug: slugify(slug || title), title: title.trim(), body, date, updated, access }
}

export function createApp({ db, token, site }: Options) {
  const app = new Hono()

  // Behind a proxy or `tailscale serve` the request address is internal, so a
  // configured public address wins when building links.
  const originOf = (requestUrl: string) => site.url?.replace(/\/+$/, '') || new URL(requestUrl).origin

  // Tables are created on first use, so a fresh deployment needs no setup step.
  let ready: Promise<void> | undefined
  app.use(async (_c, next) => {
    ready ??= db.exec(SCHEMA)
    try {
      await ready
    } catch (error) {
      ready = undefined
      throw error
    }
    await next()
  })

  // Subscriber-only posts stay hidden until reader sign-in exists.
  const publicPosts = (limit: number) =>
    db.all<Post>(
      `SELECT * FROM posts WHERE site_id = ? AND access = 'public' ORDER BY date DESC, updated DESC LIMIT ?`,
      [SITE_ID, limit],
    )

  app.use('/api/*', async (c, next) => {
    if (!token) return c.json({ error: 'Publishing is off: set CODA_TOKEN on the server.' }, 503)
    const sent = c.req.header('Authorization')?.replace(/^Bearer /, '') ?? ''
    if (!(await sameSecret(sent, token))) return c.json({ error: 'Wrong or missing token.' }, 401)
    await next()
  })

  app.get('/api/posts', async (c) => {
    const posts = await db.all<Omit<Post, 'body'>>(
      `SELECT id, slug, title, date, updated, access FROM posts WHERE site_id = ? ORDER BY date DESC`,
      [SITE_ID],
    )
    return c.json({ posts })
  })

  app.put('/api/posts/:id', async (c) => {
    const id = c.req.param('id')
    if (!ID_PATTERN.test(id)) return c.json({ error: 'invalid id' }, 400)
    const post = parsePost(await c.req.json().catch(() => null))
    if (typeof post === 'string') return c.json({ error: post }, 400)

    const [existing] = await db.all<{ slug: string }>(`SELECT slug FROM posts WHERE site_id = ? AND id = ?`, [SITE_ID, id])
    // A published address never changes, even if the note is renamed later.
    let slug = existing?.slug
    if (!slug) {
      slug = post.slug || id
      const [taken] = await db.all(`SELECT 1 FROM posts WHERE site_id = ? AND slug = ?`, [SITE_ID, slug])
      if (taken || RESERVED_SLUGS.has(slug)) slug = `${slug}-${id.slice(-8)}`
    }

    await db.run(
      `INSERT INTO posts (site_id, id, slug, title, body, date, updated, access) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT (site_id, id) DO UPDATE SET
         title = excluded.title, body = excluded.body, date = excluded.date,
         updated = excluded.updated, access = excluded.access`,
      [SITE_ID, id, slug, post.title, post.body, post.date, post.updated, post.access],
    )
    return c.json({ id, slug, url: `${originOf(c.req.url)}/${encodeURI(slug)}` })
  })

  app.delete('/api/posts/:id', async (c) => {
    await db.run(`DELETE FROM posts WHERE site_id = ? AND id = ?`, [SITE_ID, c.req.param('id')])
    return c.body(null, 204)
  })

  app.get('/styles.css', (c) => c.body(STYLES, 200, { 'Content-Type': 'text/css; charset=utf-8', 'Cache-Control': PAGE_CACHE }))

  app.get('/', async (c) => c.html(indexPage(site, await publicPosts(1000)), 200, { 'Cache-Control': PAGE_CACHE }))

  app.get('/feed.xml', async (c) =>
    c.body(feed(site, originOf(c.req.url), await publicPosts(FEED_POSTS)), 200, {
      'Content-Type': 'application/rss+xml; charset=utf-8',
      'Cache-Control': PAGE_CACHE,
    }),
  )

  app.get('/:slug', async (c) => {
    const [post] = await db.all<Post>(`SELECT * FROM posts WHERE site_id = ? AND slug = ? AND access = 'public'`, [
      SITE_ID,
      c.req.param('slug'),
    ])
    if (!post) return c.html(notFoundPage(site), 404)
    return c.html(postPage(site, post), 200, { 'Cache-Control': PAGE_CACHE })
  })

  app.notFound((c) => c.html(notFoundPage(site), 404))
  app.onError((error, c) => {
    console.error(error)
    return c.json({ error: 'Server error.' }, 500)
  })

  return app
}
