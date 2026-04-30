---
id: 096
title: Split `PlannerState.materials_available` into marker-backed entry + per-plan search field
status: ready
cluster: ai-substrate
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

092 unified `HasStoredFood` and `ThornbriarAvailable` into the substrate via `StatePredicate::HasMarker(...)`, retiring those mirror fields from `PlannerState`. `materials_available` was deferred because it's hybrid — entry-time it mirrors the world fact "the nearest reachable construction site has all materials delivered," but during A* search it is *also* mutated by `StateEffect::SetMaterialsAvailable(true)` from the `DeliverMaterials` action's effect list. That mutation lets the planner reason "after I deliver, materials are available, so the next Construct step is applicable" inside a single A* expansion — without it, multi-step build plans (`[Pickup, TravelTo, Deliver, Construct]`) wouldn't compose.

Replacing it with a pure `HasMarker(MaterialsAvailable)` query loses the search-time mutation and breaks building plans. The cleanest cure is a split:

- **Entry side (substrate)**: a colony-or-per-cat `MaterialsAvailable` marker authored from the same per-site `materials_complete()` ledger `build_planner_state` reads today (`src/systems/goap.rs:5568-5577`). The planner gates entry on `StatePredicate::HasMarker(MaterialsAvailable::KEY)`.
- **Search side (planner state)**: a new `PlannerState.materials_delivered_this_plan: bool` field (default `false`), set by `DeliverMaterials`'s effect. `Construct`'s precondition becomes `HasMarker(MaterialsAvailable) || MaterialsDeliveredThisPlan(true)` — either the world already has it, or the plan delivered it.

After the split: zero mirror fields remain on `PlannerState`. Adding a future colony fact = one marker, one `HasMarker(...)` predicate at the consumer.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)). The 092 ticket parked this case explicitly — see 092 §Out of scope and 092's plan rationale. The hybrid is exactly the shape `093` warns about: a *world fact* and a *search-state assumption* sharing one boolean is a category-error mirror that resists the substrate cure unless split.

**Hack shape**: `PlannerState.materials_available` doubles as world-state mirror (read at plan entry from per-site `materials_complete`) and search-state simulator (mutated by `SetMaterialsAvailable(true)` in `DeliverMaterials`). The two readings disagree at the start of any plan that includes a Deliver step, and the planner relies on that disagreement to compose multi-step builds.

**IAUS lever**: split. Substrate authors a `MaterialsAvailable` marker; planner adds a `materials_delivered_this_plan` search field; `Construct` precondition becomes the disjunction.

**Sequencing**: depends on 092 (`StatePredicate::HasMarker` exists). After this lands, no mirror fields remain on `PlannerState`.

## Reproduction / verification

After the refactor:

```
just check
cargo test --lib                    # 1645+ tests pass
just soak 42
just verdict logs/tuned-42
```

Building canaries (multi-trip founding builds, coordinator-spawned prefunded sites) must continue to land. The existing tests `building_haul_then_construct` and `building_construct_short_circuit_when_materials_already_available` (`src/ai/planner/actions.rs`) cover both cycles — they migrate to the new split and pin both paths.

## Out of scope

- Per-site granularity beyond the existing nearest-site lookup (a separate ticket if it ever bites).

## Log

- 2026-04-30: Opened by 092's land commit, per the antipattern-migration follow-up convention codified in `CLAUDE.md` §Long-horizon coordination.
