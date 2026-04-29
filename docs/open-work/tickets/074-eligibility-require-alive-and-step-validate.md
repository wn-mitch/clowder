---
id: 074
title: EligibilityFilter::require_alive + step-resolver validate_target
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

Audit gaps #3 and #4 (both blocking severity).

**Gap #3.** Step carryover at `goap.rs:2817–2820` copies `target_entity` from the prior step without checking the entity is still alive. A target picked at tick T may be `Dead` / `Banished` / `Incapacitated` / despawned by tick T+N. The plan keeps executing with a stale entity reference.

**Gap #4.** `TravelTo(target)` and other multi-step plans keep navigating to a despawned entity. The cat wastes plan cycles on impossible navigation.

Combined into one ticket because gap #3 (eligibility filter, gates *before* scoring) and gap #4 (step-resolver guard, gates *during* execution) are belt-and-suspenders for the same problem: dead targets must never enter the plan. Gap #3 is the load-bearing fix at the IAUS layer; gap #4 catches mid-step despawn that gap #3 missed.

Parent: ticket 071. Blocked by ticket 072 (`plan_substrate` module).

## Scope

- Add `EligibilityFilter::require_alive(target_input_key)` constructor in `src/ai/scoring.rs:37`. Reads cat-population validity (`Dead` / `Banished` / `Incapacitated` / despawned) the same way `require(marker)` reads `MarkerSnapshot`. When the candidate is invalid, the DSE scores 0.0.
- Register `plan_substrate::require_alive_filter()` on the six target-DSE builders (chain `.eligibility(plan_substrate::require_alive_filter())` on each).
- Implement `plan_substrate::validate_target` body (072 stubbed it). Returns `Result<(), TargetInvalidReason>`.
- Implement `plan_substrate::carry_target_forward`'s dead-entity check (072 stubbed it). On invalid target, write into `RecentTargetFailures` (via 073's hook) and return `None`, triggering the calling system's existing `PlanStepFailed` path with reason `TargetDespawned`.
- Step resolvers in `src/steps/disposition/*.rs` and `src/steps/building/*.rs` already call `plan_substrate::validate_target` (072 added the call sites with stub bodies). 074 just makes the calls meaningful.

## Out of scope

- Reservation / contention (audit gap #9) — separate ticket 080.
- Re-validating non-entity inputs (e.g., a tile that became corrupted between scoring and stepping). The per-tile state belongs in marker authors; this ticket scopes to entity-target validity only.

## Approach

Files:

- `src/ai/scoring.rs::EligibilityFilter` — add `require_alive` variant + constructor (around line 37 alongside the existing `require(marker)`). Reads the same `TargetValidityQuery` that `plan_substrate::validate_target` uses for consistency.
- `src/ai/dses/socialize_target.rs:83` (`socialize_target_dse()`) — chain `.eligibility(plan_substrate::require_alive_filter())` on the builder.
- `src/ai/dses/mate_target.rs`, `groom_other_target.rs`, `hunt_target.rs`, `forage_target.rs`, `engage_threat_target.rs` — same shape.
- `src/systems/plan_substrate/target.rs::validate_target` — flesh out body. Returns `Result<(), TargetInvalidReason>` with variants `Dead`, `Banished`, `Incapacitated`, `Despawned`.
- `src/systems/plan_substrate/target.rs::carry_target_forward` — extend with the dead-entity check. On invalid target, write into `RecentTargetFailures` (the `recent` argument added in 072), return `None`. The caller's existing `PlanStepFailed` path picks up the `None` and fails the step with reason `TargetDespawned` (variant added in 072).
- `src/systems/plan_substrate/target.rs::require_alive_filter` — flesh out body. Returns an `EligibilityFilter::require_alive(target_input_key)` constructed against the canonical target-input key (matches the `TARGET_*_INPUT` constants the resolvers consume).

## Verification

- `just check && just test` green.
- Unit test: `require_alive` filter returns 0.0 for a Dead / Banished / Incapacitated / despawned candidate, 1.0 otherwise.
- Unit test: `validate_target` returns the right `TargetInvalidReason` for each invalidity flavor.
- Synthetic-world integration test: a cat targeting a still-alive entity has its target killed mid-step → the next step's `validate_target` fails fast and the cat replans to a new target, logged in `RecentTargetFailures`.
- Synthetic-world integration test (interaction with 073): A cat picks target X, X dies, cat replans → IAUS picks a different target because (a) `require_alive` gates X to 0.0 in scoring, and (b) `RecentTargetFailures` records X's failure for the cooldown.
- `just soak 42 && just verdict logs/tuned-42-074` — hard gates pass.

## Log

- 2026-04-29: Opened under sub-epic 071.
