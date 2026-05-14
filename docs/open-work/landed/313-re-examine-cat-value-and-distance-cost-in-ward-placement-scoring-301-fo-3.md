---
id: 313
title: re-examine cat_value and distance_cost in ward-placement scoring (301 FO-3)
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [301-ward-placement-decision-semantics.md, 297-fox-patrol-topology-axis.md, 298-ward-placement-cat-value-coefficient.md]
landed-at: cfa8e65d7e0f
landed-on: 2026-05-13
---

## Why

301's first-light data localized the dominant biases in `compute_ward_placement`'s score formula: the `+ 0.3 * cat_value` term and the `- distance_cost` term both pull placement toward the structure-cluster centroid and cat density. For chokepoint defense (the desired pattern: wards on fox-approach corridors regardless of where cats live), both biases work against the right tile. 298 (which promoted `cat_value` to a constant) and 299 (which would promote `distance_cost`) treated these as tuning knobs; the empirical first-light reads them as **architecturally wrong-shaped**, not just mis-valued.

This ticket blocks on FO-2 (ticket 312) so the corridor-perception axis is in place to *replace* what these biases currently provide (a soft cat-reachability gate). Once corridor signal can dominate the threat side of the formula, the cat-side biases need to be either re-weighted, replaced with a different primitive, or composed differently. The "best tile" calculation should point at "where the colony's defense needs a ward" rather than "where convenience lets a cat plant one."

## Scope

- Layer-walk audit of the score formula post-312: with the corridor axis active at `weight = 0.3`, measure how much `cat_value` and `distance_cost` perturb the argmax on the isthmus scenario and on seed-42 / 99 / 7 soaks. Use `just frame-diff` against a 312-only baseline.
- Pick ONE structural candidate from the three-option menu below, on layer-walk evidence (per CLAUDE.md bugfix discipline).
- Implement the chosen candidate behind a default-preserving fixture (gradual weight ramp, new mode flag, or composition variant — pick at design time).
- Three-seed `just hypothesize` four-artifact validation.
- Update `docs/balance/301-ward-placement-decision-semantics.md` with an iter-3 follow-on entry; or if the change is substantial, a new `docs/balance/313-*.md` thread.

## Out of scope

- Re-activating descending-residual or intent-weight from 301 (those defaults stay at SingleShotArgmax + 0.0).
- New influence map inputs (those are 312's territory).
- Cat-side path-cost integration. This ticket changes the *placement scorer*, not how cats walk to wards.
- Belief-layer migration — FO-4 (longer horizon).

## Current state

312 (`FoxApproachCorridorMap`) gives the placement scorer a topology-aware threat input. With it active, `unaddressed_threat` for a corridor tile rises above the cat-cluster-interior tile's score *only if* the corridor lift exceeds the combined pull from `+ 0.3 * cat_value` and `- distance_cost` toward the structure centroid. The 301 data shows those two terms ARE the deciding factor in the threat-saturated regime — 297 / 298 / 300 ruled out the alternatives. FO-3 picks the structural revision; the corridor axis (312) gives it something to compose with.

## Approach

**Structural-option menu** (pick ONE on layer-walk evidence post-312):

- **(a) Re-weight.** Lower `ward_placement_cat_value_weight` from 0.3 → ~0.05 and lower `DIST_PENALTY_PER_TILE` from 0.005 → ~0.001. Cheapest change. Risk: too low and wards drift into impassable / corner tiles cats can't reach. Layer-walk lever — keeps the formula shape, just turns down the volume.

- **(b) Replace `distance_cost` anchor with travel-cost-from-fox-spawn.** Per 299's structural note, the load-bearing primitive in today's formula is Manhattan-distance from the structure centroid; for chokepoint defense the *right* anchor is fox-spawn (closer-to-spawn = better wards because you intercept earlier). Requires: a colony-aggregated "nearest fox-spawn corridor source" landmark, sampled at scoring time. The corridor map (312) can supply this — the spawn-source argmax of the corridor map IS the colony's read on "where foxes come from."

- **(c) Compose `cat_value` as a soft eligibility gate, not an additive bias.** Keep `cat_value` from being negative-decisive (placement should never go to inaccessible tiles), but stop letting it *reward* placement at cat-density peaks. E.g., replace `+ 0.3 * cat_value` with a multiplicative gate `score *= max(cat_value, 0.2)` — wards on dead tiles (no cat-scent at all) score ~zero, wards on warm-but-not-peak tiles score full. Sits closer to the doctrine's "items are real" framing: the constraint is "a cat must be able to plant here," not "placement should be near a cat-density peak."

**Default candidate**: (c). Reasoning: (a) tunes parameters without changing what the parameters mean (which is where 298 already lives); (b) introduces a new colony-aggregated landmark (real work, real risk of misuse elsewhere); (c) re-shapes one term in the formula to match its actual function (cat-reachability gate) and removes the "near-cat reward" that empirically misleads placement. Decide post-FO-2 once the corridor lift is observable.

**Layer-walk audit** (promoted at 313 landing):
- L1: `cat_scent` map. `[verified-correct]` per 298 (re-verified under v2 — the input is unchanged by 312's corridor axis).
- L2: per-candidate `cat_value` and `distance_cost` reads. `[verified-correct]` per 298 / 299 (re-verified under v2 — reads aren't shaped by composition).
- L3: composition. **`[verified-defect, 313]`** — under corridor=0.3 + Additive on the `surrounded_colony` scenario substrate, 6 wakes plant wards along cardinal axes near the cluster centroid (the additive density reward dominates). Under corridor=0.3 + Gate, the same 6 wakes spread across all 4 cardinal sectors, demonstrating the additive density reward was the live obstruction to ring formation. The structural lever lives at L3, confirming option (c).
- L4: argmax. `[verified-correct]` per 297 iter-2 (returns max).
- L5: directive emission. `[verified-correct]`.

## Verification

- `just check` + `just test` green.
- New unit tests proving the chosen variant: at default values (= pre-313 defaults) the score is unchanged; at the new tuned values the corridor-side and chokepoint-side tiles win the argmax in synthetic placement-maps.
- FO-1 isthmus scenario passes the corked assertion under FO-2 + FO-3 combined activation.
- Three-seed `just hypothesize`:
  - `shadow_foxes_avoided_ward_total` direction match (lift) on seed-42 ≥ +30% from 312 baseline.
  - `wards_placed_total` within ±15%.
  - Continuity canaries ≥ 1.
  - Hard-gate canaries pass (Starvation = 0, ShadowFoxAmbush ≤ 10).
- Balance writeup (`docs/balance/301-...md` iter-3 or `docs/balance/313-*.md`) documents the structural option picked, the layer-walk evidence, and the four-artifact concordance.

## Log

- 2026-05-13: opened from 301's findings-only landing. Blocked by 312 (FO-2 corridor axis).
- 2026-05-13: chose option (c). Saturating-ramp gate `(cat_value / FLOOR).clamp(0, 1)` (FLOOR default 0.2) replaces the additive `+ 0.3 * cat_value` term, applied multiplicatively to the threat-merit term only. Composition flag (`WardPlacementCatValueComposition` enum, `Additive` default) preserves byte-identity at ship default. New `surrounded_colony` scenario asserts ring formation under both compositions; chokepoint scenario stays on `Additive` because the chokepoint geometry has zero cat-scent (architectural tension surfaced — see balance/301 iter-3). Three-seed hypothesize sweep authored at `hypothesis-313-cat-value-gate-composition*.yaml`.
- 2026-05-13: three-seed sweep complete. Concordance verdict **wrong-direction on the predicted metric, no regression elsewhere** — seed-42 shifts 17 → 15 wards with 1 → 0 ShadowFoxAmbush (small positive hard-gate signal); seeds 99 and 7 are bit-identical between baseline and treatment. Per the spec's pre-registered iteration policy, **landing 313 findings-only** as a structural substrate change. Global default stays at `Additive` / corridor weight `0.0`; future iters decide on promotion based on either tuning `gate_floor` or pursuing option (b). Full four-artifact concordance recorded in `docs/balance/301-ward-placement-decision-semantics.md` iter-3.
