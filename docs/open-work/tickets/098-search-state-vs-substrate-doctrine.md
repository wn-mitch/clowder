---
id: 098
title: Document the substrate-vs-search-state boundary in `docs/systems/ai-substrate-refactor.md`
status: ready
cluster: docs
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

092's planning surfaced a subtle category error in the ticket text itself: 092 §Scope listed seven `PlannerState` fields as "mirror" candidates for migration to `StatePredicate::HasMarker(...)`, but only two were actually mirrors. The other five split into:

- **Hybrid** (`materials_available`) — entry from the world fact, mutated by `StateEffect::SetMaterialsAvailable(true)` in `DeliverMaterials`. Tracked in [096](096-materials-available-substrate-split.md).
- **Search-state only** (`prey_found`, `interaction_done`, `construction_done`, `farm_tended`) — set by `StateEffect::Set*` during A* expansion. Their truth value means "this hypothetical plan has executed step X," not "the world has fact X." There is no marker counterpart because there's no parallel authoring to collapse — the planner's own state machine is the only writer.

Forcing search-state booleans through the marker substrate is a category error: A* needs per-node mutable feasibility, but `MarkerSnapshot` is a single shared read-only reference. The substrate-over-override pattern applies to **world facts**; per-node simulation state is correctly modeled as planner state, not substrate.

This doctrine isn't currently written down. 092's confusion will recur on every future substrate-migration ticket unless the boundary is explicit.

## Scope

Add a §SubstrateVsSearchState section to `docs/systems/ai-substrate-refactor.md` that:

1. Defines **substrate** — markers authored from observable world state, read identically by IAUS DSE eligibility and GOAP `StatePredicate::HasMarker(...)`. Single source of truth across L2 and L3.
2. Defines **search-state** — fields on a domain's planner state set by `StateEffect`s during A* expansion. Per-node mutable. No external authorship.
3. Gives the test for "is X substrate or search-state?": *Does an `Effect::Set*` mutate it during search?* If yes, search-state. If no AND there's an external authorship path (a marker authoring system), substrate.
4. Hybrid case: handled by splitting (entry-marker + per-plan search field). Cross-reference [096](096-materials-available-substrate-split.md) as the canonical exemplar once it lands.
5. Anti-pattern: trying to "marker-ify" search-state booleans. Reference 092 §Scope's overstated migration list as the failure mode.

Also link from the relevant CLAUDE.md section so the doctrine is one click away when the next substrate-migration ticket opens.

## Reproduction / verification

`just check` (no code changes); the doc-update lint passes.

Manual review: pick a hypothetical new planner field (e.g., `enemy_engaged: bool` with effect `SetEnemyEngaged(true)` in a `Combat` action) — applying §SubstrateVsSearchState should classify it correctly as search-state, not substrate.

## Log

- 2026-04-30: Opened by 092's land commit, per the antipattern-migration follow-up convention codified in `CLAUDE.md` §Long-horizon coordination.
