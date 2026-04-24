// Focal-cat trace records — mirrors `TraceRecord` in
// `src/resources/trace_log.rs`. The Rust type uses
// `#[serde(tag = "layer")]`, so every trace line is a JSON object with a
// top-level `layer` discriminator and the `tick` + `cat` fields flattened
// alongside the record body.
//
// A "frame" is all records sharing a single `(tick, cat)` pair —
// operationally the same grouping key `scripts/replay_frame.py` uses.
// One trace file carries a single focal cat (the one passed to
// `--focal-cat`), so within a loaded run the frame index reduces to a
// `Map<tick, Frame>` once the focal cat is fixed.

// ---------------------------------------------------------------------------
// Layer records
// ---------------------------------------------------------------------------

export interface AttenuationBreakdown {
  species_sens: number
  role_mod: number
  injury_deficit: number
  env_mul: number
}

export interface ContributorRow {
  emitter: string
  pos: [number, number]
  distance: number
  contribution: number
}

export interface L1Record {
  layer: 'L1'
  tick: number
  cat: string
  map: string
  faction: string
  channel: string
  pos: [number, number]
  base_sample: number
  attenuation: AttenuationBreakdown
  perceived: number
  top_contributors: ContributorRow[]
}

export interface SpatialRef {
  map: string
  best_target?: string | null
}

export interface ConsiderationContribution {
  name: string
  input: number
  /** Textual description of the response curve (e.g.
   *  `"Logistic { steepness: 8.0, midpoint: 0.75 }"`). The UI renders
   *  the string verbatim; curve math is not replicated client-side. */
  curve: string
  score: number
  weight: number
  spatial?: SpatialRef
}

export interface EligibilitySummary {
  markers_required: string[]
  passed: boolean
}

export interface CompositionSummary {
  /** `"WeightedSum"` / `"CompensatedProduct"` / `"Max"` / `"Unknown"`. */
  mode: string
  raw: number
}

export interface ModifierApplication {
  name: string
  delta?: number
  multiplier?: number
}

export interface IntentionSummary {
  /** `"Goal"` / `"Activity"`. */
  kind: string
  target?: string | null
  goal_state?: string | null
}

export interface LosingAxisSlot {
  axis: string
  score: number
  deficit: number
}

export interface TargetCandidate {
  name: string
  score: number
  contributed: boolean
}

export interface TargetRanking {
  aggregation: string
  candidates: TargetCandidate[]
  winner?: string | null
}

export interface L2Record {
  layer: 'L2'
  tick: number
  cat: string
  dse: string
  eligibility: EligibilitySummary
  considerations: ConsiderationContribution[]
  composition: CompositionSummary
  maslow_pregate: number
  modifiers: ModifierApplication[]
  final_score: number
  intention: IntentionSummary
  top_losing: LosingAxisSlot[]
  targets?: TargetRanking | null
}

export interface SoftmaxSummary {
  temperature: number
  probabilities: number[]
}

export interface MomentumSummary {
  active_intention: string | null
  commitment_strength: number
  margin_threshold: number
  preempted: boolean
}

export interface ApopheniaSummary {
  pairwise_distance_sample: number
  self_autocorrelation_k_days: number[]
}

export interface L3Record {
  layer: 'L3'
  tick: number
  cat: string
  ranked: [string, number][]
  softmax: SoftmaxSummary
  momentum: MomentumSummary
  chosen: string
  intention: IntentionSummary
  goap_plan: string[]
  apophenia?: ApopheniaSummary | null
}

export interface BeliefProxySummary {
  achievement_believed: boolean
  achievable_believed: boolean
  still_goal: boolean
}

export interface PlanStateSummary {
  trips_done: number
  target_trips: number
  replan_count: number
  max_replans: number
}

export interface L3CommitmentRecord {
  layer: 'L3Commitment'
  tick: number
  cat: string
  disposition: string
  strategy: string
  proxies: BeliefProxySummary
  plan_state: PlanStateSummary
  /** `"achieved"` / `"unachievable"` / `"dropped_goal"` / `"retained"`. */
  branch: string
  dropped: boolean
}

export interface L3PlanFailureRecord {
  layer: 'L3PlanFailure'
  tick: number
  cat: string
  /** `"replan_cap"` / `"anxiety_interrupt"`. */
  reason: string
  disposition: string
  detail: unknown
}

export type TraceRecord =
  | L1Record | L2Record | L3Record
  | L3CommitmentRecord | L3PlanFailureRecord

export type TraceLayer = TraceRecord['layer']

// ---------------------------------------------------------------------------
// Frame + frame index
// ---------------------------------------------------------------------------

/** One frame = all trace records at one `(tick, cat)` pair. */
export interface Frame {
  tick: number
  cat: string
  l1: L1Record[]
  l2: L2Record[]
  l3: L3Record | null
  commitment: L3CommitmentRecord[]
  planFailure: L3PlanFailureRecord[]
}

/** DSE-score time series used by the timeline strip. */
export interface DseSeries {
  dse: string
  /** Parallel arrays — `ticks[i]` is the decision tick where `scores[i]`
   *  was recorded. Omits ticks at which the DSE failed eligibility
   *  (those appear as null in `final_score` samples, but gaps in this
   *  series map cleanly to uPlot's `spanGaps: false`). */
  ticks: number[]
  scores: (number | null)[]
}

/** Aggregate of everything the trace page needs from a loaded run. */
export interface FrameIndex {
  focalCat: string
  /** Sorted, unique "decision ticks" — ticks that carry an L3 record. */
  decisionTicks: number[]
  /** Sorted union of *all* ticks with any record, used for L1 fallback. */
  allTicks: number[]
  frames: Map<number, Frame>
  /** Union of DSE names seen in L2 records, in first-seen order. */
  dseNames: string[]
  /** Ticks where the chosen DSE changed vs. the prior decision tick. */
  chosenChangeTicks: number[]
  commitmentTicks: number[]
  planFailureTicks: number[]
  /** Per-DSE time series of L2 `final_score`. One row per `dseNames[i]`. */
  dseSeries: DseSeries[]
}

/** Build a `FrameIndex` from a flat list of trace records.
 *  Filters to the named focal cat (defensive — multi-cat trace files
 *  aren't a current shape, but nothing downstream depends on their
 *  absence). */
export function buildFrameIndex(
  records: TraceRecord[],
  focalCat: string,
): FrameIndex {
  const frames = new Map<number, Frame>()
  const dseNamesSet = new Set<string>()
  const dseNamesOrder: string[] = []
  const allTicksSet = new Set<number>()
  const decisionTicks: number[] = []
  const commitmentTicks: number[] = []
  const planFailureTicks: number[] = []

  function frameFor(tick: number): Frame {
    let f = frames.get(tick)
    if (!f) {
      f = { tick, cat: focalCat, l1: [], l2: [], l3: null, commitment: [], planFailure: [] }
      frames.set(tick, f)
    }
    return f
  }

  for (const r of records) {
    if (r.cat !== focalCat) continue
    allTicksSet.add(r.tick)
    const f = frameFor(r.tick)
    switch (r.layer) {
      case 'L1':
        f.l1.push(r)
        break
      case 'L2':
        f.l2.push(r)
        if (!dseNamesSet.has(r.dse)) {
          dseNamesSet.add(r.dse)
          dseNamesOrder.push(r.dse)
        }
        break
      case 'L3':
        f.l3 = r
        decisionTicks.push(r.tick)
        break
      case 'L3Commitment':
        f.commitment.push(r)
        commitmentTicks.push(r.tick)
        break
      case 'L3PlanFailure':
        f.planFailure.push(r)
        planFailureTicks.push(r.tick)
        break
    }
  }

  decisionTicks.sort((a, b) => a - b)
  commitmentTicks.sort((a, b) => a - b)
  planFailureTicks.sort((a, b) => a - b)
  const allTicks = [...allTicksSet].sort((a, b) => a - b)

  const chosenChangeTicks: number[] = []
  let prevChosen: string | null = null
  for (const tick of decisionTicks) {
    const chosen = frames.get(tick)?.l3?.chosen ?? null
    if (chosen && chosen !== prevChosen) {
      chosenChangeTicks.push(tick)
      prevChosen = chosen
    }
  }

  const dseSeries: DseSeries[] = dseNamesOrder.map(dse => {
    const ticks: number[] = []
    const scores: (number | null)[] = []
    for (const t of decisionTicks) {
      const l2Rows = frames.get(t)?.l2 ?? []
      const row = l2Rows.find(x => x.dse === dse)
      ticks.push(t)
      scores.push(row && row.eligibility.passed ? row.final_score : null)
    }
    return { dse, ticks, scores }
  })

  return {
    focalCat,
    decisionTicks,
    allTicks,
    frames,
    dseNames: dseNamesOrder,
    chosenChangeTicks,
    commitmentTicks,
    planFailureTicks,
    dseSeries,
  }
}

/** Find the nearest decision tick ≤ `tick` (or exact match). Returns
 *  `null` if `tick` precedes every decision tick. Binary search on the
 *  sorted `decisionTicks` array. */
export function nearestDecisionTick(index: FrameIndex, tick: number): number | null {
  const ticks = index.decisionTicks
  if (ticks.length === 0) return null
  let lo = 0, hi = ticks.length - 1, best: number | null = null
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (ticks[mid] <= tick) { best = ticks[mid]; lo = mid + 1 }
    else hi = mid - 1
  }
  return best
}

/** Return the index of `tick` within `decisionTicks`, or -1. */
export function decisionTickIndex(index: FrameIndex, tick: number): number {
  const ticks = index.decisionTicks
  let lo = 0, hi = ticks.length - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (ticks[mid] === tick) return mid
    if (ticks[mid] < tick) lo = mid + 1
    else hi = mid - 1
  }
  return -1
}

/** Step N positions along `decisionTicks`, clamping at the ends. */
export function stepDecisionTick(
  index: FrameIndex, from: number, delta: number,
): number {
  const i = decisionTickIndex(index, from)
  if (i < 0) return from
  const next = Math.max(0, Math.min(index.decisionTicks.length - 1, i + delta))
  return index.decisionTicks[next]
}
