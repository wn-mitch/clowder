// Svelte store holding simulation runs loaded into the dashboard.
// Data lives in-memory only: no localStorage, no IndexedDB, no network.
// Everything is discarded on page unload.

import { writable, derived } from 'svelte/store'
import type { LoadedFile, RunModel } from '../lib/logs/types'
import { parseLogFile } from '../lib/logs/parser'

export const runs = writable<RunModel[]>([])
export const loading = writable(false)
export const loadError = writable<string | null>(null)

/** Selected run IDs for comparison views. Kept outside the run list so
 *  selection survives run additions/removals. */
export const selectedRunIds = writable<Set<string>>(new Set())

/** When set, the dashboard shows the CatDetailPanel for that cat instead
 *  of the overview. Null = overview. */
export const selectedCatName = writable<string | null>(null)

/** Which sub-tab of the Logs page is active. */
export type LogsSubTab = 'overview' | 'cat' | 'map'
export const logsSubTab = writable<LogsSubTab>('overview')

/** Run ID the Focal-trace page is currently scrubbing. Null = pick the
 *  first run with a trace attached. */
export const selectedTraceRunId = writable<string | null>(null)

export const selectedRuns = derived(
  [runs, selectedRunIds],
  ([$runs, $ids]) => $runs.filter(r => $ids.has(r.id)),
)

/** Load N dropped files into the store. Files that share a (seed, commit,
 *  duration) triple are auto-paired into a single RunModel with both
 *  events.jsonl and narrative.jsonl attached. */
export async function loadFiles(fileList: FileList | File[]): Promise<void> {
  const files = Array.from(fileList)
  if (files.length === 0) return

  loading.set(true)
  loadError.set(null)

  try {
    const parsed = await Promise.all(files.map(parseLogFile))

    runs.update($runs => {
      const next = [...$runs]
      for (const loaded of parsed) {
        const pairIndex = findPairIndex(next, loaded)
        if (pairIndex >= 0) {
          next[pairIndex] = mergeRun(next[pairIndex], loaded)
        } else {
          next.push(runFromFile(loaded))
        }
      }
      return next
    })
  } catch (e) {
    loadError.set((e as Error).message)
  } finally {
    loading.set(false)
  }
}

export function removeRun(id: string): void {
  runs.update($runs => $runs.filter(r => r.id !== id))
  selectedRunIds.update($ids => {
    if (!$ids.has(id)) return $ids
    const next = new Set($ids)
    next.delete(id)
    return next
  })
}

export function clearRuns(): void {
  runs.set([])
  selectedRunIds.set(new Set())
}

export function toggleRunSelection(id: string): void {
  selectedRunIds.update($ids => {
    const next = new Set($ids)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    return next
  })
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

function runFromFile(file: LoadedFile): RunModel {
  return {
    id: crypto.randomUUID(),
    files: [file],
    header: file.header,
    footer: file.footer,
    events: file.events,
    narrative: file.narrative,
    traces: file.traces,
    focalCat: file.header?.focal_cat ?? null,
    parseErrors: file.parseErrors.map(e => ({ ...e, message: `[${file.name}:${e.line}] ${e.message}` })),
  }
}

function mergeRun(run: RunModel, file: LoadedFile): RunModel {
  // Prefer the events header because it carries SimConstants.
  const preferEventsHeader = file.kind === 'events' ? file.header : run.header
  const header = preferEventsHeader ?? run.header ?? file.header
  return {
    ...run,
    files: [...run.files, file],
    header,
    footer: run.footer ?? file.footer,
    events: run.events.length > 0 ? run.events : file.events,
    narrative: run.narrative.length > 0 ? run.narrative : file.narrative,
    traces: run.traces.length > 0 ? run.traces : file.traces,
    focalCat: run.focalCat ?? file.header?.focal_cat ?? null,
    parseErrors: [
      ...run.parseErrors,
      ...file.parseErrors.map(e => ({ ...e, message: `[${file.name}:${e.line}] ${e.message}` })),
    ],
  }
}

function findPairIndex(runs: RunModel[], file: LoadedFile): number {
  if (!file.header) return -1
  const h = file.header
  return runs.findIndex(r => {
    const rh = r.header
    if (!rh) return false
    if (rh.seed !== h.seed) return false
    if (rh.duration_secs !== h.duration_secs) return false
    if (rh.commit_hash !== h.commit_hash) return false
    // Only pair if the existing run doesn't already have this kind.
    const hasEvents = r.files.some(f => f.kind === 'events')
    const hasNarrative = r.files.some(f => f.kind === 'narrative')
    const hasTrace = r.files.some(f => f.kind === 'trace')
    if (file.kind === 'events' && hasEvents) return false
    if (file.kind === 'narrative' && hasNarrative) return false
    // Trace files pair by focal-cat name too — a second trace with the
    // same focal is a new run (re-run of the same seed/commit), a
    // second trace with a different focal stays unpaired as its own
    // run so the user can switch between focal cats.
    if (file.kind === 'trace') {
      if (hasTrace) return false
      if (r.focalCat && h.focal_cat && r.focalCat !== h.focal_cat) return false
    }
    return true
  })
}
