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
''
├── md/          # All markdown files (published or not)
├── posts/       # Generated HTML files
├── index.html   # Main index in root
├── styles.css   # Styles in root
├── build.sh     # Build script
└── README.md
''
## Getting Started

1. Create a new post:
   - Create a new `.md` file in the `md/` directory
   - Add frontmatter at the top with title and date:
     ```markdown
     ---
     title: Your Post Title
     date: YYYY-MM-DD
     ---
     ```
   - Write your content in markdown below the frontmatter

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

Posts are written in markdown with a simple structure:

1. **Frontmatter**: Each post starts with frontmatter (between `---` lines) that contains metadata:
   ```markdown
   ---
   date: YYYY-MM-DD
   ---
   ```
   The date is required and will be displayed with your post.

2. **Title**: The first markdown heading (`#`) in your content will be used as your post's title. This title will appear:
   - In the page header
   - In the browser tab
   - In the list of posts on the index page

3. **Content**: After the title, write your post content using standard markdown features:

```markdown
---
date: 2024-04-02
---

# Welcome to My Blog

This is our first published post. It demonstrates the new blog structure with separate directories for:

- Drafts
- Published posts
- Generated HTML



- Lists
- **Bold text**
- *Italic text*
- [Links](https://example.com)

## Code blocks

```python
def hello():
    print("Hello, World!")
```


## How It Works
1. Write posts in markdown
2. Keep drafts in the `drafts/` directory
3. Move to `posts/` when ready to publish
4. Generated HTML goes to `html/` directory
## Customization

- Edit `styles.css` to change the look and feel
- Modify `build.sh` to change how the index is generated
- Update `index.html` to change the site structure

## License

MIT 