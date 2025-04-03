#!/bin/bash

# Enable error handling
set -e

# Check if pandoc is installed, if not install it
if ! command -v pandoc &> /dev/null; then
    echo "Installing pandoc..."
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # For Linux (Cloudflare Pages)
        apt-get update && apt-get install -y pandoc
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # For macOS
        brew install pandoc
    fi
fi

# Create necessary directories if they don't exist
mkdir -p posts

# Clean posts directory
find posts -type f -delete

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
        <ul class="post-list">' > index.html

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
            <a href=\"posts/$slug.html\">$title</a>
            <div class=\"post-date\">$date</div>
        </li>" >> index.html
    
    # Create post HTML
    echo "Creating HTML file: posts/$slug.html"
    echo "<!DOCTYPE html>
<html lang=\"en\">
<head>
    <meta charset=\"utf-8\">
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
    <title>$title</title>
    <link rel=\"stylesheet\" href=\"../styles.css\">
</head>
<body>
    <header>
        <h1>$title</h1>
        <div class=\"post-date\">$date</div>
    </header>
    <main class=\"post-content\">" > "posts/$slug.html"
    
    # Skip frontmatter and convert the rest to HTML
    echo "Converting markdown to HTML..."
    # Extract content after frontmatter and convert to HTML using pandoc
    echo "Debug: Current directory: $(pwd)"
    echo "Debug: Content of $file:"
    cat "$file"
    echo "Debug: Running pandoc conversion..."
    awk 'BEGIN{p=0} /^---$/{p++; next} p==2{print}' "$file" | pandoc -f markdown -t html >> "posts/$slug.html"
    echo "Debug: Generated HTML content:"
    cat "posts/$slug.html"
    
    echo "    </main>
    <footer>
        <p><a href=\"../index.html\">← Back to all posts</a></p>
    </footer>
</body>
</html>" >> "posts/$slug.html"
    
    echo "Successfully created posts/$slug.html"
}

# Process all markdown files in the md directory
for file in md/*.md; do
    if [ -f "$file" ]; then
        process_post "$file"
    fi
done

echo '        </ul>
    </main>

    <footer>
        <p>Built with ❤️ using markdown and git</p>
    </footer>
</body>
</html>' >> index.html

echo "Site built successfully!" 