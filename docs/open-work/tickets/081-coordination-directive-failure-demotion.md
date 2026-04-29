---
id: 081
title: Coordination directive-failure demotion (colony-side stuck-loop guard)
status: blocked
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: [072, 073]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Audit gap #10 (minor → important once 072–076 land, since the directive-dispatch layer is the colony-side analog of per-cat plan-stasis). When the coordinator issues "build a kitchen" but materials are depleted, cats Build → fail → replan → coordinator re-assigns the same directive on its next tick → cats Build again → fail. The same stuck-loop pattern at the colony-coordination layer.

The per-cat 073 cooldown handles the cat side (the cat won't re-target the same blocked build site). But the coordinator keeps issuing the directive to *new* cats, who hit the same wall. Without a colony-side memory of cross-cat failure, the coordinator perpetually mis-assigns labor.

Parent: ticket 071. Blocked by 072 (`plan_substrate::lifecycle::record_step_failure`) and 073 (`RecentTargetFailures` aggregate readable as a sensor).

## Scope

- Read aggregate `RecentTargetFailures` per directive in `coordination::accumulate_build_pressure` (`src/systems/coordination.rs:788–862`) and the dispatch site at `dispatch_urgent_directives` (line ~862). Compute the cross-cat failure count for the directive's action+target pair.
- When the aggregate failure count exceeds `directive_failure_threshold`, **demote** the directive: reduce its priority for `directive_failure_demotion_ticks`. Cats stop being preferentially assigned to it; alternative directives surface.
- After the demotion window, restore default priority (the directive can be retried; if conditions still block, it demotes again).
- New `DirectiveFailureLedger` resource — tracks `(DirectiveId) → (failure_count, last_demote_tick)`. Pruned on directive completion or coordinator restart. (Or extend an existing coordinator resource if one fits the shape.)
- New `CoordinationConstants` knobs: `directive_failure_threshold: u32` (default 5 cross-cat failures), `directive_failure_demotion_ticks: u64` (default ~4000 ≈ 1 sim-hour).
- New `Feature::DirectiveDemoted` (Neutral) fires when the demotion triggers.

## Out of scope

- Adding a shared "we tried this and it didn't work" narrative event for the colony — that's narrative-layer concern, not coordination-layer.
- Auto-resolution of the underlying blocker (e.g., automatically dispatching a forage directive when materials are depleted) — that's higher-level coordinator policy; this ticket only adds demotion.
- Tuning the threshold / window — pick conservative defaults; tune via post-landing soak.

## Approach

Files:

- `src/systems/coordination.rs` — extend `accumulate_build_pressure` (line ~788) and `dispatch_urgent_directives` (line ~862) to consult the ledger; add demotion logic. The priority reduction multiplier is a constant (e.g., ×0.1) similar to 073's cooldown shape.
- `src/resources/coordination.rs` (or wherever directive state lives — find via grep on `Coordinator` / `Directive`) — add `DirectiveFailureLedger` resource. `HashMap<DirectiveId, (u32 failure_count, u64 last_demote_tick)>`.
- `src/resources/sim_constants.rs::CoordinationConstants` — add `directive_failure_threshold: u32` and `directive_failure_demotion_ticks: u64`.
- `src/resources/system_activation.rs` — `Feature::DirectiveDemoted` (Neutral); exempt from `expected_to_fire_per_soak()`.
- `src/systems/plan_substrate/lifecycle.rs::record_step_failure` — on Build/Harvest/etc. failure with a directive-tagged target, increment the ledger. The hook is via the cat's plan carrying a `directive_id` field if it does today; otherwise via the directive resource's mapping of `(action, target) → directive_id`.

## Verification

- `just check && just test` green.
- Unit test: the ledger increments on directive-tagged failure, and the directive's priority drops by the configured factor once the threshold is hit.
- Unit test: the ledger expires entries past `last_demote_tick + directive_failure_demotion_ticks`.
- Synthetic-world integration test: 5 cats each fail Build on the same materials-depleted directive → the 6th cat is assigned a different directive; after the demotion window expires, the directive becomes assignable again.
- `just soak 42 && just verdict logs/tuned-42-081` — hard gates pass.

## Log

- 2026-04-29: Opened under sub-epic 071.
