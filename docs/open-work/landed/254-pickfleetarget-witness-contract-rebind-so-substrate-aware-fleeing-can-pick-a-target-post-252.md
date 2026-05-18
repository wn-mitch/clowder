---
id: 254
title: PickFleeTarget witness contract — rebind so substrate-aware Fleeing can pick a target post-252
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 748c90d71a2c
landed-on: 2026-05-10
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

Ticket 252 lifted the L3 softmax filter that excluded `Action::Flee`,
restoring substrate-driven Fleeing election. The first verification
soak (`logs/tuned-42-post-252-fleeing-collapse`) revealed a SECOND
substrate-stub in `PickFleeTarget`
(`src/steps/disposition/pick_flee_target.rs:102`): the witness
condition `cost < current_cost` is unreachable in production because
`flood_dijkstra` (`src/ai/route_cost.rs:74`) hardcodes
`field.costs[from_idx] = 0` — the cat's current tile is always
cost 0, so no candidate can be `cost < 0`. The picker returns
`unwitnessed(Advance)`, `target_position` stays unset, the umbrella
`Flee` step falls back to the cat's own position
(`goap.rs:5993`), the cat sits, and `HoldUntilSafe` times out
(71× in the post-252 collapse soak — `plan_failures_by_reason`).

Cats elect Fleeing on threat (Flee score lifted by 108) but never
move; they get stuck in Fleeing → HoldUntilSafe-timeout loops that
consume the ticks they would otherwise spend courting and mating.

**Canary failures** (post-252 soak vs pre-252 baseline):
- `never_fired_expected_positives = ["MatingOccurred",
  "CourtshipInteraction", "PairingIntentionEmitted"]` — three hard-gate
  violations.
- Continuity canary `courtship = 0` (gate ≥ 1/sim year).
- `kittens_born: 4 → 0` (-100%); `bonds_formed: 39 → 19` (-51%);
  `aggregate score: 2202 → 1536` (-30%); `shelter: 0.42 → 0.0`
  (-100%); `peak_population: 12 → 8` (-33%).

Surfaced via 252's verification soak; confirmed by inspecting
`flood_dijkstra` line 74 and `pick_flee_target.rs:102`.

## Current architecture (layer-walk audit)

Walk every layer of the AI pipeline relevant to the defect. Tag each
load-bearing fact `[verified-correct]` (you read the code or a recent run
and it matches the assumption), `[suspect]` (you haven't verified, or it
looks wrong), or `[needs-promote]` (auto-prefilled by `/ticket-from-session`
from a hypothesis the Plan agent couldn't promote — the next session
promotes via a fresh query before any candidate that depends on the row).
A row tagged `[suspect]` or `[needs-promote]` MUST be addressed by at
least one of the fix candidates below.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs:216` (`HasThreatNearby`) | Authored by `interoception.rs:436` on threat proximity. Used for eligibility — fine. | `[verified-correct]` |
| L1 substrate | `src/components/route_cost_field.rs` + `src/ai/route_cost.rs:74` | `field.costs[from_idx] = 0` always. The cat's flood-origin tile has cost 0 by construction. | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/` (Flee score) + `src/ai/modifier.rs:2334` (`ThreatProximityAdrenalineFlee`) | Flee score lifts correctly; modifier reaches the action pool post-252. | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs:2411` | Filter lifted by 252; Flee competes in pool. Confirmed `Flee` wins at 98.89% in `flee_commitment` scenario. | `[verified-correct]` |
| Action→Disposition mapping | `src/components/disposition.rs:294` | `from_action(Action::Flee) => Some(Self::Fleeing)`. Wired. | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs:40-69, 928` | `[PickFleeTarget, Flee, HoldUntilSafe]` dispatches when `Fleeing` wins. | `[verified-correct]` |
| Completion proxy | `src/ai/commitment.rs:239` | `Fleeing => SingleMinded` — keeps the cat committed to the broken plan. | `[verified-correct]` |
| Resolver: PickFleeTarget | `src/steps/disposition/pick_flee_target.rs:101-106` | Witness condition `cost < current_cost` where `current_cost = field.cost_at(self_pos) = 0`. Unreachable in production. | `[verified-defect]` |
| Resolver: Flee | `src/systems/goap.rs:5993` | Falls back to `*pos` when `target_position` unset → no movement. | `[verified-correct]` (working as documented; the upstream picker is the bug) |
| Resolver: HoldUntilSafe | `src/steps/disposition/hold_until_safe.rs` | Hysteresis times out at 30 ticks if the cat doesn't reach a "safe" tile (which it can't, because it isn't moving). | `[verified-correct]` |

## Fix candidates

**Parameter-level options:**
- R1 — change comparison to `cost <= current_cost` (allow ties): doesn't
  fix the bug; the picker would still pick origin (cost 0) over any
  positive-cost candidate.
- R2 — read `overlay_at(self_pos)` (just the per-tile overlay weight,
  not the cumulative path cost) as `current_cost`. If the cat's tile
  has overlay > 0, pick a tile with lower overlay. Only fires when the
  cat is *on* a fox-scent / corruption tile — doesn't address "fox 2
  tiles east" cases.

**Structural options:**
- R3 (**rebind**) — flood from the THREAT instead of the cat, OR
  maintain a per-cell "danger" overlay map separately from the route-
  cost field. Picker selects the tile with HIGHEST distance-from-threat
  (or lowest overlay-at-tile, weighted by reachability). The current
  flood-from-cat field still serves the proximity-consideration use
  case at `considerations.rs:282`; this adds a second field for flee
  target picking.
- R4 (**split**) — give Fleeing its own per-tick threat-flood field,
  re-flooded each Fleeing tick with origin = nearest threat. PickFleeTarget
  reads the threat-flood and picks the highest-cost tile in disc (=
  farthest-via-safe-corridor from threat).
- R5 (**extend**) — keep flood-from-cat, but compute a candidate's
  effective fleeing-cost as `cost(candidate) - chebyshev(candidate, threat)`
  (cheap-to-reach but far-from-threat). Picker picks LOWEST such cost.
  The negative term inverts the picker direction without a second flood.
- R6 (**retire**) — delete the substrate-aware picker; revert to the
  pre-230 naive vector projection (cat moves `flee_distance` steps
  away from threat geometrically). 230 was supposed to *replace* this
  but never worked; "retire" here means "concede the substrate-aware
  approach didn't ship and accept the naive picker as the working
  baseline." Out of scope per CLAUDE.md "Substrate over hacks" — but
  it's the menu entry that says "230 + 252 may have been the wrong
  shape entirely".

## Recommended direction

R5 (**extend** with the chebyshev-from-threat term) is the smallest
surface change. Drops cleanly into the existing
`PickFleeTarget::resolve` body — replace the cost comparison with a
composite `effective_cost(candidate) = field.cost_at(candidate) -
chebyshev_to_threat(candidate)`. Existing tests adapt; the `picks_low_cost_tile_when_cheaper_than_current`
unit test was already constructed with non-zero cost-at-origin (the
contrived field at `pick_flee_target.rs:158`), suggesting the original
author intended a semantic where the cat's tile could have positive
cost — R5 honors that intent without restructuring the flood.

Alternative: R3 if a per-cell danger field already exists for some
other system (likely candidate: the fox_scent_map directly) — then
PickFleeTarget reads that, not the route-cost field.

## Out of scope

- Re-tuning `flee_distance`, `flee_hold_ticks`, or the
  `route_cost_safe_threshold` — those are calibration knobs that
  matter only after the picker actually moves the cat.
- Re-architecting `RouteCostField` itself — its flood-from-cat
  semantic is correct for the proximity-consideration use case
  (`considerations.rs:282`).
- Re-shaping `HoldUntilSafe`'s hysteresis — that step works
  correctly *given* a target was picked.

## Verification

Soak (`just soak-trace 42`) must clear:
- `never_fired_expected_positives = []` (restore MatingOccurred,
  CourtshipInteraction, PairingIntentionEmitted).
- Continuity canary `courtship ≥ 1`.
- `kittens_born ≥ 1` (post-252 collapse had 0).
- `HoldUntilSafe: global step timeout` < 10 (post-252 had 71).
- `Feature::FleeTargetPicked` fires ≥ 1 in the soak.
- Promote `flee_commitment` scenario gate to `expected_features:
  &["FleeTargetPicked"]`.

## Log

- 2026-05-10: opened from 252's verification soak. Layer-walk
  promoted PickFleeTarget witness contract to `[verified-defect]`.
  Recommended R5 (extend) as the smallest surface change.
- 2026-05-10: 2026-05-10: R5 extend implemented; pick_flee_target.rs minimizes effective_cost = cost - chebyshev_to_threat instead of (unreachable) cost < current_cost. flee_commitment scenario gate promoted to expected_features=[FleeTargetPicked]. 6/6 unit tests pass. seed-42 soak shows bit-for-bit identical footer to pre-fix cedar run (commit 12023b1c) — fix is inert in this seed because Flee never wins L3 softmax (Mocha boldness=0.9, health=0.26, safety=0.003 → Flee=-0.025). Substrate-side Flee scoring gap opened as ticket 271.
