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
  /** The folder-name field under the folder list: making a folder or renaming the selected one. */
  let folderField = $state<{ mode: 'new' | 'rename'; value: string } | null>(null)
  /** Showing the trash instead of a folder. Trashed notes are read-only. */
  let inTrash = $state(false)
  /** Search text. While it is not blank the list shows matches from every folder. */
  let query = $state('')
  const searching = $derived(query.trim() !== '')

  let titleInput = $state<HTMLInputElement>()
  let searchInput = $state<HTMLInputElement>()
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

  const listNotes = () => (inTrash ? api.trashed() : searching ? api.search(query) : api.notes(folder))

  let searchTimer: ReturnType<typeof setTimeout> | undefined
  function onSearchInput() {
    if (inTrash) leaveTrash()
    clearTimeout(searchTimer)
    searchTimer = setTimeout(() => void refresh(), 120)
  }

  function clearSearch() {
    query = ''
    void refresh()
  }

  // Refreshes overlap while typing a search; only the newest may fill the list.
  let refreshCount = 0
  async function refresh() {
    const mine = ++refreshCount
    await guard(async () => {
      folders = await api.folders()
      if (folder && !folders.includes(folder)) folder = null
      const listed = await listNotes()
      if (mine === refreshCount) notes = listed
    })
  }

  function leaveTrash() {
    inTrash = false
    current = null
  }

  async function selectFolder(next: string | null) {
    if (inTrash) leaveTrash()
    query = ''
    folder = next
    await refresh()
  }

  async function open(path: string) {
    if (current?.path === path) return
    await flush()
    await guard(async () => {
      current = inTrash ? await api.readTrashed(path) : await api.read(path)
      title = current.title
      loadToken += 1
      notice = ''
    })
  }

  async function newNote() {
    await flush()
    if (inTrash) leaveTrash()
    query = ''
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
        notes = await listNotes()
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
    if (!current || dirty || saving || inTrash) return
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

  async function showTrash() {
    await flush()
    query = ''
    current = null
    inTrash = true
    await refresh()
  }

  async function restore() {
    if (!current || !inTrash) return
    const path = current.path
    const restored = await guard(() => api.restore(path))
    if (!restored) return
    current = null
    notice = `Restored to ${restored.folder || 'Notes'}`
    await refresh()
  }

  async function moveTo(target: string) {
    if (!current) return
    await flush()
    const note = current
    const moved = await guard(() => api.move(note.path, target))
    if (moved && current === note) current = { ...moved, body: note.body }
    await refresh()
  }

  /** Create a folder inside the selected one, or rename the selected one. */
  async function submitFolderField() {
    const field = folderField
    folderField = null
    const name = field?.value.trim()
    if (!field || !name) return
    if (field.mode === 'new') {
      const parent = folder ? `${folder}/` : ''
      const created = await guard(() => api.createFolder(parent + name))
      if (created) await selectFolder(created)
      return
    }
    const old = folder
    if (!old) return
    await flush()
    const renamed = await guard(() => api.renameFolder(old, name))
    if (!renamed) return
    // The open note's path changed along with its folder.
    if (current && (current.folder === old || current.folder.startsWith(`${old}/`))) {
      const path = renamed + current.path.slice(old.length)
      current = (await guard(() => api.read(path))) ?? null
    }
    folder = renamed
    await refresh()
  }

  async function deleteFolder() {
    const target = folder
    if (!target) return
    await flush()
    const removed = await guard(() => api.deleteFolder(target))
    if (removed === undefined) return
    if (current && (current.folder === target || current.folder.startsWith(`${target}/`))) current = null
    notice = removed
      ? 'Folder deleted. Its notes are in the trash.'
      : 'Notes moved to the trash. The folder still holds other files, so it was kept.'
    folder = null
    await refresh()
  }

  function onKeydown(event: KeyboardEvent) {
    const command = (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey
    if (command && event.key.toLowerCase() === 'n') {
      event.preventDefault()
      void newNote()
    } else if (command && event.key.toLowerCase() === 'f') {
      event.preventDefault()
      searchInput?.focus()
      searchInput?.select()
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
        <button class:selected={!inTrash && folder === null} onclick={() => selectFolder(null)}>All notes</button>
      </li>
      <li>
        <button class:selected={!inTrash && folder === ''} onclick={() => selectFolder('')}>Notes</button>
      </li>
      {#each folders as path (path)}
        <li>
          <button
            class:selected={!inTrash && folder === path}
            style:padding-left="{14 + (folderDepth(path) + 1) * 14}px"
            title={path}
            onclick={() => selectFolder(path)}
          >
            {folderLabel(path)}
          </button>
        </li>
      {/each}
    </ul>
    {#if folderField === null}
      <button class="quiet" onclick={() => (folderField = { mode: 'new', value: '' })}>New folder</button>
      {#if folder && !inTrash}
        <button class="quiet" onclick={() => (folderField = { mode: 'rename', value: folderLabel(folder ?? '') })}>
          Rename folder
        </button>
        <button class="quiet" onclick={deleteFolder} title="Moves the notes in this folder to the trash">
          Delete folder
        </button>
      {/if}
    {:else}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="folder-name"
        aria-label={folderField.mode === 'new' ? 'New folder name' : 'New name for this folder'}
        placeholder="Folder name"
        autofocus
        bind:value={folderField.value}
        onblur={submitFolderField}
        onkeydown={(e) => {
          if (e.key === 'Enter') void submitFolderField()
          if (e.key === 'Escape') folderField = null
        }}
      />
    {/if}
    <button class="quiet trash" class:selected={inTrash} onclick={showTrash}>Trash</button>
  </nav>

  <section class="list" aria-label="Notes">
    <header>
      <h1>{inTrash ? 'Trash' : searching ? 'Search results' : folder === null ? 'All notes' : folder === '' ? 'Notes' : folderLabel(folder)}</h1>
      <button class="new" onclick={newNote} title="New note (Ctrl+N)">New note</button>
    </header>
    <input
      class="search"
      type="search"
      aria-label="Search notes"
      placeholder="Search all notes (Ctrl+F)"
      bind:this={searchInput}
      bind:value={query}
      oninput={onSearchInput}
      onkeydown={(e) => {
        if (e.key === 'Escape') clearSearch()
      }}
    />
    {#if notes.length === 0}
      <p class="empty">
        {inTrash
          ? 'The trash is empty.'
          : searching
            ? `No notes contain "${query.trim()}".`
            : 'No notes here yet. Press Ctrl+N to write one.'}
      </p>
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
        {#if inTrash}
          <button class="quiet" onclick={restore}>Restore</button>
        {:else}
          <select
            class="quiet move"
            aria-label="Move to folder"
            value=""
            onchange={(e) => {
              void moveTo(e.currentTarget.value)
              e.currentTarget.value = ''
            }}
          >
            <option value="" disabled hidden>Move to</option>
            {#if current.folder !== ''}<option value="">Notes</option>{/if}
            {#each folders.filter((f) => f !== current?.folder) as path (path)}
              <option value={path}>{path}</option>
            {/each}
          </select>
          <button class="quiet" aria-pressed={current.pinned} onclick={togglePin}>
            {current.pinned ? 'Unpin' : 'Pin'}
          </button>
          <button class="quiet" aria-pressed={current.published} onclick={togglePublished}>
            {current.published ? 'Unpublish' : 'Publish'}
          </button>
          <button class="quiet" onclick={trash}>Move to trash</button>
        {/if}
      </div>
      <div class="sheet">
        <input
          class="title"
          aria-label="Title"
          placeholder="Untitled"
          bind:this={titleInput}
          bind:value={title}
          readonly={inTrash}
          oninput={() => edited()}
          onkeydown={onTitleKeydown}
        />
        <Editor bind:this={editor} body={current.body} {loadToken} editable={!inTrash} onchange={edited} />
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
