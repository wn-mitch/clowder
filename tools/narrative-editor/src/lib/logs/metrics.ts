// Derives per-run summary metrics from the parsed event stream.
// Used by the RunTable to render one row per loaded run.

import type {
  ActionChosenEvent, CatSnapshotEvent, ColonyScoreEvent, DeathEvent,
  FoodLevelEvent, LogEvent, NeedsBlock, PopulationSnapshotEvent, RunModel,
  SystemActivationEvent, WildlifePopulationEvent,
} from './types'
import { NEED_KEYS } from './types'

export interface RunSummary {
  runId: string
  filenames: string[]
  seed: number | null
  durationSecs: number | null
  commitHashShort: string | null
  commitHash: string | null
  commitDirty: boolean
  commitTime: string | null
  constantsPresent: boolean
  /** Last observed ColonyScore welfare [0,1]. */
  finalWelfare: number | null
  finalAggregate: number | null
  finalLivingCats: number | null
  /** Last observed PopulationSnapshot total (mice + rats + rabbits + fish + birds). */
  finalPreyTotal: number | null
  /** True iff the footer is present AND final living cats > 0 (or unknown). */
  survived: boolean | null
  deathsByCause: Record<string, number>
  deathsTotal: number
  positiveFeaturesActive: number | null
  positiveFeaturesTotal: number | null
  negativeEventsTotal: number | null
  neutralFeaturesActive: number | null
  neutralFeaturesTotal: number | null
  eventsCount: number
  narrativeCount: number
  parseErrorsCount: number
  /** Last observed FoodLevel.current. Null if the run predates the event. */
  finalFoodStores: number | null
  /** Last observed WildlifePopulation. Null on older runs. */
  finalWildlife: { foxes: number; hawks: number; snakes: number; shadow_foxes: number } | null
  /** Last observed per-species prey counts (from PopulationSnapshot). */
  finalPreyBreakdown: { mice: number; rats: number; rabbits: number; fish: number; birds: number } | null
}

export function summarizeRun(run: RunModel): RunSummary {
  const { events, footer, header, files, narrative, parseErrors } = run

  const lastColonyScore = findLast<ColonyScoreEvent>(events, e => e.type === 'ColonyScore')
  const lastPopSnapshot = findLast<PopulationSnapshotEvent>(events, e => e.type === 'PopulationSnapshot')
  const lastActivation = findLast<SystemActivationEvent>(events, e => e.type === 'SystemActivation')
  const lastFood = findLast<FoodLevelEvent>(events, e => e.type === 'FoodLevel')
  const lastWildlife = findLast<WildlifePopulationEvent>(events, e => e.type === 'WildlifePopulation')

  const deathsByCause = gatherDeaths(events, footer?.deaths_by_cause)
  const deathsTotal = Object.values(deathsByCause).reduce((a, b) => a + b, 0)

  const positiveFeaturesActive = footer?.positive_features_active
    ?? lastColonyScore?.positive_features_active
    ?? countActive(lastActivation?.positive)
  const positiveFeaturesTotal = footer?.positive_features_total
    ?? lastColonyScore?.positive_features_total
    ?? countTotal(lastActivation?.positive)
  const neutralFeaturesActive = footer?.neutral_features_active
    ?? lastColonyScore?.neutral_features_active
    ?? countActive(lastActivation?.neutral)
  const neutralFeaturesTotal = footer?.neutral_features_total
    ?? lastColonyScore?.neutral_features_total
    ?? countTotal(lastActivation?.neutral)
  const negativeEventsTotal = footer?.negative_events_total
    ?? lastColonyScore?.negative_events_total
    ?? sumValues(lastActivation?.negative)

  const finalLivingCats = lastColonyScore?.living_cats ?? null
  const survived = finalLivingCats === null ? null : finalLivingCats > 0

  const finalPreyTotal = lastPopSnapshot
    ? lastPopSnapshot.mice + lastPopSnapshot.rats + lastPopSnapshot.rabbits
      + lastPopSnapshot.fish + lastPopSnapshot.birds
    : null

  return {
    runId: run.id,
    filenames: files.map(f => f.name),
    seed: header?.seed ?? null,
    durationSecs: header?.duration_secs ?? null,
    commitHashShort: header?.commit_hash_short ?? null,
    commitHash: header?.commit_hash ?? null,
    commitDirty: header?.commit_dirty ?? false,
    commitTime: header?.commit_time ?? null,
    constantsPresent: header?.constants !== undefined,
    finalWelfare: lastColonyScore?.welfare ?? null,
    finalAggregate: lastColonyScore?.aggregate ?? null,
    finalLivingCats,
    finalPreyTotal,
    survived,
    deathsByCause,
    deathsTotal,
    positiveFeaturesActive: positiveFeaturesActive ?? null,
    positiveFeaturesTotal: positiveFeaturesTotal ?? null,
    negativeEventsTotal: negativeEventsTotal ?? null,
    neutralFeaturesActive: neutralFeaturesActive ?? null,
    neutralFeaturesTotal: neutralFeaturesTotal ?? null,
    eventsCount: events.length,
    narrativeCount: narrative.length,
    parseErrorsCount: parseErrors.length,
    finalFoodStores: lastFood?.current ?? null,
    finalWildlife: lastWildlife
      ? {
          foxes: lastWildlife.foxes,
          hawks: lastWildlife.hawks,
          snakes: lastWildlife.snakes,
          shadow_foxes: lastWildlife.shadow_foxes,
        }
      : null,
    finalPreyBreakdown: lastPopSnapshot
      ? {
          mice: lastPopSnapshot.mice,
          rats: lastPopSnapshot.rats,
          rabbits: lastPopSnapshot.rabbits,
          fish: lastPopSnapshot.fish,
          birds: lastPopSnapshot.birds,
        }
      : null,
  }
}

// ---------------------------------------------------------------------------
// Series extractors for charts
// ---------------------------------------------------------------------------

/** Colony-aggregated Maslow needs per tick. Averages every emitted
 *  CatSnapshot for a given tick across all cats present at that tick. */
export function colonyMaslowSeries(
  events: LogEvent[],
): { tick: number; needs: NeedsBlock }[] {
  const byTick = new Map<number, { sums: NeedsBlock; count: number }>()
  for (const e of events) {
    if (e.type !== 'CatSnapshot') continue
    const snap = e as CatSnapshotEvent
    if (snap.tick === undefined || !snap.needs) continue
    let bucket = byTick.get(snap.tick)
    if (!bucket) {
      bucket = { sums: emptyNeeds(), count: 0 }
      byTick.set(snap.tick, bucket)
    }
    for (const k of NEED_KEYS) {
      const v = snap.needs[k]
      if (typeof v === 'number') bucket.sums[k] += v
    }
    bucket.count += 1
  }
  const out: { tick: number; needs: NeedsBlock }[] = []
  const ticks = Array.from(byTick.keys()).sort((a, b) => a - b)
  for (const t of ticks) {
    const { sums, count } = byTick.get(t)!
    if (count === 0) continue
    const averaged = { ...sums } as NeedsBlock
    for (const k of NEED_KEYS) averaged[k] = sums[k] / count
    out.push({ tick: t, needs: averaged })
  }
  return out
}

/** Per-cat Maslow needs timeline. Filters CatSnapshot to one cat. */
export function perCatNeedsSeries(
  events: LogEvent[],
  catName: string,
): { tick: number; needs: NeedsBlock }[] {
  const out: { tick: number; needs: NeedsBlock }[] = []
  for (const e of events) {
    if (e.type !== 'CatSnapshot') continue
    const snap = e as CatSnapshotEvent
    if (snap.cat !== catName || snap.tick === undefined || !snap.needs) continue
    out.push({ tick: snap.tick, needs: snap.needs })
  }
  return out
}

/** Per-cat ActionChosen frequencies. */
export function perCatActionCounts(
  events: LogEvent[],
  catName: string,
): Record<string, number> {
  const acc: Record<string, number> = {}
  for (const e of events) {
    if (e.type !== 'ActionChosen') continue
    const ae = e as ActionChosenEvent
    if (ae.cat !== catName) continue
    // The sim serializes Action as either a plain string or a tagged enum
    // like `{ "Eat": {...} }`. Normalize both to a printable key.
    const key = typeof ae.action === 'string'
      ? ae.action
      : typeof ae.action === 'object' && ae.action !== null
        ? Object.keys(ae.action as Record<string, unknown>)[0] ?? 'Unknown'
        : 'Unknown'
    acc[key] = (acc[key] ?? 0) + 1
  }
  return acc
}

/** Per-cat mood valence over time. */
export function perCatMoodSeries(
  events: LogEvent[],
  catName: string,
): { tick: number; valence: number }[] {
  const out: { tick: number; valence: number }[] = []
  for (const e of events) {
    if (e.type !== 'CatSnapshot') continue
    const snap = e as CatSnapshotEvent
    if (snap.cat !== catName || snap.tick === undefined) continue
    if (typeof snap.mood_valence !== 'number') continue
    out.push({ tick: snap.tick, valence: snap.mood_valence })
  }
  return out
}

/** WildlifePopulation over time. */
export function wildlifeSeries(events: LogEvent[]): {
  tick: number; foxes: number; hawks: number; snakes: number; shadow_foxes: number
}[] {
  const out: ReturnType<typeof wildlifeSeries> = []
  for (const e of events) {
    if (e.type !== 'WildlifePopulation') continue
    const w = e as WildlifePopulationEvent
    if (w.tick === undefined) continue
    out.push({
      tick: w.tick, foxes: w.foxes, hawks: w.hawks,
      snakes: w.snakes, shadow_foxes: w.shadow_foxes,
    })
  }
  return out
}

/** PopulationSnapshot broken out by species. */
export function preyBySpeciesSeries(events: LogEvent[]): {
  tick: number; mice: number; rats: number; rabbits: number; fish: number; birds: number
}[] {
  const out: ReturnType<typeof preyBySpeciesSeries> = []
  for (const e of events) {
    if (e.type !== 'PopulationSnapshot') continue
    const p = e as PopulationSnapshotEvent
    if (p.tick === undefined) continue
    out.push({
      tick: p.tick, mice: p.mice, rats: p.rats,
      rabbits: p.rabbits, fish: p.fish, birds: p.birds,
    })
  }
  return out
}

/** FoodLevel.current over time. */
export function foodStoresSeries(
  events: LogEvent[],
): { tick: number; current: number; fraction: number }[] {
  const out: ReturnType<typeof foodStoresSeries> = []
  for (const e of events) {
    if (e.type !== 'FoodLevel') continue
    const f = e as FoodLevelEvent
    if (f.tick === undefined) continue
    out.push({ tick: f.tick, current: f.current, fraction: f.fraction })
  }
  return out
}

/** Distinct cat names observed in CatSnapshot events, alphabetically sorted. */
export function availableCatNames(events: LogEvent[]): string[] {
  const set = new Set<string>()
  for (const e of events) {
    if (e.type !== 'CatSnapshot') continue
    const snap = e as CatSnapshotEvent
    if (typeof snap.cat === 'string') set.add(snap.cat)
  }
  return Array.from(set).sort()
}

function emptyNeeds(): NeedsBlock {
  return {
    hunger: 0, energy: 0, temperature: 0, safety: 0, social: 0, social_warmth: 0,
    acceptance: 0, mating: 0, respect: 0, mastery: 0, purpose: 0,
  }
}

function findLast<T extends LogEvent>(events: LogEvent[], pred: (e: LogEvent) => boolean): T | undefined {
  for (let i = events.length - 1; i >= 0; i--) {
    if (pred(events[i])) return events[i] as T
  }
  return undefined
}

function countActive(counts: Record<string, number> | undefined): number | undefined {
  if (!counts) return undefined
  let n = 0
  for (const v of Object.values(counts)) if (v > 0) n += 1
  return n
}

function countTotal(counts: Record<string, number> | undefined): number | undefined {
  if (!counts) return undefined
  return Object.keys(counts).length
}

function sumValues(counts: Record<string, number> | undefined): number | undefined {
  if (!counts) return undefined
  let n = 0
  for (const v of Object.values(counts)) n += v
  return n
}

function gatherDeaths(
  events: LogEvent[],
  footerDeaths: Record<string, number> | undefined,
): Record<string, number> {
  if (footerDeaths && Object.keys(footerDeaths).length > 0) return { ...footerDeaths }
  // Fall back to counting Death events directly.
  const acc: Record<string, number> = {}
  for (const e of events) {
    if (e.type !== 'Death') continue
    const cause = (e as DeathEvent).cause ?? 'Unknown'
    acc[cause] = (acc[cause] ?? 0) + 1
  }
  return acc
}
