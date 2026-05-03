use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::identity::{Age, Name};
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier, MoodSource};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, DeathCause, Health, Needs, Position};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
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
    mut mood_query: Query<(Entity, &Position, &mut Mood, &mut Memory, &Personality), Without<Dead>>,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    love_query: Query<(Entity, &Name, &crate::components::fate::FatedLove), Without<Dead>>,
    rival_query: Query<(Entity, &Name, &crate::components::fate::FatedRival), Without<Dead>>,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    relationships: Res<Relationships>,
) {
    let c = &constants.death;
    let needs_c = &constants.needs;
    let tick = time.tick;
    let mut newly_dead: Vec<(Entity, Position, String, DeathCause, Option<String>)> = Vec::new();

    for (entity, name, health, needs, age, pos) in &alive_query {
        let cause = if health.current <= 0.0 {
            // Ticket 032 — discriminator branches on cliff mode. Legacy:
            // hard `hunger == 0.0 ⇒ Starvation`. Graded: the cat may bottom
            // out at `hunger > 0`, so attribute to Starvation when the
            // monotonic `total_starvation_damage` accumulator crossed the
            // attribution threshold. Default config keeps legacy semantics.
            if needs_c.starvation_cliff_use_legacy {
                if needs.hunger == 0.0 {
                    Some(DeathCause::Starvation)
                } else {
                    Some(DeathCause::Injury)
                }
            } else if health.total_starvation_damage > needs_c.starvation_attribution_threshold {
                Some(DeathCause::Starvation)
            } else {
                Some(DeathCause::Injury)
            }
        } else {
            // Old age check for elders.
            let stage = age.stage(tick, config.ticks_per_season);
            if stage == crate::components::identity::LifeStage::Elder {
                let age_ticks = tick.saturating_sub(age.born_tick);
                let elder_entry = c.elder_entry_seasons * config.ticks_per_season;
                let grace = c.grace_seasons * config.ticks_per_season;
                if age_ticks > elder_entry + grace {
                    let excess_seasons =
                        ((age_ticks - elder_entry - grace) / config.ticks_per_season) as f64;
                    let chance = excess_seasons * c.chance_per_excess_season;
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
            let injury_source = if cause == DeathCause::Injury {
                health
                    .injuries
                    .iter()
                    .filter(|inj| !inj.healed)
                    .max_by_key(|inj| inj.tick_received)
                    .map(|inj| format!("{:?}", inj.source))
            } else {
                None
            };
            newly_dead.push((entity, *pos, name.0.clone(), cause, injury_source));

            match cause {
                DeathCause::Starvation => activation.record(Feature::DeathStarvation),
                DeathCause::OldAge => activation.record(Feature::DeathOldAge),
                DeathCause::Injury => activation.record(Feature::DeathInjury),
            }
            if let Some(ref mut score) = colony_score {
                match cause {
                    DeathCause::Starvation => score.deaths_starvation += 1,
                    DeathCause::OldAge => score.deaths_old_age += 1,
                    DeathCause::Injury => score.deaths_injury += 1,
                }
            }
        }
    }

    let mut event_log = event_log;

    // Process reactions to each death.
    for (dead_entity, dead_pos, dead_name, cause, injury_source) in &newly_dead {
        // Tier 3 narrative.
        let text = match cause {
            DeathCause::Starvation => format!("{dead_name} has starved."),
            DeathCause::OldAge => format!("{dead_name} does not wake."),
            DeathCause::Injury => format!("{dead_name} has died from wounds."),
        };
        log.push(tick, text.clone(), NarrativeTier::Danger);

        if let Some(ref mut elog) = event_log {
            elog.push(
                tick,
                crate::resources::event_log::EventKind::Death {
                    cat: dead_name.clone(),
                    cause: format!("{cause:?}"),
                    injury_source: injury_source.clone(),
                    location: (dead_pos.x, dead_pos.y),
                },
            );
        }

        // Nearby living cats react. Phase 4 migration: visual-channel
        // check via the unified sensory model.
        for (entity, pos, mut mood, mut memory, personality) in &mut mood_query {
            // Skip the dying cat itself — Dead component not yet applied (deferred commands).
            if entity == *dead_entity {
                continue;
            }

            if crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                &constants.sensory.cat,
                *dead_pos,
                crate::components::SensorySignature::CAT,
                c.grief_detection_range as f32,
            ) {
                mood.modifiers.push_back(
                    MoodModifier::new(c.grief_mood_penalty, c.grief_mood_ticks, format!("{dead_name} died"))
                        .with_kind(MoodSource::Grief),
                );

                memory.remember(MemoryEntry {
                    event_type: MemoryType::Death,
                    location: Some(*dead_pos),
                    involved: vec![*dead_entity],
                    tick,
                    strength: c.grief_memory_strength,
                    firsthand: true,
                });
            }

            // Bond grief: cats with a named bond receive a lasting grief modifier
            // regardless of proximity. Intensity and duration scale by bond type.
            let Some(rel) = relationships.get(*dead_entity, entity) else {
                continue;
            };
            let Some(bond) = rel.bond else {
                continue;
            };
            let (intensity, duration) = match bond {
                BondType::Mates => (c.bereavement_mates_intensity, c.bereavement_mates_ticks),
                BondType::Partners => (c.bereavement_partners_intensity, c.bereavement_partners_ticks),
                BondType::Friends => (c.bereavement_friends_intensity, c.bereavement_friends_ticks),
            };
            let grief_amount = -(intensity * rel.fondness.max(0.0));
            if grief_amount > -0.05 {
                continue;
            }
            let mut modifier = MoodModifier::new(
                grief_amount,
                duration,
                format!("{dead_name} (bonded)"),
            )
            .with_kind(MoodSource::Grief);
            crate::systems::mood::patience_extend(&mut modifier, personality.patience, &constants.mood);
            mood.modifiers.push_back(modifier);
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
                    NarrativeTier::Danger,
                );
                // Remove the component — fate doesn't repeat.
                commands
                    .entity(survivor_e)
                    .remove::<crate::components::fate::FatedLove>();
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
                    NarrativeTier::Danger,
                );
                commands
                    .entity(survivor_e)
                    .remove::<crate::components::fate::FatedRival>();
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
    constants: Res<SimConstants>,
) {
    let grace_period = constants.death.cleanup_grace_period;

    for (entity, dead) in &query {
        if time.tick.saturating_sub(dead.tick) >= grace_period {
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

    use crate::ai::CurrentAction;
    use crate::components::identity::{Age, Appearance, Gender, Name, Orientation, Species};
    use crate::components::mental::{Memory, Mood};
    use crate::components::personality::Personality;
    use crate::components::physical::{Health, Needs, Position};
    use crate::components::skills::{Corruption, MagicAffinity, Skills, Training};
    use crate::resources::time::{SimConfig, TimeState};

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 100,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(Relationships::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(check_death);
        (world, schedule)
    }

    fn test_personality() -> Personality {
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;
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
                    Health {
                        current: health,
                        max: 1.0,
                        injuries: vec![],
                        total_starvation_damage: 0.0,
                    },
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
        let has_death_entry = log
            .entries
            .iter()
            .any(|e| e.text.contains("Ash") && e.tier == NarrativeTier::Danger);
        assert!(
            has_death_entry,
            "death should produce a Danger narrative entry"
        );
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
        let has_death_memory = memory
            .events
            .iter()
            .any(|e| e.event_type == MemoryType::Death);
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

    fn spawn_cat_at(world: &mut World, name: &str, health: f32, hunger: f32, x: i32, y: i32) -> Entity {
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
                    Position::new(x, y),
                    Health {
                        current: health,
                        max: 1.0,
                        injuries: vec![],
                        total_starvation_damage: 0.0,
                    },
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
    fn bonded_cat_gets_grief_modifier_on_partner_death() {
        use crate::resources::relationships::BondType;
        let (mut world, mut schedule) = setup_world();

        // Dying cat at (5,5).
        let dying = spawn_cat_at(&mut world, "Moss", 0.0, 0.0, 5, 5);
        // Survivor with a Mates bond — far away so proximity grief doesn't fire.
        let survivor = spawn_cat_at(&mut world, "Fern", 1.0, 0.8, 30, 30);

        // Create a Mates bond with positive fondness.
        {
            let mut rels = world.resource_mut::<Relationships>();
            let rel = rels.get_or_insert(dying, survivor);
            rel.fondness = 0.8;
            rel.bond = Some(BondType::Mates);
        }

        schedule.run(&mut world);

        let mood = world.get::<Mood>(survivor).unwrap();
        let bond_grief = mood
            .modifiers
            .iter()
            .find(|m| m.source.contains("(bonded)"));
        assert!(
            bond_grief.is_some(),
            "bonded survivor should receive a grief modifier; modifiers: {:?}",
            mood.modifiers
        );
        let modifier = bond_grief.unwrap();
        assert!(modifier.amount < 0.0, "bond grief should be negative");
        assert_eq!(modifier.kind, crate::components::mental::MoodSource::Grief);
        assert!(
            modifier.ticks_remaining > 0,
            "grief modifier should have positive duration"
        );
    }

    #[test]
    fn distant_unbonded_cat_gets_no_grief_on_death() {
        let (mut world, mut schedule) = setup_world();

        spawn_cat_at(&mut world, "Moss", 0.0, 0.0, 5, 5);
        let distant = spawn_cat_at(&mut world, "Fern", 1.0, 0.8, 30, 30);

        schedule.run(&mut world);

        let mood = world.get::<Mood>(distant).unwrap();
        assert!(
            mood.modifiers.is_empty(),
            "distant unbonded cat should have no grief; modifiers: {:?}",
            mood.modifiers
        );
    }

    #[test]
    fn cleanup_despawns_after_grace() {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 700,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        world.insert_resource(crate::resources::SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_dead);

        let entity = world
            .spawn(Dead {
                tick: 100,
                cause: DeathCause::Starvation,
            })
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
        world.insert_resource(TimeState {
            tick: 400,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        world.insert_resource(crate::resources::SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(cleanup_dead);

        let entity = world
            .spawn(Dead {
                tick: 100,
                cause: DeathCause::OldAge,
            })
            .id();

        schedule.run(&mut world);

        assert!(
            world.get_entity(entity).is_ok(),
            "recently dead entity should still exist during grace period"
        );
    }
}
