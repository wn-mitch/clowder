use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::CurrentAction;
use clowder::components::identity::{Age, Name, Species};
use clowder::components::magic::Inventory;
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Health, Needs, Position};
use clowder::components::skills::{Corruption, MagicAffinity, Training};
use clowder::resources::{
    ColonyHuntingMap, ColonyKnowledge, ColonyPriority, FoodStores, NarrativeLog, Relationships,
    SimConfig, SimRng, TemplateRegistry, TileMap, TimeState, WeatherState,
};
use clowder::world_gen::colony::{
    find_colony_site, generate_starting_cats, spawn_starting_buildings,
};
use clowder::world_gen::terrain::generate_terrain;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_world(seed: u64) -> World {
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let mut sim_rng = SimRng::new(seed);

    let mut map = generate_terrain(80, 60, &mut sim_rng.rng);

    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    let constants = clowder::resources::SimConstants::default();
    clowder::world_gen::special_tiles::place_special_tiles(
        &mut map,
        colony_site,
        &mut sim_rng.rng,
        &constants.world_gen,
    );
    clowder::world_gen::herbs::initialize_tile_magic(&mut map, &mut sim_rng.rng);

    let start_tick: u64 = 200_000;
    let cat_blueprints = generate_starting_cats(
        8,
        start_tick,
        config.ticks_per_season,
        &constants.founder_age,
        &mut sim_rng.rng,
    );

    let mut world = World::new();
    spawn_starting_buildings(&mut world, colony_site, &mut map);
    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: clowder::resources::SimSpeed::Normal,
    });
    world.insert_resource(clowder::resources::time::TimeScale::from_config(
        &config, 16.6667,
    ));
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(FoodStores::default());
    world.insert_resource(ColonyKnowledge::default());
    world.insert_resource(ColonyPriority::default());
    world.insert_resource(ColonyHuntingMap::default());
    world.insert_resource(clowder::resources::ExplorationMap::default());
    world.insert_resource(clowder::resources::wind::WindState::default());
    world.insert_resource(clowder::resources::time::TransitionTracker::default());
    world.insert_resource(clowder::species::build_registry());
    world.insert_resource(clowder::components::prey::PreyDensity::default());
    world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    world.insert_resource(clowder::resources::SimConstants::default());
    world.insert_resource(clowder::resources::SystemActivation::default());
    world.insert_resource(clowder::resources::FoxScentMap::default());
    world.insert_resource(clowder::resources::PreyScentMap::default());
    world.insert_resource(clowder::resources::CatPresenceMap::default());
    world.insert_resource(clowder::resources::ForcedConditions::default());
    world.insert_resource(clowder::resources::ColonyCenter(colony_site));
    world.insert_resource(clowder::resources::ColonyScore::default());
    world.insert_resource(clowder::resources::UnmetDemand::default());
    // L2 substrate resources + Eat DSE registration (mirrors the
    // plugin/headless insertion pattern).
    world.insert_resource(clowder::ai::faction::FactionRelations::canonical());
    {
        let scoring = world
            .resource::<clowder::resources::SimConstants>()
            .scoring
            .clone();
        let mut registry = clowder::ai::eval::DseRegistry::new();
        registry.cat_dses.push(clowder::ai::dses::eat_dse());
        registry.cat_dses.push(clowder::ai::dses::hunt_dse());
        registry.cat_dses.push(clowder::ai::dses::forage_dse());
        registry.cat_dses.push(clowder::ai::dses::cook_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::flee_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::fight_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::sleep_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::idle_dse(&scoring));
        registry.cat_dses.push(clowder::ai::dses::socialize_dse());
        registry.cat_dses.push(clowder::ai::dses::groom_self_dse());
        registry.cat_dses.push(clowder::ai::dses::groom_other_dse());
        registry.cat_dses.push(clowder::ai::dses::mentor_dse());
        registry.cat_dses.push(clowder::ai::dses::caretake_dse());
        registry.cat_dses.push(clowder::ai::dses::mate_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::patrol_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::build_dse(&scoring));
        registry.cat_dses.push(clowder::ai::dses::farm_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::coordinate_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::explore_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::wander_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::herbcraft_gather_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::herbcraft_prepare_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::herbcraft_ward_dse());
        registry.cat_dses.push(clowder::ai::dses::scry_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::durable_ward_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::cleanse_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::colony_cleanse_dse());
        registry.cat_dses.push(clowder::ai::dses::harvest_dse());
        registry.cat_dses.push(clowder::ai::dses::commune_dse());
        registry
            .fox_dses
            .push(clowder::ai::dses::fox_patrolling_dse(&scoring));
        registry
            .fox_dses
            .push(clowder::ai::dses::fox_hunting_dse(&scoring));
        registry.fox_dses.push(clowder::ai::dses::fox_raiding_dse());
        registry.fox_dses.push(clowder::ai::dses::fox_fleeing_dse());
        registry
            .fox_dses
            .push(clowder::ai::dses::fox_avoiding_dse());
        registry
            .fox_dses
            .push(clowder::ai::dses::fox_den_defense_dse());
        registry
            .fox_dses
            .push(clowder::ai::dses::fox_resting_dse(&scoring));
        registry.fox_dses.push(clowder::ai::dses::fox_feeding_dse());
        registry
            .fox_dses
            .push(clowder::ai::dses::fox_dispersing_dse());
        world.insert_resource(registry);
    }
    world.insert_resource(clowder::ai::eval::ModifierPipeline::new());
    bevy_ecs::message::MessageRegistry::register_message::<clowder::components::prey::PreyKilled>(
        &mut world,
    );
    bevy_ecs::message::MessageRegistry::register_message::<clowder::components::prey::DenRaided>(
        &mut world,
    );
    bevy_ecs::message::MessageRegistry::register_message::<
        clowder::components::goap_plan::PlanNarrative,
    >(&mut world);
    bevy_ecs::message::MessageRegistry::register_message::<
        clowder::systems::magic::CorruptionPushback,
    >(&mut world);
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    let template_path = std::path::Path::new("assets/narrative");
    if let Ok(registry) = TemplateRegistry::load_from_dir(template_path) {
        world.insert_resource(registry);
    }

    let mut entity_ids: Vec<Entity> = Vec::with_capacity(8);
    for (i, cat) in cat_blueprints.into_iter().enumerate() {
        let offset_x = (i as i32 % 5) - 2;
        let offset_y = (i as i32 / 5) - 1;

        let (spawn_x, spawn_y) = {
            let map_ref = world.resource::<TileMap>();
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
                    Needs::default(),
                    Mood::default(),
                    Memory::default(),
                ),
                (
                    cat.zodiac_sign,
                    cat.skills,
                    MagicAffinity(cat.magic_affinity),
                    Corruption(0.0),
                    Training::default(),
                    CurrentAction::default(),
                    Inventory::default(),
                    clowder::components::disposition::ActionHistory::default(),
                    clowder::components::hunting_priors::HuntingPriors::default(),
                    clowder::components::grooming::GroomingCondition::default(),
                    clowder::components::goap_plan::PendingUrgencies::default(),
                    clowder::components::SensorySpecies::Cat,
                    clowder::components::SensorySignature::CAT,
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

    world
}

fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            clowder::systems::time::advance_time,
            clowder::systems::weather::update_weather,
            clowder::systems::items::decay_items,
            clowder::systems::items::prune_stored_items,
            clowder::systems::items::sync_food_stores,
            clowder::systems::needs::decay_needs,
        )
            .chain(),
    );
    // Disposition pipeline (mirrors main schedule ordering).
    schedule.add_systems(clowder::systems::disposition::check_anxiety_interrupts);
    schedule.add_systems(
        clowder::systems::disposition::evaluate_dispositions
            .after(clowder::systems::disposition::check_anxiety_interrupts),
    );
    schedule.add_systems(
        bevy_ecs::schedule::ApplyDeferred
            .after(clowder::systems::disposition::evaluate_dispositions)
            .before(clowder::systems::disposition::disposition_to_chain),
    );
    schedule.add_systems(
        clowder::systems::disposition::disposition_to_chain
            .after(clowder::systems::disposition::evaluate_dispositions),
    );
    schedule.add_systems(
        bevy_ecs::schedule::ApplyDeferred
            .after(clowder::systems::disposition::disposition_to_chain)
            .before(clowder::systems::disposition::resolve_disposition_chains),
    );
    schedule.add_systems(
        clowder::systems::disposition::resolve_disposition_chains
            .after(clowder::systems::disposition::disposition_to_chain),
    );
    schedule.add_systems(
        (
            clowder::systems::task_chains::resolve_task_chains,
            clowder::systems::social::passive_familiarity,
            clowder::systems::social::check_bonds,
            clowder::systems::narrative::generate_narrative,
        )
            .chain()
            .after(clowder::systems::disposition::resolve_disposition_chains),
    );
    schedule
}

fn run_simulation(seed: u64, ticks: u64) -> Vec<String> {
    let mut world = setup_world(seed);
    let mut schedule = build_schedule();

    for _ in 0..ticks {
        schedule.run(&mut world);
    }

    world
        .resource::<NarrativeLog>()
        .entries
        .iter()
        .map(|e| e.text.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn simulation_is_deterministic() {
    let first = run_simulation(42, 100);
    let second = run_simulation(42, 100);

    assert_eq!(
        first.len(),
        second.len(),
        "runs produced different numbers of narrative entries: {} vs {}",
        first.len(),
        second.len()
    );

    for (i, (a, b)) in first.iter().zip(second.iter()).enumerate() {
        assert_eq!(
            a, b,
            "narrative entry {i} differs between runs:\n  run1: {a}\n  run2: {b}"
        );
    }
}

#[test]
fn cats_eat_when_hungry() {
    let mut world = setup_world(42);
    let mut schedule = build_schedule();

    // Run one tick to let sync_food_stores populate FoodStores from actual items.
    schedule.run(&mut world);

    // Drive all cats to near-starving hunger.
    let entity_ids: Vec<Entity> = world.query::<Entity>().iter(&world).collect();

    for entity in entity_ids {
        if let Some(mut needs) = world.get_mut::<Needs>(entity) {
            needs.hunger = 0.1;
        }
    }

    // Cats need time to finish current actions, walk to stores, and eat.
    // With softmax selection, not every cat chooses Eat on first opportunity.
    for _ in 0..200 {
        schedule.run(&mut world);
    }

    // At least one cat should have eaten and recovered some hunger.
    // With 0.002/tick decay over 200 ticks (0.4 drain) and food_value ~0.3,
    // a cat that eats even once should be above 0.0.
    let max_hunger = world
        .query::<&Needs>()
        .iter(&world)
        .map(|n| n.hunger)
        .fold(0.0f32, f32::max);

    assert!(
        max_hunger > 0.0,
        "no cat has any hunger after 200 ticks (max={max_hunger}) — eating may be broken"
    );
}

#[test]
fn simulation_runs_1000_ticks_without_panic() {
    let mut world = setup_world(42);
    let mut schedule = build_schedule();

    for _ in 0..1000 {
        schedule.run(&mut world);
    }
    // If we reach here, no panic occurred.
}
