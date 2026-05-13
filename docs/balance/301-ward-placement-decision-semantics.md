# Ward-placement decision semantics — first-light findings (substrate-no-op land)

**Date:** 2026-05-13. Ticket 301 (architectural follow-on to 285 / 296 / 297 / 298 / 300).

## Hypothesis

297 iter-2 concluded that the threat-axis composition in `compute_ward_placement()` is rank-preserving once any single threat input saturates a sufficient number of tiles, and that the argmax is therefore decided by `+ 0.3 * cat_value − distance_cost + jitter`. Five sibling levers (magnitude, curve shape, new orthogonal axis, `cat_value` coefficient, candidate-step grid) had been independently ruled out as movers of the argmax. Ticket 301 targeted the deepest remaining lever: the **selection rule itself** — replace single-shot argmax with a descending-residual K-round greedy that progressively eats coverage as it places, so successive picks spread across the threat surface instead of co-locating in the same hot cluster.

Pre-registered prediction: the structural change would lift `shadow_foxes_avoided_ward_total` on seed-42 (the seed where 297 iter-2 documented cluster lock-in), with `wards_placed_total` holding within ±15% and the five continuity canaries holding.

## Methodology — substrate-no-op land with first-light validation

Two soaks on seed-42 at commit `bebf2378` (post-implementation, pre-landing):

- **Dormancy soak** (`logs/tuned-42-default-postmerge-bebf2378`): defaults preserved (`ward_placement_semantics = SingleShotArgmax`, `ward_intent_dse_weight = 0.0`). Acceptance gate: WardPlaced events byte-identical (across `tick, cat, ward_kind, location, strength`) to the pre-implementation baseline `logs/tuned-42-pre301-bebf2378`.
- **First-light soak** (`logs/tuned-42`, original): both flags activated (`DescendingResidual` with K=2, `ward_intent_dse_weight = 0.3`) via `CLOWDER_OVERRIDES`. Wall-budget 900s identical to dormancy.

Both runs are `just soak-trace 42 Pyre` for trace parity. The Pyre focal matches the pre-301 baseline's focal so future `just frame-diff` runs work.

## Constants landed

The implementation lands three new fields on `ScoringConstants`:

- `ward_placement_semantics: WardPlacementSemantics` (enum `SingleShotArgmax | DescendingResidual`), default `SingleShotArgmax`. **First enum-typed field in `ScoringConstants`** — serde round-trips as a JSON string literal in the events.jsonl header, preserving the comparability invariant.
- `ward_placement_residual_rounds: i32`, default `2`. Ignored when semantics is `SingleShotArgmax`.
- `ward_intent_dse_weight: f32`, default `0.0`. Substrate-dormant per 220 / 297 first-light pattern.
- `ward_intent_decay_per_wake: f32`, default `0.5`. Applied per coordinator wake under `DescendingResidual`; dormant under `SingleShotArgmax`.

Plus: new `WardIntentMap` resource (mirror of `WardCoverageMap`, registered in the L1 influence-map registry), a `via_directive: bool` field on the `WardPlaced` event so Path A vs Path B can be disambiguated downstream, and a conditional 4th `Consideration` on `HerbcraftWardDse` that activates only when `ward_intent_dse_weight > 0.0` (preserves the 3-axis `CompensatedProduct` at default — `n=3` matters because the geometric-mean compensation exponent `1/n` would otherwise shift).

## Observation

### Dormancy invariant holds (acceptance gate passes)

| Metric | Pre-301 baseline | Post-301 dormancy |
|---|---|---|
| `WardPlaced` events | 12 | 12 |
| `WardPlaced` (tick, cat, ward_kind, location, strength) tuples | identical | identical |
| `wards_placed_total` footer | 12 | 12 |
| `shadow_foxes_avoided_ward_total` | 55 | 55 |
| `deaths_by_cause.ShadowFoxAmbush` | 8 | 8 |
| Five continuity canaries each ≥ 1 | yes | yes |

The substrate-no-op land is safe to ship. The `via_directive: bool` field on `WardPlaced` is the only event-stream change and is purely additive.

### Path A produces ZERO events on seed-42

All 12 dormancy and all 11 first-light `WardPlaced` events carry `via_directive: false` — every ward in this seed's soak comes from cats self-picking `HerbcraftSetWard` and planting at their current position. The coordinator's `compute_ward_placement` is called every ~20 ticks, but the resulting `ActiveDirective::SetWard` never reaches a `resolve_set_ward` step — either no cat gets assigned, or no cat completes the walk to the directive's target.

This is an empirical corner solution on seed-42: 100% Path B. 297 iter-2's "Path B dominates" claim is sharper than expected on this seed. The structural change to `compute_ward_placement` has nothing to bite on this seed; the only observable effect under activation is via Path B reading the intent map that Path A still populates.

### First-light is anti-concordant on the primary metric

| Metric | Dormancy | First-light | Δ |
|---|---|---|---|
| `WardPlaced` total | 12 | 11 | -8% (within ±15% band) |
| Pairwise Manhattan among ward locations | 12.8 | 16.2 | **+26%** (spread lifted, as predicted) |
| Mean ward-to-nearest-fox-spawn distance | 21.5 | 44.3 | **+106%** (wards moved away from fox spawn corridors) |
| Mean ward-to-nearest-ambush-site distance | 3.4 | 3.3 | unchanged |
| `shadow_foxes_avoided_ward_total` | 55 | **0** | **-100%** (direction wrong) |
| `deaths_by_cause.ShadowFoxAmbush` | 8 | 8 | unchanged |
| `shadow_fox_spawn_total` | 9 | 6 | -33% |
| Duration in ticks (900s wall-budget) | 114,568 | 90,135 | **-21%** (activated path is wall-clock heavier) |
| Continuity canaries (grooming / play / mentoring / courtship / mythic-texture) | all ≥ 1 | all ≥ 1 | pass |

### Architectural cause — cluster lock-in was load-bearing

The score formula `unaddressed_threat + 0.3 * cat_value - distance_cost + jitter` carries two terms that pull placement toward the structure-cluster centroid and toward cat density. `cat_value` rewards tiles where cats live; `-distance_cost` penalizes tiles far from the structure centroid. The descending-residual change picks the round-(K-1) tile — the **most-spread** alternative — instead of the argmax.

The first-light data shows the mechanism clearly: wards still cluster near where cats live (ambush-site distance unchanged at 3.3 tiles) but no longer sit on fox patrol corridors (fox-spawn distance doubled from 21.5 → 44.3 tiles). Once wards are off the patrol corridors, foxes never have a ward to route around, so the `shadow_foxes_avoided_ward_total` counter collapses to zero. Deaths stay constant at 8 because the local engagement-point protection (wards near ambush sites) is unchanged.

Cluster lock-in around the structure centroid was the colony's actual fox-deterrent mechanism. The 297 iter-2 prescription ("spread placement") treated it as a bug; the empirical data reads it as a feature of the current substrate. Spreading wards geometrically is fairer but functionally counter-productive when the colony's threat geometry is concentrated.

## Concordance

| Pre-registered prediction | Observed | Verdict |
|---|---|---|
| Path A pairwise Manhattan ≥ +50% on seed-42 | N/A (Path A produced 0 events) | unprovable on this seed |
| `wards_placed_total` within ±15% | -8% | concordant |
| `shadow_foxes_avoided_ward_total` direction match (lift) | -100% (drop) | **anti-concordant** |
| Continuity canaries hold | all pass | concordant |
| Deaths hold | 8 = 8 | concordant |

### Hard-gate readout

- `deaths_by_cause.Starvation` → 0 (target 0). Pass.
- `deaths_by_cause.ShadowFoxAmbush` → 8 (target ≤ 10). Pass.
- Footer line written. Pass.
- `never_fired_expected_positives` → `["MatingOccurred"]`. **Pre-existing fail unchanged by 301** — same failure mode in the pre-301 baseline (`logs/tuned-42-pre301-bebf2378`), the post-301 dormancy soak, and the first-light soak. Tracked separately as a demographic-substrate dependency (memory `feedback_park_demographic_dependent_tuning`).

## Decision

**Land as substrate-no-op. Do not promote new defaults.**

- `ward_placement_semantics` stays at `SingleShotArgmax`.
- `ward_intent_dse_weight` stays at `0.0`.
- `ward_placement_residual_rounds` stays at `2` (consulted only under `DescendingResidual`).
- `ward_intent_decay_per_wake` stays at `0.5` (consulted only under `DescendingResidual`).

The wiring stays in place as scaffolding for follow-on tickets. The substrate-no-op land preserves seed-42 byte-identity on the load-bearing fields (`tick, cat, ward_kind, location, strength`) and adds the `via_directive: bool` field to `WardPlaced` so future analysis can disambiguate Path A from Path B.

### Three findings drive the follow-on work

1. **The score formula's non-threat biases (`cat_value`, `distance_cost`) dominate selection.** Once any threat input saturates, the argmax is decided by structure-centroid proximity and cat density. Any selection-rule change (including descending-residual) cannot produce concordant outcomes until the **input perception axes** capture what makes a tile a good ward site beyond "near cats." Fox-approach corridors, terrain chokepoints, and observed fox-traversal density are absent from today's perception substrate.

2. **Cluster lock-in is load-bearing — don't break it without an upstream substrate fix.** Wards stacking near the structure centroid is how the colony actually deters foxes on seed-42. A spread-placement algorithm without a topology-aware threat axis just moves wards away from where they're useful. The 297 iter-2 reading of cluster lock-in as a bug is empirically inverted: it's the mechanism. The selection rule isn't the substrate gap; the score function is.

3. **Path A is dormant on seed-42 (zero materialized directives).** Whatever fix lands has to bias Path B (DSE-driven cat-position placement), not just Path A (coordinator-directed target). 301's `WardIntentMap` + conditional DSE axis is the right plumbing shape — it just needs a useful signal to stamp. Today the intent stamp comes from the same `compute_ward_placement` argmax that the formula's biases already corrupt.

## Follow-on tickets

- **FO-1**: chokepoint isthmus scenario fixture. A test scenario (narrow corridor between two landmasses, cats on one side, foxes on the other) that the desired placement algorithm must satisfy: wards should "cork" the isthmus rather than paint the cat landmass.
- **FO-2**: fox-approach-corridor perception axis. New influence map populated by observed `ShadowFox` movement; wired into `compute_ward_placement` as a fourth threat-axis lift; gated dormant per 220 / 297 / 301 pattern. Acceptance: FO-1 scenario passes when the new axis is activated.
- **FO-3**: re-examine `cat_value` and `distance_cost` in placement scoring. Three structural candidates (reweight, replace anchor with fox-spawn-relative travel cost, gate `cat_value` to "not zero" rather than "as large as possible"). Sequenced after FO-2 lands.
- **FO-4** (longer horizon): migrate the corridor perception into the 258 belief layer via a new `WitnessableEvent::FoxCrossing`. Blocked by FO-2 plus the 263–270 belief-DSE consumer landing.

The 301 ticket commit opens FO-1 / FO-2 / FO-3 with `--blocked-by` chain. FO-4 opens later.

## What 301 is NOT

- Not a refutation of the descending-residual mechanism. The unit tests prove K=2 picks the round-1 (most-spread) tile; the spatial check on the first-light soak shows the +26% pairwise Manhattan spread that the algorithm is designed to produce. The mechanism works.
- Not a multi-rep sweep. Single seed (42) with single dormancy + single first-light run. Seeds 99 and 7 may show different Path A dynamics; the anti-concordance on seed-42 is sufficient to halt the activation here.
- Not a claim that selection-rule changes are useless. The wiring stays, dormant. Once FO-2's topology-aware threat axis lands and FO-3 re-balances the non-threat biases, the descending-residual flag can be re-tested — *with* the score function pointing at the right tiles. This is the inverse of the 297 framing: the lever is "what we score," not "how we pick among scores."
- Not a justification to retire the implementation. The substrate-no-op land carries minimal cost (one new resource, one new enum, one new event field, four new constants) and provides infrastructure FO-2 / FO-3 will repurpose.
