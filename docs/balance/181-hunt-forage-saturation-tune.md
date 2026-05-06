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

Soak: `just soak-trace 42 Wren` → `logs/tuned-42/` (commit `75586184` + the iteration-1 weight change, seed 42, 900s sim duration).

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

## Out of scope

- Curve shape (`Composite{Logistic(8.0, 0.5), Invert}`) — start with the substrate's default; tune weights first.
- The `min(food_fraction, hunger_satisfaction)` formula — separate balance ticket if the simple form proves insufficient.
- Ticket 182's courtship/burial regression — independent.
- Multi-seed `just hypothesize` sweep — moot now; iteration 1 directionally failed.

## Out of scope

- Curve shape (`Composite{Logistic(8.0, 0.5), Invert}`) — start with the substrate's default; tune weights first.
- The `min(food_fraction, hunger_satisfaction)` formula — separate balance ticket if the simple form proves insufficient.
- Ticket 182's courtship/burial regression — independent.
- Multi-seed `just hypothesize` sweep — only escalate after seed-42 single-soak confirms direction (per CLAUDE.md "smallest verifiable step first" and the user's "Chain-rare events: structural verification beats sweep gating" feedback rule).
