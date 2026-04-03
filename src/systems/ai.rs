use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::scoring::{score_actions, select_best_action};
use crate::ai::{Action, CurrentAction};
use crate::components::personality::Personality;
use crate::components::physical::{Needs, Position};
use crate::resources::map::TileMap;
use crate::resources::rng::SimRng;

// ---------------------------------------------------------------------------
// evaluate_actions system
// ---------------------------------------------------------------------------

/// Score available actions for every cat whose current action has finished
/// (`ticks_remaining == 0`) and assign the best-scoring next action.
///
/// Phase 1 assumptions:
/// - Food is always considered available (`food_available = true`).
/// - Wander target is chosen as a random offset (±5 tiles) from current position.
pub fn evaluate_actions(
    mut query: Query<(&Needs, &Personality, &Position, &mut CurrentAction)>,
    _map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
) {
    for (needs, personality, pos, mut current) in &mut query {
        // Only re-evaluate when the current action is complete.
        if current.ticks_remaining != 0 {
            continue;
        }

        let scores = score_actions(needs, personality, true, &mut rng.rng);
        let chosen = select_best_action(&scores);

        match chosen {
            Action::Eat => {
                current.action = Action::Eat;
                current.ticks_remaining = 5;
                current.target_position = None;
            }
            Action::Sleep => {
                current.action = Action::Sleep;
                current.ticks_remaining = 20;
                current.target_position = None;
            }
            Action::Wander => {
                let dx: i32 = rng.rng.random_range(-5i32..=5);
                let dy: i32 = rng.rng.random_range(-5i32..=5);
                let target = Position::new(pos.x + dx, pos.y + dy);
                current.action = Action::Wander;
                current.ticks_remaining = 10;
                current.target_position = Some(target);
            }
            Action::Idle => {
                current.action = Action::Idle;
                current.ticks_remaining = 5;
                current.target_position = None;
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

    use crate::components::personality::Personality;
    use crate::resources::map::{Terrain, TileMap};

    fn default_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        let mut schedule = Schedule::default();
        schedule.add_systems(evaluate_actions);
        (world, schedule)
    }

    /// A cat with ticks_remaining == 0 should get a new action assigned.
    #[test]
    fn assigns_action_when_idle() {
        let (mut world, mut schedule) = setup_world();

        let entity = world
            .spawn((
                Needs::default(),
                default_personality(),
                Position::new(10, 10),
                CurrentAction::default(), // ticks_remaining = 0
            ))
            .id();

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert!(
            ca.ticks_remaining > 0,
            "should have assigned a new action with ticks > 0"
        );
    }

    /// A cat mid-action (ticks_remaining > 0) should not have its action replaced.
    #[test]
    fn does_not_replace_active_action() {
        let (mut world, mut schedule) = setup_world();

        let entity = world
            .spawn((
                Needs::default(),
                default_personality(),
                Position::new(10, 10),
                CurrentAction {
                    action: Action::Sleep,
                    ticks_remaining: 15,
                    target_position: None,
                },
            ))
            .id();

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert_eq!(ca.action, Action::Sleep, "active Sleep should not be replaced");
        assert_eq!(ca.ticks_remaining, 15, "ticks_remaining should be unchanged");
    }

    /// A starving cat (hunger=0.05) should be assigned Eat.
    #[test]
    fn starving_cat_chooses_eat() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.hunger = 0.05;
        needs.energy = 0.9;

        let entity = world
            .spawn((
                needs,
                default_personality(),
                Position::new(5, 5),
                CurrentAction::default(),
            ))
            .id();

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert_eq!(ca.action, Action::Eat, "starving cat should choose Eat");
        assert_eq!(ca.ticks_remaining, 5);
        assert!(ca.target_position.is_none());
    }
}
