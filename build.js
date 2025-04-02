const fs = require('fs');
const path = require('path');
const marked = require('marked');
const matter = require('gray-matter');

// Ensure build directories exist
const buildDir = path.join(__dirname, 'dist');
const postsDir = path.join(buildDir, 'posts');
fs.mkdirSync(buildDir, { recursive: true });
fs.mkdirSync(postsDir, { recursive: true });

// Copy static files
fs.copyFileSync(path.join(__dirname, 'styles.css'), path.join(buildDir, 'styles.css'));
fs.copyFileSync(path.join(__dirname, 'index.html'), path.join(buildDir, 'index.html'));

// Process markdown files
const posts = [];
const sourceDir = path.join(__dirname, 'posts');

fs.readdirSync(sourceDir)
    .filter(file => file.endsWith('.md'))
    .forEach(file => {
        const content = fs.readFileSync(path.join(sourceDir, file), 'utf-8');
        const { data, content: markdown } = matter(content);
        const html = marked(markdown);
        const slug = file.replace('.md', '');
        
        // Create HTML file
        const postHtml = `
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>${data.title}</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1>${data.title}</h1>
        <div class="post-date">${new Date(data.date).toLocaleDateString()}</div>
    </header>
    <main class="post-content">
        ${html}
    </main>
    <footer>
        <p><a href="/">← Back to all posts</a></p>
    </footer>
</body>
</html>`;

        fs.writeFileSync(path.join(postsDir, `${slug}.html`), postHtml);
        
        // Add to posts index
        posts.push({
            title: data.title,
            date: data.date,
            slug: slug
        });
    });

// Sort posts by date
posts.sort((a, b) => new Date(b.date) - new Date(a.date));

// Write posts index
fs.writeFileSync(
    path.join(postsDir, 'index.json'),
    JSON.stringify(posts, null, 2)
);

console.log('Build complete!'); 