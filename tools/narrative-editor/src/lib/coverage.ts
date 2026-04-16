// Coverage computation engine for narrative templates.
// Computes per-axis and pairwise coverage heatmaps, accounting for wildcard (undefined) fields.

import type { NarrativeTemplate } from './types'
import type { CoverageAxisId } from './schema'
import {
  MOODS, WEATHERS, SEASONS, DAY_PHASES, LIFE_STAGES, TERRAINS,
  PERSONALITY_AXES, PERSONALITY_BUCKETS, NEED_AXES, NEED_LEVELS,
} from './schema'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Pre-filter templates by action and event. */
function applyFilters(
  templates: NarrativeTemplate[],
  filterAction?: string,
  filterEvent?: string,
): NarrativeTemplate[] {
  return templates.filter(t => {
    if (filterAction && t.action !== filterAction && t.action !== undefined) return false
    if (filterEvent && t.event !== filterEvent) return false
    return true
  })
}

/**
 * Check if a template matches a specific value on a given axis.
 * When `explicitOnly` is true, only templates that explicitly set the field count —
 * undefined (wildcard) fields are treated as non-matching.
 */
function matchesAxis(
  t: NarrativeTemplate,
  axis: CoverageAxisId,
  value: string,
  explicitOnly = false,
): boolean {
  const field = t[axis]
  if (field === undefined) return !explicitOnly
  return field === value
}

// ---------------------------------------------------------------------------
// Heatmap & cell inspection
// ---------------------------------------------------------------------------

/** Count templates matching a value on a single axis. */
export function countForValue(
  templates: NarrativeTemplate[],
  axis: CoverageAxisId,
  value: string,
  filterAction?: string,
  filterEvent?: string,
  explicitOnly?: boolean,
): number {
  return applyFilters(templates, filterAction, filterEvent)
    .filter(t => matchesAxis(t, axis, value, explicitOnly))
    .length
}

/** Compute pairwise heatmap: for each (xValue, yValue) cell, count matching templates. */
export function computeHeatmap(
  templates: NarrativeTemplate[],
  xAxis: CoverageAxisId,
  xValues: string[],
  yAxis: CoverageAxisId,
  yValues: string[],
  filterAction?: string,
  filterEvent?: string,
  explicitOnly?: boolean,
): number[][] {
  const filtered = applyFilters(templates, filterAction, filterEvent)
  const grid: number[][] = []

  for (const yVal of yValues) {
    const row: number[] = []
    for (const xVal of xValues) {
      const count = filtered.filter(t =>
        matchesAxis(t, xAxis, xVal, explicitOnly) && matchesAxis(t, yAxis, yVal, explicitOnly)
      ).length
      row.push(count)
    }
    grid.push(row)
  }

  return grid
}

/** Find templates matching a specific cell in the heatmap. */
export function templatesForCell(
  templates: NarrativeTemplate[],
  xAxis: CoverageAxisId,
  xValue: string,
  yAxis: CoverageAxisId,
  yValue: string,
  filterAction?: string,
  filterEvent?: string,
  explicitOnly?: boolean,
): NarrativeTemplate[] {
  return applyFilters(templates, filterAction, filterEvent)
    .filter(t =>
      matchesAxis(t, xAxis, xValue, explicitOnly) && matchesAxis(t, yAxis, yValue, explicitOnly)
    )
}

// ---------------------------------------------------------------------------
// Gaps (grid-based, kept for heatmap zero-cell detection)
// ---------------------------------------------------------------------------

export interface GapEntry {
  xAxis: CoverageAxisId
  xValue: string
  xLabel: string
  yAxis: CoverageAxisId
  yValue: string
  yLabel: string
  count: number
}

/** Find zero-coverage cells across a pair of axes. */
export function findGaps(
  templates: NarrativeTemplate[],
  xAxis: CoverageAxisId,
  xValues: { value: string; label: string }[],
  yAxis: CoverageAxisId,
  yValues: { value: string; label: string }[],
  filterAction?: string,
  filterEvent?: string,
  explicitOnly?: boolean,
): GapEntry[] {
  const filtered = applyFilters(templates, filterAction, filterEvent)
  const gaps: GapEntry[] = []

  for (const yEntry of yValues) {
    for (const xEntry of xValues) {
      const count = filtered.filter(t =>
        matchesAxis(t, xAxis, xEntry.value, explicitOnly) && matchesAxis(t, yAxis, yEntry.value, explicitOnly)
      ).length
      if (count === 0) {
        gaps.push({
          xAxis, xValue: xEntry.value, xLabel: xEntry.label,
          yAxis, yValue: yEntry.value, yLabel: yEntry.label,
          count,
        })
      }
    }
  }

  return gaps
}

// ---------------------------------------------------------------------------
// Condition coverage summary (replaces gaps tab)
// ---------------------------------------------------------------------------

export interface AxisCoverage {
  axisId: string
  axisLabel: string
  group: 'simple' | 'personality' | 'needs'
  totalValues: number
  coveredValues: { value: string; label: string; count: number }[]
  missingValues: { value: string; label: string }[]
  status: 'partial' | 'none' | 'full'
}

const SIMPLE_AXES: { id: CoverageAxisId; label: string; values: { value: string; label: string }[] }[] = [
  { id: 'mood',       label: 'Mood',       values: MOODS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'weather',    label: 'Weather',    values: WEATHERS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'season',     label: 'Season',     values: SEASONS.map(e => ({ value: e.value, label: e.label })) },
  { id: 'day_phase',  label: 'Day Phase',  values: DAY_PHASES.map(e => ({ value: e.value, label: e.label })) },
  { id: 'life_stage', label: 'Life Stage', values: LIFE_STAGES.map(e => ({ value: e.value, label: e.label })) },
  { id: 'terrain',    label: 'Terrain',    values: TERRAINS.map(e => ({ value: e.value, label: e.label })) },
]

/** Compute per-axis condition coverage for the filtered template set. */
export function computeConditionCoverage(
  templates: NarrativeTemplate[],
  filterAction?: string,
  filterEvent?: string,
): AxisCoverage[] {
  const filtered = applyFilters(templates, filterAction, filterEvent)
  const results: AxisCoverage[] = []

  // Simple scalar axes
  for (const axis of SIMPLE_AXES) {
    const covered: AxisCoverage['coveredValues'] = []
    const missing: AxisCoverage['missingValues'] = []

    for (const v of axis.values) {
      const count = filtered.filter(t => t[axis.id] === v.value).length
      if (count > 0) {
        covered.push({ value: v.value, label: v.label, count })
      } else {
        missing.push({ value: v.value, label: v.label })
      }
    }

    const status = covered.length === 0 ? 'none'
      : covered.length === axis.values.length ? 'full'
      : 'partial'

    results.push({
      axisId: axis.id,
      axisLabel: axis.label,
      group: 'simple',
      totalValues: axis.values.length,
      coveredValues: covered,
      missingValues: missing,
      status,
    })
  }

  // Personality axes
  for (const pAxis of PERSONALITY_AXES) {
    const covered: AxisCoverage['coveredValues'] = []
    const missing: AxisCoverage['missingValues'] = []

    for (const bucket of PERSONALITY_BUCKETS) {
      const count = filtered.filter(t =>
        t.personality.some(p => p.axis === pAxis.value && p.bucket === bucket.value)
      ).length
      if (count > 0) {
        covered.push({ value: bucket.value, label: bucket.label, count })
      } else {
        missing.push({ value: bucket.value, label: bucket.label })
      }
    }

    const status = covered.length === 0 ? 'none'
      : covered.length === PERSONALITY_BUCKETS.length ? 'full'
      : 'partial'

    results.push({
      axisId: `personality:${pAxis.value}`,
      axisLabel: pAxis.label,
      group: 'personality',
      totalValues: PERSONALITY_BUCKETS.length,
      coveredValues: covered,
      missingValues: missing,
      status,
    })
  }

  // Need axes
  for (const nAxis of NEED_AXES) {
    const covered: AxisCoverage['coveredValues'] = []
    const missing: AxisCoverage['missingValues'] = []

    for (const level of NEED_LEVELS) {
      const count = filtered.filter(t =>
        t.needs.some(n => n.axis === nAxis.value && n.level === level.value)
      ).length
      if (count > 0) {
        covered.push({ value: level.value, label: level.label, count })
      } else {
        missing.push({ value: level.value, label: level.label })
      }
    }

    const status = covered.length === 0 ? 'none'
      : covered.length === NEED_LEVELS.length ? 'full'
      : 'partial'

    results.push({
      axisId: `need:${nAxis.value}`,
      axisLabel: nAxis.label,
      group: 'needs',
      totalValues: NEED_LEVELS.length,
      coveredValues: covered,
      missingValues: missing,
      status,
    })
  }

  // Sort: partial first, then none, then full. Within each group, fewest covered first.
  const statusOrder = { partial: 0, none: 1, full: 2 }
  results.sort((a, b) => {
    const so = statusOrder[a.status] - statusOrder[b.status]
    if (so !== 0) return so
    return a.coveredValues.length - b.coveredValues.length
  })

  return results
}

// ---------------------------------------------------------------------------
// Total gaps (summed across all actions independently)
// ---------------------------------------------------------------------------

/** Sum missing values across all actions, computing coverage per action independently. */
export function computeTotalGaps(templates: NarrativeTemplate[]): number {
  const actions = new Set<string>()
  for (const t of templates) {
    if (t.action) actions.add(t.action)
  }

  let total = 0
  for (const action of actions) {
    const coverage = computeConditionCoverage(templates, action)
    for (const axis of coverage) {
      total += axis.missingValues.length
    }
  }
  return total
}

// ---------------------------------------------------------------------------
// Event helpers
// ---------------------------------------------------------------------------

/** Extract sorted unique event values from all loaded templates. */
export function uniqueEvents(templates: NarrativeTemplate[]): string[] {
  const events = new Set<string>()
  for (const t of templates) {
    if (t.event) events.add(t.event)
  }
  return Array.from(events).sort()
}

// ---------------------------------------------------------------------------
// Per-action summary (unchanged)
// ---------------------------------------------------------------------------

/** Per-action template count summary. */
export interface ActionSummary {
  action: string
  label: string
  count: number
  uniqueAxes: number
}

export function perActionSummary(templates: NarrativeTemplate[]): ActionSummary[] {
  const byAction = new Map<string, NarrativeTemplate[]>()

  for (const t of templates) {
    const key = t.action ?? '(unconditioned)'
    if (!byAction.has(key)) byAction.set(key, [])
    byAction.get(key)!.push(t)
  }

  return Array.from(byAction.entries()).map(([action, ts]) => {
    const axes = new Set<string>()
    for (const t of ts) {
      if (t.day_phase) axes.add('day_phase')
      if (t.season) axes.add('season')
      if (t.weather) axes.add('weather')
      if (t.mood) axes.add('mood')
      if (t.life_stage) axes.add('life_stage')
      if (t.terrain) axes.add('terrain')
      if (t.event) axes.add('event')
      if (t.personality.length > 0) axes.add('personality')
      if (t.needs.length > 0) axes.add('needs')
    }

    return {
      action,
      label: action,
      count: ts.length,
      uniqueAxes: axes.size,
    }
  }).sort((a, b) => b.count - a.count)
}
