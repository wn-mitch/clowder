<script lang="ts">
  import { removeRun, selectedRunIds, toggleRunSelection } from '../../stores/runs'
  import type { RunSummary } from '../../lib/logs/metrics'

  interface Props {
    summaries: RunSummary[]
  }

  let { summaries }: Props = $props()

  type SortKey =
    | 'commitTime' | 'seed' | 'durationSecs' | 'finalWelfare' | 'finalLivingCats'
    | 'deathsTotal' | 'positiveFeaturesActive' | 'negativeEventsTotal'

  let sortKey = $state<SortKey>('commitTime')
  let sortDir = $state<'asc' | 'desc'>('desc')

  function sortValue(s: RunSummary, key: SortKey): number | string {
    switch (key) {
      case 'commitTime': return s.commitTime ?? ''
      case 'seed': return s.seed ?? Number.NEGATIVE_INFINITY
      case 'durationSecs': return s.durationSecs ?? Number.NEGATIVE_INFINITY
      case 'finalWelfare': return s.finalWelfare ?? Number.NEGATIVE_INFINITY
      case 'finalLivingCats': return s.finalLivingCats ?? Number.NEGATIVE_INFINITY
      case 'deathsTotal': return s.deathsTotal
      case 'positiveFeaturesActive': return s.positiveFeaturesActive ?? Number.NEGATIVE_INFINITY
      case 'negativeEventsTotal': return s.negativeEventsTotal ?? Number.NEGATIVE_INFINITY
    }
  }

  let sorted = $derived.by(() => {
    const out = [...summaries]
    out.sort((a, b) => {
      const av = sortValue(a, sortKey)
      const bv = sortValue(b, sortKey)
      const cmp = av < bv ? -1 : av > bv ? 1 : 0
      return sortDir === 'asc' ? cmp : -cmp
    })
    return out
  })

  function setSort(key: SortKey) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc'
    else { sortKey = key; sortDir = 'desc' }
  }

  function sortIndicator(key: SortKey): string {
    if (sortKey !== key) return ''
    return sortDir === 'asc' ? ' ▲' : ' ▼'
  }

  function formatPercent(n: number | null): string {
    return n === null ? '—' : `${(n * 100).toFixed(0)}%`
  }

  function formatInt(n: number | null): string {
    return n === null ? '—' : String(n)
  }

  function formatDate(iso: string | null): string {
    if (!iso) return '—'
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    return d.toISOString().slice(0, 10)
  }

  function formatFeatures(active: number | null, total: number | null): string {
    if (active === null) return '—'
    if (total === null) return String(active)
    return `${active}/${total}`
  }

  function formatDeaths(cause: Record<string, number>, total: number): string {
    if (total === 0) return '0'
    const entries = Object.entries(cause).sort((a, b) => b[1] - a[1]).slice(0, 3)
    const summary = entries.map(([k, v]) => `${v} ${k}`).join(', ')
    return total > 0 ? `${total} (${summary})` : String(total)
  }
</script>

<div class="overflow-x-auto bg-surface border border-border rounded-md">
  <table class="w-full border-collapse text-sm">
    <thead>
      <tr class="text-xs text-muted uppercase tracking-wide border-b border-border">
        <th class="px-2 py-2 text-left w-6" aria-label="Select"></th>
        <th class="px-2 py-2 text-left">Run</th>
        <th class="px-2 py-2 text-left cursor-pointer" onclick={() => setSort('commitTime')}>Commit{sortIndicator('commitTime')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('seed')}>Seed{sortIndicator('seed')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('durationSecs')}>Dur{sortIndicator('durationSecs')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('finalWelfare')}>Welfare{sortIndicator('finalWelfare')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('finalLivingCats')}>Living{sortIndicator('finalLivingCats')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('deathsTotal')}>Deaths{sortIndicator('deathsTotal')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('positiveFeaturesActive')} title="Positive features active / total">Pos{sortIndicator('positiveFeaturesActive')}</th>
        <th class="px-2 py-2 text-right cursor-pointer" onclick={() => setSort('negativeEventsTotal')} title="Negative events total">Neg{sortIndicator('negativeEventsTotal')}</th>
        <th class="px-2 py-2 text-left" title="Neutral features active / total">Neut</th>
        <th class="px-2 py-2 text-right" title="Final food-stores quantity">Food</th>
        <th class="px-2 py-2 text-left" title="Final predator census: Fox / Hawk / Snake / Shadow-Fox">Pred</th>
        <th class="px-2 py-2 text-right w-16"></th>
      </tr>
    </thead>
    <tbody>
      {#each sorted as s (s.runId)}
        <tr class="border-b border-border/40 hover:bg-surface-alt/50 transition-colors duration-75">
          <td class="px-2 py-2">
            <input
              type="checkbox"
              class="accent-accent cursor-pointer"
              checked={$selectedRunIds.has(s.runId)}
              onchange={() => toggleRunSelection(s.runId)}
              aria-label="Select run for comparison"
            />
          </td>
          <td class="px-2 py-2 min-w-0">
            <div class="font-mono text-xs text-txt break-all" title={s.filenames.join(', ')}>{s.filenames.join(' + ')}</div>
            {#if s.parseErrorsCount > 0}
              <div class="text-xs text-warning">{s.parseErrorsCount} parse errors</div>
            {/if}
          </td>
          <td class="px-2 py-2">
            <div class="font-mono text-accent">
              {s.commitHashShort ?? '—'}{s.commitDirty ? '*' : ''}
            </div>
            <div class="text-xs text-muted">{formatDate(s.commitTime)}</div>
          </td>
          <td class="px-2 py-2 text-right font-mono">{formatInt(s.seed)}</td>
          <td class="px-2 py-2 text-right font-mono">{s.durationSecs === null ? '—' : `${s.durationSecs}s`}</td>
          <td class="px-2 py-2 text-right font-mono">{formatPercent(s.finalWelfare)}</td>
          <td class="px-2 py-2 text-right font-mono">{formatInt(s.finalLivingCats)}</td>
          <td class="px-2 py-2 text-right font-mono" title={Object.entries(s.deathsByCause).map(([k, v]) => `${k}: ${v}`).join('\n') || 'no deaths recorded'}>
            {formatDeaths(s.deathsByCause, s.deathsTotal)}
          </td>
          <td class="px-2 py-2 text-right font-mono">{formatFeatures(s.positiveFeaturesActive, s.positiveFeaturesTotal)}</td>
          <td class="px-2 py-2 text-right font-mono">{formatInt(s.negativeEventsTotal)}</td>
          <td class="px-2 py-2 font-mono">{formatFeatures(s.neutralFeaturesActive, s.neutralFeaturesTotal)}</td>
          <td class="px-2 py-2 text-right font-mono" title={s.finalFoodStores === null ? 'no FoodLevel events' : `${s.finalFoodStores.toFixed(1)} units`}>
            {s.finalFoodStores === null ? '—' : s.finalFoodStores.toFixed(0)}
          </td>
          <td class="px-2 py-2 font-mono text-xs" title={s.finalWildlife ? `foxes ${s.finalWildlife.foxes}, hawks ${s.finalWildlife.hawks}, snakes ${s.finalWildlife.snakes}, shadow-foxes ${s.finalWildlife.shadow_foxes}` : 'no WildlifePopulation events'}>
            {#if s.finalWildlife}
              {s.finalWildlife.foxes}/{s.finalWildlife.hawks}/{s.finalWildlife.snakes}/{s.finalWildlife.shadow_foxes}
            {:else}
              —
            {/if}
          </td>
          <td class="px-2 py-2 text-right">
            <button
              type="button"
              class="text-xs"
              onclick={() => removeRun(s.runId)}
              aria-label="Remove run"
            >×</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<p class="text-xs text-muted mt-2">
  <span class="font-mono">*</span> = commit-dirty (working tree had uncommitted changes when the binary was built).
  Select two or more rows to compare.
</p>
