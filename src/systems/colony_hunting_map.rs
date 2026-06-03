//! Substrate-derived rebuild for [`ColonyHuntingMap`].
//!
//! Ticket 293: the colony-wide hunting belief map is no longer maintained
//! by per-interaction `absorb` writes from social steps. Instead it's
//! derived each cadence from the per-cat `LocationBeliefs.prey_yield`
//! facet — max-over-cats with a strength-floor gate — using the
//! [`crate::systems::belief_aggregation::aggregate_location_belief_snapshot`]
//! helper that ticket 294 introduced. The visualization-consumer reader
//! (`snapshot.rs::record_spatial_events`) keeps reading the same passive
//! grid; only the value source changed.
//!
//! Cadence ties to `BeliefsConstants::decay_stagger_period` (default 20
//! ticks). Per-tick rebuild would be wasteful — the underlying facets
//! only update via EMA on the same stagger cadence.

use bevy_ecs::prelude::*;

use crate::components::beliefs::{FacetSlot, LocationBeliefs};
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;
use crate::resources::ColonyHuntingMap;
use crate::systems::belief_aggregation::aggregate_location_belief_snapshot;

/// Rebuild [`ColonyHuntingMap`] from per-cat `LocationBeliefs.prey_yield`
/// snapshots. Runs on the same stagger cadence as the belief-decay pass
/// so reads after each integration step see the latest aggregate.
pub fn rebuild_colony_hunting_map(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    cats: Query<&LocationBeliefs>,
    mut colony_map: ResMut<ColonyHuntingMap>,
) {
    let period = constants.beliefs.decay_stagger_period;
    if period == 0 || !time.tick.is_multiple_of(period) {
        return;
    }
    let snapshot = aggregate_location_belief_snapshot(
        cats.iter(),
        FacetSlot::PreyYield,
        &constants.belief_aggregation,
    );
    colony_map.reset_to_prior();
    let grid_w = colony_map.grid_w;
    let grid_h = colony_map.grid_h;
    for ((bx, by), value) in snapshot.iter() {
        // `LocationKey` already encodes 5-tile-bucket coordinates, which
        // matches `ColonyHuntingMap::BUCKET_SIZE = 5` — no rescaling.
        if (0..grid_w as i32).contains(bx) && (0..grid_h as i32).contains(by) {
            colony_map.beliefs[(*by as usize) * grid_w + (*bx as usize)] = *value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::beliefs::{bucket_position, MentalModel};
    use crate::resources::colony_hunting_map::DEFAULT_PRIOR;

    fn make_cat(world: &mut World, bucket: (i32, i32), value: f32, strength: f32) -> Entity {
        let mut lb = LocationBeliefs::default();
        let key = bucket_position(bucket.0 * 5, bucket.1 * 5);
        let mut model = MentalModel::default();
        model.prey_yield.value = value;
        model.prey_yield.strength = strength;
        lb.models.insert(key, model);
        world.spawn(lb).id()
    }

    fn run_rebuild(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(rebuild_colony_hunting_map);
        schedule.run(world);
    }

    #[test]
    fn substrate_rebuild_writes_max_over_cats_per_bucket() {
        let mut world = World::new();
        world.insert_resource(TimeState::default()); // tick = 0, multiple of stagger
        world.insert_resource(SimConstants::default());
        world.insert_resource(ColonyHuntingMap::default());

        make_cat(&mut world, (2, 2), 0.3, 1.0);
        make_cat(&mut world, (2, 2), 0.8, 1.0); // wins
        make_cat(&mut world, (4, 4), 0.5, 1.0);

        run_rebuild(&mut world);

        let map = world.resource::<ColonyHuntingMap>();
        // Bucket (2,2) covers tiles 10..14. Sample any tile in that bucket.
        assert!((map.get(10, 10) - 0.8).abs() < f32::EPSILON);
        assert!((map.get(20, 20) - 0.5).abs() < f32::EPSILON);
        // Bucket nobody observed → DEFAULT_PRIOR
        assert!((map.get(50, 50) - DEFAULT_PRIOR).abs() < f32::EPSILON);
    }

    #[test]
    fn rebuild_skips_ticks_off_stagger() {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = 5; // not a multiple of 20
        world.insert_resource(time);
        world.insert_resource(SimConstants::default());
        let mut map = ColonyHuntingMap::default();
        // Seed a sentinel value at one cell — after the system runs it
        // should be unchanged (because the cadence gate skips).
        map.beliefs[0] = 0.123;
        world.insert_resource(map);
        make_cat(&mut world, (2, 2), 0.9, 1.0);

        run_rebuild(&mut world);

        let map = world.resource::<ColonyHuntingMap>();
        assert!((map.beliefs[0] - 0.123).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_world_resets_to_prior() {
        let mut world = World::new();
        world.insert_resource(TimeState::default());
        world.insert_resource(SimConstants::default());
        let mut map = ColonyHuntingMap::default();
        for cell in map.beliefs.iter_mut() {
            *cell = 0.95;
        }
        world.insert_resource(map);

        run_rebuild(&mut world);

        let map = world.resource::<ColonyHuntingMap>();
        for cell in map.beliefs.iter() {
            assert!((cell - DEFAULT_PRIOR).abs() < f32::EPSILON);
        }
    }
}
