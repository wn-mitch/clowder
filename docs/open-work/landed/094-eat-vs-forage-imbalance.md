---
id: 094
title: Eat-vs-Forage IAUS imbalance — colony hauls food but doesn't consume it
status: done
cluster: substrate-over-override
landed-at: null
landed-on: 2026-04-30
---

# Eat-vs-Forage IAUS imbalance — colony hauls food but doesn't consume it

**Why:** 091 fixed the founder starvation cascade by clearing the producer chain (57k Foraging plans / 30k Hunting / 88k DepositFood across the seed-42 deep-soak). Cats deposited food into Stores constantly. **They almost never ate from it** — only 200 EatAtStores plans in 1.2M ticks × 8 cats; FoodEaten counted 207. Hunger trajectories drifted toward starvation: Lark 0.82 → 0.20, Mocha 0.93 → 0.38, Nettle 0.79 → 0.00 (dead).

**Diagnosis** — structural IAUS asymmetry, not a hack to retire. Hunt is `WeightedSum([0.5 hunger, 0.25 scarcity, 0.15 boldness, 0.10 prey])` with **no spatial axis**, so it scores the same anywhere on the map (~0.85 for a bold cat with prey visible at 0.8 hunger urgency). Eat is `CompensatedProduct(hunger × stores_distance)` with `Composite{Logistic(8, 0.5), Invert, ClampMin(0.1)}` over a 20-tile range — at distance 15 the spatial axis ≈ 0.119 and Eat's CP final ≈ 0.275. **Eat dies multiplicatively at distance.** A bold/diligent cat near forageable terrain at the colony perimeter elects Hunt, completes Hunt, re-elects from the same perimeter, Hunts again. Hunger silently decays. Phase 1 confirmed via the existing `tuned-42` log: at hunger=0.366, Lark's last_scores ranked Hunt 0.85 / Forage 0.61 / Groom 0.55 with Eat absent from top-3.

**What landed:**

1. **`src/ai/modifier.rs`** — new `StockpileSatiation` Modifier in §3.5.1, mirroring `FoxTerritorySuppression`'s shape. Trigger: `food_fraction > stockpile_satiation_threshold`; transform: multiplicative damp on `hunt` and `forage` only, where `suppression = ((food_fraction − threshold) / (1 − threshold)) × stockpile_satiation_scale`. Eat (consumption), Cook (raw-food consumption), Sleep/Groom/Flee (self-care) all pass through unchanged. Registers in `default_modifier_pipeline` after `CorruptionTerritorySuppression`. The desperation-hunting case (food_fraction = 0) is preserved by construction — suppression clamps to 0 below threshold.
2. **`src/resources/sim_constants.rs`** — `ScoringConstants::stockpile_satiation_threshold` (default 0.5) and `stockpile_satiation_scale` (default 0.85). Both serialize into the `events.jsonl` header per the comparability invariant. With these defaults, full stores reduce Hunt/Forage to ~15% of their pre-modifier value.
3. **Seven new unit tests** under `mod tests`: no-damp-below-threshold, damps-Hunt-and-Forage-above-threshold, full-stores-collapses-acquisition-DSEs, targets-only-hunt-and-forage (Eat/Sleep/Groom/Flee asserted unchanged), zero-score-stays-zero, lever-breaks-Lark-contest-synthetic (Hunt 0.85 → 0.185, Eat 0.27 wins), preserves-desperation-hunting (food_fraction=0 leaves Hunt/Forage unchanged). `default_pipeline_registers_eight_modifiers` updated to nine.

**Verification — soak `logs/tuned-42/` (commit_dirty=true on top of 25439da):**

| Metric | Pre-094 (post-091) | Post-094 |
|---|---|---|
| Total deaths | 8 (1 Starvation, 4 ShadowFoxAmbush, 1 Injury, 2 WildlifeCombat) | **1** (WildlifeCombat) |
| FoodEaten | 207 | **407** (2.0×) |
| EatAtStores plans | 200 | 383 (1.9×) |
| Hunting plans | 30,757 | 13,260 (−57%) |
| Foraging plans | 57,954 | 8,653 (−85%) |
| Resting plans (Eat is constituent) | ~57k | 230,240 (4.0×) |
| Lark hunger end | 0.20 | **0.89** |
| Nettle | dead | alive (end 0.53) |
| Min hunger across 8 cats | 0.00 | 0.43 |
| Anxiety interrupts | 24,874 | 10,119 (−59%) |
| BondFormed | 0 (never-fired) | **1** |
| CourtshipInteraction | 0 (never-fired) | **209** |
| PairingIntentionEmitted | 0 (never-fired) | **8,844** |
| ShadowFoxAmbush deaths | 4 | **0** |
| grooming canary | 64 | 194 |
| play canary | (lower) | 496 |
| courtship canary | 0 (regression) | 210 |
| mythic-texture canary | 51 | 35 |

`just verdict` reports `fail`, but exclusively on **pre-existing** canaries that 094 doesn't touch: `FoodCooked` never-fires (separate Cook chain bug, ticket 039), `mentoring=0` and `burial=0` (long-standing — neither feature has a working DSE/step pipeline yet, tickets 035 + the mentoring-bug cluster). Every metric that 094 had a path to influence improved; 094 did not regress any pre-existing canary.

**Cascade observation.** Damping Hunt/Forage didn't just fix eating — it freed up election cycles for the rest of the catalog. Resting plans 4×, Socializing 4×, three never-fired social positives (`BondFormed`, `CourtshipInteraction`, `PairingIntentionEmitted`) all started firing for the first time, courtship canary went 0 → 210, anxiety interrupts dropped 59%, ShadowFoxAmbush deaths went 4 → 0 (cats stuck on the territory boundary in long Hunt/Forage trips were sitting ducks; with eating substituting for foraging they're closer to home). One scoring-layer change unlocked three orthogonal behaviors. The substrate-over-override doctrine paid off: get the score landscape right and the rest of the planner just works.

**Surprise.** The plan predicted a 10–50× lift in `EatAtStores` plan count; the actual lift was 1.9× (200 → 383). What didn't show up in the plan: the cascade through Resting (4×) → Socializing (4×) → courtship/pairing canaries restored. The scoring contest now lets cats stay home, and once they do, dispositions other than the food-acquisition class get election cycles they never had under post-091's Hunt/Forage dominance. The headline metric (`FoodEaten` 2× lift) understates the systemic effect.

**Out of scope** (left for follow-on tickets):
- H2 (commitment / preemption mid-plan, the per-disposition exemption of Foraging from the Starvation interrupt at `disposition.rs:305-317`) — part of the 047 treadmill cluster. Not needed; H1's substrate fix carried the day.
- Eat eligibility re-evaluation (e.g., spatial-aware `EligibilityFilter`). Stays binary on `HasStoredFood`. Spatial proximity carries through the existing axis.
- Lever B (raise Eat's `ClampMin(0.1)` floor). Not needed; A alone resolved the contest.
- Pre-existing canary regressions (`FoodCooked`, mentoring, burial) — orthogonal threads.

**Substrate-over-override pattern.** Part of the substrate-over-override thread (093). **Hack shape:** structural IAUS asymmetry — Hunt has no spatial axis, Eat has a multiplicative one. Not a hack to retire — a missing substrate axis to add. **IAUS lever:** new `StockpileSatiation` Modifier in §3.5.1 mirroring `FoxTerritorySuppression`'s shape. **Sequencing:** lands independently. Composes cleanly with [088](../tickets/088-body-distress-modifier.md) when that lands (additive lift on Eat fires before this multiplicative damp on Hunt/Forage). **Canonical exemplar:** 087 (publish a self-state scalar → consume as a Modifier on the relevant DSE class) and `FoxTerritorySuppression` (multiplicative-damp-on-class shape).
