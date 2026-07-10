---
id: 531
title: Flow-field pursuit — per-target field for multi-entity convergence (140 §4 spin-off, deferred from 0.4.0)
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The one unabsorbed item from ticket 140 (landed at 0.4.0): for
multi-entity pursuit/convergence (several cats converging on a shadow
fox, wildlife on a scent source), per-entity A* is O(entities ×
tiles_in_path) per repath; a per-target flow field is O(map_area) once
per tick and every steerer just reads the flow vector at its containing
tile. Deferred from 0.4.0 deliberately (plan step 13): worst perf shape
in a perf-pinned release, and the steering library is source-agnostic —
`DesiredVelocity` doesn't care whether the desire came from a smoothed
A* corridor or a field read, so this drops in without touching the
integrator contract.

## Scope

- Per-target flow field resource (reuse the RouteCostField/Dijkstra
  machinery from `ai::route_cost` where possible), computed only for
  targets with ≥ N concurrent pursuers.
- Steering read: `flow(target_id, containing_tile) -> Vec2` feeding
  `DesiredVelocity` through the existing `steer()` path.
- Perf gate: net win vs the A*-per-pursuer baseline at the posse
  convergence scenario (chokepoint_defense / wildlife_fight families);
  flamegraph pre/post per the perf-refactor rule.

## Out of scope

- Replacing single-pursuer A* corridors (cached routes already killed
  the stuck-watchdog churn at plan step 13).

## Current state

Not started. Contract reference: `docs/systems/movement.md` (steering
is source-agnostic by design).

## Approach

See ticket 140 §4 (landed copy) for the original sketch.

## Verification

- Posse-convergence scenario: N cats converge on one fox via field
  reads; byte-visible in the L4 resolver trace as a named source.
- Perf soak 60–120s + flamegraph; verdict throughput channel.

## Log

- 2026-07-09: opened at the 0.4.0 release ceremony as 140 §4's
  spin-off (plan step 13 deferral).