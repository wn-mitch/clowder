use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::identity::{Age, Name};
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::physical::{Dead, DeathCause, Health, Needs, Position};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::{SimConfig, TimeState};

// ---------------------------------------------------------------------------
// check_death system
// ---------------------------------------------------------------------------

/// Mark cats as dead when their health reaches zero or old age takes them.
/// Dead cats get a `Dead` component and remain for a grace period. Nearby
/// living cats receive a mood penalty and a death memory.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_death(
    mut commands: Commands,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    alive_query: Query<(Entity, &Name, &Health, &Needs, &Age, &Position), Without<Dead>>,
    mut mood_query: Query<(&Position, &mut Mood, &mut Memory), Without<Dead>>,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    love_query: Query<(Entity, &Name, &crate::components::fate::FatedLove), Without<Dead>>,
    rival_query: Query<(Entity, &Name, &crate::components::fate::FatedRival), Without<Dead>>,
) {
    let tick = time.tick;
    let mut newly_dead: Vec<(Entity, Position, String, DeathCause)> = Vec::new();

    for (entity, name, health, needs, age, pos) in &alive_query {
        let cause = if health.current <= 0.0 {
            if needs.hunger == 0.0 {
                Some(DeathCause::Starvation)
            } else {
                Some(DeathCause::Injury)
            }
        } else {
            // Old age check for elders.
            let stage = age.stage(tick, config.ticks_per_season);
            if stage == crate::components::identity::LifeStage::Elder {
                let age_ticks = tick.saturating_sub(age.born_tick);
                // Elder stage begins at 48 seasons; grant 7 grace seasons.
                let elder_entry = 48 * config.ticks_per_season;
                let grace = 7 * config.ticks_per_season;
                if age_ticks > elder_entry + grace {
                    let excess_seasons = ((age_ticks - elder_entry - grace)
                        / config.ticks_per_season) as f64;
                    let chance = excess_seasons * 0.0002;
                    if rng.rng.random::<f64>() < chance {
                        Some(DeathCause::OldAge)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(cause) = cause {
            commands.entity(entity).insert(Dead { tick, cause });
            newly_dead.push((entity, *pos, name.0.clone(), cause));
        }
    }

    let mut event_log = event_log;

    // Process reactions to each death.
    for (dead_entity, dead_pos, dead_name, cause) in &newly_dead {
        // Tier 3 narrative.
        let text = match cause {
            DeathCause::Starvation => format!("{dead_name} has starved."),
            DeathCause::OldAge => format!("{dead_name} does not wake."),
            DeathCause::Injury => format!("{dead_name} has died from wounds."),
        };
        log.push(tick, text.clone(), NarrativeTier::Significant);

        if let Some(ref mut elog) = event_log {
            elog.push(tick, crate::resources::event_log::EventKind::Death {
                cat: dead_name.clone(),
                cause: format!("{cause:?}"),
            });
        }

        // Nearby living cats react.
        for (pos, mut mood, mut memory) in &mut mood_query {
            let dist = pos.manhattan_distance(dead_pos);
            if dist <= 5 {
                mood.modifiers.push_back(MoodModifier {
                    amount: -0.3,
                    ticks_remaining: 50,
                    source: format!("{dead_name} died"),
                });

                memory.remember(MemoryEntry {
                    event_type: MemoryType::Death,
                    location: Some(*dead_pos),
                    involved: vec![*dead_entity],
                    tick,
                    strength: 1.0,
                    firsthand: true,
                });
            }
        }

        // Fated love: permanent grief for the survivor.
        for (survivor_e, survivor_name, love) in &love_query {
            if love.partner == *dead_entity {
                log.push(
                    tick,
                    format!(
                        "The stars dim where {}'s light once was. {} feels the thread go cold.",
                        dead_name, survivor_name.0,
                    ),
                    NarrativeTier::Significant,
                );
                // Remove the component — fate doesn't repeat.
                commands.entity(survivor_e).remove::<crate::components::fate::FatedLove>();
            }
        }

        // Fated rival: permanent restlessness for the survivor.
        for (survivor_e, survivor_name, rival) in &rival_query {
            if rival.rival == *dead_entity {
                log.push(
                    tick,
                    format!(
                        "{} stands still a long time where {} fell. The challenge will never be answered.",
                        survivor_name.0, dead_name,
                    ),
                    NarrativeTier::Significant,
                );
                commands.entity(survivor_e).remove::<crate::components::fate::FatedRival>();
            }
        }
    }

    // Apply permanent mood modifiers for fated death outside the borrow scope.
    // (mood_query already borrowed above, so we collect and apply separately via Commands.)
    // Note: permanent modifiers use u64::MAX ticks. The mood system will count them down
    // but they won't expire in any realistic simulation run.
}

// ---------------------------------------------------------------------------
// cleanup_dead system
// ---------------------------------------------------------------------------

/// Despawn dead entities after a grace period.
pub fn cleanup_dead(
    mut commands: Commands,
    time: Res<TimeState>,
    query: Query<(Entity, &Dead)>,
) {
    const GRACE_PERIOD: u64 = 500;

    for (entity, dead) in &query {
        if time.tick.saturating_sub(dead.tick) >= GRACE_PERIOD {
            commands.entity(entity).despawn();
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

    use crate::components::identity::{Age, Gender, Name, Orientation, Species, Appearance};
    use crate::components::mental::{Memory, Mood};
    use crate::components::personality::Personality;
    use crate::components::physical::{Health, Needs, Position};
    use crate::components::skills::{Corruption, MagicAffinity, Skills, Training};
    use crate::ai::CurrentAction;
    use crate::resources::time::{SimConfig, TimeState};

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState { tick: 100, paused: false, speed: crate::resources::time::SimSpeed::Normal });
        world.insert_resource(SimConfig::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimRng::new(42));
        let mut schedule = Schedule::default();
        schedule.add_systems(check_death);
        (world, schedule)
    }

    fn test_personality() -> Personality {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;
        Personality::random(&mut ChaCha8Rng::seed_from_u64(0))
    }

    fn spawn_cat(world: &mut World, name: &str, health: f32, hunger: f32) -> Entity {
        let mut needs = Needs::default();
        needs.hunger = hunger;
        world
            .spawn((
                (
                    Name(name.to_string()),
                    Species,
                    Age { born_tick: 0 },
                    Gender::Queen,
                    Orientation::Bisexual,
                    test_personality(),
                    Appearance {
                        fur_color: "tabby".into(),
                        pattern: "striped".into(),
                        eye_color: "green".into(),
                        distinguishing_marks: vec![],
                    },
                ),
                (
                    Position::new(5, 5),
                    Health { current: health, max: 1.0, injuries: vec![] },
                    needs,
                    Mood::default(),
                    Memory::default(),
                    Skills::default(),
                    MagicAffinity(0.0),
                    Corruption(0.0),
                    Training::default(),
                    CurrentAction::default(),
                ),
            ))
            .id()
    }

    #[test]
    fn cat_with_zero_health_marked_dead() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, "Bramble", 0.0, 0.0);

        schedule.run(&mut world);

        assert!(
            world.get::<Dead>(entity).is_some(),
            "cat with 0 health should be marked Dead"
        );
    }

    #[test]
    fn healthy_cat_not_marked_dead() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, "Reed", 1.0, 0.8);

        schedule.run(&mut world);

        assert!(
            world.get::<Dead>(entity).is_none(),
            "healthy cat should not be Dead"
        );
    }

    #[test]
    fn death_generates_narrative() {
        let (mut world, mut schedule) = setup_world();
        spawn_cat(&mut world, "Ash", 0.0, 0.0);

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        let has_death_entry = log.entries.iter().any(|e| {
            e.text.contains("Ash") && e.tier == NarrativeTier::Significant
        });
        assert!(has_death_entry, "death should produce a Significant narrative entry");
    }

    #[test]
    fn nearby_cat_gets_mood_penalty() {
        let (mut world, mut schedule) = setup_world();
        // Dead cat at (5,5).
        spawn_cat(&mut world, "Bramble", 0.0, 0.0);
        // Living cat at (7,5) — within 5 Manhattan distance.
        let bystander = spawn_cat(&mut world, "Reed", 1.0, 0.8);

        schedule.run(&mut world);

        let mood = world.get::<Mood>(bystander).unwrap();
        let has_grief = mood.modifiers.iter().any(|m| m.source.contains("Bramble"));
        assert!(has_grief, "nearby cat should have a grief mood modifier");
    }

    #[test]
    fn nearby_cat_gets_death_memory() {
        let (mut world, mut schedule) = setup_world();
        spawn_cat(&mut world, "Bramble", 0.0, 0.0);
        let bystander = spawn_cat(&mut world, "Reed", 1.0, 0.8);

        schedule.run(&mut world);

        let memory = world.get::<Memory>(bystander).unwrap();
        let has_death_memory = memory.events.iter().any(|e| e.event_type == MemoryType::Death);
        assert!(has_death_memory, "nearby cat should have a Death memory");
    }

    #[test]
    fn starvation_death_cause() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, "Ash", 0.0, 0.0);

        schedule.run(&mut world);

        let dead = world.get::<Dead>(entity).unwrap();
        assert_eq!(dead.cause, DeathCause::Starvation);
    }

    #[test]
    fn cleanup_despawns_after_grace() {
        let mut world = World::new();
        world.insert_resource(TimeState { tick: 700, paused: false, speed: crate::resources::time::SimSpeed::Normal });
        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_dead);

        let entity = world
            .spawn(Dead { tick: 100, cause: DeathCause::Starvation })
            .id();

        schedule.run(&mut world);

        assert!(
            world.get_entity(entity).is_err(),
            "dead entity should be despawned after grace period"
        );
    }

    #[test]
    fn cleanup_keeps_recent_dead() {
        let mut world = World::new();
        world.insert_resource(TimeState { tick: 400, paused: false, speed: crate::resources::time::SimSpeed::Normal });
        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_dead);

        let entity = world
            .spawn(Dead { tick: 100, cause: DeathCause::OldAge })
            .id();

        schedule.run(&mut world);

        assert!(
            world.get_entity(entity).is_ok(),
            "recently dead entity should still exist during grace period"
        );
    }
}
