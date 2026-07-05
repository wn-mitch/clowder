//! Steering + path-smoothing primitives — 0.4.0 "Free Range" fluid
//! locomotion substrate (ticket 140 / plan step 5).
//!
//! Pure geometry: no ECS params, no RNG ownership, no logging. Every
//! function is a total function of its arguments so the decision layers
//! that call them stay deterministic and unit-testable. **INERT at this
//! landing** — no production system consumes these until the velocity
//! integrator (plan step 6) wires `DesiredVelocity` writers through
//! them; the landing gate is a footer-identical soak.
//!
//! ## Model
//!
//! Decision layers express *desire* (`seek` / `arrive` / `flee` /
//! `wander` / `pursue` / `separation` produce desired velocities in
//! world units per tick); the integrator turns desire into motion via
//! [`steer`] — an acceleration-limited velocity update that gives
//! momentum, curved turns, and no instant reversals. Paths from A*
//! ([`crate::ai::pathfinding::find_path`]) are 8-neighbor tile chains
//! whose headings are quantized to 45° multiples; [`smooth_path`]
//! string-pulls them into sparse world-space waypoints so travel
//! headings become arbitrary angles. Smoothing is cost-aware: a pruned
//! segment must not cross any tile more expensive than the corridor it
//! replaces (fox scent, corruption — the router paid to avoid those;
//! the smoother must not sneak back through). See
//! [`crate::ai::pathfinding::supercover_raycast_max_cost`].
//!
//! Speed caps are **Euclidean** (`clamp_length_max`) — with arbitrary
//! headings an L∞ cap would make ground speed direction-dependent
//! (+41% at 45°). Consequence carried by the plan: diagonal travel is
//! √2 slower than the grid era.

use bevy::math::Vec2;

use crate::ai::pathfinding::{supercover_raycast_max_cost, WeightedOverlay};
use crate::components::physical::Position;
use crate::resources::map::TileMap;

// ---------------------------------------------------------------------------
// Path smoothing (string pulling)
// ---------------------------------------------------------------------------

/// String-pull an A* tile path into sparse world-space waypoints.
///
/// `path` is `find_path` output: successive 8-neighbor tiles from (not
/// including) the mover's tile, ending at the goal tile. Returns the
/// world-space (tile-center) waypoints of the pruned corridor — every
/// retained waypoint is reachable from its predecessor along a straight
/// segment that (a) crosses only passable tiles and (b) never crosses a
/// tile whose `terrain.movement_cost() + sum(overlays)` exceeds the
/// most expensive tile of the original corridor span it replaces. (b)
/// is the cost-ceiling guard: without it, smoothing would shortcut
/// through fox-scent / corruption fields the router deliberately
/// detoured around.
///
/// The goal tile's waypoint is always retained. An empty `path` returns
/// an empty Vec. `start` is the mover's current world position (the
/// segment origin for the first pruning decision).
pub fn smooth_path(
    start: Vec2,
    path: &[Position],
    map: &TileMap,
    overlays: &[WeightedOverlay<'_>],
) -> Vec<Vec2> {
    if path.is_empty() {
        return Vec::new();
    }

    // Per-tile cost of each corridor tile, for the rolling ceiling.
    let tile_cost = |p: &Position| -> u32 {
        let t = map.get(p.x(), p.y()).terrain;
        t.movement_cost()
            .saturating_add(crate::ai::pathfinding::sum_overlay_cost(overlays, *p))
    };

    let mut out: Vec<Vec2> = Vec::new();
    let mut anchor: Vec2 = start;
    let mut i: usize = 0; // first corridor index not yet emitted / anchored

    while i < path.len() {
        // Furthest j such that anchor -> path[j] is clean under the
        // corridor ceiling for span i..=j.
        let mut best = i;
        let mut ceiling = tile_cost(&path[i]);
        let mut j = i + 1;
        let mut span_ceiling = ceiling;
        while j < path.len() {
            span_ceiling = span_ceiling.max(tile_cost(&path[j]));
            match supercover_raycast_max_cost(anchor, path[j].world(), map, overlays) {
                Some(max_cost) if max_cost <= span_ceiling => {
                    best = j;
                    ceiling = span_ceiling;
                    j += 1;
                }
                _ => break,
            }
        }
        let _ = ceiling;
        out.push(path[best].world());
        anchor = path[best].world();
        i = best + 1;
    }

    out
}

// ---------------------------------------------------------------------------
// Velocity integration
// ---------------------------------------------------------------------------

/// Acceleration-limited velocity update: move `vel` toward `desired` by
/// at most `max_accel` (world units per tick²). This is the single
/// place momentum comes from — a mover cannot reverse or reach full
/// speed instantly; it curves through turns at a radius set by
/// `max_accel / speed`.
///
/// Callers clamp the RESULT to the mover's max speed (the integrator
/// owns that cap; `steer` itself only limits the delta).
pub fn steer(vel: Vec2, desired: Vec2, max_accel: f32) -> Vec2 {
    vel + (desired - vel).clamp_length_max(max_accel.max(0.0))
}

// ---------------------------------------------------------------------------
// Desired-velocity generators
// ---------------------------------------------------------------------------

/// Full-speed desire straight at `target`. Zero when already there.
pub fn seek(pos: Vec2, target: Vec2, max_speed: f32) -> Vec2 {
    (target - pos).normalize_or_zero() * max_speed
}

/// Like [`seek`], but decelerates inside `slow_radius` so the mover
/// settles onto `target` instead of orbiting it. Zero at the target.
pub fn arrive(pos: Vec2, target: Vec2, max_speed: f32, slow_radius: f32) -> Vec2 {
    let offset = target - pos;
    let dist = offset.length();
    if dist <= f32::EPSILON {
        return Vec2::ZERO;
    }
    let speed = if slow_radius > f32::EPSILON && dist < slow_radius {
        max_speed * (dist / slow_radius)
    } else {
        max_speed
    };
    offset / dist * speed
}

/// Full-speed desire straight away from `threat`. When standing exactly
/// on the threat, flees along +X (arbitrary but deterministic — callers
/// with a better idea of "away" should displace `threat` themselves).
pub fn flee(pos: Vec2, threat: Vec2, max_speed: f32) -> Vec2 {
    let away = pos - threat;
    if away.length_squared() <= f32::EPSILON {
        return Vec2::X * max_speed;
    }
    away.normalize() * max_speed
}

/// Meander: current `heading` rotated by `angle_jitter * sample`
/// radians, at full speed. `sample` is a caller-supplied draw in
/// `[-1, 1]` — the primitive owns no RNG so the caller's seeded stream
/// stays the only entropy source. A zero `heading` starts along +X.
pub fn wander(heading: Vec2, max_speed: f32, angle_jitter: f32, sample: f32) -> Vec2 {
    let dir = if heading.length_squared() <= f32::EPSILON {
        Vec2::X
    } else {
        heading.normalize()
    };
    let angle = angle_jitter * sample.clamp(-1.0, 1.0);
    let (sin, cos) = angle.sin_cos();
    Vec2::new(dir.x * cos - dir.y * sin, dir.x * sin + dir.y * cos) * max_speed
}

/// Lead interception: seek the target's predicted position, using a
/// lead time of `distance / max_speed` (the time we'd need to cover the
/// current gap at full speed), capped at `max_lead_ticks` so a fast
/// escaping target doesn't produce absurd aim points. A stationary
/// target degenerates to [`seek`].
pub fn pursue(
    pos: Vec2,
    target_pos: Vec2,
    target_vel: Vec2,
    max_speed: f32,
    max_lead_ticks: f32,
) -> Vec2 {
    if max_speed <= f32::EPSILON {
        return Vec2::ZERO;
    }
    let lead = ((target_pos - pos).length() / max_speed).min(max_lead_ticks.max(0.0));
    seek(pos, target_pos + target_vel * lead, max_speed)
}

/// Personal-space force: for every neighbor closer than `radius`,
/// accumulate a push away from it that grows linearly as the gap
/// closes (zero at `radius`, unit-scale when coincident). Returns the
/// UNNORMALIZED sum — the caller blends it into a desired velocity at
/// its own weight. Replaces the jitter-teleport anti-stacking hacks
/// (plan step 7): personal space emerges instead of teleporting.
///
/// Coincident neighbors (distance ≈ 0) push along +X deterministically;
/// callers wanting symmetry-breaking should perturb inputs upstream
/// with their own seeded draw.
pub fn separation(pos: Vec2, neighbors: &[Vec2], radius: f32) -> Vec2 {
    if radius <= f32::EPSILON {
        return Vec2::ZERO;
    }
    let mut push = Vec2::ZERO;
    for &n in neighbors {
        let away = pos - n;
        let dist = away.length();
        if dist >= radius {
            continue;
        }
        let strength = (radius - dist) / radius;
        if dist <= f32::EPSILON {
            push += Vec2::X * strength;
        } else {
            push += away / dist * strength;
        }
    }
    push
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::pathfinding::find_path;
    use crate::resources::map::{Terrain, TileMap};

    fn open_map() -> TileMap {
        TileMap::new(20, 20, Terrain::Grass)
    }

    // ---- steer ------------------------------------------------------

    #[test]
    fn steer_limits_delta_to_max_accel() {
        let vel = Vec2::new(1.0, 0.0);
        let desired = Vec2::new(-1.0, 0.0); // full reversal requested
        let out = steer(vel, desired, 0.25);
        // Moved toward desired by exactly max_accel, no instant reversal.
        assert!((out - Vec2::new(0.75, 0.0)).length() < 1e-6, "got {out:?}");
    }

    #[test]
    fn steer_reaches_desired_when_within_accel() {
        let out = steer(Vec2::new(0.9, 0.0), Vec2::new(1.0, 0.0), 0.25);
        assert!((out - Vec2::new(1.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn steer_produces_curved_turn_not_pivot() {
        // Moving +X, desiring +Y: one steer step yields a diagonal-ish
        // velocity (both components non-zero) — a curve, not a pivot.
        let out = steer(Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), 0.25);
        assert!(out.x > 0.0 && out.y > 0.0, "got {out:?}");
    }

    // ---- generators ---------------------------------------------------

    #[test]
    fn seek_is_full_speed_toward_target() {
        let d = seek(Vec2::new(0.0, 0.0), Vec2::new(3.0, 4.0), 2.0);
        assert!((d.length() - 2.0).abs() < 1e-6);
        assert!(d.x > 0.0 && d.y > 0.0);
    }

    #[test]
    fn arrive_decelerates_inside_slow_radius() {
        let far = arrive(Vec2::ZERO, Vec2::new(10.0, 0.0), 1.0, 2.0);
        let near = arrive(Vec2::ZERO, Vec2::new(1.0, 0.0), 1.0, 2.0);
        assert!((far.length() - 1.0).abs() < 1e-6);
        assert!((near.length() - 0.5).abs() < 1e-6);
        assert_eq!(arrive(Vec2::ZERO, Vec2::ZERO, 1.0, 2.0), Vec2::ZERO);
    }

    #[test]
    fn flee_points_away_at_full_speed() {
        let d = flee(Vec2::new(1.0, 1.0), Vec2::new(0.0, 0.0), 1.4);
        assert!((d.length() - 1.4).abs() < 1e-6);
        assert!(d.x > 0.0 && d.y > 0.0);
        // Coincident: deterministic +X.
        assert_eq!(flee(Vec2::ZERO, Vec2::ZERO, 1.0), Vec2::X);
    }

    #[test]
    fn wander_rotates_heading_within_jitter_band() {
        let heading = Vec2::X;
        let out = wander(heading, 1.0, std::f32::consts::FRAC_PI_4, 1.0);
        // Rotated by exactly +45°.
        let expected = Vec2::new(
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        );
        assert!((out - expected).length() < 1e-5, "got {out:?}");
        // Zero sample = straight ahead; zero heading starts along +X.
        assert!((wander(heading, 1.0, 1.0, 0.0) - Vec2::X).length() < 1e-6);
        assert!((wander(Vec2::ZERO, 1.0, 1.0, 0.0) - Vec2::X).length() < 1e-6);
    }

    #[test]
    fn pursue_leads_a_moving_target() {
        // Target due east moving north: aim point must be north of the
        // target's current position.
        let d = pursue(
            Vec2::ZERO,
            Vec2::new(4.0, 0.0),
            Vec2::new(0.0, 1.0),
            1.0,
            8.0,
        );
        assert!(d.y > 0.0, "pursuit must lead north, got {d:?}");
        // Stationary target degenerates to seek.
        let s = pursue(Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::ZERO, 1.0, 8.0);
        assert!((s - seek(Vec2::ZERO, Vec2::new(4.0, 0.0), 1.0)).length() < 1e-6);
    }

    #[test]
    fn separation_pushes_away_and_scales_with_proximity() {
        let close = separation(Vec2::ZERO, &[Vec2::new(0.1, 0.0)], 0.6);
        let far = separation(Vec2::ZERO, &[Vec2::new(0.5, 0.0)], 0.6);
        assert!(close.x < 0.0 && far.x < 0.0, "push must be away (-X)");
        assert!(
            close.length() > far.length(),
            "closer neighbor pushes harder"
        );
        assert_eq!(
            separation(Vec2::ZERO, &[Vec2::new(2.0, 0.0)], 0.6),
            Vec2::ZERO
        );
    }

    // ---- smooth_path ---------------------------------------------------

    #[test]
    fn smooth_path_collapses_straight_and_open_runs() {
        let map = open_map();
        let from = Position::new(1, 1);
        let to = Position::new(9, 5);
        let path = find_path(from, to, &map, &[]).expect("path on open map");
        let smoothed = smooth_path(from.world(), &path, &map, &[]);
        // Open terrain: the entire staircase collapses to one segment.
        assert_eq!(
            smoothed,
            vec![to.world()],
            "open-terrain path should smooth to a single waypoint; got {smoothed:?}"
        );
    }

    #[test]
    fn smooth_path_keeps_goal_and_respects_walls() {
        let mut map = open_map();
        // Vertical water wall x=5 with a gap at y=9.
        for y in 0..20 {
            if y != 9 {
                map.set(5, y, Terrain::Water);
            }
        }
        let from = Position::new(1, 1);
        let to = Position::new(9, 1);
        let path = find_path(from, to, &map, &[]).expect("path through the gap");
        let smoothed = smooth_path(from.world(), &path, &map, &[]);
        assert_eq!(*smoothed.last().unwrap(), to.world(), "goal retained");
        // Every MULTI-STEP segment must survive its own raycast on
        // passable terrain (no wall-clipping shortcuts). Single
        // 8-neighbor steps are exempt: A* permits diagonal moves that
        // cut a blocked corner, supercover (correctly, conservatively)
        // does not — such steps are simply never pruned.
        let single_step = std::f32::consts::SQRT_2 + 1e-3;
        let mut prev = from.world();
        for w in &smoothed {
            if (*w - prev).length() > single_step {
                assert!(
                    supercover_raycast_max_cost(prev, *w, &map, &[]).is_some(),
                    "multi-step segment {prev:?} -> {w:?} crosses impassable terrain"
                );
            }
            prev = *w;
        }
        // And the wall forces at least one intermediate waypoint.
        assert!(
            smoothed.len() >= 2,
            "wall detour cannot smooth to a single segment; got {smoothed:?}"
        );
    }

    /// Plan risk #7 — the smoothing/overlay interaction guard: a
    /// smoothed path around a high-cost fox-scent field must never
    /// shortcut through tiles above the corridor's cost ceiling.
    #[test]
    fn smooth_path_never_crosses_above_corridor_ceiling() {
        use crate::ai::pathfinding::TileCostOverlay;

        struct ScentBlob;
        impl TileCostOverlay for ScentBlob {
            fn cost_at(&self, pos: Position) -> u32 {
                // Expensive blob centered on (5, 3) covering a 3x3 patch.
                let (x, y) = (pos.x(), pos.y());
                if (4..=6).contains(&x) && (2..=4).contains(&y) {
                    50
                } else {
                    0
                }
            }
        }

        let map = open_map();
        let blob = ScentBlob;
        let overlays = [WeightedOverlay::new(&blob, 1.0)];

        let from = Position::new(1, 3);
        let to = Position::new(9, 3);
        let path = find_path(from, to, &map, &overlays).expect("router detours the blob");
        // Sanity: the router itself avoided the blob.
        let corridor_ceiling = path
            .iter()
            .map(|p| {
                map.get(p.x(), p.y()).terrain.movement_cost()
                    + crate::ai::pathfinding::sum_overlay_cost(&overlays, *p)
            })
            .max()
            .unwrap();
        assert!(corridor_ceiling < 50, "router should have detoured");

        let smoothed = smooth_path(from.world(), &path, &map, &overlays);
        // Walk every smoothed segment; no crossed tile may exceed the
        // corridor ceiling.
        let mut prev = from.world();
        for w in &smoothed {
            let max_cost = supercover_raycast_max_cost(prev, *w, &map, &overlays)
                .expect("smoothed segment must be passable");
            assert!(
                max_cost <= corridor_ceiling,
                "smoothed segment {prev:?} -> {w:?} crosses cost {max_cost} above \
                 corridor ceiling {corridor_ceiling} — shortcut through the scent field"
            );
            prev = *w;
        }
    }
}
