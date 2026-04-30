---
id: 092
title: Unify MarkerSnapshot ↔ PlannerState/StatePredicate feasibility languages
status: done
cluster: null
landed-at: null
landed-on: 2026-04-30
---

# Unify MarkerSnapshot ↔ PlannerState/StatePredicate feasibility languages

**Why:** 091's H1 fix added `has_stored_food: bool` to *both* the IAUS marker authoring path and `PlannerState`, plumbing it through three `build_planner_state` call sites and the `EatAtStores` precondition — the canonical "patched twice" failure mode this ticket exists to retire. The codebase carried two parallel feasibility languages: IAUS DSEs consulted `MarkerSnapshot` via `EligibilityFilter` (`require(HasStoredFood)`); GOAP actions consulted `PlannerState` via `StatePredicate` (`HasStoredFood(true)`, `ThornbriarAvailable(true)`). Each new gating fact required manual sync across both layers; silent drift between them was bug-producing (091's smoking gun was IAUS knowing stores were empty while GOAP didn't, leading to phantom `[TravelTo(Stores), EatAtStores]` plans).

**Scope (substrate-over-override).** The opening ticket text listed 7 mirror fields for migration. Reading the planner during planning showed only 2 were true mirrors; the other 5 split into hybrid (`materials_available`, mutated by `SetMaterialsAvailable(true)` in `DeliverMaterials` — parked as 096) and search-state-only (`prey_found`, `interaction_done`, `construction_done`, `farm_tended` — set by `StateEffect::Set*` during A* expansion, no marker counterpart, parked as the doctrine ticket 098). The substrate-over-override pattern applies to world facts authored from observable state, not per-node A* search simulation state. Migration target narrowed to `HasStoredFood` + `ThornbriarAvailable`.

**What landed:**

1. **`src/ai/planner/mod.rs`** — added `PlanContext<'a> { markers: &MarkerSnapshot, entity: Entity }`, added `StatePredicate::HasMarker(&'static str)`, dropped `HasStoredFood`/`ThornbriarAvailable` variants, dropped `has_stored_food`/`thornbriar_available` fields from `PlannerState`, dropped vestigial `CatDomain` impl (no consumers). `StatePredicate::evaluate`, `GoapActionDef::is_applicable`, `GoalState::is_satisfied`/`heuristic`, and the concrete `make_plan` all gained `&PlanContext` parameters; `PlannerState` itself stayed `Hash + Eq` (no lifetimes) since the snapshot is search-time read-only.
2. **`src/ai/planner/actions.rs`** — `EatAtStores` precondition swapped `HasStoredFood(true)` → `HasMarker(markers::HasStoredFood::KEY)`; `GatherHerb` (under SetWard hint) swapped `ThornbriarAvailable(true)` → `HasMarker(markers::ThornbriarAvailable::KEY)`. Test fixtures consolidated under a `plan!` macro that builds the context. Two new substrate tests: `eat_at_stores_unblocked_by_has_stored_food_marker` (positive: marker present → plan reachable) and `marker_change_flips_plan_reachability` (invariant: planner reads the same snapshot the IAUS does, so flipping the marker flips planning outcome).
3. **`src/ai/planner/goals.rs`** — `goal_for_disposition(kind, current_trips, has_stored_food: bool)` → `goal_for_disposition(kind, current_trips, ctx: &PlanContext)`. Resting partial-goal branch consults `ctx.markers.has(HasStoredFood::KEY, ctx.entity)` directly — same lookup the action-side `EatAtStores` precondition uses, collapsing what would otherwise be a 3-site duplicated conditional in the planner callers.
4. **`src/systems/goap.rs`** — `build_planner_state` dropped its `food_available` parameter and the `thornbriar_available`/`has_stored_food` field initializers. The `evaluate_and_plan` planning site passes `PlanContext { markers: &markers, entity }` (markers were already in scope from the IAUS scoring path). `StepSnapshots.has_stored_food: bool` retired in favor of `StepSnapshots.planner_markers: MarkerSnapshot` — built once per tick alongside the per-tick stores scan. The two `resolve_goap_plans` replan paths construct a `PlanContext { markers: &snaps.planner_markers, entity: cat_entity }` and thread it through `make_plan` + `goal_for_disposition`.
5. **`CLAUDE.md`** — added a §Long-horizon coordination paragraph making antipattern-migration follow-ups non-optional. Parked subscope from a substrate-over-override ticket MUST open concrete `tickets/NNN-<slug>.md` files in the same commit that lands the parent — "open as follow-on if desired" rots into lost context in a large repo. The 092 land applied this rule for the first time (096/097/098 below).

**Follow-up tickets opened in this commit** (per the new convention):

- **[096 — Materials-available substrate split](../tickets/096-materials-available-substrate-split.md)** — split the hybrid `materials_available` field into a marker-backed entry check + per-plan `materials_delivered: bool` search field. After this lands, zero mirror fields remain on `PlannerState`.
- **[097 — Non-cat planner audit](../tickets/097-non-cat-planner-substrate-audit.md)** — audit fox / hawk / snake planners for the same parallel-feasibility-language smell. Each implements `core::GoapDomain` for its own state struct + predicate enum.
- **[098 — Search-state-vs-substrate doctrine](../tickets/098-search-state-vs-substrate-doctrine.md)** — document the boundary in `docs/systems/ai-substrate-refactor.md` so future substrate-migration tickets don't repeat 092's category error of listing search-state booleans as mirror candidates.

**Verification:**

- `just check` — clean (clippy + step-contracts + time-units + iaus-coherence).
- `cargo test --lib` — 1645/1645 pass (up from 1640+, includes the two new 092 substrate tests).
- `just soak 42 && just verdict` against the canonical seed-42 deep-soak — every footer field shows `delta_pct: 0.0, band: noise`. Bit-identical to the pre-092 baseline. Same deaths (4× ShadowFoxAmbush, 2× WildlifeCombat, 1× Injury, 1× Starvation), same plan-failure shape, same wildlife counts, same anxiety-interrupt total (86,547), same positive/neutral feature counts. The verdict's `fail` is the pre-existing residual: Starvation=1 (Nettle late-game, 091 land documented) and continuity (`mentoring=0, burial=0, courtship=0` — 087-era regression tracked in 094). Bit-identical is the right outcome for a substrate-unifying refactor.

**Surprise.** Two of them. (a) The `CatDomain` impl on the generic `core::GoapDomain` trait was vestigial — grep showed zero consumers; only `mod.rs` itself referenced it. The cat planner has used the concrete `make_plan` since the trait was extracted. Threading `PlanContext` through the trait would have required a `Context` associated type with lifetimes (GAT territory); dropping the dead impl avoided that complexity entirely. (b) The original ticket text listed 7 fields as "mirror" candidates, but only 2 actually were. The other 5 don't have marker counterparts because they're A*'s own per-node simulation state — `prey_found=true` means "the simulated cat has run `SearchPrey`," not "prey exists in the world." Marker-ifying them would be a category error: A* needs per-node mutable feasibility, the snapshot is shared read-only. Surfaced the distinction and codified it as 098.
