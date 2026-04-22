use std::path::PathBuf;

use bevy::prelude::Resource;
use bevy_ecs::world::World;

use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::{Age, Name, Species};
use crate::components::magic::Inventory;
use crate::components::mental::{Memory, Mood};
use crate::components::physical::{Health, Needs, Position};
use crate::components::skills::{Corruption, MagicAffinity, Training};
use crate::persistence;
use crate::resources::{
    ColonyHuntingMap, ColonyKnowledge, ColonyPriority, EventLog, FoodStores, NarrativeLog,
    NarrativeTier, Relationships, SimConfig, SimRng, TemplateRegistry, TimeState, WeatherState,
};
use crate::world_gen::colony::{
    find_colony_site, generate_starting_cats, spawn_starting_buildings,
};
use crate::world_gen::custom_cats::load_custom_cats;
use crate::world_gen::terrain::generate_terrain;

/// CLI arguments passed as a Bevy resource so the startup system can read them.
#[derive(Resource)]
pub struct AppArgs {
    pub seed: u64,
    pub load_path: Option<PathBuf>,
    pub load_log_path: Option<PathBuf>,
    pub test_map: bool,
}

/// Exclusive startup system — has direct `&mut World` access for complex
/// initialization that needs immediate resource availability.
pub fn setup_world_exclusive(world: &mut World) {
    let args_seed;
    let args_load_path;
    let args_load_log_path;
    let args_test_map;

    // Extract args before mutating world.
    {
        let args = world.resource::<AppArgs>();
        args_seed = args.seed;
        args_load_path = args.load_path.clone();
        args_load_log_path = args.load_log_path.clone();
        args_test_map = args.test_map;
    }

    // Insert the species registry early — spawn_initial_prey needs it during build_new_world.
    world.insert_resource(crate::species::build_registry());
    world.insert_resource(crate::components::prey::PreyDensity::default());
    if !world.contains_resource::<crate::resources::ColonyScore>() {
        world.insert_resource(crate::resources::ColonyScore::default());
    }

    if let Some(ref load_path) = args_load_path {
        match persistence::load_from_file(load_path) {
            Ok(save) => {
                persistence::load_world(world, save);
            }
            Err(e) => {
                eprintln!("Error loading save: {e}");
                build_new_world(world, args_seed, args_test_map);
            }
        }
    } else {
        build_new_world(world, args_seed, args_test_map);
    }

    // Load template data.
    load_templates(world);
    load_zodiac_data(world);
    load_aspiration_data(world);

    // Push initial narrative for new worlds.
    if args_load_path.is_none() {
        let current_tick = world.resource::<TimeState>().tick;
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(
            current_tick,
            "A small group of cats settles in a clearing.".to_string(),
            NarrativeTier::Significant,
        );
    }

    // Load narrative log from file if provided.
    if let Some(ref path) = args_load_log_path {
        if let Err(e) = load_log_file(world, path) {
            eprintln!("Warning: failed to load log file: {e}");
        }
    }

    // Always insert the event log for mechanical debugging.
    world.insert_resource(EventLog::default());
    if !world.contains_resource::<crate::resources::snapshot_config::SnapshotConfig>() {
        world.insert_resource(crate::resources::snapshot_config::SnapshotConfig::default());
    }
    if !world.contains_resource::<crate::resources::wind::WindState>() {
        world.insert_resource(crate::resources::wind::WindState::default());
    }

    // Ensure new resources exist (may be absent from older saves).
    if !world.contains_resource::<ColonyKnowledge>() {
        world.insert_resource(ColonyKnowledge::default());
    }
    if !world.contains_resource::<ColonyPriority>() {
        world.insert_resource(ColonyPriority::default());
    }
    if !world.contains_resource::<ColonyHuntingMap>() {
        world.insert_resource(ColonyHuntingMap::default());
    }
    if !world.contains_resource::<crate::resources::ExplorationMap>() {
        world.insert_resource(crate::resources::ExplorationMap::default());
    }
    if !world.contains_resource::<crate::systems::wildlife::DetectionCooldowns>() {
        world.insert_resource(crate::systems::wildlife::DetectionCooldowns::default());
    }
    if !world.contains_resource::<crate::resources::SimConstants>() {
        world.insert_resource(crate::resources::SimConstants::default());
    }
    if !world.contains_resource::<crate::resources::SystemActivation>() {
        world.insert_resource(crate::resources::SystemActivation::default());
    }
    if !world.contains_resource::<crate::resources::ForcedConditions>() {
        world.insert_resource(crate::resources::ForcedConditions::default());
    }
}

fn build_new_world(world: &mut World, seed: u64, test_map: bool) {
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let mut sim_rng = SimRng::new(seed);

    // Generate terrain.
    let mut map = if test_map {
        eprintln!("Using hand-crafted test map for rendering debug");
        crate::world_gen::test_map::generate_test_map()
    } else {
        generate_terrain(120, 90, &mut sim_rng.rng)
    };

    // Find colony site first (read-only) so special tiles can respect colony distance.
    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    // Place special terrain tiles (ruins, fairy rings, standing stones, deep pools).
    let constants = crate::resources::SimConstants::default();
    crate::world_gen::special_tiles::place_special_tiles(
        &mut map,
        colony_site,
        &mut sim_rng.rng,
        &constants.world_gen,
    );

    // Set initial corruption and mystery on special tiles (must be after placement).
    crate::world_gen::herbs::initialize_tile_magic(&mut map, &mut sim_rng.rng);

    // Start the clock high enough that cats can have varied ages.
    // Must exceed the maximum rolled age in ticks (see
    // `FounderAgeConstants::elder_max_seasons`) — see main.rs for the detailed
    // rationale. Short version: saturating_sub silently clamps ages below
    // start_tick, so too small a value means every founder reads back as Young.
    let start_tick: u64 = 60 * config.ticks_per_season;

    let age_consts = &constants.founder_age;
    let mut cat_blueprints = load_custom_cats(
        start_tick,
        config.ticks_per_season,
        age_consts,
        &mut sim_rng.rng,
    );
    let remaining = 8usize.saturating_sub(cat_blueprints.len());
    if remaining > 0 {
        cat_blueprints.extend(generate_starting_cats(
            remaining,
            start_tick,
            config.ticks_per_season,
            age_consts,
            &mut sim_rng.rng,
        ));
    }

    // Spawn starting buildings (sets terrain tiles and creates entities).
    spawn_starting_buildings(world, colony_site, &mut map);

    // Persist colony center and spawn decorative well entity.
    world.insert_resource(crate::resources::ColonyCenter(colony_site));
    world.spawn((
        crate::components::building::ColonyWell,
        Position::new(colony_site.x, colony_site.y),
    ));

    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: crate::resources::SimSpeed::Normal,
    });
    // Seed `last_recorded_season` so `seasons_survived` counts from 0 despite
    // the non-zero start_tick. ColonyScore was inserted earlier with defaults.
    if let Some(mut score) = world.get_resource_mut::<crate::resources::ColonyScore>() {
        score.last_recorded_season = start_tick / config.ticks_per_season;
    }
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(crate::resources::ForcedConditions::default());
    world.insert_resource(crate::resources::time::TransitionTracker::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(ColonyKnowledge::default());
    world.insert_resource(ColonyPriority::default());
    world.insert_resource(ColonyHuntingMap::default());
    world.insert_resource(crate::resources::ExplorationMap::default());
    world.insert_resource(FoodStores::default());
    world.insert_resource(crate::systems::wildlife::DetectionCooldowns::default());
    world.insert_resource(crate::resources::SystemActivation::default());
    world.insert_resource(constants);
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    // Spawn cats.
    let cat_count = cat_blueprints.len();
    let mut entity_ids: Vec<bevy_ecs::entity::Entity> = Vec::with_capacity(cat_count);
    for (i, cat) in cat_blueprints.into_iter().enumerate() {
        let offset_x = (i as i32 % 5) - 2;
        let offset_y = (i as i32 / 5) - 1;

        let (spawn_x, spawn_y) = {
            let map_ref = world.resource::<crate::resources::TileMap>();
            (
                (colony_site.x + offset_x).clamp(0, map_ref.width - 1),
                (colony_site.y + offset_y).clamp(0, map_ref.height - 1),
            )
        };

        let entity = world
            .spawn((
                (
                    Name(cat.name),
                    Species,
                    Age {
                        born_tick: cat.born_tick,
                    },
                    cat.gender,
                    cat.orientation,
                    cat.personality,
                    cat.appearance,
                    Position::new(spawn_x, spawn_y),
                    Health::default(),
                    Needs::staggered(i, cat_count),
                    Mood::default(),
                    Memory::default(),
                ),
                (
                    cat.zodiac_sign,
                    cat.skills,
                    MagicAffinity(cat.magic_affinity),
                    Corruption(0.0),
                    Training::default(),
                    crate::ai::CurrentAction::default(),
                    Inventory::default(),
                    crate::components::disposition::ActionHistory::default(),
                    HuntingPriors::default(),
                    crate::components::grooming::GroomingCondition::default(),
                    crate::components::goap_plan::PendingUrgencies::default(),
                    crate::components::SensorySpecies::Cat,
                    crate::components::SensorySignature::CAT,
                ),
            ))
            .id();
        entity_ids.push(entity);
    }

    // Initialize relationships between all pairs.
    {
        let mut relationships = Relationships::default();
        let mut rng = world.resource_mut::<SimRng>();
        for i in 0..entity_ids.len() {
            for j in (i + 1)..entity_ids.len() {
                relationships.init_pair(entity_ids[i], entity_ids[j], &mut rng.rng);
            }
        }
        world.insert_resource(relationships);
    }

    // Spawn initial wildlife far from the colony.
    crate::systems::wildlife::spawn_initial_wildlife(world, colony_site);
    crate::systems::wildlife::spawn_initial_fox_dens(world, colony_site);

    // Insert fox scent map resource.
    world.insert_resource(crate::resources::FoxScentMap::default());

    // Insert prey scent map resource (Phase 2B).
    world.insert_resource(crate::resources::PreyScentMap::default());

    // Insert cat presence map resource.
    world.insert_resource(crate::resources::CatPresenceMap::default());

    // Insert unmet-demand ledger — tracks frustrated wants (e.g. cats
    // scoring Cook but with no Kitchen) so the coordinator can prioritize
    // the missing infrastructure.
    world.insert_resource(crate::resources::UnmetDemand::default());

    // Spawn initial prey animals across their habitats.
    crate::world_gen::prey_ecosystem::seed_prey_ecosystem(world);

    // Spawn herbs based on terrain and current season.
    let current_season = {
        let time = world.resource::<TimeState>();
        let config = world.resource::<SimConfig>();
        time.season(config)
    };
    crate::world_gen::herbs::spawn_herbs(world, current_season);
    crate::world_gen::herbs::spawn_flavor_plants(world, current_season);
}

fn load_templates(world: &mut World) {
    let template_path = std::path::Path::new("assets/narrative");
    match TemplateRegistry::load_from_dir(template_path) {
        Ok(registry) => {
            world.insert_resource(registry);
        }
        Err(e) => {
            eprintln!("Warning: failed to load narrative templates: {e}");
        }
    }
}

fn load_zodiac_data(world: &mut World) {
    let path = std::path::Path::new("assets/data/zodiac.ron");
    match crate::resources::ZodiacData::load(path) {
        Ok(data) => {
            world.insert_resource(data);
        }
        Err(e) => {
            eprintln!("Warning: failed to load zodiac data: {e}");
        }
    }
}

fn load_aspiration_data(world: &mut World) {
    let path = std::path::Path::new("assets/narrative/aspirations");
    match crate::resources::AspirationRegistry::load_from_dir(path) {
        Ok(registry) => {
            world.insert_resource(registry);
        }
        Err(e) => {
            eprintln!("Warning: failed to load aspiration data: {e}");
        }
    }
}

fn load_log_file(world: &mut World, path: &std::path::Path) -> Result<(), std::io::Error> {
    use std::io::BufRead;

    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut loaded = 0u64;
    for line in reader.lines() {
        let line = line?;
        let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("bad JSON in log: {e}"),
            )
        })?;
        if v.get("_header").is_some() {
            continue;
        }
        let tick = v["tick"].as_u64().unwrap_or(0);
        let text = v["text"].as_str().unwrap_or("").to_string();
        let tier = match v["tier"].as_str().unwrap_or("Action") {
            "Micro" => NarrativeTier::Micro,
            "Significant" => NarrativeTier::Significant,
            _ => NarrativeTier::Action,
        };
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(tick, text, tier);
        loaded += 1;
    }
    eprintln!("Loaded {loaded} log entries from {}", path.display());
    Ok(())
}
