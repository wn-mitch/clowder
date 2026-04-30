---
id: 085
title: "Build-pressure farming gate: disjunctive food-or-herb demand"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-30
---

# Build-pressure farming gate: disjunctive food-or-herb demand

**Why:** Ticket 084's treatment soak was bit-identical to baseline because `FarmDse::eligibility = .require(HasGarden)` filtered Farm out before scoring — the colony never built a garden. Root cause: the build-pressure farming gate at `coordination.rs:984` only accumulated pressure when `food_fraction < 0.3`, but post-Wave-2 + L2-active colonies hold `food_fraction ≈ 0.95+` from sim start. The original ticket framed this as a single-axis threshold-tuning problem (raise 0.3 → 0.7+).

**Reframed mid-investigation.** User pushback ("gardens are multiuse") rejected single-axis tuning. Gardens grow FoodCrops *and* Thornbriar (`CropState::crop_kind`); the post-construction repurposing path at `coordination.rs:530–540` already encodes this when `ward_strength_low && !thornbriar_available`. The structural fix mirrors that disjunction one level upstream at the build-pressure gate: build a garden when *either* food demand *or* herb demand wants one.

**What landed:**

1. **`src/systems/coordination.rs`** — extended `accumulate_build_pressure` with `wards: Query<&Ward>`, `herbs: Query<&Herb, With<Harvestable>>`, `cat_inventories: Query<&Inventory, Without<Dead>>`. Pre-computes `ward_strength_low`, `wild_thornbriar_available`, `any_cat_carrying_thornbriar`. Replaced the single-axis gate at line 984 with a disjunction over `food_demand || herb_demand` via the extracted free function `should_accumulate_farming_pressure(has_garden, food_demand, herb_demand) -> bool`. Truth-table unit test (`farming_gate_truth_table`) pins the 8-row predicate shape.
2. **`herb_demand` is supply-aware and stricter than the repurposing gate.** Predicate: `ward_strength_low && !wild_thornbriar_available && !any_cat_carrying_thornbriar`. The repurposing gate uses `ward_strength_low && !wild_thornbriar_available` — appropriate for cheap reversible flips. Building is irreversible, so the supply check (no wild patches AND no cat carrying any) avoids over-building when wild herbs flicker absent for a tick. `Inventory::has_herb(HerbKind::Thornbriar)` (`src/components/magic.rs:255–259`) is the only API needed; thornbriar today only lives in cat inventories (no `DepositHerbs` to Stores), so aggregate-over-inventory is the full colony supply.
3. **`docs/balance/085-gardens-multiuse-build-gate.md`** — full hypothesis / prediction / observation / concordance. Verdict: **concordant on every prediction.** Canonical 15-min soak `logs/tuned-42/` shows `delta_pct: 0.0` on every footer field vs. baseline `logs/tuned-42-084/`; `BuildingConstructed` event diff is empty (14 → 14, same locations same ticks); courtship/play/grooming/mythic-texture all preserved.

**What is NOT in this landing (and why):**

- **No threshold tuning** of `build_pressure_farming_food_threshold` (stayed at 0.3). The v2-loose probe (threshold 0.95 + loosened `herb_demand`) verified the gate's wiring — produces a Garden + `HasGarden = true` — but broke survival canaries: `courtship 764→0`, `wards_placed 5→1`, 4 wildlife-combat deaths, the L2 PairingActivity trunk silenced. Per CLAUDE.md "A refactor that changes sim behavior is a balance change" + survival hard gates, that's a balance regression. The structural change ships standalone; aggressive thresholds would need their own balance pass.
- **No Crop feature re-promotion.** `Feature::CropTended` and `Feature::CropHarvested` stay at `expected_to_fire_per_soak() => false`. The 084 axis is structurally correct but seed 42 doesn't naturally enter the `(ward_strength_low ∧ !ThornbriarAvailable)` regime long enough for Farm to score above competing actions.

**Empirical implications captured.** Architectural-only landing: bit-identical behavior in seed 42, gate ready to fire in scenarios that genuinely satisfy the disjunction. Future scenarios where the herb axis fires (forced-weather, multi-seed, or a wild-thornbriar drought) will trigger garden construction without further code changes. Opened ticket 086 (`farm-canary-triggering-scenario`) to find such a scenario and re-promote the Crop features against it. Ticket 084 stays parked, re-blocked on 086.

**Lessons:**

1. **Empirical tuning beats theoretical calibration on calibration-sensitive systems.** The `BuildPressure` accumulator + decay dynamics require >85% gate duty cycle to clear `DECAY = 0.95` to the actionable threshold. Seed 42's food_fraction distribution (0.3-tail = 1%, 0.95-tail = 22%, 1.0-tail = 51%) makes any food-only threshold an awkward fit. The disjunctive gate is structurally the right shape; calibration is a separate question that can't be answered without empirical sweeps and survival-canary checks.
2. **Asymmetric build-vs-repurpose gates are intentional design.** The repurposing path can afford a hair-trigger predicate because the cost of being wrong is `crop_kind` flip + `growth = 0.0`. The build-pressure path needs a stricter predicate because the cost is permanent labor + placement slot. The strict supply-aware `herb_demand` reflects this principle in code.
3. **CLAUDE.md hard survival gates protect the project from "fixes that break the colony."** The v2-loose probe's regressions (courtship/wards/deaths) would have shipped under a less-rigorous gate. The four-artifact methodology + verdict canaries caught it before commit.
