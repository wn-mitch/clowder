# Movement — fluid free-range locomotion (0.4.0, epic 135 / plan Phase II)

Every creature moves with arbitrary headings, momentum, and gaits —
not 8-direction tile-hopping, and not interpolated tile-hopping. This
doc is the contract reference for the components, the integrator, the
steering library, and the conventions every decision layer must follow.
Landed across plan steps 4–13 (2026-07-05 → 2026-07-07); balance record
`docs/balance/fluid-movement-phase2.md`.

## The two-component contract

- **`Velocity(Vec2)`** (`components/physical.rs`) — the mover's actual
  persistent velocity, **integrator-owned**. Decision code never writes
  it.
- **`DesiredVelocity(Option<Vec2>)`** — written by decision layers
  (resolvers, wildlife AI, prey AI), **consumed-and-cleared** by the
  integrator each tick. No desire → velocity decays to zero. This is
  the bisectability invariant: an unmigrated resolver that writes
  `Position` directly expresses no desire, so nothing double-moves.
- **`Flying`** marker — terrain-exempt integrator branch (bounds-clamp
  only). Authored at spawn (`on_wild_animal_added` for hawks; bird
  prey get it for `BurstFlight`).

**Rule for all decision code:** author `DesiredVelocity` via
`ai::steering`, never `Position`. `MovementBudget.per_tick` is the
mover's speed cap (the pre-140 accumulator/`try_spend_step` API is
retired; the component now only carries the cap, set by spawn
observers from `MovementConstants`).

## The integrator

`systems::movement::integrate_velocities` — head of Chain 4, before
`emit_cat_moved_messages`, so every decision writer has run and
CatMoved/NearPairCache see post-move positions the same tick.

Per mover per tick:

1. `vel = steer(vel, desired, max_accel)` — acceleration-limited
   (`max_accel` 0.25 tiles/tick²; hawks 0.5), so turns curve and
   reversals arc instead of pivoting.
2. Clamp speed **Euclidean** to `MovementBudget.per_tick` — L∞ would
   make ground speed direction-dependent (+41% at 45°).
3. Terrain speed: the containing tile's `movement_cost()` bucket scales
   velocity (cost 1 → 1.0, cost 2 → 0.8, cost 3 → 0.6, cost 4 → 0.5).
   Terrain costs speed, not just route preference. `Flying` exempt.
4. Sub-stepped passability (`SUB_STEP_LEN` 0.45 < √2⁄2) — anti-tunnel
   at burst speeds — with wall-slide: try `(new.x, old.y)`, then
   `(old.x, new.y)`, else stop and zero velocity.
5. Consume `DesiredVelocity`.

## Steering library (`ai::steering`)

Pure functions, unit-tested; sources are agnostic (A* corridors, flow
fields later, raw geometry):

- `smooth_path(path, map, overlays)` — string-pulling over the A* tile
  path with a **cost-aware supercover raycast**: a waypoint is pruned
  only if every tile the shortcut crosses is passable AND its
  terrain+overlay cost stays ≤ the pruned corridor's per-tile ceiling
  (+ε). Smoothing can never shortcut through fox-scent/corruption the
  router paid to avoid. Raw A* waypoints are always 8-neighbors —
  smoothing is what breaks the 45°-quantized staircase.
- `steer(vel, desired, max_accel)` — momentum core.
- `seek` / `arrive` / `flee` / `wander` — desired-velocity generators;
  `wander` is current-heading + bounded angular jitter (meander).
- `pursue(pos, target_pos, target_vel, max_speed)` — lead interception
  toward the predicted position. Used by Hunt chase + fox pursuit.
  Its counterpart: 266 prey Bolt/Scatter flee the *predicted* threat
  position (`pos + vel × prey_bolt_lead_ticks`) — evasion and pursuit
  read the same geometry from opposite sides.
- `separation(pos, neighbors, radius)` — personal-space force
  (radius 0.6); replaced the `jitter_if_stacked` / arrival-jitter
  teleports.

Travel resolvers follow the corridor convention: A* path →
`smooth_path` → seek the furthest unpruned waypoint → pop within
`waypoint_arrival_radius` (0.6 — MUST stay > max_speed⁄2 or a fast
mover can jump the arrival window and orbit forever). A* recomputes
are throttled: only after `path_recompute_min_ticks` (8) or when the
target drifts > `path_recompute_target_drift_tiles` (3.0).

## Speeds and gaits (`MovementConstants`)

| Mover | base max speed | notes |
|---|---|---|
| cat / fox / hawk / shadowfox | 1.0 | |
| snake | 0.5 | continuous (the tick-skip stutter is retired) |
| ground prey | 1.0 × species `flee_speed` | flee cap via spawn observer |
| bird escape burst | 3.0 | `Flying`; replaces the radial teleport |

Gaits are per-action multipliers on the base: stalk 0.4 (slow sinuous
approach), walk 1.0, sprint 3.0 (chase/flee/strike). Sprint history:
1.4 and 2.4 both cratered hunt success (see the constant's doc-comment
— though those measurements carry pre-467 fish-churn denominators);
3.0 is pre-140 parity, the value the detection/alertness/catch economy
was tuned against. Chase endurance is bounded by `chase_limit_*`
ticks, not a stamina model.

## Arrival and metric conventions

- **Arrival = containing-tile equality.** `Position`'s `Eq` is
  tile-keyed, so `pos == target` means "same tile" — zero test churn
  from sub-tile positions.
- **Adjacency = `chebyshev_distance <= 1`**, unchanged. Chebyshev is
  the *tile-tactical* metric (strike range, reach-this-tick).
- **Perception = world-space Euclidean.** `distance_to` returns
  `self.0.distance(other.0)` (plan step 8, deliberately inverting
  ticket 494): isotropic continuous movement makes Euclidean the
  substrate-correct perception metric. Diagonal travel is √2 slower
  than the grid era — a hypothesis-carried re-baseline.
- Decision-side habitat/corruption checks stay tile-grid (sample the
  tile ahead along the heading); influence-map reads stay tile-grid by
  epic constraint.

## Rendering

Velocity-movers use linear interpolation in `entity_sprites.rs` —
per-tick smoothstep easing pulses against true velocity motion.

## Perf shape

Budget accumulator retired; per-tick `accumulate_movement_budget`
retired; recompute throttling above. The Phase-II exit gate measured
112.2 tps against the 112.0 Phase-I baseline with the full fluid stack
aboard, and cached hunt routes eliminated the residual stuck-watchdog
churn (`lost_during_approach` 420 → 20). Flow-field pursuit remains
deferred (steering is source-agnostic; follow-on ticket).
