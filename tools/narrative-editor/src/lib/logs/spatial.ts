// Preprocessing for the MapOverlay canvas.
// Turns an event stream into tick-keyed indices so slider scrubs are cheap.

import type {
  CatSnapshotEvent, DenSnapshotEvent, HuntingBeliefSnapshotEvent, LogEvent,
  PreyPositionsEvent, RunModel, WildlifePositionsEvent,
} from './types'

export interface CatPos { cat: string; x: number; y: number }
export interface PreyPos { species: string; x: number; y: number }
export interface WildlifePos { species: string; x: number; y: number }

export interface ActiveWard {
  kind: string
  x: number
  y: number
  sieged: boolean
  placedTick: number
}

export interface DotEvent {
  type: string
  tick: number
  x: number
  y: number
  label: string
}

export interface SpatialIndex {
  /** Map of tick → cat positions present at that tick. */
  catsByTick: Map<number, CatPos[]>
  /** Map of tick → prey positions. */
  preyByTick: Map<number, PreyPos[]>
  /** Map of tick → wildlife positions. */
  wildlifeByTick: Map<number, WildlifePos[]>
  /** Most-recent-before-tick den snapshot (DenSnapshot events are sparse). */
  densByTick: Map<number, DenSnapshotEvent>
  /** Most-recent-before-tick colony hunting belief snapshot. */
  beliefByTick: Map<number, HuntingBeliefSnapshotEvent>
  /** Ward placements and despawns as time-ordered events. */
  wardPlacedEvents: Array<ActiveWard>
  wardDespawnedEvents: Array<{ kind: string; x: number; y: number; sieged: boolean; despawnTick: number }>
  /** Time-ordered dot events (ambush, kill, death, shadow-fox spawn, etc). */
  dots: DotEvent[]
  /** Ascending list of all snapshot ticks across every index. Slider steps. */
  sortedTicks: number[]
  /** Max tick observed — slider upper bound. */
  maxTick: number
  /** Map dimensions if the header provided them. */
  mapWidth: number | null
  mapHeight: number | null
}

function locOf(e: LogEvent): [number, number] | null {
  const loc = (e as { location?: unknown }).location
  if (!Array.isArray(loc) || loc.length < 2) return null
  const [x, y] = loc
  if (typeof x !== 'number' || typeof y !== 'number') return null
  return [x, y]
}

export function buildSpatialIndex(run: RunModel): SpatialIndex {
  const catsByTick = new Map<number, CatPos[]>()
  const preyByTick = new Map<number, PreyPos[]>()
  const wildlifeByTick = new Map<number, WildlifePos[]>()
  const densByTick = new Map<number, DenSnapshotEvent>()
  const beliefByTick = new Map<number, HuntingBeliefSnapshotEvent>()
  const wardPlacedEvents: ActiveWard[] = []
  const wardDespawnedEvents: { kind: string; x: number; y: number; sieged: boolean; despawnTick: number }[] = []
  const dots: DotEvent[] = []
  let maxTick = 0

  for (const e of run.events) {
    const tick = typeof e.tick === 'number' ? e.tick : null
    if (tick === null) continue
    if (tick > maxTick) maxTick = tick

    switch (e.type) {
      case 'CatSnapshot': {
        const s = e as CatSnapshotEvent
        const [x, y] = Array.isArray(s.position) ? s.position : [null, null]
        if (typeof x !== 'number' || typeof y !== 'number') break
        let bucket = catsByTick.get(tick)
        if (!bucket) { bucket = []; catsByTick.set(tick, bucket) }
        bucket.push({ cat: s.cat, x, y })
        break
      }
      case 'PreyPositions': {
        const p = e as PreyPositionsEvent
        preyByTick.set(tick, p.positions.map(r => ({ species: r.species, x: r.x, y: r.y })))
        break
      }
      case 'WildlifePositions': {
        const w = e as WildlifePositionsEvent
        wildlifeByTick.set(tick, w.positions.map(r => ({ species: r.species, x: r.x, y: r.y })))
        break
      }
      case 'DenSnapshot': {
        densByTick.set(tick, e as DenSnapshotEvent)
        break
      }
      case 'HuntingBeliefSnapshot': {
        beliefByTick.set(tick, e as HuntingBeliefSnapshotEvent)
        break
      }
      case 'WardPlaced': {
        const loc = locOf(e); if (!loc) break
        const kind = (e as { ward_kind?: string }).ward_kind ?? 'ward'
        wardPlacedEvents.push({ kind, x: loc[0], y: loc[1], sieged: false, placedTick: tick })
        break
      }
      case 'WardDespawned': {
        const loc = locOf(e); if (!loc) break
        const kind = (e as { ward_kind?: string }).ward_kind ?? 'ward'
        const sieged = Boolean((e as { sieged?: boolean }).sieged)
        wardDespawnedEvents.push({ kind, x: loc[0], y: loc[1], sieged, despawnTick: tick })
        break
      }
      case 'Ambush': {
        const loc = locOf(e); if (!loc) break
        const pred = (e as { predator_species?: string }).predator_species ?? '?'
        const cat = (e as { cat?: string }).cat ?? '?'
        dots.push({ type: 'Ambush', tick, x: loc[0], y: loc[1], label: `${pred} ambushed ${cat}` })
        break
      }
      case 'PreyKilled': {
        const loc = locOf(e); if (!loc) break
        const cat = (e as { cat?: string }).cat ?? '?'
        const species = (e as { species?: string }).species ?? '?'
        dots.push({ type: 'PreyKilled', tick, x: loc[0], y: loc[1], label: `${cat} killed ${species}` })
        break
      }
      case 'Death': {
        const loc = locOf(e); if (!loc) break
        const cat = (e as { cat?: string }).cat ?? '?'
        const cause = (e as { cause?: string }).cause ?? '?'
        dots.push({ type: 'Death', tick, x: loc[0], y: loc[1], label: `${cat} died (${cause})` })
        break
      }
      case 'ShadowFoxSpawn': {
        const loc = locOf(e); if (!loc) break
        dots.push({ type: 'ShadowFoxSpawn', tick, x: loc[0], y: loc[1], label: 'shadow-fox spawned' })
        break
      }
      case 'ShadowFoxBanished': {
        const loc = locOf(e); if (!loc) break
        dots.push({ type: 'ShadowFoxBanished', tick, x: loc[0], y: loc[1], label: 'shadow-fox banished' })
        break
      }
      case 'KittenBorn': {
        const loc = locOf(e); if (!loc) break
        const kitten = (e as { kitten?: string }).kitten ?? '?'
        dots.push({ type: 'KittenBorn', tick, x: loc[0], y: loc[1], label: `kitten ${kitten} born` })
        break
      }
      case 'BuildingConstructed': {
        const loc = locOf(e); if (!loc) break
        const kind = (e as { kind?: string }).kind ?? '?'
        dots.push({ type: 'BuildingConstructed', tick, x: loc[0], y: loc[1], label: `${kind} completed` })
        break
      }
    }
  }

  const tickSet = new Set<number>()
  for (const t of catsByTick.keys()) tickSet.add(t)
  for (const t of preyByTick.keys()) tickSet.add(t)
  for (const t of wildlifeByTick.keys()) tickSet.add(t)
  for (const t of densByTick.keys()) tickSet.add(t)
  for (const t of beliefByTick.keys()) tickSet.add(t)
  for (const d of dots) tickSet.add(d.tick)
  for (const w of wardPlacedEvents) tickSet.add(w.placedTick)
  const sortedTicks = Array.from(tickSet).sort((a, b) => a - b)

  const header = run.header
  return {
    catsByTick, preyByTick, wildlifeByTick, densByTick, beliefByTick,
    wardPlacedEvents, wardDespawnedEvents, dots, sortedTicks, maxTick,
    mapWidth: typeof header?.map_width === 'number' ? header.map_width : null,
    mapHeight: typeof header?.map_height === 'number' ? header.map_height : null,
  }
}

/** Finds the last snapshot at or before `tick` in a tick-keyed map.
 *  Linear scan is fine because snapshot maps are small (≤ a few hundred entries). */
export function lastBeforeOrAt<T>(m: Map<number, T>, tick: number): T | null {
  let bestTick = -1
  let best: T | null = null
  for (const [t, v] of m) {
    if (t <= tick && t > bestTick) { bestTick = t; best = v }
  }
  return best
}

/** Returns the wards active at `tick` by replaying placements/despawns. */
export function activeWardsAt(index: SpatialIndex, tick: number): ActiveWard[] {
  const active: ActiveWard[] = []
  for (const w of index.wardPlacedEvents) {
    if (w.placedTick > tick) continue
    // A WardDespawned event matches the nearest WardPlaced at the same
    // location (no ward carries a unique id). Mark sieged if the despawn
    // event flags it as such.
    const matched = index.wardDespawnedEvents.find(
      d => d.despawnTick <= tick
        && d.despawnTick >= w.placedTick
        && d.x === w.x
        && d.y === w.y
        && d.kind === w.kind,
    )
    if (matched) {
      // Despawned; but render its sieged marker briefly? v1 just hides it.
      continue
    }
    active.push(w)
  }
  return active
}
