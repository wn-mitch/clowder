---
id: 256
title: Patrol DSE recalibration — influence-map-driven smart pathing + wildlife deterrent affect (post-252 cascade root cause)
status: done
cluster: ai-substrate
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: ae4202729d22
landed-on: 2026-05-10
---

## Why

The post-252 verification soak (`logs/tuned-42-post-252-fleeing-collapse`,
commit `aeca4834` dirty) collapsed reproductive continuity (mating /
courtship / pairing canaries all 0; Mate-action snapshots = 0;
Courtship-disposition plans = 0) and exposed cats to predator
ambushes. Investigation during ticket 255's calibration audit found
that **Patrol absorbs 63.65% of all action elections** in the soak
(vs Flee's 4.45%), and 2 of 3 deaths trace to Patrol exposure rather
than Flee miscalibration:

- **Cedar** (tick 1215082): oscillating Patrol around [38, 22]↔[39, 23]
  for 500+ ticks; replan switched to `[EngageThreat]`, morale-broke
  during wildlife combat, died from `WildlifeCombat` injury.
  Flee elected exactly 1× across the cat's lifetime.
- **Calcifer** (tick 1287547): cycled through Cooking → PickingUp →
  Exploring → **Guarding** dispositions; first ambush at [38, 21]
  crashed mood from 0.39 → 0.08; chose `EngageThreat` over Flee on
  the next replan; died in second ambush at [25, 41].

Architecturally, the Patrol DSE pulls cats toward a single fixed
`TerritoryPerimeterAnchor` tile computed as `Position(colony_center.x
+ patrol_perimeter_offset, colony_center.y)` at
`src/systems/disposition.rs:973` — *one tile*, not a perimeter curve,
ward-coverage centroid, or any influence-map-aware target. The path
step `resolve_patrol_to` (`src/steps/disposition/patrol_to.rs:31`)
walks vanilla A* through that target, geometrically agnostic to ward
placement, corruption, fox-scent, or recent ambush sites. The result
is the L3 patrol absorption cascade documented in ticket 181's iter-2
(memory `project_l3_patrol_absorption_cascade`): Patrol elevates,
cats walk into corruption-adjacent corridors, ShadowFoxes ambush
them, the labour pool thins, courtship bandwidth evaporates.

This ticket is not a small pathing patch. It's a **substrate-level
recalibration of Patrol's role**: from a fixed-anchor walk to an
influence-map-driven, smart-pathed wildlife-deterrent role that
respects ward coverage and pushes wildlife away from the demesne.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 influence maps | `src/resources/{FoxScentMap,WardCoverageMap,CatPresenceMap,CorruptionLandmarks}.rs` | Required substrate maps already exist; Patrol DSE consumes none of them. | `[verified-correct]` |
| L1 anchor | `src/systems/disposition.rs:973` | `TerritoryPerimeterAnchor = Position(colony_center.x + patrol_perimeter_offset, colony_center.y)` — one tile, no ward awareness. | `[verified-correct]` |
| L2 DSE | `src/ai/dses/patrol.rs:48-100` | Three composition axes: `safety_deficit` Logistic, `boldness` Linear, `safety_upper_bound` Composite-Logistic-Inverted, plus `patrol_perimeter_distance` Spatial axis to the single anchor. No fox-scent / corruption / ward-coverage axis. | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs:2411` | Patrol participates in softmax post-252 alongside all dispositions. Patrol = 63.65% of CatSnapshot `current_action` rows in the post-252 collapse soak. | `[verified-correct]` |
| Action→Disposition | `src/components/disposition.rs::from_action` | `Action::Patrol` → `DispositionKind::Guarding`. Mapping itself is fine; the substrate driving the action is the issue. | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs::patrol_actions` | Emits `[TravelTo(PatrolZone), Survey]` under `ZoneIs(PlannerZone::PatrolZone)`. The zone-target is the anchor; smart pathing would change WHERE, not the step shape. | `[verified-correct]` |
| Resolver — pathing | `src/steps/disposition/patrol_to.rs:31-65` | `resolve_patrol_to` calls `path_plan.find_full_path(*pos, target, map)` — vanilla A* on TileMap movement_cost only. No `RouteCostField` overlay, no influence-map cost. | `[verified-correct]` |
| Resolver — wildlife affect | `src/systems/fox_goap.rs:186` | Foxes use `territory_perimeter_anchor = den_pos.map(...)` for *their own* patrol pull, not for cat-presence avoidance. No symmetric "cats deter foxes" map. | `[verified-correct]` |

## Fix candidates

**Parameter-level options** (insufficient on their own — included for
completeness, not as recommendation):

- R1 — Re-tune `patrol_safety_threshold` / `patrol_exit_threshold` /
  `patrol_perimeter_offset` to push the L2 score down or move the
  anchor further from corruption. Doesn't fix the pathing or the
  wildlife-deterrent gap; just shrinks the cascade.
- R2 — Add a `CorruptionPatrolSuppression` modifier mirroring the
  extant `CorruptionTerritorySuppression` (#17 in
  `default_modifier_pipeline`) but acting on Patrol instead of
  Explore/Wander/Idle. Substrate-side; suppresses Patrol when the cat
  is near corruption. Doesn't fix the geometric anchor or the
  pathing.

**Structural options:**

- R3 (**extend**) — Replace Patrol's single-tile
  `TerritoryPerimeterAnchor` with a **ward-coverage centroid** or a
  **per-replan rotation across ward sectors**. The anchor becomes
  influence-map-derived (read from `WardCoverageMap`), not a fixed
  colony-center offset. Effect: cats naturally patrol the inside of
  the warded demesne; the patrol vector tracks where the colony has
  built protection. Same plan template, same resolver shape; only
  the anchor source rebinds.
- R4 (**extend**) — Overlay the existing `RouteCostField` machinery
  (used by Flee post-230, `src/ai/route_cost.rs`) on
  `resolve_patrol_to`'s pathing, parameterized for "patrol" cost
  weights: avoid corruption + fox-scent + recently-ambushed tiles;
  bias toward unswept-ward sectors. Effect: smart pathing — even
  when the anchor is geometrically agnostic, the path between the
  cat and the anchor avoids known-bad tiles. Composes cleanly with R3
  (which fixes the anchor) but is independent.
- R5 (**extend**) — Symmetric **wildlife-deterrent affect**: cat
  patrol presence emits into a `CatPatrolDeterrentMap` (new
  influence map, or extend `CatPresenceMap` with a "patrolling"
  weight), which the fox AI reads as cost in
  `src/systems/fox_goap.rs`. Foxes route around active patrols
  instead of through them. Closes the symmetry: fox AI already reads
  fox-scent as deterrent for cats; cats should reciprocally deter
  foxes via patrol presence. Effect: Patrol becomes a *behavioral
  effect on wildlife*, not just on the patrolling cat.
- R6 (**split**) — Split `DispositionKind::Guarding` into
  `Guarding` (proactive perimeter walk; the recalibrated role) vs
  `EngageThreat` (reactive combat response). Today Patrol's
  PlanCreated chain emits `[TravelTo(PatrolZone), Survey]` but a
  replan can switch to `[EngageThreat]` mid-Patrol (Cedar's death
  pattern) — the split would force replans to rebid into a separate
  combat disposition rather than slipping silently from "walk the
  perimeter" to "fight the wildlife." Optional; harder than R3+R4+R5.

## Recommended direction

**R3 + R4 + R5 composed.** R3 fixes WHERE Patrol points (ward-
derived anchor). R4 fixes HOW Patrol gets there (smart pathing
through influence-map cost overlays). R5 fixes WHAT Patrol *does* to
the wildlife it's nominally guarding against (deterrent affect).

R3 is the load-bearing fix; without it the anchor is still a single
geometrically-blind tile and R4's smart pathing has nothing meaningful
to path *toward*. R4 unlocks the ambush-corridor avoidance that the
post-252 collapse exposed. R5 closes the loop so Patrol becomes a
real ecological role rather than a decorative walk.

R1 / R2 are parameter-level patches that would shrink the cascade
without addressing the substrate; rejecting them per CLAUDE.md
"substrate over hacks" pillar.

R6 is optional; defer until R3+R4+R5 land — at that point we can see
whether mid-Patrol switches to `EngageThreat` are still appearing in
focal-cat traces.

## Out of scope

- `flee_lift` / `sleep_lift` calibration (255 owns; verdict: no
  change needed).
- PickFleeTarget witness contract (254 owns).
- Foxes' own patrol pull (`src/ai/dses/fox_patrolling.rs` — uses
  `TerritoryPerimeterAnchor` for the fox's denward retreat). May
  share rebinding with R3 in a future fox-AI ticket.
- Sleep DSE substrate (251 owns).

## Verification

- **Soak gate.** A fresh seed-42 deep soak post-recalibration must
  pass `just verdict logs/<dir>` with all five continuity canaries
  intact (`grooming · play · mentoring · courtship · mythic-texture`)
  and `kittens_born ≥ 1 / sim year`.
- **Action-distribution drift.** Patrol's share of `current_action`
  in the soak should drop materially below 63.65% (the post-252
  collapse rate) and ideally land near the pre-252 baseline. Measure
  via `just q actions <run-dir>`.
- **Focal-cat trace drift.** `just soak-trace 42 Cedar` (or any
  high-Patrol cat from the collapse soak) should show ambush
  exposure drop and the EngageThreat-during-Patrol cascade pattern
  not recurring. Diff via `just frame-diff <baseline> <new>`.
- **Survival canaries.** ShadowFoxAmbush deaths ≤ 10 hard gate.
- **No regression on Flee substrate.** `just scenario
  flee_calibration_open_terrain` and `flee_calibration_sleep_partner`
  continue to behave per 255's verdict.

## Log

- 2026-05-10: opened from ticket 255's audit. The post-252
  reproductive collapse (`tuned-42-post-252-fleeing-collapse`) was
  diagnosed during 255 as Patrol-cascade-driven rather than Flee-
  calibration-driven; this ticket carries the substrate fix.
  Layer-walk pre-staged from 255's investigation. `flee_calibration`
  scenarios under `src/scenarios/flee_calibration.rs` will guard
  against Flee-substrate regression during the recalibration.
- 2026-05-10: implementation landed. R3 `WardCoverageMap::sector_centroid`
  + per-replan rotation; R4 `patrol_route_cost_weight` activated
  (0.0→0.6) + per-disposition overlay weights for Guarding cats
  (FoxScent/Corruption ×1.5); R5 new `CatPatrolDeterrentMap`
  consumed by fox A* via `CatPatrolDeterrentOverlay`. Substrate
  pieces verified independently via `src/scenarios/patrol_recalibration.rs`
  (3 unit tests, 1 runnable scenario). 2043/2043 lib tests pass;
  `just check` clean (`InfluenceMap registry: 14 impl(s), all registered`).
  Verification soak `logs/tuned-42` (commit `12023b1c` dirty,
  Cedar focal):
  - **Cascade root cause fixed.** ShadowFoxAmbush deaths 3 (≤10
    hard gate); 0 starvation deaths; all 5 continuity canaries
    green (`grooming · play · mentoring · courtship · mythic-texture`);
    courtship 0 → 1609; mythic-texture 0 → 37; play 0 → 14.
  - **Patrol share modestly down.** 63.65% → 59.84% — substrate
    redirected WHERE/HOW Patrol behaves but didn't reduce its
    L2 floor.
  - **Verdict fail driven by `MatingOccurred` never-fired.** Pre-
    existing end-of-chain gap (post-252 baseline also had
    `MatingOccurred = 0`); 256 is not a regression on this gate.
    Per `feedback_chain_rare_events`, end-of-chain metrics
    deserve structural verification rather than single-soak
    gating. Opens 257 (`Mate election crowded out by Patrol in
    post-256 regime`) blocked-by 256 for the courtship→mating
    follow-on.
- 2026-05-10: opens 257 — Mate election crowded out by Patrol in
  post-256 regime. R6 (split `Guarding` into `Guarding` +
  `EngageThreat`) deferred until 257's layer-walk reveals whether
  mid-Patrol replans into combat-disposed plans are still the
  dominant pattern (Cedar's pre-256 death pattern); if so, R6
  becomes part of 257's structural-option menu.
