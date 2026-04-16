<script lang="ts">
  import { allTemplates, selectedFile } from '../stores/templates'
  import { COVERAGE_AXES, ACTIONS } from '../lib/schema'
  import {
    computeHeatmap, templatesForCell, perActionSummary,
    computeConditionCoverage, uniqueEvents, computeTotalGaps,
  } from '../lib/coverage'
  import type { NarrativeTemplate } from '../lib/types'
  import type { CoverageAxisId } from '../lib/schema'

  type View = 'heatmap' | 'summary' | 'gaps'
  let view = $state<View>('heatmap')

  // Shared filter state (persists across tabs)
  let filterAction = $state('')
  let filterEvent = $state('')

  // Heatmap controls
  let xAxisId = $state<CoverageAxisId>('action')
  let yAxisId = $state<CoverageAxisId>('mood')
  let explicitOnly = $state(false)

  // Cell detail
  let inspectedTemplates = $state<NarrativeTemplate[] | null>(null)
  let inspectedLabel = $state('')

  // Gaps: collapsed "not targeted" section
  let showUntargeted = $state(false)

  // Filter action dropdown to actions present in the selected file (or all loaded actions)
  let availableActions = $derived.by(() => {
    const source = $selectedFile ? $selectedFile.templates : $allTemplates
    const actionSet = new Set<string>()
    for (const t of source) {
      if (t.action) actionSet.add(t.action)
    }
    return ACTIONS.filter(a => actionSet.has(a.value))
  })

  // Reset filter when selected file changes and current filter is no longer available
  $effect(() => {
    if (filterAction && !availableActions.some(a => a.value === filterAction)) {
      filterAction = ''
    }
  })

  // Total gaps summed across all actions independently
  let totalGapCount = $derived(computeTotalGaps($allTemplates))

  // Dynamic event axis
  let eventValues = $derived(uniqueEvents($allTemplates))
  let allAxes = $derived([
    ...COVERAGE_AXES,
    ...(eventValues.length > 0 ? [{
      id: 'event' as CoverageAxisId,
      label: 'Event',
      values: eventValues.map(e => ({ value: e, label: e })),
    }] : []),
  ])

  let xAxis = $derived(allAxes.find(a => a.id === xAxisId) ?? allAxes[0])
  let yAxis = $derived(allAxes.find(a => a.id === yAxisId) ?? allAxes[1])

  let heatmap = $derived(
    computeHeatmap(
      $allTemplates, xAxisId, xAxis.values.map(v => v.value),
      yAxisId, yAxis.values.map(v => v.value),
      filterAction || undefined, filterEvent || undefined, explicitOnly,
    )
  )

  let summary = $derived(perActionSummary($allTemplates))

  let conditionCoverage = $derived(
    computeConditionCoverage($allTemplates, filterAction || undefined, filterEvent || undefined)
  )
  let partialAxes = $derived(conditionCoverage.filter(a => a.status === 'partial'))
  let untargetedAxes = $derived(conditionCoverage.filter(a => a.status === 'none'))
  let fullAxes = $derived(conditionCoverage.filter(a => a.status === 'full'))
  let partialCount = $derived(partialAxes.length)

  // Separate personality/needs untargeted for the summary line
  let untargetedPersonality = $derived(untargetedAxes.filter(a => a.group === 'personality'))
  let untargetedNeeds = $derived(untargetedAxes.filter(a => a.group === 'needs'))
  let untargetedSimple = $derived(untargetedAxes.filter(a => a.group === 'simple'))

  // Filtered template count for the gaps tab header
  let filteredCount = $derived(
    $allTemplates.filter(t => {
      if (filterAction && t.action !== filterAction && t.action !== undefined) return false
      if (filterEvent && t.event !== filterEvent) return false
      return true
    }).length
  )

  function cellColor(count: number): string {
    if (count === 0) return 'var(--color-negative)'
    if (count <= 2) return 'var(--color-warning)'
    return 'var(--color-positive)'
  }

  function inspectCell(xi: number, yi: number) {
    const xVal = xAxis.values[xi]
    const yVal = yAxis.values[yi]
    inspectedTemplates = templatesForCell(
      $allTemplates, xAxisId, xVal.value, yAxisId, yVal.value,
      filterAction || undefined, filterEvent || undefined, explicitOnly,
    )
    inspectedLabel = `${xVal.label} × ${yVal.label}`
  }

  function maxCount(): number {
    let max = 0
    for (const row of heatmap) for (const c of row) if (c > max) max = c
    return max || 1
  }
</script>

{#if $allTemplates.length === 0}
  <div class="flex items-center justify-center min-h-[300px] text-muted italic">
    <p>Load <code>.ron</code> files to see coverage analysis</p>
  </div>
{:else}
  <div class="bg-surface border border-border rounded-md p-4">
    <div class="flex gap-1 mb-4 border-b border-border pb-2">
      <button
        class={view === 'heatmap'
          ? 'px-3 py-1 border-none bg-transparent text-accent text-sm cursor-pointer rounded-t-md border-b-2 border-accent'
          : 'px-3 py-1 border-none bg-transparent text-muted text-sm cursor-pointer rounded-t-md hover:text-txt hover:bg-surface-alt'}
        onclick={() => view = 'heatmap'}
      >Heatmap</button>
      <button
        class={view === 'summary'
          ? 'px-3 py-1 border-none bg-transparent text-accent text-sm cursor-pointer rounded-t-md border-b-2 border-accent'
          : 'px-3 py-1 border-none bg-transparent text-muted text-sm cursor-pointer rounded-t-md hover:text-txt hover:bg-surface-alt'}
        onclick={() => view = 'summary'}
      >Summary</button>
      <button
        class={view === 'gaps'
          ? 'px-3 py-1 border-none bg-transparent text-accent text-sm cursor-pointer rounded-t-md border-b-2 border-accent'
          : 'px-3 py-1 border-none bg-transparent text-muted text-sm cursor-pointer rounded-t-md hover:text-txt hover:bg-surface-alt'}
        onclick={() => view = 'gaps'}
      >
        Gaps {#if totalGapCount > 0}<span class="text-negative font-bold">({totalGapCount})</span>{/if}
      </button>
    </div>

    {#if view === 'heatmap'}
      <div class="flex gap-4 mb-4 flex-wrap items-end">
        <div class="flex flex-col gap-1">
          <label class="text-xs text-muted uppercase tracking-wide">X Axis</label>
          <select class="min-w-[140px]" bind:value={xAxisId}>
            {#each allAxes as axis}
              <option value={axis.id}>{axis.label}</option>
            {/each}
          </select>
        </div>
        <div class="flex flex-col gap-1">
          <label class="text-xs text-muted uppercase tracking-wide">Y Axis</label>
          <select class="min-w-[140px]" bind:value={yAxisId}>
            {#each allAxes as axis}
              <option value={axis.id}>{axis.label}</option>
            {/each}
          </select>
        </div>
        <div class="flex flex-col gap-1">
          <label class="text-xs text-muted uppercase tracking-wide">Filter Action</label>
          <select class="min-w-[140px]" bind:value={filterAction}>
            <option value="">All</option>
            {#each availableActions as a}
              <option value={a.value}>{a.label}</option>
            {/each}
          </select>
        </div>
        {#if eventValues.length > 0}
          <div class="flex flex-col gap-1">
            <label class="text-xs text-muted uppercase tracking-wide">Filter Event</label>
            <select class="min-w-[140px]" bind:value={filterEvent}>
              <option value="">All</option>
              {#each eventValues as ev}
                <option value={ev}>{ev}</option>
              {/each}
            </select>
          </div>
        {/if}
        <label class="flex items-center gap-1.5 text-xs text-muted cursor-pointer pb-0.5" title="Only count templates that explicitly set a value on each axis (wildcards excluded)">
          <input type="checkbox" bind:checked={explicitOnly} class="accent-accent" />
          Explicit only
        </label>
      </div>

      <div class="overflow-x-auto mb-3">
        <table class="border-collapse text-xs">
          <thead>
            <tr>
              <th class="px-2 py-1 text-center whitespace-nowrap text-right text-muted text-[0.7rem] font-normal">{yAxis.label} \ {xAxis.label}</th>
              {#each xAxis.values as xVal}
                <th class="px-2 py-1 text-center whitespace-nowrap text-[0.7rem] text-muted font-normal [writing-mode:vertical-lr] h-20">{xVal.label}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each yAxis.values as yVal, yi}
              <tr>
                <th class="px-2 py-1 text-center whitespace-nowrap text-right font-normal text-muted text-xs pr-3">{yVal.label}</th>
                {#each xAxis.values as xVal, xi}
                  <td
                    class="px-2 py-1 text-center whitespace-nowrap cursor-pointer text-bg font-bold min-w-8 rounded-sm hover:outline-2 hover:outline-accent hover:-outline-offset-1"
                    style="background: {cellColor(heatmap[yi][xi])}; opacity: {0.3 + 0.7 * (heatmap[yi][xi] / maxCount())}"
                    title="{xVal.label} × {yVal.label}: {heatmap[yi][xi]} templates"
                    role="button"
                    tabindex="0"
                    onclick={() => inspectCell(xi, yi)}
                    onkeydown={(e) => e.key === 'Enter' && inspectCell(xi, yi)}
                  >
                    {heatmap[yi][xi]}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="flex gap-4 text-xs text-muted">
        <span class="flex items-center gap-1"><span class="inline-block w-3 h-3 rounded-sm" style="background: var(--color-negative)"></span> 0 (gap)</span>
        <span class="flex items-center gap-1"><span class="inline-block w-3 h-3 rounded-sm" style="background: var(--color-warning)"></span> 1-2</span>
        <span class="flex items-center gap-1"><span class="inline-block w-3 h-3 rounded-sm" style="background: var(--color-positive)"></span> 3+</span>
      </div>

      {#if inspectedTemplates}
        <div class="mt-4 p-3 bg-bg rounded-md">
          <div class="flex justify-between items-center mb-2">
            <h3 class="m-0 text-sm text-accent border-none p-0">{inspectedLabel} ({inspectedTemplates.length} templates)</h3>
            <button class="p-0 border-none bg-transparent text-accent text-sm cursor-pointer underline hover:text-accent-hover hover:bg-transparent" onclick={() => inspectedTemplates = null}>Close</button>
          </div>
          {#each inspectedTemplates as t}
            <div class="px-2 py-1.5 my-1 text-sm border-l-2 border-border pl-2">
              <span class="badge">{t.tier}</span>
              <span class="ml-2">{t.text}</span>
            </div>
          {/each}
        </div>
      {/if}

    {:else if view === 'summary'}
      <div>
        <h3 class="text-sm m-0 mb-3 text-accent border-none p-0">Templates per Action</h3>
        {#each summary as s}
          <div class="flex items-center gap-2 mb-1.5 text-sm">
            <span class="w-30 text-right text-muted shrink-0">{s.label}</span>
            <div class="flex-1 h-3.5 bg-bar-bg rounded-sm overflow-hidden">
              <div class="h-full bg-accent rounded-sm transition-all duration-300" style="width: {Math.min(100, (s.count / (summary[0]?.count || 1)) * 100)}%"></div>
            </div>
            <span class="w-8 text-right font-mono text-xs text-muted">{s.count}</span>
            <span class="w-12 text-[0.7rem] text-muted" title="Condition axes used">{s.uniqueAxes} axes</span>
          </div>
        {/each}
      </div>

    {:else}
      <!-- Condition Coverage (Gaps) -->
      <div>
        <div class="flex gap-4 mb-4 flex-wrap items-end">
          <div class="flex flex-col gap-1">
            <label class="text-xs text-muted uppercase tracking-wide">Filter Action</label>
            <select class="min-w-[140px]" bind:value={filterAction}>
              <option value="">All</option>
              {#each availableActions as a}
                <option value={a.value}>{a.label}</option>
              {/each}
            </select>
          </div>
          {#if eventValues.length > 0}
            <div class="flex flex-col gap-1">
              <label class="text-xs text-muted uppercase tracking-wide">Filter Event</label>
              <select class="min-w-[140px]" bind:value={filterEvent}>
                <option value="">All</option>
                {#each eventValues as ev}
                  <option value={ev}>{ev}</option>
                {/each}
              </select>
            </div>
          {/if}
        </div>

        <p class="text-xs text-muted mb-4">{filteredCount} templates match filters</p>

        {#if filteredCount === 0}
          <p class="text-muted italic my-4">No templates match the selected filters.</p>
        {:else}
          <!-- Partial coverage axes (most actionable) -->
          {#if partialAxes.length > 0}
            <div class="mb-4">
              <h4 class="text-xs text-muted uppercase tracking-wide m-0 mb-2">Partial Coverage</h4>
              {#each partialAxes as axis}
                <div class="mb-3 p-2 bg-bg rounded-md border border-border">
                  <div class="flex items-center gap-2 mb-1.5">
                    {#if axis.group === 'personality'}
                      <span class="text-xs text-muted">Personality:</span>
                    {:else if axis.group === 'needs'}
                      <span class="text-xs text-muted">Need:</span>
                    {/if}
                    <span class="text-sm font-medium text-txt">{axis.axisLabel}</span>
                    <span class="text-xs text-warning">{axis.coveredValues.length}/{axis.totalValues}</span>
                  </div>
                  <div class="flex flex-wrap gap-1">
                    {#each axis.coveredValues as v}
                      <span class="px-2 py-0.5 text-xs rounded-sm bg-positive/20 text-positive border border-positive/30">
                        {v.label} <span class="opacity-70">({v.count})</span>
                      </span>
                    {/each}
                    {#each axis.missingValues as v}
                      <span class="px-2 py-0.5 text-xs rounded-sm border border-border text-muted">
                        {v.label}
                      </span>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          <!-- Full coverage axes -->
          {#if fullAxes.length > 0}
            <div class="mb-4">
              <h4 class="text-xs text-muted uppercase tracking-wide m-0 mb-2">Full Coverage</h4>
              {#each fullAxes as axis}
                <div class="mb-2 p-2 bg-bg rounded-md border border-border">
                  <div class="flex items-center gap-2">
                    {#if axis.group === 'personality'}
                      <span class="text-xs text-muted">Personality:</span>
                    {:else if axis.group === 'needs'}
                      <span class="text-xs text-muted">Need:</span>
                    {/if}
                    <span class="text-sm text-txt">{axis.axisLabel}</span>
                    <span class="text-xs text-positive">{axis.totalValues}/{axis.totalValues}</span>
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          <!-- Not targeted axes (collapsed) -->
          {#if untargetedAxes.length > 0}
            <div class="mt-3">
              <button
                class="text-xs text-muted cursor-pointer border-none bg-transparent p-0 hover:text-txt"
                onclick={() => showUntargeted = !showUntargeted}
              >
                {showUntargeted ? '▼' : '▶'}
                {untargetedAxes.length} axes not targeted
                {#if untargetedSimple.length > 0}
                  <span class="ml-1 opacity-70">({untargetedSimple.map(a => a.axisLabel).join(', ')}{untargetedPersonality.length > 0 ? `, ${untargetedPersonality.length} personality traits` : ''}{untargetedNeeds.length > 0 ? `, ${untargetedNeeds.length} needs` : ''})</span>
                {:else}
                  <span class="ml-1 opacity-70">({untargetedPersonality.length > 0 ? `${untargetedPersonality.length} personality traits` : ''}{untargetedNeeds.length > 0 ? `${untargetedPersonality.length > 0 ? ', ' : ''}${untargetedNeeds.length} needs` : ''})</span>
                {/if}
              </button>
              {#if showUntargeted}
                <div class="mt-2 flex flex-wrap gap-1">
                  {#each untargetedAxes as axis}
                    <span class="px-2 py-0.5 text-xs rounded-sm border border-border/50 text-muted/60">
                      {#if axis.group === 'personality'}Personality: {/if}{#if axis.group === 'needs'}Need: {/if}{axis.axisLabel}
                    </span>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>
    {/if}
  </div>
{/if}
