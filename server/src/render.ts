import MarkdownIt from 'markdown-it'

// Raw HTML in a note is shown as text, not executed. The same server will
// host many writers, so a post must not be able to run script on the site.
const markdown = new MarkdownIt({ html: false, linkify: true })

export function renderMarkdown(source: string): string {
  return markdown.render(source).replace(
    // Checklists: `- [ ]` and `- [x]` items become read-only checkboxes.
    /<li>(\s*<p>)?\[([ xX])\] /g,
    (_match, paragraph: string | undefined, mark: string) =>
      `<li class="task">${paragraph ?? ''}<input type="checkbox" disabled${mark === ' ' ? '' : ' checked'}> `,
  )
}

export function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

/** Lowercase words joined by hyphens, safe to use as a URL path. */
export function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80)
    .replace(/-+$/, '')
}
