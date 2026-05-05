---
id: 169
title: Author HasConstructionSite + HasDamagedBuilding markers (buildings.rs)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 6b9e8351
landed-on: 2026-05-05
---

## Why

Two markers at `src/components/markers.rs:381` and `:384` are spec'd
in `docs/systems/ai-substrate-refactor.md` §4.3 lines 1976-1977 with
the author function `tick:buildings.rs::update_colony_building_markers`,
queried via `Q<With<ColonyState>, With<HasConstructionSite>>` /
`Q<With<ColonyState>, With<HasDamagedBuilding>>`. Neither has ever been
authored.

160's substrate-stub lint flags both markers as `fully-orphan`. This
ticket wires them.

Note: `src/ai/considerations.rs:417` carries
`MarkerConsideration::new("has_site", "HasConstructionSite", 0.3)`,
but that's a unit test fixture — `MarkerConsideration::new` itself has
no production callsites today. So there's no live "dead consideration"
to repair; just author the marker.

## Scope

1. **Implement `update_colony_building_markers`** in `src/systems/buildings.rs`.
   - `HasConstructionSite`: insert on the `ColonyState` singleton iff
     ≥1 reachable `ConstructionSite` exists (see substrate spec line 1976
     for the precise predicate; also check `ScoringContext.has_construction_site`
     at `:47` for the current ad-hoc shape).
   - `HasDamagedBuilding`: insert on the `ColonyState` singleton iff
     ≥1 `Structure` has condition < 0.4 (substrate spec line 1977; check
     `ScoringContext.has_damaged_building:49` for the current ad-hoc check).
2. **Add `pub const KEY` to both markers** if they don't have one
   (currently they don't — they're the only two `TargetExistence`
   markers without `KEY`, but the substrate-refactor spec uses
   `Q<With<ColonyState>, With<HasConstructionSite>>` query shape, so
   `KEY` may not be required for production reads). Decide based on
   how the readers in `src/ai/scoring.rs` / `src/systems/goap.rs`
   consume them — match the surrounding pattern.
3. **Drop both allowlist entries** from
   `scripts/substrate_stubs.allowlist`.

## Blocked-by

168 (`ColonyState` singleton wiring) — these markers attach to that
singleton entity.

## Out of scope

- Other colony-scoped markers (`HasFunctionalKitchen`, `HasRawFoodInStores`,
  …). They have working ad-hoc authoring today; migrating them to
  `ColonyState` singleton is part of 168, not this ticket.

## Verification

1. After landing, marker insert/remove fires on `ColonyState` singleton
   tick-by-tick as buildings change condition.
2. Existing `ScoringContext.has_construction_site` and
   `.has_damaged_building` paths return values consistent with the marker
   query (or are removed if redundant — substrate-over-override).
3. `MarkerConsideration::new("has_site", ...)` actually contributes to
   action scores when a construction site exists. Add a unit/integration
   test asserting the consideration fires.
4. `just check` passes after dropping both allowlist entries.
5. `just soak` + `just verdict` — building behavior may shift; treat as
   balance change.

## Log

- 2026-05-05: opened in same commit as 160. Blocked on 168.
- 2026-05-05: **Landed `6b9e8351`** after 168 cleared the blocker.
  Both markers now have `pub const KEY` impls; `update_colony_building_markers`
  inserts/removes them on the `ColonyState` singleton from
  `bldg_state.has_construction_site` / `.has_damaged_building`;
  `WorldStateQueries::colony_state_query` extended with two more
  `Has<>` rows; `populate_world_state` now sources both locals from
  the singleton query and pushes them into `MarkerSnapshot::set_colony`.
  5 new tick-system tests; allowlist drops both 169 entries (only
  `HideEligible 170` remains). Behavior invariant by construction:
  same predicate, same threshold (`damaged_building_threshold = 0.4`),
  same FixedUpdate ordering — soak `structures_built` 9 → 8 within
  noise. Verdict shows accumulated drift since the 2026-05-02
  baseline (commit `0783194`); burial=0 mirrored in pre-169 archive
  (tracked under ticket 035). `HasGarden` ECS-level promotion left
  to a follow-on; the `set_colony(HasGarden::KEY, …)` snapshot
  bridge already satisfies the substrate-stub lint.
