---
id: 171
title: Promote HasGarden to ECS-level singleton writer (parity with 168/169)
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

`HasGarden` is the last colony-scoped TargetExistence marker that
lacks an ECS-level writer. It has the `pub const KEY` constant
(`src/components/markers.rs:400`) and is consumed by the production
Farm DSE eligibility filter (`src/ai/dses/farm.rs:119`
`.require(markers::HasGarden::KEY)`). Its current "writer" is two
imperative-scan bridges that call `MarkerSnapshot::set_colony(KEY, …)`
directly:

- `src/systems/goap.rs:1003` (inside `populate_world_state`)
- `src/systems/disposition.rs:537` (inside the disposition path)

160's substrate-stub lint accepts the `set_colony(KEY, …)` callsite
as a "writer" pattern, which is why `HasGarden` doesn't show up in
`scripts/substrate_stubs.allowlist`. But this is the **only**
colony-scoped TargetExistence marker still authored that way after
168 (the six inventory/ward markers) and 169 (`HasConstructionSite`
/ `HasDamagedBuilding`). The asymmetry costs:

1. **Substrate-vs-search-state confusion.** §4.7 of the substrate-
   refactor spec is the load-bearing read on this boundary —
   per-tick predicates like "≥1 garden exists" belong on the
   singleton entity as ECS components, not in two parallel
   imperative scans that each have to stay in sync with the marker
   author.
2. **Two authoring sites that can drift.** `goap.rs:1003` and
   `disposition.rs:537` both call `set_colony(HasGarden::KEY, …)`
   from values computed by independent `scan_colony_buildings`
   invocations. After 169 migrated `has_construction_site` /
   `has_damaged_building` to read from the singleton query in
   `goap.rs`, `disposition.rs` is the only remaining holdout that
   still re-derives those predicates locally — but only because
   `HasGarden` keeps the imperative scan needed there. Promoting
   `HasGarden` lets `disposition.rs` drop its own
   `scan_colony_buildings` call entirely.
3. **Test coverage gap.** Unlike the other six +2 markers,
   `HasGarden` has no `update_colony_building_markers` tick-system
   test asserting insert/remove on the singleton — the existing
   `garden_detected` test (`buildings.rs:739`) only exercises the
   helper.

`buildings.rs:438-446` records this as a known follow-on in the
rustdoc note above `update_colony_building_markers`.

## Scope

1. **Author `HasGarden` on the `ColonyState` singleton** in
   `update_colony_building_markers` (`src/systems/buildings.rs`).
   Insert iff `bldg_state.has_garden`; remove otherwise. Mirrors
   the `HasConstructionSite` / `HasDamagedBuilding` blocks shipped
   in 169.
2. **Extend `colony_state_query`** at `src/systems/goap.rs:164-180`
   with `Has<markers::HasGarden>` (currently 8-tuple, becomes
   9-tuple).
3. **Read `has_garden` from the singleton query** in
   `populate_world_state` (`goap.rs:~939-947` destructure +
   `~1002-1007` snapshot population). Drop the
   `scan_colony_buildings`-derived `has_garden` local; the
   `bldg_state` binding may then be removable entirely if `has_garden`
   was its last consumer in `goap.rs`. Verify before deleting — keep
   the call if `bldg_state` is still referenced.
4. **Migrate `disposition.rs:537`** the same way. Its
   `scan_colony_buildings` call is currently the source for
   `has_construction_site` / `has_damaged_building` / `has_garden`
   that flow into `ScoringContext`; if `disposition.rs` has access
   to a `ColonyState` singleton query (or can be given one via
   SystemParam), all three can come from the singleton and the
   scan call drops. If the SystemParam expansion is non-trivial,
   the minimal change is to leave `has_construction_site` /
   `has_damaged_building` migration to a follow-on (per 169's
   "out of scope" entry) and only migrate `has_garden` here.
5. **Update the rustdoc** above `update_colony_building_markers`
   (`buildings.rs:438-446`) to remove the "follow-on" note.
6. **Add tick-system test** in `buildings.rs::tests` modeled on the
   five new tests from 169 — assert `HasGarden` insert when a
   `Garden` `Structure` exists and removal otherwise.

## Out of scope

- Migrating `disposition.rs`'s `has_construction_site` /
  `has_damaged_building` reads (169's deferred follow-on; can ride
  along if SystemParam expansion is cheap, otherwise its own
  ticket).
- Removing `ScoringContext.has_garden` field. The Farm DSE consumes
  it via `MarkerSnapshot::has(HasGarden::KEY, entity)` already
  (`farm.rs:119`); the field consumers (`scoring.rs:2230, 3242,
  3965` test fixtures) keep the field as a convenience.
- Generalizing the colony-state singleton query into a single
  `ColonyMarkerSnapshot` resource (would be a wider refactor —
  worth its own design pass).

## Verification

1. `just check` — substrate-stub lint passes (HasGarden gains an
   `.insert()` / `.remove::<>()` writer alongside the existing
   `set_colony` reader).
2. `cargo test --lib --release -p clowder` — the new tick-system
   test asserts singleton insert/remove on `HasGarden`; existing
   `garden_detected` helper test still passes; existing Farm-DSE
   eligibility tests unaffected (they populate `MarkerSnapshot`
   directly).
3. `just soak 42` then `just verdict logs/tuned-42` — Farm DSE
   activity (`CropTended`, `CropHarvested` event counts in the
   footer's `SystemActivation` block) unchanged within seed-42
   noise. Behavior is invariant by construction (same predicate,
   same threshold, same FixedUpdate ordering — `HasGarden`'s
   marker write happens in the same `.chain()` slot as the other
   colony markers).

## Log

- 2026-05-05: opened as follow-on to 169 closeout. 169 left
  `HasGarden` ECS-level wiring out of scope because the
  substrate-stub lint accepted the `set_colony(KEY, …)` snapshot
  bridge as a writer; this ticket completes the symmetry with the
  other eight colony-scoped markers (six from 168, two from 169).
