---
id: 439
title: Planner zone resolver returns no reachable target for DryingRack / SmokingRack — surfaced post-437
status: blocked
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-21
parked: null
blocked-by: [437]
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Post-437 verification soak (`logs/tuned-42-44a82ba0`, commit `a84e0ccb`) confirms the dispatch fix landed: `DryFoodDse` / `SmokeMeatDse` / `TendSmokingRackDse` are now scored, elected, and reach the planner. But the next layer down fails: `plan_failures_by_reason` now shows **1095 × `TravelTo(DryingRack): no reachable zone target`** and **719 × `TravelTo(SmokingRack): no reachable zone target`** over 113,139 ticks (~1.6% combined plan-failure rate). The Phase-1b hard-gate from ticket 367 (`FoodLoadedOnDryingRack >= 1`, `FoodDried >= 1`) still fails — both features remain in `never_fired_expected_positives` despite the dispatch firing correctly. The footer-derived `colony_score` drift surfaces as a verdict-fail (aggregate +22%, structures_built +200%, shelter -100%, fulfillment -33%, kittens_born +50%, mythic-texture canary = 0) — these are the expected consequences of cats committing to plans that A* can't complete: replans churn through L3, the time budget for other DSEs (Cook, Forage, courtship, mythic-texture rare events) compresses. Classic "fix one layer, surface next" pattern — the dispatch fix unblocked the chain, the zone resolver is the next failing link.

## Hot context

- Failing run: `logs/tuned-42-44a82ba0/` (commit `a84e0ccb`, seed 42, 113,139 elapsed ticks, 0 deaths)
- Verdict: `fail`. survival canary `fail`, continuity canary `fail:mythic-texture=0`
- Top plan failures: `EngagePrey: lost prey during approach (1860)` (unchanged), `TravelTo(DryingRack): no reachable zone target (1095)` (NEW), `TravelTo(SmokingRack): no reachable zone target (719)` (NEW)
- never_fired_expected_positives: `[FoodLoadedOnDryingRack, MeatLoadedOnSmokingRack, SmokingRackTended, FoodDried, MeatSmoked]` — same five as pre-437; the smoking-side three are deferred to 367 Commit 10, but `FoodLoadedOnDryingRack` + `FoodDried` should fire now and do not.
- A snapshot at tick 1200000+1100 shows `BuildingConstructed: 10` cumulative — buildings ARE being built; need to drill into whether DryingRack / SmokingRack are among them and whether their positions are reachable from the cats that elect DryFood.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/systems/buildings.rs::update_colony_building_markers` | `HasFunctionalDryingRack` colony marker fires when ≥1 rack has `effectiveness() > 0.0` AND ≥1 has `loaded.is_none()` | `[needs-promote]` — was `[verified-correct]` in 436's scenario fixtures (single rack at known position), unverified at colony scale; if the marker fires without any rack being CONSTRUCTED (only sited?), eligibility passes but zone resolution fails |
| L2 DSE scoring | `src/ai/dses/dry_food.rs:97` | Spatial axis `dry_food_rack_distance` uses `LandmarkAnchor::NearestKitchen` (acknowledged stub awaiting `NearestDryingRack` per Commit 4 doc) | `[suspect]` — score is computed from Kitchen distance, not Rack distance. If no Kitchen exists either, the axis may collapse to 0 (or the curve's `ClampMin(0.1)` floor). The DSE elects on the strength of the other three axes (base_rate + scarcity + diligence). Effect: cats elect DryFood without "knowing" where the rack is, planner takes over, A* fails. |
| Plan template | `src/ai/planner/actions.rs::drying_food_actions` | `DryFood` step has `ZoneIs(PlannerZone::DryingRack)` precondition — A* must enter that zone | `[verified-correct]` (Commit 9 code) |
| Zone resolver | `src/systems/goap.rs::evaluate_and_plan` ~line 1733 | `drying_rack_positions` is computed from `building_query.filter(s.kind == StructureType::DryingRack && site.is_none())` — completed-only racks | `[suspect]` — if no rack has finished construction by the time the DSE is elected, the positions slice is empty and `TravelTo(DryingRack)` has no target. Same shape for SmokingRack. |
| Zone resolver | `build_zone_distances` (callsite in `goap.rs`) | Builds the per-cat zone distance map A* consults; `PlannerZone::DryingRack` must map to a real position | `[needs-promote]` — verify the `PlannerZone::DryingRack` entry is populated from `drying_rack_positions`; if the entry is missing entirely (vs. present-but-unreachable), `TravelTo` returns the "no reachable zone target" failure mode |
| Reachability | A* pathing in `resolve_goap_plans` | The `no reachable zone target` failure mode is distinct from "no target" — implies the cat is on a tile where A* can't reach the rack's tile | `[needs-promote]` — could be (a) rack position is on a tile A* doesn't accept (terrain mismatch), (b) cat is walled off, (c) the rack hasn't actually been constructed yet but `HasFunctionalDryingRack` fired anyway (writer bug). |
| Build pipeline | `src/systems/coordination.rs::accumulate_build_pressure` | DryingRack/SmokingRack go through the same construction path as other buildings; rack site spawns with `condition: 0.0`, completes to `condition: 1.0`. | `[verified-correct]` (fixture 2 of the 436 scenario confirms `Structure::new` ships condition=1.0, hence `effectiveness() == 1.0` immediately) |

## Fix candidates

**Parameter-level options**:

- **R1** — fix the spatial axis stub. Replace `LandmarkAnchor::NearestKitchen` in `dry_food.rs:97` (and equivalents in smoke_meat/tend_smoking_rack) with a new `LandmarkAnchor::NearestDryingRack` / `NearestSmokingRack`. If the DSE's spatial axis points at the actual rack, the L2 score factors in distance correctly and cats that have no nearby rack score lower (filtering at the L3 layer rather than at A* failure). This is the acknowledged debt from Commit 4's doc-comment.

- **R2** — promote `HasFunctionalDryingRack` to gate on both "functional+idle" AND "at least one cat-reachable rack exists" via a reachability flood-fill or by exposing the rack's `Reachable` marker. Same shape as `HasFunctionalKitchen` if it has the equivalent reachability check.

- **R3** — defer dispatch to scenarios where a rack actually exists. Gate the entire `DryFood` branch in `score_actions` on `bldg_state.has_functional_drying_rack` as an outer `if` (mirroring Cook's `hungry_enough_to_cook` outer gate). Cheapest fix; eliminates the plan-failure noise immediately. Brittle long-term — moves the gate logic out of the DSE's eligibility filter and into the dispatcher, the antipattern 438 is trying to retire.

**Structural options**:

- **R4 (split)** — split `DryFoodDse` into `DryFoodDseFromInventory` (spatial axis: distance to nearest rack; eligibility: `HasDryableInInventory`) and `DryFoodDseFromStores` (spatial axis: distance to nearest stores; eligibility: composite). Cleaner per-DSE L2 trace; surfaces the source-vs-transfer-vs-sink shape the items-are-real pillar demands. Considered and deferred by 436's structural-option menu (R5); the post-437 evidence argues for reconsidering.

- **R5 (extend)** — extend `EligibilityFilter` with a `require_reachable_zone(PlannerZone)` primitive that checks both presence-of-target AND reachability before the DSE is scored. Generalizes beyond the rack case — every zone-bound DSE (Cook → Kitchen, Caretake → Kitten target, etc.) benefits from a fail-early reachability gate. Larger blast radius; might overlap with 438's dispatcher retirement.

- **R6 (rebind)** — N/A. The Action↔Disposition mapping is fine.

- **R7 (retire)** — N/A. The DSE has a load-bearing job.

## Recommended direction

Open question — the layer-walk has three `[suspect]` and three `[needs-promote]` rows that need to be promoted via fresh queries before any candidate is chosen. Two plausible promotion paths:

1. **Position scan first**: `just q events logs/tuned-42-44a82ba0 BuildingConstructed | grep -i drying` to determine whether any DryingRack actually completes construction during the soak. If zero, the failure is in the build pipeline (rack never gets built but the marker fires anyway — a `HasFunctionalDryingRack` writer-vs-reader misalignment). If non-zero, drill into reachability.

2. **Trace inspection**: walk `logs/tuned-42-44a82ba0/trace-Simba.jsonl` for the first tick where `dry_food` wins L3 — read the L2 spatial axis score for `dry_food_rack_distance`. If the score is non-zero, the planner has SOME landmark position; if it's the `NearestKitchen` floor (`ClampMin(0.1)`), the spatial axis is reading from the stubbed Kitchen anchor and not the actual rack. R1 is the right fix in that case.

R3 (outer dispatcher gate) is the fast unblock if R1's spatial-anchor fix proves too large for one commit — but it's an antipattern shift that 438 wants to retire, so prefer R1 if practical.

## Out of scope

- Smoking-side multi-ingredient retrieve (367 Commit 10) — separate work; the smoking failures here will partially resolve when Commit 10 lands (cats will only score SmokeMeat when carrying meat+fuel, which is when retrieve is a real plan branch).
- 438's registry-iterating dispatcher retirement — orthogonal, can land before or after this ticket.
- Investigating why `mythic-texture` continuity canary went to 0 — likely a downstream consequence of the L3 time-budget compression (rare events lose to repeated DryFood plans), should re-stabilize once the DryFood plan-failure rate drops to zero. If it doesn't, open a separate canary-regression ticket.

## Verification

- `just q events logs/tuned-42-44a82ba0 BuildingConstructed --filter='kind: DryingRack'` (or equivalent) — confirms rack construction count. If zero, R1 / R3 alone won't help; need to investigate the build pipeline first.
- `just soak-trace 42 Simba` post-fix — `never_fired_expected_positives` no longer includes `FoodLoadedOnDryingRack` / `FoodDried`. Plan-failure rate for `TravelTo(DryingRack)` drops to baseline (0).
- Verdict: aggregate colony_score drift returns to ±10% band; `mythic-texture` continuity canary fires ≥1 / sim-year; survival canaries hold.
- The three `drying_chain_eligibility` scenario tests (from 436/437) stay passing.

## Log
- 2026-05-21: opened from the post-437 verification soak (`logs/tuned-42-44a82ba0`). 437 fixed the dispatch (DSE now scored, elected, planned-for), exposing the next-layer defect: `TravelTo(DryingRack): no reachable zone target` × 1095 + `TravelTo(SmokingRack): no reachable zone target` × 719. Six layer-walk rows still need promotion; the position-scan + trace-inspection paths under `## Recommended direction` are the next-session entry points. Blocked-by 437 because the verification evidence depends on 437 being landed.
