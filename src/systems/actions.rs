use bevy_ecs::prelude::*;

use crate::ai::{Action, CurrentAction};
use crate::components::physical::{Needs, Position};
use crate::resources::map::TileMap;
use crate::ai::pathfinding::step_toward;

// ---------------------------------------------------------------------------
// resolve_actions system
// ---------------------------------------------------------------------------

/// Advance every in-progress cat action by one tick and apply its effect.
///
/// - Decrements `ticks_remaining` first.
/// - Then applies the per-tick effect for the action that is now in progress.
/// - Movement (Wander) calls `step_toward` each tick so the cat gradually
///   closes on its target.
pub fn resolve_actions(
    mut query: Query<(Entity, &mut CurrentAction, &mut Needs, &mut Position)>,
    map: Res<TileMap>,
) {
    for (_entity, mut current, mut needs, mut pos) in &mut query {
        if current.ticks_remaining == 0 {
            continue;
        }

        current.ticks_remaining -= 1;

        match current.action {
            Action::Eat => {
                needs.hunger = (needs.hunger + 0.04).min(1.0);
            }
            Action::Sleep => {
                needs.energy = (needs.energy + 0.02).min(1.0);
                needs.warmth = (needs.warmth + 0.01).min(1.0);
            }
            Action::Wander => {
                if let Some(target) = current.target_position {
                    if let Some(next) = step_toward(&pos, &target, &map) {
                        *pos = next;
                    }
                }
            }
            Action::Idle => {
                // No effect.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    use crate::resources::map::{Terrain, TileMap};

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_actions);
        (world, schedule)
    }

    /// Eating should increase hunger each tick.
    #[test]
    fn eating_restores_hunger() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.hunger = 0.5;

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Eat,
                    ticks_remaining: 3,
                    target_position: None,
                },
                needs,
                Position::new(5, 5),
            ))
            .id();

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert!(
            (n.hunger - 0.54).abs() < 1e-5,
            "hunger should be ~0.54 after one eat tick; got {}",
            n.hunger
        );
    }

    /// Eating should not push hunger above 1.0.
    #[test]
    fn eating_clamps_hunger_at_one() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.hunger = 0.99;

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Eat,
                    ticks_remaining: 2,
                    target_position: None,
                },
                needs,
                Position::new(5, 5),
            ))
            .id();

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, 1.0, "hunger should clamp at 1.0; got {}", n.hunger);
    }

    /// Sleeping should restore energy and warmth each tick.
    #[test]
    fn sleeping_restores_energy_and_warmth() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.energy = 0.5;
        needs.warmth = 0.5;

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Sleep,
                    ticks_remaining: 5,
                    target_position: None,
                },
                needs,
                Position::new(5, 5),
            ))
            .id();

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert!(
            (n.energy - 0.52).abs() < 1e-5,
            "energy should be ~0.52; got {}",
            n.energy
        );
        assert!(
            (n.warmth - 0.51).abs() < 1e-5,
            "warmth should be ~0.51; got {}",
            n.warmth
        );
    }

    /// Wandering with a target should move the cat each tick.
    #[test]
    fn wandering_moves_cat_toward_target() {
        let (mut world, mut schedule) = setup_world();

        let start = Position::new(0, 0);
        let target = Position::new(5, 5);

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Wander,
                    ticks_remaining: 10,
                    target_position: Some(target),
                },
                Needs::default(),
                start,
            ))
            .id();

        schedule.run(&mut world);

        let pos = *world.get::<Position>(entity).unwrap();
        let before_dist = start.manhattan_distance(&target);
        let after_dist = pos.manhattan_distance(&target);
        assert!(
            after_dist < before_dist,
            "cat should have moved closer to target; before={before_dist}, after={after_dist}"
        );
    }

    /// Idle action should have no effect on needs.
    #[test]
    fn idle_has_no_effect() {
        let (mut world, mut schedule) = setup_world();

        let needs = Needs::default();
        let hunger_before = needs.hunger;
        let energy_before = needs.energy;

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Idle,
                    ticks_remaining: 3,
                    target_position: None,
                },
                needs,
                Position::new(5, 5),
            ))
            .id();

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, hunger_before, "idle should not change hunger");
        assert_eq!(n.energy, energy_before, "idle should not change energy");
    }

    /// ticks_remaining is decremented each run.
    #[test]
    fn ticks_remaining_decrements() {
        let (mut world, mut schedule) = setup_world();

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Idle,
                    ticks_remaining: 5,
                    target_position: None,
                },
                Needs::default(),
                Position::new(5, 5),
            ))
            .id();

        schedule.run(&mut world);
        let after_one = world.get::<CurrentAction>(entity).unwrap().ticks_remaining;
        assert_eq!(after_one, 4);

        schedule.run(&mut world);
        let after_two = world.get::<CurrentAction>(entity).unwrap().ticks_remaining;
        assert_eq!(after_two, 3);
    }

    /// An entity with ticks_remaining == 0 should not be affected.
    #[test]
    fn zero_ticks_remaining_is_skipped() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.hunger = 0.5;

        let entity = world
            .spawn((
                CurrentAction {
                    action: Action::Eat,
                    ticks_remaining: 0,
                    target_position: None,
                },
                needs,
                Position::new(5, 5),
            ))
            .id();

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, 0.5, "zero-tick action should not modify needs");
    }
}
