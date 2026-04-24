// Types mirroring the Rust simulation log schema.
// Source of truth: src/resources/event_log.rs (EventKind) and
// src/main.rs (header / footer / narrative line shape).
//
// The sim emits serde-tagged enums, so each event is an object with a
// "type" discriminator. Only fields the dashboard renders are modeled
// strictly; the rest are kept open via index signatures.

export interface HeaderCommitInfo {
  commit_hash: string
  commit_hash_short: string
  commit_dirty: boolean
  commit_time: string
}

export interface SimConfigBlock {
  seed: number
  ticks_per_day_phase: number
  ticks_per_season: number
}

export interface LogHeader extends HeaderCommitInfo {
  _header: true
  seed: number
  duration_secs: number
  /** Present only on events.jsonl headers — carries tick→day/season scaling. */
  sim_config?: SimConfigBlock
  /** Present only on events.jsonl headers; full SimConstants dump. */
  constants?: Record<string, unknown>
  /** Present on events.jsonl headers from sims with the map-overlay patch.
   *  Absent on older logs — the overlay must fall back to inferred bounds. */
  map_width?: number
  map_height?: number
  /** Present only on trace-<name>.jsonl headers. Names the cat the trace
   *  was captured for. */
  focal_cat?: string
}

export interface LogFooter {
  _footer: true
  wards_placed_total?: number
  wards_despawned_total?: number
  ward_count_final?: number
  ward_avg_strength_final?: number
  shadow_foxes_avoided_ward_total?: number
  ward_siege_started_total?: number
  shadow_fox_spawn_total?: number
  anxiety_interrupt_total?: number
  positive_features_active?: number
  positive_features_total?: number
  negative_events_total?: number
  neutral_features_active?: number
  neutral_features_total?: number
  deaths_by_cause?: Record<string, number>
  plan_failures_by_reason?: Record<string, number>
  interrupts_by_reason?: Record<string, number>
  [key: string]: unknown
}

export interface BaseEvent {
  type: string
  tick?: number
  [key: string]: unknown
}

export interface ColonyScoreEvent extends BaseEvent {
  type: 'ColonyScore'
  welfare?: number
  aggregate?: number
  positive_activation_score?: number
  positive_features_active?: number
  positive_features_total?: number
  negative_events_total?: number
  neutral_features_active?: number
  neutral_features_total?: number
  living_cats?: number
}

export interface FoodLevelEvent extends BaseEvent {
  type: 'FoodLevel'
  current: number
  capacity: number
  fraction: number
}

export interface PopulationSnapshotEvent extends BaseEvent {
  type: 'PopulationSnapshot'
  mice: number
  rats: number
  rabbits: number
  fish: number
  birds: number
}

/** Maslow needs emitted inside CatSnapshot.needs. Matches the Rust
 *  `Needs` struct in src/components/physical.rs. Values are in [0, 1]
 *  where 1 = satisfied and 0 = critical. `social_warmth` is the planned
 *  warmth-split phase 3 axis (see docs/open-work.md #12); emitted as 0
 *  until phase 3 lands. */
export interface NeedsBlock {
  hunger: number
  energy: number
  temperature: number
  safety: number
  social: number
  social_warmth: number
  acceptance: number
  mating: number
  respect: number
  mastery: number
  purpose: number
}

export const NEED_KEYS: readonly (keyof NeedsBlock)[] = [
  'hunger', 'energy', 'temperature', 'safety', 'social', 'social_warmth',
  'acceptance', 'mating', 'respect', 'mastery', 'purpose',
] as const

export interface CatSnapshotEvent extends BaseEvent {
  type: 'CatSnapshot'
  cat: string
  position: [number, number]
  needs: NeedsBlock
  mood_valence: number
  current_action: string
  health: number
  corruption: number
  life_stage?: string
  season?: string
}

export interface WildlifePopulationEvent extends BaseEvent {
  type: 'WildlifePopulation'
  foxes: number
  hawks: number
  snakes: number
  shadow_foxes: number
}

export interface WildlifePosRow {
  species: string
  x: number
  y: number
}

export interface WildlifePositionsEvent extends BaseEvent {
  type: 'WildlifePositions'
  positions: WildlifePosRow[]
}

export interface PreyPosRow {
  species: string
  x: number
  y: number
}

export interface PreyPositionsEvent extends BaseEvent {
  type: 'PreyPositions'
  positions: PreyPosRow[]
}

export interface PreyDenRow {
  species: string
  x: number
  y: number
  spawns_remaining: number
  capacity: number
  predation_pressure: number
}

export interface FoxDenRow {
  x: number
  y: number
  cubs_present: number
  territory_radius: number
  scent_strength: number
}

export interface DenSnapshotEvent extends BaseEvent {
  type: 'DenSnapshot'
  prey_dens: PreyDenRow[]
  fox_dens: FoxDenRow[]
}

export interface HuntingBeliefSnapshotEvent extends BaseEvent {
  type: 'HuntingBeliefSnapshot'
  cat: string | null
  width: number
  height: number
  values: number[]
}

/** Events carrying an explicit (x, y) location — used by the map overlay. */
export interface LocatedEvent extends BaseEvent {
  location: [number, number]
}

export interface ActionChosenEvent extends BaseEvent {
  type: 'ActionChosen'
  cat: string
  action: string
}

export interface DeathEvent extends BaseEvent {
  type: 'Death'
  cat: string
  cause: string
  injury_source?: string | null
  location?: unknown
}

export interface SystemActivationEvent extends BaseEvent {
  type: 'SystemActivation'
  positive: Record<string, number>
  negative: Record<string, number>
  neutral: Record<string, number>
}

export type LogEvent =
  | ColonyScoreEvent
  | FoodLevelEvent
  | PopulationSnapshotEvent
  | CatSnapshotEvent
  | ActionChosenEvent
  | WildlifePopulationEvent
  | WildlifePositionsEvent
  | PreyPositionsEvent
  | DenSnapshotEvent
  | HuntingBeliefSnapshotEvent
  | DeathEvent
  | SystemActivationEvent
  | BaseEvent

export type NarrativeTier =
  | 'Micro' | 'Action' | 'Significant' | 'Danger' | 'Nature' | 'Legend' | string

export interface NarrativeEntry {
  tick: number
  day: number
  phase: string
  tier: NarrativeTier
  text: string
}

export type LogFileKind = 'events' | 'narrative' | 'trace' | 'unknown'

export interface ParseError {
  line: number
  message: string
}

export interface LoadedFile {
  name: string
  size: number
  kind: LogFileKind
  header: LogHeader | null
  events: LogEvent[]
  narrative: NarrativeEntry[]
  /** Focal-cat trace records. Empty for events/narrative files. */
  traces: import('./trace').TraceRecord[]
  footer: LogFooter | null
  parseErrors: ParseError[]
  loadedAt: Date
}

/** A RunModel groups up to one events.jsonl and one narrative.jsonl from the
 *  same simulation run. Files are paired by (seed, commit_hash_short, duration_secs)
 *  when a second matching file is loaded. */
export interface RunModel {
  id: string
  files: LoadedFile[]
  /** Canonical header for the run — prefers the events header if present
   *  (because it carries constants). */
  header: LogHeader | null
  footer: LogFooter | null
  events: LogEvent[]
  narrative: NarrativeEntry[]
  /** Flat list of trace records — merged across any trace files in the
   *  run. Empty when no trace sidecar was loaded. */
  traces: import('./trace').TraceRecord[]
  /** Name of the focal cat the trace was captured for, read from the
   *  trace header's `focal_cat` field. Null when no trace is loaded. */
  focalCat: string | null
  parseErrors: ParseError[]
}
