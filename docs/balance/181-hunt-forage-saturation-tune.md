# Hunt/Forage colony_food_security saturation — weight tuning iteration 1

**Date:** 2026-05-05
**Ticket:** [181](../open-work/tickets/181-hunt-forage-saturation-balance-tune.md)
**Predecessor evidence:** `logs/tuned-42-pre-181/` (15-min seed-42 deep-soak at commit `75586184`, both saturation weights at default 0.0; substrate inert).
**Substrate:** ticket 176 stage 5 (`75586184`) — `colony_food_security` axis with `Composite{Logistic(8.0, 0.5), Invert}` curve added to Hunt and Forage DSEs at zero weight, with `(1 - w)` auto-rebalance keeping RtEO sum at 1.0 for any weight setting. Scalar formula: `min(food_fraction, hunger_satisfaction)` (`src/ai/scoring.rs:530-539`).

## Hypothesis

L3 bandwidth in a healthy colony is captured by Maslow-tier-1 acquisition DSEs (Hunt + Forage), starving higher-tier DSEs (groom / mate / mentor / coordinate) of selection share. The `colony_food_security` saturation axis is the substrate-level mechanism for releasing that bandwidth: when food security is high, suppress food-seeking; when low, behave as before. Stage 5 wired the axis at zero weight; this iteration lifts it to a meaningful suppressor.

The two weights differ deliberately: Hunt's RtEO is `[0.5 hunger_urgency, 0.25 food_scarcity, 0.15 boldness, 0.10 prey_nearby]` while Forage's is `[0.24 hunger_urgency, 0.20 food_scarcity, 0.36 forage_nearby, 0.20 forage_value]`. Forage's spatial axis already loads 0.36 — equal saturation weight there overshoots the spatial signal. Asymmetric weights preserve each DSE's existing balance shape.

## Prediction

**Constants change:** `hunt_food_security_weight: 0.0 → 0.20`, `forage_food_security_weight: 0.0 → 0.15`.

**Curve mechanics:** at `colony_food_security ≈ 1.0` (well-fed), `Composite{Logistic(8, 0.5), Invert}` outputs ≈ 0.018 — saturation axis contribution is near zero, dragging the weighted score by a multiplicative factor of `(1 - w)` against the rest of the composition. Maximum-possible score reduction at full saturation: ~20% for Hunt, ~15% for Forage. At `colony_food_security < 0.5` the axis is permissive and the suppression is small.

**Predicted action-share shift** (anchored on `logs/tuned-42-pre-181`):

| Action | Pre-181 share | Predicted post-181 share | Direction |
|---|---|---|---|
| Forage | 50.65% | ~38% (-12 pp) | **down** |
| Hunt | 19.79% | ~14% (-6 pp) | **down** |
| GroomOther | 5.34% | ~9% (+4 pp) | up |
| Coordinate | 3.14% | ~5% (+2 pp) | up |
| Mentor | 0.54% | ~1.5% (+1 pp) | up |
| Mate | 0.00% | (gated by 182, no change expected) | — |

**Predicted survival/canary outcomes:**
- `deaths_by_cause.Starvation == 0` (hard gate). The new axis only suppresses *when food is secure*; cats in real hunger crisis still elect Hunt/Forage with full score weight.
- `deaths_by_cause.ShadowFoxAmbush <= 10` (hard gate, baseline 4 in pre-181).
- Continuity canaries `grooming` and `mentoring` ≥ 1 (baselines 286 and 121 — should rise modestly with the freed bandwidth).
- Continuity `courtship` and `burial` will likely remain at 0 — that's ticket 182's regression, independent of 181. **181 does not gate on those two canaries.**

**First-iteration risk acknowledgment:** the pre-181 soak shows the colony is chronically food-stressed (Forage at 50%, 717× "ForageItem: nothing found" plan-fails, late-run `FoodLevel` at zero). If `colony_food_security` averages well below 0.5 across the run, the saturation axis fires rarely and weight 0.20/0.15 may be near-inert. That outcome is itself a calibration finding — it tells us the curve placement is fine and weight needs to lift further (iteration 2: 0.30/0.25 or 0.40/0.30), not that the structural approach is wrong.

## Observation

Soak: `just soak-trace 42 Wren` → `logs/tuned-42-pre-184/` (commit `75586184` + the iteration-1 weight change, seed 42, 900s sim duration; renamed from `logs/tuned-42/` on 2026-05-06 when the post-184 soak superseded it).

**Action distribution shift (CatSnapshot.current_action, colony-wide):**

| Action | Pre-181 | Post-181 (w=0.20/0.15) | Δ pp | Predicted |
|---|---|---|---|---|
| Forage | 50.65% | 42.08% | **-8.57** | -12 (✓ direction) |
| Hunt | 19.79% | 22.43% | **+2.64** | -6 (**✗ wrong direction**) |
| Patrol | 10.04% | 25.03% | **+14.99** | (unanticipated) |
| GroomOther | 5.34% | 3.64% | -1.70 | +4 (✗) |
| Cook | 3.99% | 0.27% | -3.72 | (unanticipated) |
| Coordinate | 3.14% | 1.11% | -2.03 | +2 (✗) |
| Sleep | 2.69% | 2.87% | +0.18 | — |
| HerbcraftGather | 1.32% | 0.90% | -0.42 | — |
| MagicScry | 1.16% | 0.69% | -0.47 | — |
| Mentor | 0.54% | 0.15% | -0.39 | +1 (✗) |

**colony_score axes (apples-to-apples, same substrate commit):**

| Axis | Pre-181 | Post-181 | Δ |
|---|---|---|---|
| aggregate | 1232.58 | 958.37 | **-22.2%** |
| nourishment | 0.589 | **0.000** | **-100.0%** |
| welfare | 0.268 | 0.049 | -81.6% |
| health | 0.175 | 0.024 | -86.3% |
| happiness | 0.575 | 0.222 | -61.4% |
| seasons_survived | 4 | 2 | -50.0% |
| structures_built | 10 | 5 | -50.0% |
| bonds_formed | 10 | 11 | +10.0% |

**Continuity canaries (the explicit target of the rebalance):**

| Canary | Pre-181 | Post-181 | Δ |
|---|---|---|---|
| grooming | 286 | 188 | **-34.3%** |
| mentoring | 121 | **21** | **-82.6%** |
| mythic-texture | 11 | **0** | **-100%** |
| play | 23 | 35 | +52.2% |
| burial | 0 | 0 | (ticket 182) |
| courtship | 0 | 0 | (ticket 182) |

**Deaths:** 8 → 8 total (1 Starvation in both runs). Hard-gate `deaths_by_cause.Starvation == 0` fails identically — this isn't a 181 regression.

**Wren focal trace (L2 averages over 5,164 evals):** `hunt = 0.970`, `forage = 0.911`, `groom_self = 0.415`, `sleep = 0.381`. Hunt and Forage's other axes are scoring near max because hunger pressure dominates — the saturation suppression weight of 0.20 is a multiplicative factor on a near-saturated baseline, so absolute score reduction is small. The colony-wide Patrol jump must come from cats with lower hunger pressure where the suppression *did* meaningfully shift the L3 softmax — and Patrol is the closest non-suppressed competitor.

## Concordance

**Direction match per metric:**
- Forage % share: ✓ (both predicted and observed: down)
- Hunt % share: **✗ wrong direction** (predicted down, observed up)
- Higher-tier DSE share (Groom / Coord / Mentor): **✗ wrong direction** (predicted up, all three observed down)
- Patrol % share: not predicted; absorbed nearly all the freed bandwidth (+15 pp)

**Magnitude (where direction matched):**
- Forage: predicted -12 pp, observed -8.57 pp — within 2× ✓

**Verdict: REVERT.** The hypothesis correctly predicted Forage suppression but failed structurally on the *consequence* — freed bandwidth does not naturally flow to higher-tier DSEs in this softmax landscape; it flows to Patrol (the next-most-eligible Tier-2 DSE for cats whose food-seeking was suppressed). Worse, the colony's nourishment axis crashed to zero and continuity canaries grooming, mentoring, and mythic-texture collapsed by 34% / 83% / 100% respectively — the *opposite* of the rebalance's stated goal.

This is not a magnitude miss to iterate on; it's a structural model error. Iterating weight upward would worsen the directional miss on Hunt and deepen the colony_score crash. Iteration 2 paths require different thinking than "lift weight further":

1. **Pair saturation suppression with a positive lift on higher-tier DSEs** — if freed bandwidth doesn't reach groom/mentor/coord *passively*, give them an active boost when food security is high (e.g., a `colony_food_security` axis with non-inverted curve added to those DSEs, weighted positively).
2. **Reconsider the Maslow-ascent assumption** — the substrate spec assumes higher-tier DSEs are kept down by lower-tier *competition*. The data says higher-tier DSEs are kept down by *eligibility / cost / spatial* gates that suppression-of-rivals doesn't touch.
3. **Investigate Patrol** — Patrol absorbing 15 pp suggests Patrol's score is just below Hunt+Forage in the L3 softmax for many cats. If that's intended (defense baseline), the saturation axis is fine but the rebalance can't help higher tiers without other changes. If unintended (Patrol should be lower-priority), Patrol scoring needs its own ticket.

Recommend: park ticket 181 with the structural finding, file the path-2/path-3 follow-on, leave the substrate inert (weight=0) until a different mechanism is designed.

## Iteration 2 — 2026-05-07

**Predecessor evidence:** `logs/tuned-42-pre-181-iter2/` (15-min seed-42 deep-soak at commit `9573dc8d`, post-184 baseline, weights at default 0.0). Ticket 184 (`4db67313`) closed by removing the `CanHunt` over-gating on `Injured` that caused the iteration-1 collapse; the post-184 baseline is dramatically healthier — colony aggregate score 1861.94 (+51% vs pre-181), all six continuity canaries firing.

**Reframe.** The iteration-1 verdict named the *substrate-design assumption* as the failure point. Post-184 evidence shows that diagnosis was wrong: the actual failure was ticket 184's `CanHunt`/`Injured` over-gating, which kept the colony in chronic food crisis (mean `colony_food_security` ≈ 0.008) and made the saturation curve effectively inert (output ≈ 0.981 — nearly *no* suppression at any weight). With 184 fixed, the post-184 baseline runs at chronic abundance instead (mean fs ≈ 0.985, curve output ≈ 0.021). Per the user's "Reframe discipline" rule, the iteration-1 verified-suspect rows do *not* carry over: the test was contaminated and the structural conclusion was unfounded.

### Hypothesis

In the post-184 fs ≈ 0.985 regime, the saturation axis fires at near-full strength every tick. The substrate-design assumption — that suppressing Hunt/Forage frees L3 bandwidth for higher-tier DSEs — can be tested cleanly for the first time. The recalibration target is 6–10% multiplicative L2 score reduction (vs iteration-1's *intended* 19–15% which was actually 0.4–0.3% in practice). Asymmetric weights preserve Forage's 0.36 spatial-axis loading.

### Prediction

**Constants change:** `hunt_food_security_weight: 0.0 → 0.10`, `forage_food_security_weight: 0.0 → 0.07`.

**Curve mechanics at fs = 0.985:** axis output ≈ 0.0208. Score multiplier `(1 − w) + w · 0.0208`:
- Hunt at w=0.10: multiplier 0.902, ~9.8% reduction.
- Forage at w=0.07: multiplier 0.932, ~6.8% reduction.

**Predicted action-share shift** (anchored on `logs/tuned-42-pre-181-iter2`):

| Action | Pre-iter-2 share | Predicted post-iter-2 share | Direction |
|---|---|---|---|
| Forage | 33.74% | ~28% (-5–6 pp) | **down** |
| Hunt | 13.11% | ~10% (-3 pp) | **down** |
| Mentor | 0.78% | ~2–3% (+1.5–2 pp) | up |
| Coordinate | 4.43% | ~6% (+1.5 pp) | up |
| GroomOther | 10.31% | ~12% (+1.5 pp) | up |
| Cook | 11.87% | ~13% (+1 pp) | up (modest) |
| Patrol | 9.43% | ≤10.5% (≤+1 pp) | **flat** (guard rail) |

**Predicted survival/canary outcomes:**
- `deaths_by_cause.Starvation == 0` (post-184 was 0; suppression only at high fs).
- `deaths_by_cause.ShadowFoxAmbush ≤ 10` (post-184 was 1).
- `colony_score.aggregate` within ±10% of 1861.94 (drift > 10% triggers four-artifact escalation).
- `colony_score.nourishment` ≥ 0.55 (post-184 was 0.632; target avoids the iteration-1 zero failure mode).
- All six continuity canaries non-zero; grooming and mentoring within ±20% of post-184 (945 / 310).

**Failure modes that revert to 0.0 / 0.0:**
- Patrol delta > +5 pp → iteration-1 pattern in miniature; structural finding survives recalibration.
- Nourishment drops below 0.4 → curve is suppressing real hunger states, not just secure ones.
- Any survival hard-gate fail.

### Observation

Soak: `just soak-trace 42 Simba` → `logs/tuned-42/` (commit `58a29b37` dirty + the iteration-2 weight change, seed 42, 900s sim duration).

**Action distribution shift** (CatSnapshot.current_action, colony-wide, via `just q actions`):

| Action | Pre-iter-2 (n=7,042) | Post-iter-2 (n=2,897) | Δ pp | Predicted | Match? |
|---|---|---|---|---|---|
| Forage | 33.74% | 27.58% | **−6.16** | −5–6 | ✓ direction + magnitude |
| Hunt | 13.11% | 13.98% | **+0.87** | −3 | **✗ wrong direction** |
| Patrol | 9.43% | **16.29%** | **+6.86** | ≤+1 (guard rail) | **✗ guard-rail FAIL** |
| Cook | 11.87% | 13.57% | +1.70 | +1 | ✓ direction |
| GroomOther | 10.31% | 7.70% | **−2.61** | +1.5 | **✗ wrong direction** |
| Coordinate | 4.43% | 3.04% | **−1.39** | +1.5 | **✗ wrong direction** |
| PickUp | 3.05% | 7.73% | +4.68 | (unanticipated) | — |

**colony_score axes** (via `just verdict` + footer comparison):

| Axis | Pre-iter-2 | Post-iter-2 | Δ |
|---|---|---|---|
| aggregate | 1861.94 | 1242.47 | **−33.3%** |
| nourishment | 0.632 | 0.410 | **−35.1%** |
| welfare | 0.507 | 0.434 | −14.4% |
| health | 0.863 | 0.185 | **−78.6%** |
| happiness | 0.841 | 0.575 | **−31.6%** |
| seasons_survived | 4 | 3 | −25% |
| bonds_formed | 32 | 18 | −43.8% |
| kittens_born | 1 | 0 | **−100%** |
| structures_built | 5 | 6 | +20.0% |

**Continuity canaries:**

| Canary | Pre-iter-2 | Post-iter-2 | Δ |
|---|---|---|---|
| courtship | 2383 | **35** | **−98.5%** |
| grooming | 945 | 289 | **−69.4%** |
| mentoring | 310 | 87 | **−71.9%** |
| mythic-texture | 32 | 13 | −59.4% |
| play | 4 | 11 | +175% |
| burial | 0 | 0 | — |

**Deaths:**

| Cause | Pre-iter-2 | Post-iter-2 |
|---|---|---|
| ShadowFoxAmbush | 1 | 6 |
| Starvation | **0** | **1** |
| WildlifeCombat | 0 | 1 |
| Total | 1 | 8 |

**Hard-gate fails:** `deaths_by_cause.Starvation == 0` violated (1 starvation death). `just verdict` exit code 2.

### Concordance

**Direction match per metric:**
- Forage % share: ✓ (predicted down, observed −6.16 pp)
- Hunt % share: **✗ wrong direction** (predicted down, observed +0.87 pp — same as iteration 1)
- Higher-tier DSEs (Groom / Coord / Mentor): **✗ wrong direction** (predicted up, all three observed down)
- Patrol % share: **✗ guard-rail violated** (predicted ≤+1 pp, observed +6.86 pp — same pattern as iteration 1's +14.99, smaller magnitude)

**Magnitude (where direction matched):**
- Forage: predicted −5–6 pp, observed −6.16 pp — within prediction band ✓

**Verdict: REVERT.** Iteration 2 reproduces iteration 1's structural pattern at smaller magnitude:

| Pattern | Iter-1 (w=0.20/0.15) | Iter-2 (w=0.10/0.07) |
|---|---|---|
| Forage drops | −8.57 pp ✓ | −6.16 pp ✓ |
| Hunt rises | +2.64 pp ✗ | +0.87 pp ✗ |
| Patrol absorbs | +14.99 pp | +6.86 pp |
| Higher tiers drop | Groom/Mentor/Coord all down | Groom/Coord down, Mentor down |
| Aggregate score | −22.2% | **−33.3%** |
| Hard-gate fail | Starvation=1 | Starvation=1 |

The recalibration falsified the iteration-1 reframe. The post-184 healthy baseline did not change the substrate behavior — at any non-zero weight that produces measurable suppression, freed L3 bandwidth flows to Patrol (and PickUp, +4.68 pp), not to higher-tier DSEs. The Maslow-ascent assumption ("suppress lower-tier rivals → higher-tier elections rise") is wrong for this softmax landscape.

The 184 fix improved the colony's *food acquisition* (chronic abundance) but did not change the *L3 softmax topology*. With `colony_food_security` consistently high, the saturation curve fires every tick — every cat experiences ~10% Hunt / 7% Forage suppression — and the Patrol / PickUp landings show those are simply the next-most-eligible Tier-2 DSEs in the colony's current scoring landscape.

Worse, iter-2's aggregate −33% is *deeper* than iter-1's −22% despite half the weight, because:
1. The post-184 fs ≈ 0.985 regime makes the suppression *fire constantly* (vs iter-1's near-inert curve).
2. The colony's chronic-abundance state depends on consistent Hunt/Forage participation. Suppressing those acquisition activities causes the stockpile to drain → starvation re-emerges → mate gate (`breeding_hunger_floor`) closes → courtship collapses 98.5%.

This is precisely the failure mode the iteration-1 doc warned about under path 1: "if freed bandwidth doesn't reach groom/mentor/coord *passively*, give them an active boost when food security is high." Iteration-2 confirms the passive mechanism does not work at any tested weight.

### Mechanism: predator-exposure cascade, not direct food-economy collapse

The original 181 hypothesis modelled the saturation suppression as a direct trade between Hunt/Forage participation and stockpile draw. The iter-2 evidence reveals the actual mechanism is a **second-order ecological cascade** mediated by Patrol exposure to ShadowFoxes:

1. **L3 shift.** Saturation suppression at fs ≈ 0.985 reduces Hunt/Forage scores; Patrol absorbs +6.86 pp of action share (next-most-eligible Tier-2 DSE) and PickUp absorbs +4.68 pp.
2. **Predator coupling.** Patrol routes cats through the perimeter where ShadowFoxes spawn; the encounter rate per cat-tick rises even though `shadow_fox_spawn_total` *drops* (16 → 5 in the verdict drift). Per-spawn lethality jumps.
3. **Ambush wave.** Five ShadowFox ambush deaths land between ticks 1,220,516 – 1,232,851 (12,335-tick window): Heron, Simba (focal), Bramble, Calcifer, plus one more. Bramble's pre-death timeline shows 5 successful Ambush survivals + 3 ShadowFox banishings + 2 wards placed before her sixth ambush killed her — the colony was actively defending but the encounter cadence outran the defenses.
4. **Threat regime.** `interrupts_by_reason` shows **15,614× `modifier_preemption(acute_health_adrenaline_flee)`** in iter-2 (vs 0 in baseline). Surviving cats live in chronic flee state; plans die before completion.
5. **Plan-churn collapse.** Wren's PlanCreated cadence is 3.65 ticks avg (`just q cat-timeline` flags "plan-churn pattern (cadence < 5 ticks)"). 15,202 plans created across 55,464 ticks of life. She mounted 213 hunt attempts and killed 58 prey — the per-cat acquisition still works — but the colony has only ~3 working labourers post-ambush. Stockpile drain outpaces input.
6. **Starvation.** Wren starves at tick 1,255,465 — 24,000 ticks *after* the ambush wave that thinned the labour pool. The starvation isn't caused by individual Hunt suppression; it's caused by the labour deficit that opened when half the colony died to foxes elevated by the L3 shift.

**The iter-1 `nourishment = 0.000` is the same cascade, amplified.** Iter-1 ran with 0.20/0.15 weights against the 184 over-gating bug — the bug separately suppressed Hunt eligibility for Injured cats, so post-ambush survivors couldn't even attempt to hunt. The cascade ran to completion: empty stockpile, total nourishment crash. The 184 fix removed the over-gating but didn't change the underlying L3-shift → Patrol → ShadowFox → labour-loss mechanism, so iter-2 reproduced the cascade at smaller magnitude.

**Implication for any future tuning.** The saturation axis cannot be priced in isolation from the Patrol → ShadowFox coupling. A successful design either (a) prevents Patrol from absorbing freed bandwidth (e.g., the path-1 paired-axis positive lift on higher-tier DSEs), (b) decouples Patrol from predator exposure (a different system), or (c) prices the predator-exposure cost into Patrol's L2 score so the L3 softmax doesn't naively elevate it. None of these is reachable by further weight-tuning of `hunt_food_security_weight` / `forage_food_security_weight`.

### Recommendation

Revert to 0.0 / 0.0 (already done). Keep ticket 181 active for the next session with the cascade mechanism documented; the next iteration is a paired-axis design, not a third weight tune. Existing scenarios (`hunt_acquisition_to_kill`, `hunt_deposit_chain`, `hunt_deposit_chain_injured`, `picking_up_scavenging`, `modifier_preempts_hunt`, `farming_cycle`) all run cleanly under the reverted constants — no codebase regression to investigate.
