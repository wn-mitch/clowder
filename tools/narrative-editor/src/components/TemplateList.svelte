<script lang="ts">
  import {
    selectedFile, selectedFileName, selectedTemplateIndex,
    addTemplate, deleteTemplate, duplicateTemplate,
  } from '../stores/templates'
  import type { NarrativeTemplate } from '../lib/types'

  let searchQuery = $state('')

  function conditionChips(t: NarrativeTemplate): string[] {
    const chips: string[] = []
    if (t.action) chips.push(t.action)
    if (t.event) chips.push(`event:${t.event}`)
    if (t.day_phase) chips.push(t.day_phase)
    if (t.season) chips.push(t.season)
    if (t.weather) chips.push(t.weather)
    if (t.mood) chips.push(t.mood)
    if (t.life_stage) chips.push(t.life_stage)
    if (t.terrain) chips.push(t.terrain)
    if (t.has_target !== undefined) chips.push(t.has_target ? 'has target' : 'no target')
    for (const p of t.personality) chips.push(`${p.axis}:${p.bucket}`)
    for (const n of t.needs) chips.push(`${n.axis}:${n.level}`)
    return chips
  }

  function truncate(text: string, len: number): string {
    return text.length > len ? text.slice(0, len) + '\u2026' : text
  }

  function tierColor(tier: string): string {
    switch (tier) {
      case 'Micro': return 'text-muted'
      case 'Action': return 'text-accent'
      case 'Significant': return 'text-warning'
      case 'Danger': return 'text-negative'
      case 'Nature': return 'text-positive'
      default: return 'text-muted'
    }
  }

  let filteredTemplates = $derived.by(() => {
    if (!$selectedFile) return []
    if (!searchQuery.trim()) return $selectedFile.templates.map((t, i) => ({ t, i }))
    const q = searchQuery.toLowerCase()
    return $selectedFile.templates
      .map((t, i) => ({ t, i }))
      .filter(({ t }) =>
        t.text.toLowerCase().includes(q) ||
        (t.action?.toLowerCase().includes(q) ?? false) ||
        (t.event?.toLowerCase().includes(q) ?? false)
      )
  })
</script>

{#if $selectedFile}
  <div class="flex flex-col h-full">
    <div class="mb-3">
      <h3 class="text-base m-0 mb-2">{$selectedFile.name} <span class="font-normal text-muted text-sm">({$selectedFile.templates.length})</span></h3>
      <input
        type="text"
        placeholder="Search templates..."
        bind:value={searchQuery}
        class="w-full text-sm"
      >
    </div>

    <div class="flex-1 overflow-y-auto flex flex-col gap-2 pr-1">
      {#each filteredTemplates as { t, i } (i)}
        <div
          class="bg-surface border border-border rounded-md p-3 cursor-pointer transition-colors hover:border-accent {$selectedTemplateIndex === i ? 'border-accent border-l-[3px] bg-surface-alt' : ''}"
          role="button"
          tabindex="0"
          onclick={() => $selectedTemplateIndex = i}
          onkeydown={(e) => e.key === 'Enter' && ($selectedTemplateIndex = i)}
        >
          <div class="flex gap-2 items-center mb-1">
            <span class="text-[0.7rem] font-bold uppercase tracking-wide {tierColor(t.tier)}">{t.tier}</span>
            {#if t.weight !== 1.0}
              <span class="text-[0.7rem] text-muted font-mono">w:{t.weight}</span>
            {/if}
          </div>
          <p class="text-sm my-1 leading-snug">{truncate(t.text, 80)}</p>
          {#if conditionChips(t).length > 0}
            <div class="flex flex-wrap gap-1 mt-1.5">
              {#each conditionChips(t) as chip}
                <span class="inline-block px-1.5 py-0.5 text-[0.65rem] rounded-sm bg-bg text-muted font-mono">{chip}</span>
              {/each}
            </div>
          {/if}
          <div class="flex gap-1 mt-1.5 justify-end">
            <button
              class="px-2 py-0.5 text-[0.7rem] border border-border bg-transparent text-muted rounded-sm cursor-pointer hover:border-accent hover:text-accent hover:bg-transparent"
              title="Duplicate"
              onclick={(e: MouseEvent) => { e.stopPropagation(); $selectedFileName && duplicateTemplate($selectedFileName, i) }}
            >Dup</button>
            <button
              class="px-2 py-0.5 text-[0.7rem] border border-border bg-transparent text-muted rounded-sm cursor-pointer hover:border-negative hover:text-negative hover:bg-transparent"
              title="Delete"
              onclick={(e: MouseEvent) => { e.stopPropagation(); $selectedFileName && deleteTemplate($selectedFileName, i) }}
            >Del</button>
          </div>
        </div>
      {/each}
    </div>

    <button
      class="mt-3 w-full py-2 text-sm"
      onclick={() => $selectedFileName && addTemplate($selectedFileName)}
    >
      + Add Template
    </button>
  </div>
{:else}
  <div class="flex items-center justify-center h-50 text-muted italic">
    <p>Select a file to view its templates</p>
  </div>
{/if}
