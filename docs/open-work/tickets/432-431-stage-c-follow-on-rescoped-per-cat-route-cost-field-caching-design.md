---
id: 432
title: 431 Stage C follow-on — rescoped per-cat route-cost field caching design
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 431's original Stage C design (cache `RouteCostField` per `(Entity, GoalZoneId)` invalidated on `CatMoved` + `MapTileChanged`) assumed high cache hit rates at the `flood_dijkstra` call site (`src/systems/goap.rs:2040`). The 431 closeout layer-walk revealed the assumption was structurally wrong: `evaluate_and_plan` runs only for cats `Without<GoapPlan>` (`goap.rs:1366`), which means it fires per-replan, not per-tick. By definition a cat without a current plan just completed its previous plan (i.e. moved between the previous `flood_dijkstra` and this one). The cache hit rate at the natural call site is structurally ~0%.

Catalog row #4's 3.52% inclusive CPU on `flood_dijkstra` and row #10's 1.62% on `find_full_path` stay on the table. They need a rescoped design — most likely lifting the flood out of the planner-only-fires-on-replan path into a per-tick-but-cached substrate (e.g. flood per cat once per FixedUpdate, store on a Component or sibling Resource, invalidate on `CatMoved` for the cat or `MapTileChanged` for the tile). That decouples the cache from the replan cadence so hit rate becomes meaningful (hits when the cat hasn't moved and overlays haven't changed since last flood).

## Scope

- Design a substrate boundary where the per-cat `RouteCostField` is rebuilt only when the cat moves OR a relevant overlay tile mutates, rather than per-replan.
- Author `MapTileChanged { x, y }` Bevy `Message` (originally part of 431 Stage C). Emit sites: every writer to `TileMap`, `FoxScentMap`, `CorruptionLens` overlays.
- Refactor `evaluate_and_plan`'s flood call (`goap.rs:2040`) to read the cached `RouteCostField` instead of rebuilding. Audit the overlay-weight role-switch case (Patrol vs non-Patrol changes weights mid-soak) — that's a cache-invalidating condition not in `CatMoved`/`MapTileChanged`.
- Verify perf delta via flamegraph before/after; verify behavior preserved via `just verdict` semantic-pass against the post-431 baseline.

## Out of scope

- The `find_full_path` (`CatPathPlan::find_full_path` — row #10) is touched by the same substrate but its specific use lives in `dispatch_step_action`'s per-step execution path. Co-design if cheap; otherwise defer separately.
- Cross-system snapshot dedupe (Stage F territory) — that's a separate ticket if/when meaningful.

## Current state

Opened 2026-05-20 alongside the 431 closeout. Blocked by 431 only because 431 lands the binary-commit-truth tooling that this ticket's perf verification depends on; the actual implementation can start as soon as 431 lands.

## Approach

The simplest design: track per-cat "flood validity" as a Component `RouteCostFieldFreshness { origin: Position, origin_tick: u64, overlay_role: PatrolOrOther }`. When `evaluate_and_plan` reaches the flood site, check the freshness component: if origin matches current pos AND no `MapTileChanged` since `origin_tick` AND overlay_role unchanged, reuse the existing `RouteCostField` Component. Otherwise rebuild + update freshness.

Alternative: build a dedicated `populate_route_cost_fields` system that runs once per FixedUpdate (after `update_near_pair_cache`), iterates cats with stale fields, and re-floods only those. Decouples the flood entirely from `evaluate_and_plan`.

Prefer the alternative — the planner-coupled approach inherits the per-replan cadence by default; the dedicated populator is the architecturally cleanest decoupling and mirrors Stage B's `update_near_pair_cache` pattern.

## Verification

- Flamegraph before/after: row #4 (`flood_dijkstra`) inclusive CPU drops from 3.52% to < 1%.
- `just verdict logs/tuned-42-<commit>/` semantic-pass against the post-431 baseline (continuity canaries hold, no survival regressions).
- Cache-vs-rebuild debug-only invariant assertion mirroring 431 Stage B's `passive_familiarity` invariant guard: every N ticks, rebuild the field for one focal cat and compare cell-by-cell against the cached version.

## Log

- 2026-05-20: opened as a 431 follow-on after 431 Stage C analysis surfaced the structural cache-hit-rate issue. The original Stage C design's cache hit rate at the planner-coupled call site is ~0% because the planner only fires on plan completion (i.e. after the cat moved). Rescope: decouple the flood from the planner cadence so the cache invalidation conditions (`CatMoved`, `MapTileChanged`) determine flood frequency instead of replan frequency.
