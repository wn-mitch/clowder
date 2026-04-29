---
id: 080
title: Resource reservation — Reserved component + EligibilityFilter::require_unreserved
status: blocked
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: [072]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Audit gap #9 (important severity). Two cats can pick the same carcass / herb-tile / non-Intention mate target. There's no `Reserved` component anywhere in the codebase; first-come-first-served only. The loser arrives, finds the resource gone, fails, replans — and without 073's cooldown applied to the right entity-action key, may re-pick the same vanished target. With 073 in place the loser eventually shifts targets, but the wasted plan cycles are themselves a stuck-loop risk multiplier.

A first-class reservation primitive eliminates the contention at scoring time. Cats with the reservation continue to score normally; others see 0.0 from the eligibility filter and pick alternative candidates. Lands as an `EligibilityFilter` (the existing engine pattern for hard gates), not as a post-hoc check in resolver bodies.

Parent: ticket 071. Blocked by ticket 072 (`plan_substrate::target` API extension point).

## Scope

- New `Reserved { owner: Entity, expires_tick: u64 }` component on resources (carcass, herb tile, prey, build site, mate). Marker-style component; `Default` not derived (an empty `Reserved` is meaningless).
- `plan_substrate::target` API extension:
  - `pub fn reserve_target(commands: &mut Commands, target: Entity, owner: Entity, tick: u64, ttl_ticks: u64)` — writes `Reserved` with `expires_tick = tick + ttl_ticks`.
  - `pub fn release_target(commands: &mut Commands, target: Entity)` — removes `Reserved`.
  - `pub fn require_unreserved_filter(self_input_key: &str) -> EligibilityFilter` — gates a target DSE to 0.0 if `Reserved.owner != self_entity` and `tick < expires_tick`. Cats with the reservation continue to score normally; others see 0.0.
- Reservation lifecycle hooks in `plan_substrate::lifecycle`:
  - `record_target_picked(commands, cat, action, target, tick)` — new; called from `goap.rs` when a resolver commits to a target. Calls `reserve_target` with `ttl = reservation_ttl_ticks`.
  - `abandon_plan(...)` — release any reservations the abandoned plan held. Extend the API from 072 to track reservations on `AbandonedPlanState`.
  - `record_step_failure(...)` — release reservation on terminal failure of a `Harvest` / `Build` / `Mate` step.
- New maintenance system `expire_reservations` slotted into chain 2a's decay batch — removes `Reserved` components past their `expires_tick`. Bounds the world-size of the marker.
- Register `plan_substrate::require_unreserved_filter` on the appropriate target DSEs:
  - `hunt_target.rs` — prey
  - `forage_target.rs` — herb tiles
  - Carcass-harvest DSE — carcass entities
  - `mate_target.rs` — chosen mate
  - **Skip** target DSEs where contention is OK by design (e.g., `socialize_target.rs` — multiple cats can socialize at the same partner over time; `groom_other_target.rs` similarly).
- New `PlanningSubstrateConstants` knob: `reservation_ttl_ticks: u64` (default ~600 ticks, ~1 in-sim hour — long enough to traverse + execute, short enough that abandoned reservations expire fast).
- New `Feature::ReservationContended` (Neutral) fires from `require_unreserved_filter` when it gates a candidate to 0.0 — observability without a side channel.

## Out of scope

- Reservation on socialize / groom / play targets — multiple cats can socialize at the same partner, and the existing target-cooldown (073) handles disambiguation if it becomes a problem.
- Cross-tile reservation (e.g., a path or a tile-region) — only entity-level reservation in this ticket.
- Tuning `reservation_ttl_ticks` — pick a conservative default; tune via post-landing soak.

## Approach

Files:

- `src/components/reserved.rs` — new. `Reserved { owner: Entity, expires_tick: u64 }`.
- `src/components/mod.rs` — register.
- `src/systems/plan_substrate/target.rs` — add `reserve_target`, `release_target`, `require_unreserved_filter`.
- `src/systems/plan_substrate/lifecycle.rs` — extend `record_target_picked` (new), `abandon_plan` (release tracked reservations), `record_step_failure` (release on terminal failure).
- `src/ai/scoring.rs::EligibilityFilter` — add `require_unreserved` variant.
- `src/ai/dses/hunt_target.rs`, `forage_target.rs`, `mate_target.rs`, carcass-harvest DSE — register the filter via `.eligibility(plan_substrate::require_unreserved_filter(TARGET_*_INPUT))` on the builder.
- `src/resources/sim_constants.rs::PlanningSubstrateConstants` — add `reservation_ttl_ticks`.
- `src/resources/system_activation.rs` — `Feature::ReservationContended` (Neutral); exempt from `expected_to_fire_per_soak()` until soak data confirms.
- New maintenance system `expire_reservations` — registered in chain 2a's decay batch (`src/plugins/simulation.rs:379`).

## Verification

- `just check && just test` green.
- Unit test: `reserve_target` writes the component with the correct `expires_tick`.
- Unit test: `release_target` removes the component.
- Unit test: `require_unreserved_filter` returns 0.0 for non-owners during the reservation window, 1.0 otherwise (and 1.0 for the owner regardless).
- Synthetic-world integration test: two cats both target the same carcass at tick T → only the first succeeds (its reservation gates the second); the second's resolver picks an alternative or returns `None`. After `expires_tick`, the carcass becomes available again.
- Synthetic-world integration test: cat picks target → plan abandoned → reservation released → another cat can pick the target.
- `just soak 42 && just verdict logs/tuned-42-080` — hard gates pass; expect minor reduction in `HarvestCarcass` plan failures (contention now resolved at scoring rather than mid-step).

## Log

- 2026-04-29: Opened under sub-epic 071.
