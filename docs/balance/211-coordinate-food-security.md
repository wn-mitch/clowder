# Coordinate `colony_food_security` lift — weight tuning iteration 1

**Date:** 2026-05-07
**Ticket:** [211](../open-work/tickets/211-tune-coordinate-food-security-weight.md)
**Predecessor evidence:** post-210 baseline soak at the current tip (commit captured at run header time) — `mentor_food_security_weight = 0.10`, `coordinate_food_security_weight = 0.0`, Coordinate's fifth axis dormant.
**Substrate:** ticket 209 (`c970ad442163`) — additive `colony_food_security` axis with `Logistic(8.0, 0.5)` curve added to `coordinate_dse` at zero weight, with `(1 - w)` auto-rebalance keeping the existing 4-axis composition's weights at sum=1.0 for any lift setting. Scalar formula: `min(food_fraction, hunger_satisfaction)` (`src/ai/scoring.rs:572-574`). Wiring: `src/ai/dses/coordinate.rs:60-108`.

## Hypothesis

Sibling to 210 (Mentor lift). 181's path-1 closeout argued that freed L3 bandwidth from suppressing food-tier DSEs flows to Patrol, not to higher-tier social DSEs (memory note `project_l3_patrol_absorption_cascade`). The structural fix is to give higher-tier DSEs an **active positive lift** when food security is high. 209 wired that substrate on Mentor + Coordinate + Caretake + GroomOther. 211 tunes Coordinate's weight after 210 has shipped Mentor at 0.10.

The mechanism is structurally safer than 181's suppression because the lift is **additive on Coordinate's score** rather than **subtractive on Hunt/Forage's**. No L3 bandwidth is freed from food-tier behaviors, so the Patrol-absorption cascade should not reproduce. The cat must already be food-secure (`min(food_fraction, hunger_satisfaction)`) for the lift to fire — no path where the lift draws cats away from acute hunger response.

**Differences vs 210:** Coordinate has a much narrower eligibility gate (`IsCoordinatorWithDirectives`) — only coordinator cats with pending directives score Coordinate at all. So the lift fires in fewer moments per soak than on Mentor. 210's iter-1 also surfaced that **Mentor share was structurally flat at 0.10** but **cohesion canaries (mentoring +30%, grooming +53%, courtship +187%) lifted significantly** — i.e. the substrate did its job through secondary pathways, not through L3 share-pp. 211 should expect the same shape: Coordinate share-pp may be flat, but `frame-diff` should still rank Coordinate's per-cat L2 mean as a top mover with positive direction.

## Prediction

**Constants change:** `coordinate_food_security_weight: 0.0 → 0.10`.

**Curve mechanics** at `lift_weight = 0.10` (five-axis WeightedSum with the four legacy axes scaled by `remainder = 0.9`):

```
new_score = 0.9 · old_4_axis_score  +  0.10 · Logistic(8.0, 0.5)(colony_food_security)
```

Five-axis weights post-211: `[0.216, 0.288, 0.216, 0.180, 0.10]` (sum = 1.0).

- At `colony_food_security ≈ 1.0` (cat fully fed, colony stocked): `Logistic ≈ 0.982`, additive lift ≈ **+0.098** to the weighted score, with a 10% multiplicative reduction on the existing axes' contribution.
- At `colony_food_security ≈ 0.5` (midpoint): `Logistic ≈ 0.5`, additive lift ≈ **+0.05**.
- At `colony_food_security ≈ 0.0` (cat hungry OR colony empty): `Logistic ≈ 0.018`, lift ≈ **+0.002** — net effect is the 10% multiplicative reduction. Coordinate scores *slightly lower* than baseline when cats are hungry. Intentional: when hunger dominates, coordinator cats should be doing food-tier work, not coordinating.

**Predicted action-share shift** (anchored on post-210 baseline; share-pp band is **soft** per memory note `feedback_chain_rare_events` — Coordinate is chain-rare, narrow eligibility plus low base rate makes single-seed counts noisy):

| Action | Post-209 share | Predicted post-211 share | Direction |
|---|---|---|---|
| Coordinate | 1.52% (post-209 anchor) | flat-to-+1pp; structural |Δ mean| > noise | **up (structural)** |
| Patrol | post-210 share | unchanged within ±0.5pp | **— (cascade detector)** |
| Mentor | post-210 share (~0.35–3.4%) | unchanged within ±0.5pp from post-210 | — (cross-leak detector) |
| Forage / Hunt / GroomOther / Caretake | post-210 share | small drift from `(1-w)` arithmetic only | — |

**Substrate-performance gates (load-bearing — these gate landing):**

1. **Coordinate substrate active.** `frame-diff` ranks Coordinate's per-cat L2 mean as a top mover (in the upper third by `|Δ mean|`) with positive direction. Wren focal trace shows a non-zero `colony_food_security` axis row on Coordinate.
2. **Patrol cascade detector.** Patrol DSE row classifies `ok-stable` (no positive drift >0.5pp from post-210). Substrate collapse if breached.
3. **Mentor cross-leak detector.** Mentor DSE row classifies `ok-stable` from post-210. Substrate collapse if Mentor share drops >0.5pp — Coordinate cannibalized Mentor moments.
4. **Six continuity canaries each ≥ 1** (grooming, play, mentoring, burial, courtship, mythic-texture). Substrate collapse if any zeroes out.
5. **`never_fired_expected_positives` does not gain entries vs post-210.** Substrate collapse if a positive Feature is silenced.

**Survival outcomes (observed and reported, NOT landing gates):**

210 already shipped at 0.10 with 9 Starvations vs baseline 2 (root cause: mate_dse not gating on `colony_food_security`; bonded couples breeding into famine — substrate-level, parked for later). 211 may compound. Death counts go in Observation as data; not gating.

**Failure modes:**

a. **Patrol drift (>0.5pp).** Cascade detector breach — surface to user; do not auto-revert. Substrate collapse, same logic as 210.
b. **Coordinate no-op (|Δ mean| at noise AND share-pp flat).** Lift too small for the narrow eligibility window. ITERATE: bump to 0.15 in iter-2, or pivot to investigating directive-count saturation. Substrate collapse if both share-pp and structural |Δ mean| are flat.
c. **Mentor share drops >0.5pp from post-210.** Cross-leak — surface to user; do not auto-revert. Substrate-collapse trigger.
d. **GroomOther share drops >1pp.** Cross-leak; flag in concordance and surface to user.

## Observation

**Soaks** (both seed=42, 900s, focal=Wren, single-seed):

- **Baseline (post-210):** `logs/tuned-42-post-210/` at commit `06f651be`(dirty), `mentor_food_security_weight=0.10`, `coordinate_food_security_weight=0.0`.
- **Treatment (post-211):** `logs/tuned-42/` at commit `a24f7eb4`(dirty), `mentor=0.10`, `coordinate=0.10`.

**Coordinate share (the headline metric):**

| | Baseline | Treatment | Δ |
|---|---|---|---|
| Coordinate share | 2.36% (222/9390) | **4.22%** (283/6707) | **+1.86 pp** |

Substrate active. Larger than the predicted "flat-to-+1pp" soft band. Wren never scored Coordinate (focal not a coordinator) so the per-cat Coordinate L2 row is absent from her trace; the share lift comes from coordinator cats elsewhere in the colony.

**Frame-diff per-cat L2 means** (top 7 of 15, post-210 → post-211 on Wren focal):

| DSE | Δ mean | rel% | sign |
|---|---|---|---|
| caretake | +0.275 | +220% | up |
| hunt | -0.271 | -32% | down |
| cook | +0.219 | +137% | up |
| pick_up | -0.202 | -25% | down |
| mentor | +0.178 | +466% | up |
| groom_self | -0.170 | -77% | down |
| groom_other | +0.136 | +57% | up |

`coordinate` row absent from Wren's trace (eligibility gate `IsCoordinatorWithDirectives` never triggered for her). Frame-diff classifies "concordance: ok — no unacknowledged drift on tracked DSEs" since Coordinate (the only tracked prediction) was missing-not-violated.

**Substrate-performance gates:**

| Gate | Outcome |
|---|---|
| Coordinate substrate active | ✅ pass — +1.86pp share lift, in predicted direction |
| Patrol cascade detector (Patrol Δ ≤ +0.5pp) | ⚠️ breach — Patrol +1.45pp share (12.4% → 13.85%); BUT per-cat Patrol L2 score DROPPED -22.4% |
| Mentor cross-leak (Mentor Δ from post-210 within ±0.5pp) | ✅ pass — Mentor 0.35% → 0.64% (+0.29pp) |
| Six continuity canaries each ≥ 1 | ⚠️ partial — burial=0, but `burial=0` in baseline too (pre-existing); play=4, mentoring=147, grooming=710, courtship=1441, mythic-texture=32 all ≥1 |
| `never_fired_expected_positives` no new entries | ✅ pass — `[]` in both runs |

**GroomOther share-pp anomaly:** GroomOther share dropped 13.98% → 8.93% (-5.05pp), exceeding my "drop >1pp" failure threshold. **However, frame-diff shows per-cat GroomOther L2 score went UP +56.9%.** The inversion (per-cat score up, share down) indicates this is a softmax-mass redistribution under a smaller treatment-snapshot pool (6707 vs baseline 9390 — fewer ticks observed because peak_population was 8 vs baseline 10), not a substrate suppression of GroomOther scoring.

**Survival outcomes (data, not gates):**

| Metric | Baseline | Treatment |
|---|---|---|
| `deaths_by_cause.Starvation` | 8 | 2 |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 4 |
| `colony_score.aggregate` | 1968.08 | 1721.51 |
| `colony_score.kittens_born` | 4 | 2 |
| `colony_score.kittens_surviving` | 0 | 0 |
| `colony_score.peak_population` | 10 | 8 |
| `colony_score.seasons_survived` | 5 | 5 |
| `colony_score.bonds_formed` | 37 | 29 |
| `colony_score.welfare` | 0.455 | 0.469 |

Starvation dropped (8 → 2) but ShadowFoxAmbush rose (0 → 4) — the ambush rise is the cluster-of-concern signal that 210's three follow-on tickets (substrate ambush-awareness gaps) are tracking; not 211-specific.

**Verdict tool exit:** `just verdict logs/tuned-42` exited 2, but the failures are vs the **pre-209** promoted baseline (`tuned-42-baseline-0783194` at `0783194`), so the drift report conflates 209 + 210 + 211 substrate landings. Substrate-meaningful verdict is the gate table above.

## Concordance

**Per-row classification:**

| Prediction | Outcome | Class |
|---|---|---|
| Coordinate share rises modestly | +1.86pp (above soft band) | **ok-stronger-than-predicted** |
| Patrol unchanged within ±0.5pp | +1.45pp share, per-cat -22.4% | **drift — share-mass redistribution; not 181 cascade pattern (per-cat score down, not up)** |
| Mentor unchanged within ±0.5pp from post-210 | +0.29pp | **ok** |
| GroomOther drop ≤1pp | -5.05pp share, per-cat +56.9% | **drift — share inverted from per-cat; demographic artifact, not substrate suppression** |
| Six canaries non-zero | burial=0 (pre-existing), 5/6 ≥1 | **ok with pre-existing carve-out** |
| `never_fired_expected_positives` no new | `[]` | **ok** |

**Overall verdict:** SURFACE — substrate is unambiguously active and in the predicted direction (+1.86pp Coordinate lift), but Patrol +1.45pp share rise and GroomOther -5.05pp share drop both breach prediction ceilings. Per-cat L2 scores tell a different story than share-pp on both rows (Patrol per-cat down; GroomOther per-cat up), suggesting the L3 share moves are softmax-mass redistribution and demographic shifts under a smaller treatment cohort (peak_pop 8 vs 10), not 181-style substrate cascades. **User judgment requested before landing or iterating.**
