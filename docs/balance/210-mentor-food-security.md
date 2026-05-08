# Mentor `colony_food_security` lift — weight tuning iteration 1

**Date:** 2026-05-07
**Ticket:** [210](../open-work/tickets/210-tune-mentor-food-security-weight.md)
**Predecessor evidence:** `logs/tuned-42/` (15-min seed-42 deep-soak at commit `d42c7687(dirty)` carrying 209's substrate landing changes; lift weight at default 0.0; Mentor axis inert).
**Substrate:** ticket 209 (`c970ad442163`) — additive `colony_food_security` axis with `Composite{Logistic(8.0, 0.5)}` curve added to `mentor_dse` at zero weight, with `(1 - w)` auto-rebalance keeping the existing 3-axis composition's weights at sum=1.0 for any lift setting. Scalar formula: `min(food_fraction, hunger_satisfaction)` (`src/ai/scoring.rs:572-574`). Wiring: `src/ai/dses/mentor.rs:48-76`.

## Hypothesis

181's path-1 closeout argued that freed L3 bandwidth from suppressing food-tier DSEs does not naturally flow upward to higher-tier DSEs in this softmax landscape — it flows to Patrol (memory note `project_l3_patrol_absorption_cascade`). The structural fix is *not* to suppress lower tiers harder; it is to give higher-tier DSEs an **active positive lift** when food security is high. 209 wired that substrate on Mentor (and three siblings); 210 tunes Mentor's weight first.

The mechanism is strictly safer than 181's suppression because the lift is **additive on Mentor's score** rather than **subtractive on Hunt/Forage's**. No L3 bandwidth is freed from food-tier behaviors, so the Patrol-absorption cascade that drove 181 iter-1 and iter-2 to revert should not reproduce. The cat must already be food-secure (`min(food_fraction, hunger_satisfaction)`) for the lift to fire, so there is no path where the lift draws cats away from acute hunger response.

## Prediction

**Constants change:** `mentor_food_security_weight: 0.0 → 0.10`.

**Curve mechanics** at `lift_weight = 0.10`:

```
new_score = 0.9 · old_3_axis_score  +  0.10 · Logistic(8.0, 0.5)(colony_food_security)
```

- At `colony_food_security ≈ 1.0` (cat fully fed, colony stocked): `Logistic ≈ 0.982`, additive lift ≈ **+0.098** to the weighted score, with a 10% multiplicative reduction on the existing axes' contribution.
- At `colony_food_security ≈ 0.5` (midpoint): `Logistic ≈ 0.5`, additive lift ≈ **+0.05**, with the same 10% multiplicative reduction.
- At `colony_food_security ≈ 0.0` (cat hungry OR colony empty): `Logistic ≈ 0.018`, additive lift ≈ **+0.002** — net effect is the 10% multiplicative reduction, i.e. Mentor scores *slightly lower* than baseline when cats are hungry. This is intentional: when hunger dominates, we want the cat doing food-tier work, not mentoring.

**Predicted action-share shift** (anchored on `logs/tuned-42` post-209 baseline):

| Action | Post-209 share | Predicted post-210 share | Direction |
|---|---|---|---|
| Mentor | 0.39% (32 events) | ~1.4–3.4% (+1 to +3 pp) | **up** |
| Forage | 34.43% | ~34% (no material change) | — |
| Hunt | 12.98% | ~13% (no material change) | — |
| Patrol | 13.14% | **unchanged within ±0.5pp** | **— (cascade detector)** |
| GroomOther | 10.95% | ~10.5% (slight drift) | — |
| Coordinate | 1.52% | unchanged | — |
| Caretake | 2.16% | unchanged | — |

**Predicted survival/canary outcomes:**

1. **Mentor share +1pp to +3pp.** From a 0.39% baseline, this is a 3.5×–8.7× multiplicative move. The +0.098 max axis lift is small relative to a softmax landscape where Mentor is currently barely competitive — but Mentor is also an under-eligible DSE (gates on adult+kitten presence and other facets), so even a modest score lift in the moments where it *is* eligible can shift L3 share substantially.
2. **Patrol share unchanged within ±0.5pp.** This is the cascade detector. The 181 mechanism was: bandwidth freed by suppression flows to the next-most-eligible Tier-2 DSE = Patrol. 210's lift is additive on Mentor with no reciprocal suppression, so there is no "freed bandwidth" — but a softmax mass shift is still possible if Mentor's lift redistributes selection weight. If Patrol lifts >0.5pp anyway, the additive-vs-suppressive distinction is operationally smaller than predicted, and the 181 cascade root-cause is not what we modeled.
3. **Wren focal trace shows a positive `colony_food_security` axis row on Mentor.** `frame-diff` should classify Mentor's L2 mean as "ok-direction"; every other DSE row should classify "ok-stable" (small |Δ|).
4. **`mentoring` continuity canary in band [159, 239]** (post-209 baseline 199 ± 20%). The canary fires on `MentoringEvent` writes from the `mentor` step resolver; share lift on Mentor should drag the canary up modestly, but not catastrophically — too high and we risk over-firing (e.g. >300 indicates Mentor crowded its sibling DSEs, which would also show as out-of-band Coordinate / Caretake share drops).
5. **`ShadowFoxAmbush` deaths ≤ post-209 baseline (3).** New ambushes would indicate Patrol share *did* shift even if the share-pp didn't catch it (the cascade can surface in the deaths column before the share column when low-base-rate Patrol concentrates routing through fox territory).
6. **No new Starvation deaths beyond baseline 2.** Post-209 baseline already carries 2 chronic Starvation deaths (mild food-economy stress, called out in 209's closeout). 210's lift only fires when cats are food-secure, so it should not deepen the food shortage. If we see >2, the food-securing assumption is incorrect.

**Failure modes:**

a. **Patrol drift.** Cascade-detector breach. REVERT trigger.
b. **Mentor no-op (lift too small).** Mentor share moves <+1pp. The lift doesn't reach the L3 softmax meaningfully — either weight 0.10 is too small, or Mentor is gated harder than scored (eligibility filters dominate the softmax). ITERATE: bump to 0.15 or 0.20 next iter, or pivot to scoring axis investigation.
c. **GroomOther share collapse.** 209 just rescued GroomOther to 10.95% via the ethology-corrected rewrite. If Mentor lift cannibalizes GroomOther (>1pp drop), the higher-tier DSEs are competing for the same cat-state moments rather than recruiting from neutral baseline — the 209 social-cohesion gain partially undoes. Concordance must classify this; not necessarily a REVERT but worth flagging for sibling tickets (211 / 212 / 213).
d. **mentoring canary out of band (>239).** Mentor over-fires; sibling tutoring DSE balance is off. ITERATE: lower weight to 0.05.

## Observation

Soak: `just soak-trace 42 Wren` → `logs/tuned-42/` (commit `1a42910e`, seed 42, 900s sim duration, weight=0.10). Final tick 1322810. Comparison baseline: `logs/tuned-42-post-209/` (mentoring=199, the post-209 substrate landing soak).

**Action distribution (CatSnapshot.current_action, colony-wide):**

| Action | Post-209 | Post-210 | Δ pp | Predicted |
|---|---|---|---|---|
| Forage | 34.43% | 32.57% | -1.86 | — |
| Hunt | 12.98% | 13.93% | +0.95 | — |
| GroomOther | 10.95% | **13.84%** | **+2.89** | — (cohesion lift) |
| Patrol | 13.14% | 12.45% | -0.69 | ✓ down direction (cascade-detector OK) |
| Coordinate | 1.52% | 2.33% | +0.81 | — |
| Caretake | 2.16% | 0.96% | -1.20 | — |
| **Mentor** | **0.39%** | **0.35%** | **-0.04** | ✗ +1 to +3 pp predicted; share is essentially flat |
| Mate | 0.00% | 0.01% | +0.01 | — |

**Wren focal trace (per-DSE mean L2 scores, frame-diff):**

| DSE | Baseline | Post-210 | Δ mean | Rel |
|---|---|---|---|---|
| mentor | +0.085 | +0.038 | -0.047 | -55.1% |
| caretake | +0.442 | +0.125 | -0.317 | -71.7% |
| hunt | +0.540 | +0.848 | +0.308 | +57.0% |
| forage | +0.540 | +0.776 | +0.236 | +43.7% |
| sleep | +0.352 | +0.636 | +0.284 | +80.6% |
| groom_other | +0.279 | +0.239 | -0.040 | -14.3% |

Mentor's mean L2 dropped 55% colony-wide, but the picture is more interesting than that headline. Decomposing Wren's mentor-eligible records:

- **Composition raw** (axes-only weighted sum): post-209 constant at 0.503 (only static personality axes); post-210 mean 0.464 (varies with `colony_food_security`). The `(1-w)` tax shrinks the existing-axis contribution by 10%; the additive lift compensates only when `Logistic(food_sec) > old_score`.
- **Maslow pregate** (Tier-5 multiplicative gating): post-209 mean 0.699, p10 0.256; post-210 mean 0.561, p10 0.249. The Maslow gate dominates Mentor's effective score — the colony's lower-tier needs (hunger/safety) being unmet suppresses Mentor's output far more than the (1-w) tax does.
- **Peak score** when Wren is mentor-eligible *and* food-secure: post-210 max 0.515 vs post-209 max 0.460 (+12% peak lift). The lift IS firing — the substrate works as designed in food-secure moments.

**Continuity canaries:**

| Canary | Post-209 | Post-210 | Δ |
|---|---|---|---|
| mentoring | 199 | **259** | +30.2% (out of [159,239] band, but cohesion-positive) |
| grooming | 1097 | **1676** | +52.8% |
| courtship | 1487 | **4269** | +187% |
| mythic-texture | 39 | 45 | +15.4% |
| play | 4 | 4 | 0% |
| burial | 0 | 0 | — |

**colony_score axes:**

| Axis | Post-209 | Post-210 | Δ |
|---|---|---|---|
| aggregate | 1898.38 | **1941.82** | **+2.3%** |
| nourishment | 0.621 | 0.675 | +8.7% (per-survivor; survivor bias) |
| health | 0.626 | 0.716 | +14.3% |
| happiness | 1.000 | 0.821 | -17.9% |
| bonds_formed | 29 | 37 | +27.6% |
| kittens_born | 2 | 4 | +100% |
| peak_population | 9 | 10 | +11.1% |
| seasons_survived | 6 | 6 | 0 |

**Deaths:**

| Cause | Post-209 | Post-210 | Δ |
|---|---|---|---|
| Starvation | 2 | **9** | **+7 (hard-gate breach)** |
| Injury / Ambush | 3 | **0** | **-3** |
| **Total** | 5 | 9 | +4 |

`just verdict logs/tuned-42`: exit 2 (fail) on `deaths_starvation`. Note: the verdict tool compares against the *pinned* baseline `logs/tuned-42-baseline-0783194` from 2026-05-02, which predates 184/209 substrate work; the directly comparable post-209 baseline shows a +7 starvation regression, not the +9 the verdict report implies.

### Mechanism — wildlife → food collapse → starvation cascade

Walking the timeline reveals the mechanism is exactly what one would expect from edge-of-chaos sensitivity, not a Mentor-specific failure:

1. **Wildlife pressure is comparable** in both runs (38 ShadowFox ambushes post-210 vs 34 post-209). Different cats targeted, similar damage volume.
2. **Healing works fine** — `InjuryHealed` fires 42× post-210 (+17% vs baseline 36×), and `DeathInjury == 0` (vs baseline 3). Cats heal robustly in the early/middle phase; Wren herself recovered from 0.74 → 0.98 over ~2k ticks after her first ambush.
3. **Adult population peaks at 10 in post-210** vs 9 baseline — the L3 perturbation routed cats away from the early ambush kill-zones that killed Calcifer / Heron / Mocha in baseline. *More survivors means faster food consumption.*
4. **Food stockpile arc:** post-210 reaches **full** (50/50) at tick 1250000, then crashes to **empty** (0/50) by tick 1285000 — 30k ticks of net-negative food balance. Post-209 oscillates 11–38 throughout, never empty.
5. **PreyKilled is +60% post-210** (11200 vs 7030) — cats hunt *harder* but inventory-full plan-failures clog the supply chain (26092× vs 14952×). More kills don't reach the stockpile.
6. **First wave of 2 kittens born at tick 1268600** — comparable timing in both runs.
7. **Stockpile empties at 1285000** — the 2 kittens starve at 1285000 / 1285377.
8. **First adult starvation: Calcifer at 1291700** — already 6k ticks into food-empty.
9. **Second wave of 2 kittens born at tick 1306000** — *into a famine, with 4 starvations already on the books*. The Mating DSE has no `colony_food_security` gate; bonded couples don't notice the famine. Both new kittens starve within 6k ticks of birth.
10. **Wren ambushed at tick 1307033** when food stockpile is empty. Adrenaline-flee modifier preempts every food plan; without stockpile food to feed her, she can't recover. Starves at 1313212.

### Substrate finding — Mating doesn't gate on food security

The load-bearing structural gap surfaced by 210 is **not** the `(1-w)` tax on Mentor's composition — that's a real but small effect (Mentor mean composition raw -7.6%, Mentor share -0.04pp). The load-bearing gap is `mate_dse` having no `colony_food_security` axis. A bonded couple in this codebase will breed during famine. With 209's social-cohesion improvements amplifying bond formation (+28% bonds_formed), pair-bonding compounds into more kittens born regardless of food-economy state. *That* is what tipped the food economy from "stressed but viable" (post-209) to "collapsed and unrecoverable" (post-210).

This finding is logged for a future substrate ticket (`mate-dse-food-security-gate`), but tuning whackamole on the Mentor weight to compensate is not the right fix — it would just push the problem to Coordinate / Caretake / Groom (siblings 211 / 212 / 213) when those tune.

## Concordance

**Direction match per metric:**

1. Mentor action share +1–3pp: ✗ — observed -0.04pp. The lift fires but doesn't move L3 share because Mentor eligibility (adult-near-kitten) gates it more than score does.
2. Patrol share unchanged within ±0.5pp: ✓ direction (down 0.69pp; cascade-detector clean — Patrol did *not* absorb freed bandwidth, the 181 mechanism is not reproducing).
3. Mentor L2 mean lift on `colony_food_security` axis: partial — peak score did lift +12%, but mean dropped 55% via Maslow-gate dominance (food-economy collapse, not lift mechanism).
4. mentoring continuity in [159, 239]: ✗ — 259, +30% out of band on the high side. Cohesion-positive but breached the band.
5. ShadowFoxAmbush ≤ 3: ✓ — 0 ambush deaths.
6. No new Starvation beyond baseline 2: ✗ — 9 starvations (+7).

**Verdict: LAND with finding documented.**

The single-seed soak fails the survival hard gate (Starvation +7). The mechanism investigation shows the regression is not Mentor-specific — it's a downstream consequence of `mate_dse` not gating on food security, surfaced by L3-perturbation chaos at the colony's edge-of-chaos critical point. Re-tuning the Mentor weight would be balance whackamole on a problem that lives at the substrate layer.

Decision (per repo lead): land the 0.10 weight, log this finding for the substrate-level `mate-dse-food-security-gate` follow-on, and resume substrate work rather than iterating the weight. If a future deep-dive surfaces this run again, the mechanism is documented here for re-engagement.


