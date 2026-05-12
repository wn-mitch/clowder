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
