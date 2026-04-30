---
id: 097
title: Audit fox / hawk / snake planners for the parallel-feasibility-language smell 092 retired for cats
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

092 collapsed the cat planner's parallel-feasibility-language smell: GOAP `StatePredicate::HasStoredFood`/`ThornbriarAvailable` consulted `PlannerState` mirror fields, while IAUS `EligibilityFilter::require(...)` consulted `MarkerSnapshot` — two parallel records of the same world facts that could drift silently. Retired by adding `StatePredicate::HasMarker(&'static str)` and threading `PlanContext { markers, entity }` through the cat planner's A* loop.

The same smell may exist in the non-cat planners:
- `src/ai/fox_planner/{mod.rs, actions.rs, goals.rs}` — fox state types, used for shadow-fox AI.
- `src/ai/hawk_planner/actions.rs` — hawk state.
- `src/ai/snake_planner/actions.rs` — snake state.

Each implements `core::GoapDomain` for its own state struct + predicate enum. `src/systems/fox_goap.rs:448` already builds a fox `MarkerSnapshot` for IAUS DSE eligibility, so the substrate exists on the fox side; the question is whether the fox planner mirrors any colony/per-fox facts (e.g., `StoreVisible`, `CatThreateningDen`, `HasCubs`, `WardNearbyFox`) on its `PlannerState`-equivalent and whether those mirrors drift.

## Scope

For each non-cat planner, audit:
1. Predicate enum variants. Identify any that mirror an existing marker (per `src/components/markers.rs` fox-marker section, lines 411-470).
2. State-struct fields. Identify mirror booleans (set at entry, never mutated by effects).
3. If mirrors exist, port to the same shape 092 used for cats:
   - Add `HasMarker(&'static str)` (or a domain-specific equivalent) to the predicate enum.
   - Thread a `&MarkerSnapshot` + entity into the species' `make_plan` call (likely via the generic `core::GoapDomain` trait — adding a `Context` associated type, defaulting to `()` for species without markers, or adopting cat's concrete-`make_plan` shape).
4. If no mirrors exist, document the audit result in `docs/systems/ai-substrate-refactor.md` and close.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)). 092 fixed the cat side; this ticket completes the structural fix across all species so the doctrine ("markers are the substrate, planner consumes the substrate, never re-authors it") holds uniformly.

## Reproduction / verification

```
just check
cargo test --lib
just soak 42
just verdict logs/tuned-42
```

For non-cat species, the soak's footer fields (`fox_predation_attempts`, `cubs_born`, `hawk_strikes`, etc.) should be unchanged within ±10% — this is a refactor, not a balance change.

## Log

- 2026-04-30: Opened by 092's land commit, per the antipattern-migration follow-up convention codified in `CLAUDE.md` §Long-horizon coordination.
