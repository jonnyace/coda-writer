# Minimal Markdown Blog

A lightweight, self-contained blog system that uses markdown files and git for content management. No dependencies required!

## Features

- Write posts in markdown
- No database required
- No dependencies
- Version control for all content
- Fast loading times
- No JavaScript
- Beautiful typography
- Mobile-friendly

## Getting Started

1. Create a new post:
   - Create a new `.md` file in the `posts/` directory
   - Add frontmatter at the top:
     ```markdown
     ---
     title: Your Post Title
     date: YYYY-MM-DD
     ---
     ```
   - Write your content in markdown

2. Generate the index:
   ```bash
   ./build.sh
   ```

3. Preview locally:
   ```bash
   python3 -m http.server 8000
   ```

## Deployment

1. Push your content to GitHub
2. Connect your repository to Cloudflare Pages
3. Set build command to: `./build.sh`
4. Set output directory to: `.`

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
- Modify `build.sh` to change how the index is generated
- Update `index.html` to change the site structure

## License

MIT 