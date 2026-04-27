# Hangry curve recalibration — Logistic(8, 0.75) → Logistic(8, 0.5)

**Date:** 2026-04-27
**Ticket:** [044](../open-work/tickets/044-hangry-curve-recalibration.md)
**Commit (lands with):** _(this commit)_
**Predecessor evidence:** `logs/collapse-probe-42/` (1-hour collapse-probe soak, 17 in-game years, full extinction).

## Hypothesis

Cats die of starvation with food in stores because the `Eat` DSE only becomes competitive at very low hunger. The hangry anchor `Logistic(steepness=8, midpoint=0.75)` on `hunger_urgency = 1 - needs.hunger` sits near zero across most of the hunger range:

| hunger | urgency | old score (mid=0.75) |
|---|---|---|
| 0.9 (sated) | 0.1 | 0.005 |
| 0.7 (lightly hungry) | 0.3 | 0.012 |
| 0.5 (half-hungry) | 0.5 | 0.018 |
| 0.4 (hungry) | 0.6 | 0.119 |
| 0.3 (very hungry) | 0.7 | 0.500 |
| 0.1 (starving) | 0.9 | 0.769 |

The window in which Eat outscores its competitors (Groom, Socialize, Sleep — typically ~0.3–0.6) is hunger ∈ [0, 0.3]. Any path failure, commitment lock, or competing-need spike during that narrow window starves the cat. Real cats do not eat this way — they nibble continuously, scaling food-seeking with hunger gradient, not threshold.

## Prediction

Recalibrate to `Logistic(steepness=8, midpoint=0.5)`. Same family (drop-in for the substrate's named-anchor mechanism), same steepness (preserves "decisive once threshold crossed"), midpoint shifts from "very hungry" to "half-hungry."

| hunger | urgency | new score (mid=0.5) | Δ |
|---|---|---|---|
| 0.9 (sated) | 0.1 | 0.039 | +0.034 |
| 0.7 (lightly hungry) | 0.3 | 0.168 | +0.156 |
| 0.5 (half-hungry) | 0.5 | 0.500 | +0.482 |
| 0.4 (hungry) | 0.6 | 0.690 | +0.571 |
| 0.3 (very hungry) | 0.7 | 0.832 | +0.332 |
| 0.1 (starving) | 0.9 | 0.961 | +0.192 |

Predicted directional effects:

- **`deaths_by_cause.Starvation`:** sharp drop, ideally from 4 → 0 on the seed-42 collapse probe; from ~1.2±1.7 → near-zero on the 15-min healthy-colony baseline. Direct prediction.
- **`positive_features_active.FoodEaten`:** rises (cats top up at half-hungry instead of waiting for very hungry).
- **`continuity_tallies.courtship` / `MatingOccurred`:** *may* rise. Mating gate is `breeding_hunger_floor=0.6`. Under the old curve, hunger oscillates ~0.3↔0.7 (cat eats only when very hungry, then post-meal recovers); about half the time hunger < 0.6 and mating fails. Under the new curve, hunger oscillates ~0.5↔0.7 (cat tops up at half-hungry); cat spends more time gate-eligible.
- **Hunt / Forage cadence:** rises (same anchor). Wildlife predation likely increases marginally; previously near-static prey populations may show genuine ecological cycling.
- **Fox `Hunting` / `Raiding`:** foxes also nibble-vs-panic-hunt; expect more frequent low-intensity fox activity and less crisis-mode spikes.

## Observation

_Pending verification soak. Will populate after `cargo run --release -- --headless --seed 42 --duration 3600` against the post-fix tree._

## Concordance

_Pending observation — will mark direction-match and magnitude-band per metric._

## Out of scope

- Spec-doc references in `docs/systems/ai-substrate-refactor.md` (§2.3 calibration table) still show 0.75 in 9 places. The code is the source of truth; spec sweep tracked as ticket 044's follow-on.
- The `breeding_hunger_floor=0.6` gate itself (ticket 032 item #3 proposes lowering to 0.4). Independent change — wait for this one to land before re-evaluating.
- The `if needs.hunger == 0.0` starvation cliff (ticket 032 item #1). Independent — softening the cliff still matters for edge cases that bypass the eat-loop entirely (e.g. an injury immobilizes a sated cat for days).
