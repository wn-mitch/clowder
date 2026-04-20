<script lang="ts">
  import {
    runs, selectedRuns, loadError, clearRuns, logsSubTab, type LogsSubTab,
  } from '../stores/runs'
  import { summarizeRun } from '../lib/logs/metrics'
  import { collectMismatches } from '../lib/logs/diff'
  import { NEED_KEYS } from '../lib/logs/types'
  import type { RunModel } from '../lib/logs/types'
  import {
    colonyMaslowSeries, wildlifeSeries,
  } from '../lib/logs/metrics'
  import FileDropZone from '../components/logs/FileDropZone.svelte'
  import RunTable from '../components/logs/RunTable.svelte'
  import SchemaGuardBanner from '../components/logs/SchemaGuardBanner.svelte'
  import ComparisonChart from '../components/logs/ComparisonChart.svelte'
  import ActivationGrid from '../components/logs/ActivationGrid.svelte'
  import MultiLineChart, { type SeriesDef } from '../components/logs/MultiLineChart.svelte'
  import StackedSpeciesChart from '../components/logs/StackedSpeciesChart.svelte'
  import CatDetailPanel from '../components/logs/CatDetailPanel.svelte'
  import MapOverlay from '../components/logs/MapOverlay.svelte'

  type ChartMetric =
    | 'welfare' | 'aggregate' | 'population' | 'food-stores'
    | 'prey-by-species' | 'predators-by-species' | 'colony-maslow'

  let chartMetric = $state<ChartMetric>('welfare')

  let summaries = $derived($runs.map(summarizeRun))
  let compareRuns = $derived($selectedRuns.length > 0 ? $selectedRuns : $runs)
  let mismatches = $derived(collectMismatches(compareRuns))

  const SCALAR_METRICS: ChartMetric[] = ['welfare', 'aggregate', 'population', 'food-stores']
  const MULTI_METRICS: ChartMetric[] = ['prey-by-species', 'predators-by-species', 'colony-maslow']

  const SCALAR_META: Record<string, { title: string; yLabel: string }> = {
    welfare:       { title: 'Colony welfare over time', yLabel: 'welfare [0,1]' },
    aggregate:     { title: 'Aggregate score over time', yLabel: 'aggregate' },
    population:    { title: 'Prey population total', yLabel: 'prey count' },
    'food-stores': { title: 'Food stores over time', yLabel: 'food units' },
  }

  const PRED_COLORS = {
    foxes: '#d4a574', hawks: '#7ec87e', snakes: '#b4d474', shadow_foxes: '#d474b4',
  }
  const PRED_DEFS: SeriesDef[] = [
    { label: 'foxes',        color: PRED_COLORS.foxes },
    { label: 'hawks',        color: PRED_COLORS.hawks },
    { label: 'snakes',       color: PRED_COLORS.snakes },
    { label: 'shadow-foxes', color: PRED_COLORS.shadow_foxes },
  ]
  function extractPredators(run: RunModel): { xs: number[]; ys: (number | null)[][] } {
    const series = wildlifeSeries(run.events)
    return {
      xs: series.map(s => s.tick),
      ys: [
        series.map(s => s.foxes),
        series.map(s => s.hawks),
        series.map(s => s.snakes),
        series.map(s => s.shadow_foxes),
      ],
    }
  }

  // Colony Maslow — user picks which of the 10 needs to show.
  const NEED_COLORS: Record<string, string> = {
    hunger: '#d47474', energy: '#d4a574', warmth: '#e0b888', safety: '#74b4d4',
    social: '#7ec87e', acceptance: '#b4d474', mating: '#d474b4',
    respect: '#a0a0d4', mastery: '#74d4b4', purpose: '#d4d4a0',
  }
  let visibleMaslow = $state<Set<string>>(new Set(['hunger', 'energy', 'warmth', 'safety']))

  let maslowDefs = $derived<SeriesDef[]>(
    NEED_KEYS.filter(k => visibleMaslow.has(k))
      .map(k => ({ label: k, color: NEED_COLORS[k] })),
  )

  function extractColonyMaslow(run: RunModel): { xs: number[]; ys: (number | null)[][] } {
    const series = colonyMaslowSeries(run.events)
    const xs = series.map(s => s.tick)
    const keys = NEED_KEYS.filter(k => visibleMaslow.has(k))
    const ys = keys.map(k => series.map(s => s.needs[k] as number))
    return { xs, ys }
  }

  function toggleMaslowNeed(key: string) {
    const next = new Set(visibleMaslow)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    visibleMaslow = next
  }

  function setSubTab(tab: LogsSubTab) {
    logsSubTab.set(tab)
  }
</script>

<section class="flex flex-col gap-4">
  <header class="flex items-baseline justify-between gap-4">
    <div>
      <h1 class="m-0 text-xl">Simulation Log Viewer</h1>
      <p class="m-0 mt-1 text-sm text-muted">
        Load one or more <code>events.jsonl</code> / <code>narrative.jsonl</code> files
        to compare runs side by side. Files from the same run
        (matching seed, commit, and duration) pair automatically.
      </p>
    </div>
    {#if $runs.length > 0}
      <button type="button" class="text-sm" onclick={clearRuns}>Clear all</button>
    {/if}
  </header>

  <FileDropZone />

  {#if $loadError}
    <div class="bg-negative/10 border border-negative text-negative rounded-md p-3 text-sm">
      Load failed: {$loadError}
    </div>
  {/if}

  <SchemaGuardBanner {mismatches} />

  {#if $runs.length === 0}
    <div class="text-muted italic text-sm p-4">
      No runs loaded yet. Generate logs with <code>just soak 42</code> or
      <code>just headless</code>, then drop the resulting
      <code>events.jsonl</code> / <code>narrative.jsonl</code> above.
    </div>
  {:else}
    <RunTable {summaries} />

    {#if compareRuns.length > 0}
      <!-- Sub-tabs within the Logs page. -->
      <div class="flex items-center gap-1 border-b border-border mt-2 text-sm">
        <button
          type="button"
          class="px-3 py-1 border-none bg-transparent cursor-pointer {$logsSubTab === 'overview' ? 'text-accent border-b-2 border-accent' : 'text-muted'}"
          onclick={() => setSubTab('overview')}
        >Overview</button>
        <button
          type="button"
          class="px-3 py-1 border-none bg-transparent cursor-pointer {$logsSubTab === 'cat' ? 'text-accent border-b-2 border-accent' : 'text-muted'}"
          onclick={() => setSubTab('cat')}
        >Cat detail</button>
        <button
          type="button"
          class="px-3 py-1 border-none bg-transparent cursor-pointer {$logsSubTab === 'map' ? 'text-accent border-b-2 border-accent' : 'text-muted'}"
          onclick={() => setSubTab('map')}
        >Map</button>
        <span class="ml-auto text-xs text-muted">
          {$selectedRuns.length > 0
            ? `Comparing ${$selectedRuns.length} selected run${$selectedRuns.length === 1 ? '' : 's'}.`
            : `Showing all ${$runs.length} loaded runs.`}
        </span>
      </div>

      {#if $logsSubTab === 'overview'}
        <div class="flex flex-col gap-3">
          <div class="flex flex-wrap items-center gap-2 text-xs text-muted mt-2">
            <span>Metric:</span>
            {#each SCALAR_METRICS as m (m)}
              <button
                type="button"
                class="text-xs px-2 py-0.5 {chartMetric === m ? 'text-accent border-accent' : 'text-muted'}"
                onclick={() => chartMetric = m}
              >{m}</button>
            {/each}
            <span class="text-muted px-1">·</span>
            {#each MULTI_METRICS as m (m)}
              <button
                type="button"
                class="text-xs px-2 py-0.5 {chartMetric === m ? 'text-accent border-accent' : 'text-muted'}"
                onclick={() => chartMetric = m}
              >{m}</button>
            {/each}
          </div>

          {#if SCALAR_METRICS.includes(chartMetric)}
            <ComparisonChart
              runs={compareRuns}
              metric={chartMetric as 'welfare' | 'aggregate' | 'population' | 'food-stores'}
              title={SCALAR_META[chartMetric].title}
              yLabel={SCALAR_META[chartMetric].yLabel}
            />
          {:else if chartMetric === 'prey-by-species'}
            <StackedSpeciesChart runs={compareRuns} />
          {:else if chartMetric === 'predators-by-species'}
            <MultiLineChart
              runs={compareRuns}
              seriesDefs={PRED_DEFS}
              extract={extractPredators}
              title="Predators by species"
              yLabel="count"
            />
          {:else if chartMetric === 'colony-maslow'}
            <div class="flex flex-wrap items-center gap-2 text-xs">
              <span class="text-muted">Needs:</span>
              {#each NEED_KEYS as need (need)}
                <label class="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    class="accent-accent cursor-pointer"
                    checked={visibleMaslow.has(need)}
                    onchange={() => toggleMaslowNeed(need)}
                  />
                  <span style="color: {NEED_COLORS[need]}">{need}</span>
                </label>
              {/each}
            </div>
            <MultiLineChart
              runs={compareRuns}
              seriesDefs={maslowDefs}
              extract={extractColonyMaslow}
              title="Colony-averaged Maslow needs"
              yLabel="need [0,1]"
            />
          {/if}

          <h2 class="m-0 mt-2 text-base">System activation</h2>
          <ActivationGrid runs={compareRuns} />
        </div>
      {:else if $logsSubTab === 'cat'}
        <CatDetailPanel runs={compareRuns} />
      {:else if $logsSubTab === 'map'}
        <MapOverlay runs={compareRuns} />
      {/if}
    {/if}
  {/if}
</section>
