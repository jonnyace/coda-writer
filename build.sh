#!/bin/bash

# Create posts directory if it doesn't exist
mkdir -p posts

# Generate index.html with a list of all markdown files
echo '<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>My Notes</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1>My Notes</h1>
        <p>A collection of thoughts, ideas, and learnings.</p>
    </header>

    <main>
        <ul class="post-list">' > index.html

# Add each markdown file to the index
for file in posts/*.md; do
    if [ -f "$file" ]; then
        title=$(grep "^title:" "$file" | cut -d'"' -f2)
        date=$(grep "^date:" "$file" | cut -d'"' -f2)
        slug=$(basename "$file" .md)
        echo "            <li class=\"post-item\">
                <a href=\"$slug.html\">$title</a>
                <div class=\"post-date\">$date</div>
            </li>" >> index.html
    fi
done

echo '        </ul>
    </main>

    <footer>
        <p>Built with ❤️ using markdown and git</p>
    </footer>
</body>
</html>' >> index.html

echo "Index generated successfully!" 