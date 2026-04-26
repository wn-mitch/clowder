use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::CurrentAction;
use crate::components::fate::{FateAssigned, FatedLove, FatedRival};
use crate::components::identity::{Age, LifeStage, Name};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Position};
use crate::components::zodiac::ZodiacSign;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeScale, TimeState};
use crate::resources::zodiac::ZodiacData;

// ---------------------------------------------------------------------------
// assign_fated_connections
// ---------------------------------------------------------------------------

/// Assigns fated love and fated rival connections to cats reaching Young stage.
///
/// Runs every tick but only processes cats that lack the `FateAssigned` marker.
/// Throttled to one fate event per in-game day so narrative beats trickle in
/// rather than arriving as a burst at game start (cadence governed by
/// `FateConstants::assign_cooldown`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn assign_fated_connections(
    query: Query<
        (Entity, &Name, &Age, &Personality, &Position, &ZodiacSign),
        (Without<FateAssigned>, Without<Dead>),
    >,
    all_cats: Query<(Entity, &Name, &Personality, &ZodiacSign), (Without<Dead>,)>,
    existing_loves: Query<Entity, With<FatedLove>>,
    existing_rivals: Query<Entity, With<FatedRival>>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    zodiac_data: Option<Res<ZodiacData>>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
    mut last_assign_tick: Local<u64>,
    mut activation: ResMut<SystemActivation>,
) {
    let Some(zodiac_data) = zodiac_data else {
        return;
    };
    let c = &constants.fate;

    // Throttle: minimum-gap of one in-game day between assignments.
    if time.tick < *last_assign_tick + c.assign_cooldown.ticks(&time_scale) {
        return;
    }

    // Collect entities that already have fated connections to avoid double-assignment.
    let has_love: std::collections::HashSet<Entity> = existing_loves.iter().collect();
    let has_rival: std::collections::HashSet<Entity> = existing_rivals.iter().collect();

    // Snapshot candidates (all living cats) for scoring.
    let candidates: Vec<(Entity, &str, &Personality, ZodiacSign)> = all_cats
        .iter()
        .map(|(e, n, p, z)| (e, n.0.as_str(), p, *z))
        .collect();

    // Track entities assigned in this tick to avoid double-assigning within the batch.
    let mut love_assigned_this_tick: std::collections::HashSet<Entity> =
        std::collections::HashSet::new();
    let mut rival_assigned_this_tick: std::collections::HashSet<Entity> =
        std::collections::HashSet::new();

    for (entity, name, age, personality, _pos, &sign) in &query {
        let stage = age.stage(time.tick, config.ticks_per_season);
        if stage == LifeStage::Kitten {
            continue;
        }

        // Mark as processed regardless of whether we find a match.
        commands.entity(entity).insert(FateAssigned);
        // Record this tick and process only one cat per invocation.
        *last_assign_tick = time.tick;

        // --- Fated love ---
        if !has_love.contains(&entity) && !love_assigned_this_tick.contains(&entity) {
            let mut best: Option<(Entity, &str, ZodiacSign, f32)> = None;
            for &(cand_e, cand_name, cand_pers, cand_sign) in &candidates {
                if cand_e == entity {
                    continue;
                }
                if has_love.contains(&cand_e) || love_assigned_this_tick.contains(&cand_e) {
                    continue;
                }

                let compat = zodiac_data.compatibility(sign, cand_sign);
                let zodiac_score = if compat > 0.0 {
                    c.love_zodiac_score
                } else {
                    0.0
                };

                // Warmth/sociability alignment: bonus if both high or both low.
                let warmth_align = 1.0 - (personality.warmth - cand_pers.warmth).abs();
                let social_align = 1.0 - (personality.sociability - cand_pers.sociability).abs();
                let personality_score =
                    c.love_personality_weight * ((warmth_align + social_align) / 2.0);

                let jitter = rng.rng.random_range(-c.love_jitter..c.love_jitter);
                let total = zodiac_score + personality_score + jitter;

                if best.as_ref().is_none_or(|(_, _, _, s)| total > *s) {
                    best = Some((cand_e, cand_name, cand_sign, total));
                }
            }

            if let Some((partner_e, partner_name, partner_sign, _score)) = best {
                activation.record(Feature::FateAssigned);
                love_assigned_this_tick.insert(entity);
                love_assigned_this_tick.insert(partner_e);

                commands.entity(entity).insert(FatedLove {
                    partner: partner_e,
                    awakened: false,
                    assigned_tick: time.tick,
                });
                commands.entity(partner_e).insert(FatedLove {
                    partner: entity,
                    awakened: false,
                    assigned_tick: time.tick,
                });

                log.push(
                    time.tick,
                    format!(
                        "The stars mark {} and {} -- {} and {}, drawn together by a thread older than memory.",
                        name.0, partner_name, sign.label(), partner_sign.label(),
                    ),
                    NarrativeTier::Significant,
                );
            }
        }

        // --- Fated rival ---
        if !has_rival.contains(&entity) && !rival_assigned_this_tick.contains(&entity) {
            let mut best: Option<(Entity, &str, ZodiacSign, f32)> = None;
            for &(cand_e, cand_name, cand_pers, cand_sign) in &candidates {
                if cand_e == entity {
                    continue;
                }
                if has_rival.contains(&cand_e) || rival_assigned_this_tick.contains(&cand_e) {
                    continue;
                }

                let compat = zodiac_data.compatibility(sign, cand_sign);
                let zodiac_score = if compat < 0.0 {
                    c.rival_zodiac_score
                } else {
                    0.0
                };

                // Opposing ambition/pride: bonus if axes differ significantly.
                let ambition_diff = (personality.ambition - cand_pers.ambition).abs();
                let pride_diff = (personality.pride - cand_pers.pride).abs();
                let personality_score =
                    c.rival_personality_weight * ((ambition_diff + pride_diff) / 2.0);

                let jitter = rng.rng.random_range(-c.rival_jitter..c.rival_jitter);
                let total = zodiac_score + personality_score + jitter;

                if best.as_ref().is_none_or(|(_, _, _, s)| total > *s) {
                    best = Some((cand_e, cand_name, cand_sign, total));
                }
            }

            if let Some((rival_e, rival_name, rival_sign, _score)) = best {
                activation.record(Feature::FateAssigned);
                rival_assigned_this_tick.insert(entity);
                rival_assigned_this_tick.insert(rival_e);

                commands.entity(entity).insert(FatedRival {
                    rival: rival_e,
                    awakened: false,
                    assigned_tick: time.tick,
                });
                commands.entity(rival_e).insert(FatedRival {
                    rival: entity,
                    awakened: false,
                    assigned_tick: time.tick,
                });

                log.push(
                    time.tick,
                    format!(
                        "{} and {} lock eyes across the clearing -- {} and {}, bound by a challenge written in the stars.",
                        name.0, rival_name, sign.label(), rival_sign.label(),
                    ),
                    NarrativeTier::Significant,
                );
            }
        }

        // Only process one cat per invocation to stagger narrative messages.
        break;
    }
}

// ---------------------------------------------------------------------------
// awaken_fated_connections
// ---------------------------------------------------------------------------

/// Checks whether dormant fated connections should awaken.
///
/// - Fated love awakens when both cats are within `love_awaken_distance` tiles.
/// - Fated rival awakens when both cats perform the same action type within
///   `rival_awaken_distance` tiles.
///
/// Pairs are collected and deduplicated so that each awakening produces exactly
/// one narrative line, not two (one per cat in the pair).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn awaken_fated_connections(
    mut love_query: Query<(Entity, &Name, &Position, &mut FatedLove), Without<Dead>>,
    mut rival_query: Query<
        (Entity, &Name, &Position, &CurrentAction, &mut FatedRival),
        Without<Dead>,
    >,
    positions: Query<&Position, Without<Dead>>,
    actions: Query<&CurrentAction, Without<Dead>>,
    names: Query<&Name>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.fate;

    // --- Fated loves: collect pairs, deduplicate, then apply. -----------------
    let mut love_pairs: Vec<(Entity, Entity)> = Vec::new();
    for (entity, _name, pos, love) in &love_query {
        if love.awakened {
            continue;
        }
        let Ok(partner_pos) = positions.get(love.partner) else {
            continue;
        };
        if pos.manhattan_distance(partner_pos) <= c.love_awaken_distance {
            let pair = if entity < love.partner {
                (entity, love.partner)
            } else {
                (love.partner, entity)
            };
            love_pairs.push(pair);
        }
    }
    love_pairs.sort();
    love_pairs.dedup();
    love_pairs.truncate(1); // At most one awakening per tick.

    for (a, b) in &love_pairs {
        activation.record(Feature::FateAwakened);
        if let Ok((_, _, _, mut love_a)) = love_query.get_mut(*a) {
            love_a.awakened = true;
        }
        if let Ok((_, _, _, mut love_b)) = love_query.get_mut(*b) {
            love_b.awakened = true;
        }
        let name_a = names.get(*a).map_or("one", |n| n.0.as_str());
        let name_b = names.get(*b).map_or("another", |n| n.0.as_str());
        log.push(
            time.tick,
            format!(
                "Something stirs between {} and {} as their paths cross.",
                name_a, name_b
            ),
            NarrativeTier::Significant,
        );
    }

    // --- Fated rivals: collect pairs, deduplicate, then apply. ----------------
    let mut rival_pairs: Vec<(Entity, Entity)> = Vec::new();
    for (entity, _name, pos, current, rival) in &rival_query {
        if rival.awakened {
            continue;
        }
        let Ok(rival_pos) = positions.get(rival.rival) else {
            continue;
        };
        if pos.manhattan_distance(rival_pos) > c.rival_awaken_distance {
            continue;
        }
        let Ok(rival_action) = actions.get(rival.rival) else {
            continue;
        };
        if current.action == rival_action.action && current.ticks_remaining > 0 {
            let pair = if entity < rival.rival {
                (entity, rival.rival)
            } else {
                (rival.rival, entity)
            };
            rival_pairs.push(pair);
        }
    }
    rival_pairs.sort();
    rival_pairs.dedup();
    rival_pairs.truncate(1); // At most one awakening per tick.

    for (a, b) in &rival_pairs {
        activation.record(Feature::FateAwakened);
        if let Ok((_, _, _, _, mut rival_a)) = rival_query.get_mut(*a) {
            rival_a.awakened = true;
        }
        if let Ok((_, _, _, _, mut rival_b)) = rival_query.get_mut(*b) {
            rival_b.awakened = true;
        }
        let name_a = names.get(*a).map_or("one", |n| n.0.as_str());
        let name_b = names.get(*b).map_or("another", |n| n.0.as_str());
        log.push(
            time.tick,
            format!(
                "{} and {} lock eyes across the clearing. A challenge unspoken.",
                name_a, name_b
            ),
            NarrativeTier::Significant,
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::CurrentAction;
    use crate::components::identity::Age;
    use crate::components::identity::{Gender, Orientation, Species};
    use crate::components::magic::Inventory;
    use crate::components::mental::{Memory, Mood};
    use crate::components::physical::{Health, Needs};
    use crate::components::skills::{Corruption, MagicAffinity, Skills, Training};
    use crate::resources::relationships::Relationships;
    use crate::resources::sim_constants::SimConstants;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn test_rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(42)
    }

    fn test_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 100_000,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        let config = SimConfig {
            ticks_per_season: 2000,
            ..SimConfig::default()
        };
        world.insert_resource(TimeScale::from_config(&config, 16.6667));
        world.insert_resource(config);
        world.insert_resource(SimConstants::default());
        world.insert_resource(SimRng::new(42));
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(Relationships::default());
        world.insert_resource(SystemActivation::default());
        let zodiac_data = ZodiacData::load(std::path::Path::new("assets/data/zodiac.ron")).unwrap();
        world.insert_resource(zodiac_data);

        let mut schedule = Schedule::default();
        schedule.add_systems(assign_fated_connections);
        (world, schedule)
    }

    /// Tick gap matching `FateConstants::assign_cooldown` at default
    /// scale (1000 ticks/day = once per in-game day).
    fn assign_cooldown_ticks() -> u64 {
        let config = SimConfig {
            ticks_per_season: 2000,
            ..SimConfig::default()
        };
        let ts = TimeScale::from_config(&config, 16.6667);
        SimConstants::default().fate.assign_cooldown.ticks(&ts)
    }

    fn spawn_cat(world: &mut World, name: &str, sign: ZodiacSign, born_tick: u64) -> Entity {
        let mut rng = test_rng();
        world
            .spawn((
                (
                    Name(name.to_string()),
                    Species,
                    Age { born_tick },
                    Gender::Tom,
                    Orientation::Straight,
                    Personality::random(&mut rng),
                    crate::components::identity::Appearance {
                        fur_color: "grey".to_string(),
                        pattern: "solid".to_string(),
                        eye_color: "amber".to_string(),
                        distinguishing_marks: vec![],
                    },
                    Position::new(5, 5),
                    Health::default(),
                    Needs::default(),
                    Mood::default(),
                    Memory::default(),
                ),
                (
                    sign,
                    Skills::default(),
                    MagicAffinity(0.1),
                    Corruption(0.0),
                    Training::default(),
                    CurrentAction::default(),
                    Inventory::default(),
                ),
            ))
            .id()
    }

    /// Advance tick by the given amount to surpass the cooldown.
    fn advance_tick(world: &mut World, delta: u64) {
        let mut time = world.resource_mut::<TimeState>();
        time.tick += delta;
    }

    #[test]
    fn assigns_fated_love_to_young_cats() {
        let (mut world, mut schedule) = test_world();

        // Two Young cats with compatible signs.
        let _a = spawn_cat(&mut world, "Bramble", ZodiacSign::LeapingFlame, 90_000);
        let _b = spawn_cat(&mut world, "Fern", ZodiacSign::StormFur, 90_000);

        // First run assigns one cat; second run (after cooldown) assigns the other.
        schedule.run(&mut world);
        advance_tick(&mut world, assign_cooldown_ticks());
        schedule.run(&mut world);

        let love_count = world.query::<&FatedLove>().iter(&world).count();
        assert_eq!(love_count, 2, "both cats should have FatedLove");

        let assigned_count = world.query::<&FateAssigned>().iter(&world).count();
        assert_eq!(assigned_count, 2);
    }

    #[test]
    fn kittens_are_not_assigned_fate() {
        let (mut world, mut schedule) = test_world();

        // Kitten (born 1 season ago at tick 100_000, ticks_per_season=2000 → born at 98_000).
        let _a = spawn_cat(&mut world, "Kit", ZodiacSign::LeapingFlame, 99_000);
        let _b = spawn_cat(&mut world, "Paw", ZodiacSign::StormFur, 99_000);

        schedule.run(&mut world);

        let assigned_count = world.query::<&FateAssigned>().iter(&world).count();
        assert_eq!(assigned_count, 0, "kittens should not get FateAssigned");
    }

    #[test]
    fn fated_love_is_mutual() {
        let (mut world, mut schedule) = test_world();
        let a = spawn_cat(&mut world, "Bramble", ZodiacSign::LeapingFlame, 90_000);
        let b = spawn_cat(&mut world, "Fern", ZodiacSign::StormFur, 90_000);

        schedule.run(&mut world);
        advance_tick(&mut world, assign_cooldown_ticks());
        schedule.run(&mut world);

        let love_a = world.get::<FatedLove>(a).expect("A should have love");
        let love_b = world.get::<FatedLove>(b).expect("B should have love");
        assert_eq!(love_a.partner, b);
        assert_eq!(love_b.partner, a);
    }

    #[test]
    fn fate_assignments_are_throttled() {
        let (mut world, mut schedule) = test_world();

        let _a = spawn_cat(&mut world, "Bramble", ZodiacSign::LeapingFlame, 90_000);
        let _b = spawn_cat(&mut world, "Fern", ZodiacSign::StormFur, 90_000);
        let _c = spawn_cat(&mut world, "Moss", ZodiacSign::WarmDen, 90_000);
        let _d = spawn_cat(&mut world, "Reed", ZodiacSign::LoneThorn, 90_000);

        // Single run should only assign one cat.
        schedule.run(&mut world);
        let assigned = world.query::<&FateAssigned>().iter(&world).count();
        assert_eq!(assigned, 1, "only one cat should be assigned per tick");

        // Running again without advancing tick should not assign more.
        schedule.run(&mut world);
        let assigned = world.query::<&FateAssigned>().iter(&world).count();
        assert_eq!(
            assigned, 1,
            "cooldown should prevent immediate re-assignment"
        );

        // After cooldown, one more should be assigned.
        advance_tick(&mut world, assign_cooldown_ticks());
        schedule.run(&mut world);
        let assigned = world.query::<&FateAssigned>().iter(&world).count();
        assert_eq!(assigned, 2, "second cat should be assigned after cooldown");
    }
}
