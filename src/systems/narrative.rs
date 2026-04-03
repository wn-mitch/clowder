use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::identity::Name;
use crate::components::physical::Needs;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// generate_narrative system
// ---------------------------------------------------------------------------

/// Emit a narrative line for each cat that is on the last tick of its current
/// action (`ticks_remaining == 1`).
///
/// Narrating at tick 1 (rather than 0) means each action produces exactly one
/// entry as it completes, avoiding per-tick spam.
pub fn generate_narrative(
    query: Query<(&Name, &CurrentAction, &Needs)>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
    let tick = time.tick;

    for (name, current, needs) in &query {
        if current.ticks_remaining != 1 {
            continue;
        }

        let cat = &name.0;

        match current.action {
            Action::Eat => {
                let options = [
                    format!("{cat} eats from the stores."),
                    format!("{cat} has a quick meal."),
                    format!("{cat} chews thoughtfully."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                log.push(tick, options[idx].clone(), NarrativeTier::Action);
            }

            Action::Sleep => {
                let options = [
                    format!("{cat} curls up and sleeps."),
                    format!("{cat} naps in a quiet corner."),
                    format!("{cat} dozes off."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                log.push(tick, options[idx].clone(), NarrativeTier::Action);
            }

            Action::Wander => {
                let options = [
                    format!("{cat} wanders about."),
                    format!("{cat} explores nearby."),
                    format!("{cat} stretches and strolls."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                log.push(tick, options[idx].clone(), NarrativeTier::Action);
            }

            Action::Idle => {
                // Rate-limit: narrate only 1 in 5 idle completions.
                let roll: u32 = rng.rng.random_range(0..5);
                if roll != 0 {
                    continue;
                }

                let text = if needs.hunger < 0.3 {
                    format!("{cat}'s stomach growls.")
                } else if needs.energy < 0.3 {
                    format!("{cat} yawns widely.")
                } else {
                    let options = [
                        format!("{cat} sits quietly."),
                        format!("{cat} grooms a paw."),
                        format!("{cat} watches the sky."),
                    ];
                    let idx = rng.rng.random_range(0..options.len());
                    options[idx].clone()
                };

                log.push(tick, text, NarrativeTier::Micro);
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

    use crate::resources::narrative::NarrativeLog;
    use crate::resources::rng::SimRng;
    use crate::resources::time::TimeState;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimRng::new(42));
        let mut schedule = Schedule::default();
        schedule.add_systems(generate_narrative);
        (world, schedule)
    }

    /// An action with ticks_remaining == 1 should produce a log entry.
    #[test]
    fn narrates_on_last_tick() {
        let (mut world, mut schedule) = setup_world();

        world.spawn((
            Name("Mochi".to_string()),
            CurrentAction {
                action: Action::Eat,
                ticks_remaining: 1,
                target_position: None,
            },
            Needs::default(),
        ));

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        assert_eq!(log.entries.len(), 1, "should have one entry");
        assert!(
            log.entries[0].text.contains("Mochi"),
            "entry should mention the cat's name"
        );
        assert_eq!(log.entries[0].tier, NarrativeTier::Action);
    }

    /// An action with ticks_remaining != 1 should not produce a log entry.
    #[test]
    fn does_not_narrate_mid_action() {
        let (mut world, mut schedule) = setup_world();

        world.spawn((
            Name("Pepper".to_string()),
            CurrentAction {
                action: Action::Sleep,
                ticks_remaining: 10,
                target_position: None,
            },
            Needs::default(),
        ));

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        assert!(log.entries.is_empty(), "should not narrate mid-action");
    }

    /// Idle narration uses Micro tier (not Action).
    #[test]
    fn idle_uses_micro_tier() {
        // Run enough times so the 1-in-5 rate limit fires at least once.
        // With seed 42 this converges quickly; 20 runs is safe.
        let (mut world, mut schedule) = setup_world();

        world.spawn((
            Name("Dusk".to_string()),
            CurrentAction {
                action: Action::Idle,
                ticks_remaining: 1,
                target_position: None,
            },
            Needs::default(),
        ));

        // Run 20 ticks to give rate-limit at least one chance to pass.
        for _ in 0..20 {
            schedule.run(&mut world);
        }

        let log = world.resource::<NarrativeLog>();
        let idle_entries: Vec<_> = log
            .entries
            .iter()
            .filter(|e| e.tier == NarrativeTier::Micro)
            .collect();

        assert!(
            !idle_entries.is_empty(),
            "at least one Micro-tier idle entry should appear in 20 ticks"
        );
    }

    /// Hungry cat idle text mentions the stomach.
    #[test]
    fn hungry_idle_mentions_stomach() {
        let (mut world, mut schedule) = setup_world();

        // Override SimRng so roll == 0 every time (force narration).
        // Seed 0 with random_range(0..5) → check a few outcomes. Instead,
        // we spawn multiple cats to increase the chance one fires.
        let mut needs = Needs::default();
        needs.hunger = 0.1; // below 0.3 threshold

        for _ in 0..10 {
            world.spawn((
                Name("Pip".to_string()),
                CurrentAction {
                    action: Action::Idle,
                    ticks_remaining: 1,
                    target_position: None,
                },
                needs.clone(),
            ));
        }

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        let stomach_entries: Vec<_> = log
            .entries
            .iter()
            .filter(|e| e.text.contains("stomach"))
            .collect();

        // With 10 cats and 1-in-5 rate, ~2 should fire → at least one stomach line.
        assert!(
            !stomach_entries.is_empty(),
            "at least one 'stomach growls' entry expected for hungry cats"
        );
    }
}
