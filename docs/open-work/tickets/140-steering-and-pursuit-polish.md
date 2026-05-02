---
id: 140
title: Phase 3 — Steering, smooth pursuit / flee, pathfinder polish
status: blocked
cluster: substrate-migration
added: 2026-05-02
parked: null
blocked-by: [139]
supersedes: []
related-systems: [project-vision.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase 3 of the continuous-position migration (epic ticket 135). With Phase 2 (#139) landed — `Position` is `Vec2<f32>`, pathfinding returns continuous waypoints — cats and wildlife still move in straight lines from tile center to tile center. Phase 3 introduces **steering** so motion curves naturally around obstacles, around each other, and along pursuit / flee arcs. This is the "doing it well" pass on top of the substrate migration.

## Scope

### Steering primitives

1. **`Velocity` component** — `Vec2<f32>`. Authored alongside `Position`; updated per tick by steering systems; consumed by movement integrator that adds `velocity * dt` to `position`.

2. **Seek / arrive / flee / wander steering behaviors** in `src/ai/steering.rs`:
   - `seek(pos, target, max_speed) -> Vec2` — accelerate toward target.
   - `arrive(pos, target, slowdown_radius, max_speed) -> Vec2` — seek with slowdown near target.
   - `flee(pos, threat, max_speed) -> Vec2` — accelerate away from threat (used by Flee DSE step resolver).
   - `wander(pos, last_velocity, jitter) -> Vec2` — for idle / patrol motion.
   - Optional v1: `obstacle_avoidance(pos, velocity, map) -> Vec2` — short-range raycast avoidance for unwalkable tiles.

3. **Movement integrator system** in Chain 2 (after AI decisions, before render). Reads `Velocity`, integrates to `Position`, clamps speed by `MovementBudget` (Phase 1 #138) — budget tracks distance traveled this tick rather than discrete steps.

### Pathfinder polish

4. **Flow-field pursuit.** For multi-entity pursuit (e.g. wildlife converging on a scent source, cats fleeing a shadow fox), compute a per-target flow field once per tick; steerers read the flow vector at their containing tile. O(map_area) once vs. O(entities × tiles_in_path) for per-entity A*.

5. **Path-recompute throttling.** A* recomputes only when target moves > N tiles or every M ticks. Cache previous waypoints per entity.

6. **Step-resolver migration.** `resolve_*` step functions under `src/steps/` that move a cat one tile become "set velocity toward next waypoint" handlers. Each resolver's "Real-world effect" rustdoc heading updates to reflect continuous motion.

### Distance-spent vs. tile-counted budget

7. **`MovementBudget` (#138) reframes** to budget *distance per tick* rather than *step opportunities*. Default cat budget = 1.0 distance unit per tick (one tile-side per tick at typical pace). Snake = 0.5 distance units per tick. The existing `escape_viability` mobility term (#138) keeps reading `budget_per_tick`; semantic survives.

## Verification

- `just check` / `just test` green.
- Visual check — `just run-game`; cats curve around obstacles, flee along smooth arcs, pursue with steering. No tile-snap teleports.
- `just soak 42 && just verdict` — expect drift on:
  - **Ambush success rates** — smoother flee curves should reduce shadow-fox catches; canary may trigger on `deaths_by_cause.ShadowFoxAmbush`.
  - **Pursuit duration** — smarter steering means quicker resolution; some Hunt successes shift in tick-count.
- **Hypothesis (per CLAUDE.md balance methodology):**

  *Steering with smooth flee and pursuit will:*
  *(a) Reduce ShadowFoxAmbush deaths by 10–30% as cats execute curved flees that exploit terrain better than straight-line tile-step retreats.*
  *(b) Increase Hunt success rate by 5–15% as cats curve into prey rather than approaching cardinally.*
  *(c) Leave continuity canaries within ±10%.*

  Run `just hypothesize` end-to-end. Promote new baseline.

## Out of scope

- **Crowd dynamics / flocking** — coordinated group motion. Separate scope.
- **Combat positioning** — flank / surround / kite. Tactical AI is its own surface.
- **Animation / sprite rotation aligned with velocity.** Render polish ticket.
- **Cross-platform determinism.** Single-platform bar (epic constraint).

## Log

- 2026-05-02: Opened as Phase 3 of the 135 continuous-position-migration epic. Blocked-by 131 (Phase 2 substrate); independent of Phase 0 (#137) and Phase 1 (#138).
