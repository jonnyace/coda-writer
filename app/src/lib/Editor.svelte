<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { TaskItem, TaskList } from '@tiptap/extension-list'
  import { Placeholder } from '@tiptap/extensions'
  import { Markdown } from '@tiptap/markdown'
  import StarterKit from '@tiptap/starter-kit'
  import { onMount } from 'svelte'

  interface Props {
    /** Markdown to show. Only read when `loadToken` changes. */
    body: string
    /** Bump to replace the editor's content (opening a note, external edit). */
    loadToken: number
    /** `false` shows the note read-only (trashed notes). */
    editable?: boolean
    onchange: (markdown: string) => void
  }
  let { body, loadToken, editable = true, onchange }: Props = $props()

  let element: HTMLDivElement
  let editor: Editor | undefined
  let loaded = -1

  onMount(() => {
    editor = new Editor({
      element,
      extensions: [
        StarterKit,
        TaskList,
        TaskItem.configure({ nested: true }),
        Placeholder.configure({ placeholder: 'Start writing' }),
        Markdown,
      ],
      // Only edits made in the editor reach `onchange`. Loading a note does
      // not, so opening a file never rewrites it.
      onUpdate: ({ editor }) => onchange(editor.getMarkdown()),
    })
    editor.setEditable(editable, false)
    load()
    return () => editor?.destroy()
  })

  function load() {
    if (!editor || loaded === loadToken) return
    loaded = loadToken
    editor.commands.setContent(body, { contentType: 'markdown', emitUpdate: false })
  }

  $effect(() => {
    void loadToken
    load()
  })

  $effect(() => {
    editor?.setEditable(editable, false)
  })

  export function focus() {
    editor?.commands.focus('start')
  }
</script>

<div class="editor" bind:this={element}></div>
