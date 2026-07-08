use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::hawk_scoring::{HawkNeeds, HawkPersonality};
use crate::ai::snake_scoring::{SnakeNeeds, SnakePersonality};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{ConstructionSite, Structure};
use crate::components::identity::Name;
use crate::components::magic::Ward;
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier, MoodSource};
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::{PreyAnimal, PreyConfig};
use crate::components::wildlife::{
    BehaviorType, FoxAiPhase, FoxDen, FoxLifeStage, FoxSex, FoxState, HawkAiPhase, HawkState,
    SnakeAiPhase, SnakeState, WildAnimal, WildSpecies, WildlifeAiState,
};
use crate::resources::cat_scent_map::CatScentMap;
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
            // Ticket 025 Phase 2 — hawks and snakes now spawn with their
            // GOAP runtime state attached. Foxes and ShadowFoxes keep
            // their existing component sets (foxes via dens; ShadowFox
            // via corruption spawn).
            match species {
                WildSpecies::Hawk => {
                    commands.spawn((
                        animal,
                        spawn_pos,
                        Health::default(),
                        ai_state,
                        crate::components::SensorySpecies::Wild(species),
                        crate::components::SensorySignature::WILDLIFE,
                        HawkState::new_adult(),
                        HawkAiPhase::Soaring {
                            center_x: spawn_pos.x(),
                            center_y: spawn_pos.y(),
                            angle: 0.0,
                        },
                        HawkNeeds::default(),
                        HawkPersonality::random(&mut rng.rng),
                    ));
                }
                WildSpecies::Snake => {
                    commands.spawn((
                        animal,
                        spawn_pos,
                        Health::default(),
                        ai_state,
                        crate::components::SensorySpecies::Wild(species),
                        crate::components::SensorySignature::WILDLIFE,
                        SnakeState::new_adult(),
                        SnakeAiPhase::Waiting,
                        SnakeNeeds::default(),
                        SnakePersonality::random(&mut rng.rng),
                    ));
                }
                _ => {
                    commands.spawn((
                        animal,
                        spawn_pos,
                        Health::default(),
                        ai_state,
                        crate::components::SensorySpecies::Wild(species),
                        crate::components::SensorySignature::WILDLIFE,
                    ));
                }
            }

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
            let dx = if pos.x() == 0 {
                1
            } else if pos.x() == map.width - 1 {
                -1
            } else if rng.random() {
                1
            } else {
                -1
            };
            let dy = if pos.y() == 0 {
                1
            } else if pos.y() == map.height - 1 {
                -1
            } else {
                0
            };
            WildlifeAiState::Patrolling { dx, dy }
        }
        BehaviorType::Circle => {
            // Circle around a point ~8 tiles inward from spawn.
            let center_x =
                (pos.x() + (map.width / 2 - pos.x()).signum() * 8).clamp(0, map.width - 1);
            let center_y =
                (pos.y() + (map.height / 2 - pos.y()).signum() * 8).clamp(0, map.height - 1);
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
            if map.in_bounds(cat_pos.x(), cat_pos.y()) {
                let terrain = map.get(cat_pos.x(), cat_pos.y()).terrain;
                if matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
                    range -= c.forest_range_penalty;
                }
            }
            // Patrolling cats get doubled detection range.
            if current.action == Action::Patrol {
                range *= 2.0;
            }
            // Watchtower doubles detection range for cats standing on one.
            if watchtower_positions
                .iter()
                .any(|wp| cat_pos.chebyshev_distance(wp) == 0)
            {
                range *= 2.0;
            }
            range.max(1.0)
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
                detection_range,
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

            mood.modifiers.push_back(
                MoodModifier::new(
                    c.threat_mood_penalty,
                    c.threat_mood_ticks,
                    format!("{} spotted", species.name()),
                )
                .with_kind(MoodSource::Fear),
            );

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
/// Predators with their GOAP-side state component (`FoxState` /
/// `HawkState` / `SnakeState`) only hunt when their AiPhase indicates
/// active predation; on a successful kill they receive species-specific
/// satiation (ticket 025 Phase 2).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn predator_hunt_prey(
    mut commands: Commands,
    predators: Query<
        (
            Entity,
            &WildAnimal,
            &Position,
            Option<&FoxAiPhase>,
            Option<&HawkAiPhase>,
            Option<&SnakeAiPhase>,
        ),
        Without<PreyAnimal>,
    >,
    prey: Query<(Entity, &PreyConfig, &Position), With<PreyAnimal>>,
    mut fox_states: Query<&mut FoxState>,
    mut hawk_states: Query<&mut HawkState>,
    mut snake_states: Query<&mut SnakeState>,
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
    let hc = &constants.hawk_ecology;
    let snc = &constants.snake_ecology;
    let satiation_prey_kill = fc.satiation_after_prey_kill.ticks(&time_scale);
    let satiation_dive_kill = hc.satiation_after_dive_kill.ticks(&time_scale);
    let satiation_strike_kill = snc.satiation_after_strike_kill.ticks(&time_scale);
    for (pred_entity, predator, pred_pos, fox_phase, hawk_phase, snake_phase) in predators.iter() {
        // Foxes with ecology: only hunt when in HuntingPrey phase.
        if let Some(phase) = fox_phase {
            if !matches!(phase, FoxAiPhase::HuntingPrey { .. }) {
                continue;
            }
        }
        // Hawks with ecology: only hunt when diving.
        if let Some(phase) = hawk_phase {
            if !matches!(phase, HawkAiPhase::HuntingPrey { .. }) {
                continue;
            }
        }
        // Snakes with ecology: only hunt when striking.
        if let Some(phase) = snake_phase {
            if !matches!(phase, SnakeAiPhase::Striking { .. }) {
                continue;
            }
        }

        // Only hunt sometimes.
        if rng.rng.random::<f32>() > c.predator_hunt_chance {
            continue;
        }

        let hunt_range: f32 = match predator.species {
            WildSpecies::Fox => c.predator_hunt_range_fox,
            WildSpecies::Hawk => c.predator_hunt_range_hawk,
            WildSpecies::Snake => c.predator_hunt_range_snake,
            WildSpecies::ShadowFox => c.predator_hunt_range_shadow_fox,
        };
        let predator_profile = constants
            .sensory
            .profile_for(crate::components::SensorySpecies::Wild(predator.species));

        // Find nearest prey in range. Phase 5a: predator sight with LoS.
        let mut nearest: Option<(Entity, f32)> = None;
        for (prey_entity, _prey_animal, prey_pos) in prey.iter() {
            if !crate::systems::sensing::observer_sees_at_with_los(
                crate::components::SensorySpecies::Wild(predator.species),
                *pred_pos,
                predator_profile,
                *prey_pos,
                crate::components::SensorySignature::PREY,
                hunt_range,
                &map,
            ) {
                continue;
            }
            let dist = pred_pos.distance_to(prey_pos);
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
                    // Ticket 025 Phase 2 — hawk/snake satiation parallels
                    // the fox branch. The dive/strike *event* Features
                    // (`HawkDiveLanded` / `SnakeStruckPrey`) fire from
                    // the step resolvers when the predator arrives in
                    // range; this site handles the *kill outcome*. We do
                    // not double-emit those Features here.
                    if let Ok(mut hawk_state) = hawk_states.get_mut(pred_entity) {
                        hawk_state.satiation_ticks = satiation_dive_kill;
                        hawk_state.hunger = (hawk_state.hunger - 0.3).max(0.0);
                    }
                    if let Ok(mut snake_state) = snake_states.get_mut(pred_entity) {
                        snake_state.satiation_ticks = satiation_strike_kill;
                        snake_state.hunger = (snake_state.hunger - 0.4).max(0.0);
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
        if !carcass.cleansed && map.in_bounds(pos.x(), pos.y()) {
            let tile = map.get_mut(pos.x(), pos.y());
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
// carcass_scent_tick — Phase 2C §5.6.3 row #6
// ---------------------------------------------------------------------------

/// Actionable carcasses (`!cleansed || !harvested`) deposit scent onto
/// `CarcassScentMap` each tick; the whole grid decays globally. Mirrors
/// `prey_scent_tick` (`src/systems/prey.rs:541+`) — carcass scent
/// becomes a grid-addressable influence-map read on the substrate
/// scaffolded in Phase 2A.
///
/// **Phase 2C scope:** deposit + decay only. Consumer reads in
/// `goap.rs:1133–1145` still go through per-pair `observer_smells_at`;
/// the cutover is a separate balance-affecting follow-on so the
/// structural landing carries no scoring delta. The map is observable
/// via the focal-cat trace (registered in `trace_emit.rs:120+`).
///
/// The `actionable` filter mirrors `goap.rs:840–846`'s
/// `carcass_positions` snapshot — fully cleansed AND harvested
/// carcasses are de facto inert; once the map cuts over to drive
/// scoring, scoring should not see lingering scent from finished
/// carcasses.
pub fn carcass_scent_tick(
    carcasses: Query<(&crate::components::wildlife::Carcass, &Position)>,
    mut scent_map: ResMut<crate::resources::CarcassScentMap>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let c = &constants.wildlife;
    // Global decay first — prior-tick deposits fade before this
    // tick's stamps land, matching `prey_scent_tick` ordering.
    scent_map.decay_all(c.carcass_scent_decay_rate.per_tick(&time_scale));
    for (carcass, pos) in &carcasses {
        if !carcass.cleansed || !carcass.harvested {
            scent_map.deposit(pos.x(), pos.y(), c.carcass_scent_deposit_per_tick);
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
        let off_map = !map.in_bounds(pos.x(), pos.y());
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

        let find_spawn = |min_dist: f32,
                          species: WildSpecies,
                          rng: &mut rand_chacha::ChaCha8Rng|
         -> Option<Position> {
            for _ in 0..200 {
                let x: i32 = rng.random_range(0..map_width);
                let y: i32 = rng.random_range(0..map_height);
                let pos = Position::new(x, y);
                if pos.distance_to(&colony_center) < min_dist {
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
                    let dx = if pos.x() == 0 {
                        1
                    } else if pos.x() == map_width - 1 {
                        -1
                    } else if rng.random() {
                        1
                    } else {
                        -1
                    };
                    let dy = if pos.y() == 0 {
                        1
                    } else if pos.y() == map_height - 1 {
                        -1
                    } else {
                        0
                    };
                    WildlifeAiState::Patrolling { dx, dy }
                }
                BehaviorType::Circle => {
                    let center_x =
                        (pos.x() + (map_width / 2 - pos.x()).signum() * 8).clamp(0, map_width - 1);
                    let center_y = (pos.y() + (map_height / 2 - pos.y()).signum() * 8)
                        .clamp(0, map_height - 1);
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
    // Ticket 025 Phase 2 — hawks/snakes get their GOAP runtime state
    // attached at world-gen time, matching the edge-spawn path.
    for (species, pos, ai) in spawn_positions {
        match species {
            WildSpecies::Hawk => {
                let personality = {
                    let rng = &mut world.resource_mut::<SimRng>().rng;
                    HawkPersonality::random(rng)
                };
                world.spawn((
                    WildAnimal::new(species),
                    pos,
                    Health::default(),
                    ai,
                    crate::components::SensorySpecies::Wild(species),
                    crate::components::SensorySignature::WILDLIFE,
                    HawkState::new_adult(),
                    HawkAiPhase::Soaring {
                        center_x: pos.x(),
                        center_y: pos.y(),
                        angle: 0.0,
                    },
                    HawkNeeds::default(),
                    personality,
                ));
            }
            WildSpecies::Snake => {
                let personality = {
                    let rng = &mut world.resource_mut::<SimRng>().rng;
                    SnakePersonality::random(rng)
                };
                world.spawn((
                    WildAnimal::new(species),
                    pos,
                    Health::default(),
                    ai,
                    crate::components::SensorySpecies::Wild(species),
                    crate::components::SensorySignature::WILDLIFE,
                    SnakeState::new_adult(),
                    SnakeAiPhase::Waiting,
                    SnakeNeeds::default(),
                    personality,
                ));
            }
            _ => {
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

                if pos.distance_to(&colony_center) < fc.initial_den_min_distance {
                    continue;
                }

                let terrain = terrain_snapshot[(y * map_width + x) as usize];
                if !matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
                    continue;
                }

                // Check spacing from other dens.
                let too_close = den_positions
                    .iter()
                    .any(|dp| pos.distance_to(dp) < fc.min_den_spacing);
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

        // Ticket 050: founder-pair den claim emits DenClaimed so
        // `update_den_marker` can author HasDen event-driven from the
        // very first tick (otherwise the per-tick scan would lag one
        // tick behind the spawn). `Messages::write` is the
        // exclusive-world equivalent of `MessageWriter::write`.
        {
            let mut messages = world.resource_mut::<bevy_ecs::message::Messages<
                crate::messages::fox_lifecycle::DenClaimed,
            >>();
            messages.write(crate::messages::fox_lifecycle::DenClaimed {
                fox: male_entity,
                den: den_entity,
                position: den_pos,
                tick,
            });
            messages.write(crate::messages::fox_lifecycle::DenClaimed {
                fox: female_entity,
                den: den_entity,
                position: den_pos,
                tick,
            });
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
    // Ticket 050 — fox-lifecycle messages drive event-driven §4
    // marker authoring (`fox_spatial::update_den_marker` /
    // `update_cub_marker`).
    mut den_claimed_w: bevy_ecs::message::MessageWriter<crate::messages::fox_lifecycle::DenClaimed>,
    mut den_lost_w: bevy_ecs::message::MessageWriter<crate::messages::fox_lifecycle::DenLost>,
    mut cubs_born_w: bevy_ecs::message::MessageWriter<crate::messages::fox_lifecycle::CubsBorn>,
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
                    let den_e = fs.home_den;
                    if let Some(den_e) = den_e {
                        if let Ok((_, mut den, _)) = dens.get_mut(den_e) {
                            den.cubs_present = den.cubs_present.saturating_sub(1);
                        }
                        // Ticket 050: cub→juvenile is a DenLost emit
                        // site (cub disperses from its birth den).
                        den_lost_w.write(crate::messages::fox_lifecycle::DenLost {
                            fox: *entity,
                            den: den_e,
                            reason: crate::messages::fox_lifecycle::DenLostReason::Maturation,
                            tick: time.tick,
                        });
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
                    if let Ok((_, fs, _, mut health)) = foxes.get_mut(*entity) {
                        health.current = 0.0;
                        // Ticket 050: fox death is a DenLost emit
                        // site when the fox held a home_den. Marker
                        // authors react on the next frame.
                        if let Some(den_e) = fs.home_den {
                            den_lost_w.write(crate::messages::fox_lifecycle::DenLost {
                                fox: *entity,
                                den: den_e,
                                reason: crate::messages::fox_lifecycle::DenLostReason::Death,
                                tick: time.tick,
                            });
                        }
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

        let Some(&(mother_entity, _, _)) = female else {
            continue;
        };

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
            let cub_entity = commands
                .spawn((
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
                ))
                .id();
            // Ticket 050: cub birth claims the cub's home_den. Event
            // drives `update_den_marker` to insert HasDen on the cub
            // without waiting for the next per-tick scan.
            den_claimed_w.write(crate::messages::fox_lifecycle::DenClaimed {
                fox: cub_entity,
                den: den_entity,
                position: *den_pos,
                tick: time.tick,
            });
        }

        den.cubs_present = litter_size;
        activation.record(Feature::FoxBred);
        // Ticket 050: litter spawn is the CubsBorn emit site. Drives
        // `update_cub_marker` to insert HasCubs on the mother.
        cubs_born_w.write(crate::messages::fox_lifecycle::CubsBorn {
            mother: mother_entity,
            den: den_entity,
            count: litter_size,
            position: *den_pos,
            tick: time.tick,
        });
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
    cat_scent: Res<CatScentMap>,
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
                let terrain = if map.in_bounds(pos.x(), pos.y()) {
                    map.get(pos.x(), pos.y()).terrain
                } else {
                    Terrain::Grass
                };
                let is_forest = matches!(terrain, Terrain::DenseForest | Terrain::LightForest);
                let far_from_dens = dens
                    .iter()
                    .all(|(_, _, dp)| pos.distance_to(dp) >= fc.min_den_spacing);
                let low_scent = scent_map.get(pos.x(), pos.y()) < 0.1;

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
            if !map.in_bounds(pos.x(), pos.y()) {
                continue;
            }
            // After reaching map edge area, revert to patrol.
            if pos.x() <= 1 || pos.x() >= map.width - 2 || pos.y() <= 1 || pos.y() >= map.height - 2
            {
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
            let flee_dx = if pos.x() < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y() < map.height / 2 { -1 } else { 1 };
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
                    wc.base_detection_range,
                    &map,
                )
            })
            .count();
        if cats_nearby >= fc.outnumbered_flee_count {
            let flee_dx = if pos.x() < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y() < map.height / 2 { -1 } else { 1 };
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
                        .find(|(_, cp, _)| den_pos.distance_to(cp) <= fc.den_defense_range);
                    if let Some((cat_e, cat_pos, _)) = threat {
                        *phase = FoxAiPhase::Confronting {
                            target_id: cat_e.to_bits(),
                            ticks_remaining: standoff_max_ticks,
                        };
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x(),
                            target_y: cat_pos.y(),
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
                        wc.base_detection_range,
                        &map,
                    )
            });
            if let Some((cat_e, cat_pos, _)) = vulnerable_cat {
                *phase = FoxAiPhase::Confronting {
                    target_id: cat_e.to_bits(),
                    ticks_remaining: standoff_max_ticks,
                };
                *ai_state = WildlifeAiState::Stalking {
                    target_x: cat_pos.x(),
                    target_y: cat_pos.y(),
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
                    fc.raid_smell_range,
                ) && !cat_positions
                    .iter()
                    .any(|(_, cp, _)| sp.distance_to(cp) <= fc.guard_deterrent_range)
            });
            if let Some(sp) = store_pos {
                *phase = FoxAiPhase::Raiding {
                    target_x: sp.x(),
                    target_y: sp.y(),
                };
                *ai_state = WildlifeAiState::Stalking {
                    target_x: sp.x(),
                    target_y: sp.y(),
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
                        wc.predator_hunt_range_fox * 3.0,
                        &map,
                    )
                })
                .min_by_key(|(_, pp)| pos.tile_distance_squared(pp));
            if let Some((prey_e, prey_pos)) = nearest_prey {
                *phase = FoxAiPhase::HuntingPrey {
                    target: Some(prey_e.to_bits()),
                };
                *ai_state = WildlifeAiState::Stalking {
                    target_x: prey_pos.x(),
                    target_y: prey_pos.y(),
                };
                continue;
            }
        }

        // --- Well-fed: rest at den ---
        if fox.hunger < 0.3 && fox.home_den.is_some() {
            if let Some(den_entity) = fox.home_den {
                if let Ok((_, _, den_pos)) = dens.get(den_entity) {
                    let dist = pos.distance_to(den_pos);
                    if dist <= 2.0 {
                        *phase = FoxAiPhase::Resting { ticks: 500 };
                        *ai_state = WildlifeAiState::Waiting;
                        continue;
                    } else {
                        // Return to den.
                        *phase = FoxAiPhase::Returning {
                            x: den_pos.x(),
                            y: den_pos.y(),
                        };
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: den_pos.x(),
                            target_y: den_pos.y(),
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
                    let radius_i = den.territory_radius.round() as i32;
                    let edge_x = den_pos.x()
                        + if pos.x() > den_pos.x() {
                            radius_i
                        } else {
                            -radius_i
                        };
                    let edge_y = den_pos.y();
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
                .filter(|(wp, radius)| (pos.distance_to(wp)) <= *radius)
                .min_by_key(|(wp, _)| pos.tile_distance_squared(wp));
            if let Some((ward_pos, _)) = nearest_ward {
                let away_dx = (pos.x() - ward_pos.x()).signum();
                let away_dy = (pos.y() - ward_pos.y()).signum();
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
            let presence = cat_scent.get(pos.x(), pos.y());
            if presence >= fc.cat_scent_avoidance_threshold {
                // Move toward the lowest-presence adjacent bucket.
                let bs = cat_scent.bucket_size;
                let mut best_dx: i32 = 0;
                let mut best_dy: i32 = 0;
                let mut best_val = presence;
                for (ddx, ddy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = pos.x() + ddx * bs;
                    let ny = pos.y() + ddy * bs;
                    let v = cat_scent.get(nx, ny);
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
            .filter(|(_, cp, _)| pos.distance_to(cp) <= fc.cat_avoidance_range)
            .min_by_key(|(_, cp, _)| pos.tile_distance_squared(cp));
        if let Some((_, cat_pos, _)) = closest_cat {
            // Move in the opposite direction from the cat.
            let away_dx = (pos.x() - cat_pos.x()).signum();
            let away_dy = (pos.y() - cat_pos.y()).signum();
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
                let den_presence = cat_scent.get(den_pos.x(), den_pos.y());
                let effective_radius: f32 = if den_presence > 0.1 {
                    // Contract by up to 50% based on cat scent intensity.
                    let contraction = (den_presence * 0.5).min(0.5);
                    (den.territory_radius * (1.0 - contraction)).max(3.0)
                } else {
                    den.territory_radius
                };

                // If far from effective territory, return.
                let dist = pos.distance_to(den_pos);
                if dist > effective_radius {
                    *phase = FoxAiPhase::Returning {
                        x: den_pos.x(),
                        y: den_pos.y(),
                    };
                    *ai_state = WildlifeAiState::Stalking {
                        target_x: den_pos.x(),
                        target_y: den_pos.y(),
                    };
                    continue;
                }

                // 3.15: When hungry, shift patrol toward nearest prey.
                if fox.hunger > 0.4 {
                    let nearest_prey_pos = prey
                        .iter()
                        .filter(|(_, pp)| den_pos.distance_to(pp) <= effective_radius * 2.0)
                        .min_by_key(|(_, pp)| pos.tile_distance_squared(pp))
                        .map(|(_, pp)| *pp);
                    if let Some(prey_pos) = nearest_prey_pos {
                        let dx = (prey_pos.x() - pos.x()).signum();
                        let dy = (prey_pos.y() - pos.y()).signum();
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
// fox_movement — RETIRED (140 step 9)
// ---------------------------------------------------------------------------
//
// The legacy `fox_movement` system moved foxes per `FoxAiPhase` with
// direct Position writes. Every moving fox phase is authored by the
// fox GOAP dispatcher's phase mirror (`phase_for_action` covers all
// travel-family actions, including juvenile Dispersing plans and the
// hurt-flee FleeArea), so its writes were pure double-driving on top
// of `fox_steps::desire_toward` — and its `Fleeing` arm wrote
// Position with only an in-bounds check, marching injured foxes into
// water. Pre-140 they self-rescued because `step_toward` teleported
// onto the first (always-passable) A* waypoint; the Chain-4
// integrator correctly refuses to cross impassable terrain, which
// turned a water-stranded fox into a starvation death (seed-42
// tuned-42-dc11ac39: both foxes dead in the NW lake by tick 1214400).
// Decision layers write DesiredVelocity; the integrator owns motion.

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
        (
            &Position,
            &mut Health,
            &mut Mood,
            &Name,
            &mut crate::components::CatBodyModel,
            &crate::components::equipment::WearableSlots,
        ),
        (Without<WildAnimal>, Without<Dead>),
    >,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
    map: Res<TileMap>,
    mut body_part_writer: MessageWriter<crate::messages::body_part_injury::BodyPartInjury>,
    // 477 — focal-cat resolver-trace sink for confrontation armor reduction.
    focal_trace: crate::resources::trace_log::FocalTraceParam,
) {
    let fc = &constants.fox_ecology;
    let focal_sink = focal_trace.sink(time.tick);
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
                let flee_dx = if pos.x() < map.width / 2 { -1 } else { 1 };
                let flee_dy = if pos.y() < map.height / 2 { -1 } else { 1 };
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
            if let Ok((_cat_pos, mut cat_health, mut mood, name, mut cat_body_model, wearables)) =
                cats.get_mut(target_entity)
            {
                // 477 — armor reduces confrontation escalation damage.
                let em = crate::components::equipment_effects::equipment_modifiers_for(
                    wearables,
                    &constants.combat,
                );
                let reduced = crate::systems::combat::armor_reduced_damage(
                    fc.standoff_damage_on_escalation,
                    crate::components::physical::InjurySource::FoxConfrontation,
                    crate::components::body_zones::WoundKind::Normal,
                    &em,
                );
                cat_health.current = (cat_health.current - reduced).max(0.0);
                // 095 Phase 1 — anatomical substrate is canonical.
                crate::systems::combat::damage_to_body_part(
                    target_entity,
                    &mut cat_body_model,
                    fc.standoff_damage_on_escalation,
                    time.tick,
                    crate::components::physical::InjurySource::FoxConfrontation,
                    &constants.combat,
                    &mut rng,
                    &mut body_part_writer,
                    &mut activation,
                    Some(&em),
                    focal_sink.as_ref(),
                );
                mood.modifiers.push_back(
                    MoodModifier::new(
                        constants.wildlife.threat_mood_penalty,
                        constants.wildlife.threat_mood_ticks,
                        "fox fight",
                    )
                    .with_kind(MoodSource::Fear),
                );
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
            let flee_dx = if pos.x() < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y() < map.height / 2 { -1 } else { 1 };
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

        let dist = (pos.x() - target_x).abs() + (pos.y() - target_y).abs();

        // Check if a cat appeared near the stores — abort if so.
        let guarded = cat_positions.iter().any(|cp| {
            let store_pos = Position::new(target_x, target_y);
            cp.distance_to(&store_pos) <= fc.guard_deterrent_range
        });

        if guarded {
            // Abort raid — flee.
            let flee_dx = if pos.x() < map.width / 2 { -1 } else { 1 };
            let flee_dy = if pos.y() < map.height / 2 { -1 } else { 1 };
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
            let dx = if rng_dx(pos.x(), map.width) { 1 } else { -1 };
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
                scent_map.deposit(pos.x(), pos.y(), fc.scent_deposit);
                if matches!(phase, FoxAiPhase::ScentMarking) {
                    activation.record(Feature::FoxScentMarked);
                }
            }
            _ => {}
        }
    }
}

/// 312: per-tick decay + deposit for `FoxApproachCorridorMap`.
/// Mirrors `fox_scent_tick`'s shape (per-tick reader of `FoxState +
/// FoxAiPhase + Position`, decay first then deposits). Deposits
/// only fire when the fox is in `FoxAiPhase::PatrolTerritory` —
/// actively patrolling (moved by the GOAP travel resolvers' desire
/// writes since 140 step 9) and advancing into a new tile.
/// `Resting`, `ScentMarking`, `DenGuarding`, and stalking phases are
/// excluded so stationary or pinned foxes don't paint the corridor
/// map.
///
/// The substrate is dormant in scoring at land
/// (`ward_fox_approach_corridor_weight = 0.0`), but the populator
/// still runs every tick so the trace emitter has live samples once
/// the weight is lifted at first-light. Cubs are skipped because
/// they don't patrol.
pub fn update_fox_approach_corridor_map(
    foxes: Query<(&FoxState, &FoxAiPhase, &Position)>,
    mut corridor: ResMut<crate::resources::FoxApproachCorridorMap>,
    constants: Res<SimConstants>,
) {
    corridor.decay_all(constants.wildlife.fox_approach_corridor_half_life_ticks);

    let deposit = constants.wildlife.fox_approach_corridor_deposit_per_tick;
    if deposit <= 0.0 {
        return;
    }
    for (fox, phase, pos) in &foxes {
        if fox.life_stage == FoxLifeStage::Cub {
            continue;
        }
        if matches!(phase, FoxAiPhase::PatrolTerritory { .. }) {
            corridor.deposit(pos.x(), pos.y(), deposit);
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
                pos.distance_to(&colony) >= 7.0,
                "wildlife at ({}, {}) is too close to colony at ({}, {})",
                pos.x(),
                pos.y(),
                colony.x(),
                colony.y()
            );
        }
    }
}
