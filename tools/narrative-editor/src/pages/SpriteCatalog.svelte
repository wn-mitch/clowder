<script lang="ts">
  // Sprite catalog browser (ticket 449 Phase 5 + polish).
  // Reads atlas registry from assets/sprites/bindings.toml — every
  // [atlases.<name>] entry becomes a tab. Replaces the standalone
  // `tools/sprite_catalog.html` (1304 LoC) with a Svelte page that stays
  // in sync with the manifest: declare a new atlas in TOML and it shows
  // up here automatically, no code edit needed.

  import { onMount } from 'svelte'
  import { parse as parseToml } from 'smol-toml'
  import AtlasGrid from '../components/AtlasGrid.svelte'

  interface AtlasInfo {
    texture: string
    cols: number
    rows: number
    tile: number
    note?: string
  }
  interface BindingsLite {
    atlases: Record<string, AtlasInfo>
  }

  let bindings = $state<BindingsLite | null>(null)
  let error = $state<string | null>(null)
  let activeSheet = $state<string>('items')
  let selectedIdx = $state<number | null>(null)

  // Named-cell mappings — lifted from the EXISTING table in the original
  // sprite_catalog.html. Reflects sprite identifiers in the source Sprout
  // Lands pack, not what the game binds (those are in bindings.toml,
  // visible via #/sprites' green outlines).
  const NAMED: Record<string, Record<number, string>> = {
    items: {
      0: 'RawMouse / RawBird (drumstick)',
      1: 'RawRat / RawRabbit (meat cut)',
      5: 'Moss / DriedGrass / Feather (leaf)',
      7: 'Mushroom',
      11: 'GRASS',
      2: 'AXE',
      3: 'HOE',
      4: 'EMPTYJAR',
      6: 'LARGEEMPTYJAR',
      8: 'CORNPACKET',
      9: 'CORN',
      10: 'WATERINGCAN',
      16: 'CARROTPACKET',
      17: 'CARROT',
      18: 'STICK',
      19: 'LOG',
      24: 'CAULIFLOWERPACKET',
      25: 'CAULIFLOWER',
      26: 'STUMP',
      27: 'STICK_27',
      32: 'TOMATOPACKET',
      33: 'TOMATO',
      34: 'PLANK',
      35: 'STONEBRICK',
      40: 'EGGPLANTPACKET',
      41: 'EGGPLANT',
      42: 'STONE',
      43: 'ROCK',
      48: 'ROSEPACKET',
      49: 'ROSE',
      50: 'APPLE',
      56: 'LETTUCEPACKET',
      57: 'LETTUCE',
      58: 'LEMON',
      64: 'WHEATPACKET',
      65: 'WHEAT',
      66: 'PEAR',
      72: 'PUMPKINPACKET',
      73: 'PUMPKIN',
      74: 'PEACH',
      80: 'TURNIPPACKET',
      81: 'TURNIP',
      82: 'STRAWBERRY',
      88: 'BROCOLIPACKET',
      89: 'BROCOLI',
      90: 'GRAPE',
      91: 'WHITEEGG',
      92: 'BLACKEGG',
      93: 'PINKEGG',
      94: 'GREENEGG',
      95: 'BLUEEGG',
      96: 'RADISHPACKET',
      97: 'RADISH',
      98: 'PLUM',
      104: 'STARFRUITPACKET',
      105: 'STARFRUIT',
      112: 'PEAPACKET',
      113: 'PEA',
      12: 'WHITEJAR',
      13: 'WHITEJARSPECIAL',
      14: 'LARGEWHITEJAR',
      15: 'LARGEWHITEJARSPECIAL',
      28: 'BROWNJAR',
      29: 'BROWNJARSPECIAL',
      30: 'LARGEBROWNJAR',
      31: 'LARGEBROWNJARSPECIAL',
      44: 'PURPLEJAR',
      45: 'PURPLEJARSPECIAL',
      46: 'LARGEPURPLEJAR',
      47: 'LARGEPURPLEJARSPECIAL',
      60: 'PINKJAR',
      61: 'PINKJARSPECIAL',
      62: 'LARGEPINKJAR',
      63: 'LARGEPINKJARSPECIAL',
      76: 'GREENJAR',
      77: 'GREENJARSPECIAL',
      78: 'LARGEGREENJAR',
      79: 'LARGEGREENJARSPECIAL',
      20: 'WHITEBOTTLE',
      21: 'WHITEBOTTLESPECIAL',
      22: 'LARGEWHITEBOTTLE',
      23: 'LARGEWHITEBOTTLESPECIAL',
      36: 'BROWNBOTTLE',
      37: 'BROWNBOTTLESPECIAL',
      38: 'LARGEBROWNBOTTLE',
      39: 'LARGEBROWNBOTTLESPECIAL',
      52: 'PURPLEBOTTLE',
      53: 'PURPLEBOTTLESPECIAL',
      54: 'LARGEPURPLEBOTTLE',
      55: 'LARGEPURPLEBOTTLESPECIAL',
      68: 'PINKBOTTLE',
      69: 'PINKBOTTLESPECIAL',
      70: 'LARGEPINKBOTTLE',
      71: 'LARGEPINKBOTTLESPECIAL',
      84: 'GREENBOTTLE',
      85: 'GREENBOTTLESPECIAL',
      86: 'LARGEGREENBOTTLE',
      87: 'LARGEGREENBOTTLESPECIAL',
    },
    herbs: {
      0: 'HEALINGMOSSSINGLE',
      1: 'HEALINGMOSSCAP',
      2: 'HEALINGMOSSCLUSTER',
      3: 'THORNBRIARSINGLE',
      4: 'THORNBRIARCLUSTER',
      5: 'THORNBRIARCAP',
      6: 'THORNBRIARANGRYCAP',
      10: 'HUGEBOULDERTOPLEFT',
      11: 'HUGEBOULDERTOPRIGHT',
      12: 'PEBBLE',
      13: 'ROCK',
      14: 'STONE',
      15: 'STONECHUNK',
      16: 'STONEFLAT',
      17: 'BOULDER',
      18: 'BIGBOULDERTOPLEFT',
      19: 'BIGBOULDERTOPRIGHT',
      22: 'MOSSYHUGEBOULDERTOPMIDDLE',
      23: 'HUGEBOULDERMIDDLERIGHT',
      24: 'CATNIPSPROUT',
      25: 'CATNIPSPROUTS',
      26: 'CATNIPBUNCH',
      27: 'CATNIPBUSH',
      30: 'BIGBOULDERBOTTOMLEFT',
      31: 'BIGBOULDERBOTTOMRIGHT',
      32: 'HUGEBOULDERBOTTOMLEFT',
      33: 'HUGEBOULDERBOTTOMRIGHT',
      34: 'MOSSYHUGEBOULDERBOTTOMLEFT',
      35: 'MOSSYHUGEBOULDERBOTTOMRIGHT',
      36: 'SUNFLOWERSHOOT',
      37: 'SUNFLOWERSPROUT',
      38: 'SUNFLOWERBLOOM',
      39: 'SUNFLOWERBLOSSOMTOP',
      40: 'SLUMBERSHADESPROUT',
      41: 'SLUMBERSHADEBUD',
      42: 'SLUMBERSHADEBLOOM',
      43: 'SLUMBERSHADEBLOSSOM',
      44: 'ROSE',
      48: 'ORACLEORCHIDSPROUT',
      49: 'ORACLEORCHIDBLOOM',
      50: 'ORACLEORCHIDBLOSSOM',
      51: 'SUNFLOWERBLOSSOMBOTTOM',
      52: 'DREAMROOTSPROUT',
      53: 'DREAMROOTBUD',
      54: 'DREAMROOTBLOOM',
      55: 'DREAMROOTBLOSSOM',
      56: 'CALMROOTBLOOM',
      57: 'CALMROOTBLOSSOM',
      58: 'MOONPETALBUD',
      59: 'MOONPETALBLOOM',
    },
    trees: {
      24: 'LightForest tree (medium, variant 0)',
      25: 'LightForest tree (medium, variant 1)',
      26: 'LightForest tree (medium, variant 2)',
      36: 'DenseForest tree (full, variant 0)',
      37: 'DenseForest tree (full, variant 1)',
      38: 'DenseForest tree (full, variant 2)',
    },
    chars: {
      0: 'Cat — front-facing idle',
    },
  }

  async function loadBindings() {
    error = null
    try {
      const res = await fetch('/assets/sprites/bindings.toml', { cache: 'no-store' })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      bindings = parseToml(await res.text()) as unknown as BindingsLite
      // Validate the default tab still exists in the (possibly extended) atlases.
      if (bindings.atlases && !bindings.atlases[activeSheet]) {
        activeSheet = Object.keys(bindings.atlases)[0] ?? 'items'
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  onMount(loadBindings)

  let atlasEntries = $derived(
    bindings?.atlases ? Object.entries(bindings.atlases) : []
  )
  let currentMeta = $derived(bindings?.atlases?.[activeSheet])
  let currentNames = $derived<Record<number, string>>(NAMED[activeSheet] ?? {})
  let selectedLabel = $derived(
    selectedIdx !== null ? currentNames[selectedIdx] ?? '(unnamed)' : ''
  )
  let selectedCol = $derived(
    selectedIdx !== null && currentMeta ? selectedIdx % currentMeta.cols : null
  )
  let selectedRow = $derived(
    selectedIdx !== null && currentMeta ? Math.floor(selectedIdx / currentMeta.cols) : null
  )
  let namedCount = $derived(Object.keys(currentNames).length)

  function atlasSrc(meta: AtlasInfo): string {
    return `/assets/${encodeURI(meta.texture)}`
  }
</script>

<div class="flex flex-col gap-4">
  <header class="flex items-baseline gap-4 flex-wrap">
    <h1 class="text-xl font-bold text-accent">Sprite catalog</h1>
    <span class="text-xs text-muted">
      Read-only browser of every <code class="text-txt">[atlases.*]</code> entry in
      <code class="text-txt">bindings.toml</code>. Click a cell to inspect; named
      cells (green outline) ship with the source pack.
      To rebind a game variant, use <a href="#/sprites" class="text-accent underline">#/sprites</a>.
    </span>
  </header>

  {#if error}
    <div class="border border-red-700 bg-red-950 text-red-200 px-3 py-2 rounded">
      Failed to load bindings.toml: {error}
    </div>
  {/if}

  {#if !bindings}
    <p class="text-muted">Loading…</p>
  {:else}
    <nav class="flex gap-1 border-b border-border pb-2 flex-wrap">
      {#each atlasEntries as [key, meta]}
        <button
          class="px-3 py-1.5 border-none bg-transparent text-sm rounded cursor-pointer transition-colors {activeSheet === key ? 'text-accent bg-surface' : 'text-muted hover:text-txt hover:bg-surface-alt'}"
          title={meta.note ?? ''}
          onclick={() => {
            activeSheet = key
            selectedIdx = null
          }}
        >
          {key}
        </button>
      {/each}
    </nav>

    <div class="flex gap-4 items-start">
      <div class="flex-1 min-w-0 overflow-auto">
        {#if currentMeta}
          <p class="text-xs text-muted mb-1">
            {currentMeta.cols * currentMeta.rows} cells · {currentMeta.cols}×{currentMeta.rows} grid of {currentMeta.tile}px tiles · {namedCount} named
          </p>
          {#if currentMeta.note}
            <p class="text-xs text-muted italic mb-2">{currentMeta.note}</p>
          {/if}
          <AtlasGrid
            src={atlasSrc(currentMeta)}
            cols={currentMeta.cols}
            rows={currentMeta.rows}
            tile={currentMeta.tile}
            scale={currentMeta.tile >= 48 ? 2 : 4}
            highlightedIndex={selectedIdx}
            namedCells={currentNames}
            onCellClick={(i) => (selectedIdx = i)}
          />
        {/if}
      </div>

      <aside class="w-[260px] sticky top-2 self-start flex flex-col gap-3">
        <div class="p-3 border border-border rounded bg-surface">
          <h3 class="text-xs uppercase tracking-wider text-muted mb-2">Selected</h3>
          {#if selectedIdx === null}
            <p class="text-xs text-muted">Click a cell in the atlas.</p>
          {:else if currentMeta}
            <div class="flex flex-col gap-1 text-xs">
              <div class="flex justify-between"><span class="text-muted">index</span><span class="text-txt font-bold">#{selectedIdx}</span></div>
              <div class="flex justify-between"><span class="text-muted">col</span><span class="text-txt">{selectedCol}</span></div>
              <div class="flex justify-between"><span class="text-muted">row</span><span class="text-txt">{selectedRow}</span></div>
              <div class="flex justify-between"><span class="text-muted">tile</span><span class="text-txt">{currentMeta.tile}px</span></div>
              <div class="mt-2 pt-2 border-t border-border">
                <span class="text-muted">name:</span>
                <div class="text-txt mt-1 break-words">{selectedLabel}</div>
              </div>
            </div>
          {/if}
        </div>

        {#if namedCount > 0}
          <div class="p-3 border border-border rounded bg-surface">
            <h3 class="text-xs uppercase tracking-wider text-muted mb-2">Named cells ({namedCount})</h3>
            <div class="max-h-[400px] overflow-y-auto flex flex-col gap-0.5 text-[11px]">
              {#each Object.entries(currentNames).sort((a, b) => Number(a[0]) - Number(b[0])) as [idx, name]}
                <button
                  type="button"
                  class="flex items-baseline gap-2 text-left px-1 py-0.5 rounded hover:bg-surface-alt cursor-pointer {selectedIdx === Number(idx) ? 'bg-accent/10 text-accent' : 'text-txt'}"
                  onclick={() => (selectedIdx = Number(idx))}
                >
                  <span class="text-muted w-8 text-right shrink-0">#{idx}</span>
                  <span class="truncate">{name}</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</div>
