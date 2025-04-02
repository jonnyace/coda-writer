# Minimal Markdown Blog

A lightweight, self-contained blog system that uses markdown files and git for content management.

## Features

- Write posts in markdown
- No database required
- Version control for all content
- Fast loading times
- Minimal JavaScript
- Beautiful typography
- Mobile-friendly

## Getting Started

1. Install dependencies:
   ```bash
   npm install
   ```

2. Create a new post:
   - Create a new `.md` file in the `posts/` directory
   - Add frontmatter at the top:
     ```markdown
     ---
     title: Your Post Title
     date: YYYY-MM-DD
     ---
     ```
   - Write your content in markdown

3. Build the site:
   ```bash
   npm run build
   ```

4. Preview locally:
   ```bash
   npm run serve
   ```

## Deployment

1. Push your content to GitHub
2. Connect your repository to Cloudflare Pages
3. Set build command to: `npm run build`
4. Set output directory to: `dist`

## Writing Posts

Posts are written in markdown with frontmatter. Here's an example:

```markdown
---
title: My First Post
date: 2024-03-20
---

# My First Post

This is the content of my post. You can use all standard markdown features:

- Lists
- **Bold text**
- *Italic text*
- [Links](https://example.com)

## Code blocks

```python
def hello():
    print("Hello, World!")
```
```

## Customization

- Edit `styles.css` to change the look and feel
- Modify `build.js` to change how posts are processed
- Update `index.html` to change the site structure

## License

MIT 