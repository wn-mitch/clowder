use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::identity::Name;
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::PreyAnimal;
use crate::components::wildlife::{BehaviorType, WildAnimal, WildSpecies, WildlifeAiState};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

/// Per-cat cooldown tracking for threat detection narratives.
/// Suppresses repeated detection lines for the same cat for 100 ticks (~1 day).
#[derive(Resource, Default, Debug)]
pub struct DetectionCooldowns {
    /// Per-cat detection cooldown (entity → earliest next tick).
    pub cat_cooldowns: HashMap<Entity, u64>,
    /// Per-species spawn narrative cooldown (species → earliest next tick).
    pub spawn_cooldowns: HashMap<WildSpecies, u64>,
}

/// Narrative cooldown in ticks between detection messages for the same cat.
const DETECTION_NARRATIVE_COOLDOWN: u64 = 100;

// ---------------------------------------------------------------------------
// Wildlife AI system
// ---------------------------------------------------------------------------

/// Move each wild animal according to its behavior pattern.
pub fn wildlife_ai(
    mut query: Query<(&WildAnimal, &mut Position, &mut WildlifeAiState)>,
    mut map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
) {
    for (animal, mut pos, mut ai_state) in &mut query {
        match *ai_state {
            WildlifeAiState::Patrolling { dx, dy } => {
                let next = Position::new(pos.x + dx, pos.y + dy);
                if map.in_bounds(next.x, next.y) && is_patrol_terrain(map.get(next.x, next.y).terrain, animal.species) {
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
            WildlifeAiState::Circling { center_x, center_y, ref mut angle } => {
                *angle += 0.3; // ~20 ticks for a full circle
                if *angle > std::f32::consts::TAU {
                    *angle -= std::f32::consts::TAU;
                }
                let radius = 8.0;
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
        }

        // ShadowFox spreads corruption to tiles it crosses.
        if animal.species == WildSpecies::ShadowFox && map.in_bounds(pos.x, pos.y) {
            let tile = map.get_mut(pos.x, pos.y);
            tile.corruption = (tile.corruption + 0.01).min(1.0);
        }

        // Small random direction jitter for patrol creatures to avoid getting stuck.
        if matches!(*ai_state, WildlifeAiState::Patrolling { .. })
            && rng.rng.random::<f32>() < 0.1
        {
            let new_dx = rng.rng.random_range(-1i32..=1);
            let new_dy = rng.rng.random_range(-1i32..=1);
            if new_dx != 0 || new_dy != 0 {
                *ai_state = WildlifeAiState::Patrolling { dx: new_dx, dy: new_dy };
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
        WildSpecies::Snake => matches!(
            terrain,
            Terrain::Rock | Terrain::Mud | Terrain::Grass
        ),
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
/// Rate limit for spawn narratives: one per species per 50 ticks.
const SPAWN_NARRATIVE_COOLDOWN: u64 = 50;

pub fn spawn_wildlife(
    query: Query<&WildAnimal>,
    mut commands: Commands,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut cooldowns: ResMut<DetectionCooldowns>,
) {
    // ShadowFox is corruption-spawned only, not edge-spawned.
    for species in [WildSpecies::Fox, WildSpecies::Hawk, WildSpecies::Snake] {
        let current_count = query.iter().filter(|a| a.species == species).count();
        if current_count >= species.population_cap() {
            continue;
        }

        if rng.rng.random::<f32>() >= species.spawn_chance() {
            continue;
        }

        // Pick a random map-edge tile.
        if let Some(spawn_pos) = pick_edge_spawn(&map, species, &mut rng.rng) {
            let animal = WildAnimal::new(species);
            let ai_state = initial_ai_state(species, &spawn_pos, &map, &mut rng.rng);
            commands.spawn((
                animal,
                spawn_pos,
                Health::default(),
                ai_state,
            ));

            // Rate-limited spawn narrative.
            let on_cooldown = cooldowns
                .spawn_cooldowns
                .get(&species)
                .is_some_and(|&last| time.tick.saturating_sub(last) < SPAWN_NARRATIVE_COOLDOWN);

            if !on_cooldown {
                let text = match species {
                    WildSpecies::Fox => "A fox emerges from the forest edge.",
                    WildSpecies::Hawk => "A hawk begins circling overhead.",
                    WildSpecies::Snake => "A snake slithers out from the underbrush.",
                    WildSpecies::ShadowFox => "A shadow-fox materializes from the corruption.",
                };
                log.push(time.tick, text.to_string(), NarrativeTier::Action);
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
        WildSpecies::Hawk => matches!(
            terrain,
            Terrain::Grass | Terrain::Sand
        ),
        WildSpecies::Snake => matches!(
            terrain,
            Terrain::Rock | Terrain::Mud
        ),
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
            let dx = if pos.x == 0 { 1 } else if pos.x == map.width - 1 { -1 } else if rng.random() { 1 } else { -1 };
            let dy = if pos.y == 0 { 1 } else if pos.y == map.height - 1 { -1 } else { 0 };
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

/// Base detection range in Manhattan tiles.
const BASE_DETECTION_RANGE: i32 = 5;
/// Range penalty when the cat is in forest terrain.
const FOREST_RANGE_PENALTY: i32 = 2;

/// Each tick, living cats scan for nearby wildlife and react with fear.
///
/// Cats already performing a Fight action skip detection (they know the threat).
/// Detection is deduped: a cat won't re-trigger fear for a threat it already
/// has a fresh `ThreatSeen` memory about.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn detect_threats(
    mut cats: Query<(
        Entity,
        &Position,
        &CurrentAction,
        &mut Needs,
        &mut Memory,
        &mut Mood,
        &Name,
    ), Without<Dead>>,
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
) {
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
            let mut range = BASE_DETECTION_RANGE;
            if map.in_bounds(cat_pos.x, cat_pos.y) {
                let terrain = map.get(cat_pos.x, cat_pos.y).terrain;
                if matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
                    range -= FOREST_RANGE_PENALTY;
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
            let dist = cat_pos.manhattan_distance(&threat_pos);
            if dist > detection_range {
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
            needs.safety = (needs.safety - 0.15).max(0.0);

            memory.remember(MemoryEntry {
                event_type: MemoryType::ThreatSeen,
                location: Some(threat_pos),
                involved: vec![threat_entity],
                tick: time.tick,
                strength: 1.0,
                firsthand: true,
            });

            mood.modifiers.push_back(MoodModifier {
                amount: -0.2,
                ticks_remaining: 30,
                source: format!("{} spotted", species.name()),
            });

            // Detection narrative with per-cat cooldown.
            let on_cooldown = cooldowns
                .cat_cooldowns
                .get(&cat_entity)
                .is_some_and(|&last| time.tick.saturating_sub(last) < DETECTION_NARRATIVE_COOLDOWN);

            if !on_cooldown {
                let cat = &name.0;
                let text = match species {
                    WildSpecies::Fox => {
                        let variants = [
                            format!("{cat} spots a fox slinking through the undergrowth."),
                            format!("{cat} catches the scent of fox on the wind."),
                            format!("{cat} freezes \u{2014} a rust-red shape moves between the trees."),
                            format!("{cat} hears something prowling in the brush."),
                        ];
                        let idx = rng.rng.random_range(0..variants.len());
                        variants[idx].clone()
                    }
                    WildSpecies::Hawk => {
                        let variants = [
                            format!("A hawk circles overhead \u{2014} {cat} freezes."),
                            format!("{cat} spots a shadow sweeping across the ground \u{2014} a hawk."),
                            format!("{cat} looks up sharply. A raptor rides the thermals."),
                        ];
                        let idx = rng.rng.random_range(0..variants.len());
                        variants[idx].clone()
                    }
                    WildSpecies::Snake => {
                        let variants = [
                            format!("{cat} hisses and recoils \u{2014} a snake lies coiled nearby."),
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
pub fn predator_hunt_prey(
    mut commands: Commands,
    predators: Query<(&WildAnimal, &Position), Without<PreyAnimal>>,
    prey: Query<(Entity, &PreyAnimal, &Position)>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
) {
    for (predator, pred_pos) in predators.iter() {
        // Only hunt sometimes (10% chance per tick).
        if rng.rng.random::<f32>() > 0.1 {
            continue;
        }

        let hunt_range: i32 = match predator.species {
            WildSpecies::Fox => 3,
            WildSpecies::Hawk => 5,
            WildSpecies::Snake => 1, // Ambush range
            WildSpecies::ShadowFox => 3,
        };

        // Find nearest prey in range.
        let mut nearest: Option<(Entity, i32)> = None;
        for (prey_entity, _prey_animal, prey_pos) in prey.iter() {
            let dist = pred_pos.manhattan_distance(prey_pos);
            if dist <= hunt_range
                && (nearest.is_none() || dist < nearest.unwrap().1)
            {
                nearest = Some((prey_entity, dist));
            }
        }

        if let Some((prey_entity, _)) = nearest {
            if let Ok((_, prey_animal, _)) = prey.get(prey_entity) {
                // 30% kill chance per hunt attempt.
                if rng.rng.random::<f32>() < 0.3 {
                    let species_name = prey_animal.species.name();
                    let predator_name = predator.species.name();
                    commands.entity(prey_entity).despawn();

                    // Rate-limited logging: ~5% of kills produce a narrative line.
                    if rng.rng.random::<f32>() < 0.05 {
                        log.push(
                            time.tick,
                            format!("A {predator_name} snatches a {species_name} from the undergrowth."),
                            NarrativeTier::Micro,
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wildlife cleanup
// ---------------------------------------------------------------------------

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
            log.push(time.tick, text.to_string(), NarrativeTier::Micro);
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Initial wildlife spawning (called from world gen)
// ---------------------------------------------------------------------------

/// Spawn initial wildlife far from the colony center.
pub fn spawn_initial_wildlife(
    world: &mut World,
    colony_center: Position,
) {
    let mut spawn_positions: Vec<(WildSpecies, Position, WildlifeAiState)> = Vec::new();

    // Extract map dimensions and terrain data we need, then borrow rng separately.
    let map_width = world.resource::<TileMap>().width;
    let map_height = world.resource::<TileMap>().height;

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

        let find_spawn = |min_dist: i32, species: WildSpecies, rng: &mut rand_chacha::ChaCha8Rng| -> Option<Position> {
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

        let make_ai = |species: WildSpecies, pos: &Position, rng: &mut rand_chacha::ChaCha8Rng| -> WildlifeAiState {
            match species.default_behavior() {
                BehaviorType::Patrol => {
                    let dx = if pos.x == 0 { 1 } else if pos.x == map_width - 1 { -1 } else if rng.random() { 1 } else { -1 };
                    let dy = if pos.y == 0 { 1 } else if pos.y == map_height - 1 { -1 } else { 0 };
                    WildlifeAiState::Patrolling { dx, dy }
                }
                BehaviorType::Circle => {
                    let center_x = (pos.x + (map_width / 2 - pos.x).signum() * 8).clamp(0, map_width - 1);
                    let center_y = (pos.y + (map_height / 2 - pos.y).signum() * 8).clamp(0, map_height - 1);
                    WildlifeAiState::Circling {
                        center_x,
                        center_y,
                        angle: rng.random_range(0.0..std::f32::consts::TAU),
                    }
                }
                BehaviorType::Ambush => WildlifeAiState::Waiting,
            }
        };

        // Foxes: 2-3 at forest-edge tiles, 15+ tiles from colony.
        let fox_count: u32 = rng.random_range(2..=3);
        for _ in 0..fox_count {
            if let Some(pos) = find_spawn(15, WildSpecies::Fox, rng) {
                let ai = make_ai(WildSpecies::Fox, &pos, rng);
                spawn_positions.push((WildSpecies::Fox, pos, ai));
            }
        }

        // Hawks: 1-2 at grass tiles, 15+ tiles from colony.
        let hawk_count: u32 = rng.random_range(1..=2);
        for _ in 0..hawk_count {
            if let Some(pos) = find_spawn(15, WildSpecies::Hawk, rng) {
                let ai = make_ai(WildSpecies::Hawk, &pos, rng);
                spawn_positions.push((WildSpecies::Hawk, pos, ai));
            }
        }

        // Snakes: 1-2 at rock/mud tiles, 10+ tiles from colony.
        let snake_count: u32 = rng.random_range(1..=2);
        for _ in 0..snake_count {
            if let Some(pos) = find_spawn(10, WildSpecies::Snake, rng) {
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
        ));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use rand_chacha::ChaCha8Rng;
    use rand_chacha::rand_core::SeedableRng;

    fn test_rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(42)
    }

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
        world.spawn((
            WildAnimal::new(species),
            pos,
            Health::default(),
            ai_state,
        )).id()
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
        assert!(pos.x != 5 || pos.y != 15, "fox should have moved from (5, 15)");
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
            WildlifeAiState::Circling { center_x: 20, center_y: 15, angle: 0.0 },
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

        // Spawn max foxes.
        for i in 0..WildSpecies::Fox.population_cap() {
            world.spawn((
                WildAnimal::new(WildSpecies::Fox),
                Position::new(i as i32, 0),
                Health::default(),
                WildlifeAiState::Patrolling { dx: 1, dy: 0 },
            ));
        }

        let fox_count_before = world.query::<&WildAnimal>()
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

        let fox_count_after = world.query::<&WildAnimal>()
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

        let colony = Position::new(40, 30);
        spawn_initial_wildlife(&mut world, colony);

        let positions: Vec<Position> = world
            .query_filtered::<&Position, With<WildAnimal>>()
            .iter(&world)
            .copied()
            .collect();

        assert!(!positions.is_empty(), "should spawn at least some wildlife");
        for pos in &positions {
            assert!(
                pos.manhattan_distance(&colony) >= 10,
                "wildlife at ({}, {}) is too close to colony at ({}, {})",
                pos.x, pos.y, colony.x, colony.y
            );
        }
    }
}
