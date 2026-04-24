<script lang="ts">
  import type { RunModel } from '../../lib/logs/types'
  import { activationSummary, type ActivationSummary } from '../../lib/logs/metrics'

  interface Props {
    runs: RunModel[]
  }

  let { runs }: Props = $props()

  type Category = 'positive' | 'neutral' | 'negative'

  // Display order + colors per category. Positive/negative come first
  // because silently-dead positive subsystems and negative-event spikes
  // are the load-bearing diagnostics; neutral is last as reference.
  const CATEGORIES: { key: Category; label: string; accent: string }[] = [
    { key: 'positive', label: 'Positive', accent: 'text-positive' },
    { key: 'negative', label: 'Negative', accent: 'text-negative' },
    { key: 'neutral',  label: 'Neutral',  accent: 'text-muted'    },
  ]

  let perRun = $derived(runs.map(run => ({ run, summary: activationSummary(run) })))

  function shortLabel(run: RunModel): string {
    const h = run.header
    if (!h) return run.id.slice(0, 6)
    const commit = h.commit_hash_short ?? '—'
    const dirty = h.commit_dirty ? '*' : ''
    return `seed ${h.seed} · ${commit}${dirty}`
  }

  /** Union of feature names seen in any run for this category. Used to
   *  build one row per feature so two runs with different emission sets
   *  align by name. */
  function featureUnion(summaries: ActivationSummary[], category: Category): string[] {
    const set = new Set<string>()
    for (const s of summaries) {
      for (const k of Object.keys(s[category])) set.add(k)
    }
    return Array.from(set)
  }

  function countOf(summary: ActivationSummary, category: Category, name: string): number {
    return summary[category][name] ?? 0
  }

  function isCanaryNeverFired(summary: ActivationSummary, name: string): boolean {
    return summary.neverFiredExpected.includes(name)
  }

  interface Row {
    name: string
    counts: number[]        // parallel to perRun
    sumAcrossRuns: number
    anyCanary: boolean
  }

  function buildRows(category: Category): { live: Row[]; silent: Row[] } {
    const summaries = perRun.map(x => x.summary)
    const names = featureUnion(summaries, category)
    const rows: Row[] = names.map(name => {
      const counts = summaries.map(s => countOf(s, category, name))
      const sumAcrossRuns = counts.reduce((a, b) => a + b, 0)
      const anyCanary = summaries.some(s => isCanaryNeverFired(s, name))
      return { name, counts, sumAcrossRuns, anyCanary }
    })
    // Canary-flagged rows (expected positive that still fired zero) float
    // to the top; within groups, higher cumulative count first, then alpha.
    rows.sort((a, b) => {
      if (a.anyCanary !== b.anyCanary) return a.anyCanary ? -1 : 1
      if (b.sumAcrossRuns !== a.sumAcrossRuns) return b.sumAcrossRuns - a.sumAcrossRuns
      return a.name.localeCompare(b.name)
    })
    const live = rows.filter(r => r.sumAcrossRuns > 0 || r.anyCanary)
    const silent = rows.filter(r => r.sumAcrossRuns === 0 && !r.anyCanary)
    return { live, silent }
  }

  function runsWord(n: number): string {
    return n === 1 ? 'run' : 'runs'
  }

  function sourceBadge(summaries: ActivationSummary[]): string {
    if (summaries.length === 0) return ''
    const sources = new Set(summaries.map(s => s.source))
    if (sources.size === 1) {
      const only = summaries[0].source
      if (only === 'event')  return 'source: SystemActivation events'
      if (only === 'footer') return 'source: footer aggregates only'
      return 'source: neither SystemActivation events nor footer aggregates present'
    }
    return 'source: mixed across runs (see per-run columns)'
  }

  let hasAnyData = $derived(perRun.some(x => x.summary.source !== 'none'))
  let expandedSilent = $state<Record<Category, boolean>>({
    positive: false, neutral: false, negative: false,
  })

  function toggleSilent(cat: Category) {
    expandedSilent = { ...expandedSilent, [cat]: !expandedSilent[cat] }
  }

  function barClass(category: Category): string {
    if (category === 'negative') return 'bg-negative/60'
    if (category === 'positive') return 'bg-positive/60'
    return 'bg-accent/50'
  }
</script>

<section class="flex flex-col gap-3">
  {#if runs.length === 0}
    <p class="text-sm italic text-muted">No runs loaded.</p>
  {:else if !hasAnyData}
    <p class="text-sm italic text-muted">
      No SystemActivation events or footer aggregates in the loaded runs.
      Re-run <code>just soak</code> against a current build to populate this grid.
    </p>
  {:else}
    {#if runs.length > 1}
      <div class="flex flex-wrap items-center gap-2 text-xs text-muted">
        <span>Comparing {runs.length} {runsWord(runs.length)}:</span>
        {#each perRun as { run } (run.id)}
          <span class="font-mono">{shortLabel(run)}</span>
        {/each}
      </div>
    {/if}

    {#each CATEGORIES as cat (cat.key)}
      {@const rows = buildRows(cat.key)}
      {@const summaries = perRun.map(x => x.summary)}
      <div class="bg-surface border border-border rounded-md">
        <header class="flex items-baseline justify-between gap-2 px-3 py-2 border-b border-border">
          <h3 class="m-0 text-sm {cat.accent}">{cat.label}</h3>
          <span class="text-[10px] text-muted italic">{sourceBadge(summaries)}</span>
        </header>

        {#if rows.live.length === 0 && rows.silent.length === 0}
          <p class="px-3 py-2 text-xs italic text-muted">
            No {cat.label.toLowerCase()} features emitted by any run.
          </p>
        {:else}
          <div class="px-3 py-2 flex flex-col gap-1">
            {#if runs.length > 1}
              <div class="grid items-center gap-2 text-[10px] text-muted uppercase tracking-wider"
                   style="grid-template-columns: minmax(10rem, 1fr) repeat({runs.length}, minmax(0, 1fr));">
                <span>feature</span>
                {#each perRun as { run } (run.id)}
                  <span class="font-mono truncate text-right" title={shortLabel(run)}>{shortLabel(run)}</span>
                {/each}
              </div>
            {/if}

            {#each rows.live as row (row.name)}
              {@const rowMax = Math.max(...row.counts, 1)}
              <div class="grid items-center gap-2 text-xs"
                   style="grid-template-columns: minmax(10rem, 1fr) repeat({runs.length}, minmax(0, 1fr));">
                <span class="flex items-center gap-1.5 min-w-0">
                  {#if row.anyCanary}
                    <span class="px-1 py-[1px] rounded bg-negative/20 text-negative text-[9px] font-bold leading-none tracking-wider"
                          title="Expected positive feature — canary failed on at least one run">CANARY</span>
                  {/if}
                  <span class="font-mono truncate" title={row.name}>{row.name}</span>
                </span>
                {#each row.counts as count, i (i)}
                  {@const share = rowMax > 0 ? count / rowMax : 0}
                  <div class="flex items-center gap-1.5">
                    <div class="flex-1 h-2 bg-surface-alt rounded overflow-hidden">
                      {#if count > 0}
                        <div class="h-full {barClass(cat.key)}"
                             style="width: {(share * 100).toFixed(1)}%"></div>
                      {/if}
                    </div>
                    <span class="font-mono tabular-nums w-10 text-right {count === 0 ? 'text-muted' : ''}">
                      {count === 0 ? '·' : count}
                    </span>
                  </div>
                {/each}
              </div>
            {/each}

            {#if rows.silent.length > 0}
              <button
                type="button"
                class="text-left text-[11px] text-muted italic bg-transparent border-none cursor-pointer px-0 mt-1"
                onclick={() => toggleSilent(cat.key)}
              >
                {expandedSilent[cat.key] ? '▾' : '▸'}
                {rows.silent.length} {cat.label.toLowerCase()}
                feature{rows.silent.length === 1 ? '' : 's'} never fired on any run
              </button>
              {#if expandedSilent[cat.key]}
                <div class="text-[11px] text-muted font-mono flex flex-wrap gap-x-3 gap-y-0.5 pl-3">
                  {#each rows.silent as row (row.name)}
                    <span>{row.name}</span>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</section>
