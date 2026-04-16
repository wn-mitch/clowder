<script lang="ts">
  import { onMount } from 'svelte'
  import {
    files, selectedFileName,
    importFiles, exportFile, removeFile,
    selectedTemplateIndex,
    loadFromGithub, loadingFromGithub, githubError,
  } from '../stores/templates'

  let dragOver = $state(false)
  let fileInput: HTMLInputElement

  onMount(() => {
    // Auto-load from GitHub if no files are loaded
    if ($files.size === 0) {
      loadFromGithub()
    }
  })

  function handleDrop(e: DragEvent) {
    e.preventDefault()
    dragOver = false
    if (e.dataTransfer?.files) {
      importFiles(e.dataTransfer.files)
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault()
    dragOver = true
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement
    if (input.files) {
      importFiles(input.files)
      input.value = ''
    }
  }

  function selectFile(name: string) {
    $selectedFileName = name
    $selectedTemplateIndex = null
  }
</script>

<div class="flex flex-col gap-4">
  {#if $loadingFromGithub}
    <div class="border border-border rounded-md p-6 text-center">
      <p class="text-sm text-muted">Loading templates from GitHub...</p>
    </div>
  {:else if $githubError && $files.size === 0}
    <div class="border border-negative rounded-md p-4 text-center">
      <p class="text-sm text-negative mb-2">Failed to load from GitHub</p>
      <p class="text-xs text-muted mb-3">{$githubError}</p>
      <div class="flex gap-2 justify-center">
        <button onclick={() => loadFromGithub()}>Retry</button>
        <button onclick={() => fileInput.click()}>Browse Local Files</button>
      </div>
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="border-2 border-dashed rounded-md p-4 text-center transition-colors {dragOver ? 'border-accent bg-surface-alt' : 'border-border'}"
      ondrop={handleDrop}
      ondragover={handleDragOver}
      ondragleave={() => dragOver = false}
    >
      <p class="my-0.5 text-xs text-muted">Drop local <code class="text-accent">.ron</code> files to add</p>
      <button class="text-xs px-2 py-1 mt-1" onclick={() => fileInput.click()}>Browse</button>
    </div>
  {/if}

  <input
    bind:this={fileInput}
    type="file"
    accept=".ron"
    multiple
    style="display: none"
    onchange={handleFileSelect}
  >

  {#if $files.size > 0}
    <div>
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm m-0">Files ({$files.size})</h3>
        <button
          class="text-xs px-2 py-0.5 border-none bg-transparent text-muted cursor-pointer hover:text-accent hover:bg-transparent"
          title="Reload from GitHub"
          onclick={() => loadFromGithub()}
        >&#x21bb;</button>
      </div>
      {#each [...$files.entries()].sort((a, b) => a[0].localeCompare(b[0])) as [name, file]}
        <div
          class="flex items-center justify-between px-3 py-2 rounded-md cursor-pointer transition-colors hover:bg-surface-alt {$selectedFileName === name ? 'bg-surface border-l-[3px] border-accent' : ''}"
          role="button"
          tabindex="0"
          onclick={() => selectFile(name)}
          onkeydown={(e) => e.key === 'Enter' && selectFile(name)}
        >
          <div class="flex flex-col min-w-0">
            <span class="text-sm font-bold whitespace-nowrap overflow-hidden text-ellipsis">
              {name}
              {#if file.dirty}<span class="text-warning font-bold">*</span>{/if}
            </span>
            <span class="text-xs text-muted">{file.templates.length} templates</span>
          </div>
          <div class="flex gap-1 shrink-0">
            <button
              class="p-1 text-sm border-none bg-transparent text-muted rounded-sm cursor-pointer hover:bg-surface-alt hover:text-accent"
              title="Export"
              onclick={(e: MouseEvent) => { e.stopPropagation(); exportFile(name) }}
            >
              &#x2913;
            </button>
            <button
              class="p-1 text-sm border-none bg-transparent text-muted rounded-sm cursor-pointer hover:bg-surface-alt hover:text-negative"
              title="Remove"
              onclick={(e: MouseEvent) => { e.stopPropagation(); removeFile(name) }}
            >
              &times;
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
