---
id: 179
title: ColonyStoresChronicallyFull → Build DSE consumer + Coordinator BuildStores directive (176 follow-on)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-06
---

## Why

Ticket 176 stage 4 (`32f51f9b`) authored the
`ColonyStoresChronicallyFull` marker on `ColonyState` from
chronicity tracking of `Feature::DepositRejected` events. The
marker is plumbed to `MarkerSnapshot` and the
`build_chronic_full_weight` SimConstants knob is in place at
default 0.0.

What's missing:

- **Build DSE consumer** — Build (`src/ai/dses/build.rs`)
  doesn't yet read the marker. Adding a fourth consideration
  (`MarkerConsideration` against `ColonyStoresChronicallyFull::KEY`)
  with weight `build_chronic_full_weight` would let a positive
  knob lift Build's score when the marker is set.
- **Coordinator BuildStores directive** — When the marker is
  set, the coordinator in `assess_colony_needs`
  (`src/systems/coordination.rs:327-542`) should queue a
  `Build` directive of `StructureType::Stores` if no such
  construction site exists yet, mirroring the existing
  `building_threshold_base` repair logic.

Together these wire the colony's "we need more storage"
demand signal end-to-end: chronicity tracker → marker →
Build DSE lift + Coordinator directive → ConstructionSite spawn
→ cats build a new Stores building.

## Direction

- Build DSE: add a `MarkerConsideration` axis or a new
  ScalarConsideration on a `colony_stores_chronically_full`
  scalar (mirror the `has_construction_site` plumbing from
  scoring.rs:656-661 — bool field on ScoringContext, scalar
  projection in ctx_scalars). Wire weight via
  `build_chronic_full_weight` from SimConstants.
- Coordinator: in `assess_colony_needs`, after the existing
  building-pressure check, query the
  `MarkerSnapshot.has(ColonyStoresChronicallyFull::KEY,
  colony)` (or read the singleton component directly) and
  enqueue a Build(Stores) directive when set + no Stores site
  in progress.
- Tune `build_chronic_full_weight` from default-zero in a
  follow-on balance ticket; this ticket lands the structure.

## Out of scope

- Disposal DSE balance-tuning (178).
- Coordinator priority arbitration if multiple Build directives
  contend (a separate concern).

## Verification

- Six-cat scenario with one full Stores: BuildStores directive
  fires within N ticks; ConstructionSite spawns near colony
  center.
- Post-fix soak shows non-zero Stores-construction completions
  when the colony grows past the single-Stores capacity.
- Survival hard-gates pass.

## Resolution

The Build DSE consumer landed; the Coordinator-side wiring
turned out to be **redundant** with existing infrastructure
(see "Scope narrowing" below).

**Build DSE consumer (shipped):**

- New `Consideration::Marker(MarkerConsideration::new(
  CHRONIC_FULL_INPUT, ColonyStoresChronicallyFull::KEY,
  scoring.build_chronic_full_weight))` as the fourth axis on
  `BuildDse` (`src/ai/dses/build.rs`).
- Composition redistributed from `[0.5, 0.25, 0.25]` (3 axes)
  to `[0.4, 0.25, 0.20, 0.15]` (4 axes, sum=1.0). Diligence
  remains primary; site_distance keeps a 0.25 weight; repair
  and chronic-full are auxiliary pulls (chronic-full smaller
  than repair so undamaged buildings under chronic-full pressure
  don't outrun urgent repair work).
- `default_build_chronic_full_weight()` lifted from `0.0` →
  `0.5` per the wave-closeout plan: the axis was wired-but-
  dormant before 179, so lifting is required for behavioral
  effect. Plausibility value; balance follow-on (181-style
  `just hypothesize` loop) is the next ticket — see
  "Land-day follow-on" below.
- New tests: `build_consideration_count_is_four` and
  `build_chronic_full_axis_reads_colony_marker` on the BuildDse
  composition shape.

## Scope narrowing — Coordinator wiring already covered

The original "Direction" §2 called for a `Build(Stores)`
directive enqueue in `assess_colony_needs` keyed on
`ColonyStoresChronicallyFull`. On read, this is **already
wired** through a different path:

- `assess_build_pressure` (`src/systems/coordination.rs:919-1090`)
  tracks per-coordinator `BuildPressure` axes including
  `pressure.storage`, which increments when `stores_full =
  any Stores building is currently at effective capacity`.
- When `pressure.highest_actionable(threshold)` returns a
  blueprint, the system enqueues
  `Directive { kind: Build, blueprint: Some(StructureType::Stores) }`.

`stores_full` (instantaneous) and `ColonyStoresChronicallyFull`
(chronicity-windowed) overlap substantially in steady state —
both fire when cats can't deposit because Stores is at capacity.
Adding a parallel directive enqueue from the chronic marker
would double-count the demand signal. The DSE-side consumer
captures the chronic signal at L2 (where it can shape per-cat
Build election); the coordinator continues to use the
instantaneous signal for top-down direction. Documenting this
divergence here so the next reader doesn't re-open the question.

## Land-day follow-on

- **Balance follow-on** — open a tuning ticket for
  `build_chronic_full_weight` once post-wave soak data lands.
  The 0.5 plausibility default may over- or under-weight the
  chronic-full pull relative to diligence + site_distance;
  validate via `just hypothesize` four-artifact loop.

## Log

- 2026-05-05: opened by ticket 176's closeout. Marker authored
  in stage 4; this ticket wires the consumers.
- 2026-05-06: landed (wave-closeout step 1 of 3). Build DSE
  consumer shipped via new MarkerConsideration; coordinator-
  side directive enqueue narrowed to "covered by existing
  `assess_build_pressure`" rather than duplicated. Plan at
  `~/.claude/plans/i-just-finished-a-compiled-hanrahan.md`.
