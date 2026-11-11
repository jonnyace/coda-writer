<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { api, type Note, type NoteSummary } from './lib/api'
  import Editor from './lib/Editor.svelte'

  const SAVE_DELAY_MS = 500

  let folders = $state<string[]>([])
  /** `null` shows every note; `''` is the top-level folder. */
  let folder = $state<string | null>(null)
  let notes = $state<NoteSummary[]>([])
  let current = $state<Note | null>(null)
  let title = $state('')
  let loadToken = $state(0)
  let error = $state('')
  let notice = $state('')
  let newFolderName = $state<string | null>(null)

  let titleInput = $state<HTMLInputElement>()
  let editor = $state<Editor>()

  // Edits not yet written to disk. `null` body means only the title changed.
  let dirty = false
  let pendingBody: string | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let saving: Promise<void> | null = null

  onMount(() => {
    void refresh()
    const unlisten = api.onChanged(() => void onExternalChange())
    const flushNow = () => void flush()
    window.addEventListener('blur', flushNow)
    return () => {
      window.removeEventListener('blur', flushNow)
      void unlisten.then((stop) => stop())
    }
  })

  async function guard<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await work()
      error = ''
      return result
    } catch (e) {
      error = String(e)
    }
  }

  async function refresh() {
    await guard(async () => {
      folders = await api.folders()
      if (folder && !folders.includes(folder)) folder = null
      notes = await api.notes(folder)
    })
  }

  async function selectFolder(next: string | null) {
    folder = next
    await refresh()
  }

  async function open(path: string) {
    if (current?.path === path) return
    await flush()
    await guard(async () => {
      current = await api.read(path)
      title = current.title
      loadToken += 1
      notice = ''
    })
  }

  async function newNote() {
    await flush()
    const created = await guard(() => api.create(folder ?? ''))
    if (!created) return
    await refresh()
    await open(created.path)
    await tick()
    titleInput?.focus()
  }

  function edited(body?: string) {
    if (body !== undefined) pendingBody = body
    dirty = true
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void flush(), SAVE_DELAY_MS)
  }

  /** Write pending edits. Saves run one at a time so a rename cannot race a later save. */
  async function flush(): Promise<void> {
    clearTimeout(saveTimer)
    while (saving) await saving
    if (!dirty || !current) return
    const note = current
    const body = pendingBody ?? note.body
    const savedTitle = title
    dirty = false
    pendingBody = null
    saving = (async () => {
      try {
        const summary = await api.save(note.path, savedTitle, body)
        error = ''
        if (current === note) current = { ...summary, body }
        notes = await api.notes(folder)
        if (summary.published) {
          // The note is safe on disk either way; only the blog copy is stale.
          api.pushPublished(summary.path).catch((e) => (error = `Saved, but the blog was not updated: ${e}`))
        }
      } catch (e) {
        // Keep the edit queued so the next keystroke or blur retries it.
        dirty = true
        pendingBody ??= body
        error = `Could not save "${savedTitle || 'Untitled'}": ${e}`
      } finally {
        saving = null
      }
    })()
    await saving
  }

  /** Files changed on disk: our own save, or an edit from another program. */
  async function onExternalChange() {
    await refresh()
    if (!current || dirty || saving) return
    const open = current
    try {
      const fresh = await api.read(open.path)
      if (current !== open || dirty) return
      if (fresh.body !== open.body || fresh.title !== open.title) {
        current = fresh
        title = fresh.title
        loadToken += 1
      }
    } catch {
      // The open note was moved or deleted outside the app.
      if (current === open && !dirty) current = null
    }
  }

  async function togglePin() {
    if (!current) return
    await flush()
    const note = current
    const summary = await guard(() => api.setPinned(note.path, !note.pinned))
    if (summary && current === note) current = { ...summary, body: note.body }
    await refresh()
  }

  async function togglePublished() {
    if (!current) return
    await flush()
    const note = current
    const result = await guard(() => api.setPublished(note.path, !note.published))
    if (!result) return
    if (current === note) current = { ...result.note, body: note.body }
    notice = result.url ? `Published at ${result.url}` : 'Unpublished'
    await refresh()
  }

  async function trash() {
    if (!current) return
    clearTimeout(saveTimer)
    while (saving) await saving
    dirty = false
    pendingBody = null
    const path = current.path
    const index = notes.findIndex((n) => n.path === path)
    if ((await guard(() => api.trash(path).then(() => true))) !== true) return
    current = null
    await refresh()
    const next = notes[Math.min(index, notes.length - 1)]
    if (next) await open(next.path)
  }

  async function addFolder() {
    const name = newFolderName?.trim()
    newFolderName = null
    if (!name) return
    const parent = folder ? `${folder}/` : ''
    const created = await guard(() => api.createFolder(parent + name))
    if (created) await selectFolder(created)
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && !event.shiftKey && event.key.toLowerCase() === 'n') {
      event.preventDefault()
      void newNote()
    }
  }

  function onTitleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === 'ArrowDown') {
      event.preventDefault()
      editor?.focus()
    }
  }

  const time = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' })
  const day = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })
  const full = new Intl.DateTimeFormat(undefined, { year: 'numeric', month: 'short', day: 'numeric' })

  function when(iso: string): string {
    const date = new Date(iso)
    const now = new Date()
    if (date.toDateString() === now.toDateString()) return time.format(date)
    if (date.getFullYear() === now.getFullYear()) return day.format(date)
    return full.format(date)
  }

  const folderLabel = (path: string) => path.split('/').pop() ?? path
  const folderDepth = (path: string) => path.split('/').length - 1
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <nav class="folders" aria-label="Folders">
    <ul>
      <li>
        <button class:selected={folder === null} onclick={() => selectFolder(null)}>All notes</button>
      </li>
      <li>
        <button class:selected={folder === ''} onclick={() => selectFolder('')}>Notes</button>
      </li>
      {#each folders as path (path)}
        <li>
          <button
            class:selected={folder === path}
            style:padding-left="{14 + (folderDepth(path) + 1) * 14}px"
            title={path}
            onclick={() => selectFolder(path)}
          >
            {folderLabel(path)}
          </button>
        </li>
      {/each}
    </ul>
    {#if newFolderName === null}
      <button class="quiet" onclick={() => (newFolderName = '')}>New folder</button>
    {:else}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="folder-name"
        aria-label="Folder name"
        placeholder="Folder name"
        autofocus
        bind:value={newFolderName}
        onblur={addFolder}
        onkeydown={(e) => {
          if (e.key === 'Enter') void addFolder()
          if (e.key === 'Escape') newFolderName = null
        }}
      />
    {/if}
  </nav>

  <section class="list" aria-label="Notes">
    <header>
      <h1>{folder === null ? 'All notes' : folder === '' ? 'Notes' : folderLabel(folder)}</h1>
      <button class="new" onclick={newNote} title="New note (Ctrl+N)">New note</button>
    </header>
    {#if notes.length === 0}
      <p class="empty">No notes here yet. Press Ctrl+N to write one.</p>
    {:else}
      <ul>
        {#each notes as note (note.path)}
          <li>
            <button class:selected={current?.path === note.path} onclick={() => open(note.path)}>
              <span class="note-title">
                {#if note.pinned}<span class="pin" title="Pinned" aria-label="Pinned"></span>{/if}
                {note.title || 'Untitled'}
              </span>
              <span class="note-meta">
                <time datetime={note.updated}>{when(note.updated)}</time>
                <span class="snippet">{note.snippet}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <main class="page">
    {#if current}
      <div class="tools">
        <span class="path" title={current.path}>
          {current.path}{#if current.published}<span class="published">Published</span>{/if}
        </span>
        <button class="quiet" aria-pressed={current.pinned} onclick={togglePin}>
          {current.pinned ? 'Unpin' : 'Pin'}
        </button>
        <button class="quiet" aria-pressed={current.published} onclick={togglePublished}>
          {current.published ? 'Unpublish' : 'Publish'}
        </button>
        <button class="quiet" onclick={trash}>Move to trash</button>
      </div>
      <div class="sheet">
        <input
          class="title"
          aria-label="Title"
          placeholder="Untitled"
          bind:this={titleInput}
          bind:value={title}
          oninput={() => edited()}
          onkeydown={onTitleKeydown}
        />
        <Editor bind:this={editor} body={current.body} {loadToken} onchange={edited} />
      </div>
    {:else}
      <p class="empty">Choose a note, or press Ctrl+N to start a new one.</p>
    {/if}
    {#if error}
      <p class="error" role="alert">{error}</p>
    {:else if notice}
      <p class="error notice" role="status">{notice}</p>
    {/if}
  </main>
</div>
