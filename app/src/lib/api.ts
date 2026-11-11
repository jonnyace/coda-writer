// Thin typed wrapper over the Rust commands in src-tauri/src/lib.rs.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface NoteSummary {
  /** Path relative to the notes folder, e.g. `work/meeting.md`. */
  path: string
  folder: string
  id: string | null
  title: string
  snippet: string
  /** Day the note was written, `YYYY-MM-DD`. */
  date: string
  updated: string
  pinned: boolean
  published: boolean
}

export interface Note extends NoteSummary {
  body: string
}

export const api = {
  notesRoot: () => invoke<string>('notes_root'),
  folders: () => invoke<string[]>('list_folders'),
  /** `null` lists every note; `''` lists notes in the top-level folder. */
  notes: (folder: string | null) => invoke<NoteSummary[]>('list_notes', { folder }),
  /** Notes in any folder containing every word of `query`. */
  search: (query: string) => invoke<NoteSummary[]>('search_notes', { query }),
  read: (path: string) => invoke<Note>('read_note', { path }),
  create: (folder: string) => invoke<NoteSummary>('create_note', { folder }),
  save: (path: string, title: string, body: string) =>
    invoke<NoteSummary>('save_note', { path, title, body }),
  setPinned: (path: string, value: boolean) =>
    invoke<NoteSummary>('set_note_flag', { path, key: 'pinned', value }),
  /** Publish to the blog or take down. `url` is the post's address when published. */
  setPublished: (path: string, value: boolean) =>
    invoke<{ note: NoteSummary; url: string | null }>('set_published', { path, value }),
  /** Update the blog copy of a published note; does nothing for private notes. */
  pushPublished: (path: string) => invoke<string | null>('push_published', { path }),
  trash: (path: string) => invoke<void>('trash_note', { path }),
  createFolder: (name: string) => invoke<string>('create_folder', { name }),
  /** Fires when files in the notes folder change, including edits made outside the app. */
  onChanged: (handler: () => void) => listen('notes-changed', handler),
}
