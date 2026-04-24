<script lang="ts">
  import type { RunModel } from '../../lib/logs/types'
  import { NEED_KEYS } from '../../lib/logs/types'
  import {
    availableCatNames, perCatActionCounts, perCatMoodSeries, perCatNeedsSeries,
  } from '../../lib/logs/metrics'
  import MultiLineChart, { type SeriesDef } from './MultiLineChart.svelte'
  import { selectedCatName } from '../../stores/runs'

  interface Props {
    runs: RunModel[]
  }

  let { runs }: Props = $props()

  // Default the cat picker to whatever selectedCatName holds, falling back
  // to the first cat observed in the first run.
  let catNames = $derived(
    Array.from(new Set(runs.flatMap(r => availableCatNames(r.events)))).sort(),
  )
  let currentCat = $derived($selectedCatName ?? catNames[0] ?? null)

  // One line per Maslow need. Default-visible are the four most diagnostic
  // under typical balance work (hunger/energy/temperature/safety).
  // `social_warmth` is warmth-split phase 3 (docs/open-work.md #12); emits 0
  // until that ships.
  const NEED_COLORS: Record<typeof NEED_KEYS[number], string> = {
    hunger: '#d47474', energy: '#d4a574', temperature: '#e0b888', safety: '#74b4d4',
    social: '#7ec87e', social_warmth: '#f0a0c0', acceptance: '#b4d474',
    mating: '#d474b4', respect: '#a0a0d4', mastery: '#74d4b4', purpose: '#d4d4a0',
  }
  const DEFAULT_VISIBLE = new Set<string>(['hunger', 'energy', 'temperature', 'safety'])
  let visibleNeeds = $state<Set<string>>(new Set(DEFAULT_VISIBLE))

  let needsSeriesDefs = $derived<SeriesDef[]>(
    NEED_KEYS
      .filter(k => visibleNeeds.has(k))
      .map(k => ({ label: k, color: NEED_COLORS[k] })),
  )

  function extractNeeds(run: RunModel): { xs: number[]; ys: (number | null)[][] } {
    if (!currentCat) return { xs: [], ys: [] }
    const series = perCatNeedsSeries(run.events, currentCat)
    const xs = series.map(s => s.tick)
    const keys = NEED_KEYS.filter(k => visibleNeeds.has(k))
    const ys = keys.map(k => series.map(s => s.needs[k] as number))
    return { xs, ys }
  }

  function extractMood(run: RunModel): { xs: number[]; ys: (number | null)[][] } {
    if (!currentCat) return { xs: [], ys: [] }
    const series = perCatMoodSeries(run.events, currentCat)
    return { xs: series.map(s => s.tick), ys: [series.map(s => s.valence)] }
  }

  const MOOD_DEFS: SeriesDef[] = [{ label: 'mood', color: '#d4a574' }]

  // Action counts table — merged across all loaded runs so the distribution
  // is stable when the run selection changes.
  let actionRows = $derived.by(() => {
    if (!currentCat) return []
    const merged: Record<string, number> = {}
    for (const run of runs) {
      const counts = perCatActionCounts(run.events, currentCat)
      for (const [k, v] of Object.entries(counts)) {
        merged[k] = (merged[k] ?? 0) + v
      }
    }
    const entries = Object.entries(merged).sort((a, b) => b[1] - a[1])
    const total = entries.reduce((acc, [, v]) => acc + v, 0)
    return entries.map(([action, count]) => ({
      action, count, pct: total > 0 ? count / total : 0,
    }))
  })

  function toggleNeed(key: string) {
    const next = new Set(visibleNeeds)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    visibleNeeds = next
  }

  function pickCat(e: Event) {
    const target = e.target as HTMLSelectElement
    selectedCatName.set(target.value)
  }
</script>

<section class="flex flex-col gap-3">
  <header class="flex items-center gap-3">
    <h2 class="m-0 text-base">Cat detail</h2>
    <select
      class="bg-surface border border-border rounded-md px-2 py-1 text-sm"
      value={currentCat ?? ''}
      onchange={pickCat}
    >
      {#each catNames as name (name)}
        <option value={name}>{name}</option>
      {/each}
    </select>
    {#if catNames.length === 0}
      <span class="text-sm text-muted italic">
        No CatSnapshot events in the loaded runs.
      </span>
    {/if}
  </header>

  {#if currentCat}
    <div>
      <div class="flex flex-wrap items-center gap-2 mb-1 text-xs">
        <span class="text-muted">Needs:</span>
        {#each NEED_KEYS as need (need)}
          <label class="flex items-center gap-1 cursor-pointer">
            <input
              type="checkbox"
              class="accent-accent cursor-pointer"
              checked={visibleNeeds.has(need)}
              onchange={() => toggleNeed(need)}
            />
            <span style="color: {NEED_COLORS[need]}">{need}</span>
          </label>
        {/each}
      </div>
      <MultiLineChart
        runs={runs}
        seriesDefs={needsSeriesDefs}
        extract={extractNeeds}
        title={`${currentCat} — needs`}
        yLabel="need [0,1]"
      />
    </div>

    <div>
      <h3 class="m-0 mt-2 text-sm text-muted">Mood valence</h3>
      <MultiLineChart
        runs={runs}
        seriesDefs={MOOD_DEFS}
        extract={extractMood}
        title={`${currentCat} — mood`}
        yLabel="valence [−1,1]"
        height={180}
      />
    </div>

    <div>
      <h3 class="m-0 mt-2 text-sm text-muted">Action distribution</h3>
      {#if actionRows.length === 0}
        <p class="text-sm italic text-muted">
          No ActionChosen events for {currentCat}. Enable ActionChosen emission or load a run that has it.
        </p>
      {:else}
        <div class="bg-surface border border-border rounded-md p-2">
          {#each actionRows as row (row.action)}
            <div class="flex items-center gap-2 text-xs py-0.5">
              <span class="w-32 text-muted truncate" title={row.action}>{row.action}</span>
              <div class="flex-1 bg-surface-alt h-3 rounded overflow-hidden">
                <div
                  class="h-full bg-accent"
                  style="width: {(row.pct * 100).toFixed(1)}%"
                ></div>
              </div>
              <span class="font-mono w-14 text-right">{row.count}</span>
              <span class="font-mono w-10 text-right text-muted">{(row.pct * 100).toFixed(0)}%</span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</section>
