use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::step_toward;
use crate::ai::CurrentAction;
use crate::components::magic::{
    Harvestable, Herb, Inventory, MisfireEffect, RemedyEffect, RemedyKind, Seasonal, Ward,
    WardKind,
};
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::physical::{Dead, Health, InjuryKind, Injury, Needs, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::components::task_chain::{StepKind, StepStatus, TaskChain};
use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::time::{Season, SimConfig, TimeState};

// ---------------------------------------------------------------------------
// corruption_spread
// ---------------------------------------------------------------------------

/// Every 10 ticks, tiles with corruption > 0.3 bleed a fraction of their
/// corruption into the 4-adjacent neighbours.
pub fn corruption_spread(mut map: ResMut<TileMap>, time: Res<TimeState>) {
    if !time.tick.is_multiple_of(10) {
        return;
    }

    // Snapshot corrupted tiles so we don't double-count within one pass.
    let mut sources: Vec<(i32, i32, f32)> = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let c = map.get(x, y).corruption;
            if c > 0.3 {
                sources.push((x, y, c));
            }
        }
    }

    let deltas: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    for (sx, sy, corruption) in sources {
        let spread = corruption * 0.001;
        for (dx, dy) in &deltas {
            let nx = sx + dx;
            let ny = sy + dy;
            if map.in_bounds(nx, ny) {
                let tile = map.get_mut(nx, ny);
                tile.corruption = (tile.corruption + spread).min(1.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ward_decay
// ---------------------------------------------------------------------------

/// Each tick, every ward loses strength. Wards on WardPost tiles decay at half
/// speed. Wards that hit zero strength are despawned.
pub fn ward_decay(
    mut wards: Query<(Entity, &mut Ward, &Position)>,
    map: Res<TileMap>,
    mut commands: Commands,
) {
    for (entity, mut ward, pos) in &mut wards {
        let on_ward_post = map.in_bounds(pos.x, pos.y)
            && map.get(pos.x, pos.y).terrain == Terrain::WardPost;

        let effective_decay = if on_ward_post {
            ward.decay_rate * 0.5
        } else {
            ward.decay_rate
        };

        ward.strength -= effective_decay;

        if ward.strength <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// apply_remedy_effects
// ---------------------------------------------------------------------------

/// Tick active remedy buffs. Healing and energy tonics apply each tick; mood
/// tonic pushes a single modifier on the first tick only.
pub fn apply_remedy_effects(
    mut query: Query<(Entity, &mut RemedyEffect, &mut Health, &mut Needs, &mut Mood)>,
    mut commands: Commands,
) {
    for (entity, mut remedy, mut health, mut needs, mut mood) in &mut query {
        match remedy.kind {
            RemedyKind::HealingPoultice => {
                health.current = (health.current + 0.05).min(health.max);
            }
            RemedyKind::EnergyTonic => {
                needs.energy = (needs.energy + 0.03).min(1.0);
            }
            RemedyKind::MoodTonic => {
                // Only on the first tick of application.
                if remedy.ticks_remaining == remedy.kind.duration() {
                    mood.modifiers.push_back(MoodModifier {
                        amount: 0.2,
                        ticks_remaining: 50,
                        source: "herbal remedy".to_string(),
                    });
                }
            }
        }

        remedy.ticks_remaining = remedy.ticks_remaining.saturating_sub(1);
        if remedy.ticks_remaining == 0 {
            commands.entity(entity).remove::<RemedyEffect>();
        }
    }
}

// ---------------------------------------------------------------------------
// personal_corruption_effects
// ---------------------------------------------------------------------------

/// High personal corruption causes mood drops and erratic behaviour.
pub fn personal_corruption_effects(
    mut cats: Query<(&Corruption, &mut Mood, Option<&mut CurrentAction>)>,
    _relationships: Res<Relationships>,
    mut rng: ResMut<SimRng>,
) {
    // TODO: corruption > 0.5 should also decay fondness toward all known cats
    // in Relationships. This requires mutable access to Relationships plus
    // multi-entity iteration (needs to know *which* cats this cat knows),
    // which conflicts with the current borrow structure. Defer to a dedicated
    // system or event-based approach.

    for (corruption, mut mood, current_action) in &mut cats {
        if corruption.0 > 0.3
            && rng.rng.random::<f32>() < 0.05
        {
            mood.modifiers.push_back(MoodModifier {
                amount: -0.15,
                ticks_remaining: 10,
                source: "corruption".to_string(),
            });
        }

        if corruption.0 > 0.7
            && rng.rng.random::<f32>() < 0.02
        {
            if let Some(mut action) = current_action {
                action.ticks_remaining = 0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// corruption_tile_effects
// ---------------------------------------------------------------------------

/// Cats standing on corrupted ground receive a mood penalty. Herbs on heavily
/// corrupted tiles become twisted.
pub fn corruption_tile_effects(
    mut cats: Query<(&Position, &mut Mood), Without<Dead>>,
    map: Res<TileMap>,
    mut herbs: Query<(&Position, &mut Herb)>,
) {
    for (pos, mut mood) in &mut cats {
        if !map.in_bounds(pos.x, pos.y) {
            continue;
        }
        let corruption = map.get(pos.x, pos.y).corruption;
        if corruption > 0.1 {
            let already_has = mood
                .modifiers
                .iter()
                .any(|m| m.source == "corrupted ground");
            if !already_has {
                mood.modifiers.push_back(MoodModifier {
                    amount: -0.1 * corruption,
                    ticks_remaining: 5,
                    source: "corrupted ground".to_string(),
                });
            }
        }
    }

    for (pos, mut herb) in &mut herbs {
        if !map.in_bounds(pos.x, pos.y) {
            continue;
        }
        let corruption = map.get(pos.x, pos.y).corruption;
        if corruption > 0.3 {
            herb.twisted = true;
        }
    }
}

// ---------------------------------------------------------------------------
// herb_seasonal_check
// ---------------------------------------------------------------------------

/// On season transitions, add or remove the `Harvestable` marker on herbs
/// depending on whether the current season is in the herb's available list.
pub fn herb_seasonal_check(
    query: Query<(Entity, &Herb, &Seasonal, Option<&Harvestable>)>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut commands: Commands,
) {
    if !time.tick.is_multiple_of(config.ticks_per_season) {
        return;
    }

    let current_season = Season::from_tick(time.tick, &config);

    for (entity, herb, seasonal, harvestable) in &query {
        let in_season = seasonal.available.contains(&current_season);

        if in_season && !herb.twisted && harvestable.is_none() {
            commands.entity(entity).insert(Harvestable);
        } else if !in_season && harvestable.is_some() {
            commands.entity(entity).remove::<Harvestable>();
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_shadow_fox_from_corruption
// ---------------------------------------------------------------------------

/// Heavily corrupted tiles (> 0.7) may spontaneously spawn shadow-foxes, up to
/// a population cap of 2.
pub fn spawn_shadow_fox_from_corruption(
    map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
    wildlife: Query<&WildAnimal>,
    time: Res<TimeState>,
    mut commands: Commands,
) {
    if !time.tick.is_multiple_of(10) {
        return;
    }

    let shadow_fox_count = wildlife
        .iter()
        .filter(|a| a.species == WildSpecies::ShadowFox)
        .count();

    if shadow_fox_count >= 2 {
        return;
    }

    for y in 0..map.height {
        for x in 0..map.width {
            if map.get(x, y).corruption > 0.7 && rng.rng.random::<f32>() < 0.001 {
                commands.spawn((
                    WildAnimal::new(WildSpecies::ShadowFox),
                    Position::new(x, y),
                    WildlifeAiState::Patrolling { dx: 1, dy: 0 },
                    Health {
                        current: 1.0,
                        max: 1.0,
                        injuries: Vec::new(),
                    },
                ));
                // Cap at 1 spawn per check.
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// resolve_magic_task_chains
// ---------------------------------------------------------------------------

/// Ticks magic-related TaskChain steps (GatherHerb, PrepareRemedy, ApplyRemedy,
/// SetWard, Scry, CleanseCorruption, SpiritCommunion).
///
/// Runs after `resolve_task_chains` in the schedule. Handles disjoint StepKind
/// variants so the two systems don't conflict.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_magic_task_chains(
    mut cats: Query<
        (
            Entity,
            &mut TaskChain,
            &mut CurrentAction,
            &mut Position,
            &mut Skills,
            &mut Inventory,
            &mut Mood,
            &mut Memory,
            &MagicAffinity,
            &mut Corruption,
            &mut Health,
        ),
        (Without<Dead>, Without<Herb>),
    >,
    herb_entities: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    mut map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    mut relationships: ResMut<Relationships>,
    time: Res<TimeState>,
    mut commands: Commands,
) {
    // Collect workshop positions for speed bonus detection.
    // (We can't query buildings here due to borrow conflicts, so we check terrain.)
    let workshop_tiles_exist = true; // simplified — workshop bonus checked via terrain

    let mut chains_to_remove: Vec<Entity> = Vec::new();
    // Deferred fondness changes for gratitude mechanic.
    let mut gratitude: Vec<(Entity, Entity, f32)> = Vec::new();

    for (cat_entity, mut chain, mut current, mut pos, mut skills, mut inventory, mut mood, mut memory, magic_aff, mut corruption, mut health) in &mut cats {
        let Some(step) = chain.current_mut() else {
            chains_to_remove.push(cat_entity);
            current.ticks_remaining = 0;
            continue;
        };

        // Only handle magic steps.
        let is_magic_step = matches!(
            step.kind,
            StepKind::GatherHerb
                | StepKind::PrepareRemedy { .. }
                | StepKind::ApplyRemedy { .. }
                | StepKind::SetWard { .. }
                | StepKind::Scry
                | StepKind::CleanseCorruption
                | StepKind::SpiritCommunion
        );
        if !is_magic_step {
            continue;
        }

        // Ensure step is in progress.
        if matches!(step.status, StepStatus::Pending) {
            step.status = StepStatus::InProgress { ticks_elapsed: 0 };
        }

        let ticks = match &mut step.status {
            StepStatus::InProgress { ticks_elapsed } => {
                *ticks_elapsed += 1;
                *ticks_elapsed
            }
            _ => continue,
        };

        // Workshop proximity check via terrain.
        let at_workshop = map.in_bounds(pos.x, pos.y)
            && map.get(pos.x, pos.y).terrain == Terrain::Workshop;
        let _ = workshop_tiles_exist;

        match step.kind.clone() {
            StepKind::GatherHerb => {
                if ticks >= 5 {
                    // Find the herb entity at the target position.
                    let target_entity = step.target_entity;
                    if let Some(herb_e) = target_entity {
                        if let Ok((_, herb, _)) = herb_entities.get(herb_e) {
                            if inventory.add_herb(herb.kind) {
                                commands.entity(herb_e).despawn();
                                skills.herbcraft += skills.growth_rate() * 0.01;
                                chain.advance();
                            } else {
                                chain.fail_current("inventory full".into());
                            }
                        } else {
                            chain.fail_current("herb already taken".into());
                        }
                    } else {
                        chain.fail_current("no herb target".into());
                    }
                }
            }

            StepKind::PrepareRemedy { remedy } => {
                let required_ticks = if at_workshop { 10 } else { 15 };
                if ticks >= required_ticks {
                    let herb_needed = remedy.required_herb();
                    if inventory.take_herb(herb_needed) {
                        skills.herbcraft += skills.growth_rate() * 0.01;
                        chain.advance();
                    } else {
                        chain.fail_current("missing herb for remedy".into());
                    }
                }
            }

            StepKind::ApplyRemedy { remedy } => {
                // Move to patient if not adjacent.
                if let Some(target_pos) = step.target_position {
                    if pos.manhattan_distance(&target_pos) > 1 {
                        if let Some(next) = step_toward(&pos, &target_pos, &map) {
                            *pos = next;
                        }
                        // Don't advance yet — still walking.
                        continue;
                    }
                }

                // Apply remedy to target.
                if let Some(patient) = step.target_entity {
                    commands.entity(patient).insert(RemedyEffect {
                        kind: remedy,
                        ticks_remaining: remedy.duration(),
                        healer: Some(cat_entity),
                    });
                    // Gratitude: deferred fondness increase.
                    gratitude.push((patient, cat_entity, 0.1));

                    let cat_name = "a herbalist"; // simplified
                    log.push(
                        time.tick,
                        format!("{cat_name} applies a remedy with careful paws."),
                        NarrativeTier::Action,
                    );
                }
                skills.herbcraft += skills.growth_rate() * 0.005;
                chain.advance();
            }

            StepKind::SetWard { kind } => {
                if ticks >= 8 {
                    // Consume thornbriar if setting a thornward.
                    if kind == WardKind::Thornward
                        && !inventory.take_herb(crate::components::magic::HerbKind::Thornbriar)
                    {
                        chain.fail_current("no thornbriar for ward".into());
                        continue;
                    }

                    // Check for misfire on magical actions.
                    if kind == WardKind::DurableWard {
                        if let Some(misfire) = check_misfire(magic_aff.0, skills.magic, &mut rng.rng) {
                            apply_misfire(misfire, cat_entity, &mut mood, &mut corruption, &mut health, &pos, &mut commands, &mut log, time.tick);
                            if matches!(misfire, MisfireEffect::Fizzle) {
                                chain.fail_current("misfire: fizzle".into());
                                continue;
                            }
                            if matches!(misfire, MisfireEffect::InvertedWard) {
                                // Spawn inverted ward instead.
                                commands.spawn((
                                    Ward::inverted_at(kind),
                                    Position::new(pos.x, pos.y),
                                ));
                                chain.advance();
                                continue;
                            }
                        }
                    }

                    // Spawn the ward entity.
                    let ward = match kind {
                        WardKind::Thornward => Ward::thornward(),
                        WardKind::DurableWard => Ward::durable(),
                    };
                    commands.spawn((ward, Position::new(pos.x, pos.y)));
                    skills.herbcraft += skills.growth_rate() * 0.01;
                    if kind == WardKind::DurableWard {
                        skills.magic += skills.growth_rate() * 0.01;
                    }
                    chain.advance();
                }
            }

            StepKind::Scry => {
                if ticks == 1 {
                    // Misfire check on first tick.
                    if let Some(misfire) = check_misfire(magic_aff.0, skills.magic, &mut rng.rng) {
                        apply_misfire(misfire, cat_entity, &mut mood, &mut corruption, &mut health, &pos, &mut commands, &mut log, time.tick);
                        if matches!(misfire, MisfireEffect::Fizzle) {
                            chain.fail_current("misfire: fizzle".into());
                            continue;
                        }
                    }
                }
                if ticks >= 10 {
                    // Create a memory of a random distant tile.
                    let rx = rng.rng.random_range(0..map.width);
                    let ry = rng.rng.random_range(0..map.height);
                    memory.remember(MemoryEntry {
                        event_type: MemoryType::ResourceFound,
                        location: Some(Position::new(rx, ry)),
                        involved: vec![],
                        tick: time.tick,
                        strength: 0.6,
                        firsthand: true,
                    });
                    skills.magic += skills.growth_rate() * 0.01;
                    chain.advance();
                }
            }

            StepKind::CleanseCorruption => {
                if ticks == 1 {
                    // Misfire check on first tick.
                    if let Some(misfire) = check_misfire(magic_aff.0, skills.magic, &mut rng.rng) {
                        apply_misfire(misfire, cat_entity, &mut mood, &mut corruption, &mut health, &pos, &mut commands, &mut log, time.tick);
                        if matches!(misfire, MisfireEffect::Fizzle) {
                            chain.fail_current("misfire: fizzle".into());
                            continue;
                        }
                    }
                }

                // Per-tick: reduce tile corruption.
                if map.in_bounds(pos.x, pos.y) {
                    let tile = map.get_mut(pos.x, pos.y);
                    tile.corruption = (tile.corruption - skills.magic * 0.01).max(0.0);
                }
                // Occupational hazard: personal corruption increases.
                corruption.0 = (corruption.0 + 0.005).min(1.0);
                skills.magic += skills.growth_rate() * 0.005;

                // Advance when tile is cleansed or after 100 ticks.
                let done = if map.in_bounds(pos.x, pos.y) {
                    map.get(pos.x, pos.y).corruption < 0.05
                } else {
                    true
                };
                if done || ticks >= 100 {
                    chain.advance();
                }
            }

            StepKind::SpiritCommunion => {
                if ticks == 1 {
                    if let Some(misfire) = check_misfire(magic_aff.0, skills.magic, &mut rng.rng) {
                        apply_misfire(misfire, cat_entity, &mut mood, &mut corruption, &mut health, &pos, &mut commands, &mut log, time.tick);
                        if matches!(misfire, MisfireEffect::Fizzle) {
                            chain.fail_current("misfire: fizzle".into());
                            continue;
                        }
                    }
                }
                if ticks >= 15 {
                    mood.modifiers.push_back(MoodModifier {
                        amount: 0.3,
                        ticks_remaining: 100,
                        source: "spirit communion".to_string(),
                    });
                    skills.magic += skills.growth_rate() * 0.01;
                    chain.advance();
                }
            }

            // Non-magic steps handled by resolve_task_chains.
            _ => {}
        }

        if chain.is_complete() {
            chains_to_remove.push(cat_entity);
            current.ticks_remaining = 0;
        }
    }

    for entity in chains_to_remove {
        commands.entity(entity).remove::<TaskChain>();
    }

    // Apply deferred gratitude fondness changes.
    for (healed, healer, amount) in gratitude {
        relationships.modify_fondness(healed, healer, amount);
    }
}

/// Apply a misfire effect to the caster.
#[allow(clippy::too_many_arguments)]
fn apply_misfire(
    effect: MisfireEffect,
    _cat_entity: Entity,
    mood: &mut Mood,
    corruption: &mut Corruption,
    health: &mut Health,
    pos: &Position,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    tick: u64,
) {
    match effect {
        MisfireEffect::Fizzle => {
            mood.modifiers.push_back(MoodModifier {
                amount: -0.1,
                ticks_remaining: 20,
                source: "embarrassment".to_string(),
            });
            log.push(tick, "A cat concentrates... and nothing happens.".to_string(), NarrativeTier::Significant);
        }
        MisfireEffect::CorruptionBacksplash => {
            corruption.0 = (corruption.0 + 0.1).min(1.0);
            log.push(tick, "Dark energy surges back into a cat!".to_string(), NarrativeTier::Significant);
        }
        MisfireEffect::InvertedWard => {
            // Spawned by the caller — just log here.
            log.push(tick, "The ward twists, its light turning sickly...".to_string(), NarrativeTier::Significant);
        }
        MisfireEffect::WoundTransfer => {
            health.injuries.push(Injury {
                kind: InjuryKind::Minor,
                tick_received: tick,
                healed: false,
            });
            log.push(tick, "A cat gasps as a wound appears on their own flank.".to_string(), NarrativeTier::Significant);
        }
        MisfireEffect::LocationReveal => {
            // Create a MagicEvent memory that wildlife systems can check.
            log.push(tick, "Something in the darkness turns its head...".to_string(), NarrativeTier::Significant);
            // The inverted ward spawned at the caster's location acts as a beacon.
            commands.spawn((
                Ward::inverted_at(WardKind::Thornward),
                Position::new(pos.x, pos.y),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// check_misfire (helper, not a system)
// ---------------------------------------------------------------------------

/// Determine whether a magic attempt misfires, based on the gap between
/// affinity and skill. Returns `None` when the attempt succeeds cleanly.
pub fn check_misfire(affinity: f32, skill: f32, rng: &mut impl Rng) -> Option<MisfireEffect> {
    if skill >= affinity * 0.8 {
        return None;
    }
    let chance = (affinity - skill) * 0.5;
    if rng.random::<f32>() >= chance {
        return None;
    }
    let roll: f32 = rng.random();
    Some(match roll {
        r if r < 0.3 => MisfireEffect::Fizzle,
        r if r < 0.5 => MisfireEffect::CorruptionBacksplash,
        r if r < 0.7 => MisfireEffect::InvertedWard,
        r if r < 0.9 => MisfireEffect::WoundTransfer,
        _ => MisfireEffect::LocationReveal,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use crate::resources::time::SimSpeed;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(TileMap::new(10, 10, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(Relationships::default());
        world
    }

    // -----------------------------------------------------------------------
    // check_misfire
    // -----------------------------------------------------------------------

    #[test]
    fn misfire_no_misfire_when_skilled() {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;

        let mut rng = ChaCha8Rng::seed_from_u64(1);
        // skill (0.5) >= affinity (0.5) * 0.8 = 0.4 → always None
        for _ in 0..100 {
            assert!(check_misfire(0.5, 0.5, &mut rng).is_none());
        }
    }

    #[test]
    fn misfire_high_chance_when_unskilled() {
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;

        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut some_count = 0;
        for _ in 0..200 {
            if check_misfire(0.9, 0.1, &mut rng).is_some() {
                some_count += 1;
            }
        }
        // chance = (0.9 - 0.1) * 0.5 = 0.4 → ~40% should misfire
        assert!(
            some_count > 40,
            "expected many misfires with unskilled cat, got {some_count}/200"
        );
    }

    // -----------------------------------------------------------------------
    // ward_decay
    // -----------------------------------------------------------------------

    #[test]
    fn ward_decay_removes_at_zero() {
        let mut world = test_world();

        let ward_entity = world
            .spawn((
                Ward {
                    kind: crate::components::magic::WardKind::Thornward,
                    strength: 0.01,
                    decay_rate: 0.02,
                    inverted: false,
                },
                Position::new(0, 0),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(ward_decay);
        schedule.run(&mut world);

        // After one tick: strength = 0.01 - 0.02 = -0.01 → despawned
        assert!(
            world.get_entity(ward_entity).is_err(),
            "ward should be despawned after strength drops to zero"
        );
    }

    // -----------------------------------------------------------------------
    // corruption_spread
    // -----------------------------------------------------------------------

    #[test]
    fn corruption_spreads_to_adjacent() {
        let mut world = test_world();

        // Set tick to a multiple of 10 so the system runs.
        world.resource_mut::<TimeState>().tick = 10;

        // Set one tile to corruption 0.5 (above 0.3 threshold).
        world.resource_mut::<TileMap>().get_mut(5, 5).corruption = 0.5;

        let mut schedule = Schedule::default();
        schedule.add_systems(corruption_spread);
        schedule.run(&mut world);

        let map = world.resource::<TileMap>();
        let expected_spread = 0.5 * 0.001;

        // 4-adjacent tiles should have gained corruption.
        for (nx, ny) in [(5, 4), (5, 6), (4, 5), (6, 5)] {
            let c = map.get(nx, ny).corruption;
            assert!(
                (c - expected_spread).abs() < 1e-6,
                "tile ({nx},{ny}) should have corruption {expected_spread}, got {c}"
            );
        }

        // Diagonal should be unaffected.
        let diag = map.get(6, 6).corruption;
        assert!(
            diag.abs() < 1e-6,
            "diagonal tile should be unaffected, got {diag}"
        );
    }

    // -----------------------------------------------------------------------
    // apply_remedy_effects
    // -----------------------------------------------------------------------

    #[test]
    fn remedy_heals_over_time() {
        let mut world = test_world();

        let cat = world
            .spawn((
                Health {
                    current: 0.5,
                    max: 1.0,
                    injuries: Vec::new(),
                },
                Needs::default(),
                Mood::default(),
                RemedyEffect {
                    kind: RemedyKind::HealingPoultice,
                    ticks_remaining: RemedyKind::HealingPoultice.duration(),
                    healer: None,
                },
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_remedy_effects);
        schedule.run(&mut world);

        let health = world.get::<Health>(cat).unwrap();
        assert!(
            (health.current - 0.55).abs() < 1e-6,
            "health should increase by 0.05 per tick, got {}",
            health.current
        );
    }
}
