import { beforeEach, describe, expect, it } from 'vitest'
import { createApp } from '../src/app'
import { sqlite } from '../src/db-sqlite'

const TOKEN = 'test-token'
const ID = '0193a1c9-2c99-70b6-bfdc-917a872e6e2e'
const OTHER_ID = '0193a1c9-2c99-70b6-bfdc-aaaaaaaaaaaa'

let app: ReturnType<typeof createApp>

const post = (overrides: Record<string, unknown> = {}) => ({
  slug: 'hello-world',
  title: 'Hello World',
  body: 'First **post**.\n\n- [ ] todo\n- [x] done\n',
  date: '2026-11-08',
  updated: '2026-11-08T15:00:00Z',
  ...overrides,
})

const publish = (id: string, body: unknown, token = TOKEN) =>
  app.request(`/api/posts/${id}`, {
    method: 'PUT',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })

const page = async (path: string) => {
  const response = await app.request(path)
  return { status: response.status, text: await response.text() }
}

beforeEach(() => {
  app = createApp({ db: sqlite(':memory:'), token: TOKEN, site: { title: 'Test Blog', description: '' } })
})

describe('publishing', () => {
  it('shows a published post on the index, its own page and the feed', async () => {
    const response = await publish(ID, post())
    expect(response.status).toBe(200)
    expect(await response.json()).toMatchObject({ slug: 'hello-world', url: 'http://localhost/hello-world' })

    expect((await page('/')).text).toContain('<a href="/hello-world">Hello World</a>')
    const shown = await page('/hello-world')
    expect(shown.status).toBe(200)
    expect(shown.text).toContain('<strong>post</strong>')
    expect(shown.text).toContain('<input type="checkbox" disabled> todo')
    expect(shown.text).toContain('<input type="checkbox" disabled checked> done')
    expect((await page('/feed.xml')).text).toContain('<link>http://localhost/hello-world</link>')
  })

  it('keeps the address when the note is renamed and republished', async () => {
    await publish(ID, post())
    const again = await publish(ID, post({ slug: 'a-new-name', title: 'A New Name' }))
    expect(await again.json()).toMatchObject({ slug: 'hello-world' })
    expect((await page('/hello-world')).text).toContain('<h1>A New Name</h1>')
  })

  it('gives a second post with the same name its own address', async () => {
    await publish(ID, post())
    const second = (await (await publish(OTHER_ID, post({ body: 'second' }))).json()) as { slug: string }
    expect(second.slug).toBe('hello-world-aaaaaaaa')
    expect((await page('/hello-world')).text).toContain('First')
    expect((await page(`/${second.slug}`)).text).toContain('second')
  })

  it('does not let a post take a path the server needs', async () => {
    const response = await publish(ID, post({ slug: 'api', title: 'API' }))
    expect(((await response.json()) as { slug: string }).slug).not.toBe('api')
  })

  it('removes an unpublished post', async () => {
    await publish(ID, post())
    const removed = await app.request(`/api/posts/${ID}`, { method: 'DELETE', headers: { Authorization: `Bearer ${TOKEN}` } })
    expect(removed.status).toBe(204)
    expect((await page('/hello-world')).status).toBe(404)
    expect((await page('/')).text).toContain('Nothing published yet.')
  })

  it('hides subscriber-only posts from the public blog', async () => {
    await publish(ID, post({ access: 'subscribers' }))
    expect((await page('/hello-world')).status).toBe(404)
    expect((await page('/')).text).not.toContain('Hello World')
    expect((await page('/feed.xml')).text).not.toContain('Hello World')
  })

  it('builds links from the configured public address', async () => {
    app = createApp({ db: sqlite(':memory:'), token: TOKEN, site: { title: 'T', description: '', url: 'https://blog.example.com/' } })
    expect(await (await publish(ID, post())).json()).toMatchObject({ url: 'https://blog.example.com/hello-world' })
    expect((await page('/feed.xml')).text).toContain('<link>https://blog.example.com/hello-world</link>')
  })
})

describe('safety', () => {
  it('rejects a wrong or missing token', async () => {
    expect((await publish(ID, post(), 'nope')).status).toBe(401)
    expect((await app.request('/api/posts')).status).toBe(401)
    expect((await page('/hello-world')).status).toBe(404)
  })

  it('refuses to publish when no token is configured', async () => {
    app = createApp({ db: sqlite(':memory:'), token: undefined, site: { title: 'T', description: '' } })
    expect((await publish(ID, post(), '')).status).toBe(503)
    expect((await page('/')).status).toBe(200)
  })

  it('shows HTML in a note as text instead of running it', async () => {
    await publish(ID, post({ title: '<script>alert(1)</script>', body: '<script>alert(2)</script>\n\n[x](javascript:alert(3))' }))
    const { text } = await page('/hello-world')
    expect(text).not.toContain('<script>')
    expect(text).not.toContain('href="javascript:')
    expect((await page('/')).text).not.toContain('<script>')
  })

  it('rejects malformed posts', async () => {
    expect((await publish(ID, post({ date: 'yesterday' }))).status).toBe(400)
    expect((await publish(ID, post({ title: 42 }))).status).toBe(400)
    expect((await publish('bad id!', post())).status).toBe(400)
    expect((await publish(ID, 'not json')).status).toBe(400)
  })
})
