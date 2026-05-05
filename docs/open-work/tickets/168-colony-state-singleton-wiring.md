---
id: 168
title: Wire ColonyState colony-singleton entity (Phase 4b.2 promotion)
status: in-progress
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

`ColonyState` is declared in `src/components/markers.rs:300` as the marker
for the single colony-state entity that colony-scoped markers
(`HasFunctionalKitchen`, `HasRawFoodInStores`, `HasStoredFood`,
`ThornbriarAvailable`, `WardStrengthLow`, `WardsUnderSiege`, …) are spec'd
to attach to via `Q<With<ColonyState>, With<MarkerN>>` queries.

Today the marker has zero readers and zero writers in `src/`:

- `src/systems/goap.rs:913` carries a deferred-comment naming this
  promotion as Phase 4b.2 of the substrate refactor.
- `src/ai/scoring.rs:89-94` describes the target `Q<With<ColonyState>,
  With<MarkerN>>` shape but gates it behind "(when promoted)".
- All colony-scoped markers currently attach to per-cat entities and
  the evaluator queries them per-cat — exactly the pattern the
  singleton promotion is meant to replace.

160's substrate-stub lint flags `ColonyState` as `fully-orphan`. This
ticket wires it.

## Scope

1. **Spawn the singleton.** Extend the colony spawn path
   (`SpawnColony` handler) to insert `commands.spawn((ColonyState, …))`
   exactly once per simulation. Add a debug-assert that exactly one
   such entity exists.
2. **Migrate colony-scoped markers** off per-cat entities onto the
   `ColonyState` singleton. This is the substantive change — affects
   the marker-author systems (`buildings.rs::update_colony_building_markers`,
   `magic.rs::update_herb_availability_markers`, etc.) and the queries
   in `src/ai/scoring.rs` and `src/systems/goap.rs` that consume them.
3. **Update the substrate spec** §4.3 line ~1985 to flip the colony-marker
   rows from "(promoted)" pending to landed, and remove the deferred-comment
   in `src/systems/goap.rs:913`.
4. **Drop the `ColonyState` allowlist entry** from
   `scripts/substrate_stubs.allowlist`. The 160 lint will then enforce
   that the marker stays wired.

## Out of scope

- Authoring the orphan markers `HasConstructionSite` /
  `HasDamagedBuilding` — covered by ticket 169 (same author system,
  different marker type).
- `HideEligible` authoring — covered by ticket 170.

## Current state

160 just landed; the `ColonyState` allowlist entry references this ticket.
Spec is `docs/systems/ai-substrate-refactor.md` §4.3 (lines ~1985-1995
list the colony-scoped markers that depend on this).

## Verification

1. Spawn singleton exists exactly once after `SpawnColony` (debug-assert).
2. Every colony-scoped marker query in `src/ai/scoring.rs` and
   `src/systems/goap.rs` matches the singleton, not per-cat entities.
3. `just check` passes after dropping the allowlist entry.
4. `just soak` + `just verdict` — the migration is a substrate refactor
   and **a refactor that changes sim behavior is a balance change** per
   CLAUDE.md. Confirm no canary regression and footer drift < ±10% on
   characteristic metrics. If drift > ±10%, draft hypothesis per
   `just hypothesize` workflow.

## Log

- 2026-05-05: opened in same commit as 160. Allowlist entry pending
  this ticket landing.
