---
id: 073
title: RecentTargetFailures component + target_recent_failure Consideration
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

Audit gaps #1 and #2 (both blocking severity) — the load-bearing fix that closes the seed-42 stuck-loop pattern.

**Gap #1.** No per-cat persistence of "target X failed me recently" beyond the per-plan `failed_actions: HashSet<ActionKind>` set on `GoapPlan` (`src/components/goap_plan.rs:30`). When `replan_count > max_replans` the plan is abandoned and the failure memory is destroyed; the cat's next disposition pick re-selects the same action.

**Gap #2.** The six target resolvers (`socialize_target.rs:109–137`, `mate_target.rs:34–39`, `groom_other_target.rs`, `hunt_target.rs`, `forage_target.rs`, `engage_threat_target.rs`) score on bond / distance / novelty / species-compat / partner-bond only — **no input from recent-failure memory**. Even if memory existed, the target picker would re-select the same blocked target.

These two gaps reinforce each other into the fatal pattern: Nettle's 66 identical `TravelTo(SocialTarget)` failures across 100K ticks; Mocha's 109 `HarvestCarcass` failures; Lark's 91 `EngageThreat` failures. Combined into one ticket because gap #1's component has no observable effect alone — the consideration in gap #2 is its first reader.

Parent: ticket 071 (planning-substrate-hardening). Blocked by ticket 072 (`plan_substrate` module).

## Scope

- New `RecentTargetFailures` component on cats: `HashMap<(ActionKind, Entity), u64>` of most-recent-failure tick. Inserted lazily on first failure; pruned by a maintenance system in chain 2a's decay batch.
- New IAUS sensor `target_recent_failure_age_normalized(cat, action, target) -> f32 ∈ [0, 1]` published on `EvalInputs` (`src/ai/scoring.rs:30`) — 1.0 = no recent failure or fully expired; 0.0 = just failed.
- New `Consideration::Scalar(ScalarConsideration::new(plan_substrate::TARGET_RECENT_FAILURE_INPUT, cooldown_curve))` added as the next axis on each of the six target DSEs.
- Cooldown curve: `Piecewise` knots `[(0.0, 0.1), (1.0, 1.0)]` (input is age normalized over `target_failure_cooldown_ticks`). Fresh failure ⇒ candidate's product score multiplied by 0.1; recovers linearly.
- Renormalize each DSE's existing axis weights ×(N/(N+1)) so steady-state scores match pre-073 (no regression on cats with no recent failures).
- New `Feature::TargetCooldownApplied` (Neutral) fires from the sensor when it returns < 1.0.
- `plan_substrate::record_step_failure` and `abandon_plan` (from 072) extend to write into `RecentTargetFailures` when the failed step has a `target_entity`. Call sites pass the `&mut RecentTargetFailures` they already query.
- New `PlanningSubstrateConstants` substruct on `SimConstants` with `target_failure_cooldown_ticks: u64` (default 8000 ≈ 2 sim-hours).

## Out of scope

- Tuning the cooldown ticks or curve shape — pick conservative defaults (8000 ticks, Piecewise 0.1→1.0); tune via `just rebuild-sensitivity-map` after the substrate is fully girded.
- Adding cooldown awareness to non-target DSEs (e.g., `eat`, `sleep` — these are self-state DSEs without a target).
- Recording cooldown for plans that succeeded — only failure paths write.

## Approach

Files:

- `src/components/recent_target_failures.rs` — new. Model after `src/components/pairing.rs:85–98`: small per-cat component, tick-keyed map, `Default` derived.
- `src/components/mod.rs` — register the new module.
- `src/resources/sim_constants.rs` — `PlanningSubstrateConstants` substruct (model after `PairingConstants` at end of file): `target_failure_cooldown_ticks: u64`.
- `src/resources/system_activation.rs` — `Feature::TargetCooldownApplied` (Neutral); exempt from `expected_to_fire_per_soak()` until soak data confirms.
- `src/ai/scoring.rs::EvalInputs` — publish the `target_recent_failure_age_normalized` sensor.
- `src/ai/dses/socialize_target.rs:83` (`socialize_target_dse()`) — add the consideration as the next axis; renormalize the existing five weights ×(5/6).
- `src/ai/dses/mate_target.rs`, `groom_other_target.rs`, `hunt_target.rs`, `forage_target.rs`, `engage_threat_target.rs` — same shape (each builder gains the consideration with its own weight renormalization).
- `src/systems/plan_substrate/lifecycle.rs::record_step_failure` and `abandon_plan` — wire `RecentTargetFailures` writes (the `Option<&mut>` arguments added in 072 now have meaningful inputs from caller queries).
- New maintenance system `prune_recent_target_failures` slotted into chain 2a's decay batch (`src/plugins/simulation.rs:379` alongside `decay_grooming` / `decay_exploration`). Bounds the per-cat map size by expiring entries older than `target_failure_cooldown_ticks`.

## Verification

- `just check && just test` green.
- Unit test: the sensor returns `(now - failed_tick) / cooldown_ticks` clamped to `[0, 1]`.
- Unit test: the cooldown curve maps sensor 0.0 → 0.1, sensor 1.0 → 1.0, sensor 0.5 → 0.55 (linear interpolation between knots).
- Unit test: each renormalized DSE's no-failure steady-state score equals its pre-073 score within fp tolerance (the renormalization preserves shape on the no-cooldown path).
- Synthetic-world integration test: a cat with a blocked `TravelTo(target)` issues a plan, fails, replans → asserts the next target is *different* during the cooldown window.
- Inverse synthetic test: cooldown expires after the configured window → cat re-targets the original candidate.
- `just soak 42 && just verdict logs/tuned-42-073` — hard gates pass. Drift on Socialize / Hunt / Forage DSE picks expected (cooldown shifts toward novel candidates) but bounded.

## Log

- 2026-04-29: Opened under sub-epic 071.
