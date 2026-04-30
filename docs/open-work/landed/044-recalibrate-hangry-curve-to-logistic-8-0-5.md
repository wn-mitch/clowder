---
id: 044
title: "Recalibrate hangry curve to Logistic(8, 0.5)"
status: done
cluster: null
landed-at: c8f8652
landed-on: 2026-04-27
---

# Recalibrate hangry curve to Logistic(8, 0.5)

**Landed:** 2026-04-27 | **Commit:** `c8f8652`

**Why:** Post-043 1-hour collapse-probe (`logs/collapse-probe-42-fix-043-044/`) still showed cats dying of starvation with food in stores. Three of four starvation victims (Ivy/Simba/Lark) were re-planning normally — Eat just never won. Snapshot evidence at tick 1,250,500: Ivy at hunger=0.61 had Eat=0.046, ranked 10th of 12; Lark at hunger=0.58 had Eat=0.105, ranked 7th. The hangry anchor `Logistic(steepness=8, midpoint=0.75)` returns ~0.05 at urgency=0.4 (hunger=0.6) and only crosses 0.5 at urgency=0.75 (hunger=0.25). The window between "Eat starts winning" and "starvation cliff" was so narrow that any plan-failure during it starved the cat.

**Root cause:** Curve calibration. The "threshold not ramp" intent (per §2.3 spec) set the threshold at "very hungry" rather than "half-hungry," producing a near-cliff hunger response that didn't match real-cat eat-cycle behavior. Real cats nibble continuously; the colony's cats waited until emergency hunger before considering food, and any path failure or competing-need spike during the narrow eating window proved fatal.

**Fix:** One-line change to `src/ai/curves.rs::hangry()`: midpoint `0.75 → 0.5`. Steepness stays at 8 (preserves decisive selection once threshold crossed). Family stays Logistic — the named-anchor mechanism (introduced by the AI substrate refactor specifically for this kind of single-curve swap) propagates the change to every consumer (Eat, Hunt, Forage, fox Hunting, fox Raiding) without per-call edits.

| hunger | urgency | old score (mid=0.75) | new score (mid=0.5) |
|---|---|---|---|
| 0.9 (sated) | 0.1 | 0.005 | 0.039 |
| 0.5 (half-hungry) | 0.5 | 0.018 | 0.500 |
| 0.4 (hungry) | 0.6 | 0.119 | 0.690 |
| 0.1 (starving) | 0.9 | 0.769 | 0.961 |

Tests updated in `curves.rs`, `considerations.rs`, `eat.rs`, and `scoring.rs` (the `diligent_cat_prefers_forage_over_hunt` test now uses hunger=0.5 — at high urgency the new curve saturates the hunger axis on both DSEs, drowning the personality differentiator). Spec-trace doc-comments updated in scoring.rs / eval.rs / considerations.rs / trace_log.rs. Spec doc (`docs/systems/ai-substrate-refactor.md`) still references 0.75 in 9 places — code is source of truth; spec sweep deferred.

**Verification:** All 1344 release tests pass; `just check` (fmt + clippy + step-resolver + time-unit linters) passes. 1-hour collapse probe at seed 42 (`logs/collapse-probe-42-fix-043-044/`) drops Starvation 4 → 2, CriticalHealth interrupts 4,723 → 1,429 (mostly from ticket 043), and `ward_siege_started_total` 0 → 310 (magic register fully alive). Reproduction still silent — separate bottleneck (ticket 027), and a year-15 wildlife-combat cluster surfaces as a new failure mode (tickets 045 / 046 / 047).

**Hypothesis / prediction / observation / concordance:** Balance change. Hypothesis = "Eat curve threshold too high, narrow eating window starves cats." Prediction = Starvation drops, FoodEaten rises, hunger oscillates closer to [0.5, 0.7]. Observation = Starvation 4 → 2 (50% drop), expected positive feature breadth 24/45 → 25/45, ward register fully active. Concordance = direction match on Starvation; magnitude in band; no survival regression (ShadowFoxAmbush 2 = 2). Full balance doc: `docs/balance/hangry-recalibration.md`.

---
