---
id: 465
title: Cat approach pathing failure at stalk range
status: ready
orchestration: substrate-sensitive
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-05-24
parked: null
blocked-by: []
supersedes: []
related-systems: [sensory.md, ai-substrate-refactor.md]
related-balance: [100-tremor-action-multiplier.md]
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

Ticket 464 (the `effective_stalk_distance` defaults retune) landed and
recovered colony-wide hunt success 13.3% → 18.47% and Rat success
14.1% → 45.4%, but **Rabbit success stayed pinned at 17.05%** — basically
unchanged from the broken post-100 state of 17.5%. The failure pattern
is sharply localized: **"stuck during approach" accounts for 97.8% of
Rabbit losses** (352 of 360 losses in run `tuned-42-cfc6f4fa`). The
narrative line: "stuck during approach (1068, 80.9% of losses)" —
colony-wide, but Rabbit is the dominant species in that count.

This is the pathing weakness 464 §Out of scope explicitly named for
escalation if R1's soak still showed residual `lost_during_stalk`
elevation. It surfaced. The mechanism is *separate* from the stalk-distance
tuning: even at the tightened R1 distances (~7 tiles for a typical
patient cat on Rabbit), the A*-driven approach pathing gives up on a
non-trivial fraction of attempts. Rats catch fine at the same distances
because rat alert_radius is smaller (4 vs Rabbit's 6) — the approach
window is shorter and less likely to wander through clutter.

Run-dir: `logs/tuned-42-cfc6f4fa` (constants reflect 464's R1 — verify
via header's `disposition.species_push: 1.0` / `alertness_push: 1.5`,
not via the `commit_hash` field which reads the parent due to a
build.rs/jj interaction).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | n/a | Not a marker-driven defect; the cat has Hunt-eligible markers and the prey is sensed | `[verified-correct]` (via 464's audit + colony-wide kill rate recovery on Rat) |
| L2 DSE scores | `src/ai/dses/hunt_target.rs` | Hunt selection rate per-10kt unchanged colony-wide; cats DO commit to Hunt | `[verified-correct]` (464 frame-diff: Hunt not in top-15 movers) |
| L3 softmax | `src/ai/scoring.rs` | Hunt wins softmax; the GoapPlan is built | `[verified-correct]` |
| Plan template | `src/ai/planner/...` (`EngagePrey`) | Plan transitions Approach → Stalk/Chase/Pounce by distance threshold | `[verified-correct]` (464 layer-walk) |
| EngagePrey resolver | `src/systems/goap.rs::resolve_engage_prey:9037-9042` | Stalk-start distance now tuned via 464's R1; the per-tile lift is reduced | `[verified-correct]` |
| Approach pathing | `src/systems/goap.rs::step_toward` + A* | Cat moves toward prey via `step_toward`; one-tile-per-tick step. Fails on cluttered terrain — "stuck" reason 97.8% of Rabbit losses | `[suspect]` — load-bearing defect lives here |
| Stalk-phase pathing | (same) | Stalk-specific pathing fires once `actual_distance ≤ effective_stalk_distance`; `lost_during_stalk` recovered from 301 → 108 with 464's R1 | `[verified-correct]` (R1 fixed this layer) |

## Fix candidates

**Parameter-level options:**

- **R1 — increase A* search budget or relax the "stuck" giveup threshold**
  in `step_toward`. Cheap; risks pathfinder CPU growth and edge-case loops.
  Likely insufficient on its own — the "stuck" reason hits when the
  pathfinder *succeeds* but the next-step tile is impassable (transient
  occupant, prey blocking, terrain irregularity).
- **R2 — add a one-tile-jitter retry** when `step_toward` fails to find a
  walkable next step. The cat re-evaluates from an adjacent tile rather
  than abandoning the plan. Mirrors the prey's evasive-jink pattern.
- **R3 — bias the approach path's heuristic away from terrain features
  the pathfinder gets stuck on** (e.g., LightForest density, water-adjacent
  tiles). Requires identifying *which* tile-shapes correlate with "stuck"
  events via `just q events --kind=PlanFailure` on a Rabbit-heavy run.

**Structural options:**

- **R4 (split) — give Approach its own resolver step distinct from
  EngagePrey.** Currently EngagePrey decides phase per tick; the
  Approach phase has different needs (path planning toward a moving
  target) than Stalk/Chase/Pounce (close-range tactical movement). A
  dedicated `ApproachPrey` step with its own pathfinder profile (more
  patience, alternate-route discovery) would isolate the failure mode.
- **R5 (extend) — branch `step_toward` behavior on whether the caller is
  in approach-to-prey context.** Use a prey-aware path budget that's
  longer than the default movement budget. Keeps the step shape but
  thickens its behavior. Less invasive than R4.
- **R6 (rebind) — n/a.** No Action→Disposition mapping change applies.
- **R7 (retire) — n/a.** Approach is load-bearing.

## Recommended direction

**Investigate first via `just q events --kind=PlanFailure` filtered to
Rabbit hunts in `logs/tuned-42-cfc6f4fa`** to characterize which terrain
shapes produce the "stuck" failures. Then ship R2 (one-tile-jitter retry)
if the failures cluster on transient-occupant blocks, OR R5 (prey-aware
path budget) if the failures cluster on terrain-shape dead-ends. R4
(split Approach) is the structural option held in reserve if neither
parameter fix lands the Rabbit success rate within ±10% of baseline
(target ≥ 29.7%).

## Out of scope

- The `effective_stalk_distance` arithmetic — 464 handled it.
- Multi-focal sweep on a high-boldness focal — deferred per 464's §Out
  of scope.
- Per-personality pathfinder profiles (bold cats give up faster) — would
  be a *second* axis on top of any fix here; revisit only if R2 / R5
  don't bring Rabbit into the band.

## Verification

- `just check` + `just test` clean.
- `just q events --kind=PlanFailure logs/tuned-42-cfc6f4fa` (first; characterize the failure terrain).
- `just soak-trace 42 Simba` post-fix.
- `just verdict logs/tuned-42-<sha>` — survival/continuity must pass.
- `just q hunt-success logs/tuned-42-<sha> --species=rabbit` — must be ≥ 29.7%
  (within ±10% of baseline 33.0%).
- Colony-wide hunt success ≥ baseline 19.7% (R1 brought it to 18.47%, just
  below; this ticket should restore the missing margin).

## Log

- 2026-05-24: opened from 464's R1 soak. Colony-wide hunt success recovered
  to 18.47% (target ≥ 17.7% met), Rat fully recovered (14.1% → 45.4%), but
  Rabbit stalled at 17.05% (target ≥ 29.7% missed by ~13 pp). Failure pattern
  "stuck during approach" 97.8% of Rabbit losses — the pre-existing A* path-
  weakness 464 §Out of scope explicitly named. R1/R2 likely; R4 structural
  fallback if parameter fixes don't recover Rabbit within band.
