// Trace-page state — the loaded-run list itself stays in `runs.ts`, but
// the scrubber's local state (which run, which tick, which DSE is
// expanded) lives here so the logs page and trace page don't fight over
// the same store.

import { derived, writable, type Readable } from 'svelte/store'
import { runs, selectedTraceRunId } from './runs'
import {
  buildFrameIndex, nearestDecisionTick, type FrameIndex, type Frame,
} from '../lib/logs/trace'
import type { RunModel } from '../lib/logs/types'

/** Tick the scrubber is pinned to. Null = snap to first decision tick
 *  whenever the run changes. */
export const focalTick = writable<number | null>(null)

/** When a DSE card is expanded in L2, its name goes here. Null = show
 *  all cards in compact form. */
export const selectedDse = writable<string | null>(null)

/** Runs that carry focal-cat traces — the trace-page run picker's
 *  options. */
export const runsWithTraces: Readable<RunModel[]> = derived(
  runs,
  $runs => $runs.filter(r => r.traces.length > 0),
)

/** The active trace run — either the explicitly-selected one, or the
 *  first run with traces as a fallback. Null when none exists. */
export const activeTraceRun: Readable<RunModel | null> = derived(
  [runs, selectedTraceRunId],
  ([$runs, $id]) => {
    if ($id) {
      const hit = $runs.find(r => r.id === $id && r.traces.length > 0)
      if (hit) return hit
    }
    return $runs.find(r => r.traces.length > 0) ?? null
  },
)

/** Frame index for the active run. Rebuilt whenever the run changes —
 *  `buildFrameIndex` is O(n) and runs once per dropped trace file, so
 *  caching isn't load-bearing. Still, memoise on the run id so scrubbing
 *  doesn't trigger rebuilds. */
const indexCache = new Map<string, FrameIndex>()

export const frameIndex: Readable<FrameIndex | null> = derived(
  activeTraceRun,
  $run => {
    if (!$run || $run.traces.length === 0) return null
    const focal = $run.focalCat
    if (!focal) return null
    const cached = indexCache.get($run.id)
    if (cached && cached.focalCat === focal) return cached
    const built = buildFrameIndex($run.traces, focal)
    indexCache.set($run.id, built)
    return built
  },
)

/** The frame at the current focal tick, snapped to the nearest decision
 *  tick ≤ focalTick. Null when no trace is loaded or no tick is
 *  resolvable. */
export const currentFrame: Readable<Frame | null> = derived(
  [frameIndex, focalTick],
  ([$idx, $tick]) => {
    if (!$idx) return null
    const requested = $tick ?? $idx.decisionTicks[0] ?? null
    if (requested === null) return null
    const snapped = nearestDecisionTick($idx, requested) ?? $idx.decisionTicks[0]
    if (snapped === undefined) return null
    return $idx.frames.get(snapped) ?? null
  },
)
