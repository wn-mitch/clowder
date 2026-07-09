---
id: 516
title: score_target_consideration prefix-routing sends unprefixed scalar axes to the no-op fetch_self — hunt_target prey_yield/prey_calm/alertness-tolerance dead in production; 263 affordance key same class
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-07
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [516-hunt-axis-revival.md]
landed-at: afcc14e2
landed-on: 2026-07-09
---

## Why

`score_target_consideration` (`src/ai/target_dse.rs:459`) routes `Consideration::Scalar` inputs by name-prefix magic: names starting with `target_` resolve through the per-target fetcher; **everything else falls through to `fetch_self`** — and every target-taking resolver defines `fetch_self` as a `|_, _| 0.0` stub. Any scalar axis registered without the `target_` prefix silently reads 0.0 for every candidate, with no L2 visibility (the trace records the axis with score 0.0, which reads as "input was low", not "input was never fetched").

**Verified live instance (probe test, 2026-07-07):** `hunt_target`'s `prey_yield`, `prey_calm`, and `prey_alertness_tolerance` axes are all dead in production. A Rabbit listed *first* in the candidate slice loses to a Mouse at equal distance and alertness — hunt target selection currently ranks on the spatial pursuit-cost axis + `target_predictability` only. The §6.5.5 yield-aware targeting (the entire point of the 4c.7 port) and ticket 100's boldness-tolerance axis have never fired. The existing unit tests pass by coincidence: WeightedSum ties break toward the **later** candidate, and every yield/alertness test happens to list its expected winner second (`picks_higher_yield_at_equal_distance`, `alertness_penalizes_otherwise_better_prey`); the remaining tests are decided by the (working) spatial axis.

**Second instance:** 263's dormant `hunt_best_predation_affordance` key has the same defect — at activation (ticket 315) it would read 0.0 for every candidate and the activation soak would "verify" a null axis. 264's six new axes dodged the trap by shipping `target_`-prefixed keys (`target_affordance_socialize` etc.) with tied-position behavioral tests that fail on a dead arm.

This is the silent-canary conjunction class (`learning_silent_canary_conjunction_asymmetry` / pillar 2): a naming convention enforced by convention only, failing silent at the perception layer.

## Scope

- **R1 (structural fix):** make the routing explicit instead of prefix-magical. Preferred shape per `docs/conventions/compile-time-contracts.md`: route **all** scalar considerations on target-taking DSEs through `fetch_target_scalar` (it receives `(name, cat, target)` — strictly more information; a self-scoped input can ignore `target`). Audit first: confirm every target-taking resolver's `fetch_self` is the no-op stub (hunt/socialize/groom/mate/mentor/caretake/apply_remedy/fight/bury/build/herbcraft/dependent_kitten) — if any real self-scoped read exists, keep `fetch_self` for it via an explicit opt-in, not a prefix.
- **R2 (revive hunt axes):** with routing fixed, `prey_yield` / `prey_calm` / `prey_alertness_tolerance` come alive — a real behavioral change to hunt target selection. Four-artifact gate: predictions on per-species kill composition (yield-ranked: Rat > Fish > Rabbit > Bird > Mouse at comparable distance), hunt-success drift within the Phase-V 30–50% biology band work (coordinate with plan step 25 — this fix should land BEFORE or WITH the band calibration so the band is tuned against live axes).
- **R3 (test discipline):** convert the coincidence-passing tests to tied-position, expected-winner-first shape (the 264 pattern: ties break toward the later candidate, so a dead arm fails). Add a compile-time or test-level contract that every `ScalarConsideration` name registered on a target-taking DSE has a matching fetch arm — e.g. a per-DSE test that scores a candidate with a sentinel fetcher asserting every axis name was queried.
- **R4:** re-key 263's `hunt_best_predation_affordance` (dormant — rename is trace-invisible) to survive whatever routing convention R1 lands; unblocks 315.

## Out of scope

- Activating any dormant 263/264 axis (tickets 315 / plan step 20 own).
- Self-state DSE dispatch (`score_actions` / ctx_scalars) — different path, already covered by the score_dse_by_id CI check.

## Current state

- Probe evidence: temporary test `probe_yield_axis_first_candidate` (added+removed in the 264 wire session, 2026-07-07) — rabbit-first loses to mouse at equal distance/alertness: `left: Some(3v0), right: Some(2v0)`.
- 264's axes are already safe (target_-prefixed); hunt's four unprefixed axes are the live blast radius.
- Landing order note: R2 changes hunt behavior — own landing, own soak, archive-vs-archive verdict if landed adjacent to other Phase IV/V behavior changes.

## Verification

- Tied-position behavioral tests per revived axis (expected winner FIRST in the candidate slice).
- Four-artifact soak for R2 (hunt composition + success-rate predictions), `just verdict` hard gates.
- Sentinel-fetcher coverage test (R3) proving every registered scalar axis name is queried during scoring.

## Log

- 2026-07-07: opened from the 264 dormant-wire session (plan.md step 17). Discovered when 264's tied-position tests failed against `affordance_*`-named axes; probe confirmed hunt's yield/calm/tolerance axes dead in production. 264 shipped with target_-prefixed keys as the dodge; this ticket owns the structural fix + hunt revival.
- 2026-07-09: R1–R4 implemented (plan.md step 25, landed ahead of 266 so the hunt-band gates measure live axes). R1: `fetch_self` channel **deleted** from `evaluate_target_taking` / `evaluate_target_taking_with_reservations` / `score_target_consideration` — all scalars route through the target-scoped fetcher; audit confirmed every production resolver's `fetch_self` was the `0.0` stub (13 resolvers). Full unprefixed-axis census: hunt's four + fight's `ally_proximity` (also dead pre-fix, contra its own comment). R2: hunt yield/calm/tolerance live — four-artifact gate vs the post-310 baseline. R3: coincidence tests converted to expected-winner-FIRST tied-position; ticket-100 bold/patient test upgraded from structural to a real margin-gap assertion via `FocalTargetHook`; new `unprefixed_scalar_routes_to_target_fetcher` regression pin in target_dse.rs; new `ally_proximity_lifts_candidate_scores` liveness pin. R4: `hunt_best_predation_affordance` needs no rename — routing is now name-agnostic; 315 unblocks at landing.
- 2026-07-09: **latent design gap noted** — `ally_proximity` is uniform across candidates and `resolve_fight_target` returns only the winning target (goap.rs consumes `picked` alone), so the §6.5.9 "backup confidence" lift never reaches the fight *election*. The axis is now honest but latent until a caller consumes `aggregated_score`. Not this ticket's scope; candidate for a 267-adjacent follow-on.
