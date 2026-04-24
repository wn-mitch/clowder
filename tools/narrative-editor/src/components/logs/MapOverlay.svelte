<script lang="ts">
  import { onDestroy } from 'svelte'
  import type { RunModel } from '../../lib/logs/types'
  import {
    buildSpatialIndex, lastBeforeOrAt, activeWardsAt, type SpatialIndex,
  } from '../../lib/logs/spatial'

  interface Props {
    runs: RunModel[]
  }

  let { runs }: Props = $props()

  let activeRunId = $state<string | null>(null)
  let currentTick = $state<number>(0)
  let playing = $state(false)
  let layers = $state<Record<string, boolean>>({
    cats: true,
    prey: true,
    predators: true,
    dens: true,
    wards: true,
    ambushes: true,
    kills: false,
    deaths: true,
    beliefs: false,
  })

  // Event-dot trailing window (ticks). Keeps the screen from saturating.
  const DOT_WINDOW = 500

  // Species palettes. Keep in sync with other chart components.
  const PREY_COLOR: Record<string, string> = {
    Mouse: '#d4a574', Rat: '#a0766c', Rabbit: '#e0b888',
    Fish: '#74b4d4', Bird: '#7ec87e',
  }
  const WILDLIFE_COLOR: Record<string, string> = {
    Fox: '#d4a574', Hawk: '#7ec87e', Snake: '#b4d474', ShadowFox: '#d474b4',
  }

  let indices = $derived(new Map(runs.map(r => [r.id, buildSpatialIndex(r)])))
  let activeRun = $derived(runs.find(r => r.id === activeRunId) ?? runs[0] ?? null)
  let activeIndex = $derived(activeRun ? indices.get(activeRun.id) ?? null : null)

  // Slider bounds.
  let minTick = $derived(activeIndex?.sortedTicks[0] ?? 0)
  let maxTick = $derived(activeIndex?.maxTick ?? 0)

  $effect(() => {
    // Reset the slider when the active run changes.
    if (!activeIndex) return
    if (currentTick < minTick || currentTick > maxTick) {
      currentTick = maxTick
    }
    if (activeRunId === null && runs.length > 0) activeRunId = runs[0].id
  })

  let canvas = $state<HTMLCanvasElement | null>(null)

  function fallbackMapDims(idx: SpatialIndex): { w: number; h: number } {
    if (idx.mapWidth && idx.mapHeight) return { w: idx.mapWidth, h: idx.mapHeight }
    // Infer from observed coordinates.
    let maxX = 0, maxY = 0
    for (const arr of idx.catsByTick.values()) for (const c of arr) { if (c.x > maxX) maxX = c.x; if (c.y > maxY) maxY = c.y }
    for (const arr of idx.preyByTick.values()) for (const p of arr) { if (p.x > maxX) maxX = p.x; if (p.y > maxY) maxY = p.y }
    for (const arr of idx.wildlifeByTick.values()) for (const p of arr) { if (p.x > maxX) maxX = p.x; if (p.y > maxY) maxY = p.y }
    for (const d of idx.dots) { if (d.x > maxX) maxX = d.x; if (d.y > maxY) maxY = d.y }
    return { w: Math.max(maxX + 1, 32), h: Math.max(maxY + 1, 32) }
  }

  function draw() {
    if (!canvas || !activeIndex) return
    const idx = activeIndex
    const { w: mapW, h: mapH } = fallbackMapDims(idx)

    const cssW = canvas.clientWidth
    const scale = Math.max(1, Math.floor(cssW / mapW))
    const drawW = mapW * scale
    const drawH = mapH * scale

    const dpr = window.devicePixelRatio || 1
    canvas.width = drawW * dpr
    canvas.height = drawH * dpr
    canvas.style.height = `${drawH}px`
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.imageSmoothingEnabled = false

    // Background.
    ctx.fillStyle = '#1a1a1a'
    ctx.fillRect(0, 0, drawW, drawH)

    // Hunting belief heatmap (painted first, under everything else).
    if (layers.beliefs) {
      const belief = lastBeforeOrAt(idx.beliefByTick, currentTick)
      if (belief) {
        const gw = belief.width
        const gh = belief.height
        if (gw > 0 && gh > 0) {
          const cellW = drawW / gw
          const cellH = drawH / gh
          let vMin = Infinity, vMax = -Infinity
          for (const v of belief.values) { if (v < vMin) vMin = v; if (v > vMax) vMax = v }
          const range = Math.max(1e-6, vMax - vMin)
          for (let gy = 0; gy < gh; gy++) {
            for (let gx = 0; gx < gw; gx++) {
              const v = belief.values[gy * gw + gx]
              const t = (v - vMin) / range
              const r = Math.round(255 * (1 - t))
              const g = Math.round(220 * t)
              ctx.fillStyle = `rgba(${r}, ${g}, 80, 0.35)`
              ctx.fillRect(gx * cellW, gy * cellH, cellW + 0.5, cellH + 0.5)
            }
          }
        }
      }
    }

    // Dens: prey dens as hollow squares, fox dens as filled squares with cubs count.
    if (layers.dens) {
      const dens = lastBeforeOrAt(idx.densByTick, currentTick)
      if (dens) {
        for (const p of dens.prey_dens) {
          ctx.strokeStyle = PREY_COLOR[p.species] ?? '#aaa'
          ctx.lineWidth = 1
          ctx.strokeRect(p.x * scale - 2, p.y * scale - 2, scale + 4, scale + 4)
        }
        for (const f of dens.fox_dens) {
          ctx.fillStyle = '#8a3838'
          ctx.fillRect(f.x * scale - 3, f.y * scale - 3, scale + 6, scale + 6)
          if (f.cubs_present > 0) {
            ctx.fillStyle = '#fff'
            ctx.font = `${Math.max(8, scale * 2)}px system-ui`
            ctx.fillText(`${f.cubs_present}`, f.x * scale + scale + 2, f.y * scale + scale)
          }
        }
      }
    }

    // Active wards — color by kind, dashed if sieged is ever recorded.
    if (layers.wards) {
      const wards = activeWardsAt(idx, currentTick)
      for (const w of wards) {
        ctx.strokeStyle = '#e0c074'
        ctx.lineWidth = 1.5
        ctx.setLineDash(w.sieged ? [2, 2] : [])
        ctx.strokeRect(w.x * scale - 1, w.y * scale - 1, scale + 2, scale + 2)
        ctx.setLineDash([])
      }
    }

    // Prey and wildlife positions at the nearest spatial tick ≤ current.
    if (layers.prey) {
      const prey = lastBeforeOrAt(idx.preyByTick, currentTick)
      if (prey) {
        for (const p of prey) {
          ctx.fillStyle = PREY_COLOR[p.species] ?? '#888'
          ctx.fillRect(p.x * scale, p.y * scale, Math.max(2, scale), Math.max(2, scale))
        }
      }
    }
    if (layers.predators) {
      const wildlife = lastBeforeOrAt(idx.wildlifeByTick, currentTick)
      if (wildlife) {
        for (const w of wildlife) {
          ctx.fillStyle = WILDLIFE_COLOR[w.species] ?? '#aaa'
          const size = Math.max(3, scale + 1)
          ctx.fillRect(w.x * scale - 1, w.y * scale - 1, size, size)
        }
      }
    }

    // Cats (drawn last among living entities so they sit on top).
    if (layers.cats) {
      const cats = lastBeforeOrAt(idx.catsByTick, currentTick)
      if (cats) {
        for (const c of cats) {
          ctx.fillStyle = '#ffd27f'
          const size = Math.max(3, scale + 1)
          ctx.fillRect(c.x * scale - 1, c.y * scale - 1, size, size)
        }
      }
    }

    // Event dots with a trailing window.
    const cutoff = currentTick - DOT_WINDOW
    for (const d of idx.dots) {
      if (d.tick > currentTick) continue
      if (d.type === 'Death') {
        if (!layers.deaths) continue
      } else if (d.type === 'PreyKilled') {
        if (!layers.kills || d.tick < cutoff) continue
      } else if (d.type === 'Ambush') {
        if (!layers.ambushes || d.tick < cutoff) continue
      } else if (d.tick < cutoff) {
        // Spawn/banish/kitten/build events: also bounded by window.
        continue
      }
      const age = (currentTick - d.tick) / DOT_WINDOW
      let color: string
      switch (d.type) {
        case 'Ambush': color = `rgba(212, 116, 116, ${1 - age})`; break
        case 'PreyKilled': color = `rgba(126, 200, 126, ${1 - age})`; break
        case 'Death': color = 'rgba(30, 30, 30, 0.9)'; break // persistent
        case 'ShadowFoxSpawn': color = `rgba(212, 116, 180, ${1 - age})`; break
        case 'ShadowFoxBanished': color = `rgba(230, 190, 90, ${1 - age})`; break
        case 'KittenBorn': color = `rgba(255, 210, 127, ${1 - age})`; break
        case 'BuildingConstructed': color = `rgba(160, 200, 250, ${1 - age})`; break
        default: color = `rgba(180, 180, 180, ${1 - age})`
      }
      ctx.fillStyle = color
      ctx.beginPath()
      ctx.arc(d.x * scale + scale / 2, d.y * scale + scale / 2, Math.max(2, scale / 1.5), 0, Math.PI * 2)
      ctx.fill()
    }
  }

  let animId: number | null = null
  function animate() {
    if (!playing) { animId = null; return }
    const stepTicks = Math.max(1, Math.round((maxTick - minTick) / 300))
    currentTick = Math.min(maxTick, currentTick + stepTicks)
    if (currentTick >= maxTick) playing = false
    animId = requestAnimationFrame(animate)
  }

  function togglePlay() {
    playing = !playing
    if (playing && animId === null) animId = requestAnimationFrame(animate)
  }

  $effect(() => {
    void currentTick; void layers; void activeIndex
    requestAnimationFrame(draw)
  })

  onDestroy(() => {
    if (animId !== null) cancelAnimationFrame(animId)
  })

  function toggleLayer(key: string) {
    layers = { ...layers, [key]: !layers[key] }
  }

  function pickRun(e: Event) {
    activeRunId = (e.target as HTMLSelectElement).value
  }
</script>

<section class="flex flex-col gap-3">
  {#if runs.length === 0}
    <p class="text-sm italic text-muted">No runs loaded.</p>
  {:else}
    <header class="flex items-center gap-3 flex-wrap">
      <h2 class="m-0 text-base">Map overlay</h2>
      {#if runs.length > 1}
        <select
          class="bg-surface border border-border rounded-md px-2 py-1 text-sm"
          value={activeRunId ?? runs[0].id}
          onchange={pickRun}
        >
          {#each runs as r (r.id)}
            <option value={r.id}>
              seed {r.header?.seed ?? '?'} · {r.header?.commit_hash_short ?? '—'}
            </option>
          {/each}
        </select>
      {/if}
      {#if activeIndex && activeIndex.mapWidth === null}
        <span class="text-xs text-warning">
          Header has no map dims — inferring from point bounds.
        </span>
      {/if}
    </header>

    <div class="flex gap-3">
      <canvas
        bind:this={canvas}
        class="border border-border rounded-md bg-surface flex-1"
        style="image-rendering: pixelated; max-height: 600px"
      ></canvas>

      <div class="flex flex-col gap-1 text-xs w-40 shrink-0">
        <div class="text-muted uppercase tracking-wide">Layers</div>
        {#each Object.keys(layers) as key (key)}
          <label class="flex items-center gap-1 cursor-pointer">
            <input
              type="checkbox"
              class="accent-accent cursor-pointer"
              checked={layers[key]}
              onchange={() => toggleLayer(key)}
            />
            <span>{key}</span>
          </label>
        {/each}
      </div>
    </div>

    <div class="flex items-center gap-2 text-xs">
      <button
        type="button"
        class="px-2 py-0.5 border border-border rounded"
        onclick={togglePlay}
      >{playing ? 'Pause' : 'Play'}</button>
      <span class="font-mono text-muted">tick</span>
      <input
        type="range"
        min={minTick}
        max={maxTick}
        step={Math.max(1, Math.round((maxTick - minTick) / 300))}
        value={currentTick}
        oninput={(e) => currentTick = Number((e.target as HTMLInputElement).value)}
        class="flex-1"
      />
      <span class="font-mono w-20 text-right">{currentTick}</span>
    </div>
  {/if}
</section>
