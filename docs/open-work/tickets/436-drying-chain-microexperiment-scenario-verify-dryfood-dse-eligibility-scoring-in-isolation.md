---
id: 436
title: Drying chain microexperiment scenario — verify DryFood DSE eligibility + scoring in isolation
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Post-367-Commit-9 verification soak (`logs/tuned-42-5598499f`) confirms the racks are constructed (drying rack site marked at tick 1204480) but `FoodLoadedOnDryingRack` still never fires across 108,270 ticks of post-build operation. `DryFood` action does not appear in *any* cat's `last_scores` array after the rack is built — the DSE is not scoring at all, which means eligibility is filtering every cat. The Commit 9 split-shape fix (new `HasDryableAccessible` composite marker + `RetrieveDryable` plan template extension) was structurally correct but is not being elected. Need a focused scenario to triage *why* the DSE is silent: eligibility filter failing, composite marker not firing, scoring zeroing out, or something further upstream. The `just soak` cycle is the wrong tool — 15 minutes per iteration when the question is "given this exact world-state, does the DSE win?" answerable in ~3 seconds by a scenario.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers (writer) | `src/systems/buildings.rs::update_colony_building_markers` | `HasDryableInStores` colony marker fires when ≥1 RawFish/RawOrgan sits in any `StoredItems` | `[verified-correct]` (Commit 9 code authored) |
| L1 markers (writer) | `src/systems/buildings.rs::update_colony_building_markers` | `HasFunctionalDryingRack` colony marker requires `Structure::effectiveness() > 0.0` AND ≥1 rack with `loaded.is_none()` | `[suspect]` — does effectiveness exceed 0.0 immediately after build? `condition: 0.0` at site spawn (line 1413) suggests rack is built at full condition by `resolve_construct`, but unverified post-build |
| L1 markers (writer) | `src/systems/items.rs::update_inventory_markers` | `HasDryableInInventory` fires when cat has `RawFish || RawOrgan` | `[verified-correct]` (sister markers for cooking work) |
| L1 markers (composite) | `src/systems/goap.rs::evaluate_and_plan` ~line 1981 | `HasDryableAccessible = has_dryable_inv OR (has_free_slot AND has_dryable_in_stores)`; set_entity'd per cat | `[suspect]` — fired in code but never verified to ACTUALLY equal `true` for any cat |
| L1 markers (snapshot mirror) | `src/systems/goap.rs::evaluate_and_plan` ~line 1641 | `markers.set_colony(HasDryableInStores::KEY, has_dryable_in_stores)` | `[verified-correct]` (substrate-stub-check passes; marker-snapshot-wiring check passes) |
| L2 DSE | `src/ai/dses/dry_food.rs` | Eligibility = `CanDry + HasFunctionalDryingRack + HasDryableAccessible - Incapacitated` | `[verified-correct]` (test `dry_food_dse_eligibility_shape` updated and passing) |
| L2 DSE | `src/ai/dses/dry_food.rs` | DSE registered in `populate_dse_registry` | `[verified-correct]` (line 31 of `src/plugins/simulation.rs`) |
| L3 softmax | `src/ai/scoring.rs` | DSE scored = eligibility(true) × weighted_sum(considerations) | `[suspect]` — never reached if eligibility filters |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` | `Action::DryFood → Some(DispositionKind::DryingFood)` (line 308) | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs::drying_food_actions` | `[DropItem?, RetrieveDryable (× 2 arms), DryFood]` mirrors `cooking_actions` | `[verified-correct]` (Commit 9 code) — A* may reject if no valid path exists |
| Completion proxy | `src/components/commitment.rs` | `DryingFood` uses `TripsAtLeast(1)`; final `DryFood` step's `IncrementTrips` effect satisfies | `[verified-correct]` (single-action 367 pattern, mirrors `Bury`) |
| Resolver | `src/steps/disposition/retrieve_dryable_from_stores.rs` | Filters to `RawFish`/`RawOrgan` only | `[verified-correct]` (Commit 9 code) |
| Resolver | `src/steps/disposition/load_drying_rack.rs` | Consumes per-recipe; emits `Feature::FoodLoadedOnDryingRack` | `[verified-correct]` (Commit 4 code) |

## Fix candidates

**Parameter-level options** — only viable once a `[suspect]` row is promoted; current evidence points at one of the suspects rather than a tuning gap:
- R1 — eligibility tuning. If the scenario shows DryFood scores correctly when manually wired with all markers true, but `HasDryableAccessible` is silently false in production, the bug is in the composite-marker computation. Inspect `inventory.is_full()` projection — kittens vs adults? Slot-count mismatch?
- R2 — scoring tuning. If eligibility passes but the DSE scores below other DSEs (e.g., Cook outranking DryFood), the issue is composition weights or the spatial axis pointing at `NearestKitchen` (line 97 of `dry_food.rs` — known stub awaiting `NearestDryingRack` anchor per Commit 4 doc).
- R3 — rack effectiveness gating. If the scenario shows `HasFunctionalDryingRack` is off post-build (rack condition starts at 0.0 per construction site), the writer at `buildings.rs:689` (`bldg_state.has_functional_drying_rack && drying_racks.iter().any(|s| s.loaded.is_none())`) needs auditing — maybe `effectiveness() > 0.0` is the gate that fails.

**Structural options** (drafted per Bugfix discipline):
- R4 (**extend**) — extend the scenario approach itself. Multiple scenarios per DSE family (DryFood, SmokeMeat, TendSmokingRack) ride on a shared `preservation_chain_scenario` harness with parameterized fixtures (inventory has dryable / inventory empty + stores has dryable / inventory full + stores has dryable). Each scenario asserts which DSE wins in each fixture. Closer to TDD for the substrate.
- R5 (**split**) — split the DryFood DSE into two: `DryFoodFromInventory` (eligibility narrow on `HasDryableInInventory`, single-step plan `[DryFood]`) and `DryFoodFromStores` (eligibility wide on `HasDryableAccessible`, multi-step plan with retrieve prefix). Cleaner per-DSE trace but doubles the catalog; defer unless the composite marker proves to be a recurring footgun.
- R6 (**rebind**) — keep DryFoodDse, retire `HasDryableAccessible` as a separate marker, and bake the OR-logic into the EligibilityFilter via a new `require_any` primitive. Substrate-level — would obsolete the composite marker pattern across the codebase (precedent: `CanWardFromSupply` is also a composite). Out of scope for 436; surface as 437 if the audit motivates it.
- R7 (**retire**) — N/A; the substrate has a load-bearing job (367's hypothesis depends on it).

## Recommended direction

R4 (extend) — author `src/scenarios/drying_chain_eligibility.rs` per the existing `hunt_deposit_chain.rs` precedent. Fixtures:

1. **Hot inventory** — 1 adult cat with `RawFish` in slot 0, functional+idle DryingRack at (5,5), cat at (5,5). Expected: DryFood wins. Validates the "cat already has dryable" path.
2. **Empty inventory + stores has dryable** — 1 adult cat with empty inventory, Stores with RawFish, functional+idle DryingRack adjacent. Expected: DryFood wins via `[RetrieveDryable, DryFood]` chain. Validates the composite marker path (the Commit 9 split-shape fix).
3. **Empty inventory + empty stores** — 1 adult cat, empty Stores, functional rack. Expected: DryFood is NOT in the score table (eligibility filtered).

For each fixture, the scenario prints the focal cat's top-5 ranked DSEs per tick for the first ~50 ticks. If fixture 2 doesn't show DryFood at the top, the scenario isolates the failure (eligibility on which sub-marker?) without 15-min soak feedback.

After the scenario reproduces the failure, *then* apply parameter-level tuning (R1/R2/R3). The user signaled "we can tune from there" — meaning the scenario unblocks tuning, not the other way around.

## Out of scope

- Smoking-chain mirror (Commit 10 of 367 — separate fixture + parameterized harness can wait until the drying chain is verified).
- `NearestDryingRack` spatial-anchor landing (the `LandmarkAnchor::NearestKitchen` stub in `dry_food.rs:97` is acknowledged in Commit 4's doc-comment; if R2 says spatial axis is the culprit, open as a separate ticket).
- The `LandmarkAnchor::NearestKitchen` stub for SmokeMeat / TendSmokingRack DSEs — same shape.

## Verification

- `just scenario drying_chain_eligibility` runs all three fixtures in <5s; each asserts the expected winning DSE.
- Once scenario isolates the failure mode, follow-on commit(s) on 367 apply the fix.
- Final verification: `just soak-trace 42 Simba` produces `FoodLoadedOnDryingRack >= 1` and `FoodDried >= 1` in the footer's `never_fired_expected_positives` list (i.e., they're absent — they fired).

## Log
- 2026-05-21: opened post-367-Commit-9 verification soak. Soak confirmed dryfood action never scores (zero appearances in `last_scores` after rack-built tick 1204480), indicating eligibility filters every cat despite the Commit 9 composite-marker fix. Layer-walk identifies four `[suspect]` rows; scenario will isolate which one is the culprit before any further code changes.
