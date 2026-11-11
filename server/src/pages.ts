import { escapeHtml, renderMarkdown } from './render'

export interface Site {
  title: string
  description: string
  /** Public address, e.g. `https://blog.example.com`. Defaults to the address of each request. */
  url?: string
}

export interface Post {
  id: string
  slug: string
  title: string
  body: string
  date: string
  updated: string
  access: string
}

// The palette is the original blog's (site/styles.css), with a light variant.
export const STYLES = `
:root {
  --bg: #01242e; --fg: #dfe8ea; --muted: #7c9fa9; --accent: #8cc2dd; --line: #0c3946; --code: #011b23;
  color-scheme: dark;
}
@media (prefers-color-scheme: light) {
  :root { --bg: #f7f9f8; --fg: #0f2a31; --muted: #5a7780; --accent: #1a6c8c; --line: #d2dedd; --code: #e9efee; color-scheme: light; }
}
* { box-sizing: border-box; }
body {
  background: var(--bg); color: var(--fg);
  font: 1.125rem/1.7 'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif;
  margin: 0 auto; max-width: 42rem; padding: 2.5rem 1.25rem 4rem;
  overflow-wrap: break-word;
}
a { color: var(--accent); }
.site { font-family: system-ui, sans-serif; font-size: 0.95rem; margin-bottom: 3rem; }
.site a { color: var(--fg); font-weight: 650; text-decoration: none; }
.site p, time, .empty { color: var(--muted); }
.site p { margin: 0.2rem 0 0; }
time { font: 0.9rem system-ui, sans-serif; }
h1 { font-size: 2.2rem; letter-spacing: -0.01em; line-height: 1.2; margin: 0.3rem 0 2rem; }
h2 { font-size: 1.45rem; line-height: 1.3; margin: 2.2rem 0 0.6rem; }
h3 { font-size: 1.2rem; margin: 1.8rem 0 0.5rem; }
.posts { list-style: none; margin: 0; padding: 0; }
.posts li { margin-bottom: 1.6rem; }
.posts a { display: block; font-size: 1.35rem; font-weight: 600; line-height: 1.3; text-decoration: none; }
.posts a:hover { text-decoration: underline; }
li.task { list-style: none; margin-left: -1.4em; }
blockquote { border-left: 3px solid var(--line); color: var(--muted); margin: 1.2rem 0; padding-left: 1rem; }
code { background: var(--code); border-radius: 4px; font: 0.85em ui-monospace, monospace; padding: 0.1em 0.35em; }
pre { background: var(--code); border-radius: 6px; overflow-x: auto; padding: 0.9rem 1rem; }
pre code { background: none; padding: 0; }
img { max-width: 100%; }
hr { border: 0; border-top: 1px solid var(--line); margin: 2.5rem 0; }
footer { font: 0.9rem system-ui, sans-serif; margin-top: 4rem; }
`

function layout(site: Site, pageTitle: string, content: string): string {
  const description = site.description ? `<p>${escapeHtml(site.description)}</p>` : ''
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(pageTitle)}</title>
<link rel="stylesheet" href="/styles.css">
<link rel="alternate" type="application/rss+xml" title="${escapeHtml(site.title)}" href="/feed.xml">
</head>
<body>
<header class="site"><a href="/">${escapeHtml(site.title)}</a>${description}</header>
<main>
${content}
</main>
</body>
</html>`
}

const titleOf = (post: Pick<Post, 'title'>) => post.title || 'Untitled'

function dateLabel(date: string): string {
  const parsed = new Date(`${date}T00:00:00Z`)
  if (Number.isNaN(parsed.getTime())) return date
  return parsed.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric', timeZone: 'UTC' })
}

const timeTag = (date: string) => `<time datetime="${escapeHtml(date)}">${escapeHtml(dateLabel(date))}</time>`

export function indexPage(site: Site, posts: Post[]): string {
  const list = posts.length
    ? `<ul class="posts">${posts
        .map((post) => `<li><a href="/${encodeURI(post.slug)}">${escapeHtml(titleOf(post))}</a>${timeTag(post.date)}</li>`)
        .join('\n')}</ul>`
    : '<p class="empty">Nothing published yet.</p>'
  return layout(site, site.title, list)
}

export function postPage(site: Site, post: Post): string {
  return layout(
    site,
    `${titleOf(post)} | ${site.title}`,
    `<article>
${timeTag(post.date)}
<h1>${escapeHtml(titleOf(post))}</h1>
${renderMarkdown(post.body)}
</article>
<footer><a href="/">All posts</a></footer>`,
  )
}

export function notFoundPage(site: Site): string {
  return layout(site, `Not found | ${site.title}`, '<h1>Page not found</h1><p><a href="/">See all posts</a></p>')
}

export function feed(site: Site, origin: string, posts: Post[]): string {
  const items = posts
    .map((post) => {
      const url = `${origin}/${encodeURI(post.slug)}`
      const published = new Date(`${post.date}T00:00:00Z`)
      const pubDate = Number.isNaN(published.getTime()) ? '' : `<pubDate>${published.toUTCString()}</pubDate>`
      return `<item><title>${escapeHtml(titleOf(post))}</title><link>${url}</link><guid>${url}</guid>${pubDate}<description>${escapeHtml(renderMarkdown(post.body))}</description></item>`
    })
    .join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel>
<title>${escapeHtml(site.title)}</title>
<link>${origin}/</link>
<description>${escapeHtml(site.description || site.title)}</description>
${items}
</channel></rss>`
}
