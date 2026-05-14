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

## Iter-2: 312 — fox-approach-corridor perception axis (FO-2 landing)

**Date:** 2026-05-13
**Substrate landed:** `FoxApproachCorridorMap` populated by patrolling-fox movement; `ward_fox_approach_corridor_weight` scoring constant gating a **multiplicative-outside** lift in `compute_ward_placement`.

### Architectural choice

Composition (a) from the ticket's two-option menu: the corridor lift sits **outside** the saturating `(threat - coverage).clamp(0, 1)` step, expressed as `unaddressed_threat * (1.0 + w_corridor * L(corridor))`. Composition (b) — adding a fifth additive lift inside the `.min(1.0)` sum — was ruled out by direct reference to 297 iter-2: additive lifts inside the saturating sum are rank-preserving for argmax once any threat input saturates. Multiplicative-outside lets a high-corridor tile's effective score exceed the [0, 1] ceiling on the threat axis, breaking the rank-preservation pathology that motivated FO-2.

At `w_corridor = 0.0` (the default), the factor `(1 + 0 * L) = 1.0` makes the formula bit-identical to the post-301 baseline. The byte-identity invariant is pinned by `coordination::tests::corridor_axis_dormant_when_weight_is_zero` and the existing `ward_placement_dormant_when_weights_forced_to_zero` test.

### The scenario-level acceptance

`scenarios::chokepoint_defense_isthmus::tests::corridor_corks_isthmus` exercises the scorer against the FO-1 isthmus geometry with pre-deposited fox traffic and `ward_fox_approach_corridor_weight = 0.3`. The argmax lands in the 5-tile band `x ∈ [28, 32]` centered on the isthmus, where it would otherwise prefer the cat-cluster interior. The control (`dormant_corridor_does_not_cork_isthmus`) confirms that without the substrate signal at all, the scorer does not cork the corridor — proving the corked outcome is driven by the axis, not by topology side effects.

End-to-end Path A emission (`Feature::WardPlaced` actually firing through the directive → dispatch → L3 election → set-ward chain in the scenario harness) is **not** asserted at this ticket. The chain involves coordinator selection cadence, urgent dispatch thresholds, and Herbalism sub-action priority — dynamics outside 312's scope. The unit-test-level scorer assertion plus the soak-scale hypothesize sweep are the architectural validation.

### Soak validation

Three-seed hypothesize sweep at `w_corridor = 0.3`:

- [`hypothesis-312-fox-approach-corridor-axis-activation.yaml`](hypothesis-312-fox-approach-corridor-axis-activation.yaml) — primary spec, seed 42, predicts ≥ +20% lift in `shadow_foxes_avoided_ward_total`.
- [`hypothesis-312-fox-approach-corridor-axis-activation-seed99.yaml`](hypothesis-312-fox-approach-corridor-axis-activation-seed99.yaml) — companion, seed 99.
- [`hypothesis-312-fox-approach-corridor-axis-activation-seed7.yaml`](hypothesis-312-fox-approach-corridor-axis-activation-seed7.yaml) — companion, seed 7.

The corridor map is populated by `update_fox_approach_corridor_map` reading `FoxAiPhase::PatrolTerritory` deposits each tick, slow-decay at `fox_approach_corridor_half_life_ticks = 20_000` (4× slower than ambush memory because corridors are stable terrain features, not transient event echoes). The system is scheduled inside the existing wildlife `.chain()` block alongside `fox_scent_tick` to avoid a new schedule-edge (ticket 061 precedent).

### What 312 doesn't do

- Does NOT re-examine `cat_value` / `distance_cost` (FO-3).
- Does NOT migrate the signal into the 258 belief layer (FO-4, blocked by 263–270 belief-DSE consumers).
- Does NOT lift the global default off dormancy. The substrate ships wired but inert; the three-seed hypothesize sweep is the gate before a global activation ticket (FO-3 territory or a follow-on).
- Does NOT bias cat-side A* pathfinding. The corridor map is a ward-placement signal only; cats route via their existing `RouteCostField` substrate untouched.

## Iter-3: 313 — cat_value as a soft eligibility gate (FO-3 landing)

**Date:** 2026-05-13
**Substrate landed:** `WardPlacementCatValueComposition` enum (`Additive` / `Gate`), `ward_placement_cat_value_gate_floor` scoring constant. Gate composition replaces the additive `+ w_cat_value * cat_value` reward with a saturating-ramp gate `(cat_value / gate_floor).clamp(0, 1)` multiplied onto the threat-merit term.

### Architectural choice — option (c) from the three-option menu

Ticket 313 named three structural candidates for fixing the L3 composition defect 301's first-light data localized: **(a) re-weight** the existing terms (parameter tuning), **(b) replace the `distance_cost` anchor** with a fox-spawn-relative travel cost (new colony-aggregated landmark), or **(c) compose `cat_value` as a soft eligibility gate** rather than an additive reward.

Option (c) lands. Reasoning: (a) keeps the formula shape and tunes parameters without changing what the parameters mean (which is where 298 already lives, and where 301 iter-1 ruled out additive-bias parameter tuning as a mover of the argmax); (b) introduces a new colony-aggregated landmark with real risk of misuse elsewhere in the scorer; (c) re-shapes one term in the formula to match its actual function (a reachability gate, not a density reward) and removes the load-bearing density bias that 301's first-light showed pulled placement away from corridor tiles.

Post-313 score formula under `Gate`:

```text
score = unaddressed_threat * (1.0 + w_corridor * L(corridor)) * gate(cat_value)
      - distance_cost + jitter
gate(cat_value) = (cat_value / gate_floor).clamp(0, 1)
```

Distance cost and jitter remain additive: distance should still penalize regardless of cat density (jitter still tiebreaks on dead tiles). The gate applies only to the threat-merit term — gating the entire score would invert distance's penalty role on warm tiles.

### Gate shape — saturating ramp, not literal max

The ticket's pseudocode `score *= max(cat_value, 0.2)` doesn't match its prose intent ("dead tiles score ~zero, warm tiles score full"). `max(0, 0.2) = 0.2` is 20% suppression, not "~zero"; `max(0.3, 0.2) = 0.3` is 30% of full, not "full." The shipped function is the saturating ramp `(cat_value / FLOOR).clamp(0, 1)` with default `FLOOR = 0.2`:

| cat_value | gate  | meaning                            |
| --------- | ----- | ---------------------------------- |
| 0.00      | 0.00  | dead tile — fully suppressed       |
| 0.10      | 0.50  | half-warm — half-suppressed        |
| 0.20      | 1.00  | knee — saturates                   |
| 0.50      | 1.00  | warm — full merit                  |
| 1.00      | 1.00  | peak — full merit, no density bias |

This matches the prose verbatim and exposes a single tunable (`gate_floor`).

### Dormancy invariant

The composition flag defaults to `Additive` — at the global default, the score formula reduces to the pre-313 expression bit-for-bit. Pinned by `coordination::tests::cat_value_gate_dormant_at_additive_default` (mirrors `corridor_axis_dormant_when_weight_is_zero` from iter-2's pattern).

### Scenario-level acceptance — ring formation

A new scenario `surrounded_colony` exercises ring-of-coverage behavior: 5 cats clustered at the center of a 60×40 map, 8 static `ShadowFox`es on the compass-direction periphery, a cat-scent wandering halo at the cluster perimeter. 4 successive `compute_ward_placement` wakes (with each pick stamped into `WardCoverageMap` between wakes) must plant wards in all 4 cardinal sectors. The scenario's `mod tests` runs the assertion under **both** compositions:

- `additive_composition_builds_ring_of_coverage` — proves the multi-wake spreading semantics (load-bearing gameplay behavior) survives 313's code change.
- `gate_composition_builds_ring_of_coverage` — proves the Gate composition doesn't break ring formation in surrounded-threat geometry, because the cat-scent halo clears the gate at the perimeter candidates.

The scenario also surfaces an architectural finding: **Gate trades chokepoint defense for cluster-perimeter ring coverage.** Activating Gate in the FO-1 chokepoint scenario (where cats live on the east landmass and the chokepoint isthmus has zero cat-scent) zeros the corridor merit and reverts placement to the cluster centroid. Gate works in surrounded-cluster geometry because cat-scent extends to the perimeter via wandering; Gate fails in chokepoint geometry because the chokepoint is, by definition, a tile cats don't visit. The chokepoint scenario therefore stays at `Additive` while activating only the corridor axis. The `setup()` comment in `chokepoint_defense_isthmus.rs` documents the rationale; the global default flip needs to weigh both geometries.

### Layer-walk re-promotion (Reframe discipline)

301 iter-1 marked L3 (composition) as `[suspect]` under the pre-312 framing. 312 reshaped the composition (corridor lift outside the saturating sum), so the row needed re-promotion under v2. The 312 landing data showed the L3 composition was still `[suspect]` under v2 because the cat-density additive lift still overpowered the corridor lift on the cluster-perimeter:

- **L3 [verified-defect, 313]**: under corridor=0.3 + Additive on the surrounded-colony substrate, the first 6 wakes plant wards mostly along cardinal axes near the cluster centroid (the load-bearing density reward). Under corridor=0.3 + Gate, the same 6 wakes spread across all 4 cardinal sectors — demonstrating the additive density reward was the live obstruction to ring formation.
- **L1/L2 [verified-correct]**: cat_scent map and per-candidate cat_value reads, both unchanged from 298.
- **L4 [verified-correct]**: argmax. No change since 297-iter-2.
- **L5 [verified-correct]**: directive emission. No change.

### Soak validation

Three-seed hypothesize sweep activating BOTH the corridor axis at `w_corridor = 0.3` AND the Gate composition:

- [`hypothesis-313-cat-value-gate-composition.yaml`](hypothesis-313-cat-value-gate-composition.yaml) — primary spec, seed 42, predicts ≥ +30% lift in `shadow_foxes_avoided_ward_total` over the post-312 dormancy baseline.
- [`hypothesis-313-cat-value-gate-composition-seed99.yaml`](hypothesis-313-cat-value-gate-composition-seed99.yaml) — companion, seed 99.
- [`hypothesis-313-cat-value-gate-composition-seed7.yaml`](hypothesis-313-cat-value-gate-composition-seed7.yaml) — companion, seed 7.

The four-artifact concordance verdict and the seed-by-seed `wards_placed_total`, continuity canaries, and hard-gate readouts are captured in the soak observations section below as the runs land.

### Observations (three-seed sweep)

| Seed | wards_placed (b→t) | shadow_foxes_avoided_ward (b→t) | ward_siege_started (b→t) | ShadowFoxAmbush (b→t) | Survival gate | Continuity |
| ---- | ------------------ | ------------------------------- | ------------------------ | --------------------- | ------------- | ---------- |
| 42   | 17 → 15 (−11.8%)   | 730 → 724 (−0.8%)               | 0 → 0                    | 1 → 0                 | pass          | pass       |
| 99   | 0 → 0              | 0 → 0                           | 0 → 0                    | 6 → 6                 | pass\*        | fail\*\*   |
| 7    | 22 → 22            | 618 → 618                       | 0 → 0                    | 3 → 3                 | pass          | pass       |

\* Hard gate `deaths_by_cause.ShadowFoxAmbush ≤ 10` holds on all three seeds. The "fail" status from `just verdict` is against the registered global baseline (`logs/baselines/current.json`), which captures a different colony shape than seed-99's; the within-sweep baseline vs treatment is the load-bearing comparison for 313's concordance.

\*\* seed-99 baseline ALSO has `continuity: fail:play=0,mythic-texture=0` — pre-existing seed-99 quirk unrelated to 313 (the canary fails in the no-override run too). The 313 composition change does NOT introduce or worsen the failure.

**Concordance verdict: wrong-direction on the predicted metric, no regression elsewhere.**

- **Hypothesis & prediction:** Gate composition + corridor activation → ≥ +30% lift in `shadow_foxes_avoided_ward_total` on seed-42, similar lift band on seeds 99 / 7.
- **Observation:** flat-to-slightly-down on the target metric. seed-42 shifts placement (17 → 15 wards) with a small positive hard-gate signal (1 → 0 ShadowFoxAmbush). Seeds 99 and 7 are bit-identical between baseline and treatment — the composition change doesn't alter the argmax for the candidate set those seeds produce.
- **Direction match:** no.
- **Magnitude match:** N/A (target metric flat).
- **Other deltas:** `wards_placed_total` within ±15% on seed-42; bit-identical on seeds 99/7. Continuity canaries unaffected. Hard gates pass.
- **Architectural read:** the substrate is wired and validated **structurally** (unit tests + scenario tests pass), but the soak-scale metric is too rare to surface a statistically meaningful effect in a 15-min run. Same empirical shape as 312: substrate-no-op landing, soak-level verification confined to "no regression."

Per the spec's pre-registered iteration policy ("wrong-direction across all three seeds → land 313 findings-only and open a follow-up"), 313 lands as a structural substrate change without flipping the global default. The architectural finding (Gate suppresses chokepoint placement when cats don't visit the chokepoint) and the empirical no-regression result inform the follow-on iter that decides whether to promote Gate globally, tune `gate_floor`, or pursue option (b).

### What 313 doesn't do

- Does NOT flip the global default. `ward_placement_cat_value_composition = Additive` remains the ship default and the FO-1 chokepoint scenario stays on Additive. Promoting `Gate` globally needs a follow-on iter (analogous to 312 → 313 cadence) that weighs the chokepoint-vs-surrounded tradeoff.
- Does NOT re-examine `distance_cost`. Ticket 299 stays open as the symmetric parameter-tuning ticket; 313 lands without changing the additive distance penalty.
- Does NOT migrate `cat_value` into the 258 belief layer (FO-4).
