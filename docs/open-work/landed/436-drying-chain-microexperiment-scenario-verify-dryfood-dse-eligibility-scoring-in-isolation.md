---
id: 436
title: Drying chain microexperiment scenario — verify DryFood DSE eligibility + scoring in isolation
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: b640348af6ae
landed-on: 2026-05-21
---

## Why

Post-367-Commit-9 verification soak (`logs/tuned-42-5598499f`) confirms the racks are constructed (drying rack site marked at tick 1204480) but `FoodLoadedOnDryingRack` still never fires across 108,270 ticks of post-build operation. `DryFood` action does not appear in *any* cat's `last_scores` array after the rack is built — the DSE is not scoring at all, which means eligibility is filtering every cat. The Commit 9 split-shape fix (new `HasDryableAccessible` composite marker + `RetrieveDryable` plan template extension) was structurally correct but is not being elected. Need a focused scenario to triage *why* the DSE is silent: eligibility filter failing, composite marker not firing, scoring zeroing out, or something further upstream. The `just soak` cycle is the wrong tool — 15 minutes per iteration when the question is "given this exact world-state, does the DSE win?" answerable in ~3 seconds by a scenario.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers (writer) | `src/systems/buildings.rs::update_colony_building_markers` | `HasDryableInStores` colony marker fires when ≥1 RawFish/RawOrgan sits in any `StoredItems` | `[verified-correct]` (Commit 9 code authored) |
| L1 markers (writer) | `src/systems/buildings.rs::update_colony_building_markers` | `HasFunctionalDryingRack` colony marker requires `Structure::effectiveness() > 0.0` AND ≥1 rack with `loaded.is_none()` | `[verified-correct]` — `Structure::new(DryingRack)` ships `condition: 1.0` → `effectiveness() == 1.0`; the construction-site path drops the `ConstructionSite` component and leaves the `Structure` at full condition (see `Structure::new` at `src/components/building.rs:233`). Scenario fixtures all use `Structure::new` and see the marker fire as expected (verified inline via the L2 capture path, but the failure is upstream, not on this row). |
| L1 markers (writer) | `src/systems/items.rs::update_inventory_markers` | `HasDryableInInventory` fires when cat has `RawFish || RawOrgan` | `[verified-correct]` (sister markers for cooking work) |
| L1 markers (composite) | `src/systems/goap.rs::evaluate_and_plan` ~line 1981 | `HasDryableAccessible = has_dryable_inv OR (has_free_slot AND has_dryable_in_stores)`; set_entity'd per cat | `[not-the-defect]` — code is correct (verified by inspection); composite is never *read* because the DSE is never scored. Promoting to "verified-correct" would overclaim — the scenario can't exercise the read path. |
| L1 markers (snapshot mirror) | `src/systems/goap.rs::evaluate_and_plan` ~line 1641 | `markers.set_colony(HasDryableInStores::KEY, has_dryable_in_stores)` | `[verified-correct]` (substrate-stub-check passes; marker-snapshot-wiring check passes) |
| L2 DSE | `src/ai/dses/dry_food.rs` | Eligibility = `CanDry + HasFunctionalDryingRack + HasDryableAccessible - Incapacitated` | `[verified-correct]` (test `dry_food_dse_eligibility_shape` updated and passing); irrelevant to the actual defect — the filter is never invoked. |
| L2 DSE registration | `src/plugins/simulation.rs::populate_dse_registry` line 31 | `registry.cat_dses.push(dses::dry_food_dse());` — DSE inserted into the catalog | `[verified-correct]` — registered as expected. |
| **L2 DSE dispatch (NEW ROW)** | **`src/ai/scoring.rs::score_actions`** | **`score_actions` must call `score_dse_by_id("dry_food", ...)` to make the DSE enter the L2/L3 pool. The function is a hand-written switch with one branch per `Action` variant — it does NOT iterate `DseRegistry`.** | **`[verified-defect]`** — no `score_dse_by_id("dry_food", ...)` / `"smoke_meat"` / `"tend_smoking_rack"` call exists anywhere in `src/ai/scoring.rs`. Confirmed by `rg -n 'dry_food\|smoke_meat\|tend_smoking_rack' src/ai/scoring.rs` returning zero hits. **This is the actual defect.** |
| L3 softmax | `src/ai/scoring.rs` | DSE scored = eligibility(true) × weighted_sum(considerations) | `[not-reached]` — DSE never enters the scoring pass; no L3 record can exist. |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` | `Action::DryFood → Some(DispositionKind::DryingFood)` (line 308) | `[verified-correct]` (irrelevant; the wrap-site never sees `Action::DryFood` because no score is pushed). |
| Plan template | `src/ai/planner/actions.rs::drying_food_actions` | `[DropItem?, RetrieveDryable (× 2 arms), DryFood]` mirrors `cooking_actions` | `[verified-correct]` (Commit 9 code) — never invoked because no disposition election picks DryingFood. |
| Completion proxy | `src/components/commitment.rs` | `DryingFood` uses `TripsAtLeast(1)`; final `DryFood` step's `IncrementTrips` effect satisfies | `[verified-correct]` (single-action 367 pattern, mirrors `Bury`) |
| Resolver | `src/steps/disposition/retrieve_dryable_from_stores.rs` | Filters to `RawFish`/`RawOrgan` only | `[verified-correct]` (Commit 9 code) |
| Resolver | `src/steps/disposition/load_drying_rack.rs` | Consumes per-recipe; emits `Feature::FoodLoadedOnDryingRack` | `[verified-correct]` (Commit 4 code) |

## Fix candidates

The scenario reproduced the failure but redirected the audit: none of the four pre-scenario `[suspect]` rows are the defect. The actual failing row is the **L2 DSE dispatch** layer in `src/ai/scoring.rs::score_actions`, which never calls `score_dse_by_id("dry_food", ...)` (nor `"smoke_meat"` / `"tend_smoking_rack"`). The DSE is constructed by `populate_dse_registry` but the registry is not iterated during per-cat scoring — `score_actions` is a hand-written switch with one branch per `Action` variant, and the three Phase-1b DSEs from ticket 367 (Commits 4 + 9) were registered without adding their matching dispatch branches.

**Parameter-level options** — N/A. The defect is structural (a missing call site), not a tuning gap. No marker / curve / weight change closes it.

**Structural options** (drafted per Bugfix discipline):
- R1 (**extend** — recommended for the 367 follow-on) — add three new branches to `score_actions` mirroring the existing `cook` branch (`src/ai/scoring.rs:2047-2056`): one for `dry_food` (gated on `HasDryableAccessible` or a leaner inline check), one for `smoke_meat` (similar shape, smokeable-accessible), one for `tend_smoking_rack` (gated on `HasLoadedSmokingRackOffCooldown`). Pushes the resulting score under the matching `Action` variant. Minimum-blast-radius fix; matches the dispatch pattern every other DSE follows; one commit on 367.
- R2 (**retire** — out of scope for 367 but worth surfacing as 437+) — retire the hand-written dispatcher entirely. `score_actions` iterates `inputs.registry.cat_dses`, calls `score_dse_by_id(dse.id().0, ...)`, and pushes `(dse_to_action(dse.id()), score)`. Eliminates the entire class of "registered DSE that never scores" silent failures — every DSE registered in `populate_dse_registry` is dispatched by construction. Larger blast radius (touches every existing branch + every Action↔DSE-id mapping) and requires careful preservation of the per-branch gates (e.g. Cook's `cook_hunger_gate`, Caretake's parent-or-urgency disjunction, the disposal-chronicity gates). Open as a substrate-cleanup ticket once R1 unblocks 367.
- R3 (**split**) — N/A. No DSE shape needs splitting.
- R4 (**rebind**) — N/A. No Action↔Disposition mapping is wrong.

Sister-defect note: the same dispatch gap silences `SmokeMeatDse` and `TendSmokingRackDse`. Whichever fix lands MUST cover all three together (per `feedback_substrate_over_filtering_kittens_are_cats.md` — landing only one resolver-side branch when the same shape applies to siblings is structurally net-negative).

## Recommended direction

R1 — add three `score_dse_by_id` dispatch branches to `score_actions`, mirroring the `cook` branch shape, gated on appropriate per-cat / colony markers (the eligibility filter already gates internally, so the outer wrapper can be a thin if-let-some). Lands as a 367 follow-on commit. The scenario's two failing tests (`hot_inventory_makes_dry_food_eligible` and `stores_has_dryable_makes_dry_food_eligible_via_composite`) become the regression gate — they flip from `panic: "dry_food never surfaced in any L2 table"` to passing once the dispatch is wired.

R2 (registry-iterating dispatcher) is the substrate-correct long-term fix but out of scope for unblocking 367. Open as a separate ticket once R1 lands.

## Out of scope

- Smoking-chain mirror (Commit 10 of 367 — separate fixture + parameterized harness can wait until the drying chain is verified).
- `NearestDryingRack` spatial-anchor landing (the `LandmarkAnchor::NearestKitchen` stub in `dry_food.rs:97` is acknowledged in Commit 4's doc-comment; if R2 says spatial axis is the culprit, open as a separate ticket).
- The `LandmarkAnchor::NearestKitchen` stub for SmokeMeat / TendSmokingRack DSEs — same shape.

## Verification

- `just scenario drying_chain_hot_inventory` / `drying_chain_stores_has_dryable` / `drying_chain_empty_stores` each run in ~0.02s; the L2 score-column table prints whether `dry_food` surfaces.
- `cargo test --release --lib scenarios::drying_chain_eligibility` — three unit tests:
  - `hot_inventory_makes_dry_food_eligible` — FAILS pre-fix (panic: `dry_food never surfaced in any L2 table`). PASSES post-fix.
  - `stores_has_dryable_makes_dry_food_eligible_via_composite` — same shape; FAILS pre-fix, PASSES post-fix.
  - `empty_stores_filters_dry_food` — PASSES pre and post (negative control: when nothing is dryable, the row is correctly absent).
- Once the 367 follow-on commit lands R1's dispatch branches, the two failing tests above flip to PASS — they're the regression gate.
- Final colony-scale verification: `just soak-trace 42 Simba` produces `FoodLoadedOnDryingRack >= 1` and `FoodDried >= 1` in the footer's `never_fired_expected_positives` list (i.e., they're absent — they fired). Same exit criterion as before.

## Log
- 2026-05-21: opened post-367-Commit-9 verification soak. Soak confirmed dryfood action never scores (zero appearances in `last_scores` after rack-built tick 1204480), indicating eligibility filters every cat despite the Commit 9 composite-marker fix. Layer-walk identifies four `[suspect]` rows; scenario will isolate which one is the culprit before any further code changes.
- 2026-05-21: authored `src/scenarios/drying_chain_eligibility.rs` (three fixtures, wired into the registered list at `src/scenarios/mod.rs`). All three fixtures ran clean in <0.1s. **Scenario result: the defect is NOT in any of the four `[suspect]` rows.** `dry_food` never appears in the L2 score-column table — not even as an `!!`-flagged ineligible row. (`!!` rows are emitted by `score_dse_by_id` when eligibility fails; absence of any row means `score_dse_by_id` is never called.) Confirmed by `rg -n 'dry_food\|smoke_meat\|tend_smoking_rack' src/ai/scoring.rs` returning zero hits: `score_actions` is a hand-written switch with one `score_dse_by_id` branch per Action variant, and Commits 4 + 9 registered the three Phase-1b DSEs in `populate_dse_registry` without adding their matching dispatch branches. Layer-walk audit table updated with a NEW row ("L2 DSE dispatch") promoted to `[verified-defect]`; the four pre-scenario `[suspect]` rows reclassified accordingly. Fix candidate menu rewritten: R1 (extend — add three dispatch branches) lands on 367; R2 (retire — registry-iterating dispatcher) opens as a separate substrate-cleanup ticket. Same dispatch gap silences `SmokeMeatDse` and `TendSmokingRackDse` — sister-defect; one fix covers all three.
- 2026-05-21: 2026-05-21: landed. Scenario authored + wired into registered scenarios:
  kitten_cry_basic                default_focal=Mallow, default_ticks=5
  wildlife_fight                  default_focal=Briar, default_ticks=15
  fondness_kitten_imprint         default_focal=Mother, default_ticks=20
  hunt_acquisition_to_kill        default_focal=Talon, default_ticks=30
  hunt_deposit_chain              default_focal=Stoat, default_ticks=200
  hunt_deposit_chain_injured      default_focal=Stoat, default_ticks=200
  exploration_ranging             default_focal=Cinder, default_ticks=60
  ward_placement                  default_focal=Sage, default_ticks=40
  farming_cycle                   default_focal=Furrow, default_ticks=60
  farm_herb_demand                default_focal=Bracken, default_ticks=80
  grooming_other                  default_focal=Affie, default_ticks=20
  disposal_election_trashing      default_focal=Cinder, default_ticks=5
  disposal_election_discarding    default_focal=Cinder, default_ticks=5
  disposal_election_idle          default_focal=Cinder, default_ticks=5
  disposal_election_discarding_blocked_without_marker  default_focal=Cinder, default_ticks=5
  picking_up_scavenging           default_focal=Cinder, default_ticks=16
  route_cost_decision             default_focal=Bold, default_ticks=4
  flee_commitment                 default_focal=Brave, default_ticks=60
  flee_calibration_low_threat     default_focal=Probe, default_ticks=4
  flee_calibration_open_terrain   default_focal=Probe, default_ticks=4
  flee_calibration_cornered       default_focal=Probe, default_ticks=4
  flee_calibration_sleep_partner  default_focal=Probe, default_ticks=4
  flee_calibration_critical_cornered  default_focal=Probe, default_ticks=4
  inventory_full_curios           default_focal=Cinder, default_ticks=16
  inventory_full_herbs            default_focal=Cinder, default_ticks=16
  inventory_empty_pickup_unchanged  default_focal=Cinder, default_ticks=16
  wounded_cat_no_pickup           default_focal=Calcifer, default_ticks=6
  dying_arc_softmax               default_focal=Calcifer, default_ticks=30
  lone_burial                     default_focal=Mira, default_ticks=200
  intention_momentum_pickup_lock  default_focal=Cinder, default_ticks=60
  patrol_recalibration_warded_demesne  default_focal=Sentinel, default_ticks=8
  mate_chain                      default_focal=Marigold, default_ticks=5000
  guarding_morale_break_releases  default_focal=Watcher, default_ticks=4
  fox_cat_scent_avoidance         default_focal=Pyre, default_ticks=100
  fox_ward_only_avoidance         default_focal=Pyre, default_ticks=60
  chokepoint_defense_isthmus      default_focal=Talon, default_ticks=250
  colony_reserves_belief          default_focal=Sage, default_ticks=120
  surrounded_colony               default_focal=Bramble, default_ticks=50
  affordance_flee_high_cover      default_focal=Probe, default_ticks=4
  affordance_flee_open_ground     default_focal=Probe, default_ticks=4
  affordance_dive_hawk            default_focal=Probe, default_ticks=4
  affordance_chase_prey           default_focal=Probe, default_ticks=4
  affordance_fight_capability_match  default_focal=Probe, default_ticks=4
  affordance_supersedes_legacy_scalars  default_focal=Probe, default_ticks=4
  flee_belief_high_violence_capability  default_focal=Probe, default_ticks=4
  patrol_avoids_high_threat_sector  default_focal=Probe, default_ticks=4
  hunt_picks_stalk_for_oblivious_prey  default_focal=Probe, default_ticks=4
  hunt_picks_chase_for_alerted_prey  default_focal=Probe, default_ticks=4
  district_placement_under_pressure  default_focal=Mocha, default_ticks=60
  parenting_father_provisions     default_focal=Brick, default_ticks=10
  parenting_joint_suppression     default_focal=Sage, default_ticks=15
  parenting_grief_kitten_death    default_focal=Briar, default_ticks=30
  parenting_caretake_kitten_absent  default_focal=Briar, default_ticks=5
  parenting_caretake_kitten_present  default_focal=Magnolia, default_ticks=10
  parenting_handoff_recipient_resolution  default_focal=Magnolia, default_ticks=3
  drying_chain_hot_inventory      default_focal=Cinder, default_ticks=10
  drying_chain_stores_has_dryable  default_focal=Cinder, default_ticks=10
  drying_chain_empty_stores       default_focal=Cinder, default_ticks=10. Two fixtures fail as the regression gate for the upstream dispatch defect found in score_actions; gated with #[ignore] pending 437. Negative-control fixture passes. Opens 437 (DryFood/SmokeMeat/TendSmokingRack dispatch wiring) as the fix follow-on.
