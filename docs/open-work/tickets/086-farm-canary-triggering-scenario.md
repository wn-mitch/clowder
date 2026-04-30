---
id: 086
title: Find a triggering scenario for Farm DSE canary (CropTended / CropHarvested)
status: ready
cluster: balance
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [084-farm-herb-ward-demand.md, 085-gardens-multiuse-build-gate.md]
landed-at: null
landed-on: null
---

## Why

Tickets 084 and 085 closed the structural gaps for Farm DSE in healthy-colony regimes — 084 added a `farm_herb_pressure` axis (`scoring.rs:493–503`) so Farm scores above zero when wards are weak and thornbriar is absent, and 085 added a disjunctive build-pressure gate (`coordination.rs:984`) so a colony with healthy food *but* low ward stockpile builds a garden in the first place. Both axes are structurally correct (covered by unit tests).

Empirically: neither fires in the canonical seed-42 15-min release soak. The colony's natural dynamics never produce a sustained `(ward_strength_low ∧ !ThornbriarAvailable)` state because:

- Wild Thornbriar patches respawn cyclically; cats gather on the rare absence (≥6 GatherHerbCompleted per soak).
- Once *any* cat carries thornbriar in inventory, `any_cat_carrying_thornbriar = true` and the build-gate suppresses (085's strict supply check). Even loosening that to world-only `!wild_thornbriar_available` only hits the `farm_herb_pressure = 1.0` regime in narrow windows that don't overcome `BuildPressure::DECAY = 0.95` enough for `pressure.farming` to cross the actionable threshold.
- Loosening further to `herb_demand = ward_strength_low` alone breaks survival canaries (085 v2-loose probe: `courtship 764→0`, `wards_placed 5→1`, 4 wildlife-combat deaths) by redistributing cat-time from social/defense to construction.

So: the existing canary gate `Feature::CropTended` / `Feature::CropHarvested` is correctly demoted (`expected_to_fire_per_soak() => false`, ticket 083), but the underlying validation — "does Farm ever fire end-to-end in *some* sim regime?" — remains untested at the integration level. Unit tests cover the score wiring; only a soak that reaches the trigger conditions covers the full Farm-DSE → CropTended → CropHarvested → Thornbriar-spawn → ward-recovery loop.

## Scope

Find or construct a sim scenario where:

1. `pressure.farming` accumulates above the actionable threshold (food_demand or herb_demand sustained ≥85% of coordinator ticks).
2. A Garden ConstructionSite spawns and completes within the 15-min soak window.
3. `HasGarden` flips true; FarmDse becomes eligible.
4. `farm_herb_pressure = 1.0` at some point post-Garden (`ward_strength_low ∧ !ThornbriarAvailable`).
5. A cat scores Farm above competing actions and tends; `Feature::CropTended` ≥ 1.
6. A Thornbriar plot reaches `growth = 1.0`; `Feature::CropHarvested` ≥ 1.

Plus: hard gates hold (Starvation = 0, ShadowFoxAmbush ≤ 10), continuity canaries don't regress.

Candidate approaches:

- **Multi-seed sweep over {1, 7, 42, 100, 99, ... }.** The 085 gap-repro sweep (`logs/sweep-gap-repro/`) already showed seeds 1 and 100 had food_fraction much lower than seed 42 (mean 0.5–0.7 vs 0.95). Some seed may naturally trigger the gate.
- **Forced-weather soaks.** `--force-weather Storm` or `Fog` may destabilize wards faster (more decay, more siege) without perturbing food economy. Combined with a wild-herb-suppression weather, may force the `(ward_low ∧ !thornbriar)` regime.
- **Targeted constants override** for the test scenario (e.g., `ward_decay_rate × 2`, `wild_herb_spawn_rate × 0.1`) — *only as a probe*; the canary should re-promote against unmodified constants if a natural seed/weather combination works.

## Out of scope

- **Re-tuning `BuildPressure::DECAY` or `BASE_RATE`.** These affect every build axis (cooking, workshop, defense, farming) — broad surface, separate balance pass if ever.
- **Loosening `farm_herb_pressure` itself in `scoring.rs:493–503`.** Would touch 084's structural design. The axis is correct as-is; the gap is observability of its triggering regime.
- **Adding new Farm considerations.** 084 + 085 already give Farm the structural surface area it needs.
- **Weakening the canary gate (e.g., changing `expected_to_fire_per_soak`).** The whole point is to validate the canary against a passing soak, not to silence it.

## Approach

1. **Multi-seed sweep first.** `just sweep farm-canary-probe "" "1 7 42 99 100 314 2025" 1 900` against current HEAD. For each seed: count `Feature::CropTended` / `CropHarvested` from SystemActivation tail; identify any seed with ≥1 of each.
2. **If a seed fires naturally:** lock it in as a secondary canary seed (alongside seed 42 as the primary). Re-promote `Feature::CropTended` and `CropHarvested` to `expected_to_fire_per_soak() => true`; verify the seed-42 canary stays passing (Farm stays dormant when not needed) and the chosen seed canary fires. Document in `docs/balance/086-farm-canary-scenario.md`.
3. **If no seed fires naturally:** run forced-weather variants (`--force-weather Storm` × multi-seed) to identify a triggering combination. Document the weather-conditional canary semantics — i.e., "Farm is expected to fire under prolonged storm but not in clear weather." This may require a new canary classification (`expected_under_weather: Storm`) or a separate weather-canary wiring in `system_activation.rs`.
4. **If neither natural nor weather-forced triggers fire:** open a deeper investigation ticket. Possible root causes: ward decay too slow, wild thornbriar respawn too fast, gather DSE too eager. Each is its own balance question.

## Verification

- Hard gates hold on the chosen scenario: Starvation = 0, ShadowFoxAmbush ≤ 10.
- `Feature::CropTended` ≥ 1 and `Feature::CropHarvested` ≥ 1 in the chosen scenario.
- `Farming` PlanCreated count > 0 with the herb-demand axis providing the lift (verified via focal-cat trace showing `farm_herb_pressure = 1.0` at scoring time).
- Continuity canaries don't regress vs canonical seed-42 baseline on the chosen scenario.
- 084's acceptance gate passes on the chosen scenario.
- Re-promoted features survive the never-fired-canary check across the canonical seed AND the chosen scenario.

## Log

- 2026-04-30: Opened. Carved out from 084's continued parking after 085 landed the disjunctive build-pressure gate. 085's balance doc (`docs/balance/085-gardens-multiuse-build-gate.md ## Why P1–P3 are unchanged in seed 42`) captures the empirical evidence that seed 42 doesn't naturally exercise the Farm trigger regime.
