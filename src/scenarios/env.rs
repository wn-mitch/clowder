//! Environment helpers for scenario setup. Each scenario calls
//! [`init_scenario_world`] first to populate all the standard resources
//! `build_new_world` would, then uses [`spawn_cat`] / [`spawn_kitten`] /
//! prey / herb helpers to populate entities under scenario control.

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;

use crate::components::physical::{Needs, Position};
use crate::plugins::setup::spawn_cat_from_blueprint;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::{
    ColonyHuntingMap, ColonyKnowledge, ColonyPriority, EventLog, FoodStores, NarrativeLog,
    Relationships, SimConfig, SimRng, TimeState,
};

use super::preset::{CatPreset, PresetParts};

/// Initialize all the standard resources `build_new_world` does, but with
/// a flat-grass terrain and no entities. After this, scenarios use
/// [`spawn_cat`] / [`spawn_kitten`] / [`spawn_prey_at`] / etc. to populate
/// the world.
///
/// Defaults: 40×40 flat-grass terrain, colony center at (20, 20),
/// `start_tick = 60 * ticks_per_season` (matches `build_new_world` so
/// founder ages compute correctly).
pub fn init_scenario_world(world: &mut World, seed: u64) {
    init_scenario_world_with(world, seed, ScenarioWorldConfig::default());
}

/// Configuration for [`init_scenario_world_with`]. Most scenarios just use
/// `Default::default()`; bigger maps or off-center colonies override the
/// relevant fields.
pub struct ScenarioWorldConfig {
    pub width: i32,
    pub height: i32,
    pub colony_center: Position,
}

impl Default for ScenarioWorldConfig {
    fn default() -> Self {
        Self {
            width: 40,
            height: 40,
            colony_center: Position::new(20, 20),
        }
    }
}

/// Like [`init_scenario_world`] but with a custom map size / colony
/// center.
pub fn init_scenario_world_with(world: &mut World, seed: u64, cfg: ScenarioWorldConfig) {
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let sim_rng = SimRng::new(seed);
    let constants = crate::resources::SimConstants::from_env();

    let map = TileMap::new(cfg.width, cfg.height, Terrain::Grass);
    let colony_site = cfg.colony_center;

    // Mirrors `build_new_world` line 206 so cat ages compute correctly.
    let start_tick: u64 = 60 * config.ticks_per_season;

    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: crate::resources::SimSpeed::Normal,
    });
    if let Some(mut score) = world.get_resource_mut::<crate::resources::ColonyScore>() {
        score.last_recorded_season = start_tick / config.ticks_per_season;
    }
    world.insert_resource(crate::resources::WeatherState::default());
    world.insert_resource(crate::resources::ForcedConditions::default());
    world.insert_resource(crate::resources::time::TransitionTracker::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(ColonyKnowledge::default());
    world.insert_resource(ColonyPriority::default());
    world.insert_resource(ColonyHuntingMap::default());
    world.insert_resource(crate::resources::ExplorationMap::default());
    world.insert_resource(crate::resources::CorruptionLandmarks::default());
    world.insert_resource(crate::resources::ColonyLandmarks::default());
    world.insert_resource(FoodStores::default());
    world.insert_resource(crate::systems::wildlife::DetectionCooldowns::default());
    world.insert_resource(crate::resources::SystemActivation::default());
    world.insert_resource(constants);
    world.insert_resource(map);
    world.insert_resource(sim_rng);
    world.insert_resource(crate::resources::ColonyCenter(colony_site));
    world.insert_resource(EventLog::default());
    world.insert_resource(config);
    world.insert_resource(Relationships::default());

    // Influence maps — same set `build_new_world` inserts at lines 351-386.
    world.insert_resource(crate::resources::FoxScentMap::default());
    world.insert_resource(crate::resources::PreyScentMap::default());
    world.insert_resource(crate::resources::CarcassScentMap::default());
    world.insert_resource(crate::resources::CatPresenceMap::default());
    world.insert_resource(crate::resources::WardCoverageMap::default());
    world.insert_resource(crate::resources::FoodLocationMap::default());
    world.insert_resource(crate::resources::GardenLocationMap::default());
    world.insert_resource(crate::resources::ConstructionSiteMap::default());
    world.insert_resource(crate::resources::KittenCryMap::default());
    world.insert_resource(crate::resources::HerbLocationMap::default());
    world.insert_resource(crate::resources::UnmetDemand::default());

    // Species registry — needed for spawn_prey_at, prey scoring, and
    // hunt-related DSEs. setup_world_exclusive itself inserts this very
    // early; we re-insert here so init_scenario_world is a complete
    // alternative to build_new_world.
    world.insert_resource(crate::species::build_registry());
    world.insert_resource(crate::components::prey::PreyDensity::default());
    if !world.contains_resource::<crate::resources::ColonyScore>() {
        world.insert_resource(crate::resources::ColonyScore::default());
    }

    // Colony-singleton entity — mirrors `build_new_world` so scenario
    // harness DSE queries resolve `Single<_, With<ColonyState>>`.
    // Ticket 168.
    world.spawn(crate::components::markers::ColonyState);
    debug_assert_eq!(
        world
            .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<crate::components::markers::ColonyState>>()
            .iter(world)
            .count(),
        1,
        "exactly one ColonyState singleton must exist after init_scenario_world_with"
    );
}

/// Spawn an adult cat from a preset. Routes through
/// [`spawn_cat_from_blueprint`] so the component bundle is identical to
/// the production founder spawn.
pub fn spawn_cat(world: &mut World, preset: CatPreset) -> Entity {
    let PresetParts {
        position,
        needs,
        fulfillment,
        blueprint,
        markers,
    } = preset.into_blueprint();
    let entity = spawn_cat_from_blueprint(world, blueprint, position, needs, fulfillment);
    markers.apply(world, entity);
    entity
}

/// Spawn a kitten with a `KittenDependency` linking it to its mother /
/// father. Position, needs, personality come from the preset.
pub fn spawn_kitten(
    world: &mut World,
    preset: CatPreset,
    mother: Entity,
    father: Entity,
) -> Entity {
    let PresetParts {
        position,
        mut needs,
        fulfillment,
        blueprint,
        markers,
    } = preset.into_blueprint();
    // Kitten Needs default per pregnancy.rs:128-133.
    if !preset_overrode_needs(&needs) {
        needs = Needs {
            hunger: 0.5,
            energy: 0.8,
            mating: 1.0,
            ..Needs::default()
        };
    }
    let entity = spawn_cat_from_blueprint(world, blueprint, position, needs, fulfillment);
    world
        .entity_mut(entity)
        .insert(crate::components::KittenDependency::new(mother, father));
    markers.apply(world, entity);
    entity
}

/// Spawn a single prey animal of `kind` at `pos`. Used by hunt-related
/// scenarios. The species registry must already be inserted (it's part
/// of the standard scenario world init).
pub fn spawn_prey_at(
    world: &mut World,
    pos: Position,
    kind: crate::components::prey::PreyKind,
) -> Entity {
    let registry = world.resource::<crate::species::SpeciesRegistry>();
    let profile = registry.find(kind);
    let bundle = crate::components::prey::prey_bundle(profile);
    world.spawn(bundle).insert(pos).id()
}

/// Spawn a complete garden structure at `pos`. Required for the `Farm`
/// DSE to score above zero (`HasGarden` colony marker is authored from
/// live Garden structures via `update_colony_facility_markers`).
pub fn spawn_garden_at(world: &mut World, pos: Position) -> Entity {
    use crate::components::building::{Structure, StructureType};
    world
        .spawn((Structure::new(StructureType::Garden), pos))
        .id()
}

/// Set corruption on the tile at `pos`. Used by ward-related scenarios
/// that need a corruption gradient pulling the cat toward setting a
/// ward.
pub fn mark_tile_corrupted(world: &mut World, pos: Position, level: f32) {
    let mut map = world.resource_mut::<TileMap>();
    if map.in_bounds(pos.x, pos.y) {
        let tile = map.get_mut(pos.x, pos.y);
        tile.corruption = level.clamp(0.0, 1.0);
    }
}

/// Add `count` herbs of the given kind to the cat's inventory.
pub fn give_herbs(world: &mut World, cat: Entity, herb: crate::components::magic::HerbKind, count: u32) {
    use crate::components::items::ItemKind;
    let item_kind = match herb {
        crate::components::magic::HerbKind::HealingMoss => ItemKind::HerbHealingMoss,
        crate::components::magic::HerbKind::Moonpetal => ItemKind::HerbMoonpetal,
        crate::components::magic::HerbKind::Calmroot => ItemKind::HerbCalmroot,
        crate::components::magic::HerbKind::Thornbriar => ItemKind::HerbThornbriar,
        crate::components::magic::HerbKind::Dreamroot => ItemKind::HerbDreamroot,
        crate::components::magic::HerbKind::Catnip => ItemKind::HerbCatnip,
        crate::components::magic::HerbKind::Slumbershade => ItemKind::HerbSlumbershade,
        crate::components::magic::HerbKind::OracleOrchid => ItemKind::HerbOracleOrchid,
    };
    let mut em = world.entity_mut(cat);
    let mut inv = em
        .get_mut::<crate::components::magic::Inventory>()
        .expect("cat must have an Inventory before give_herbs");
    for _ in 0..count {
        inv.add_item(item_kind);
    }
}

/// Spawn a hawk wildlife threat at `pos`. Hawks circle a target point
/// rather than ambush, so this scenario gives them a circling AI state
/// centered on the spawn position. Used by `wildlife_fight` to test the
/// L2 fight/flee scoring under threat presence.
pub fn spawn_hawk_at(world: &mut World, pos: Position) -> Entity {
    use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
    let animal = WildAnimal::new(WildSpecies::Hawk);
    let ai = WildlifeAiState::Circling {
        center_x: pos.x,
        center_y: pos.y,
        angle: 0.0,
    };
    world
        .spawn((
            animal,
            pos,
            crate::components::physical::Health::default(),
            ai,
            crate::components::SensorySpecies::Wild(WildSpecies::Hawk),
            crate::components::SensorySignature::WILDLIFE,
        ))
        .id()
}

/// Sentinel — needs object that hasn't been touched by the caller still
/// matches the founder default. We use this to detect whether the caller
/// explicitly customized needs (in which case respect them) or left them
/// at default (in which case substitute the kitten default).
fn preset_overrode_needs(n: &Needs) -> bool {
    let default = Needs::default();
    n.hunger != default.hunger
        || n.energy != default.energy
        || n.temperature != default.temperature
        || n.safety != default.safety
        || n.social != default.social
        || n.acceptance != default.acceptance
        || n.mating != default.mating
        || n.respect != default.respect
        || n.mastery != default.mastery
        || n.purpose != default.purpose
}
