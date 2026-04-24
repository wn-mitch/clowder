<script lang="ts">
  import { loadFiles, loading } from '../../stores/runs'

  let dragActive = $state(false)
  let inputEl = $state<HTMLInputElement | null>(null)

  function onDragOver(e: DragEvent) {
    e.preventDefault()
    dragActive = true
  }

  function onDragLeave(e: DragEvent) {
    // Ignore events bubbling from child elements.
    if (e.currentTarget !== e.target) return
    dragActive = false
  }

  async function onDrop(e: DragEvent) {
    e.preventDefault()
    dragActive = false
    const files = e.dataTransfer?.files
    if (files && files.length > 0) await loadFiles(files)
  }

  async function onInputChange() {
    const files = inputEl?.files
    if (files && files.length > 0) await loadFiles(files)
    if (inputEl) inputEl.value = ''
  }
</script>

<div
  role="region"
  aria-label="Drop simulation log files here"
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
  class="border-2 border-dashed {dragActive ? 'border-accent bg-surface-alt' : 'border-border bg-surface'} rounded-md p-8 text-center transition-colors duration-150"
>
  <p class="m-0 mb-2 text-sm text-muted">
    Drop <code>events.jsonl</code>, <code>narrative.jsonl</code>, or
    <code>trace-&lt;name&gt;.jsonl</code> here, or
  </p>
  <button
    type="button"
    onclick={() => inputEl?.click()}
    disabled={$loading}
    class="text-sm"
  >
    {$loading ? 'Loading…' : 'Choose files…'}
  </button>
  <input
    bind:this={inputEl}
    type="file"
    multiple
    accept=".jsonl,application/json,text/plain"
    onchange={onInputChange}
    class="hidden"
  />
  <p class="m-0 mt-3 text-xs text-muted/70">
    Data stays on your machine — nothing is uploaded.
  </p>
</div>
