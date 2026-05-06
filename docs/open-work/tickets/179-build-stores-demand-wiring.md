---
id: 179
title: ColonyStoresChronicallyFull → Build DSE consumer + Coordinator BuildStores directive (176 follow-on)
status: ready
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
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

## Log

- 2026-05-05: opened by ticket 176's closeout. Marker authored
  in stage 4; this ticket wires the consumers.
