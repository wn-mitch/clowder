//! Velocity integrator — 0.4.0 "Free Range" fluid locomotion
//! (ticket 140 / plan step 6).
//!
//! The single system that turns [`DesiredVelocity`] (written by
//! migrated decision-layer resolvers) into [`Position`] motion via
//! [`Velocity`] under the acceleration cap. Runs at the **head of
//! Chain 4, before `emit_cat_moved_messages`** — after every decision
//! writer, so CatMoved subscribers (NearPairCache etc.) and the render
//! interpolation see post-move positions the same tick.
//!
//! ## Per-tick justification
//! Physics. One of the four sanctioned per-tick families (plan exec,
//! sense+score, decay, physics — CLAUDE.md ECS rules).
//!
//! ## Contract (mirrors `DesiredVelocity`'s rustdoc)
//! - Desire present → `vel = steer(vel, desired, max_accel)`, clamped
//!   **Euclidean** to `MovementBudget.per_tick` (reinterpreted as max
//!   speed; the `escape_viability` read of `per_tick` survives), then
//!   sub-stepped through the map with wall-slide.
//! - Desire absent → `Velocity` zeroes immediately and the entity does
//!   not move. This is the staged-migration bisectability invariant:
//!   unmigrated resolvers write `Position` directly and express no
//!   desire, so nothing can be moved twice in one tick.
//! - [`Flying`] movers skip passability entirely; they get the map
//!   bounds clamp only.
//!
//! ## Anti-tunneling
//! Motion is applied in sub-steps of ≤ 0.45 tiles (< half a tile), so
//! even `bird_burst_speed = 3.0` cannot cross a wall tile between
//! passability checks. On a blocked sub-step the integrator
//! wall-slides — tries `(new.x, old.y)` then `(old.x, new.y)`,
//! zeroing the blocked velocity component — and stops dead (velocity
//! zeroed) only when both slide axes are blocked.

use bevy_ecs::prelude::*;

use crate::components::movement_budget::MovementBudget;
use crate::components::physical::{DesiredVelocity, Flying, Position, Velocity};
use crate::resources::map::TileMap;
use crate::resources::SimConstants;

/// Max sub-step length in tiles. Strictly less than 0.5 so a mover
/// can never skip over a tile column/row between passability checks.
const SUB_STEP_LEN: f32 = 0.45;

#[allow(clippy::type_complexity)]
pub fn integrate_velocities(
    mut movers: Query<(
        &mut Position,
        &mut Velocity,
        &mut DesiredVelocity,
        &MovementBudget,
        Option<&Flying>,
    )>,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
) {
    let max_accel = constants.movement.max_accel;
    let passable = |p: bevy::math::Vec2| -> bool {
        let (tx, ty) = (p.x.floor() as i32, p.y.floor() as i32);
        map.in_bounds(tx, ty) && map.get(tx, ty).terrain.is_passable()
    };

    for (mut pos, mut vel, mut desired, budget, flying) in &mut movers {
        // Consume the desire (clear-on-read — the contract).
        let want = desired.0.take();
        let Some(want) = want else {
            // Bisectability invariant: no desire ⇒ no motion, and any
            // leftover momentum is dropped so a legacy Position-writing
            // resolver can never be double-moved by stale velocity.
            if vel.0 != bevy::math::Vec2::ZERO {
                vel.0 = bevy::math::Vec2::ZERO;
            }
            continue;
        };

        // 140 step 9 — airborne movers turn harder: `Flying` selects
        // `hawk_max_accel` (hawks now; step 10's burst birds inherit).
        // Ground movers share the uniform `max_accel`.
        let accel = if flying.is_some() {
            constants.movement.hawk_max_accel
        } else {
            max_accel
        };
        vel.0 = crate::ai::steering::steer(vel.0, want, accel);
        // Euclidean speed cap — with arbitrary headings an L∞ cap
        // would make ground speed direction-dependent (+41% at 45°).
        let max_speed = budget.per_tick.max(0.0);
        let speed = vel.0.length();
        if speed > max_speed && speed > f32::EPSILON {
            vel.0 *= max_speed / speed;
        }
        if vel.0.length_squared() <= 1e-10 {
            continue;
        }

        let n = (vel.0.length() / SUB_STEP_LEN).ceil().max(1.0) as u32;
        let dv = vel.0 / n as f32;
        let mut cur = pos.0;

        if flying.is_some() {
            // Terrain-exempt: bounds clamp only (keep a half-tile
            // margin so the containing tile stays in-bounds).
            cur += vel.0;
            cur.x = cur.x.clamp(0.5, map.width as f32 - 0.5);
            cur.y = cur.y.clamp(0.5, map.height as f32 - 0.5);
            pos.0 = cur;
            continue;
        }

        for _ in 0..n {
            let cand = cur + dv;
            if passable(cand) {
                cur = cand;
                continue;
            }
            // Wall-slide: keep the passable axis component, zero the
            // blocked one (momentum stops pushing into the wall).
            let slide_x = bevy::math::Vec2::new(cand.x, cur.y);
            let slide_y = bevy::math::Vec2::new(cur.x, cand.y);
            if passable(slide_x) {
                cur = slide_x;
                vel.0.y = 0.0;
            } else if passable(slide_y) {
                cur = slide_y;
                vel.0.x = 0.0;
            } else {
                vel.0 = bevy::math::Vec2::ZERO;
                break;
            }
        }
        pos.0 = cur;
    }
}

/// 140 step 7 — personal-space desire pass. Replaces the retired
/// jitter/arrival teleports (`jitter_if_stacked`, travel-arrival
/// snap+jitter): cats standing inside each other's
/// `movement.separation_radius` accumulate a `steering::separation`
/// push that blends into their `DesiredVelocity` — idle stacked cats
/// drift apart over a few ticks, traveling cats bow around each other,
/// and nobody teleports.
///
/// Pair source is `NearPairCache` (already maintained event-driven,
/// already restricted to cats + wildlife by 506); wildlife endpoints
/// are skipped by the `DesiredVelocity` query miss until their species
/// migrations land. Accumulation runs in BTreeMap pair order into a
/// BTreeMap accumulator — deterministic float summation order.
///
/// Runs immediately BEFORE `integrate_velocities` in the Chain-4
/// nested chain so the push lands in the same tick's integration.
pub fn apply_separation(
    cache: Res<crate::resources::near_pair_cache::NearPairCache>,
    constants: Res<SimConstants>,
    mut movers: Query<(&Position, &mut DesiredVelocity)>,
    // Pairs with a DEPENDENT KITTEN endpoint are exempt: kitten
    // stacking is prosocial substrate (nursing, feeding, cuddle
    // piles). The first step-7 soak shoved feeding pairs apart every
    // tick and starved two kittens (Duskkit-45 t1299805, Sparkkit-34
    // t1310268, logs/tuned-42-60ab5916) — the kittens-are-cats
    // sister-defect family.
    kittens: Query<(), With<crate::components::kitten::KittenDependency>>,
) {
    let radius = constants.movement.separation_radius;
    if radius <= f32::EPSILON {
        return;
    }
    let max_speed = constants.movement.cat_max_speed;

    // Accumulate pushes in deterministic (BTreeMap) order.
    let mut pushes: std::collections::BTreeMap<Entity, bevy::math::Vec2> =
        std::collections::BTreeMap::new();
    for (&(a, b), _) in cache.pairs.iter() {
        if kittens.contains(a) || kittens.contains(b) {
            continue;
        }
        let (Ok((pa, _)), Ok((pb, _))) = (movers.get(a), movers.get(b)) else {
            continue;
        };
        let (pa, pb) = (pa.0, pb.0);
        if pa.distance(pb) >= radius {
            continue;
        }
        let push_a = crate::ai::steering::separation(pa, &[pb], radius);
        let push_b = crate::ai::steering::separation(pb, &[pa], radius);
        *pushes.entry(a).or_insert(bevy::math::Vec2::ZERO) += push_a;
        *pushes.entry(b).or_insert(bevy::math::Vec2::ZERO) += push_b;
    }

    for (entity, push) in pushes {
        if push.length_squared() <= f32::EPSILON {
            continue;
        }
        let Ok((_, mut desired)) = movers.get_mut(entity) else {
            continue;
        };
        // Per-neighbor strength is already (radius - d) / radius in
        // [0, 1]; scaling by max_speed makes a fully-overlapped cat
        // want full-speed escape and a barely-touching one a nudge.
        let blend = push.clamp_length_max(1.0) * max_speed;
        desired.0 = Some(match desired.0 {
            Some(d) => d + blend,
            None => blend,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;
    use bevy::math::Vec2;
    use bevy_ecs::schedule::Schedule;

    fn world_with_map() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(integrate_velocities);
        (world, schedule)
    }

    fn spawn_mover(world: &mut World, pos: Position) -> Entity {
        world
            .spawn((
                pos,
                Velocity::default(),
                DesiredVelocity::default(),
                MovementBudget::cat(),
            ))
            .id()
    }

    #[test]
    fn no_desire_means_no_motion_and_zeroed_velocity() {
        let (mut world, mut schedule) = world_with_map();
        let e = spawn_mover(&mut world, Position::new(5, 5));
        // Seed stale momentum, no desire.
        world.get_mut::<Velocity>(e).unwrap().0 = Vec2::new(1.0, 0.0);
        schedule.run(&mut world);
        assert_eq!(world.get::<Position>(e).unwrap().tile(), (5, 5));
        assert_eq!(world.get::<Velocity>(e).unwrap().0, Vec2::ZERO);
    }

    #[test]
    fn desire_accelerates_under_cap_and_moves() {
        let (mut world, mut schedule) = world_with_map();
        let e = spawn_mover(&mut world, Position::new(5, 5));
        let start = world.get::<Position>(e).unwrap().0;
        world.get_mut::<DesiredVelocity>(e).unwrap().0 = Some(Vec2::new(1.0, 0.0));
        schedule.run(&mut world);
        let after = world.get::<Position>(e).unwrap().0;
        let moved = after - start;
        // One tick from rest: |v| = max_accel (0.25), along +X.
        let accel = SimConstants::default().movement.max_accel;
        assert!((moved.x - accel).abs() < 1e-5, "moved {moved:?}");
        assert!(moved.y.abs() < 1e-6);
        // Desire consumed.
        assert!(world.get::<DesiredVelocity>(e).unwrap().0.is_none());
    }

    #[test]
    fn velocity_persists_and_reaches_speed_cap() {
        let (mut world, mut schedule) = world_with_map();
        let e = spawn_mover(&mut world, Position::new(2, 5));
        for _ in 0..10 {
            world.get_mut::<DesiredVelocity>(e).unwrap().0 = Some(Vec2::new(5.0, 0.0));
            schedule.run(&mut world);
        }
        let v = world.get::<Velocity>(e).unwrap().0;
        // Clamped to MovementBudget::cat().per_tick = 1.0, never above.
        assert!((v.length() - 1.0).abs() < 1e-4, "v = {v:?}");
    }

    #[test]
    fn wall_slide_keeps_passable_axis() {
        let (mut world, mut schedule) = world_with_map();
        {
            let mut map = world.resource_mut::<TileMap>();
            // Wall directly east of the mover.
            map.set(6, 5, Terrain::Water);
        }
        let e = spawn_mover(&mut world, Position::new(5, 5));
        // Desire diagonal NE into the wall column; slide should keep +Y.
        for _ in 0..8 {
            world.get_mut::<DesiredVelocity>(e).unwrap().0 = Some(Vec2::new(1.0, 1.0));
            schedule.run(&mut world);
        }
        let p = world.get::<Position>(e).unwrap();
        assert!(
            world
                .resource::<TileMap>()
                .get(p.tile().0, p.tile().1)
                .terrain
                .is_passable(),
            "mover ended on impassable tile {:?}",
            p.tile()
        );
        assert!(p.0.y > 5.9, "should have slid along +Y; at {:?}", p.0);
    }

    #[test]
    fn burst_speed_cannot_tunnel_through_wall() {
        let (mut world, mut schedule) = world_with_map();
        {
            let mut map = world.resource_mut::<TileMap>();
            // 1-tile wall across the mover's path.
            for y in 0..20 {
                map.set(8, y, Terrain::Water);
            }
        }
        let e = world
            .spawn((
                Position::new(6, 5),
                Velocity::default(),
                DesiredVelocity::default(),
                // Burst-speed budget (bird escape profile).
                MovementBudget {
                    accumulator: 3.0,
                    per_tick: 3.0,
                },
            ))
            .id();
        for _ in 0..6 {
            world.get_mut::<DesiredVelocity>(e).unwrap().0 = Some(Vec2::new(3.0, 0.0));
            schedule.run(&mut world);
        }
        let p = world.get::<Position>(e).unwrap();
        assert!(
            p.tile().0 < 8,
            "ground mover must not tunnel through the x=8 wall; at {:?}",
            p.tile()
        );
    }

    #[test]
    fn stacked_idle_cats_drift_apart_without_teleporting() {
        let (mut world, schedule) = world_with_map();
        // Two cats on the SAME tile center — the retired jitter case.
        let a = spawn_mover(&mut world, Position::new(5, 5));
        let b = spawn_mover(&mut world, Position::new(5, 5));
        // Nudge b off exact coincidence so the push direction is
        // defined by geometry, not the coincident +X fallback.
        world.get_mut::<Position>(b).unwrap().0 += Vec2::new(0.05, 0.0);
        world.insert_resource(crate::resources::near_pair_cache::NearPairCache::default());
        {
            let mut cache =
                world.resource_mut::<crate::resources::near_pair_cache::NearPairCache>();
            let key = crate::resources::near_pair_cache::normalize_pair(a, b);
            cache.pairs.insert(key, 0.0);
        }
        let mut sep_schedule = Schedule::default();
        sep_schedule.add_systems((apply_separation, integrate_velocities).chain());
        let d0 = {
            let pa = world.get::<Position>(a).unwrap().0;
            let pb = world.get::<Position>(b).unwrap().0;
            pa.distance(pb)
        };
        for _ in 0..8 {
            sep_schedule.run(&mut world);
        }
        let pa = world.get::<Position>(a).unwrap().0;
        let pb = world.get::<Position>(b).unwrap().0;
        assert!(
            pa.distance(pb) > d0 + 0.1,
            "stacked cats should drift apart; d0={d0} d={}",
            pa.distance(pb)
        );
        let _ = schedule;
    }

    #[test]
    fn flying_ignores_walls_but_respects_bounds() {
        let (mut world, mut schedule) = world_with_map();
        {
            let mut map = world.resource_mut::<TileMap>();
            for y in 0..20 {
                map.set(8, y, Terrain::Water);
            }
        }
        let e = world
            .spawn((
                Position::new(6, 5),
                Velocity::default(),
                DesiredVelocity::default(),
                MovementBudget {
                    accumulator: 3.0,
                    per_tick: 3.0,
                },
                Flying,
            ))
            .id();
        for _ in 0..20 {
            world.get_mut::<DesiredVelocity>(e).unwrap().0 = Some(Vec2::new(3.0, 0.0));
            schedule.run(&mut world);
        }
        let p = world.get::<Position>(e).unwrap();
        assert!(
            p.tile().0 > 8,
            "flier should cross the wall; at {:?}",
            p.tile()
        );
        assert!(
            p.0.x <= 19.5 + 1e-4,
            "flier must stay inside map bounds; at {:?}",
            p.0
        );
    }
}
