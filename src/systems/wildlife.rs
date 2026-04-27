use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::building::{ConstructionSite, Structure};
use crate::components::identity::Name;
use crate::components::magic::Ward;
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::{PreyAnimal, PreyConfig};
use crate::components::wildlife::{
    BehaviorType, FoxAiPhase, FoxDen, FoxLifeStage, FoxSex, FoxState, WildAnimal, WildSpecies,
    WildlifeAiState,
};
use crate::resources::cat_presence_map::CatPresenceMap;
use crate::resources::food::FoodStores;
use crate::resources::fox_scent_map::FoxScentMap;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{Season, SimConfig, TimeScale, TimeState};

/// Per-cat cooldown tracking for threat detection narratives.
/// Suppresses repeated detection lines for the same cat for 100 ticks (~1 day).
#[derive(Resource, Default, Debug)]
pub struct DetectionCooldowns {
    /// Per-cat detection cooldown (entity → earliest next tick).
    pub cat_cooldowns: HashMap<Entity, u64>,
    /// Per-species spawn narrative cooldown (species → earliest next tick).
    pub spawn_cooldowns: HashMap<WildSpecies, u64>,
}

// Detection narrative cooldown is now read from SimConstants.wildlife.detection_narrative_cooldown.

// ---------------------------------------------------------------------------
// Wildlife AI system
// ---------------------------------------------------------------------------

/// Move each wild animal according to its behavior pattern.
#[allow(clippy::type_complexity)]
pub fn wildlife_ai(
    mut query: Query<(&WildAnimal, &mut Position, &mut WildlifeAiState), Without<FoxState>>,
    wards: Query<(&Ward, &Position), Without<WildAnimal>>,
    cat_positions: Query<
        &Position,
        (
            With<Needs>,
            Without<Dead>,
            Without<PreyAnimal>,
            Without<WildAnimal>,
        ),
    >,
    mut map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.wildlife;
    let ward_multiplier = constants.magic.shadow_fox_ward_repel_multiplier;

    // Snapshot ward positions (non-inverted, alive) for shadow fox avoidance.
    let ward_positions: Vec<(Position, f32)> = wards
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius() * ward_multiplier))
        .collect();

    for (animal, mut pos, mut ai_state) in &mut query {
        match *ai_state {
            WildlifeAiState::Patrolling { dx, dy } => {
                // Shadow fox ward avoidance: reverse if next step enters a ward.
                if animal.species == WildSpecies::ShadowFox {
                    let next = Position::new(pos.x + dx, pos.y + dy);
                    if let Some((wp, _radius)) = ward_positions
                        .iter()
                        .find(|(wp, radius)| (next.manhattan_distance(wp) as f32) <= *radius)
                    {
                        // Chance to siege the ward instead of retreating.
                        if rng.rng.random::<f32>() < c.ward_siege_chance {
                            *ai_state = WildlifeAiState::EncirclingWard {
                                ward_x: wp.x,
                                ward_y: wp.y,
                                angle: 0.0,
                                ticks: 0,
                            };
                            activation.record(Feature::WardSiegeStarted);
                        } else {
                            *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                        }
                        activation.record(Feature::ShadowFoxAvoidedWard);
                        continue;
                    }
                }

                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y)
                    && is_patrol_terrain(map.get(next.x, next.y).terrain, animal.species)
                {
                    *pos = next;
                } else {
                    // Reverse direction and try the other way.
                    let rev = Position::new(pos.x - dx, pos.y - dy);
                    if map.in_bounds(rev.x, rev.y) {
                        *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                        *pos = rev;
                    }
                    // If neither works, stay put (cornered).
                }
            }
            WildlifeAiState::Circling {
                center_x,
                center_y,
                ref mut angle,
            } => {
                *angle += c.circling_angle_step;
                if *angle > std::f32::consts::TAU {
                    *angle -= std::f32::consts::TAU;
                }
                let radius = c.circling_radius;
                let target_x = center_x + (angle.cos() * radius) as i32;
                let target_y = center_y + (angle.sin() * radius) as i32;

                // Move one step toward the circle target.
                let dx = (target_x - pos.x).signum();
                let dy = (target_y - pos.y).signum();
                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y)
                    && map.get(next.x, next.y).terrain.is_wildlife_passable()
                {
                    *pos = next;
                }
            }
            WildlifeAiState::Waiting => {
                // Ambush: don't move.
            }
            WildlifeAiState::Fleeing { dx, dy } => {
                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y) {
                    *pos = next;
                }
                // If we'd go off-map, despawn is handled by cleanup_wildlife.
            }
            WildlifeAiState::EncirclingWard {
                ward_x,
                ward_y,
                ref mut angle,
                ref mut ticks,
            } => {
                *ticks += 1;

                // Check if ward still exists (not destroyed).
                let ward_alive = ward_positions
                    .iter()
                    .any(|(wp, _)| wp.x == ward_x && wp.y == ward_y);

                // Break siege if cat approaches or ward destroyed or timed out.
                // Phase 5a: shadow-fox sight channel with LoS check.
                let cat_nearby = cat_positions.iter().any(|cp| {
                    crate::systems::sensing::observer_sees_at_with_los(
                        crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
                        *pos,
                        &constants.sensory.shadow_fox,
                        *cp,
                        crate::components::SensorySignature::CAT,
                        c.siege_break_range as f32,
                        &map,
                    )
                });
                if !ward_alive || *ticks >= c.ward_siege_max_ticks {
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                } else if cat_nearby {
                    // Aggression: siege provokes confrontation.
                    if let Some(cat_pos) = cat_positions
                        .iter()
                        .min_by_key(|cp| (cp.x - pos.x).abs() + (cp.y - pos.y).abs())
                    {
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x,
                            target_y: cat_pos.y,
                        };
                    }
                } else {
                    // Orbit at ward edge + 1 tile.
                    *angle += c.circling_angle_step;
                    if *angle > std::f32::consts::TAU {
                        *angle -= std::f32::consts::TAU;
                    }
                    let orbit_radius = ward_positions
                        .iter()
                        .find(|(wp, _)| wp.x == ward_x && wp.y == ward_y)
                        .map(|(_, r)| *r + 1.0)
                        .unwrap_or(4.0);
                    let tx = ward_x + (angle.cos() * orbit_radius) as i32;
                    let ty = ward_y + (angle.sin() * orbit_radius) as i32;
                    let dx = (tx - pos.x).signum();
                    let dy = (ty - pos.y).signum();
                    let next = Position::new(pos.x + dx, pos.y + dy);
                    if map.in_bounds(next.x, next.y)
                        && map.get(next.x, next.y).terrain.is_wildlife_passable()
                    {
                        *pos = next;
                    }

                    // Deposit siege corruption at 3x normal rate.
                    if map.in_bounds(pos.x, pos.y) {
                        let tile = map.get_mut(pos.x, pos.y);
                        tile.corruption = (tile.corruption + c.ward_siege_corruption_rate).min(1.0);
                    }
                }
            }

            WildlifeAiState::Stalking { target_x, target_y } => {
                // Shadow fox ward avoidance: cancel stalk if next step enters a ward.
                if animal.species == WildSpecies::ShadowFox {
                    let dx = (target_x - pos.x).signum();
                    let dy = (target_y - pos.y).signum();
                    let next = Position::new(pos.x + dx, pos.y + dy);
                    let enters_ward = ward_positions
                        .iter()
                        .any(|(wp, radius)| (next.manhattan_distance(wp) as f32) <= *radius);
                    if enters_ward {
                        *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                        activation.record(Feature::ShadowFoxAvoidedWard);
                        continue;
                    }
                }

                // Move one step toward the target cat.
                let dx = (target_x - pos.x).signum();
                let dy = (target_y - pos.y).signum();
                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y)
                    && map.get(next.x, next.y).terrain.is_wildlife_passable()
                {
                    *pos = next;
                } else {
                    // Can't reach target, revert to patrolling.
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                }
            }
        }

        // ShadowFox spreads corruption to tiles it crosses.
        if animal.species == WildSpecies::ShadowFox && map.in_bounds(pos.x, pos.y) {
            let tile = map.get_mut(pos.x, pos.y);
            tile.corruption = (tile.corruption + c.shadow_fox_corruption_deposit).min(1.0);
        }

        // Small random direction jitter for patrol creatures to avoid getting stuck.
        if matches!(*ai_state, WildlifeAiState::Patrolling { .. })
            && rng.rng.random::<f32>() < c.patrol_jitter_chance
        {
            let new_dx = rng.rng.random_range(-1i32..=1);
            let new_dy = rng.rng.random_range(-1i32..=1);
            if new_dx != 0 || new_dy != 0 {
                *ai_state = WildlifeAiState::Patrolling {
                    dx: new_dx,
                    dy: new_dy,
                };
            }
        }
    }
}

/// Returns true if the given terrain is suitable for patrolling by this species.
fn is_patrol_terrain(terrain: Terrain, species: WildSpecies) -> bool {
    match species {
        WildSpecies::Fox => matches!(
            terrain,
            Terrain::LightForest | Terrain::DenseForest | Terrain::Grass
        ),
        WildSpecies::Hawk => matches!(
            terrain,
            Terrain::Grass | Terrain::Sand | Terrain::LightForest
        ),
        WildSpecies::Snake => matches!(terrain, Terrain::Rock | Terrain::Mud | Terrain::Grass),
        WildSpecies::ShadowFox => matches!(
            terrain,
            Terrain::LightForest | Terrain::DenseForest | Terrain::Grass
        ),
    }
}

// ---------------------------------------------------------------------------
// Wildlife spawning system
// ---------------------------------------------------------------------------

/// Attempt to spawn new wildlife at map edges, respecting population caps.
#[allow(clippy::too_many_arguments)]
pub fn spawn_wildlife(
    query: Query<&WildAnimal>,
    mut commands: Commands,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut cooldowns: ResMut<DetectionCooldowns>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.wildlife;
    // Fox population is managed by FoxDen breeding, not edge-spawned.
    // ShadowFox is corruption-spawned only.
    for species in [WildSpecies::Hawk, WildSpecies::Snake] {
        let current_count = query.iter().filter(|a| a.species == species).count();
        if current_count >= species.population_cap() {
            continue;
        }

        if rng.rng.random::<f32>() >= species.spawn_chance() {
            continue;
        }

        // Pick a random map-edge tile.
        if let Some(spawn_pos) = pick_edge_spawn(&map, species, &mut rng.rng) {
            activation.record(Feature::WildlifeSpawned);
            let animal = WildAnimal::new(species);
            let ai_state = initial_ai_state(species, &spawn_pos, &map, &mut rng.rng);
            commands.spawn((
                animal,
                spawn_pos,
                Health::default(),
                ai_state,
                crate::components::SensorySpecies::Wild(species),
                crate::components::SensorySignature::WILDLIFE,
            ));

            // Rate-limited spawn narrative.
            let on_cooldown = cooldowns
                .spawn_cooldowns
                .get(&species)
                .is_some_and(|&last| time.tick.saturating_sub(last) < c.spawn_narrative_cooldown);

            if !on_cooldown {
                let text = match species {
                    WildSpecies::Fox => "A fox emerges from the forest edge.",
                    WildSpecies::Hawk => "A hawk begins circling overhead.",
                    WildSpecies::Snake => "A snake slithers out from the underbrush.",
                    WildSpecies::ShadowFox => "A shadow-fox materializes from the corruption.",
                };
                log.push(time.tick, text.to_string(), NarrativeTier::Danger);
                cooldowns.spawn_cooldowns.insert(species, time.tick);
            }
        }
    }
}

/// Pick a random map-edge tile suitable for the given species.
fn pick_edge_spawn(map: &TileMap, species: WildSpecies, rng: &mut impl Rng) -> Option<Position> {
    // Collect candidate edge tiles.
    let mut candidates = Vec::new();

    // Top and bottom rows.
    for x in 0..map.width {
        for &y in &[0, map.height - 1] {
            if is_spawn_terrain(map.get(x, y).terrain, species) {
                candidates.push(Position::new(x, y));
            }
        }
    }
    // Left and right columns (skip corners already counted).
    for y in 1..(map.height - 1) {
        for &x in &[0, map.width - 1] {
            if is_spawn_terrain(map.get(x, y).terrain, species) {
                candidates.push(Position::new(x, y));
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let idx = rng.random_range(0..candidates.len());
    Some(candidates[idx])
}

/// Returns true if the terrain is suitable for spawning this species.
fn is_spawn_terrain(terrain: Terrain, species: WildSpecies) -> bool {
    match species {
        WildSpecies::Fox => matches!(
            terrain,
            Terrain::LightForest | Terrain::DenseForest | Terrain::Grass
        ),
        WildSpecies::Hawk => matches!(terrain, Terrain::Grass | Terrain::Sand),
        WildSpecies::Snake => matches!(terrain, Terrain::Rock | Terrain::Mud),
        WildSpecies::ShadowFox => matches!(
            terrain,
            Terrain::LightForest | Terrain::DenseForest | Terrain::Grass
        ),
    }
}

/// Create the initial AI state for a newly spawned animal.
fn initial_ai_state(
    species: WildSpecies,
    pos: &Position,
    map: &TileMap,
    rng: &mut impl Rng,
) -> WildlifeAiState {
    match species.default_behavior() {
        BehaviorType::Patrol => {
            // Pick a random direction along the edge.
            let dx = if pos.x == 0 {
                1
            } else if pos.x == map.width - 1 {
                -1
            } else if rng.random() {
                1
            } else {
                -1
            };
            let dy = if pos.y == 0 {
                1
            } else if pos.y == map.height - 1 {
                -1
            } else {
                0
            };
            WildlifeAiState::Patrolling { dx, dy }
        }
        BehaviorType::Circle => {
            // Circle around a point ~8 tiles inward from spawn.
            let center_x = (pos.x + (map.width / 2 - pos.x).signum() * 8).clamp(0, map.width - 1);
            let center_y = (pos.y + (map.height / 2 - pos.y).signum() * 8).clamp(0, map.height - 1);
            WildlifeAiState::Circling {
                center_x,
                center_y,
                angle: rng.random_range(0.0..std::f32::consts::TAU),
            }
        }
        BehaviorType::Ambush => WildlifeAiState::Waiting,
    }
}

// ---------------------------------------------------------------------------
// Threat detection system
// ---------------------------------------------------------------------------

// Detection range constants are now read from SimConstants.wildlife.

/// Each tick, living cats scan for nearby wildlife and react with fear.
///
/// Cats already performing a Fight action skip detection (they know the threat).
/// Detection is deduped: a cat won't re-trigger fear for a threat it already
/// has a fresh `ThreatSeen` memory about.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn detect_threats(
    mut cats: Query<
        (
            Entity,
            &Position,
            &CurrentAction,
            &mut Needs,
            &mut Memory,
            &mut Mood,
            &Name,
        ),
        Without<Dead>,
    >,
    wildlife: Query<(Entity, &Position, &WildAnimal)>,
    watchtowers: Query<
        (&crate::components::building::Structure, &Position),
        Without<crate::components::building::ConstructionSite>,
    >,
    map: Res<TileMap>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut cooldowns: ResMut<DetectionCooldowns>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
) {
    let c = &constants.wildlife;

    // Snapshot wildlife positions so we can iterate cats mutably.
    let threats: Vec<(Entity, Position, WildSpecies)> = wildlife
        .iter()
        .map(|(e, p, a)| (e, *p, a.species))
        .collect();

    // Cache watchtower positions for detection range bonus.
    let watchtower_positions: Vec<Position> = watchtowers
        .iter()
        .filter(|(s, _)| {
            s.kind == crate::components::building::StructureType::Watchtower
                && s.effectiveness() > 0.0
        })
        .map(|(_, pos)| *pos)
        .collect();

    for (cat_entity, cat_pos, current, mut needs, mut memory, mut mood, name) in &mut cats {
        // Cats already fighting know about the threat.
        if current.action == Action::Fight {
            continue;
        }

        let detection_range = {
            let mut range = c.base_detection_range;
            if map.in_bounds(cat_pos.x, cat_pos.y) {
                let terrain = map.get(cat_pos.x, cat_pos.y).terrain;
                if matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
                    range -= c.forest_range_penalty;
                }
            }
            // Patrolling cats get doubled detection range.
            if current.action == Action::Patrol {
                range *= 2;
            }
            // Watchtower doubles detection range for cats standing on one.
            if watchtower_positions
                .iter()
                .any(|wp| cat_pos.manhattan_distance(wp) == 0)
            {
                range *= 2;
            }
            range.max(1)
        };

        for &(threat_entity, threat_pos, species) in &threats {
            // Phase 5a migration: cat-observer sight channel, with the
            // terrain/action/watchtower-modulated range threaded via
            // max_range_override.
            if !crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *cat_pos,
                &constants.sensory.cat,
                threat_pos,
                crate::components::SensorySignature::WILDLIFE,
                detection_range as f32,
            ) {
                continue;
            }

            // Dedup: skip if cat already has a fresh ThreatSeen memory for this entity.
            let already_detected = memory.events.iter().any(|e| {
                e.event_type == MemoryType::ThreatSeen
                    && e.strength > 0.5
                    && e.involved.contains(&threat_entity)
            });
            if already_detected {
                continue;
            }

            // React to the threat.
            needs.safety = (needs.safety - c.threat_safety_drain).max(0.0);

            memory.remember(MemoryEntry {
                event_type: MemoryType::ThreatSeen,
                location: Some(threat_pos),
                involved: vec![threat_entity],
                tick: time.tick,
                strength: 1.0,
                firsthand: true,
            });

            mood.modifiers.push_back(MoodModifier {
                amount: c.threat_mood_penalty,
                ticks_remaining: c.threat_mood_ticks,
                source: format!("{} spotted", species.name()),
            });

            // Detection narrative with per-cat cooldown.
            let on_cooldown = cooldowns
                .cat_cooldowns
                .get(&cat_entity)
                .is_some_and(|&last| {
                    time.tick.saturating_sub(last) < c.detection_narrative_cooldown
                });

            if !on_cooldown {
                let cat = &name.0;
                let text = match species {
                    WildSpecies::Fox => {
                        let variants = [
                            format!("{cat} spots a fox slinking through the undergrowth."),
                            format!("{cat} catches the scent of fox on the wind."),
                            format!(
                                "{cat} freezes \u{2014} a rust-red shape moves between the trees."
                            ),
                            format!("{cat} hears something prowling in the brush."),
                        ];
                        let idx = rng.rng.random_range(0..variants.len());
                        variants[idx].clone()
                    }
                    WildSpecies::Hawk => {
                        let variants = [
                            format!("A hawk circles overhead \u{2014} {cat} freezes."),
                            format!(
                                "{cat} spots a shadow sweeping across the ground \u{2014} a hawk."
                            ),
                            format!("{cat} looks up sharply. A raptor rides the thermals."),
                        ];
                        let idx = rng.rng.random_range(0..variants.len());
                        variants[idx].clone()
                    }
                    WildSpecies::Snake => {
                        let variants = [
                            format!(
                                "{cat} hisses and recoils \u{2014} a snake lies coiled nearby."
                            ),
                            format!("A dry rattle stops {cat} mid-stride."),
                            format!("{cat} leaps back from a serpent half-hidden in the grass."),
                        ];
                        let idx = rng.rng.random_range(0..variants.len());
                        variants[idx].clone()
                    }
                    WildSpecies::ShadowFox => {
                        let variants = [
                            format!("A chill runs through {cat} \u{2014} a shadow-fox drifts among the trees."),
                            format!("{cat}'s fur stands on end. Something wrong moves in the darkness."),
                            format!("The air turns cold around {cat}. A shadow-fox is near."),
                        ];
                        let idx = rng.rng.random_range(0..variants.len());
                        variants[idx].clone()
                    }
                };
                log.push(time.tick, text, NarrativeTier::Action);
                cooldowns.cat_cooldowns.insert(cat_entity, time.tick);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Predator hunt prey system
// ---------------------------------------------------------------------------

/// Predators (fox, hawk, snake) hunt nearby prey entities.
/// When a predator kills prey, the prey entity is despawned immediately.
/// Foxes with `FoxState` only hunt when in `HuntingPrey` phase and gain satiation.
#[allow(clippy::too_many_arguments)]
pub fn predator_hunt_prey(
    mut commands: Commands,
    predators: Query<(Entity, &WildAnimal, &Position, Option<&FoxAiPhase>), Without<PreyAnimal>>,
    prey: Query<(Entity, &PreyConfig, &Position), With<PreyAnimal>>,
    mut fox_states: Query<&mut FoxState>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    constants: Res<SimConstants>,
    map: Res<TileMap>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.wildlife;
    let fc = &constants.fox_ecology;
    let satiation_prey_kill = fc.satiation_after_prey_kill.ticks(&time_scale);
    for (pred_entity, predator, pred_pos, fox_phase) in predators.iter() {
        // Foxes with ecology: only hunt when in HuntingPrey phase.
        if let Some(phase) = fox_phase {
            if !matches!(phase, FoxAiPhase::HuntingPrey { .. }) {
                continue;
            }
        }

        // Only hunt sometimes.
        if rng.rng.random::<f32>() > c.predator_hunt_chance {
            continue;
        }

        let hunt_range: i32 = match predator.species {
            WildSpecies::Fox => c.predator_hunt_range_fox,
            WildSpecies::Hawk => c.predator_hunt_range_hawk,
            WildSpecies::Snake => c.predator_hunt_range_snake,
            WildSpecies::ShadowFox => c.predator_hunt_range_shadow_fox,
        };
        let predator_profile = constants
            .sensory
            .profile_for(crate::components::SensorySpecies::Wild(predator.species));

        // Find nearest prey in range. Phase 5a: predator sight with LoS.
        let mut nearest: Option<(Entity, i32)> = None;
        for (prey_entity, _prey_animal, prey_pos) in prey.iter() {
            if !crate::systems::sensing::observer_sees_at_with_los(
                crate::components::SensorySpecies::Wild(predator.species),
                *pred_pos,
                predator_profile,
                *prey_pos,
                crate::components::SensorySignature::PREY,
                hunt_range as f32,
                &map,
            ) {
                continue;
            }
            let dist = pred_pos.manhattan_distance(prey_pos);
            if nearest.is_none() || dist < nearest.unwrap().1 {
                nearest = Some((prey_entity, dist));
            }
        }

        if let Some((prey_entity, _)) = nearest {
            if let Ok((_, prey_cfg, prey_pos)) = prey.get(prey_entity) {
                if rng.rng.random::<f32>() < c.predator_kill_chance {
                    let species_name = prey_cfg.name;
                    let predator_name = predator.species.name();
                    let kill_pos = *prey_pos;
                    let kill_kind = prey_cfg.kind;
                    commands.entity(prey_entity).despawn();

                    // Shadow fox kills sometimes leave rotting carcasses that emit corruption.
                    if predator.species == WildSpecies::ShadowFox
                        && rng.rng.random::<f32>() < c.carcass_drop_chance
                    {
                        commands.spawn((
                            crate::components::wildlife::Carcass {
                                prey_kind: kill_kind,
                                age_ticks: 0,
                                corruption_rate: c.carcass_corruption_rate,
                                cleansed: false,
                                harvested: false,
                            },
                            kill_pos,
                            crate::components::SensorySignature::CARCASS,
                        ));
                        activation.record(Feature::CarcassSpawned);
                    }

                    // Fox-specific: gain satiation from kill.
                    if let Ok(mut fox_state) = fox_states.get_mut(pred_entity) {
                        fox_state.satiation_ticks = satiation_prey_kill;
                        fox_state.hunger = (fox_state.hunger - 0.3).max(0.0);
                        activation.record(Feature::FoxHuntedPrey);
                    }

                    // Rate-limited logging.
                    if rng.rng.random::<f32>() < c.predator_kill_narrative_chance {
                        let text = match predator.species {
                            WildSpecies::Fox | WildSpecies::ShadowFox => {
                                format!("A {predator_name} snatches a {species_name} from the undergrowth.")
                            }
                            WildSpecies::Hawk => {
                                format!("A hawk dives and plucks a {species_name} from the ground.")
                            }
                            WildSpecies::Snake => {
                                format!("A snake strikes at a {species_name} in the grass.")
                            }
                        };
                        log.push(time.tick, text, NarrativeTier::Nature);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// carcass_decay — rotting carcasses emit corruption until they crumble
// ---------------------------------------------------------------------------

pub fn carcass_decay(
    mut commands: Commands,
    mut carcasses: Query<(Entity, &mut crate::components::wildlife::Carcass, &Position)>,
    mut map: ResMut<TileMap>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
) {
    let c = &constants.wildlife;
    for (entity, mut carcass, pos) in &mut carcasses {
        carcass.age_ticks += 1;

        // Emit corruption unless cleansed.
        if !carcass.cleansed && map.in_bounds(pos.x, pos.y) {
            let tile = map.get_mut(pos.x, pos.y);
            tile.corruption = (tile.corruption + carcass.corruption_rate).min(1.0);
        }

        // Crumble after max age.
        if carcass.age_ticks >= c.carcass_max_age {
            log.push(
                time.tick,
                "The remains crumble to dust.".to_string(),
                NarrativeTier::Nature,
            );
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// predator_stalk_cats — foxes actively hunt nearby cats
// ---------------------------------------------------------------------------

/// Foxes within detection range of cats may switch to Stalking behavior.
/// A stalking fox that reaches an adjacent tile ambushes the nearest cat,
/// dealing damage and draining safety.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn predator_stalk_cats(
    mut wildlife: Query<
        (
            &mut WildAnimal,
            &Position,
            &mut WildlifeAiState,
            &mut Health,
        ),
        (Without<Dead>, Without<crate::components::wildlife::Carcass>),
    >,
    mut cats: Query<
        (Entity, &Position, &mut Health, &mut Needs, &mut Mood, &Name),
        (Without<WildAnimal>, Without<Dead>),
    >,
    wards: Query<(&Ward, &Position), Without<WildAnimal>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    time: Res<TimeState>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.wildlife;
    let ward_multiplier = constants.magic.shadow_fox_ward_repel_multiplier;

    // Snapshot ward positions (non-inverted, alive).
    let ward_positions: Vec<(Position, f32)> = wards
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius() * ward_multiplier))
        .collect();

    // Snapshot cat positions for stalking target selection.
    let cat_positions: Vec<(Entity, Position)> =
        cats.iter().map(|(e, p, _, _, _, _)| (e, *p)).collect();

    for (mut animal, wl_pos, mut ai_state, _health) in &mut wildlife {
        // Only shadow foxes stalk via this system. Regular foxes use fox_ai_decision.
        if animal.species != WildSpecies::ShadowFox {
            continue;
        }

        // Tick down ambush cooldown.
        if animal.ambush_cooldown > 0 {
            animal.ambush_cooldown -= 1;
        }

        // --- Ward avoidance: shadow foxes absolutely avoid wards ---
        let in_ward = ward_positions
            .iter()
            .any(|(wp, radius)| (wl_pos.manhattan_distance(wp) as f32) <= *radius);
        if in_ward {
            // Flee away from nearest ward.
            if let Some((ward_pos, _)) = ward_positions
                .iter()
                .min_by_key(|(wp, _)| wl_pos.manhattan_distance(wp))
            {
                let away_dx = (wl_pos.x - ward_pos.x).signum();
                let away_dy = (wl_pos.y - ward_pos.y).signum();
                let dx = if away_dx != 0 { away_dx } else { 1 };
                let dy = if away_dy != 0 { away_dy } else { 0 };
                *ai_state = WildlifeAiState::Patrolling { dx, dy };
                activation.record(Feature::ShadowFoxAvoidedWard);
                continue;
            }
        }

        match *ai_state {
            WildlifeAiState::Patrolling { .. } | WildlifeAiState::Circling { .. } => {
                // Don't initiate new stalks during post-ambush cooldown.
                if animal.ambush_cooldown > 0 {
                    continue;
                }

                // Find nearest cat within detection range, not inside a
                // ward. Phase 5a: shadow-fox sight channel with LoS.
                let nearest = cat_positions
                    .iter()
                    .filter(|(_, cp)| {
                        crate::systems::sensing::observer_sees_at_with_los(
                            crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
                            *wl_pos,
                            &constants.sensory.shadow_fox,
                            *cp,
                            crate::components::SensorySignature::CAT,
                            c.base_detection_range as f32,
                            &map,
                        )
                    })
                    .filter(|(_, cp)| {
                        !ward_positions
                            .iter()
                            .any(|(wp, radius)| (cp.manhattan_distance(wp) as f32) <= *radius)
                    })
                    .min_by_key(|(_, cp)| wl_pos.manhattan_distance(cp));

                if let Some((_, cat_pos)) = nearest {
                    // 5% chance per tick to begin stalking.
                    if rng.rng.random::<f32>() < 0.05 {
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x,
                            target_y: cat_pos.y,
                        };
                    }
                }
            }
            WildlifeAiState::Stalking { target_x, target_y } => {
                // Cancel stalk if target is inside a ward's radius.
                let target_pos = Position::new(target_x, target_y);
                let target_warded = ward_positions
                    .iter()
                    .any(|(wp, radius)| (target_pos.manhattan_distance(wp) as f32) <= *radius);
                if target_warded {
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                    activation.record(Feature::ShadowFoxAvoidedWard);
                    continue;
                }

                let dist = (wl_pos.x - target_x).abs() + (wl_pos.y - target_y).abs();

                if dist <= 1 {
                    // Ambush! Find the nearest cat at the target position.
                    let target_pos = Position::new(target_x, target_y);
                    if let Some((cat_entity, _)) = cat_positions
                        .iter()
                        .filter(|(_, cp)| cp.manhattan_distance(&target_pos) <= 1)
                        .min_by_key(|(_, cp)| wl_pos.manhattan_distance(cp))
                    {
                        if let Ok((_, _, mut cat_health, mut needs, mut mood, name)) =
                            cats.get_mut(*cat_entity)
                        {
                            let tile_corruption = if map.in_bounds(wl_pos.x, wl_pos.y) {
                                map.get(wl_pos.x, wl_pos.y).corruption
                            } else {
                                0.0
                            };
                            let damage = animal.threat_power
                                * (1.0 + tile_corruption * c.corruption_threat_multiplier);
                            cat_health.current = (cat_health.current - damage).max(0.0);
                            crate::systems::combat::apply_injury(
                                &mut cat_health,
                                damage,
                                time.tick,
                                crate::components::physical::InjurySource::ShadowFoxAmbush,
                                &constants.combat,
                            );
                            needs.safety = (needs.safety - c.threat_safety_drain).max(0.0);

                            let species_name = match animal.species {
                                WildSpecies::Fox => "fox",
                                WildSpecies::ShadowFox => "shadow-fox",
                                _ => "predator",
                            };
                            log.push(
                                time.tick,
                                format!(
                                    "A {species_name} lunges at {} from the undergrowth!",
                                    name.0
                                ),
                                NarrativeTier::Danger,
                            );
                            if let Some(ref mut elog) = event_log {
                                elog.push(
                                    time.tick,
                                    crate::resources::event_log::EventKind::Ambush {
                                        cat: name.0.clone(),
                                        predator_species: format!("{:?}", animal.species),
                                        location: (wl_pos.x, wl_pos.y),
                                        damage,
                                    },
                                );
                            }

                            mood.modifiers
                                .push_back(crate::components::mental::MoodModifier {
                                    amount: c.threat_mood_penalty,
                                    ticks_remaining: c.threat_mood_ticks,
                                    source: "ambushed by predator".to_string(),
                                });
                        }

                        // Nearby cats witness the ambush — drain their safety.
                        for (witness_entity, witness_pos) in &cat_positions {
                            if *witness_entity == *cat_entity {
                                continue;
                            }
                            if wl_pos.manhattan_distance(witness_pos) <= c.ambush_witness_range {
                                if let Ok((_, _, _, mut w_needs, mut w_mood, _)) =
                                    cats.get_mut(*witness_entity)
                                {
                                    w_needs.safety =
                                        (w_needs.safety - c.ambush_witness_safety_drain).max(0.0);
                                    w_mood.modifiers.push_back(
                                        crate::components::mental::MoodModifier {
                                            amount: c.threat_mood_penalty * 0.5,
                                            ticks_remaining: c.threat_mood_ticks,
                                            source: "witnessed predator attack".to_string(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    // After ambush, revert to patrolling with cooldown before next stalk.
                    animal.ambush_cooldown = c.ambush_cooldown_ticks;
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                } else if dist > c.base_detection_range * 2 {
                    // Target moved too far, give up.
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                } else {
                    // Update target to nearest cat's current position.
                    if let Some((_, cat_pos)) = cat_positions
                        .iter()
                        .min_by_key(|(_, cp)| wl_pos.manhattan_distance(cp))
                    {
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x,
                            target_y: cat_pos.y,
                        };
                    }
                }
            }
            _ => {}
        }
    }
}

/// Despawn wildlife that has moved off-map (fleeing) or has 0 health.
pub fn cleanup_wildlife(
    query: Query<(Entity, &Position, &Health, &WildAnimal), With<WildAnimal>>,
    map: Res<TileMap>,
    mut commands: Commands,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
) {
    for (entity, pos, health, animal) in &query {
        let off_map = !map.in_bounds(pos.x, pos.y);
        let dead = health.current <= 0.0;

        if off_map || dead {
            let text = match animal.species {
                WildSpecies::Fox => "A fox retreats into the wilderness.",
                WildSpecies::Hawk => "A hawk glides away over the treetops.",
                WildSpecies::Snake => "A snake disappears into the undergrowth.",
                WildSpecies::ShadowFox => "A shadow-fox dissolves into the dark.",
            };
            log.push(time.tick, text.to_string(), NarrativeTier::Nature);
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Initial wildlife spawning (called from world gen)
// ---------------------------------------------------------------------------

/// Spawn initial wildlife far from the colony center.
pub fn spawn_initial_wildlife(world: &mut World, colony_center: Position) {
    let mut spawn_positions: Vec<(WildSpecies, Position, WildlifeAiState)> = Vec::new();

    // Extract map dimensions and terrain data we need, then borrow rng separately.
    let map_width = world.resource::<TileMap>().width;
    let map_height = world.resource::<TileMap>().height;

    // Snapshot wildlife constants before mutable borrows.
    let wc = world.resource::<SimConstants>().wildlife.clone();

    // Build a lightweight terrain snapshot for spawn searches.
    let terrain_snapshot: Vec<(i32, i32, Terrain)> = {
        let map = world.resource::<TileMap>();
        let mut tiles = Vec::new();
        for y in 0..map.height {
            for x in 0..map.width {
                tiles.push((x, y, map.get(x, y).terrain));
            }
        }
        tiles
    };

    {
        let rng = &mut world.resource_mut::<SimRng>().rng;

        let find_spawn = |min_dist: i32,
                          species: WildSpecies,
                          rng: &mut rand_chacha::ChaCha8Rng|
         -> Option<Position> {
            for _ in 0..200 {
                let x: i32 = rng.random_range(0..map_width);
                let y: i32 = rng.random_range(0..map_height);
                let pos = Position::new(x, y);
                if pos.manhattan_distance(&colony_center) < min_dist {
                    continue;
                }
                let terrain = terrain_snapshot[(y * map_width + x) as usize].2;
                if is_spawn_terrain(terrain, species) {
                    return Some(pos);
                }
            }
            None
        };

        let make_ai = |species: WildSpecies,
                       pos: &Position,
                       rng: &mut rand_chacha::ChaCha8Rng|
         -> WildlifeAiState {
            match species.default_behavior() {
                BehaviorType::Patrol => {
                    let dx = if pos.x == 0 {
                        1
                    } else if pos.x == map_width - 1 {
                        -1
                    } else if rng.random() {
                        1
                    } else {
                        -1
                    };
                    let dy = if pos.y == 0 {
                        1
                    } else if pos.y == map_height - 1 {
                        -1
                    } else {
                        0
                    };
                    WildlifeAiState::Patrolling { dx, dy }
                }
                BehaviorType::Circle => {
                    let center_x =
                        (pos.x + (map_width / 2 - pos.x).signum() * 8).clamp(0, map_width - 1);
                    let center_y =
                        (pos.y + (map_height / 2 - pos.y).signum() * 8).clamp(0, map_height - 1);
                    WildlifeAiState::Circling {
                        center_x,
                        center_y,
                        angle: rng.random_range(0.0..std::f32::consts::TAU),
                    }
                }
                BehaviorType::Ambush => WildlifeAiState::Waiting,
            }
        };

        // Foxes are now spawned via fox dens — see spawn_initial_fox_dens below.

        // Hawks at grass tiles.
        let hawk_count: u32 =
            rng.random_range(wc.initial_hawk_count_min..=wc.initial_hawk_count_max);
        for _ in 0..hawk_count {
            if let Some(pos) = find_spawn(wc.initial_hawk_min_distance, WildSpecies::Hawk, rng) {
                let ai = make_ai(WildSpecies::Hawk, &pos, rng);
                spawn_positions.push((WildSpecies::Hawk, pos, ai));
            }
        }

        // Snakes at rock/mud tiles.
        let snake_count: u32 =
            rng.random_range(wc.initial_snake_count_min..=wc.initial_snake_count_max);
        for _ in 0..snake_count {
            if let Some(pos) = find_spawn(wc.initial_snake_min_distance, WildSpecies::Snake, rng) {
                let ai = make_ai(WildSpecies::Snake, &pos, rng);
                spawn_positions.push((WildSpecies::Snake, pos, ai));
            }
        }
    }

    // Spawn entities outside the borrow.
    for (species, pos, ai) in spawn_positions {
        world.spawn((
            WildAnimal::new(species),
            pos,
            Health::default(),
            ai,
            crate::components::SensorySpecies::Wild(species),
            crate::components::SensorySignature::WILDLIFE,
        ));
    }
}

// ===========================================================================
// Fox ecology systems
// ===========================================================================

// ---------------------------------------------------------------------------
// spawn_initial_fox_dens — called from world gen after spawn_initial_wildlife
// ---------------------------------------------------------------------------

/// Place 1–2 fox dens in DenseForest far from the colony, each with a mated adult pair.
pub fn spawn_initial_fox_dens(world: &mut World, colony_center: Position) {
    let map_width = world.resource::<TileMap>().width;
    let map_height = world.resource::<TileMap>().height;
    let fc = world.resource::<SimConstants>().fox_ecology.clone();
    let tick = world.resource::<TimeState>().tick;

    // Build terrain snapshot for spawn searches.
    let terrain_snapshot: Vec<Terrain> = {
        let map = world.resource::<TileMap>();
        let mut tiles = Vec::with_capacity((map.width * map.height) as usize);
        for y in 0..map.height {
            for x in 0..map.width {
                tiles.push(map.get(x, y).terrain);
            }
        }
        tiles
    };

    let den_count: u32;
    let mut den_positions: Vec<Position> = Vec::new();

    {
        let rng = &mut world.resource_mut::<SimRng>().rng;
        den_count = rng.random_range(fc.initial_den_count_min..=fc.initial_den_count_max);

        for _ in 0..den_count {
            // Try to find a suitable forest tile far from colony and other dens.
            let mut found = None;
            for _ in 0..300 {
                let x: i32 = rng.random_range(0..map_width);
                let y: i32 = rng.random_range(0..map_height);
                let pos = Position::new(x, y);

                if pos.manhattan_distance(&colony_center) < fc.initial_den_min_distance {
                    continue;
                }

                let terrain = terrain_snapshot[(y * map_width + x) as usize];
                if !matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
                    continue;
                }

                // Check spacing from other dens.
                let too_close = den_positions
                    .iter()
                    .any(|dp| pos.manhattan_distance(dp) < fc.min_den_spacing);
                if too_close {
                    continue;
                }

                found = Some(pos);
                break;
            }

            if let Some(pos) = found {
                den_positions.push(pos);
            }
        }
    }

    // Spawn den entities and mated pairs.
    for den_pos in den_positions {
        let den_entity = world
            .spawn((FoxDen::new(fc.territory_radius, tick), den_pos))
            .id();

        // Spawn mated pair at the den.
        let dx_m: i32;
        let dy_m: i32;
        {
            let rng = &mut world.resource_mut::<SimRng>().rng;
            dx_m = if rng.random() { 1 } else { -1 };
            dy_m = 0;
        }

        let male_personality = {
            let rng = &mut world.resource_mut::<SimRng>().rng;
            crate::components::fox_personality::FoxPersonality::random(rng)
        };
        let male_entity = world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                den_pos,
                Health::default(),
                WildlifeAiState::Patrolling { dx: dx_m, dy: dy_m },
                FoxState::new_adult(FoxSex::Male, Some(den_entity)),
                FoxAiPhase::PatrolTerritory { dx: dx_m, dy: dy_m },
                crate::components::fox_personality::FoxNeeds::default(),
                male_personality,
                crate::components::fox_spatial::FoxHuntingBeliefs::default_map(),
                crate::components::fox_spatial::FoxThreatMemory::default_map(),
                crate::components::fox_spatial::FoxExplorationMap::default_map(),
                crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                crate::components::SensorySignature::WILDLIFE,
            ))
            .id();

        let female_personality = {
            let rng = &mut world.resource_mut::<SimRng>().rng;
            crate::components::fox_personality::FoxPersonality::random(rng)
        };
        let female_entity = world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                den_pos,
                Health::default(),
                WildlifeAiState::Patrolling {
                    dx: -dx_m,
                    dy: dy_m,
                },
                FoxState::new_adult(FoxSex::Female, Some(den_entity)),
                FoxAiPhase::DenGuarding,
                crate::components::fox_personality::FoxNeeds::default(),
                female_personality,
                crate::components::fox_spatial::FoxHuntingBeliefs::default_map(),
                crate::components::fox_spatial::FoxThreatMemory::default_map(),
                crate::components::fox_spatial::FoxExplorationMap::default_map(),
                crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                crate::components::SensorySignature::WILDLIFE,
            ))
            .id();

        // Cross-link mates.
        if let Some(mut male_state) = world.get_mut::<FoxState>(male_entity) {
            male_state.mate = Some(female_entity);
        }
        if let Some(mut female_state) = world.get_mut::<FoxState>(female_entity) {
            female_state.mate = Some(male_entity);
        }
    }
}

// ---------------------------------------------------------------------------
// fox_needs_tick — decay hunger, update boldness, advance age
// ---------------------------------------------------------------------------

/// Per-tick fox state maintenance: hunger decay, satiation countdown, boldness
/// update, and age tracking.
pub fn fox_needs_tick(
    mut foxes: Query<&mut FoxState>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let fc = &constants.fox_ecology;
    let hunger_per_tick = fc.hunger_decay_rate.per_tick(&time_scale);
    for mut fox in &mut foxes {
        // Age.
        fox.age_ticks += 1;

        // Hunger: decay toward 1.0 unless satiated.
        if fox.satiation_ticks > 0 {
            fox.satiation_ticks -= 1;
        } else {
            fox.hunger = (fox.hunger + hunger_per_tick).min(1.0);
        }

        // Cooldown.
        fox.post_action_cooldown = fox.post_action_cooldown.saturating_sub(1);

        // Boldness: nonlinear function of hunger. Foxes are only bold when desperate.
        fox.boldness = fox.hunger.powi(2);
    }
}

// ---------------------------------------------------------------------------
// fox_lifecycle_tick — aging, breeding, mortality
// ---------------------------------------------------------------------------

/// Manage fox life stage transitions, breeding, and mortality.
#[allow(clippy::too_many_arguments)]
pub fn fox_lifecycle_tick(
    mut commands: Commands,
    mut foxes: Query<(Entity, &mut FoxState, &Position, &mut Health)>,
    mut dens: Query<(Entity, &mut FoxDen, &Position)>,
    time: Res<TimeState>,
    sim_config: Res<SimConfig>,
    time_scale: Res<TimeScale>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    mut activation: ResMut<SystemActivation>,
) {
    let fc = &constants.fox_ecology;
    let cub_duration_ticks = fc.cub_duration.ticks(&time_scale);
    let juvenile_duration_ticks = fc.juvenile_duration.ticks(&time_scale);
    let max_age_ticks = fc.max_age.ticks(&time_scale);
    let starvation_death_ticks = fc.starvation_death_duration.ticks(&time_scale);

    // Collect fox data to avoid borrow conflicts.
    let fox_snapshot: Vec<(Entity, FoxState, Position)> = foxes
        .iter()
        .map(|(e, fs, p, _)| (e, fs.clone(), *p))
        .collect();

    for (entity, fox_state, _fox_pos) in &fox_snapshot {
        // --- Life stage advancement ---
        let new_stage = match fox_state.life_stage {
            FoxLifeStage::Cub if fox_state.age_ticks >= cub_duration_ticks => {
                Some(FoxLifeStage::Juvenile)
            }
            FoxLifeStage::Juvenile
                if fox_state.age_ticks >= cub_duration_ticks + juvenile_duration_ticks =>
            {
                Some(FoxLifeStage::Adult)
            }
            FoxLifeStage::Adult if fox_state.age_ticks >= max_age_ticks => {
                Some(FoxLifeStage::Elder)
            }
            _ => None,
        };

        if let Some(stage) = new_stage {
            if let Ok((_, mut fs, _, _)) = foxes.get_mut(*entity) {
                let old_stage = fs.life_stage;
                fs.life_stage = stage;

                if let (FoxLifeStage::Cub, FoxLifeStage::Juvenile) = (old_stage, stage) {
                    // Detach from den, begin dispersing.
                    if let Some(den_e) = fs.home_den {
                        if let Ok((_, mut den, _)) = dens.get_mut(den_e) {
                            den.cubs_present = den.cubs_present.saturating_sub(1);
                        }
                    }
                    fs.home_den = None;
                    activation.record(Feature::FoxCubMatured);
                }
            }
        }

        // --- Mortality checks ---
        let should_die = match fox_state.life_stage {
            FoxLifeStage::Juvenile if fox_state.home_den.is_none() => {
                rng.rng.random::<f32>() < fc.juvenile_mortality_per_tick
            }
            FoxLifeStage::Elder => rng.rng.random::<f32>() < fc.elder_mortality_per_tick,
            _ => false,
        };

        // Starvation: sustained max hunger for `starvation_death_ticks` ticks.
        // We need to advance the starvation counter on the live FoxState.
        let (starving, counter_now) = {
            if let Ok((_, mut fs_live, _, _)) = foxes.get_mut(*entity) {
                if fs_live.hunger >= 1.0 {
                    fs_live.starvation_ticks += 1;
                } else {
                    fs_live.starvation_ticks = 0;
                }
                (
                    fs_live.starvation_ticks >= starvation_death_ticks,
                    fs_live.starvation_ticks,
                )
            } else {
                (false, 0)
            }
        };
        let _ = counter_now; // reserved for future telemetry

        if should_die || starving {
            if let Ok((_, _, _, health)) = foxes.get(*entity) {
                if health.current > 0.0 {
                    // Kill the fox.
                    if let Ok((_, _, _, mut health)) = foxes.get_mut(*entity) {
                        health.current = 0.0;
                    }
                    let cause = if starving {
                        "starvation"
                    } else {
                        "the wilderness"
                    };
                    log.push(
                        time.tick,
                        format!("A fox succumbs to {cause}."),
                        NarrativeTier::Nature,
                    );
                    activation.record(Feature::FoxDied);
                }
            }
        }
    }

    // --- Breeding (winter only) ---
    if time.season(&sim_config) != Season::Winter {
        return;
    }

    // Check once per day (tick divisible by ticks_per_day_phase * 4).
    let ticks_per_day = sim_config.ticks_per_day_phase * 4;
    if ticks_per_day == 0 || !time.tick.is_multiple_of(ticks_per_day) {
        return;
    }

    for (den_entity, mut den, den_pos) in &mut dens {
        if den.cubs_present > 0 {
            continue; // Already has cubs this season.
        }

        // Find a female adult at this den with a mate.
        let female = fox_snapshot.iter().find(|(_, fs, _)| {
            fs.home_den == Some(den_entity)
                && fs.sex == FoxSex::Female
                && fs.life_stage == FoxLifeStage::Adult
                && fs.mate.is_some()
        });

        if female.is_none() {
            continue;
        }

        let litter_size = rng
            .rng
            .random_range(fc.litter_size_min..=fc.litter_size_max);

        for _ in 0..litter_size {
            let sex = if rng.rng.random() {
                FoxSex::Male
            } else {
                FoxSex::Female
            };
            let cub_personality =
                crate::components::fox_personality::FoxPersonality::random(&mut rng.rng);
            commands.spawn((
                WildAnimal::new(WildSpecies::Fox),
                *den_pos,
                Health::default(),
                WildlifeAiState::Waiting,
                FoxState::new_cub(sex, den_entity),
                FoxAiPhase::DenGuarding,
                crate::components::fox_personality::FoxNeeds::default(),
                cub_personality,
                crate::components::fox_spatial::FoxHuntingBeliefs::default_map(),
                crate::components::fox_spatial::FoxThreatMemory::default_map(),
                crate::components::fox_spatial::FoxExplorationMap::default_map(),
                crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                crate::components::SensorySignature::WILDLIFE,
            ));
        }

        den.cubs_present = litter_size;
        activation.record(Feature::FoxBred);
        log.push(
            time.tick,
            format!(
                "A fox den stirs with new life \u{2014} {} cubs born.",
                litter_size
            ),
            NarrativeTier::Nature,
        );
    }
}

// ---------------------------------------------------------------------------
// fox_ai_decision — priority-ordered behavior selection
// ---------------------------------------------------------------------------

/// Each tick, evaluate the fox's priority-ordered decision tree and set
/// both `FoxAiPhase` (intent) and `WildlifeAiState` (movement).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn fox_ai_decision(
    mut foxes: Query<
        (
            Entity,
            &mut FoxState,
            &mut FoxAiPhase,
            &mut WildlifeAiState,
            &Position,
            &Health,
        ),
        Without<crate::components::fox_goap_plan::FoxGoapPlan>,
    >,
    cats: Query<(Entity, &Position, &Health), (With<Needs>, Without<Dead>, Without<WildAnimal>)>,
    dens: Query<(Entity, &FoxDen, &Position)>,
    stores: Query<
        &Position,
        (
            With<Structure>,
            Without<ConstructionSite>,
            Without<WildAnimal>,
            Without<Dead>,
        ),
    >,
    prey: Query<(Entity, &Position), With<PreyAnimal>>,
    wards: Query<(&Ward, &Position), Without<WildAnimal>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
    scent_map: Res<FoxScentMap>,
    cat_presence: Res<CatPresenceMap>,
) {
    let fc = &constants.fox_ecology;
    let wc = &constants.wildlife;
    let standoff_max_ticks = fc.standoff_max_duration.ticks(&time_scale);

    // Snapshot cat positions for proximity checks.
    let cat_positions: Vec<(Entity, Position, f32)> = cats
        .iter()
        .map(|(e, p, h)| (e, *p, h.current / h.max))
        .collect();

    // Snapshot active ward positions + repel radii.
    let ward_positions: Vec<(Position, f32)> = wards
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius()))
        .collect();

    for (_fox_entity, mut fox, mut phase, mut ai_state, pos, health) in &mut foxes {
        // --- Cubs stay at den ---
        if fox.life_stage == FoxLifeStage::Cub {
            *phase = FoxAiPhase::DenGuarding;
            *ai_state = WildlifeAiState::Waiting;
            continue;
        }

        // --- Juveniles without a den: disperse ---
        if fox.life_stage == FoxLifeStage::Juvenile && fox.home_den.is_none() {
            if !matches!(*phase, FoxAiPhase::Dispersing { .. }) {
                let dx = if rng.rng.random() { 1 } else { -1 };
                let dy = if rng.rng.random() { 1 } else { -1 };
                *phase = FoxAiPhase::Dispersing { dx, dy };
                *ai_state = WildlifeAiState::Patrolling { dx, dy };
            }

            // Check if juvenile can establish a new den.
            if dens.iter().count() < fc.max_dens {
                let terrain = if map.in_bounds(pos.x, pos.y) {
                    map.get(pos.x, pos.y).terrain
                } else {
                    Terrain::Grass
                };
                let is_forest = matches!(terrain, Terrain::DenseForest | Terrain::LightForest);
                let far_from_dens = dens
                    .iter()
                    .all(|(_, _, dp)| pos.manhattan_distance(dp) >= fc.min_den_spacing);
                let low_scent = scent_map.get(pos.x, pos.y) < 0.1;

                if is_forest && far_from_dens && low_scent {
                    // Small chance per tick to settle.
                    if rng.rng.random::<f32>() < 0.001 {
                        activation.record(Feature::FoxDenEstablished);
                        // Den establishment happens in fox_lifecycle_tick or a dedicated system.
                        // For now, mark the juvenile as settled — the den will be created next tick
                        // by checking for settled juveniles. Actually, let's just create it here
                        // since we don't have commands in this system... we do have the fox entity.
                        // But we can't spawn new entities without Commands. Let's defer den creation
                        // to fox_lifecycle_tick.
                        // For now, stop dispersing and start patrolling this area.
                        fox.life_stage = FoxLifeStage::Adult;
                        let dx = if rng.rng.random() { 1 } else { -1 };
                        *phase = FoxAiPhase::PatrolTerritory { dx, dy: 0 };
                        *ai_state = WildlifeAiState::Patrolling { dx, dy: 0 };
                    }
                }
            }
            continue;
        }

        // --- Don't re-evaluate during active confrontation ---
        if let FoxAiPhase::Confronting {
            ticks_remaining, ..
        } = &*phase
        {
            if *ticks_remaining > 0 {
                continue;
            }
        }

        // --- Don't re-evaluate during active fleeing ---
        if matches!(*phase, FoxAiPhase::Fleeing { .. }) {
            // Check if off-map (cleanup_wildlife handles despawn).
            if !map.in_bounds(pos.x, pos.y) {
                continue;
            }
            // After reaching map edge area, revert to patrol.
            if pos.x <= 1 || pos.x >= map.width - 2 || pos.y <= 1 || pos.y >= map.height - 2 {
                let dx = if rng.rng.random() { 1 } else { -1 };
                *phase = FoxAiPhase::PatrolTerritory { dx, dy: 0 };
                *ai_state = WildlifeAiState::Patrolling { dx, dy: 0 };
            }
            continue;
        }

        // --- Cooldown: skip decisions if recently acted ---
        if fox.post_action_cooldown > 0 {
            // If resting, stay resting.
            if matches!(*phase, FoxAiPhase::Resting { .. }) {
                *ai_state = WildlifeAiState::Waiting;
                continue;
            }
        }

        // --- Health check: flee if badly hurt ---
        let hp_frac = health.current / health.max;
        if hp_frac < fc.flee_health_threshold && hp_frac > 0.0 {
            let flee_dx = if pos.x < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y < map.height / 2 { -1 } else { 1 };
            *phase = FoxAiPhase::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            *ai_state = WildlifeAiState::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            activation.record(Feature::FoxRetreated);
            continue;
        }

        // --- Outnumbered check --- Phase 5a: fox sight with LoS.
        let cats_nearby = cat_positions
            .iter()
            .filter(|(_, cp, _)| {
                crate::systems::sensing::observer_sees_at_with_los(
                    crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                    *pos,
                    &constants.sensory.fox,
                    *cp,
                    crate::components::SensorySignature::CAT,
                    wc.base_detection_range as f32,
                    &map,
                )
            })
            .count();
        if cats_nearby >= fc.outnumbered_flee_count {
            let flee_dx = if pos.x < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y < map.height / 2 { -1 } else { 1 };
            *phase = FoxAiPhase::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            *ai_state = WildlifeAiState::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            activation.record(Feature::FoxRetreated);
            continue;
        }

        // --- Den defense: attack anything near den with cubs ---
        if let Some(den_entity) = fox.home_den {
            if let Ok((_, den, den_pos)) = dens.get(den_entity) {
                if den.cubs_present > 0 {
                    let threat = cat_positions
                        .iter()
                        .find(|(_, cp, _)| den_pos.manhattan_distance(cp) <= fc.den_defense_range);
                    if let Some((cat_e, cat_pos, _)) = threat {
                        *phase = FoxAiPhase::Confronting {
                            target_id: cat_e.to_bits(),
                            ticks_remaining: standoff_max_ticks,
                        };
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x,
                            target_y: cat_pos.y,
                        };
                        activation.record(Feature::FoxDenDefense);
                        activation.record(Feature::FoxStandoff);
                        continue;
                    }
                }
            }
        }

        // --- Desperate: confront vulnerable cats ---
        // Phase 5a: fox sight with LoS.
        if fox.hunger > fc.desperate_hunger_threshold && fox.post_action_cooldown == 0 {
            let vulnerable_cat = cat_positions.iter().find(|(_, cp, hp_frac)| {
                *hp_frac < 0.3
                    && crate::systems::sensing::observer_sees_at_with_los(
                        crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                        *pos,
                        &constants.sensory.fox,
                        *cp,
                        crate::components::SensorySignature::CAT,
                        wc.base_detection_range as f32,
                        &map,
                    )
            });
            if let Some((cat_e, cat_pos, _)) = vulnerable_cat {
                *phase = FoxAiPhase::Confronting {
                    target_id: cat_e.to_bits(),
                    ticks_remaining: standoff_max_ticks,
                };
                *ai_state = WildlifeAiState::Stalking {
                    target_x: cat_pos.x,
                    target_y: cat_pos.y,
                };
                activation.record(Feature::FoxStandoff);
                continue;
            }
        }

        // --- Hungry: raid unguarded stores ---
        // Phase 5a migration: fox scent channel (stores lure via olfaction).
        // Stores have no SensorySignature component, so we construct an
        // ad-hoc CARCASS-like signature in-place (strong scent).
        if fox.hunger > 0.6 && fox.post_action_cooldown == 0 {
            let store_pos = stores.iter().find(|sp| {
                crate::systems::sensing::observer_smells_at(
                    crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                    *pos,
                    &constants.sensory.fox,
                    **sp,
                    crate::components::SensorySignature::CARCASS,
                    fc.raid_smell_range as f32,
                ) && !cat_positions
                    .iter()
                    .any(|(_, cp, _)| sp.manhattan_distance(cp) <= fc.guard_deterrent_range)
            });
            if let Some(sp) = store_pos {
                *phase = FoxAiPhase::Raiding {
                    target_x: sp.x,
                    target_y: sp.y,
                };
                *ai_state = WildlifeAiState::Stalking {
                    target_x: sp.x,
                    target_y: sp.y,
                };
                continue;
            }
        }

        // --- Moderately hungry: hunt prey ---
        // Phase 5a: fox sight with LoS. Extended hunt range (3×
        // predator_hunt_range_fox) passed via max_range_override.
        if fox.hunger > 0.4 && fox.post_action_cooldown == 0 {
            let nearest_prey = prey
                .iter()
                .filter(|(_, pp)| {
                    crate::systems::sensing::observer_sees_at_with_los(
                        crate::components::SensorySpecies::Wild(WildSpecies::Fox),
                        *pos,
                        &constants.sensory.fox,
                        **pp,
                        crate::components::SensorySignature::PREY,
                        (wc.predator_hunt_range_fox * 3) as f32,
                        &map,
                    )
                })
                .min_by_key(|(_, pp)| pos.manhattan_distance(pp));
            if let Some((prey_e, prey_pos)) = nearest_prey {
                *phase = FoxAiPhase::HuntingPrey {
                    target: Some(prey_e.to_bits()),
                };
                *ai_state = WildlifeAiState::Stalking {
                    target_x: prey_pos.x,
                    target_y: prey_pos.y,
                };
                continue;
            }
        }

        // --- Well-fed: rest at den ---
        if fox.hunger < 0.3 && fox.home_den.is_some() {
            if let Some(den_entity) = fox.home_den {
                if let Ok((_, _, den_pos)) = dens.get(den_entity) {
                    let dist = pos.manhattan_distance(den_pos);
                    if dist <= 2 {
                        *phase = FoxAiPhase::Resting { ticks: 500 };
                        *ai_state = WildlifeAiState::Waiting;
                        continue;
                    } else {
                        // Return to den.
                        *phase = FoxAiPhase::Returning {
                            x: den_pos.x,
                            y: den_pos.y,
                        };
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: den_pos.x,
                            target_y: den_pos.y,
                        };
                        continue;
                    }
                }
            }
        }

        // --- Territory patrol: maintain scent marks ---
        if let Some(den_entity) = fox.home_den {
            if let Ok((_, den, den_pos)) = dens.get(den_entity) {
                if den.scent_strength < 0.3 {
                    *phase = FoxAiPhase::ScentMarking;
                    // Move toward territory edge.
                    let edge_x = den_pos.x
                        + if pos.x > den_pos.x {
                            den.territory_radius
                        } else {
                            -den.territory_radius
                        };
                    let edge_y = den_pos.y;
                    *ai_state = WildlifeAiState::Stalking {
                        target_x: edge_x.clamp(0, map.width - 1),
                        target_y: edge_y.clamp(0, map.height - 1),
                    };
                    continue;
                }
            }
        }

        // --- Ward deterrent: move away from wards (soft — ignored when desperate) ---
        if fox.hunger < fc.ward_hunger_override_threshold {
            let nearest_ward = ward_positions
                .iter()
                .filter(|(wp, radius)| (pos.manhattan_distance(wp) as f32) <= *radius)
                .min_by_key(|(wp, _)| pos.manhattan_distance(wp));
            if let Some((ward_pos, _)) = nearest_ward {
                let away_dx = (pos.x - ward_pos.x).signum();
                let away_dy = (pos.y - ward_pos.y).signum();
                let dx = if away_dx != 0 {
                    away_dx
                } else if rng.rng.random() {
                    1
                } else {
                    -1
                };
                let dy = if away_dy != 0 { away_dy } else { 0 };
                *phase = FoxAiPhase::PatrolTerritory { dx, dy };
                *ai_state = WildlifeAiState::Patrolling { dx, dy };
                activation.record(Feature::FoxAvoidedWard);
                continue;
            }
        }

        // --- Cat presence deterrent: avoid high cat-presence zones ---
        if fox.hunger < fc.ward_hunger_override_threshold {
            let presence = cat_presence.get(pos.x, pos.y);
            if presence >= fc.cat_presence_avoidance_threshold {
                // Move toward the lowest-presence adjacent bucket.
                let bs = cat_presence.bucket_size;
                let mut best_dx: i32 = 0;
                let mut best_dy: i32 = 0;
                let mut best_val = presence;
                for (ddx, ddy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = pos.x + ddx * bs;
                    let ny = pos.y + ddy * bs;
                    let v = cat_presence.get(nx, ny);
                    if v < best_val {
                        best_val = v;
                        best_dx = ddx;
                        best_dy = ddy;
                    }
                }
                // Fallback to random direction if all neighbors are equally saturated.
                if best_dx == 0 && best_dy == 0 {
                    best_dx = if rng.rng.random() { 1 } else { -1 };
                }
                *phase = FoxAiPhase::PatrolTerritory {
                    dx: best_dx,
                    dy: best_dy,
                };
                *ai_state = WildlifeAiState::Patrolling {
                    dx: best_dx,
                    dy: best_dy,
                };
                activation.record(Feature::FoxAvoidedPresence);
                continue;
            }
        }

        // --- Mutual avoidance: move away from nearby cats ---
        let closest_cat = cat_positions
            .iter()
            .filter(|(_, cp, _)| pos.manhattan_distance(cp) <= fc.cat_avoidance_range)
            .min_by_key(|(_, cp, _)| pos.manhattan_distance(cp));
        if let Some((_, cat_pos, _)) = closest_cat {
            // Move in the opposite direction from the cat.
            let away_dx = (pos.x - cat_pos.x).signum();
            let away_dy = (pos.y - cat_pos.y).signum();
            let dx = if away_dx != 0 {
                away_dx
            } else if rng.rng.random() {
                1
            } else {
                -1
            };
            let dy = if away_dy != 0 { away_dy } else { 0 };
            *phase = FoxAiPhase::PatrolTerritory { dx, dy };
            *ai_state = WildlifeAiState::Patrolling { dx, dy };
            activation.record(Feature::FoxAvoidedCat);
            continue;
        }

        // --- Default: patrol territory ---
        if let Some(den_entity) = fox.home_den {
            if let Ok((_, den, den_pos)) = dens.get(den_entity) {
                // 3.16: Cat presence near den contracts effective patrol radius.
                let den_presence = cat_presence.get(den_pos.x, den_pos.y);
                let effective_radius = if den_presence > 0.1 {
                    // Contract by up to 50% based on cat presence intensity.
                    let contraction = (den_presence * 0.5).min(0.5);
                    ((den.territory_radius as f32) * (1.0 - contraction)).max(3.0) as i32
                } else {
                    den.territory_radius
                };

                // If far from effective territory, return.
                let dist = pos.manhattan_distance(den_pos);
                if dist > effective_radius {
                    *phase = FoxAiPhase::Returning {
                        x: den_pos.x,
                        y: den_pos.y,
                    };
                    *ai_state = WildlifeAiState::Stalking {
                        target_x: den_pos.x,
                        target_y: den_pos.y,
                    };
                    continue;
                }

                // 3.15: When hungry, shift patrol toward nearest prey.
                if fox.hunger > 0.4 {
                    let nearest_prey_pos = prey
                        .iter()
                        .filter(|(_, pp)| den_pos.manhattan_distance(pp) <= effective_radius * 2)
                        .min_by_key(|(_, pp)| pos.manhattan_distance(pp))
                        .map(|(_, pp)| *pp);
                    if let Some(prey_pos) = nearest_prey_pos {
                        let dx = (prey_pos.x - pos.x).signum();
                        let dy = (prey_pos.y - pos.y).signum();
                        let dx = if dx != 0 {
                            dx
                        } else if rng.rng.random() {
                            1
                        } else {
                            -1
                        };
                        *phase = FoxAiPhase::PatrolTerritory { dx, dy };
                        *ai_state = WildlifeAiState::Patrolling { dx, dy };
                        continue;
                    }
                }
            }
        }

        // Already patrolling — just continue.
        if !matches!(*phase, FoxAiPhase::PatrolTerritory { .. }) {
            let dx = if rng.rng.random() { 1 } else { -1 };
            *phase = FoxAiPhase::PatrolTerritory { dx, dy: 0 };
            *ai_state = WildlifeAiState::Patrolling { dx, dy: 0 };
        }
    }
}

// ---------------------------------------------------------------------------
// fox_movement — handle fox-specific movement for FoxAiPhase states
// ---------------------------------------------------------------------------

/// Move foxes according to their FoxAiPhase. Runs INSTEAD of wildlife_ai for
/// entities with FoxState (which are excluded from wildlife_ai via Without filter).
pub fn fox_movement(
    mut foxes: Query<(&FoxAiPhase, &mut Position, &mut WildlifeAiState), With<FoxState>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
) {
    let _c = &constants.wildlife;
    for (phase, mut pos, mut ai_state) in &mut foxes {
        match phase {
            FoxAiPhase::Resting { .. } | FoxAiPhase::DenGuarding => {
                // Don't move.
            }
            FoxAiPhase::PatrolTerritory { dx, dy } | FoxAiPhase::Dispersing { dx, dy } => {
                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y)
                    && is_patrol_terrain(map.get(next.x, next.y).terrain, WildSpecies::Fox)
                {
                    *pos = next;
                } else {
                    // Reverse and try.
                    let rev = Position::new(pos.x - dx, pos.y - dy);
                    if map.in_bounds(rev.x, rev.y) {
                        *pos = rev;
                        *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                    }
                }
                // Jitter.
                if rng.rng.random::<f32>() < constants.wildlife.patrol_jitter_chance {
                    let new_dx = rng.rng.random_range(-1i32..=1);
                    let new_dy = rng.rng.random_range(-1i32..=1);
                    if new_dx != 0 || new_dy != 0 {
                        *ai_state = WildlifeAiState::Patrolling {
                            dx: new_dx,
                            dy: new_dy,
                        };
                    }
                }
            }
            FoxAiPhase::HuntingPrey { .. }
            | FoxAiPhase::Returning { .. }
            | FoxAiPhase::Raiding { .. }
            | FoxAiPhase::ScentMarking
            | FoxAiPhase::Confronting { .. } => {
                // These all use WildlifeAiState::Stalking for movement.
                // The stalking movement is: move one step toward target.
                if let WildlifeAiState::Stalking { target_x, target_y } = *ai_state {
                    let dx = (target_x - pos.x).signum();
                    let dy = (target_y - pos.y).signum();
                    let next = Position::new(pos.x + dx, pos.y + dy);
                    if map.in_bounds(next.x, next.y)
                        && map.get(next.x, next.y).terrain.is_wildlife_passable()
                    {
                        *pos = next;
                    }
                }
            }
            FoxAiPhase::Fleeing { dx, dy } => {
                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y) {
                    *pos = next;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// fox_confrontation_tick — resolve standoffs
// ---------------------------------------------------------------------------

/// Tick down active fox confrontations. May escalate to minor damage or end
/// with one party retreating.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn fox_confrontation_tick(
    mut foxes: Query<
        (
            Entity,
            &mut FoxState,
            &mut FoxAiPhase,
            &mut WildlifeAiState,
            &Position,
            &mut Health,
        ),
        (
            With<WildAnimal>,
            Without<crate::components::wildlife::ActiveConfrontation>,
        ),
    >,
    mut cats: Query<
        (&Position, &mut Health, &mut Mood, &Name),
        (Without<WildAnimal>, Without<Dead>),
    >,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
    map: Res<TileMap>,
) {
    let fc = &constants.fox_ecology;
    let post_action_cooldown_ticks = fc.post_action_cooldown.ticks(&time_scale);

    for (_fox_entity, mut fox, mut phase, mut ai_state, pos, mut fox_health) in &mut foxes {
        let (target_id, ticks_remaining) = match &mut *phase {
            FoxAiPhase::Confronting {
                target_id,
                ticks_remaining,
            } => (*target_id, ticks_remaining),
            _ => continue,
        };

        if *ticks_remaining == 0 {
            // Standoff expired — fox retreats.
            if rng.rng.random::<f32>() < fc.standoff_fox_retreat_chance {
                let flee_dx = if pos.x < map.width / 2 { -1 } else { 1 };
                let flee_dy = if pos.y < map.height / 2 { -1 } else { 1 };
                *phase = FoxAiPhase::Fleeing {
                    dx: flee_dx,
                    dy: flee_dy,
                };
                *ai_state = WildlifeAiState::Fleeing {
                    dx: flee_dx,
                    dy: flee_dy,
                };
                fox.post_action_cooldown = post_action_cooldown_ticks;
                activation.record(Feature::FoxRetreated);
                log.push(
                    time.tick,
                    "A fox thinks better of it and slinks away.".to_string(),
                    NarrativeTier::Action,
                );
            } else {
                // Fox holds ground, cat retreats (handled by cat AI).
                let dx = if rng.rng.random() { 1 } else { -1 };
                *phase = FoxAiPhase::PatrolTerritory { dx, dy: 0 };
                *ai_state = WildlifeAiState::Patrolling { dx, dy: 0 };
                fox.post_action_cooldown = post_action_cooldown_ticks;
                log.push(
                    time.tick,
                    "The fox stands its ground, hackles raised.".to_string(),
                    NarrativeTier::Danger,
                );
            }
            continue;
        }

        *ticks_remaining -= 1;

        // Determine escalation chance based on context.
        // NOTE: pre-GOAP fox_ai_decision initiates confrontations via one of two
        // paths — den defense (cubs at den + cat nearby) and desperate attack
        // (starving fox + vulnerable cat). We can't distinguish here because the
        // FoxAiPhase::Confronting variant doesn't carry the reason. Approximate:
        // treat it as den defense ONLY when the fox actually has cubs present,
        // not merely when it has a home_den. This avoids inflating escalation
        // for every territorial fox.
        let has_cubs_at_den = false; // conservative default
        let esc_chance = if has_cubs_at_den {
            fc.den_defense_escalation_chance
        } else {
            fc.standoff_escalation_chance
        };

        if rng.rng.random::<f32>() < esc_chance {
            // Escalation! Minor damage to both parties.
            fox_health.current = (fox_health.current - fc.standoff_damage_on_escalation).max(0.0);

            // Try to find the target cat and damage it.
            let target_entity = Entity::from_bits(target_id);
            if let Ok((_, mut cat_health, mut mood, name)) = cats.get_mut(target_entity) {
                cat_health.current =
                    (cat_health.current - fc.standoff_damage_on_escalation).max(0.0);
                crate::systems::combat::apply_injury(
                    &mut cat_health,
                    fc.standoff_damage_on_escalation,
                    time.tick,
                    crate::components::physical::InjurySource::FoxConfrontation,
                    &constants.combat,
                );
                mood.modifiers.push_back(MoodModifier {
                    amount: constants.wildlife.threat_mood_penalty,
                    ticks_remaining: constants.wildlife.threat_mood_ticks,
                    source: "fox fight".to_string(),
                });
                log.push(
                    time.tick,
                    format!(
                        "Claws flash between {} and a fox \u{2014} both draw blood!",
                        name.0
                    ),
                    NarrativeTier::Danger,
                );
            }

            activation.record(Feature::FoxStandoffEscalated);

            // After escalation, fox retreats.
            let flee_dx = if pos.x < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y < map.height / 2 { -1 } else { 1 };
            *phase = FoxAiPhase::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            *ai_state = WildlifeAiState::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            fox.post_action_cooldown = post_action_cooldown_ticks;
        }
    }
}

// ---------------------------------------------------------------------------
// fox_store_raid_tick — foxes steal from unguarded stores
// ---------------------------------------------------------------------------

/// Foxes in the Raiding phase approach stores and steal food if unguarded.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn fox_store_raid_tick(
    mut foxes: Query<(
        &mut FoxState,
        &mut FoxAiPhase,
        &mut WildlifeAiState,
        &Position,
    )>,
    cats: Query<&Position, (With<Needs>, Without<Dead>, Without<WildAnimal>)>,
    mut food: ResMut<FoodStores>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
    map: Res<TileMap>,
) {
    let fc = &constants.fox_ecology;
    let post_action_cooldown_ticks = fc.post_action_cooldown.ticks(&time_scale);
    let satiation_after_store_raid = fc.satiation_after_store_raid.ticks(&time_scale);

    let cat_positions: Vec<Position> = cats.iter().copied().collect();

    for (mut fox, mut phase, mut ai_state, pos) in &mut foxes {
        let (target_x, target_y) = match *phase {
            FoxAiPhase::Raiding { target_x, target_y } => (target_x, target_y),
            _ => continue,
        };

        let dist = (pos.x - target_x).abs() + (pos.y - target_y).abs();

        // Check if a cat appeared near the stores — abort if so.
        let guarded = cat_positions.iter().any(|cp| {
            let store_pos = Position::new(target_x, target_y);
            cp.manhattan_distance(&store_pos) <= fc.guard_deterrent_range
        });

        if guarded {
            // Abort raid — flee.
            let flee_dx = if pos.x < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y < map.height / 2 { -1 } else { 1 };
            *phase = FoxAiPhase::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            *ai_state = WildlifeAiState::Fleeing {
                dx: flee_dx,
                dy: flee_dy,
            };
            fox.post_action_cooldown = post_action_cooldown_ticks;
            activation.record(Feature::FoxRetreated);
            continue;
        }

        if dist <= 1 && !food.is_empty() {
            // Steal food!
            let stolen = food.withdraw(fc.raid_food_stolen);
            if stolen > 0.0 {
                fox.satiation_ticks = satiation_after_store_raid;
                fox.hunger = (fox.hunger - 0.4).max(0.0);
                fox.post_action_cooldown = post_action_cooldown_ticks;
                activation.record(Feature::FoxStoreRaided);
                log.push(
                    time.tick,
                    format!("A fox raids the colony stores, making off with {stolen:.1} food!"),
                    NarrativeTier::Danger,
                );
            }

            // After raiding, return to den or patrol.
            let dx = if rng_dx(pos.x, map.width) { 1 } else { -1 };
            *phase = FoxAiPhase::PatrolTerritory { dx, dy: 0 };
            *ai_state = WildlifeAiState::Patrolling { dx, dy: 0 };
        }
    }
}

/// Helper: pick a direction based on position relative to center.
fn rng_dx(x: i32, width: i32) -> bool {
    x < width / 2
}

// ---------------------------------------------------------------------------
// fox_scent_tick — deposit and decay territorial scent
// ---------------------------------------------------------------------------

/// Foxes deposit scent during patrol/marking phases. All scent decays globally.
pub fn fox_scent_tick(
    foxes: Query<(&FoxState, &FoxAiPhase, &Position)>,
    mut scent_map: ResMut<FoxScentMap>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
) {
    let fc = &constants.fox_ecology;

    // Global decay (territorial mark, ~10 in-game days at default scale).
    scent_map.decay_all(fc.scent_decay_rate.per_tick(&time_scale));

    // Fox deposits.
    for (fox, phase, pos) in &foxes {
        if fox.life_stage == FoxLifeStage::Cub {
            continue;
        }
        match phase {
            FoxAiPhase::ScentMarking | FoxAiPhase::PatrolTerritory { .. } => {
                scent_map.deposit(pos.x, pos.y, fc.scent_deposit);
                if matches!(phase, FoxAiPhase::ScentMarking) {
                    activation.record(Feature::FoxScentMarked);
                }
            }
            _ => {}
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

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        let mut map = TileMap::new(40, 30, Terrain::Grass);
        // Add some forest for foxes.
        for x in 0..10 {
            map.set(x, 0, Terrain::DenseForest);
            map.set(x, 29, Terrain::DenseForest);
        }
        // Add rock for snakes.
        for x in 30..40 {
            map.set(x, 0, Terrain::Rock);
            map.set(x, 29, Terrain::Rock);
        }
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(wildlife_ai);
        (world, schedule)
    }

    fn spawn_animal(
        world: &mut World,
        species: WildSpecies,
        pos: Position,
        ai_state: WildlifeAiState,
    ) -> Entity {
        world
            .spawn((WildAnimal::new(species), pos, Health::default(), ai_state))
            .id()
    }

    #[test]
    fn fox_patrols_along_forest() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::Fox,
            Position::new(5, 15),
            WildlifeAiState::Patrolling { dx: 1, dy: 0 },
        );

        schedule.run(&mut world);

        let pos = *world.get::<Position>(entity).unwrap();
        // Fox should have moved (either forward or jittered).
        assert!(
            pos.x != 5 || pos.y != 15,
            "fox should have moved from (5, 15)"
        );
    }

    #[test]
    fn snake_stays_still() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::Snake,
            Position::new(10, 10),
            WildlifeAiState::Waiting,
        );

        schedule.run(&mut world);

        let pos = *world.get::<Position>(entity).unwrap();
        assert_eq!(pos, Position::new(10, 10), "snake should not move");
    }

    #[test]
    fn hawk_circles_and_moves() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::Hawk,
            Position::new(20, 15),
            WildlifeAiState::Circling {
                center_x: 20,
                center_y: 15,
                angle: 0.0,
            },
        );

        schedule.run(&mut world);

        let pos = *world.get::<Position>(entity).unwrap();
        // Hawk should have moved from start (circling).
        assert!(
            pos.x != 20 || pos.y != 15,
            "hawk should have moved from (20, 15)"
        );
    }

    #[test]
    fn spawn_wildlife_respects_population_cap() {
        let mut world = World::new();
        let map = TileMap::new(40, 30, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(TimeState::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(DetectionCooldowns::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());

        // Spawn max foxes.
        for i in 0..WildSpecies::Fox.population_cap() {
            world.spawn((
                WildAnimal::new(WildSpecies::Fox),
                Position::new(i as i32, 0),
                Health::default(),
                WildlifeAiState::Patrolling { dx: 1, dy: 0 },
            ));
        }

        let fox_count_before = world
            .query::<&WildAnimal>()
            .iter(&world)
            .filter(|a| a.species == WildSpecies::Fox)
            .count();
        assert_eq!(fox_count_before, WildSpecies::Fox.population_cap());

        // Run spawn system many times — should not add more foxes.
        let mut schedule = Schedule::default();
        schedule.add_systems(spawn_wildlife);
        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let fox_count_after = world
            .query::<&WildAnimal>()
            .iter(&world)
            .filter(|a| a.species == WildSpecies::Fox)
            .count();
        assert_eq!(
            fox_count_after,
            WildSpecies::Fox.population_cap(),
            "should not exceed population cap"
        );
    }

    #[test]
    fn initial_wildlife_spawns_far_from_colony() {
        let mut world = World::new();
        let map = TileMap::new(80, 60, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());

        let colony = Position::new(40, 30);
        world.insert_resource(crate::resources::time::TimeState::default());
        spawn_initial_wildlife(&mut world, colony);
        spawn_initial_fox_dens(&mut world, colony);

        let positions: Vec<Position> = world
            .query_filtered::<&Position, With<WildAnimal>>()
            .iter(&world)
            .copied()
            .collect();

        assert!(!positions.is_empty(), "should spawn at least some wildlife");
        for pos in &positions {
            assert!(
                pos.manhattan_distance(&colony) >= 7,
                "wildlife at ({}, {}) is too close to colony at ({}, {})",
                pos.x,
                pos.y,
                colony.x,
                colony.y
            );
        }
    }
}
