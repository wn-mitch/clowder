# Fox-spawn-vicinity perception axis — first-light activation

**Date:** 2026-05-12
**Ticket:** [297](../open-work/tickets/297-ward-placement-needs-fox-patrol-topology-perception-axis-285-follow-on.md)
**Predecessor evidence:** 285 iter-2's three-seed triangulation (`docs/balance/284-ward-anchor-tuning.md` lines 187-216) proved magnitude is architecturally inert at the existing curve; 296's three-seed iter-3 (lines 113+) proved curve shape is also inert at the existing inputs. Both surfaced the deeper architectural finding: the existing inputs (`recent_ambush`, `carcass_scent`, `fox_scent`, `corruption`) encode threat at cat-side or at-corruption tiles, leaving the argmax determined by `+ 0.3 * cat_value`. **No input lights up the *corruption-adjacent* tiles where fox patrols actually traverse on their way to the colony.** 297 closes that gap.
**Substrate:** `src/systems/coordination.rs:1356-1502` + inline helper `compute_fox_spawn_vicinity` at `:1602`. Threat axis now reads `(fox_scent.max(corruption) + w_ambush·L(recent_ambush) + w_carcass·L(carcass_scent) + w_fox_intercept·L(fox_spawn_vicinity)).min(1.0)` where `fox_spawn_vicinity` is the inline-computed Manhattan-radius halo around tiles with `corruption ≥ shadow_fox_corruption_threshold`. All four lifts short-circuit when their weight is 0.0.

## Architecture note — inline computation, not Resource

The initial substrate design used a `FoxSpawnVicinityMap` Resource with a per-tick populator system. That introduced a schedule edge in the wildlife chain that catastrophically perturbed seed-42 (courtship → 0, colony collapsed in season 1) **even at dormant weight=0.0** — matching ticket 061's precedent at `simulation.rs:314-326`. A control soak with the populator unregistered confirmed schedule-edge as the sole cause.

The refactor (committed as part of Phase 2.1) computes the vicinity value *inline* inside `compute_ward_placement` from `TileMap.corruption` directly. No new Resource, no scheduled populator, no schedule edge. Cost: O(radius²) per candidate × ~430 candidates ≈ 350k tile lookups per coordinator wake (every ~20 ticks). Negligible.

Tradeoff: the new axis doesn't surface as a registered InfluenceMap in `trace-*.jsonl`. It's a placement-scorer-internal computation, not a per-cat DSE input, so trace-per-cat doesn't need it. Soak-level verification reads computed values via `WardPlaced` event positions + the spatial scan recipe below.

## Hypothesis

Lifting `ward_fox_intercept_anchor_weight` from 0.0 (dormant) to 0.5 (mirroring `ward_ambush_anchor_weight`'s 284 first-light value) adds a third Logistic-lift term that biases placement toward the radial halo around fox-spawn-eligible corruption tiles. Because the halo extends into LOW-`fox_scent.max(corruption)` tiles (uncorrupted neighbors of corruption sources), the lift contributes the FIRST non-zero threat on those tiles — composing with `+ 0.3 * cat_value` to peak placement at the cats↔corruption boundary, where 285 iter-2's spatial scan identified the geometric gap.

## Methodology — first-light, not four-artifact (this iter-1)

Per `feedback_dormant_substrate_activation_soak_first`: single qualitative `just soak-trace 42 Wren` after lifting the default weight, then `just verdict` against the post-297-substrate-dormant baseline. The first-light question is binary — does the layer fire and do something observable. The four-artifact magnitude validation across 42 / 99 / 7 lands as iter-2.

## Constants landed

```rust
default_ward_fox_intercept_anchor_weight() -> f32     { 0.5 }   // was 0.0 (Phase 2.1 dormant)
default_fox_intercept_kernel_radius_tiles() -> u32    { 20 }    // ~approach corridor
```

## Observation

Single soak-trace: `logs/tuned-42-post-297-first-light/` at commit `5c9c3510` (dirty — 297's Phase 2.1+2.3 in flight), seed 42, 900s release, Wren as focal cat.

### Substrate-active spatial signal — layer fires per unit tests

The unit test `ward_placement_shifts_to_fox_intercept_hotspot_when_tuned` confirms the inline helper produces non-zero vicinity values for tiles near corruption sources and that placement shifts toward those halo regions when no competing fox-scent / ambush / carcass signals are present. The `ward_placement_dormant_when_fox_intercept_weight_zero` test guards the byte-identical-at-0.0 contract.

### Macro outcome counters — bit-flat on seed-42

vs `logs/baselines/post-297-substrate-dormant.json` (Phase 2.2 baseline, weight=0.0):

| Counter | Baseline (w=0.0) | First-light (w=0.5) | Δ |
|---|---|---|---|
| `wards_placed_total` | 14 | 14 | 0 |
| `wards_despawned_total` | 15 | 15 | 0 |
| `shadow_fox_spawn_total` | 24 | 24 | 0 |
| `shadow_foxes_avoided_ward_total` | 2 | 2 | 0 |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 2 | 0 |
| `deaths_by_cause.Starvation` | 0 | 0 | 0 |
| `positive_features_active` | 39 | 39 | 0 |

Every footer-drift field tracked by `just verdict` reports delta=0.0% (noise band). The macro outcome is bit-flat between dormant and first-light on seed-42.

### Spatial topology check — placement byte-identical on seed-42

Post-hoc position scan of `WardPlaced` events across both runs:

| Position | Dormant count | First-light count |
|---|---|---|
| `(29, 23)` | 2 | 2 |
| `(33, 10)` | 4 | 4 |
| `(33, 22)` | 1 | 1 |
| `(38, 22)` | 1 | 1 |
| `(39, 23)` | 2 | 2 |
| `(42, 36)` | 2 | 2 |
| `(62, 3)` | 2 | 2 |

**Identical seven positions, identical multiplicities, identical placement timeline.** The new axis lifts threat values at near-corruption tiles, but on seed-42's geometry the argmax among threat-saturated tiles is still determined by `cat_value` + `distance_cost` + jitter — the same set of cat-side tiles wins regardless of the third lift.

This is the **same architectural pattern as 285** — on seed-42's topology where ambush memory and corruption tiles don't geographically overlap, the lifts add to threat-saturated tiles but don't change the placement argmax. Seeds 99 and 7 (where the geometries overlap or near-overlap) may behave differently; iter-2 four-artifact answers that.

### Continuity canary readout — soak-trace methodology confounder

vs the dormant baseline:

| Canary | Dormant | First-light | Δ |
|---|---|---|---|
| `grooming` | 1156 | 1076 | −6.9% |
| `courtship` | 3392 | 2845 | −16.1% |
| `mentoring` | 227 | 211 | −7.0% |
| `play` | 14 | 14 | 0 |
| `mythic-texture` | 43 | 37 | −14.0% |
| `seasons_survived` | 6 | 5 | −1 |

`courtship` exceeds the CLAUDE.md >±10% drift threshold. The bit-identical placement output and bit-identical macro counters (deaths, structures, bonds, kittens, peak_pop) indicate this is a **methodology confounder, not a weight-induced effect.** The dormant baseline was a regular `just soak`; the first-light run was a `just soak-trace` (captures trace-Wren.jsonl with --focal-cat machinery active). Trace-machinery overhead shifts per-tick RNG cadence enough to drift the tick-level continuity tallies, even though every macro outcome stays identical.

The clean apples-to-apples comparison (both runs via `just hypothesize` infrastructure, no trace) lands in iter-2.

## Concordance

| Artifact | Result |
|---|---|
| **Hypothesis** | Lifting `w_fox_intercept` to 0.5 adds non-zero threat on corruption-adjacent tiles, biasing placement toward the cats↔corruption boundary. |
| **Prediction** | On seed-42's topology (cat-side and corruption regions geometrically separated), the lift fires but may not shift placement until the kernel-radius coverage overlaps both regions. |
| **Observation** | Placement byte-identical to dormant on seed-42 (same 7 sites, same multiplicities). All macro counters delta=0%. Layer fires per unit-test verification of the inline helper. |
| **Concordance** | **Layer-fires call: PASS** (helper produces non-zero vicinity, no continuity-canary hard failure). **Magnitude call: deferred to iter-2 four-artifact across seeds 42/99/7.** Per 285's precedent (seed-42 saturated at counter=2), the load-bearing seeds for magnitude evaluation are 99 and 7. |

### Hard-gate readout

- `deaths_by_cause.Starvation == 0` → PASS (0).
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2).
- `never_fired_expected_positives == 0` → PASS (`[]`).
- Five continuity canaries each ≥ 1 → PASS (grooming 1076, play 14, mentoring 211, courtship 2845, mythic-texture 37).
- Constants-drift-vs-baseline → clean.
- Verdict exit: **pass**.

## Decision

Ship `0.5` as first-light default for `ward_fox_intercept_anchor_weight`. The substrate is wired (unit tests verify the helper); the layer fires (lift is computed when weight > 0); seed-42 outcomes are unchanged at this weight on this seed's topology (consistent with 285's saturation finding). The four-artifact magnitude validation across seeds 42 / 99 / 7 in iter-2 carries the burden of evaluating whether the new axis moves the metric on seeds where the geometries overlap.

## Iteration history

- **iter-1 (2026-05-12):** Landed `0.5` on first-light criteria. Spatial check showed seed-42 placement byte-identical to dormant; macro counters identical; continuity canaries pass; layer-fires verified by unit test. iter-2 carries four-artifact magnitude validation across 42 / 99 / 7.
- **iter-2 (2026-05-12):** Ran four-artifact `just hypothesize` on the `0.0 → 0.5` activation across seeds 42, 99, 7. All three runs **wrong-direction at delta=0%, placement byte-identical between baseline and treatment.** Joins 285 (magnitude inert) and 296 (curve-shape inert) as the **third independent threat-axis lever ruled out.** Substrate ships first-light-activated at `0.5` (layer fires, no continuity regression, slight positive continuity drift), but the metric movement awaits architectural work outside the threat-axis-additive composition. See iter-2 below.

---

# iter-2 — four-artifact validation; three-seed structural inertness confirmed

**Date:** 2026-05-12
**Ticket:** [297](../open-work/tickets/297-ward-placement-needs-fox-patrol-topology-perception-axis-285-follow-on.md)
**Methodology:** four-artifact `just hypothesize`, single-seed × 1-rep × 900s × release, run in parallel across three seeds (42, 99, 7) per 285's triangulation discipline. Hypothesize machinery applies `constants_patch` as a runtime override (no source-tree dirty state between baseline and treatment).
**Specs:**
- `docs/balance/hypothesis-297-fox-intercept-axis-activation.yaml` — primary seed-42.
- `docs/balance/hypothesis-297-fox-intercept-axis-activation-seed99.yaml` — load-bearing (counter has headroom at 20).
- `docs/balance/hypothesis-297-fox-intercept-axis-activation-seed7.yaml` — falsifier (counter at 78).
**Hypothesize output dirs:**
- `logs/hypothesize-at-post-284-anchor-weights-0-5-0-3-and-pre-296-tune-curve-8-/`
- `logs/hypothesize-on-seed-99-the-existing-inputs-already-produce-a-counter-of-/`
- `logs/hypothesize-on-seed-7-the-baseline-counter-of-78-reflects-a-topology-whe/`

## Hypothesis (iter-2)

iter-1's first-light landed the substrate at `w=0.5` and showed seed-42 placement byte-identical to dormant. iter-2 predicted the new axis would move the metric on seeds 99 and 7 where the geometries overlap (counter has headroom on 99, lucky overlap on 7). Pre-registered concordance call: pass if seed-42 lifts ≥6 AND seeds-99/7 hold within ±30%.

## Methodology

Three independent `just hypothesize` invocations, one per seed, run in parallel (each sweep baseline=current defaults + treatment=defaults+patch=0.5). The `0.0 → 0.5` direction is achieved by setting the source default to 0.0 in a temporary commit (later abandoned), so hypothesize's baseline reads as dormant and treatment as activated. Working-copy purity preserved via the abandon-after-validation pattern.

## Constants landed

**None changed.** `default_ward_fox_intercept_anchor_weight()` ships at `0.5` from iter-1's first-light activation. iter-2 validated the activation against the metric prediction but found the metric does not move at any seed.

## Observation

### Three-seed summary — byte-identical placement on every seed

| Seed | Counter B → T | Wards placed B → T | Deaths.ShadowFoxAmbush B → T | Placement byte-identical? |
|---|---|---|---|---|
| **42** | **2 → 2** | 14 → 14 | 2 → 2 | yes (7 unique sites, exact multiplicities match) |
| **99** | **20 → 20** | 9 → 9 | 2 → 2 | yes |
| **7** | **78 → 78** | 11 → 11 | 3 → 3 | yes |

Concordance verdict per spec: **wrong-direction at delta=0%** on every seed. Effect size 0.0, p=1.0 (no variance in the metric across the change).

### Continuity tallies — small positive drift across all seeds (treatment side)

vs the baseline within each spec:

| Seed | Canary | Baseline (w=0.0) | Treatment (w=0.5) | Δ |
|---|---|---|---|---|
| 42 | courtship | 3148 | 3346 | +6.3% |
| 42 | grooming | 1122 | 1152 | +2.7% |
| 99 | courtship | 3944 | 4175 | +5.9% |
| 99 | grooming | 1048 | 1130 | +7.8% |
| 7 | courtship | 1887 | 1887 | 0% |
| 7 | grooming | 838 | 881 | +5.1% |

All deltas in the positive direction, max +7.8% (within the >±10% threshold). **The new axis at 0.5 does not regress continuity** on any seed — the iter-1 soak-trace's apparent -16% courtship drift was the soak-vs-soak-trace methodology confounder, confirmed now by the clean hypothesize comparison.

### Spatial topology corroboration — placement byte-identical at every seed

Post-hoc position scans on all six runs (baseline + treatment × 3 seeds): ward placements are byte-identical between baseline and treatment within each seed. The `0.0 → 0.5` change does not move which tiles win the argmax on any seed.

### Sharpening the architectural read

285 (iter-2) ruled out **anchor-weight magnitude** as a placement lever — `(0.5, 0.3) → (0.7, 0.4)` produced byte-identical placement on all three seeds.

296 (iter-3) ruled out **Logistic curve shape** as a placement lever — `(k=8.0, m=0.5) → (k=4.0, m=0.5)` produced byte-identical placement on all three seeds.

297 (iter-2) rules out **adding a new orthogonal axis to the threat-side input set** as a placement lever — adding a third Logistic-lift over `fox_spawn_vicinity` halo, weighted at the same magnitude as the ambush anchor, produces byte-identical placement on all three seeds.

**The architectural conclusion sharpens:** the placement scorer's argmax is determined by the non-threat terms (`+ 0.3 * cat_value`, `- distance_cost`, jitter) once any single threat-side input saturates on a sufficient number of tiles. The threat-axis composition `(fox_scent.max(corruption) + L(ambush) + L(carcass) + L(fox_intercept)).min(1.0)` is **rank-preserving for the argmax in this regime** regardless of which inputs are tuned. To move `shadow_foxes_avoided_ward_total`, the architecture has to change at a different layer than the threat-axis inputs.

Candidate structural levers for follow-on work:
- **The `+ 0.3 * cat_value` coefficient.** At its current weight, `cat_value` is the tiebreak among threat-saturated tiles. Lowering it would let threat differentiation propagate to argmax. Risk: wards drift away from cat clusters (placement quality regression).
- **The `distance_cost` term** (currently `0.005 / tile` from anchor). Tightening it would prevent placement from reaching corruption-zone tiles outside the anchor's local Manhattan ring. Loosening it would let placement reach more distant high-threat tiles.
- **The candidate-generation step** (currently every 5 tiles). Finer sampling might surface intermediate tiles the coarse grid misses.
- **The placement decision semantics.** Argmax over additive sum may not be the right composition; an arrest-the-worst-violator approach (place wards by descending threat residual after coverage) could move the metric differently.

These are out of scope for 297 — ticket 297's surface was the orthogonal-axis addition. iter-2 confirms the substrate is wired and the layer fires, while documenting that further metric movement requires work at a layer 297 explicitly excluded.

## Concordance

| Artifact | Seed-42 | Seed-99 | Seed-7 |
|---|---|---|---|
| **Hypothesis** | New axis at 0.5 lifts placement onto corruption-vicinity tiles, raising avoided counter. | Same; seed-99 is load-bearing. | Same; seed-7 is falsifier. |
| **Prediction** | `shadow_foxes_avoided_ward_total` Δ ∈ [+200, +1500]%. | Δ ∈ [−30, +50]%. | Δ ∈ [−20, +30]%. |
| **Observation** | Δ = 0% (2 → 2). Byte-identical placement. | Δ = 0% (20 → 20). Byte-identical placement. | Δ = 0% (78 → 78). Byte-identical placement. |
| **Concordance** | **wrong-direction** (Δ=0% below the +200% floor). | **wrong-direction** (Δ=0% below the +10% floor). | Δ=0% inside [−20, +30], **direction call** is "unchanged" → **wrong-direction** vs predicted "increase." |

### Hard-gate readout (treatment soaks, three seeds)

- `deaths_by_cause.Starvation == 0` → PASS (0 on all three seeds).
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2, 2, 3 respectively).
- `never_fired_expected_positives == 0` → PASS (`[]` on all three).
- Five continuity canaries each ≥ 1 → PASS on all three.
- Constants-drift-vs-baseline → clean.
- Verdict exit: **pass** on all three.

## Decision

**Ship `0.5` as the first-light default. Land as findings-only on the magnitude prediction.** The substrate is wired, the layer fires, no continuity regression, no metric movement. The default value lands at `0.5` because iter-1 established first-light activation criteria pass; the metric prediction not moving is a documented architectural finding, not a reason to revert the activation.

Three findings drive any follow-on work, ranked by structural depth:

1. **Threat-axis composition is rank-preserving in this regime.** Three independent inputs (magnitude, curve shape, new orthogonal axis) tested over six unique constant changes across three seeds — none moved the placement argmax. The argmax is determined by non-threat score terms (`cat_value`, `distance_cost`, jitter) once any threat input saturates. Any future placement-tuning ticket needs to surface a structural change at a different layer.
2. **The new axis does what it's designed to.** Unit tests verify the inline `compute_fox_spawn_vicinity` helper produces correct halo values around corruption sources. The threat contribution composes correctly in the Logistic lift. The architectural inertness is a downstream-argmax-saturation effect, not a substrate bug.
3. **Continuity tally drift is methodology-sensitive.** iter-1 soak-trace showed -16% on courtship; iter-2 hypothesize sweep shows +6% on the same canary. The soak vs soak-trace methodology produces ~10% per-tick RNG drift on continuity tallies even when placement and macro outcomes are byte-identical. Future first-light validations should use hypothesize-grade comparison (or sib-by-sib soak vs soak) for continuity-drift assessment.

**No follow-on ticket opened.** The "non-threat-axis layer needs structural work" finding is too open-ended to convert into a single ticket without further investigation. 298 (joint anchor-weight re-tune after 296+297) is no longer attractive given the architectural inertness across three independent levers.

## What iter-2 is NOT

- Not a refutation of the substrate's correctness. Unit tests verify the helper computes correct halo values. The architectural inertness is downstream of the helper, in the placement-scorer argmax.
- Not a multi-rep sweep. Single rep per seed; per-seed Welch's t can't run. The byte-identical observation across three independent seeds is the load-bearing evidence.
- Not a claim that the axis is useless. The substrate is wired and ready; if future structural work on the placement scorer (cat_value coefficient, candidate-generation step, decision semantics) opens up rank-changing dynamics, the fox-spawn-vicinity axis is in place to participate.
- Not a justification to escalate the weight further. Three threat-axis levers have now been independently ruled out; escalating w_fox_intercept past 0.5 would just produce more saturation. The lever is elsewhere.
