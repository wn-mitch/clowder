# Build-pressure farming gate — disjunctive food-or-herb demand

**Date:** 2026-04-30
**Ticket:** [085](../open-work/tickets/085-build-pressure-farming-threshold-mismatch.md) (reframed mid-investigation)
**Parent commit:** _kvtymznq (`feat: Farm DSE herb/ward demand axis (084) + 083 closeout`)_
**Predecessor evidence:** post-084 treatment soak (`logs/tuned-42-084/`, commit `410f544c`-dirty over `e838bb7`) — 14 `BuildingConstructed` events, all `kind: structure`, none Garden. `CropTended = 0`, `CropHarvested = 0`. The 084 herb-axis added a `farm_herb_pressure` consideration to FarmDse, but the test was bit-identical to baseline because `FarmDse::eligibility = .require(HasGarden)` filtered Farm out before scoring — the colony never built a garden in the first place.

## Hypothesis

The original ticket framing (`085-build-pressure-farming-threshold-mismatch.md`) attributed the gap to a single-axis food-only gate at `coordination.rs:984`:

```rust
if !has_garden && food_fraction < cc.build_pressure_farming_food_threshold { // 0.3
    pressure.farming += rate;
} else { pressure.farming *= BuildPressure::DECAY; }
```

— and proposed tuning the threshold from 0.3 toward the post-082 food economy median (~0.95). User pushback ("gardens are multiuse") reframed the fix: gardens grow FoodCrops *and* Thornbriar (`CropState::crop_kind ∈ {FoodCrops, Thornbriar}`, `src/components/building.rs:298–304`); the post-construction repurposing path at `coordination.rs:530–540` already encodes this when `ward_strength_low && !thornbriar_available`. The build-pressure gate should mirror the repurposing gate one level upstream: build a garden when *either* food demand *or* herb demand wants one.

The structural fix replaces the single-axis food gate with a disjunction:

```rust
let food_demand = food_fraction < cc.build_pressure_farming_food_threshold;
let herb_demand = ward_strength_low
    && !wild_thornbriar_available
    && !any_cat_carrying_thornbriar;
if !has_garden && (food_demand || herb_demand) {
    pressure.farming += rate;
} else { pressure.farming *= BuildPressure::DECAY; }
```

`herb_demand` is **stricter** than the repurposing gate's predicate. The repurposing path uses `ward_strength_low && !wild_thornbriar_available` — appropriate for a cheap, reversible decision (`crop_kind` flip + `growth = 0.0` reset). Building a permanent structure is irreversible; a stricter "no thornbriar anywhere in the colony's reach" predicate (no wild patches, no cat carrying any) avoids over-building when wild herbs flicker absent for a tick.

## Prediction

Treatment: this commit. Baseline: parent (post-084).

| Metric | Pre-085 baseline (`tuned-42-084`) | Post-085 prediction | Direction | Magnitude band |
|---|---|---|---|---|
| **P1: `BuildingConstructed` count** | 14 | unchanged in seed 42 (gate fires 0% of ticks) | ↔ | bit-identical |
| **P2: `CropTended` count** | 0 | unchanged in seed 42 | ↔ | no-change |
| **P3: `CropHarvested` count** | 0 | unchanged in seed 42 | ↔ | no-change |
| **P4: `wards_placed_total`** | 5 | within ±1 | ↔ | no-regression |
| **P5: `continuity_tallies.courtship`** | 764 | within ±5% | ↔ | hard no-regression — the L2 PairingActivity trunk must not be perturbed |
| **P6: `continuity_tallies.{grooming, play, mythic-texture}`** | 61 / 254 / 50 | within ±10% | ↔ | no-regression |
| **P7: `deaths_by_cause.Starvation`** | 0 | 0 | ↔ | hard gate hold |
| **P8: `deaths_by_cause.ShadowFoxAmbush`** | 3 | ≤ 10 | ↔ | hard gate hold |
| **P9: `never_fired_expected_positives`** | 3-item set | unchanged set | ↔ | trunk stability |

The seed-42 prediction is **deliberately no-change**. In this seed:
- `food_fraction` median ≈ 0.95, so `food_demand = food_fraction < 0.3` fires 0% of ticks.
- Wild thornbriar is sustained in seed 42 (cats gather 6+ herbs across the soak), so `wild_thornbriar_available || any_cat_carrying_thornbriar` is true continuously after the first gather. `herb_demand` is suppressed.
- Disjunction `(food_demand || herb_demand)` evaluates to false at every coordinator tick.
- `pressure.farming` decays to zero each cycle. No Build directive issued. `has_garden` stays false. No new BuildingConstructed events.

## Observation

Canonical 15-min release soak `logs/tuned-42/` (seed 42, this commit) versus baseline `logs/tuned-42-084/` (commit `410f544c`-dirty over `e838bb7`):

| Metric | 084 baseline | Post-085 treatment | Δ |
|---|---|---|---|
| `BuildingConstructed` count | 14 | 14 | 0 (bit-identical) |
| `Feature::CropTended` | 0 | 0 | 0 |
| `Feature::CropHarvested` | 0 | 0 | 0 |
| `wards_placed_total` | 5 | 5 | 0 |
| `wards_despawned_total` | 5 | 5 | 0 |
| `ward_siege_started_total` | 34 | 34 | 0 |
| `shadow_fox_spawn_total` | 26 | 26 | 0 |
| `anxiety_interrupt_total` | 19382 | 19382 | 0 |
| `continuity_tallies.courtship` | 764 | 764 | 0 |
| `continuity_tallies.grooming` | 61 | 60 | -1 (within noise) |
| `continuity_tallies.play` | 254 | 254 | 0 |
| `continuity_tallies.mythic-texture` | 50 | 50 | 0 |
| `continuity_tallies.mentoring` | 0 | 0 | 0 (pre-existing) |
| `continuity_tallies.burial` | 0 | 0 | 0 (pre-existing) |
| `deaths_by_cause.ShadowFoxAmbush` | 3 | 3 | 0 |
| `deaths_by_cause.Starvation` | 0 | 0 | 0 (hard gate hold) |
| `never_fired_expected_positives` | `[MatingOccurred, GroomedOther, MentoredCat]` | same | 0 |

`just verdict logs/tuned-42` reports `delta_pct: 0.0` on every footer field, `constants_drift_vs_baseline: clean`, and `seed_match_vs_baseline: match`. The pre-existing `fail:mentoring=0,burial=0` continuity flag carries through unchanged from baseline (verified via `just verdict logs/tuned-42-084` showing identical failure reasons). Diff of `BuildingConstructed` event records between baseline and treatment is empty.

`BuildingConstructed` events at the same 5 distinct locations (`[29,15]`, `[29,16]`, `[28,15]`, `[29,23]`, `[34,19]`) at the same ticks (1201494, 1201537, 1203541-1203544). No new garden built.

## Concordance

| Prediction | Observed | Direction match | Magnitude in band | Verdict |
|---|---|---|---|---|
| **P1** `BuildingConstructed` count unchanged | 14 → 14 | ✓ | bit-identical | **concordant** |
| **P2** `CropTended` unchanged | 0 → 0 | ✓ | bit-identical | **concordant** |
| **P3** `CropHarvested` unchanged | 0 → 0 | ✓ | bit-identical | **concordant** |
| **P4** `wards_placed_total` within ±1 | 5 → 5 | ✓ | within band | **concordant** |
| **P5** `courtship` within ±5% | 764 → 764 | ✓ | bit-identical | **concordant** |
| **P6** `grooming/play/mythic-texture` within ±10% | 61→60 / 254→254 / 50→50 | ✓ | within band | **concordant** |
| **P7** `Starvation` = 0 hard gate | 0 → 0 | ✓ | hard gate hold | **concordant** |
| **P8** `ShadowFoxAmbush` ≤ 10 hard gate | 3 → 3 | ✓ | hard gate hold | **concordant** |
| **P9** `never_fired_expected_positives` unchanged | 3-item set unchanged | ✓ | bit-identical | **concordant** |

**Verdict: concordant on every prediction.** The structural change is empirically inert in seed 42 — exactly as predicted. The architecture is now consistent with the post-construction repurposing path and FarmDse's herb-pressure axis without perturbing colony dynamics.

## Why P1–P3 are unchanged in seed 42 (architectural-only landing)

The disjunctive gate is **structurally correct** — verified by a probe with `food_threshold=0.95` and a loosened herb_demand (`= ward_strength_low` only): the gate fires, `pressure.farming` accumulates above the actionable threshold, a Garden ConstructionSite spawns, and `HasGarden` flips true. Empirical evidence at `logs/tuned-42-085-v2-loose/events.jsonl`: narrative `Mocha marks out the site for a new garden` at tick 1203500, 17 BuildingConstructed events vs. 14 baseline.

But that loosened configuration breaks survival canaries:

| Metric | 084 baseline | v1-strict (this commit) | v2-loose (probe; not landing) |
|---|---|---|---|
| `continuity_tallies.courtship` | 764 | 764 | **0** ❌ |
| `wards_placed_total` | 5 | 5 | **1** ❌ |
| `deaths_by_cause.WildlifeCombat` | 0 | 0 | **4** ❌ |
| `never_fired_expected_positives` adds | — | — | `CourtshipInteraction`, `PairingIntentionEmitted` |

The L2 PairingActivity trunk silenced; cats died in wildlife combat with no wards up. Aggressive thresholds redistribute cat-time toward construction at the cost of social/defense activity. **Per CLAUDE.md "Hard survival gates" + "A refactor that changes sim behavior is a balance change"**, that's a balance regression, not a fix.

So 085 lands the **structural change only** — the disjunctive gate with the strict supply-aware herb_demand. Bit-identical behavior in seed 42 (gate never fires); the architecture is now consistent with the repurposing gate at `coordination.rs:530–540` and FarmDse's `farm_herb_pressure` axis at `scoring.rs:493–503`. Future scenarios where the herb axis genuinely fires (e.g., a forced-weather soak that destabilizes wards combined with a wild-thornbriar drought) will trigger garden construction without further code changes.

## Decision

**Ship the structural change. Keep ticket 084 parked.** `Feature::CropTended` and `Feature::CropHarvested` stay at `expected_to_fire_per_soak() => false` — re-promotion is gated on a triggering scenario being reproducible per CLAUDE.md soak gates, which this investigation showed seed 42 doesn't naturally provide.

Open follow-up ticket [086](../open-work/tickets/086-farm-canary-triggering-scenario.md) to find a seed/scenario where Farm DSE can be exercised end-to-end (CropTended ≥ 1, CropHarvested ≥ 1) — likely needs a forced-weather soak or a multi-seed sweep to identify a colony where the `(ward_strength_low, !ThornbriarAvailable)` regime persists long enough for Farm to score above competing actions.

## Out of scope (revised after the v2-loose probe)

- **Threshold tuning of `build_pressure_farming_food_threshold` (0.3 → 0.85+).** Original 085 framing. Replaced by the disjunctive gate. Empirically broke survival canaries in the v2-loose probe; not safe to ship.
- **Loosening `herb_demand` to drop the supply check.** Tested in v2-loose; same regression class. Rejected.
- **Loosening `farm_herb_pressure` itself in `scoring.rs:493–503`.** Would touch 084's structural design. Belongs to ticket 086 if at all.
- **Reducing `BuildPressure::DECAY` (0.95 → 0.99).** Broad change affecting all build axes (cooking, workshop, defense, farming). Out of scope; would need its own balance pass.

## Risks

1. **Architectural change with no observable behavior change in canonical seed.** That's the point — 085 is structural prep. Risk: the gate fires unexpectedly under conditions we haven't tested. Mitigation: the `should_accumulate_farming_pressure` truth-table unit test pins the predicate shape, and the supply-aware herb_demand mirrors the repurposing path's intent.
2. **Future plumbing assumption.** If a later ticket adds `DepositHerbs` (cats deposit gathered thornbriar into Stores instead of holding it), `any_cat_carrying_thornbriar` no longer captures the full colony supply. The supply-aware predicate would need to extend with a `stockpile_thornbriar` count. Documented here so the next change doesn't silently leave an inventory-only signal in place.
3. **`wild_thornbriar_available` is a "world has plant" signal, not a "stocked thornbriar" signal.** Same predicate the repurposing path uses; consistent semantics across the system. The wild-patch flicker concern is mitigated by the conjunction with `!any_cat_carrying_thornbriar` — a single cat carrying thornbriar suppresses the gate even during wild-patch absences.
