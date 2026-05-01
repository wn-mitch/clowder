---
id: 076
title: LastResortPromotion Modifier + no-target step resolvers (spiral-of-failure escalation)
status: done
cluster: planning-substrate
added: 2026-04-29
parked: 2026-04-29
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 0b95bbd9
landed-on: 2026-05-01
---

## Why

Audit gap #6 (important severity). When `AnxietyInterrupt` fires (critical hunger / energy / health) the cat's recovery disposition is `Resting` or `Eating`. Today **no escalation path exists if recovery itself fails repeatedly** — no resting site, no food in inventory, no healer, blocked navigation, etc. The cat cycles through failing recovery dispositions until hunger kills it.

Mocha / Nettle / Lark in the failed seed-42 run hit exactly this pattern: anxiety dropped 70% (24,874 → 7,469) because plan-replan churn drowned out anxiety windows, and the cats had no last-resort fallback when their recovery actions kept failing.

Lands as a Modifier promoting no-target last-resort actions (RestInPlace, EatInventoryUnconditional) when recovery has failed N times. Stays inside the IAUS score economy: when the trigger clears (cat actually rests / eats), the modifier deactivates and normal scoring resumes. **No out-of-band side channel; no override of the IAUS pick.**

Parent: ticket 071. Blocked by 072 (`plan_substrate` API) and 073 (`RecentTargetFailures` sensor).

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)). **Re-evaluate this ticket with the substrate-over-override lens before unparking.**

**Hack shape**: this ticket proposes adding a modifier that promotes no-target last-resort actions (RestInPlace, EatInventoryUnconditional) when recovery actions fail N times. The original framing is sound (post-failure escalation), but the lens raises a question: is `LastResortPromotion` the right *shape* of substrate fix, or a wrong-shape hack of its own?

**IAUS lever — alternative framing**: what 076 wants might be **fallback DSEs that are always eligible at low score** (so they win when nothing else can plan), not a special-case modifier that lifts them only post-failure. The existing modifier framing reads as "compensate for the recovery DSEs failing"; the alternative reads as "give cats a substrate-native ground state when all other DSEs fail eligibility." [088](088-body-distress-modifier.md) (Body-distress Modifier) is *proactive* (lifts on distress) and is a clearer substrate primitive; 076's *reactive* (lifts on failure-count) framing is more override-shaped.

**Sequencing**: parked pending 027b reactivation soak (ticket 082). If 082 shows recovery-loop pathology, reconsider — but reframe with the lens first. Possibly close-and-replace with a "fallback DSE substrate" ticket. 088 is the substrate twin and is the cleaner primitive.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Scope

- New step resolver `src/steps/disposition/rest_in_place.rs` — no-target Sleep that consumes the cat's current tile, ignoring quality / shelter preferences. Follows the §5 step-resolver contract (5 rustdoc headings, `record_if_witnessed` shape).
- New step resolver `src/steps/disposition/eat_inventory_unconditional.rs` — eats from inventory ignoring food-type preferences. Follows the same contract.
- New `LastResortPromotion` modifier in `src/ai/modifier.rs`.
- Trigger: sensor `RecentTargetFailures.count((Resting | Eating, _)) >= last_resort_failure_threshold` via `plan_substrate::RECOVERY_FAILURE_COUNT_INPUT`.
- Effect: additive lift `last_resort_score_lift` on `RestInPlace` / `EatInventoryUnconditional` action scores — promotes them above target-requiring recovery actions.
- New `PlanningSubstrateConstants` knobs: `last_resort_failure_threshold: u32` (default 3), `last_resort_score_lift: f32` (default sized to overcome standard Resting/Eating scores).

## Out of scope

- Replacing the `AnxietyInterrupt` hard-preempt path — that stays as-is. This ticket adds a *score-economy* escalation, not a new preempt.
- Last-resort actions for non-Maslow-physiological needs (no last-resort socializing, mating, etc.). Only physiological recovery has a survival imperative.
- Tuning the threshold or lift size — pick conservative defaults; tune via post-landing soak.

## Approach

Files:

- `src/steps/disposition/rest_in_place.rs` — new. Model after `src/steps/disposition/sleep.rs` (or whichever existing rest step is closest). Real-world effect: increment energy on the cat's current tile, ignoring shelter / quality. The five required rustdoc headings from CLAUDE.md §"GOAP Step Resolver Contract" (Real-world effect / Plan-level preconditions / Runtime preconditions / Witness / Feature emission).
- `src/steps/disposition/eat_inventory_unconditional.rs` — new. Model after the existing `eat` step. Real-world effect: consume any food in inventory, restoring hunger. Bypasses food-type preference checks the normal `eat` resolver enforces.
- `src/ai/modifier.rs` — add `LastResortPromotion` modifier struct + `new(sc)` constructor. Reads `RECOVERY_FAILURE_COUNT_INPUT` sensor; applies additive lift to the two new actions when the trigger fires.
- `src/ai/scoring.rs::EvalInputs` — publish the `recovery_failure_count` sensor: counts entries in `RecentTargetFailures` matching `(Resting | Eating, _)` whose tick-age is within `target_failure_cooldown_ticks`.
- `src/resources/sim_constants.rs::PlanningSubstrateConstants` — add `last_resort_failure_threshold: u32` and `last_resort_score_lift: f32`.
- Modifier registration site — register `LastResortPromotion::new(sc)` alongside `CommitmentTenure::new(sc)` (ticket 075) and the existing modifiers.
- New `Feature::*` variants for the two no-target actions (mirror existing eat / sleep features). Exempt from `expected_to_fire_per_soak()` until soak data confirms.

## Verification

- `just check && just test` green.
- Unit test: `LastResortPromotion::compute` returns the configured lift when `recovery_failure_count >= threshold`, 0.0 otherwise.
- Unit test on each new step resolver: real-world effect observed (energy / hunger increment); witness recorded; feature fires only on success.
- Synthetic-world integration test: a cat that fails Resting 3 times then has anxiety fire → next plan picks `RestInPlace` over targeted Resting. Without the modifier, the same setup loops Resting failures until starvation (regression-test framing).
- `just soak 42 && just verdict logs/tuned-42-076` — hard gates pass. Expect `Feature::TargetCooldownApplied` and the new last-resort features to fire non-zero counts on the seed-42 stuck-pattern recovery.

## Log

- 2026-04-29: Opened under sub-epic 071.
- 2026-04-29: Parked. With 073 (cooldown) + 074 (require_alive) + 078 (Pairing Intention Consideration) shipped, the originating mate-selection failure that triggered the seed-42 Nettle/Mocha/Lark cascade should not occur — making this ticket's escalation path moot for the failure mode it was designed to catch. The IAUS-engine score-lift Modifier itself is straightforward, but the two new no-target step resolvers (`rest_in_place`, `eat_inventory_unconditional`) require full §5 contract compliance and DSE registration — substantial new-feature work that's high-risk to ship without targeted soak validation. Decision: defer until Wave 4's 027b reactivation soak (ticket 082) demonstrates whether the post-Wave-2 substrate handles the originating cascade. If 082's soak shows recovery-loop pathology, unpark this ticket and ship the full implementation. If 082 passes hard gates, this becomes follow-on hardening at lower priority.
- 2026-05-01: **Retired without implementation.** The substrate-over-override lens (epic 093) reframed this ticket: the *reactive* "lift on failure-count" shape is itself override-shaped, while the *proactive* "lift on body-distress scalar" shape that 088 actually shipped is the cleaner substrate primitive for the same problem class. 088's `BodyDistressPromotion` Modifier reads 087's `body_distress_composite` (composite of hunger / energy / thermal / health deficits) and lifts the six-DSE self-care class (Flee/Sleep/Eat/Hunt/Forage/GroomSelf) when distress crosses 0.7 — addresses the same "cat keeps failing recovery and dies" pathology this ticket targeted, but via continuous interoceptive perception rather than a per-failure counter. 094's `StockpileSatiation` damp + 088's lift compose to keep the score landscape honest under stockpile / distress states without the no-target step resolvers (`rest_in_place`, `eat_inventory_unconditional`) this ticket would have introduced. 123's `RecentDispositionFailures` cooldown (landed 2026-05-01) is the *failure-history* substrate axis the planner was lacking — it covers the per-cat memory dimension this ticket also gestured at, but at the disposition-scope rather than the action-scope, which fits the planner-elects-but-cannot-satisfy retry-storm shape that's the actual symptom. **Net:** 088 + 094 + 123 between them cover the "post-failure escalation" surface this ticket scoped, in substrate-doctrine-compliant shapes. No new no-target step resolvers needed; no `LastResortPromotion` Modifier needed. If a future failure mode surfaces that none of those three address, open a fresh ticket framed around the *specific* substrate gap rather than reviving this one.
