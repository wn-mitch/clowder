---
id: 097
title: Audit fox / hawk / snake planners for the parallel-feasibility-language smell 092 retired for cats
status: done
cluster: ai-substrate
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: b6edd580
landed-on: 2026-05-02
---

## Why

092 collapsed the cat planner's parallel-feasibility-language smell: GOAP `StatePredicate::HasStoredFood`/`ThornbriarAvailable` consulted `PlannerState` mirror fields, while IAUS `EligibilityFilter::require(...)` consulted `MarkerSnapshot` — two parallel records of the same world facts that could drift silently. Retired by adding `StatePredicate::HasMarker(&'static str)` and threading `PlanContext { markers, entity }` through the cat planner's A* loop.

The same smell might exist in the non-cat planners:
- `src/ai/fox_planner/{mod.rs, actions.rs, goals.rs}` — fox state types, used for shadow-fox AI.
- `src/ai/hawk_planner/actions.rs` — hawk state.
- `src/ai/snake_planner/actions.rs` — snake state.

## What landed (audit-only close)

**No marker-mirror smell present.** Audit reproduced in `docs/systems/ai-substrate-refactor.md` §4.7.6.

Evidence:

1. **Predicate enums vs effect catalogs.** Every field on `FoxPlannerState`, `HawkPlannerState`, and `SnakePlannerState` has at least one corresponding `*StateEffect::Set*` variant that mutates it during A* expansion (`SetZone`, `SetCarryingFood`, `SetPreyFound`, `SetCubsFed`, `SetTerritoryMarked`, `SetDenSecured`, `SetInteractionDone`, `IncrementTrips` for fox; `SetZone`, `SetPreySpotted`, `SetHungerOk`, `IncrementTrips` for hawk; `SetZone`, `SetPreyInRange`, `SetHungerOk`, `SetWarm`, `IncrementTrips` for snake). Per §4.7.1, fields mutated by effects during plan projection are search state, not substrate; per §4.7.4, marker-ifying them would break A*'s feasibility model.

2. **No markers paralleled by predicates.** The fox marker substrate (`HasDen`, `HasCubs`, `CubsHungry`, `StoreVisible`, `StoreGuarded`, `CatThreateningDen`, `WardNearbyFox`, `IsDispersingJuvenile`) is read by IAUS DSE eligibility filters out of `MarkerSnapshot` at `src/systems/fox_goap.rs:448`. None of these eight markers is named by a `FoxStatePredicate` variant; the fox planner's `DenSecured(bool)` predicate is plan-projected (`EstablishDen` action's `SetDenSecured(true)` effect at `src/ai/fox_planner/actions.rs:161`), not a `HasDen` mirror. Hawks and snakes carry no markers at all.

3. **Layer separation by construction.** The non-cat planners pre-emptively kept the two layers cleanly separated from inception: `MarkerSnapshot` for world-fact substrate (IAUS L2 eligibility), `*PlannerState` for plan-projection search state (GOAP A*). The vector that motivated 092 (predicate boolean initialized from world state, then re-read by IAUS through a parallel marker) doesn't exist on the non-cat side.

**Doctrine recorded:** §4.7.6 in `docs/systems/ai-substrate-refactor.md` so future substrate-migration tickets see the audit result without re-running the grep.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](../tickets/093-substrate-over-override-epic.md)). 092 fixed the cat side; this audit confirms the structural fix isn't needed elsewhere — the non-cat planners already hold the doctrine.

## Verification

- `cargo test --lib` green (no code changes; doc-only).

## Log

- 2026-04-30: Opened by 092's land commit, per the antipattern-migration follow-up convention codified in `CLAUDE.md` §Long-horizon coordination.
- 2026-05-02: Landed as no-op close. Audit reproduced as §4.7.6 in `docs/systems/ai-substrate-refactor.md`. Unblocks ticket 125 (footer schema extension).
