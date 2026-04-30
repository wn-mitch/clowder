---
id: 083
title: L2 PairingActivity Farming dormancy reconciliation
status: done
cluster: null
landed-at: 1265400
landed-on: 2026-04-29
---

# L2 PairingActivity Farming dormancy reconciliation

**Why:** Ticket 082's L2 PairingActivity activation soak showed Farming PlanCreated dropping 448 → 0 vs the post-072 baseline. The original ticket framed it as a Bevy 0.18 topological-sort reshuffle that needed a `.before/.after` pin or DSE filter cleanup.

**Diagnosis correction.** The framing was mechanically wrong on two grounds:

1. Chain 2a's marker batch is wrapped in `.chain()` at `src/plugins/simulation.rs:378`, which enforces source order — the existing balance doc (`docs/balance/027-l2-pairing-activity.md:12`) had already corrected this on 2026-04-29 morning.
2. The simulation pins a single-threaded executor for determinism (`SimulationPlugin::build`), so cross-system topological ambiguity isn't a possible variable.

The first ~65k ticks of the regression run (`logs/tuned-42-082-pairing-active-farming-regress/`) are **byte-identical** to the post-072 baseline while `PairingIntentionEmitted = 0`, which empirically rules out scheduler-shift effects.

**Actual mechanism — balance cascade through food economy.** Pairing first fires at tick 1265400 (~30k ticks before Farming's first fire in the baseline). From that point the food economy diverges:

| Metric | Pre-072 baseline | Post-082 (pairing on) |
|---|---|---|
| food_fraction median | 0.96 | **0.98** |
| food_fraction mean | 0.83 | **0.94** |
| FoodCooked | 227k | 255k (+12%) |
| FoodEaten | 138k | 165k (+20%) |
| PreyKilled | 514 | 895 (+74%) |

Farm DSE is `CompensatedProduct(food_scarcity, diligence, garden_distance)`. With median `food_fraction ≈ 0.98`, `food_scarcity = (1 - 0.98)² ≈ 0.0004` correctly gates Farm dormant. **Farming silence under abundant food is intended ecology**, not a regression.

**What landed:**

1. **`src/plugins/simulation.rs`** — uncommented the `crate::ai::pairing::author_pairing_intentions` schedule edge at chain 2a's marker batch (between `update_mate_eligibility_markers` and `update_capability_markers`). Collapsed the deferral-block comment to a one-line activation pointer that documents the actual mechanism (food-economy lift, not topological reshuffle) and references ticket 084 for the herb/ward-demand follow-on.
2. **`src/resources/system_activation.rs`** — `expected_to_fire_per_soak()` reconciliation:
   - Promoted `PairingIntentionEmitted` to `=> true` (canary now validates the L2 trunk fires; soak shows 14651 emissions).
   - `PairingBiasApplied` stays at `=> false` per the balance doc's "P3 untestable single-seed" — promote when ticket 082's multi-seed sweep clears.
   - Demoted `CropTended` and `CropHarvested` to `=> false`. Rationale: the silent-dead-farming class of bug is now caught by Phase 5a's `record_if_witnessed` discipline + step-resolver tests on `tend.rs`/`harvest.rs`, not by a runtime canary. Ticket 084 owns re-promotion once Farm reads herb/ward demand.
   - Updated `expected_to_fire_per_soak_classification` test and the two `never_fired_expected_positives_*` tests to match.
3. **`docs/balance/027-l2-pairing-activity.md`** — appended `## Activation observation (2026-04-29, ticket 083)` block with the post-hardening soak's footer table, diagnosis, canary-reconciliation rationale, and the herb/ward-demand follow-on (ticket 084). Concordance update flips P4a (Starvation) from "regression-confirmed" to "pass".
4. **`docs/open-work/tickets/084-farm-herb-ward-demand-axis.md`** — opened the herb-pressure follow-on so Farm DSE eventually motivates Thornbriar plots even with food stockpiles full.

**Verification — soak `logs/tuned-42-083/`:**

- `Starvation = 0` ✓ (hard gate)
- `ShadowFoxAmbush = 5` ≤ 10 ✓ (hard gate)
- `PairingIntentionEmitted = 14651` ✓ (L2 trunk live)
- `PairingDropped = 14650` (1:1 oscillation flagged for ticket 082's drop-cadence multi-seed look)
- four pass continuity canaries (grooming=61, play=254, courtship=764, mythic-texture=51) each ≥ 1 ✓
- mentoring/burial = 0 (pre-existing zeros tracked separately, not load-bearing for 083)
- `CropTended` / `CropHarvested` not in `never_fired_expected_positives` ✓ (canary reconciliation worked)
- `Farming` PlanCreated = 0 (intended dormancy under abundant-food ecology; ticket 084 owns the re-motivation lever)
- `cargo test --lib` all 1618 tests pass.

**Surprise.** The original ticket's 4-hypothesis frame (topological shift, snapshot race, component-presence filter, scoring-access conflict) was the wrong shape entirely — every one was empirically falsifiable in 30 minutes by reading the byte-identical first ~65k ticks of the existing 082 reproducer log. The right framing was always "where does the 0.98-vs-0.96 food_fraction lift come from?" The earlier balance doc had already gotten there independently and parked the wrong-framing in writing; the ticket's draft language hadn't caught up.

---
