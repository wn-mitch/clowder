---
id: 084
title: Farm DSE — tie scoring to herb/ward stockpile demand so gardens stay productive under abundant food
status: parked
cluster: balance
added: 2026-04-29
parked: 2026-04-30
blocked-by: [086]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md, 084-farm-herb-ward-demand.md, 085-gardens-multiuse-build-gate.md]
landed-at: null
landed-on: null
---

## Why

Ticket 083 activated L2 PairingActivity and demoted `CropTended` / `CropHarvested` from the never-fired-positives canary. The demotion is grounded — Farm DSE is `CompensatedProduct(food_scarcity, diligence, garden_distance)` and a healthy post-Wave-2 food economy correctly gates Farm dormant when `food_fraction ≈ 0.98`. But this leaves a real gap: gardens are dual-purpose (`CropKind::FoodCrops` → Berries/Roots, `CropKind::Thornbriar` → ward herb) and `coordination.rs::evaluate_coordinators` (line ~532) repurposes a garden to Thornbriar when `ward_strength_low && !thornbriar_available`. Under abundant food + low ward stockpile, the repurposed Thornbriar plot has no scoring path that motivates a cat to tend it — Farm only reads `food_scarcity`, which stays near zero when food is full.

Operationally: a colony with full pantries that's losing wards has gardens flipped to Thornbriar but no farmer. The Thornbriar plot sits at `growth = 0.0` until food_fraction collapses for some other reason — by which point wards may already be down.

## Scope

- Add a `ward_resource_pressure` (or equivalent) consideration to `FarmDse` that reads ward-stockpile signals (`WardStrengthLow` colony marker + Thornbriar availability) so a Thornbriar-repurposed garden draws a farmer even with food abundant.
- Re-promote `Feature::CropTended` and `Feature::CropHarvested` to `expected_to_fire_per_soak() => true` once the new axis demonstrably drives gardens-under-low-wards in soaks.
- Run the four-artifact methodology (hypothesis · prediction · observation · concordance) per CLAUDE.md, since this is a balance change to a characteristic metric (Farming PlanCreated, CropTended count).

## Out of scope

- Restructuring the garden dual-purpose split (FoodCrops vs Thornbriar) — both kinds remain.
- Changing `CompositionMode::CompensatedProduct` on Farm — the gate is correct shape, just under-axes.
- Wild-Thornbriar gathering balance (separate concern via `herbcraft_gather_dse`).
- Adding a parallel "tend any crop" DSE — single-DSE-with-extra-axis is the smaller surface change and follows the §L2.10.7 pattern.

## Approach

1. Pick the demand signal. Candidates:
   - `WardStrengthLow` colony marker (already populated at `goap.rs:947`) + `!thornbriar_available` colony state.
   - Stockpile-fraction-style: `thornbriar_in_stores / desired_stockpile`, scaled the same way `food_scarcity = 1 - food_fraction` is.
   - Hybrid: outer gate via `WardStrengthLow`, axis curve via stockpile fraction.
2. Add the axis to `FarmDse::new()` (`src/ai/dses/farm.rs`) — likely a Linear or scarcity-shaped curve over the chosen scalar.
3. Plumb the scalar into `EvalInputs` (`scoring.rs::ctx_scalars`) so the new Consideration's input name resolves at scoring time.
4. Confirm no circular dependency: the axis reads colony state already populated by `evaluate_and_plan` before scoring runs.
5. Open `docs/balance/084-farm-herb-ward-demand.md` with hypothesis + prediction; run baseline (HEAD before this change) + treatment (HEAD with this change) sweeps; compute concordance.

## Verification

Acceptance gate (single-seed first, then four-artifact sweep):

- `just soak 42 && just verdict logs/tuned-42-084` — hard gates hold (Starvation = 0, ShadowFoxAmbush ≤ 10, four pass canaries).
- `CropTended ≥ 1` and `CropHarvested ≥ 1` in conditions where `ward_strength_low` triggers during the run.
- `Farming` PlanCreated count > 0 with the new axis providing the lift.
- Multi-seed sweep clears the four-artifact concordance check; magnitude within ±10% of prediction.

On pass: re-promote `Feature::CropTended` / `Feature::CropHarvested` in `system_activation.rs::expected_to_fire_per_soak`, update the `expected_to_fire_per_soak_classification` test, and append observation block to `docs/balance/084-farm-herb-ward-demand.md`.

## Log

- 2026-04-29: Opened. Carved out from ticket 083 (`l2-pairing-farming-scheduler-regression`) — 083 closes the L2 activation question by demoting the Farm canaries; 084 owns the herb-driven Farm-motivation thread that justifies eventual canary re-promotion.
- 2026-04-29: Code change landed (axis added to `FarmDse`, `farm_herb_pressure` scalar plumbed through `ctx_scalars`, commit `410f544c`). Signal choice: the same boolean condition the coordinator uses at `coordination.rs:532` (`ward_strength_low && !thornbriar_available`). Curve: Linear identity, weight 1.0, under existing `CompensatedProduct`.
- 2026-04-29: Treatment soak `logs/tuned-42-084/` ran against `410f544c`-dirty. **Outcome: bit-identical to baseline `tuned-42-083` on every farming-related metric.** P1–P4 are untestable in this seed; P5a–d (regression checks) all hold — the axis addition is empirically inert.
- 2026-04-29: **Parked.** Diagnosis (in `docs/balance/084-farm-herb-ward-demand.md ## Observation`): the colony never builds a garden in this regime, so `HasGarden` eligibility filter blocks Farm DSE before the new axis can affect anything. Root cause is upstream at `coordination.rs:984` — `pressure.farming` only accumulates when `food_fraction < 0.3`, but post-Wave-2 + L2-active colonies maintain `food_fraction = 0.98` from simulation start. The 084 axis is structurally correct and stays committed; canary re-promotion of `Feature::CropTended` / `Feature::CropHarvested` is **blocked-by: [085]** which owns the threshold-mismatch fix.
- 2026-04-30: 085 landed the disjunctive build-pressure gate (`food_demand || herb_demand`) as architectural prep — bit-identical to baseline in seed 42 (gate never fires in this seed's natural dynamics). Empirical investigation showed the gate works structurally (verified via probe: `food_threshold=0.95` + loosened `herb_demand=ward_strength_low` produces a Garden + `HasGarden = true`), but every loosening that makes the gate fire in seed 42 also breaks survival canaries (`courtship 764→0`, `wards_placed 5→1`, 4 wildlife-combat deaths) by redistributing cat-time from social/defense to construction. 084's `farm_herb_pressure` axis remains untestable on seed 42 because the colony never enters the `(ward_strength_low ∧ !ThornbriarAvailable)` regime long enough for Farm to score above competing actions — wild thornbriar respawns; cats gather on the rare absence. Re-blocked on **086** (find a triggering seed/scenario for the Farm canary, e.g. forced-weather destabilization or multi-seed sweep). 085's balance doc captures the empirical evidence at `docs/balance/085-gardens-multiuse-build-gate.md ## Why P1–P3 are unchanged in seed 42`.
