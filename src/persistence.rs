use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io;
use std::path::Path;

use bevy_ecs::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::ai::CurrentAction;
use crate::components::building::{ConstructionSite, CropState, GateState, Structure};
use crate::components::identity::{Age, Appearance, Gender, Name, Orientation, Species};
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills, Training};
use crate::resources::relationships::{Relationship, Relationships};
use crate::resources::{
    FoodStores, NarrativeLog, SimConfig, SimRng, TileMap, TimeState, WeatherState,
};

// ---------------------------------------------------------------------------
// Save file types
// ---------------------------------------------------------------------------

/// Top-level save file containing the full world snapshot.
#[derive(Serialize, Deserialize)]
pub struct SaveFile {
    pub time: TimeState,
    pub config: SimConfig,
    pub weather: WeatherState,
    pub food: FoodStores,
    pub map: TileMap,
    pub narrative: NarrativeLog,
    pub rng: ChaCha8Rng,
    pub cats: Vec<CatSnapshot>,
    pub relationships: Vec<SavedRelationship>,
    #[serde(default)]
    pub buildings: Vec<BuildingSnapshot>,
}

/// Serializable snapshot of a building entity.
#[derive(Serialize, Deserialize)]
pub struct BuildingSnapshot {
    pub name: String,
    pub position: Position,
    pub structure: Structure,
    pub construction_site: Option<ConstructionSite>,
    pub crop_state: Option<CropState>,
    pub gate_state: Option<GateState>,
}

/// Serializable snapshot of a single cat entity.
#[derive(Serialize, Deserialize)]
pub struct CatSnapshot {
    pub name: String,
    pub born_tick: u64,
    pub gender: Gender,
    pub orientation: Orientation,
    pub personality: Personality,
    pub appearance: Appearance,
    pub position: Position,
    pub health: Health,
    pub needs: Needs,
    pub mood: Mood,
    pub memory: SavedMemory,
    pub skills: Skills,
    pub magic_affinity: f32,
    pub corruption: f32,
    pub training: SavedTraining,
    pub current_action: CurrentAction,
    pub dead: Option<Dead>,
    #[serde(default)]
    pub inventory: Vec<ItemSlot>,
    #[serde(default)]
    pub is_coordinator: bool,
    #[serde(default)]
    pub directive_queue: Option<Vec<crate::components::coordination::Directive>>,
}

// ---------------------------------------------------------------------------
// Entity-remapped types
// ---------------------------------------------------------------------------

/// Memory buffer with entity references replaced by cat-vec indices.
#[derive(Serialize, Deserialize)]
pub struct SavedMemory {
    pub events: VecDeque<SavedMemoryEntry>,
    pub capacity: usize,
}

/// A single memory entry with entity refs replaced by indices.
#[derive(Serialize, Deserialize)]
pub struct SavedMemoryEntry {
    pub event_type: MemoryType,
    pub location: Option<Position>,
    /// Indices into the `cats` vec. Stale refs are dropped on save.
    pub involved: Vec<usize>,
    pub tick: u64,
    pub strength: f32,
    pub firsthand: bool,
}

/// Training relationships as cat-vec indices.
#[derive(Serialize, Deserialize)]
pub struct SavedTraining {
    pub mentor: Option<usize>,
    pub apprentice: Option<usize>,
}

/// A single relationship entry with entity pair replaced by cat-vec indices.
#[derive(Serialize, Deserialize)]
pub struct SavedRelationship {
    pub cat_a: usize,
    pub cat_b: usize,
    pub data: Relationship,
}

// ---------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------

/// Capture the full world state into a serializable snapshot.
pub fn save_world(world: &mut World) -> SaveFile {
    // Collect all cat entities in a stable order.
    let mut cat_entities: Vec<Entity> = world
        .query_filtered::<Entity, With<Species>>()
        .iter(world)
        .collect();
    cat_entities.sort_by_key(|e| e.index());

    let entity_to_index: HashMap<Entity, usize> = cat_entities
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, i))
        .collect();

    // Snapshot each cat.
    let cats: Vec<CatSnapshot> = cat_entities
        .iter()
        .map(|&entity| snapshot_cat(world, entity, &entity_to_index))
        .collect();

    // Snapshot resources.
    let time = world.resource::<TimeState>().clone();
    let config = world.resource::<SimConfig>().clone();
    let weather_state = world.resource::<WeatherState>();
    let weather = WeatherState {
        current: weather_state.current,
        ticks_until_change: weather_state.ticks_until_change,
    };
    let food = world.resource::<FoodStores>().clone();
    let rng = world.resource::<SimRng>().rng.clone();

    // Clone map — one-time save operation, not per-tick.
    let map_ref = world.resource::<TileMap>();
    let map = clone_tile_map(map_ref);

    let narrative_ref = world.resource::<NarrativeLog>();
    let narrative = NarrativeLog {
        entries: narrative_ref.entries.clone(),
        capacity: narrative_ref.capacity,
        total_pushed: narrative_ref.total_pushed,
    };

    // Save relationships, remapping entity pairs to indices.
    let relationships_res = world.resource::<Relationships>();
    let relationships: Vec<SavedRelationship> = relationships_res
        .iter()
        .filter_map(|((a, b), rel)| {
            let idx_a = entity_to_index.get(&a)?;
            let idx_b = entity_to_index.get(&b)?;
            Some(SavedRelationship {
                cat_a: *idx_a,
                cat_b: *idx_b,
                data: rel.clone(),
            })
        })
        .collect();

    // Snapshot buildings.
    let mut building_entities: Vec<Entity> = world
        .query_filtered::<Entity, With<Structure>>()
        .iter(world)
        .collect();
    building_entities.sort_by_key(|e| e.index());

    let buildings: Vec<BuildingSnapshot> = building_entities
        .iter()
        .map(|&entity| {
            let name = world
                .get::<Name>(entity)
                .map(|n| n.0.clone())
                .unwrap_or_default();
            let position = *world.get::<Position>(entity).unwrap();
            let structure = world.get::<Structure>(entity).unwrap().clone();
            let construction_site = world.get::<ConstructionSite>(entity).cloned();
            let crop_state = world.get::<CropState>(entity).cloned();
            let gate_state = world.get::<GateState>(entity).cloned();
            BuildingSnapshot {
                name,
                position,
                structure,
                construction_site,
                crop_state,
                gate_state,
            }
        })
        .collect();

    SaveFile {
        time,
        config,
        weather,
        food,
        map,
        narrative,
        rng,
        cats,
        relationships,
        buildings,
    }
}

fn snapshot_cat(world: &World, entity: Entity, entity_map: &HashMap<Entity, usize>) -> CatSnapshot {
    let name = world.get::<Name>(entity).unwrap();
    let age = world.get::<Age>(entity).unwrap();
    let gender = world.get::<Gender>(entity).unwrap();
    let orientation = world.get::<Orientation>(entity).unwrap();
    let personality = world.get::<Personality>(entity).unwrap();
    let appearance = world.get::<Appearance>(entity).unwrap();
    let position = world.get::<Position>(entity).unwrap();
    let health = world.get::<Health>(entity).unwrap();
    let needs = world.get::<Needs>(entity).unwrap();
    let mood = world.get::<Mood>(entity).unwrap();
    let memory = world.get::<Memory>(entity).unwrap();
    let skills = world.get::<Skills>(entity).unwrap();
    let magic_affinity = world.get::<MagicAffinity>(entity).unwrap();
    let corruption = world.get::<Corruption>(entity).unwrap();
    let training = world.get::<Training>(entity).unwrap();
    let current_action = world.get::<CurrentAction>(entity).unwrap();
    let dead = world.get::<Dead>(entity);
    let inventory = world.get::<Inventory>(entity);

    let is_coordinator = world
        .get::<crate::components::coordination::Coordinator>(entity)
        .is_some();
    let directive_queue = world
        .get::<crate::components::coordination::DirectiveQueue>(entity)
        .map(|q| q.directives.clone());

    CatSnapshot {
        name: name.0.clone(),
        born_tick: age.born_tick,
        gender: *gender,
        orientation: *orientation,
        personality: personality.clone(),
        appearance: appearance.clone(),
        position: *position,
        health: health.clone(),
        needs: needs.clone(),
        mood: mood.clone(),
        memory: save_memory(memory, entity_map),
        skills: skills.clone(),
        magic_affinity: magic_affinity.0,
        corruption: corruption.0,
        training: save_training(training, entity_map),
        current_action: current_action.clone(),
        dead: dead.cloned(),
        inventory: inventory.map(|i| i.slots.clone()).unwrap_or_default(),
        is_coordinator,
        directive_queue,
    }
}

fn save_memory(memory: &Memory, entity_map: &HashMap<Entity, usize>) -> SavedMemory {
    let events = memory
        .events
        .iter()
        .map(|entry| save_memory_entry(entry, entity_map))
        .collect();
    SavedMemory {
        events,
        capacity: memory.capacity,
    }
}

fn save_memory_entry(entry: &MemoryEntry, entity_map: &HashMap<Entity, usize>) -> SavedMemoryEntry {
    let involved = entry
        .involved
        .iter()
        .filter_map(|e| entity_map.get(e).copied())
        .collect();
    SavedMemoryEntry {
        event_type: entry.event_type,
        location: entry.location,
        involved,
        tick: entry.tick,
        strength: entry.strength,
        firsthand: entry.firsthand,
    }
}

fn save_training(training: &Training, entity_map: &HashMap<Entity, usize>) -> SavedTraining {
    SavedTraining {
        mentor: training.mentor.and_then(|e| entity_map.get(&e).copied()),
        apprentice: training
            .apprentice
            .and_then(|e| entity_map.get(&e).copied()),
    }
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

/// Reconstruct the ECS world from a save file.
///
/// Clears all existing entities and overwrites resources. The caller is
/// responsible for reloading the `TemplateRegistry` from disk afterward.
pub fn load_world(world: &mut World, save: SaveFile) {
    // Clear existing entities.
    world.clear_entities();

    // Insert resources.
    world.insert_resource(save.time);
    world.insert_resource(save.config);
    world.insert_resource(save.weather);
    world.insert_resource(save.food);
    world.insert_resource(save.map);
    world.insert_resource(save.narrative);
    world.insert_resource(SimRng { rng: save.rng });

    // Spawn cats and collect handles for entity remapping.
    let mut entity_handles: Vec<Entity> = Vec::with_capacity(save.cats.len());
    for cat in &save.cats {
        let entity = world
            .spawn((
                (
                    Name(cat.name.clone()),
                    Species,
                    Age {
                        born_tick: cat.born_tick,
                    },
                    cat.gender,
                    cat.orientation,
                    cat.personality.clone(),
                    cat.appearance.clone(),
                    cat.position,
                    cat.health.clone(),
                    cat.needs.clone(),
                    cat.mood.clone(),
                    Memory::default(),
                ),
                (
                    cat.skills.clone(),
                    MagicAffinity(cat.magic_affinity),
                    Corruption(cat.corruption),
                    Training::default(),
                    cat.current_action.clone(),
                    Inventory {
                        slots: cat.inventory.clone(),
                    },
                ),
            ))
            .id();
        // Add Dead component if present.
        if let Some(ref dead) = cat.dead {
            world.entity_mut(entity).insert(dead.clone());
        }
        // Restore coordinator status.
        if cat.is_coordinator {
            use crate::components::coordination::{Coordinator, DirectiveQueue};
            let queue = cat
                .directive_queue
                .as_ref()
                .map(|d| DirectiveQueue {
                    directives: d.clone(),
                })
                .unwrap_or_default();
            world.entity_mut(entity).insert((Coordinator, queue));
        }
        entity_handles.push(entity);
    }

    // Reconstruct relationships from saved data.
    let mut relationships = Relationships::default();
    for saved_rel in &save.relationships {
        if let (Some(&a), Some(&b)) = (
            entity_handles.get(saved_rel.cat_a),
            entity_handles.get(saved_rel.cat_b),
        ) {
            relationships.insert(a, b, saved_rel.data.clone());
        }
    }
    world.insert_resource(relationships);

    // Respawn buildings.
    for building in &save.buildings {
        let mut entity_cmds = world.spawn((
            Name(building.name.clone()),
            building.position,
            building.structure.clone(),
        ));
        if let Some(ref site) = building.construction_site {
            entity_cmds.insert(site.clone());
        }
        if let Some(ref crop) = building.crop_state {
            entity_cmds.insert(crop.clone());
        }
        if let Some(ref gate) = building.gate_state {
            entity_cmds.insert(gate.clone());
        }
    }

    // Remap entity references in Memory and Training.
    for (i, cat) in save.cats.iter().enumerate() {
        let entity = entity_handles[i];

        let memory = load_memory(&cat.memory, &entity_handles);
        let training = load_training(&cat.training, &entity_handles);

        let mut entity_mut = world.entity_mut(entity);
        *entity_mut.get_mut::<Memory>().unwrap() = memory;
        *entity_mut.get_mut::<Training>().unwrap() = training;
    }
}

fn load_memory(saved: &SavedMemory, entity_handles: &[Entity]) -> Memory {
    let events = saved
        .events
        .iter()
        .map(|entry| load_memory_entry(entry, entity_handles))
        .collect();
    Memory {
        events,
        capacity: saved.capacity,
    }
}

fn load_memory_entry(entry: &SavedMemoryEntry, entity_handles: &[Entity]) -> MemoryEntry {
    let involved = entry
        .involved
        .iter()
        .filter_map(|&idx| entity_handles.get(idx).copied())
        .collect();
    MemoryEntry {
        event_type: entry.event_type,
        location: entry.location,
        involved,
        tick: entry.tick,
        strength: entry.strength,
        firsthand: entry.firsthand,
    }
}

fn load_training(saved: &SavedTraining, entity_handles: &[Entity]) -> Training {
    Training {
        mentor: saved
            .mentor
            .and_then(|idx| entity_handles.get(idx).copied()),
        apprentice: saved
            .apprentice
            .and_then(|idx| entity_handles.get(idx).copied()),
    }
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

/// Save the world state to a JSON file, creating parent directories as needed.
pub fn save_to_file(world: &mut World, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let save = save_world(world);
    let json = serde_json::to_string_pretty(&save)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

/// Load a save file from disk.
pub fn load_from_file(path: &Path) -> io::Result<SaveFile> {
    let json = fs::read_to_string(path)?;
    serde_json::from_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

// ---------------------------------------------------------------------------
// TileMap clone helper
// ---------------------------------------------------------------------------

fn clone_tile_map(map: &TileMap) -> TileMap {
    let tiles = (0..map.height)
        .flat_map(|y| {
            (0..map.width).map(move |x| {
                let t = map.get(x, y);
                crate::resources::map::Tile::new_with(t.terrain, t.corruption, t.mystery)
            })
        })
        .collect();
    TileMap::from_raw(map.width, map.height, tiles)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::identity::Orientation;
    use crate::components::mental::{MemoryEntry, MemoryType};
    use crate::resources::map::Terrain;
    use crate::resources::SimSpeed;
    use rand::SeedableRng;

    /// Build a minimal world with two cats for testing.
    fn build_test_world() -> World {
        let mut world = World::new();

        // Resources
        world.insert_resource(TimeState {
            tick: 5000,
            paused: false,
            speed: SimSpeed::Fast,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(WeatherState::default());
        world.insert_resource(FoodStores::new(15.0, 50.0, 0.002));
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimRng::new(42));
        world.insert_resource(TileMap::new(10, 10, Terrain::Grass));

        // Cat A
        let cat_a = world
            .spawn((
                (
                    Name("Mochi".to_string()),
                    Species,
                    Age { born_tick: 1000 },
                    Gender::Queen,
                    Orientation::Bisexual,
                    Personality::random(&mut rand_chacha::ChaCha8Rng::seed_from_u64(1)),
                    Appearance {
                        fur_color: "ginger".to_string(),
                        pattern: "tabby".to_string(),
                        eye_color: "green".to_string(),
                        distinguishing_marks: vec![],
                    },
                    Position::new(5, 5),
                    Health::default(),
                    Needs::default(),
                    Mood::default(),
                    Memory::default(),
                ),
                (
                    Skills::default(),
                    MagicAffinity(0.3),
                    Corruption(0.0),
                    Training::default(),
                    CurrentAction::default(),
                ),
            ))
            .id();

        // Cat B
        let cat_b = world
            .spawn((
                (
                    Name("Basil".to_string()),
                    Species,
                    Age { born_tick: 2000 },
                    Gender::Tom,
                    Orientation::Straight,
                    Personality::random(&mut rand_chacha::ChaCha8Rng::seed_from_u64(2)),
                    Appearance {
                        fur_color: "black".to_string(),
                        pattern: "solid".to_string(),
                        eye_color: "amber".to_string(),
                        distinguishing_marks: vec!["torn ear".to_string()],
                    },
                    Position::new(6, 5),
                    Health::default(),
                    Needs::default(),
                    Mood::default(),
                    Memory::default(),
                ),
                (
                    Skills::default(),
                    MagicAffinity(0.1),
                    Corruption(0.0),
                    Training::default(),
                    CurrentAction::default(),
                ),
            ))
            .id();

        // Give cat_a a memory involving cat_b.
        world
            .get_mut::<Memory>(cat_a)
            .unwrap()
            .remember(MemoryEntry {
                event_type: MemoryType::SocialEvent,
                location: Some(Position::new(5, 5)),
                involved: vec![cat_b],
                tick: 4500,
                strength: 0.8,
                firsthand: true,
            });

        // Set up a training relationship: A mentors B.
        *world.get_mut::<Training>(cat_a).unwrap() = Training {
            mentor: None,
            apprentice: Some(cat_b),
        };
        *world.get_mut::<Training>(cat_b).unwrap() = Training {
            mentor: Some(cat_a),
            apprentice: None,
        };

        // Initialize relationships.
        let mut relationships = Relationships::default();
        relationships.insert(
            cat_a,
            cat_b,
            Relationship {
                fondness: 0.5,
                familiarity: 0.3,
                romantic: 0.0,
                bond: None,
                last_interaction: 4000,
            },
        );
        world.insert_resource(relationships);

        world
    }

    #[test]
    fn round_trip_preserves_resources() {
        let mut world = build_test_world();
        let save = save_world(&mut world);

        let mut new_world = World::new();
        load_world(&mut new_world, save);

        let time = new_world.resource::<TimeState>();
        assert_eq!(time.tick, 5000);
        assert_eq!(time.speed, SimSpeed::Fast);

        let food = new_world.resource::<FoodStores>();
        assert!((food.current - 15.0).abs() < 1e-6);
    }

    #[test]
    fn round_trip_preserves_cat_data() {
        let mut world = build_test_world();
        let save = save_world(&mut world);

        let mut new_world = World::new();
        load_world(&mut new_world, save);

        // Check we have two cats.
        let cat_count = new_world
            .query_filtered::<Entity, With<Species>>()
            .iter(&new_world)
            .count();
        assert_eq!(cat_count, 2);

        // Verify Mochi's data.
        let mut names: Vec<String> = new_world
            .query::<&Name>()
            .iter(&new_world)
            .map(|n| n.0.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["Basil", "Mochi"]);
    }

    #[test]
    fn round_trip_remaps_entity_refs() {
        let mut world = build_test_world();
        let save = save_world(&mut world);

        let mut new_world = World::new();
        load_world(&mut new_world, save);

        // Find Mochi and Basil by name.
        let mut cats: Vec<(Entity, String)> = new_world
            .query::<(Entity, &Name)>()
            .iter(&new_world)
            .map(|(e, n)| (e, n.0.clone()))
            .collect();
        cats.sort_by(|a, b| a.1.cmp(&b.1));
        let basil_entity = cats[0].0;
        let mochi_entity = cats[1].0;

        // Mochi's memory should reference Basil's new Entity.
        let memory = new_world.get::<Memory>(mochi_entity).unwrap();
        assert_eq!(memory.events.len(), 1);
        assert_eq!(memory.events[0].involved, vec![basil_entity]);

        // Training: Mochi mentors Basil.
        let mochi_training = new_world.get::<Training>(mochi_entity).unwrap();
        assert_eq!(mochi_training.apprentice, Some(basil_entity));
        assert_eq!(mochi_training.mentor, None);

        let basil_training = new_world.get::<Training>(basil_entity).unwrap();
        assert_eq!(basil_training.mentor, Some(mochi_entity));
        assert_eq!(basil_training.apprentice, None);
    }

    #[test]
    fn stale_entity_refs_dropped_on_save() {
        let mut world = build_test_world();

        // Despawn one cat to create stale refs.
        let cat_b: Entity = world
            .query::<(Entity, &Name)>()
            .iter(&world)
            .find(|(_, n)| n.0 == "Basil")
            .unwrap()
            .0;
        world.despawn(cat_b);

        let save = save_world(&mut world);

        // Mochi's memory involved list should be empty (Basil is gone).
        assert_eq!(save.cats.len(), 1);
        assert_eq!(save.cats[0].name, "Mochi");
        assert!(save.cats[0].memory.events[0].involved.is_empty());
        assert!(save.cats[0].training.apprentice.is_none());
    }

    #[test]
    fn file_round_trip() {
        let mut world = build_test_world();
        let dir = std::env::temp_dir().join("clowder_test_save");
        let path = dir.join("test.json");

        save_to_file(&mut world, &path).expect("save failed");
        assert!(path.exists());

        let save = load_from_file(&path).expect("load failed");
        assert_eq!(save.cats.len(), 2);
        assert_eq!(save.time.tick, 5000);

        // Clean up.
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dead_cats_round_trip() {
        let mut world = build_test_world();

        // Mark Basil as dead.
        let basil: Entity = world
            .query::<(Entity, &Name)>()
            .iter(&world)
            .find(|(_, n)| n.0 == "Basil")
            .unwrap()
            .0;
        world.entity_mut(basil).insert(Dead {
            tick: 4800,
            cause: crate::components::physical::DeathCause::Starvation,
        });

        let save = save_world(&mut world);

        let mut new_world = World::new();
        load_world(&mut new_world, save);

        let new_basil: Entity = new_world
            .query::<(Entity, &Name)>()
            .iter(&new_world)
            .find(|(_, n)| n.0 == "Basil")
            .unwrap()
            .0;
        let dead = new_world.get::<Dead>(new_basil).unwrap();
        assert_eq!(dead.tick, 4800);
    }
}
