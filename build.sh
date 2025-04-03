#!/bin/bash

# Enable error handling
set -e

# Create necessary directories if they don't exist
mkdir -p drafts posts html

# Clean html directory but preserve styles.css if it exists
find html -type f ! -name 'styles.css' -delete

echo "Generating index.html..."

# Generate index.html with a list of all markdown files
echo '<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>My Notes</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <header>
        <h1>My Notes</h1>
        <p>A collection of thoughts, ideas, and learnings.</p>
    </header>

    <main>
        <ul class="post-list">' > html/index.html

# Function to process a markdown file
process_post() {
    local file=$1
    
    echo "Processing $file..."
    
    # Extract date from frontmatter and title from first markdown heading
    date=$(grep "^date:" "$file" | sed 's/^date: *//')
    title=$(grep "^# " "$file" | head -n 1 | sed 's/^# *//')
    slug=$(basename "$file" .md)
    
    echo "Title: $title"
    echo "Date: $date"
    echo "Slug: $slug"
    
    # Add to index
    echo "            <li class=\"post-item\">
            <a href=\"$slug.html\">$title</a>
            <div class=\"post-date\">$date</div>
        </li>" >> html/index.html
    
    # Create post HTML
    echo "Creating HTML file: html/$slug.html"
    echo "<!DOCTYPE html>
<html lang=\"en\">
<head>
    <meta charset=\"utf-8\">
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
    <title>$title</title>
    <link rel=\"stylesheet\" href=\"styles.css\">
</head>
<body>
    <header>
        <h1>$title</h1>
        <div class=\"post-date\">$date</div>
    </header>
    <main class=\"post-content\">" > "html/$slug.html"
    
    # Skip frontmatter and convert the rest to HTML
    echo "Converting markdown to HTML..."
    awk 'BEGIN{p=0} /^---$/{p++; next} p==2{print}' "$file" | while IFS= read -r line; do
        echo "<p>$line</p>" >> "html/$slug.html"
    done
    
    echo "    </main>
    <footer>
        <p><a href=\"index.html\">← Back to all posts</a></p>
    </footer>
</body>
</html>" >> "html/$slug.html"
    
    echo "Successfully created html/$slug.html"
}

# Process files based on arguments
if [ "$#" -eq 0 ]; then
    # No arguments, process all published posts
    for file in posts/*.md; do
        if [ -f "$file" ]; then
            process_post "$file"
        fi
    done
else
    # Process specific files
    for file in "$@"; do
        if [ -f "$file" ]; then
            # Check if file is in drafts or posts directory
            if [[ "$file" == drafts/* ]]; then
                # Move from drafts to posts
                filename=$(basename "$file")
                mv "$file" "posts/$filename"
                process_post "posts/$filename"
            elif [[ "$file" == posts/* ]]; then
                process_post "$file"
            else
                echo "Error: File must be in drafts/ or posts/"
                exit 1
            fi
        fi
    done
fi

echo '        </ul>
    </main>

    <footer>
        <p>Built with ❤️ using markdown and git</p>
    </footer>
</body>
</html>' >> html/index.html

# Ensure styles.css exists in html directory
if [ ! -f html/styles.css ]; then
    cp styles.css html/
fi

# Clean up root directory
rm -f index.html

echo "Site built successfully!" 