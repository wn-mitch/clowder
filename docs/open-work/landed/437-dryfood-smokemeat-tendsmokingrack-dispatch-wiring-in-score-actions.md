---
id: 437
title: DryFood / SmokeMeat / TendSmokingRack dispatch wiring in score_actions
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-21
---

## Why

`DryFoodDse`, `SmokeMeatDse`, and `TendSmokingRackDse` are registered in `populate_dse_registry` (`src/plugins/simulation.rs:31-33`, ticket 367 Commits 4 + 9) but never scored — `src/ai/scoring.rs::score_actions` is a hand-written dispatcher with one `score_dse_by_id(<id>, ...)` branch per `Action` variant, and the three Phase-1b preservation DSEs have no matching branches. The DSEs are constructed at startup, hold their full L2 considerations + eligibility filter, and are accessible by name via the registry; they simply never enter the per-cat scoring pass. Net effect: `Action::DryFood` / `Action::SmokeMeat` / `Action::TendSmokingRack` never appear in any cat's `last_scores`, never reach L3 softmax, never get elected. Confirmed by `rg -n 'dry_food\|smoke_meat\|tend_smoking_rack' src/ai/scoring.rs` returning zero hits. Isolated by ticket [436](436-drying-chain-microexperiment-scenario-verify-dryfood-dse-eligibility-scoring-in-isolation.md)'s scenario microexperiment — all three fixtures show the L2 score table missing the `dry_food` row entirely (not even as `eligible: false` — that capture path lives *inside* `score_dse_by_id`). Violates the 367 Phase-1b hard-gate target: `FoodLoadedOnDryingRack >= 1` over a 15-min seed-42 soak (post-Commit-9 soak `logs/tuned-42-5598499f` shows 0 fires across 108k post-rack-built ticks).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers (writer) | `src/systems/buildings.rs::update_colony_building_markers` | `HasFunctionalDryingRack` / `HasFunctionalSmokingRack` / `HasLoadedSmokingRackOffCooldown` / `HasDryableInStores` colony markers fire from rack + store-content scans | `[verified-correct]` (substrate-stub-check + marker-snapshot-wiring check pass; scenario 436's fixture 2 confirms rack-functional firing) |
| L1 markers (composite) | `src/systems/goap.rs::evaluate_and_plan` ~line 1981 | `HasDryableAccessible = has_dryable_inv OR (has_free_slot AND has_dryable_in_stores)` set per-cat | `[verified-correct]` (Commit 9 code; never read in production today because the DSE is never scored) |
| L2 DSE registration | `src/plugins/simulation.rs::populate_dse_registry` lines 31-33 | `dry_food_dse()` / `smoke_meat_dse()` / `tend_smoking_rack_dse()` pushed into `registry.cat_dses` | `[verified-correct]` |
| **L2 DSE dispatch** | **`src/ai/scoring.rs::score_actions`** | **`score_actions` must call `score_dse_by_id("dry_food", ctx, inputs, &mut scalars)` (and the two sibling calls) to make each DSE enter the L2/L3 pool. The function is a hand-written switch; no registry-iteration loop.** | **`[verified-defect]`** — no such calls exist; confirmed by grep. |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` lines 308-310 | `Action::DryFood → Some(DispositionKind::DryingFood)`; same for `SmokeMeat` → `SmokingMeat` and `TendSmokingRack` → `TendingSmokingRack` | `[verified-correct]` |
| Plan template (DryFood) | `src/ai/planner/actions.rs::drying_food_actions` | `[DropItem?, RetrieveDryable (×2 arms), DryFood]` — Commit 9 split-shape chain | `[verified-correct]` |
| Plan template (SmokeMeat) | `src/ai/planner/actions.rs::smoking_meat_actions` | Single-step `[SmokeMeat]` with `ZoneIs(SmokingRack)` precondition — assumes cat already carries raw meat + fuel | `[verified-correct]` (intentionally minimal; multi-ingredient retrieve mirror is 367 Commit 10 work, NOT this ticket) |
| Plan template (TendSmokingRack) | `src/ai/planner/actions.rs::tend_smoking_rack_actions` | Single-step `[TendSmokingRack]` with `ZoneIs(SmokingRack)` precondition | `[verified-correct]` |
| Completion proxies | `src/components/commitment.rs` | All three Dispositions use `TripsAtLeast(1)`; the final step's `IncrementTrips` effect satisfies | `[verified-correct]` |
| Resolvers | `src/steps/disposition/{load_drying_rack.rs,load_smoking_rack.rs,tend_smoking_rack.rs,retrieve_dryable_from_stores.rs}` | Resolvers exist and emit `FoodLoadedOnDryingRack` / `FoodLoadedOnSmokingRack` / `SmokingRackTended` etc. | `[verified-correct]` (Commits 4-9) |

## Fix candidates

**Parameter-level options** — N/A. The defect is structural (missing call sites). No marker / curve / weight change closes it.

**Structural options** (per Bugfix discipline):

- **R1 (extend — recommended)** — add three new branches to `score_actions` mirroring the existing `cook` branch (`src/ai/scoring.rs:2047-2056`). Each branch:
  1. Calls `score_dse_by_id("<id>", ctx, inputs, &mut scalars)`.
  2. Pushes `(Action::<Variant>, score + jitter)` onto `scores` when score > 0.
  The eligibility filter (`.require(CanDry/CanSmoke + HasFunctional...Rack + HasDryable...)`) gates internally, so the outer wrapper is a thin pass-through. No outer-gate `if`-guard needed (unlike Cook's `cook_hunger_gate`) because preservation is Maslow tier-2 buffer-building, not tied to a hunger threshold. Mechanical fix; one commit; matches the dispatch pattern every other DSE follows.

- **R2 (retire — out of scope, opens as separate ticket)** — retire the hand-written dispatcher entirely. `score_actions` iterates `inputs.registry.cat_dses`, calls `score_dse_by_id(dse.id().0, ...)`, and pushes `(dse_to_action(dse.id()), score)` via a single Action↔DSE-id mapping helper. Eliminates the entire class of "registered DSE never scores" silent failures by construction — `populate_dse_registry` becomes the single source of truth in spirit as well as in name. Larger blast radius (touches every branch + every gate + the Action↔DSE-id mapping) and requires careful preservation of branch-specific outer gates (Cook's `cook_hunger_gate`, Caretake's disjunction, the disposal-chronicity gates). Worth opening as a follow-on substrate-cleanup ticket once R1 lands.

- **R3 (split)** — N/A. No DSE shape needs splitting.

- **R4 (rebind)** — N/A. No Action↔Disposition mapping is wrong.

## Recommended direction

R1. Add three `score_dse_by_id` dispatch branches to `score_actions`, one each for `dry_food`, `smoke_meat`, `tend_smoking_rack`. Mirror the `cook` branch's shape (`src/ai/scoring.rs:2047-2056`) but without `cook`'s outer hunger gate (preservation has no equivalent precondition — eligibility is enough). Land as a single commit on this ticket (or as 367 Commit 11 if the user prefers to bundle with the 367 thread). The R2 (registry-iterating dispatcher) substrate cleanup opens as a separate ticket post-landing.

## Out of scope

- R2 (registry-iterating dispatcher) — surface as a follow-on substrate-cleanup ticket once R1 lands. Listed under the "Antipattern migration follow-ups are non-optional" discipline.
- Smoking-side multi-ingredient retrieve mirror (367 Commit 10): `smoking_meat_actions` currently assumes the cat already holds raw meat + fuel. The dispatch fix lands the DSE in the L2 pool, but `SmokeMeat` will rarely fire colony-scale until Commit 10's multi-ingredient retrieve arms land. That's deliberate per 367's commit cadence; this ticket only restores the *ability* to score, not the empirical fire-rate.
- `NearestDryingRack` / `NearestSmokingRack` spatial-anchor landing — the `LandmarkAnchor::NearestKitchen` stub in `dry_food.rs:97` (and equivalent in smoke_meat/tend_smoking_rack) is acknowledged debt awaiting follow-on work per Commit 4's doc-comment.

## Verification

- `cargo test --release --lib scenarios::drying_chain_eligibility` — the two currently-`#[ignore]`d tests (`hot_inventory_makes_dry_food_eligible` and `stores_has_dryable_makes_dry_food_eligible_via_composite`) flip from panic to passing. Remove the `#[ignore]` lines in the same commit. The third test (`empty_stores_filters_dry_food`) stays passing as the negative control.
- `just scenario drying_chain_hot_inventory` — `dry_food` row appears in the L2 score-column table with `eligible: true` and a non-zero `final_score`.
- `just scenario drying_chain_stores_has_dryable` — same: `dry_food` row appears with `eligible: true` via the composite-marker path.
- Final colony-scale verification: `just soak-trace 42 Simba` produces `FoodLoadedOnDryingRack >= 1` and `FoodDried >= 1` in the footer's `never_fired_expected_positives` list (i.e., they're absent — they fired). Same exit criterion as 367's Phase-1b hard gate.
- `just verdict logs/tuned-42-<commit>` — survival canaries hold (Starvation = 0; ShadowFox ambush <= 10).

## Log
- 2026-05-21: opened from ticket 436's layer-walk audit. The scenario microexperiment isolated a dispatch-layer defect upstream of every `[suspect]` row in 436's original audit — `score_actions` never calls `score_dse_by_id("dry_food", ...)` despite the DSE being registered. Same gap silences `SmokeMeatDse` and `TendSmokingRackDse`. Fix is R1 (3 mechanical dispatch branches mirroring `cook`'s shape); R2 (registry-iterating dispatcher) surfaces as a separate substrate-cleanup ticket post-landing.
- 2026-05-21: 2026-05-21: landed. Three score_dse_by_id branches added to score_actions (no outer gate — eligibility filter is enough). Adjacent fix: scenarios::preset::CatPreset::adult had a stale born_tick=0 + ticks_per_season=1000 comment, but the default is 20000, so 'adult' preset cats actually read as Elder at start_tick. Adult-gated capabilities (CanDry, CanCook) were silently false; fixture 2's eligibility wouldn't have passed without the preset fix even with dispatch wired. The two previously #[ignore]'d scenario tests now pass as the regression gate; full lib + integration test suites green. Opens [[438]] (retire hand-written dispatcher; iterate DseRegistry instead) blocked-by 437.
