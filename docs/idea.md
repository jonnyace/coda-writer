
### Storing Markdown in Supabase

Supabase provides two main options for storing markdown: the database (as text in a table) or the Storage API (as files in buckets). Here’s how we can approach it:

#### Option 2: Store Markdown in Supabase Storage
- **Where**: In a Supabase Storage bucket as `.md` files.
- **Structure**:
  - Bucket: `blogs`
  - Path: `username/blogname/post-id.md`
    ```
    blogs/
      user1/
        my-travel-blog/
          post-123.md
          post-124.md
        my-tech-blog/
          post-125.md
      user2/
        my-life-blog/
          post-126.md
    ```
- **Schema**:
  - `users`: `id`, `email`, `stripe_customer_id`
  - `blogs`: `id`, `user_id`, `name`, `slug`
  - `posts`: `id`, `blog_id`, `title`, `file_path` (e.g., `user1/my-travel-blog/post-123.md`), `created_at`
- **How It Works**:
  - When a user saves a post, Tiptap generates markdown, which is uploaded to the `blogs` bucket under their username and blog name.
  - Metadata (e.g., title, blog ID) is stored in the `posts` table, with `file_path` pointing to the markdown file in storage.
  - Cloudflare serves the static site by fetching markdown from Supabase Storage and rendering it.
- **Pros**:
  - Keeps the `.md` file concept intact.
  - Scales better for large files (Storage is optimized for blobs).
  - Separates content from metadata, aligning with your original file-based approach.
- **Cons**:
  - Slightly more complex: Requires managing both database and storage.
  - Requires an extra step to fetch file contents vs. querying a single table.

#### Recommendation
**Go with Option 2: Supabase Storage.** It preserves your markdown-file-based workflow, keeps the system lightweight, and leverages Supabase’s built-in storage solution. The database tracks metadata (e.g., blog ownership, post titles), while Storage holds the actual `.md` files, making it feel closer to your original design without needing git.

- **Storage Path**: `blogs/{username}/{blogname}/{post-id}.md`
- **Deployment**: Cloudflare Workers fetches markdown from Supabase Storage and renders it as HTML.

---

### Updated Workflow Without Git
1. **User Creates a Blog**:
   - Adds a blog name in the dashboard → saved to `blogs` table in Supabase.
2. **User Writes a Post**:
   - Uses Tiptap → markdown is generated → uploaded to Supabase Storage → metadata saved to `posts` table.
3. **Post Publishes**:
   - Cloudflare Worker queries Supabase for post metadata, fetches the `.md` file from Storage, and renders it as a static page (e.g., `username-blogname.yourdomain.com/post-123`).
4. **Feed Updates**:
   - Cloudflare Worker queries the `posts` table for a list of recent posts across all users.

---

### Frontend: Adjusted Dashboard

The frontend stays simple and normie-friendly, now interacting with Supabase instead of git. Here’s how it adapts:

#### Dashboard Layout (Same as Before)
```
--------------------------------------------------
| [Your Name]          [Logout]                  |
|------------------------------------------------|
| My Blogs           | Feed                      |
|------------------------------------------------|
| [Blog 1]           | [Post from User2]        |
| [Blog 2]           | [Post from User3]        |
| [+ New Blog]       | [Post from User1]        |
|------------------------------------------------|
| [Selected Blog: Blog 1]                        |
| [Post Title 1]   [Edit] [Delete]              |
| [Post Title 2]   [Edit] [Delete]              |
| [+ New Post]                                   |
--------------------------------------------------
```

#### Key Adjustments
1. **Blog Creation**:
   - “+ New Blog” → prompts for a name → inserts into `blogs` table via Supabase API.
   - No git repo setup needed.

2. **Post Writing**:
   - “+ New Post” → opens Tiptap editor.
   - On save, markdown is uploaded to Supabase Storage (e.g., `blogs/username/blogname/post-123.md`), and metadata is saved to `posts` table.
   - No git commit step—just a direct API call.

3. **Blog Switching**:
   - Select a blog from “My Blogs” → loads its posts from the `posts` table (no git fetching).

4. **Feed**:
   - Queries `posts` table for recent posts, displays titles and links to static URLs.

#### Tech Stack
- **HTML + CSS**: Static dashboard base.
- **Minimal JS**: Vanilla JS or Alpine.js for interactivity.
- **Tiptap**: For post writing, outputs markdown.
- **Supabase Client**: Handles auth, database queries, and storage uploads.
- **Cloudflare Workers**: Renders static pages from Supabase Storage.

#### User Flow
1. **Login**: Supabase Auth → dashboard loads.
2. **Create Blog**: “+ New Blog” → saved to `blogs` table.
3. **Write Post**: “+ New Post” → Tiptap → markdown uploaded to Storage, metadata to database.
4. **View Blog**: Visit `username-blogname.yourdomain.com` → Cloudflare renders posts.

---

### Example Implementation

#### Supabase Setup
- **Tables**:
  ```sql
  CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT UNIQUE,
    stripe_customer_id TEXT
  );

  CREATE TABLE blogs (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    name TEXT,
    slug TEXT UNIQUE
  );

  CREATE TABLE posts (
    id UUID PRIMARY KEY,
    blog_id UUID REFERENCES blogs(id),
    title TEXT,
    file_path TEXT, -- e.g., "blogs/user1/my-travel-blog/post-123.md"
    created_at TIMESTAMP DEFAULT NOW()
  );
  ```
- **Storage Bucket**: Create a `blogs` bucket in Supabase with public read access (for rendering) and restricted write access (via API keys).

#### Edge Function: Save Post
```javascript
// Deno (Supabase Edge Function)
import { createClient } from 'https://esm.sh/@supabase/supabase-js';

const supabase = createClient(Deno.env.get('SUPABASE_URL'), Deno.env.get('SUPABASE_KEY'));

Deno.serve(async (req) => {
  const { user_id, blog_id, title, content } = await req.json();

  // Insert post metadata
  const { data: post } = await supabase
    .from('posts')
    .insert({ blog_id, title, file_path: '' })
    .select()
    .single();

  // Get blog slug
  const { data: blog } = await supabase.from('blogs').select('slug').eq('id', blog_id).single();
  const username = (await supabase.from('users').select('email').eq('id', user_id).single()).data.email.split('@')[0];
  const filePath = `blogs/${username}/${blog.slug}/${post.id}.md`;

  // Upload markdown to Storage
  await supabase.storage.from('blogs').upload(filePath, content, { contentType: 'text/markdown' });

  // Update post with file path
  await supabase.from('posts').update({ file_path: filePath }).eq('id', post.id);

  return new Response('Post saved', { status: 200 });
});
```

#### Frontend: Dashboard Snippet
```html
<!DOCTYPE html>
<html>
<head>
  <title>My Blogs</title>
  <script src="https://cdn.jsdelivr.net/npm/@tiptap/core/dist/tiptap.umd.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/@supabase/supabase-js"></script>
</head>
<body>
  <header>
    <span>Welcome, <span id="username"></span></span>
    <button onclick="supabase.auth.signOut()">Logout</button>
  </header>
  <div class="container">
    <aside>
      <h2>My Blogs</h2>
      <ul id="blogs-list"></ul>
      <button onclick="createBlog()">+ New Blog</button>
    </aside>
    <main>
      <h2>Feed</h2>
      <div id="feed"></div>
    </main>
    <section>
      <h3>Selected Blog: <span id="blog-name"></span></h3>
      <ul id="posts-list"></ul>
      <button onclick="openEditor()">+ New Post</button>
    </section>
  </div>

  <script>
    const supabase = Supabase.createClient('your-url', 'your-key');
    let selectedBlog = null;

    async function loadBlogs() {
      const { data: user } = await supabase.auth.getUser();
      document.getElementById('username').textContent = user.email.split('@')[0];
      const { data } = await supabase.from('blogs').select('*').eq('user_id', user.id);
      document.getElementById('blogs-list').innerHTML = data.map(blog => `
        <li onclick="selectBlog('${blog.id}', '${blog.name}')">${blog.name}</li>
      `).join('');
    }

    async function selectBlog(id, name) {
      selectedBlog = id;
      document.getElementById('blog-name').textContent = name;
      const { data } = await supabase.from('posts').select('*').eq('blog_id', id);
      document.getElementById('posts-list').innerHTML = data.map(post => `
        <li>${post.title} <button>Edit</button> <button>Delete</button></li>
      `).join('');
    }

    async function createBlog() {
      const name = prompt('Blog name:');
      if (name) {
        const slug = name.toLowerCase().replace(/\s+/g, '-');
        await supabase.from('blogs').insert({ user_id: (await supabase.auth.getUser()).data.user.id, name, slug });
        loadBlogs();
      }
    }

    async function openEditor() {
      if (!selectedBlog) return alert('Select a blog first!');
      // Tiptap setup (simplified)
      const editor = new Tiptap.Editor({ content: '', extensions: [Tiptap.StarterKit] });
      // Add save button logic to upload to Supabase
      document.body.appendChild(/* Tiptap modal with save button */);
    }

    loadBlogs();
  </script>
</body>
</html>
```

#### Cloudflare Worker: Render Blog
```javascript
addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request));
});

async function handleRequest(request) {
  const supabase = createClient('your-url', 'your-key');
  const url = new URL(request.url);
  const [username, blogname, postId] = url.pathname.split('/').slice(1); // e.g., /user1/my-travel-blog/post-123

  const { data: post } = await supabase
    .from('posts')
    .select('file_path, title')
    .eq('file_path', `blogs/${username}/${blogname}/${postId}.md`)
    .single();

  const { data: markdown } = await supabase.storage.from('blogs').download(post.file_path);
  const html = `<h1>${post.title}</h1><div>${marked(markdown)}</div>`; // Use a markdown parser like 'marked'

  return new Response(html, { headers: { 'Content-Type': 'text/html' } });
}
```



### Where Stripe Fits
Stripe will integrate at two key points:
1. **User Dashboard**: A frontend interface where users can subscribe to premium features (e.g., custom domains, themes, or analytics).
2. **Backend Logic**: Supabase Edge Functions to process payments, verify subscriptions, and update user status, with Cloudflare leveraging this data to unlock premium features on the public site.

Here’s how it ties into the existing components:
- **Supabase**: Stores user payment status (e.g., `stripe_customer_id`, `subscription_status`) and triggers backend logic via Edge Functions.
- **Cloudflare**: Checks user status to enable premium features (e.g., custom domains) when rendering blogs.
- **Frontend**: Offers a “Go Premium” button and payment flow in the dashboard.

---

### What Stripe Enables
Stripe can power a subscription model or one-time purchases. For simplicity, let’s assume a subscription model (e.g., $5/month for premium features). Potential premium features could include:
- Custom domains (e.g., `myblog.com` instead of `username-blogname.yourdomain.com`).
- Premium themes or typography options.
- Analytics (e.g., page views for their blog).
- Priority in the feed (e.g., featured posts).

---

### Updated Architecture with Stripe
#### Supabase Schema
Add payment-related fields to track subscriptions:
- `users`: `id`, `email`, `stripe_customer_id` (Stripe’s unique ID for the user), `subscription_status` (e.g., `active`, `inactive`)
- `blogs`: `id`, `user_id`, `name`, `slug`, `custom_domain` (premium feature, nullable)
- `posts`: `id`, `blog_id`, `title`, `file_path`, `created_at`

#### Stripe Integration Points
1. **Subscription Signup**:
   - User clicks “Go Premium” in the dashboard → initiates a Stripe Checkout session.
   - Supabase Edge Function creates the session and redirects the user to Stripe’s hosted payment page.

2. **Webhook Handling**:
   - Stripe sends a webhook to your backend when a payment succeeds or a subscription changes (e.g., renewed, canceled).
   - A Supabase Edge Function processes the webhook, updating the user’s `subscription_status` in the `users` table.

3. **Premium Feature Enforcement**:
   - Cloudflare Worker checks the user’s `subscription_status` via Supabase before rendering premium features (e.g., custom domain routing).

---

### How It Works

#### 1. Frontend: Initiating Payment
In the user dashboard, add a “Go Premium” button that triggers a Stripe Checkout session.

**Updated Dashboard Snippet**:
```html
<header>
  <span>Welcome, <span id="username"></span></span>
  <button onclick="goPremium()">Go Premium</button>
  <button onclick="supabase.auth.signOut()">Logout</button>
</header>

<script>
  const supabase = Supabase.createClient('your-url', 'your-key');

  async function goPremium() {
    const { data: user } = await supabase.auth.getUser();
    const response = await fetch('https://your-edge-function.deno.dev/create-checkout', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ user_id: user.id, email: user.email }),
    });
    const { url } = await response.json();
    window.location.href = url; // Redirect to Stripe Checkout
  }

  // Rest of the dashboard logic (loadBlogs, etc.) remains unchanged
</script>
```

#### 2. Edge Function: Create Checkout Session
A Supabase Edge Function creates a Stripe Checkout session and returns the payment URL.

```javascript
// Deno (Supabase Edge Function)
import { createClient } from 'https://esm.sh/@supabase/supabase-js';
import { Stripe } from 'https://esm.sh/stripe@8.174.0';

const supabase = createClient(Deno.env.get('SUPABASE_URL'), Deno.env.get('SUPABASE_KEY'));
const stripe = new Stripe(Deno.env.get('STRIPE_SECRET_KEY'));

Deno.serve(async (req) => {
  if (req.method !== 'POST') return new Response('Method not allowed', { status: 405 });

  const { user_id, email } = await req.json();

  // Check if user already has a Stripe customer ID
  let { data: user } = await supabase.from('users').select('stripe_customer_id').eq('id', user_id).single();
  let customerId = user.stripe_customer_id;

  if (!customerId) {
    const customer = await stripe.customers.create({ email });
    customerId = customer.id;
    await supabase.from('users').update({ stripe_customer_id: customerId }).eq('id', user_id);
  }

  // Create Checkout session
  const session = await stripe.checkout.sessions.create({
    customer: customerId,
    payment_method_types: ['card'],
    mode: 'subscription',
    line_items: [{ price: 'price_12345', quantity: 1 }], // Replace with your Stripe Price ID
    success_url: 'https://yourdomain.com/dashboard?success=true',
    cancel_url: 'https://yourdomain.com/dashboard?canceled=true',
  });

  return new Response(JSON.stringify({ url: session.url }), { status: 200 });
});
```

- **Setup**: Replace `'price_12345'` with a Price ID from your Stripe dashboard (e.g., $5/month subscription).
- **Environment Variables**: Add `STRIPE_SECRET_KEY` to your Supabase Edge Function environment.

#### 3. Edge Function: Handle Webhook
Stripe sends webhooks to confirm payment events (e.g., subscription created, renewed, or canceled).

```javascript
// Deno (Supabase Edge Function)
import { createClient } from 'https://esm.sh/@supabase/supabase-js';
import { Stripe } from 'https://esm.sh/stripe@8.174.0';

const supabase = createClient(Deno.env.get('SUPABASE_URL'), Deno.env.get('SUPABASE_KEY'));
const stripe = new Stripe(Deno.env.get('STRIPE_SECRET_KEY'));

Deno.serve(async (Jewelryreq => {
  const sig = req.headers.get('stripe-signature');
  const body = await req.text();
  let event;

  try {
    event = stripe.webhooks.constructEvent(body, sig, Deno.env.get('STRIPE_WEBHOOK_SECRET'));
  } catch (err) {
    return new Response('Webhook signature verification failed', { status: 400 });
  }

  if (event.type === 'customer.subscription.created' || event.type === 'customer.subscription.updated') {
    const customerId = event.data.object.customer;
    const status = event.data.object.status === 'active' ? 'active' : 'inactive';

    await supabase
      .from('users')
      .update({ subscription_status: status })
      .eq('stripe_customer_id', customerId);
  } else if (event.type === 'customer.subscription.deleted') {
    const customerId = event.data.object.customer;
    await supabase
      .from('users')
      .update({ subscription_status: 'inactive' })
      .eq('stripe_customer_id', customerId);
  }

  return new Response('Webhook processed', { status: 200 });
});
```

- **Setup**: Configure the webhook endpoint in Stripe (e.g., `https://your-edge-function.deno.dev/webhook`) and add `STRIPE_WEBHOOK_SECRET` to your environment.

#### 4. Cloudflare Worker: Render Premium Features
The Worker checks the user’s subscription status to enable premium features like custom domains.

```javascript
addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request));
});

async function handleRequest(request) {
  const supabase = createClient('your-url', 'your-key');
  const url = new URL(request.url);
  const [username, blogname, postId] = url.pathname.split('/').slice(1);

  const { data: blog } = await supabase
    .from('blogs')
    .select('slug, user_id')
    .eq('slug', blogname)
    .single();

  const { data: user } = await supabase
    .from('users')
    .select('subscription_status')
    .eq('id', blog.user_id)
    .single();

  const { data: post } = await supabase
    .from('posts')
    .select('file_path, title')
    .eq('file_path', `blogs/${username}/${blogname}/${postId}.md`)
    .single();

  const { data: markdown } = await supabase.storage.from('blogs').download(post.file_path);

  let html = `<h1>${post.title}</h1><div>${marked(markdown)}</div>`;
  if (user.subscription_status === 'active') {
    html += `<footer>Premium Blog - Custom Domain Enabled</footer>`;
    // Add custom domain logic here if mapped via Cloudflare
  }

  return new Response(html, { headers: { 'Content-Type': 'text/html' } });
}
```

---

### User Flow with Stripe
1. **User Goes Premium**:
   - Clicks “Go Premium” → redirected to Stripe Checkout → pays $5/month.
2. **Payment Confirmed**:
   - Stripe webhook updates `subscription_status` to `active` in Supabase.
3. **Premium Features Unlocked**:
   - Cloudflare renders the blog with premium features (e.g., custom domain, special badge).
4. **Dashboard Feedback**:
   - Dashboard shows “Premium Active” instead of “Go Premium” button.

---

### Setup Steps
1. **Stripe Account**:
   - Create a Stripe account, set up a $5/month subscription product, and get your API keys.
2. **Supabase**:
   - Add `stripe_customer_id` and `subscription_status` to the `users` table.
3. **Edge Functions**:
   - Deploy the `create-checkout` and `webhook` functions with Stripe keys.
4. **Dashboard**:
   - Add the “Go Premium” button and logic.
5. **Cloudflare**:
   - Update the Worker to check `subscription_status`.

-

---

### Homepage Design Goals
- **Showcase Feed**: Display a mix of recent and random posts to highlight activity and variety.
- **Signup Prompt**: Include an easy signup form to convert visitors into users.
- **Lightweight**: No JavaScript, fast loading via Cloudflare, and mobile-friendly.
- **Engaging**: Encourage exploration of blogs and immediate account creation.
To transform the homepage feed into an endless scroll that includes the first few lines of each post, we’ll need to make a few adjustments. Since the homepage is rendered statically by a Cloudflare Worker with no JavaScript, we’ll shift to a lightweight JavaScript approach to enable endless scrolling. We’ll fetch post content from Supabase Storage to extract a preview (first few lines), and use the Supabase Edge Function to paginate the feed dynamically. Here’s how we can do it while keeping it user-friendly and performant.

---

### Updated Design Goals
- **Endless Scroll**: Load more posts as the user scrolls down, without manual “Load More” buttons.
- **Post Previews**: Show the first few lines (e.g., 2-3 lines or ~100 characters) of each post’s markdown content.
- **Mixed Feed**: Retain the random/recent mix from before.
- **Lightweight**: Minimize JS, leverage Cloudflare caching, and keep it mobile-friendly.
- **Signup Integration**: Keep the signup form at the top.

---

### Updated Homepage Layout
```
--------------------------------------------------
| [Your Platform Name]                            |
|------------------------------------------------|
| Welcome to [Your Platform Name]                |
| Create your own blog in seconds!               |
| [Email Field] [Sign Up Button]                 |
|------------------------------------------------|
| Community Feed                                 |
| - "Post Title 1" by user1 (blog-slug)          |
|   First few lines of the post...               |
| - "Post Title 2" by user2 (blog-slug)          |
|   First few lines of the post...               |
| ... (scrolls endlessly)                        |
--------------------------------------------------
```

---

### Implementation

#### 1. Updated Supabase Edge Function: Paginated Feed with Previews
Modify the feed Edge Function to support pagination and fetch post content previews from Supabase Storage.

**Edge Function: Get Feed**
```javascript
// Deno (Supabase Edge Function)
import { createClient } from 'https://esm.sh/@supabase/supabase-js';

const supabase = createClient(Deno.env.get('SUPABASE_URL'), Deno.env.get('SUPABASE_KEY'));

Deno.serve(async (req) => {
  const url = new URL(req.url);
  const page = parseInt(url.searchParams.get('page') || '1');
  const perPage = 10; // Posts per page

  // Fetch recent posts (half of perPage)
  const { data: recentPosts } = await supabase
    .from('posts')
    .select('id, title, file_path, created_at, blog_id')
    .order('created_at', { ascending: false })
    .range((page - 1) * perPage / 2, page * perPage / 2 - 1)
    .limit(perPage / 2);

  // Fetch random posts (half of perPage)
  const { data: randomPosts } = await supabase
    .from('posts')
    .select('id, title, file_path, created_at, blog_id')
    .order('random()')
    .limit(perPage / 2);

  // Combine and deduplicate
  const allPosts = [...recentPosts, ...randomPosts];
  const uniquePosts = Array.from(new Map(allPosts.map(post => [post.id, post])).values());

  // Enrich with blog, user, and preview data
  const feed = await Promise.all(uniquePosts.map(async (post) => {
    const { data: blog } = await supabase
      .from('blogs')
      .select('slug, user_id')
      .eq('id', post.blog_id)
      .single();
    const { data: user } = await supabase
      .from('users')
      .select('email')
      .eq('id', blog.user_id)
      .single();
    const username = user.email.split('@')[0];

    // Fetch markdown content from Storage and extract preview
    const { data: markdown } = await supabase.storage.from('blogs').download(post.file_path);
    const text = new TextDecoder().decode(markdown);
    const preview = text.split('\n').slice(0, 2).join(' ').substring(0, 100) + '...';

    const url = `https://${username}-${blog.slug}.yourdomain.com/${post.id}`;
    return {
      title: post.title,
      url,
      username,
      blogSlug: blog.slug,
      created_at: post.created_at,
      preview,
    };
  }));

  // Shuffle for a mixed feel
  const shuffledFeed = feed.sort(() => Math.random() - 0.5);

  return new Response(JSON.stringify(shuffledFeed), {
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'public, max-age=300' },
    status: 200,
  });
});
```

- **Pagination**: Uses `range` for recent posts to fetch in batches (e.g., 0-4, 5-9). Random posts are unpaged but limited to half the batch size.
- **Previews**: Downloads markdown from Storage, takes the first 2 lines or ~100 characters.
- **Caching**: Adds a 5-minute cache header for Cloudflare to reduce load on Supabase.

#### 2. Updated Homepage: Endless Scroll with JS
Since endless scrolling requires JavaScript, we’ll add a minimal script to the homepage.

**Homepage HTML + JS**
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Your Platform Name</title>
  <style>
    body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
    header { text-align: center; }
    .signup { margin: 20px 0; text-align: center; }
    .feed { margin-top: 20px; }
    .post { margin: 15px 0; border-bottom: 1px solid #eee; padding-bottom: 10px; }
    .preview { font-size: 0.9em; color: #666; }
    input { padding: 8px; margin-right: 10px; }
    button { padding: 8px 16px; background: #007bff; color: white; border: none; cursor: pointer; }
    button:hover { background: #0056b3; }
  </style>
</head>
<body>
  <header>
    <h1>Your Platform Name</h1>
  </header>
  <section class="signup">
    <p>Welcome to Your Platform Name</p>
    <p>Create your own blog in seconds!</p>
    <form action="https://your-edge-function.deno.dev/signup" method="POST">
      <input type="email" name="email" placeholder="Enter your email" required>
      <button type="submit">Sign Up</button>
    </form>
  </section>
  <section class="feed" id="feed">
    <h2>Community Feed</h2>
    <!-- Posts loaded dynamically -->
  </section>

  <script>
    let page = 1;
    let isLoading = false;

    async function loadPosts() {
      if (isLoading) return;
      isLoading = true;

      const response = await fetch(`https://your-edge-function.deno.dev/feed?page=${page}`);
      const posts = await response.json();

      const feed = document.getElementById('feed');
      posts.forEach(post => {
        const div = document.createElement('div');
        div.className = 'post';
        div.innerHTML = `
          <div>
            <a href="${post.url}">${post.title}</a> by ${post.username} (${post.blogSlug})
            <small>${new Date(post.created_at).toLocaleDateString()}</small>
          </div>
          <div class="preview">${post.preview}</div>
        `;
        feed.appendChild(div);
      });

      page++;
      isLoading = false;
    }

    // Initial load
    loadPosts();

    // Infinite scroll
    window.addEventListener('scroll', () => {
      if (window.innerHeight + window.scrollY >= document.body.offsetHeight - 200) {
        loadPosts();
      }
    });
  </script>
</body>
</html>
```

- **Endless Scroll**: Triggers `loadPosts` when the user scrolls near the bottom (200px threshold).
- **Previews**: Displays the `preview` field below each title.
- **No Frameworks**: Uses vanilla JS for simplicity and minimal overhead.
- **Deploy**: Serve this via Cloudflare Workers with a static file or as a Worker response.

#### 3. Cloudflare Worker: Serve Homepage
Wrap the HTML in a Worker for caching and dynamic routing.

**Cloudflare Worker**
```javascript
addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request));
});

async function handleRequest(request) {
  const url = new URL(request.url);
  if (url.pathname !== '/') {
    return fetch(request);
  }

  const html = `<!-- Paste the HTML from above here -->`;

  return new Response(html, {
    headers: { 'Content-Type': 'text/html', 'Cache-Control': 'public, max-age=300' },
  });
}
```

- **Caching**: 5-minute cache to reduce regeneration (feed updates still happen via JS).
- **Route**: `yourdomain.com/*` with `/` serving the homepage.

---

### How It Works
1. **Initial Load**:
   - Page loads with the first 10 posts (5 recent, 5 random) from `page=1`.
2. **Scroll**:
   - As the user scrolls near the bottom, `page=2` fetches the next batch (next 5 recent + 5 random), appending them to the feed.
3. **Previews**:
   - Each post shows its title, author, blog slug, date, and a short preview (e.g., “Here’s what I learned on my trip to Japan...”).
4. **Signup**:
   - Form at the top submits to the signup Edge Function, unchanged from before.

---

### Feed Behavior
- **Mixed Content**: Half recent, half random per batch, shuffled for variety.
- **Endless**: Continues loading as long as there are posts in Supabase (random posts may repeat, but recent ones paginate sequentially).
- **Previews**: Limited to ~100 characters or 2 lines, giving a taste of the content without overwhelming the feed.

---

### Setup Steps
1. **Edge Function**:
   - Update the feed function with pagination and previews, deploy it.
2. **Homepage**:
   - Deploy the new HTML+JS via Cloudflare Workers or as a static file.
3. **Test**:
   - Scroll the homepage, verify posts load endlessly, and check previews display correctly.
4. **Signup**:
   - Ensure the signup form still works with the existing signup Edge Function.

---

### Example Feed Output
```
Your Platform Name

Welcome to Your Platform Name
Create your own blog in seconds!
[Enter your email] [Sign Up]

Community Feed
- "My Trip to Japan" by user1 (my-travel-blog) - Apr 3, 2025
  Here’s what I learned on my trip to Japan last week...
- "Old Code Tricks" by user2 (my-tech-blog) - Jan 15, 2025
  Found some old code snippets that still work great...
- "Today’s Thoughts" by user3 (my-life-blog) - Apr 4, 2025
  Just reflecting on the day and what’s ahead...
(Scrolls endlessly...)
```

---

### Enhancements
- **Debouncing**: Add a throttle to `loadPosts` to prevent rapid-fire requests.
- **Loading Indicator**: Show a “Loading…” message while fetching new posts.
- **Preview Length**: Adjust the preview cutoff (e.g., 150 chars) based on testing.
- **Cache Busting**: Add a query param to force feed refresh if needed.

