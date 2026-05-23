<script lang="ts">
  import { onMount } from 'svelte'
  import { parse as parseToml, stringify as stringifyToml } from 'smol-toml'
  import AtlasGrid from '../components/AtlasGrid.svelte'

  // ---------------------------------------------------------------------
  // Manifest schema — mirrors src/rendering/sprite_bindings.rs (ticket 448).
  // Phase 4 (ticket 449): click-to-repick + write-back via POST.
  // ---------------------------------------------------------------------

  // Items bind either to an atlas grid cell (the original Sprout Lands
  // form — `atlas + index`) or to a single PNG file on disk (the
  // Fan-tasy props form — `texture`). Discriminated by which keys are
  // present in the TOML table; matches the untagged Rust enum in
  // src/rendering/sprite_bindings.rs::ItemBinding.
  interface AtlasItemBinding {
    atlas: string
    index: number
    note?: string
  }
  interface TextureItemBinding {
    texture: string
    note?: string
  }
  type ItemBinding = AtlasItemBinding | TextureItemBinding

  function isAtlasItem(b: ItemBinding): b is AtlasItemBinding {
    return 'atlas' in b && 'index' in b
  }
  function isTextureItem(b: ItemBinding): b is TextureItemBinding {
    return 'texture' in b
  }
  interface PlantBinding {
    atlas: string
    indices_by_stage: [number, number, number, number]
    note?: string
  }
  interface BuildingBinding {
    textures: string[]
    native_size: [number, number]
    tiles_wide: number
    note?: string
  }
  interface AtlasInfo {
    texture: string
    cols: number
    rows: number
    tile: number
    note?: string
  }
  interface BindingsFile {
    atlases: Record<string, AtlasInfo>
    items: Record<string, ItemBinding>
    herbs: Record<string, PlantBinding>
    flavor_plants: Record<string, PlantBinding>
    buildings: Record<string, BuildingBinding>
    buildings_winter?: Record<string, BuildingBinding>
  }

  const STAGE_NAMES = ['Sprout', 'Bud', 'Bloom', 'Blossom'] as const

  let bindings = $state<BindingsFile | null>(null)
  let error = $state<string | null>(null)
  let category = $state<'items' | 'buildings' | 'herbs' | 'flavor_plants'>('items')

  // ---------------------------------------------------------------------
  // Selection + side panel — what the user clicked to begin repicking.
  // ---------------------------------------------------------------------

  type Selection =
    | { kind: 'item'; key: string }
    | { kind: 'herb'; key: string; stage: 0 | 1 | 2 | 3 }
    | { kind: 'flavor'; key: string; stage: 0 | 1 | 2 | 3 }
    | { kind: 'building'; key: string; winter: boolean; variant: number }

  let selection = $state<Selection | null>(null)
  let dirty = $state(false)
  let saveStatus = $state<'idle' | 'saving' | 'saved' | 'error'>('idle')
  let saveMessage = $state<string>('')
  let pngList = $state<string[]>([])
  let pngFilter = $state<string>('')

  async function loadBindings() {
    error = null
    try {
      const res = await fetch('/assets/sprites/bindings.toml', { cache: 'no-store' })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const text = await res.text()
      bindings = parseToml(text) as unknown as BindingsFile
      dirty = false
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  async function loadPngList() {
    if (pngList.length > 0) return
    try {
      const res = await fetch('/api/sprite-assets/png')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json() as { paths: string[] }
      pngList = data.paths
    } catch {
      pngList = []
    }
  }

  onMount(loadBindings)

  // ---------------------------------------------------------------------
  // Save — POST current bindings back to the dev server, which writes
  // assets/sprites/bindings.toml. The Bevy hot-reload watcher picks the
  // change up within ~0.5s.
  // ---------------------------------------------------------------------

  async function save() {
    if (!bindings) return
    saveStatus = 'saving'
    saveMessage = ''
    try {
      const body = stringifyToml(bindings as unknown as Record<string, unknown>)
      const res = await fetch('/api/sprite-bindings', {
        method: 'POST',
        headers: { 'Content-Type': 'text/plain' },
        body,
      })
      if (!res.ok) {
        const j = await res.json().catch(() => ({ error: `HTTP ${res.status}` }))
        throw new Error(j.error ?? 'save failed')
      }
      dirty = false
      saveStatus = 'saved'
      const now = new Date()
      saveMessage = `saved at ${now.toLocaleTimeString()}`
      setTimeout(() => {
        if (saveStatus === 'saved') saveStatus = 'idle'
      }, 3000)
    } catch (e) {
      saveStatus = 'error'
      saveMessage = e instanceof Error ? e.message : String(e)
    }
  }

  // ---------------------------------------------------------------------
  // Repick handlers
  // ---------------------------------------------------------------------

  function repickIndex(newIdx: number) {
    if (!bindings || !selection) return
    if (selection.kind === 'item') {
      const b = bindings.items[selection.key]
      // Texture-form items have no index to repick; the side panel
      // shows the PNG picker instead, so this path is unreachable
      // for them in normal use.
      if (isAtlasItem(b)) b.index = newIdx
    } else if (selection.kind === 'herb') {
      bindings.herbs[selection.key].indices_by_stage[selection.stage] = newIdx
    } else if (selection.kind === 'flavor') {
      bindings.flavor_plants[selection.key].indices_by_stage[selection.stage] = newIdx
    }
    dirty = true
  }

  /**
   * Swap which atlas an item / herb / flavor entry uses. Index clamps to
   * 0 because the old index almost certainly doesn't survive a different
   * atlas's grid layout. The user picks a new cell from the new atlas's
   * grid immediately afterward.
   */
  function switchAtlas(newAtlas: string) {
    if (!bindings || !selection) return
    // Items can opt out of atlas form entirely via the synthetic
    // `[single PNG]` option — converts the binding to texture form.
    if (newAtlas === TEXTURE_FORM_SENTINEL && selection.kind === 'item') {
      convertItemBindingForm(true)
      return
    }
    if (!bindings.atlases[newAtlas]) return
    if (selection.kind === 'item') {
      const b = bindings.items[selection.key]
      // Texture-form items have no atlas to switch; ignore in this
      // path (the side panel doesn't surface this control for them).
      if (!isAtlasItem(b)) return
      if (b.atlas === newAtlas) return
      b.atlas = newAtlas
      b.index = 0
    } else if (selection.kind === 'herb') {
      const b = bindings.herbs[selection.key]
      if (b.atlas === newAtlas) return
      b.atlas = newAtlas
      b.indices_by_stage = [0, 0, 0, 0]
    } else if (selection.kind === 'flavor') {
      const b = bindings.flavor_plants[selection.key]
      if (b.atlas === newAtlas) return
      b.atlas = newAtlas
      b.indices_by_stage = [0, 0, 0, 0]
    }
    dirty = true
  }

  function repickBuildingTexture(path: string) {
    if (!bindings || !selection || selection.kind !== 'building') return
    const table = selection.winter ? bindings.buildings_winter : bindings.buildings
    if (!table) return
    const b = table[selection.key]
    if (!b) return
    b.textures[selection.variant] = path
    dirty = true
  }

  /**
   * Repoint a texture-form item at a different PNG path. Mirrors
   * `repickBuildingTexture` — the side panel shows the same PNG
   * picker UI for texture items as it does for building variants.
   */
  function repickItemTexture(path: string) {
    if (!bindings || !selection || selection.kind !== 'item') return
    const b = bindings.items[selection.key]
    if (!isTextureItem(b)) return
    b.texture = path
    dirty = true
  }

  /**
   * Sentinel value in the atlas <select> that doesn't correspond to a
   * registered grid atlas — picking it converts an atlas-form item
   * binding into a single-PNG texture binding (Fan-tasy props or any
   * other standalone sprite). Atlas keys are user-defined but can't
   * shadow this because the leading bracket is invalid in TOML keys.
   */
  const TEXTURE_FORM_SENTINEL = '[single PNG]'

  /**
   * Convert an atlas-form item binding to texture form (or vice versa).
   * Called from the side-panel atlas dropdown when the user picks the
   * `[single PNG]` sentinel, or from the "switch to atlas" button on
   * the texture-form side panel. Picks a sensible default for the new
   * form's required fields; the user finalises in the picker that
   * appears next.
   */
  function convertItemBindingForm(toTexture: boolean) {
    if (!bindings || !selection || selection.kind !== 'item') return
    const current = bindings.items[selection.key]
    const note = current.note
    if (toTexture && isAtlasItem(current)) {
      bindings.items[selection.key] = { texture: '', ...(note ? { note } : {}) }
      loadPngList()
    } else if (!toTexture && isTextureItem(current)) {
      // Default to the `items` atlas (the original Sprout Lands sheet)
      // at index 0. The user picks a real cell from the grid next.
      bindings.items[selection.key] = {
        atlas: 'items',
        index: 0,
        ...(note ? { note } : {}),
      }
    } else {
      return
    }
    dirty = true
  }

  function currentSelectionAtlas(): string | null {
    if (!selection || !bindings) return null
    if (selection.kind === 'item') {
      const b = bindings.items[selection.key]
      return b && isAtlasItem(b) ? b.atlas : null
    }
    if (selection.kind === 'herb') return bindings.herbs[selection.key]?.atlas ?? null
    if (selection.kind === 'flavor') return bindings.flavor_plants[selection.key]?.atlas ?? null
    return null
  }

  function currentSelectionIndex(): number | null {
    if (!selection || !bindings) return null
    if (selection.kind === 'item') {
      const b = bindings.items[selection.key]
      return b && isAtlasItem(b) ? b.index : null
    }
    if (selection.kind === 'herb')
      return bindings.herbs[selection.key]?.indices_by_stage[selection.stage] ?? null
    if (selection.kind === 'flavor')
      return bindings.flavor_plants[selection.key]?.indices_by_stage[selection.stage] ?? null
    return null
  }

  function selectionLabel(): string {
    if (!selection) return ''
    if (selection.kind === 'item') return `item · ${selection.key}`
    if (selection.kind === 'herb') return `herb · ${selection.key} · ${STAGE_NAMES[selection.stage]}`
    if (selection.kind === 'flavor')
      return `flavor · ${selection.key} · ${STAGE_NAMES[selection.stage]}`
    return `building · ${selection.key} · v${selection.variant}${selection.winter ? ' (winter)' : ''}`
  }

  // ---------------------------------------------------------------------
  // CSS helpers — atlas-cell crop + building preview
  // ---------------------------------------------------------------------

  function atlasSrc(meta: AtlasInfo): string {
    return `/assets/${encodeURI(meta.texture)}`
  }

  function atlasCellStyle(atlas: string, index: number, render: number): string {
    const meta = bindings?.atlases?.[atlas]
    if (!meta) return `background:#400; width:${render}px; height:${render}px;`
    const col = index % meta.cols
    const row = Math.floor(index / meta.cols)
    const scale = render / meta.tile
    const bgW = meta.cols * meta.tile * scale
    const bgH = meta.rows * meta.tile * scale
    return [
      `background-image: url("${atlasSrc(meta)}");`,
      `background-size: ${bgW}px ${bgH}px;`,
      `background-position: -${col * meta.tile * scale}px -${row * meta.tile * scale}px;`,
      `width: ${render}px;`,
      `height: ${render}px;`,
      `image-rendering: pixelated;`,
    ].join(' ')
  }

  /**
   * Fit a building's preview into a maxSize × maxSize box, preserving
   * native aspect ratio. Tall sprites (Watchtower 68×149, WardPost 24×59)
   * pin to height = maxSize and compute width proportionally; wide sprites
   * pin to width. Result: every preview occupies a similar visual area
   * regardless of source aspect, instead of WardPost becoming an 80×196
   * sliver next to Den's 80×82 chunk.
   */
  function buildingPreviewStyle(b: BuildingBinding, variantIdx: number, maxSize: number) {
    const sourceW = b.native_size[0]
    const sourceH = b.native_size[1]
    let w: number, h: number
    if (sourceH > sourceW) {
      h = maxSize
      w = (maxSize * sourceW) / sourceH
    } else {
      w = maxSize
      h = (maxSize * sourceH) / sourceW
    }
    const path = `/assets/${b.textures[variantIdx]}`
    return [
      `background-image: url("${encodeURI(path)}");`,
      `background-size: contain;`,
      `background-position: center;`,
      `background-repeat: no-repeat;`,
      `width: ${w}px;`,
      `height: ${h}px;`,
      `image-rendering: pixelated;`,
    ].join(' ')
  }

  /**
   * Filename component of a texture path, useful as a sub-label so the
   * user can identify which PNG a variant points at without expanding
   * the full path. e.g. "House_Hay_1.png" from "new_sprites/.../House_Hay_1.png".
   */
  function textureLabel(path: string): string {
    return path.split('/').pop() ?? path
  }

  let itemEntries = $derived(
    bindings ? Object.entries(bindings.items).sort(([a], [b]) => a.localeCompare(b)) : []
  )
  let herbEntries = $derived(
    bindings ? Object.entries(bindings.herbs).sort(([a], [b]) => a.localeCompare(b)) : []
  )
  let flavorEntries = $derived(
    bindings ? Object.entries(bindings.flavor_plants).sort(([a], [b]) => a.localeCompare(b)) : []
  )
  let buildingEntries = $derived(
    bindings ? Object.entries(bindings.buildings).sort(([a], [b]) => a.localeCompare(b)) : []
  )
  let winterEntries = $derived(
    bindings?.buildings_winter
      ? Object.entries(bindings.buildings_winter).sort(([a], [b]) => a.localeCompare(b))
      : []
  )

  // Filter for the PNG picker — restrict to plausible building/prop paths.
  let filteredPngs = $derived.by(() => {
    if (!pngFilter.trim()) return pngList.slice(0, 200)
    const needle = pngFilter.toLowerCase()
    return pngList.filter((p) => p.toLowerCase().includes(needle)).slice(0, 200)
  })

  // Reverse map of currently-bound atlas indices for the highlighted
  // category — drawn as green outlines on the AtlasGrid so the user can
  // see at a glance which cells the manifest already references.
  let currentNamedCells = $derived.by<Record<number, string>>(() => {
    if (!bindings || !selection) return {}
    const atlas = currentSelectionAtlas()
    if (!atlas) return {}
    const out: Record<number, string> = {}
    if (atlas === 'items') {
      for (const [k, b] of Object.entries(bindings.items)) {
        if (!isAtlasItem(b)) continue
        if (out[b.index]) out[b.index] += `, ${k}`
        else out[b.index] = k
      }
    } else if (atlas === 'herbs') {
      for (const [k, b] of Object.entries(bindings.herbs)) {
        for (let s = 0; s < 4; s++) {
          const idx = b.indices_by_stage[s]
          const tag = `${k} ${STAGE_NAMES[s]}`
          out[idx] = out[idx] ? `${out[idx]}, ${tag}` : tag
        }
      }
      for (const [k, b] of Object.entries(bindings.flavor_plants)) {
        for (let s = 0; s < 4; s++) {
          const idx = b.indices_by_stage[s]
          const tag = `${k} ${STAGE_NAMES[s]}`
          out[idx] = out[idx] ? `${out[idx]}, ${tag}` : tag
        }
      }
    }
    return out
  })
</script>

<div class="flex flex-col gap-4">
  <header class="flex items-baseline gap-4 flex-wrap">
    <h1 class="text-xl font-bold text-accent">Sprite bindings</h1>
    <span class="text-xs text-muted">
      Click any entry to repick from its source atlas. Save → writes to
      <code class="text-txt">assets/sprites/bindings.toml</code> → Bevy
      hot-reload picks it up live (~1s).
    </span>
    <div class="ml-auto flex items-center gap-2">
      {#if dirty}
        <span class="text-xs text-orange-400">unsaved changes</span>
      {/if}
      {#if saveStatus === 'saving'}
        <span class="text-xs text-muted">saving…</span>
      {:else if saveStatus === 'saved'}
        <span class="text-xs text-green-400">{saveMessage}</span>
      {:else if saveStatus === 'error'}
        <span class="text-xs text-red-400" title={saveMessage}>save failed</span>
      {/if}
      <button
        class="px-3 py-1 text-xs border border-accent rounded bg-accent/20 text-accent hover:bg-accent hover:text-bg disabled:opacity-40 disabled:cursor-not-allowed"
        disabled={!dirty || saveStatus === 'saving'}
        onclick={save}
      >
        Save
      </button>
      <button
        class="px-3 py-1 text-xs border border-border rounded bg-surface text-txt hover:bg-surface-alt"
        onclick={loadBindings}
        title="Reload from disk (discards unsaved changes)"
      >
        Reload
      </button>
    </div>
  </header>

  {#if error}
    <div class="border border-red-700 bg-red-950 text-red-200 px-3 py-2 rounded">
      Failed to load bindings.toml: {error}
    </div>
  {/if}

  <nav class="flex gap-1 border-b border-border pb-2">
    {#each ['items', 'buildings', 'herbs', 'flavor_plants'] as const as tab}
      <button
        class="px-3 py-1.5 border-none bg-transparent text-sm rounded cursor-pointer transition-colors {category === tab ? 'text-accent bg-surface' : 'text-muted hover:text-txt hover:bg-surface-alt'}"
        onclick={() => {
          category = tab
          selection = null
        }}
      >
        {tab.replace('_', ' ')}
      </button>
    {/each}
  </nav>

  <div class="flex gap-4 items-start">
    <!-- Gallery (left column) -->
    <div class="flex-1 min-w-0">
      {#if !bindings}
        <p class="text-muted">Loading…</p>
      {:else if category === 'items'}
        <p class="text-xs text-muted mb-2">
          {itemEntries.length} items — atlas-bound use named sheets (e.g. items 8×15), texture-bound point at single PNGs (Fan-tasy props)
        </p>
        <div class="grid grid-cols-2 lg:grid-cols-3 gap-2">
          {#each itemEntries as [name, binding]}
            {@const isSelected = selection?.kind === 'item' && selection.key === name}
            <button
              type="button"
              class="flex items-center gap-3 p-2 border rounded text-left cursor-pointer transition-colors {isSelected ? 'border-accent bg-accent/10' : 'border-border bg-surface hover:bg-surface-alt'}"
              onclick={() => {
                selection = { kind: 'item', key: name }
                if (isTextureItem(binding)) loadPngList()
              }}
            >
              {#if isAtlasItem(binding)}
                <div style={atlasCellStyle(binding.atlas, binding.index, 48)}></div>
              {:else}
                <img
                  src={`/assets/${encodeURI(binding.texture)}`}
                  alt=""
                  class="w-12 h-12 object-contain bg-bg-deep"
                  style="image-rendering: pixelated;"
                  loading="lazy"
                />
              {/if}
              <div class="flex flex-col min-w-0">
                <span class="text-sm text-txt truncate" title={name}>{name}</span>
                {#if isAtlasItem(binding)}
                  <span class="text-xs text-muted">{binding.atlas} · #{binding.index}</span>
                {:else}
                  <span class="text-xs text-muted truncate" title={binding.texture}>
                    {binding.texture.split('/').pop()}
                  </span>
                {/if}
                {#if binding.note}
                  <span class="text-[10px] text-muted truncate" title={binding.note}>{binding.note}</span>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      {:else if category === 'herbs'}
        <p class="text-xs text-muted mb-2">{herbEntries.length} herb species · 4 stages each</p>
        <div class="flex flex-col gap-2">
          {#each herbEntries as [name, binding]}
            <div class="p-2 border border-border rounded bg-surface">
              <div class="flex items-baseline justify-between mb-2">
                <span class="text-sm text-txt">{name}</span>
                {#if binding.note}
                  <span class="text-[10px] text-muted">{binding.note}</span>
                {/if}
              </div>
              <div class="flex gap-2">
                {#each binding.indices_by_stage as idx, stageI}
                  {@const isSelected = selection?.kind === 'herb' && selection.key === name && selection.stage === stageI}
                  <button
                    type="button"
                    class="flex flex-col items-center gap-1 p-1 border rounded cursor-pointer transition-colors {isSelected ? 'border-accent bg-accent/10' : 'border-border bg-bg hover:bg-surface-alt'}"
                    onclick={() => (selection = { kind: 'herb', key: name, stage: stageI as 0 | 1 | 2 | 3 })}
                  >
                    <div style={atlasCellStyle(binding.atlas, idx, 48)}></div>
                    <span class="text-[10px] text-muted">{STAGE_NAMES[stageI]} · #{idx}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      {:else if category === 'flavor_plants'}
        <p class="text-xs text-muted mb-2">{flavorEntries.length} flavor plants · 4 stages each</p>
        <div class="flex flex-col gap-2">
          {#each flavorEntries as [name, binding]}
            <div class="p-2 border border-border rounded bg-surface">
              <div class="flex items-baseline justify-between mb-2">
                <span class="text-sm text-txt">{name}</span>
                {#if binding.note}
                  <span class="text-[10px] text-muted">{binding.note}</span>
                {/if}
              </div>
              <div class="flex gap-2">
                {#each binding.indices_by_stage as idx, stageI}
                  {@const isSelected = selection?.kind === 'flavor' && selection.key === name && selection.stage === stageI}
                  <button
                    type="button"
                    class="flex flex-col items-center gap-1 p-1 border rounded cursor-pointer transition-colors {isSelected ? 'border-accent bg-accent/10' : 'border-border bg-bg hover:bg-surface-alt'}"
                    onclick={() => (selection = { kind: 'flavor', key: name, stage: stageI as 0 | 1 | 2 | 3 })}
                  >
                    <div style={atlasCellStyle(binding.atlas, idx, 48)}></div>
                    <span class="text-[10px] text-muted">{STAGE_NAMES[stageI]} · #{idx}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      {:else if category === 'buildings'}
        <p class="text-xs text-muted mb-2">
          {buildingEntries.length} buildings (summer) · {winterEntries.length} winter variants
        </p>
        <h2 class="text-sm font-bold text-accent mt-2 mb-1">Summer</h2>
        <div class="flex flex-col gap-2">
          {#each buildingEntries as [name, binding]}
            <div class="flex gap-3 p-2 border border-border rounded bg-surface items-start">
              <div class="flex flex-col flex-1 min-w-0">
                <span class="text-sm text-txt font-bold">{name}</span>
                <span class="text-[10px] text-muted">
                  {binding.native_size[0]}×{binding.native_size[1]}px native · {binding.tiles_wide} tiles wide · {binding.textures.length} variant{binding.textures.length === 1 ? '' : 's'}
                </span>
                {#if binding.note}
                  <span class="text-[10px] text-muted mt-1">{binding.note}</span>
                {/if}
              </div>
              <div class="flex gap-2 items-end shrink-0">
                {#each binding.textures as path, vi}
                  {@const isSelected = selection?.kind === 'building' && selection.key === name && !selection.winter && selection.variant === vi}
                  <button
                    type="button"
                    aria-label={`Pick ${name} variant ${vi}`}
                    title={textureLabel(path)}
                    class="flex flex-col items-center gap-1 p-1 border rounded cursor-pointer transition-colors {isSelected ? 'border-accent bg-accent/10' : 'border-border bg-bg hover:bg-surface-alt'}"
                    onclick={() => {
                      selection = { kind: 'building', key: name, winter: false, variant: vi }
                      loadPngList()
                    }}
                  >
                    <div style={buildingPreviewStyle(binding, vi, 128)}></div>
                    <span class="text-[9px] text-muted max-w-[140px] truncate">{textureLabel(path)}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/each}
        </div>

        {#if winterEntries.length > 0}
          <h2 class="text-sm font-bold text-accent mt-4 mb-1">Winter</h2>
          <div class="flex flex-col gap-2">
            {#each winterEntries as [name, binding]}
              <div class="flex gap-3 p-2 border border-border rounded bg-surface items-start">
                <div class="flex flex-col flex-1 min-w-0">
                  <span class="text-sm text-txt font-bold">{name}</span>
                  <span class="text-[10px] text-muted">
                    {binding.native_size[0]}×{binding.native_size[1]}px native · {binding.textures.length} variant{binding.textures.length === 1 ? '' : 's'}
                  </span>
                </div>
                <div class="flex gap-2 items-end shrink-0">
                  {#each binding.textures as path, vi}
                    {@const isSelected = selection?.kind === 'building' && selection.key === name && selection.winter && selection.variant === vi}
                    <button
                      type="button"
                      aria-label={`Pick ${name} winter variant ${vi}`}
                      title={textureLabel(path)}
                      class="flex flex-col items-center gap-1 p-1 border rounded cursor-pointer transition-colors {isSelected ? 'border-accent bg-accent/10' : 'border-border bg-bg hover:bg-surface-alt'}"
                      onclick={() => {
                        selection = { kind: 'building', key: name, winter: true, variant: vi }
                        loadPngList()
                      }}
                    >
                      <div style={buildingPreviewStyle(binding, vi, 128)}></div>
                      <span class="text-[9px] text-muted max-w-[140px] truncate">{textureLabel(path)}</span>
                    </button>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>

    <!-- Side panel (right column) — repick UX -->
    {#if selection && bindings}
      <aside class="w-[520px] sticky top-2 self-start p-3 border border-accent/40 rounded bg-surface">
        <div class="flex items-center justify-between mb-2">
          <span class="text-sm text-accent">{selectionLabel()}</span>
          <button
            class="text-xs text-muted hover:text-txt"
            onclick={() => (selection = null)}
            title="Close panel"
          >
            ✕
          </button>
        </div>

        {#if selection.kind === 'item' && isTextureItem(bindings.items[selection.key])}
          <!-- Texture-form item: PNG-path picker (mirrors building UI). -->
          {@const itemB = bindings.items[selection.key] as TextureItemBinding}
          <div class="flex items-center gap-2 mb-2 text-[11px]">
            <span class="text-muted">form: single PNG (texture)</span>
            <button
              class="ml-auto px-2 py-1 text-[11px] border border-border rounded bg-surface text-txt hover:bg-surface-alt"
              onclick={() => convertItemBindingForm(false)}
              title="Convert this item to an atlas-grid binding"
            >
              ⇄ switch to atlas grid
            </button>
          </div>
          <div class="flex items-center gap-3 mb-2">
            {#if itemB.texture}
              <img
                src={`/assets/${encodeURI(itemB.texture)}`}
                alt=""
                class="w-24 h-24 object-contain bg-bg-deep"
                style="image-rendering: pixelated;"
                loading="lazy"
              />
            {:else}
              <div class="w-24 h-24 bg-bg-deep border border-dashed border-border flex items-center justify-center text-[10px] text-muted">
                pick a PNG →
              </div>
            {/if}
            <div class="flex flex-col min-w-0">
              <span class="text-[11px] text-muted">current:</span>
              <code class="text-[10px] text-txt break-all">{itemB.texture || '(none — pick below)'}</code>
            </div>
          </div>
          <input
            type="text"
            class="w-full mb-2 px-2 py-1 text-xs bg-bg-deep border border-border rounded text-txt"
            placeholder="filter PNG paths (e.g. 'LotusFlower', 'Cattail', 'Basket')"
            bind:value={pngFilter}
          />
          <div class="max-h-[400px] overflow-y-auto border border-border rounded">
            {#if pngList.length === 0}
              <p class="text-xs text-muted p-2">loading PNG list…</p>
            {:else}
              {#each filteredPngs as path}
                {@const isCurrent = path === itemB.texture}
                <button
                  type="button"
                  class="flex items-center gap-2 w-full p-1 text-left border-b border-border last:border-b-0 hover:bg-surface-alt {isCurrent ? 'bg-accent/10' : ''}"
                  onclick={() => repickItemTexture(path)}
                >
                  <img
                    src={`/assets/${encodeURI(path)}`}
                    alt=""
                    class="w-10 h-10 object-contain bg-bg-deep"
                    style="image-rendering: pixelated;"
                    loading="lazy"
                  />
                  <code class="text-[10px] text-txt break-all flex-1">{path}</code>
                  {#if isCurrent}
                    <span class="text-[10px] text-accent">current</span>
                  {/if}
                </button>
              {/each}
              {#if filteredPngs.length === 0}
                <p class="text-xs text-muted p-2">no matches</p>
              {/if}
            {/if}
          </div>
        {:else if selection.kind === 'item' || selection.kind === 'herb' || selection.kind === 'flavor'}
          {@const atlas = currentSelectionAtlas()}
          {@const meta = atlas ? bindings.atlases[atlas] : null}
          {@const idx = currentSelectionIndex()}
          <div class="flex items-center gap-2 mb-2 text-[11px]">
            <span class="text-muted">atlas:</span>
            <select
              class="bg-bg-deep border border-border rounded px-2 py-1 text-txt"
              value={atlas ?? ''}
              onchange={(e) => switchAtlas((e.currentTarget as HTMLSelectElement).value)}
            >
              {#each Object.entries(bindings.atlases) as [name, info]}
                <option value={name}>{name} · {info.cols}×{info.rows}</option>
              {/each}
              {#if selection.kind === 'item'}
                <option disabled>──────────</option>
                <option value={TEXTURE_FORM_SENTINEL}>{TEXTURE_FORM_SENTINEL} (Fan-tasy / standalone PNG)</option>
              {/if}
            </select>
          </div>
          {#if meta?.note}
            <p class="text-[10px] text-muted mb-2 italic">{meta.note}</p>
          {/if}
          {#if meta}
            <p class="text-[11px] text-muted mb-2">
              Click a cell to repick. Green outlines = cells already bound elsewhere; thick accent ring = active selection.
            </p>
            <AtlasGrid
              src={atlasSrc(meta)}
              cols={meta.cols}
              rows={meta.rows}
              tile={meta.tile}
              scale={3}
              highlightedIndex={idx}
              namedCells={currentNamedCells}
              onCellClick={repickIndex}
            />
          {/if}
        {:else if selection.kind === 'building'}
          {@const table = selection.winter ? bindings.buildings_winter : bindings.buildings}
          {@const b = table?.[selection.key]}
          {#if b}
            <div class="flex items-center gap-3 mb-2">
              <div style={buildingPreviewStyle(b, selection.variant, 192)}></div>
              <div class="flex flex-col min-w-0">
                <span class="text-[11px] text-muted">current:</span>
                <code class="text-[10px] text-txt break-all">{b.textures[selection.variant]}</code>
              </div>
            </div>
            <input
              type="text"
              class="w-full mb-2 px-2 py-1 text-xs bg-bg-deep border border-border rounded text-txt"
              placeholder="filter PNG paths (e.g. 'House_', 'Watchtower', 'Snow')"
              bind:value={pngFilter}
            />
            <div class="max-h-[400px] overflow-y-auto border border-border rounded">
              {#if pngList.length === 0}
                <p class="text-xs text-muted p-2">loading PNG list…</p>
              {:else}
                {#each filteredPngs as path}
                  {@const isCurrent = path === b.textures[selection.variant]}
                  <button
                    type="button"
                    class="flex items-center gap-2 w-full p-1 text-left border-b border-border last:border-b-0 hover:bg-surface-alt {isCurrent ? 'bg-accent/10' : ''}"
                    onclick={() => repickBuildingTexture(path)}
                  >
                    <img
                      src={`/assets/${encodeURI(path)}`}
                      alt=""
                      class="w-10 h-10 object-contain bg-bg-deep"
                      style="image-rendering: pixelated;"
                      loading="lazy"
                    />
                    <code class="text-[10px] text-txt break-all flex-1">{path}</code>
                    {#if isCurrent}
                      <span class="text-[10px] text-accent">current</span>
                    {/if}
                  </button>
                {/each}
                {#if filteredPngs.length === 0}
                  <p class="text-xs text-muted p-2">no matches</p>
                {/if}
              {/if}
            </div>
          {/if}
        {/if}
      </aside>
    {/if}
  </div>
</div>
