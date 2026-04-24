<script lang="ts">
  import type { Frame, FrameIndex, L1Record } from '../../lib/logs/trace'

  interface Props {
    frame: Frame
    /** Used for the per-map sparkline — we look back over recent frames
     *  to show how perception evolved into this tick. */
    index: FrameIndex
  }
  let { frame, index }: Props = $props()

  /** If the current tick has no L1 records (can happen when a decision
   *  tick didn't coincide with a perception tick), fall back to the
   *  nearest prior tick within a short window so the panel is still
   *  populated. Real soaks typically have L1 at every decision tick,
   *  but the substrate spec allows lazy emission. */
  function l1WithFallback(): { records: L1Record[]; fromTick: number | null } {
    if (frame.l1.length > 0) return { records: frame.l1, fromTick: frame.tick }
    const allTicks = index.allTicks
    let lo = 0, hi = allTicks.length - 1, best: number | null = null
    while (lo <= hi) {
      const mid = (lo + hi) >> 1
      if (allTicks[mid] < frame.tick) { best = allTicks[mid]; lo = mid + 1 }
      else hi = mid - 1
    }
    if (best === null) return { records: [], fromTick: null }
    const rec = index.frames.get(best)?.l1 ?? []
    return { records: rec, fromTick: best }
  }

  let l1 = $derived(l1WithFallback())

  function fmt(n: number, d = 3) {
    if (!Number.isFinite(n)) return '—'
    return n.toFixed(d)
  }

  function factionBadgeColor(faction: string): string {
    switch (faction.toLowerCase()) {
      case 'fox':      return 'bg-negative/20 text-negative'
      case 'colony':   return 'bg-accent/20 text-accent'
      case 'observer': return 'bg-positive/20 text-positive'
      case 'neutral':  return 'bg-muted/20 text-muted'
      default:         return 'bg-muted/10 text-muted'
    }
  }

  /** Build a sparse perceived-value history for one map, looking back
   *  across prior decision ticks. Returns up to `windowTicks` points
   *  used by the inline sparkline. */
  function perceivedHistory(map: string, windowTicks = 40): { ticks: number[]; values: number[] } {
    const ticks: number[] = []
    const values: number[] = []
    const decision = index.decisionTicks
    let cutoff = 0
    // Walk back from the current decision tick.
    for (let i = decision.length - 1; i >= 0 && ticks.length < windowTicks; i--) {
      const t = decision[i]
      if (t > frame.tick) continue
      cutoff = t
      const f = index.frames.get(t)
      const row = f?.l1.find(r => r.map === map)
      if (row) { ticks.unshift(t); values.unshift(row.perceived) }
    }
    void cutoff
    return { ticks, values }
  }

  function sparklinePath(values: number[], w: number, h: number): string {
    if (values.length < 2) return ''
    const max = Math.max(0.001, ...values)
    const dx = w / (values.length - 1)
    const pts = values.map((v, i) => {
      const x = i * dx
      const y = h - (v / max) * h
      return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`
    })
    return pts.join(' ')
  }
</script>

<div class="flex flex-col gap-2">
  {#if l1.records.length === 0}
    <div class="text-xs italic text-muted p-3 bg-surface border border-border rounded">
      No L1 records at or before this tick.
    </div>
  {:else}
    {#if l1.fromTick !== frame.tick}
      <div class="text-[10px] italic text-muted px-1">
        showing L1 from tick {l1.fromTick} (no sample at {frame.tick})
      </div>
    {/if}

    {#each l1.records as r (r.map + ':' + r.channel + ':' + r.faction)}
      {@const hist = perceivedHistory(r.map)}
      <div class="bg-surface border border-border rounded-md p-2.5 text-xs">
        <div class="flex items-baseline gap-2 mb-1">
          <span class="font-mono font-semibold text-txt">{r.map}</span>
          <span class="px-1.5 py-0.5 rounded text-[10px] uppercase tracking-wider {factionBadgeColor(r.faction)}">{r.faction}</span>
          <span class="text-[10px] text-muted">{r.channel}</span>
          <span class="ml-auto text-[10px] text-muted font-mono">pos ({r.pos[0]}, {r.pos[1]})</span>
        </div>

        <!-- base sample → perceived summary -->
        <div class="grid grid-cols-[4rem_1fr_3rem] items-center gap-2 font-mono">
          <span class="text-muted">base</span>
          <div class="relative h-2 bg-surface-alt rounded overflow-hidden">
            <div class="absolute left-0 top-0 bottom-0 bg-muted/50"
                 style="width: {Math.min(100, r.base_sample * 100)}%"></div>
          </div>
          <span class="text-right tabular-nums">{fmt(r.base_sample)}</span>

          <span class="text-muted">perceived</span>
          <div class="relative h-2 bg-surface-alt rounded overflow-hidden">
            <div class="absolute left-0 top-0 bottom-0 bg-accent/70"
                 style="width: {Math.min(100, r.perceived * 100)}%"></div>
          </div>
          <span class="text-right tabular-nums font-semibold">{fmt(r.perceived)}</span>
        </div>

        <!-- Attenuation breakdown -->
        <div class="mt-2 pt-2 border-t border-border">
          <div class="text-[10px] uppercase tracking-wider text-muted mb-1">Attenuation</div>
          <div class="grid grid-cols-[6rem_1fr_3rem] gap-x-2 gap-y-0.5 font-mono text-[11px]">
            <span class="text-muted">species sens</span>
            <div class="relative h-1.5 bg-surface-alt rounded overflow-hidden self-center">
              <div class="absolute left-0 top-0 bottom-0 bg-accent/40"
                   style="width: {Math.min(100, r.attenuation.species_sens * 100)}%"></div>
            </div>
            <span class="text-right tabular-nums">{fmt(r.attenuation.species_sens, 2)}</span>

            <span class="text-muted">role mod</span>
            <div class="relative h-1.5 bg-surface-alt rounded overflow-hidden self-center">
              <div class="absolute left-0 top-0 bottom-0 bg-accent/40"
                   style="width: {Math.min(100, r.attenuation.role_mod * 100)}%"></div>
            </div>
            <span class="text-right tabular-nums">{fmt(r.attenuation.role_mod, 2)}</span>

            <span class="text-muted">injury deficit</span>
            <div class="relative h-1.5 bg-surface-alt rounded overflow-hidden self-center">
              <div class="absolute left-0 top-0 bottom-0 bg-negative/40"
                   style="width: {Math.min(100, r.attenuation.injury_deficit * 100)}%"></div>
            </div>
            <span class="text-right tabular-nums">{fmt(r.attenuation.injury_deficit, 2)}</span>

            <span class="text-muted">env mul</span>
            <div class="relative h-1.5 bg-surface-alt rounded overflow-hidden self-center">
              <div class="absolute left-0 top-0 bottom-0 bg-accent/40"
                   style="width: {Math.min(100, r.attenuation.env_mul * 100)}%"></div>
            </div>
            <span class="text-right tabular-nums">{fmt(r.attenuation.env_mul, 2)}</span>
          </div>
        </div>

        <!-- Recent-history sparkline -->
        {#if hist.values.length >= 2}
          <div class="mt-2 pt-2 border-t border-border flex items-center gap-2">
            <span class="text-[10px] uppercase tracking-wider text-muted">recent</span>
            <svg viewBox="0 0 120 18" preserveAspectRatio="none" class="w-full h-4 flex-1">
              <path d={sparklinePath(hist.values, 120, 18)} fill="none"
                    stroke="var(--color-accent, #d4a574)" stroke-width="1" />
            </svg>
            <span class="text-[10px] text-muted font-mono">{hist.values.length}pt</span>
          </div>
        {/if}

        <!-- Top contributors -->
        {#if r.top_contributors.length > 0}
          <div class="mt-2 pt-2 border-t border-border">
            <div class="text-[10px] uppercase tracking-wider text-muted mb-1">Top contributors</div>
            <div class="flex flex-col gap-0.5 font-mono text-[11px]">
              {#each r.top_contributors as c (c.emitter)}
                <div class="grid grid-cols-[1fr_3rem_3rem] gap-2">
                  <span class="truncate">{c.emitter} <span class="text-muted">({c.pos[0]},{c.pos[1]})</span></span>
                  <span class="text-right text-muted tabular-nums">d {c.distance}</span>
                  <span class="text-right tabular-nums">{fmt(c.contribution)}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</div>
