use std::path::PathBuf;

use bevy::prelude::Resource;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;

use crate::components::fulfillment::Fulfillment;
use crate::components::grooming::GroomingCondition;
use crate::components::identity::{Age, Name, Species};
use crate::components::magic::Inventory;
use crate::components::mental::{Memory, Mood};
use crate::components::physical::{Health, Needs, Position};
use crate::components::skills::{Corruption, MagicAffinity, Training};
use crate::persistence;
use crate::resources::{
    ColonyHuntingMap, ColonyKnowledge, ColonyPriority, EventLog, FoodStores, NarrativeLog,
    NarrativeTier, Relationships, SimConfig, SimRng, TemplateRegistry, TimeScale, TimeState,
    WeatherState,
};
use crate::world_gen::colony::{
    find_colony_site, generate_starting_cats, spawn_starting_buildings, CatBlueprint,
};
use crate::world_gen::custom_cats::load_custom_cats;
use crate::world_gen::terrain::generate_terrain;

/// Ticket 162 — scenario harness world-setup override. When inserted as a
/// resource before `SimulationPlugin` builds, the contained closure replaces
/// `build_new_world` inside `setup_world_exclusive`. The rest of
/// `setup_world_exclusive` (template loading, narrative, TimeScale rebuild,
/// default-resource backfills) still runs, so the closure is responsible
/// only for the resource + entity setup that `build_new_world` normally
/// performs (terrain, SimConfig, SimRng, SimConstants, all influence maps,
/// cats, prey, herbs).
///
/// Scenarios use `crate::scenarios::env` helpers to do the heavy lifting
/// rather than reimplementing all of `build_new_world`'s resource init.
type ScenarioSetupFn = Box<dyn FnOnce(&mut World, u64) + Send + Sync>;

#[derive(Resource)]
pub struct WorldSetup {
    setup: ScenarioSetupFn,
}

impl WorldSetup {
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(&mut World, u64) + Send + Sync + 'static,
    {
        Self { setup: Box::new(f) }
    }

    fn run(self, world: &mut World, seed: u64) {
        (self.setup)(world, seed);
    }
}

/// Ticket 452 — canonical component bundle for a cat entity. Returning
/// `impl Bundle` lets both `World`-mode callers (founder spawn via
/// `spawn_cat_from_blueprint`) and `Commands`-mode callers (the
/// `tick_pregnancy` births loop in `src/systems/pregnancy.rs`) compose
/// it via `world.spawn(bundle)` or `commands.spawn(bundle)`. Kitten-only
/// markers (`KittenDependency`, `BornInSim`) are post-inserted by the
/// caller after spawning the canonical bundle.
///
/// `health` and `grooming` are parameters (not defaults inside the
/// bundle) so the kitten path can express its per-spawn divergence at
/// the call site: production kittens spawn with nutrition-scaled
/// `Health` (`0.7 + avg_nutrition * 0.3`) and a low `GroomingCondition`
/// reflecting the birth-membrane coat state. Founder callers pass
/// `Health::default()` / `GroomingCondition::default()`.
pub fn cat_bundle(
    blueprint: CatBlueprint,
    position: Position,
    needs: Needs,
    fulfillment: Fulfillment,
    health: Health,
    grooming: GroomingCondition,
) -> impl Bundle {
    (
        (
            Name(blueprint.name),
            Species,
            Age {
                born_tick: blueprint.born_tick,
            },
            blueprint.gender,
            blueprint.orientation,
            blueprint.personality,
            blueprint.appearance,
            position,
            health,
            needs,
            fulfillment,
            Mood::default(),
            Memory::default(),
        ),
        (
            blueprint.zodiac_sign,
            blueprint.skills,
            MagicAffinity(blueprint.magic_affinity),
            Corruption(0.0),
            Training::default(),
            crate::ai::CurrentAction::default(),
            // Ticket 017 — carry pouch + worn equip slots (OSRS-style),
            // nested as a sub-bundle to keep this tuple within Bevy's
            // 15-element limit. `WearableSlots` is required by the
            // `resolve_goap_plans` query + the equipment-aware combat/hunt
            // systems; empty at spawn, populated by auto-equip-on-craft.
            (
                Inventory::default(),
                crate::components::equipment::WearableSlots::default(),
            ),
            crate::components::disposition::ActionHistory::default(),
            grooming,
            crate::components::goap_plan::PendingUrgencies::default(),
            crate::components::SensorySpecies::Cat,
            crate::components::SensorySignature::CAT,
            // Ticket 073 — per-cat recently-failed target memory.
            // Ticket 108 — per-cat snapshot of last tick's
            // `safety_deficit` for the `ThreatProximityAdrenaline`
            // derivative. Default 0.0 matches a freshly-spawned cat
            // at full safety. Save-loaded cats fall through the
            // lazy-insert path in `update_prev_safety_deficit`.
            crate::components::PrevSafetyDeficit::default(),
        ),
        // 258 — C3 subjective belief substrate. Four per-cat mental-
        // model Components seeded empty; `belief_integrator`
        // populates them on first `WitnessableEvent` for a given
        // subject (or first `Implant`-evidence first-encounter for
        // predators).
        (
            crate::components::CatBeliefs::default(),
            crate::components::LocationBeliefs::default(),
            crate::components::PredatorBeliefs::default(),
            crate::components::ContextBeliefs::default(),
            // 308 — per-cat subjective belief about colony-wide
            // reserve stockpile counts (thornbriar / remedy-herb).
            // Authored by belief_integrator from three new
            // WitnessableEvent variants; dormant at land (consumer
            // is ticket 309).
            crate::components::beliefs::ColonyReservesBelief::default(),
            // 374 — per-cat housing-security belief (home_den +
            // four orthogonal sub-axes: belonging, quality,
            // continuity, threat). Spawned empty with no home_den.
            // Founder spawn, kitten birth, and construction-complete
            // claim paths set the field; six WitnessableEvent
            // variants update the sub-axes via belief_integrator.
            crate::components::ShelterBeliefs::default(),
            // 095 Phase 1 — anatomical injury substrate. Shadow
            // co-resident with Health during Stage A; sole source of
            // truth after Stage B cutover.
            crate::components::CatBodyModel::default(),
            // Ticket 138 — per-cat step-opportunity accumulator. Cats
            // default to `per_tick = 1.0` (every-tick cadence); the
            // gate at each `step_toward`-style call-site is a no-op
            // behaviorally today, but the substrate is in place so
            // future per-cat-cadence tuning is parameter-only.
            crate::components::MovementBudget::cat(),
            // 140 step 6 — fluid-movement components. Velocity is
            // integrator-owned; DesiredVelocity starts empty (no
            // desire) so unmigrated resolvers keep exclusive control
            // of Position until their migration step.
            crate::components::physical::Velocity::default(),
            crate::components::physical::DesiredVelocity::default(),
        ),
    )
}

/// Ticket 162 — single source of truth for the founder spawn bundle. Both
/// `build_new_world` (production colony spawn) and `crate::scenarios::env`
/// (microexperiment harness) call this so a missing component on either
/// path is impossible. Drift control is enforced by an integration test
/// (see `tests/scenarios.rs::cat_preset_matches_founder_bundle`).
///
/// Ticket 452 — thin wrapper over `cat_bundle`; founder spawns use
/// `Health::default()` and `GroomingCondition::default()`. Production
/// kittens (`pregnancy.rs::tick_pregnancy`) call `cat_bundle` directly
/// via `Commands` to express their per-spawn `Health` and low
/// `GroomingCondition`.
pub fn spawn_cat_from_blueprint(
    world: &mut World,
    blueprint: CatBlueprint,
    position: Position,
    needs: Needs,
    fulfillment: Fulfillment,
) -> Entity {
    world
        .spawn(cat_bundle(
            blueprint,
            position,
            needs,
            fulfillment,
            Health::default(),
            GroomingCondition::default(),
        ))
        .id()
}

/// CLI arguments passed as a Bevy resource so the startup system can read them.
#[derive(Resource)]
pub struct AppArgs {
    pub seed: u64,
    pub load_path: Option<PathBuf>,
    pub load_log_path: Option<PathBuf>,
    pub test_map: bool,
    /// Wall-seconds-per-in-game-day peg used to construct [`TimeScale`]
    /// during Startup. Headless: from `--game-day-seconds`. Windowed:
    /// derived from the initial `SimSpeed` (Normal → 1000s/day at the
    /// 1000-ticks/day default). Ticket 033.
    pub wall_seconds_per_game_day: f32,
}

/// Exclusive startup system — has direct `&mut World` access for complex
/// initialization that needs immediate resource availability.
pub fn setup_world_exclusive(world: &mut World) {
    let args_seed;
    let args_load_path;
    let args_load_log_path;
    let args_test_map;
    let args_wall_seconds_per_game_day;

    // Extract args before mutating world.
    {
        let args = world.resource::<AppArgs>();
        args_seed = args.seed;
        args_load_path = args.load_path.clone();
        args_load_log_path = args.load_log_path.clone();
        args_test_map = args.test_map;
        args_wall_seconds_per_game_day = args.wall_seconds_per_game_day;
    }

    // Insert the species registry early — spawn_initial_prey needs it during build_new_world.
    world.insert_resource(crate::species::build_registry());
    world.insert_resource(crate::components::prey::PreyDensity::default());
    if !world.contains_resource::<crate::resources::ColonyScore>() {
        world.insert_resource(crate::resources::ColonyScore::default());
    }

    // Provisional TimeScale so worldgen subsystems that pre-simulate
    // prey ecology (`seed_prey_ecosystem` → `presimulate_prey`) can
    // resolve `Res<TimeScale>` parameters on prey/fox systems gained
    // in ticket 033 Phase 4. Built from `SimConfig::default()`; the
    // post-build_new_world block at the bottom of this function
    // re-inserts the canonical TimeScale once the live SimConfig has
    // landed (defensive for the load_path case where saved SimConfig
    // may differ from defaults).
    {
        let provisional =
            TimeScale::from_config(&SimConfig::default(), args_wall_seconds_per_game_day);
        world.insert_resource(provisional);
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
    } else if let Some(setup) = world.remove_resource::<WorldSetup>() {
        // Ticket 162 — scenario harness override. Closure replaces
        // `build_new_world` entirely; templates / narrative / TimeScale /
        // default-resource backfills below still run as normal.
        setup.run(world, args_seed);
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

    // Build the TimeScale anchor from the live SimConfig + the host
    // peg. SimConfig is in place either via build_new_world or via
    // load_world above. Ticket 033 — single source of truth for the
    // ticks ↔ in-game time ↔ wall-clock conversion.
    {
        let sim_config = world.resource::<SimConfig>().clone();
        let time_scale = TimeScale::from_config(&sim_config, args_wall_seconds_per_game_day);
        world.insert_resource(time_scale);
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
    if !world.contains_resource::<crate::resources::CorruptionLandmarks>() {
        world.insert_resource(crate::resources::CorruptionLandmarks::default());
    }
    if !world.contains_resource::<crate::resources::ColonyLandmarks>() {
        world.insert_resource(crate::resources::ColonyLandmarks::default());
    }
    if !world.contains_resource::<crate::systems::wildlife::DetectionCooldowns>() {
        world.insert_resource(crate::systems::wildlife::DetectionCooldowns::default());
    }
    if !world.contains_resource::<crate::resources::SimConstants>() {
        // `from_env` reads the optional `CLOWDER_OVERRIDES` JSON env var
        // and deep-merges it into the defaults. Used by
        // `scripts/hypothesize.py` to drive treatment runs without
        // rebuilding the binary; the applied patch is echoed into the
        // events.jsonl header by `write_jsonl_headers`.
        world.insert_resource(crate::resources::SimConstants::from_env());
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
    // Use `from_env` so worldgen constants honor any CLOWDER_OVERRIDES patch.
    let constants = crate::resources::SimConstants::from_env();
    crate::world_gen::special_tiles::place_special_tiles(
        &mut map,
        colony_site,
        &mut sim_rng.rng,
        &constants.world_gen,
    );

    // Set initial corruption and mystery on special tiles (must be after placement).
    crate::world_gen::herbs::initialize_tile_magic(&mut map, &mut sim_rng.rng);

    // Start the clock high enough that cats can have varied ages. Must exceed
    // the maximum rolled age in ticks (see `FounderAgeConstants::adult_max_seasons`)
    // — saturating_sub silently clamps ages below start_tick, so too small a
    // value means every founder reads back as Young.
    let start_tick: u64 = 60 * config.ticks_per_season;

    let age_consts = &constants.founder_age;
    const TOTAL_FOUNDERS: usize = 8;
    let stages = crate::world_gen::colony::allocate_founder_stages(
        TOTAL_FOUNDERS,
        age_consts,
        &mut sim_rng.rng,
    );
    let mut stage_iter = stages.into_iter();

    let mut cat_blueprints = load_custom_cats(
        start_tick,
        config.ticks_per_season,
        age_consts,
        &mut stage_iter,
        &mut sim_rng.rng,
    );
    let remaining = TOTAL_FOUNDERS.saturating_sub(cat_blueprints.len());
    if remaining > 0 {
        cat_blueprints.extend(generate_starting_cats(
            remaining,
            start_tick,
            config.ticks_per_season,
            age_consts,
            &mut stage_iter,
            &mut sim_rng.rng,
        ));
    }

    // Spawn starting buildings (sets terrain tiles and creates entities).
    spawn_starting_buildings(world, colony_site, &mut map);

    // Persist colony center and spawn decorative well entity.
    world.insert_resource(crate::resources::ColonyCenter(colony_site));
    world.spawn((
        crate::components::building::ColonyWell,
        Position::new(colony_site.x(), colony_site.y()),
    ));

    // Colony-singleton entity — host for colony-scoped substrate markers
    // (`HasFunctionalKitchen`, `HasStoredFood`, `ThornbriarAvailable`,
    // `WardStrengthLow`, `WardsUnderSiege`, …). Authored each tick by
    // `update_colony_building_markers` / `update_herb_availability_markers`
    // / `update_ward_coverage_markers` / `update_ward_siege_marker`; read
    // by `evaluate_and_plan` to populate `MarkerSnapshot`. Ticket 168.
    world.spawn(crate::components::markers::ColonyState);
    debug_assert_eq!(
        world
            .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<crate::components::markers::ColonyState>>()
            .iter(world)
            .count(),
        1,
        "exactly one ColonyState singleton must exist after build_new_world"
    );

    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: crate::resources::SimSpeed::Normal,
    });
    // Seed `last_recorded_season` so `seasons_survived` counts from 0 despite
    // the non-zero start_tick. ColonyScore was inserted earlier with defaults.
    if let Some(mut score) = world.get_resource_mut::<crate::resources::ColonyScore>() {
        score.last_recorded_season = start_tick / config.ticks_per_season;
        // Anchor for the TPS-invariant checkpoint: `emit_colony_score`
        // computes elapsed = tick - run_start_tick (ticks are absolute).
        score.run_start_tick = start_tick;
    }
    // Ticket 490 — founder-dispersion canary accumulator (sampled by
    // `emit_cat_snapshots`, surfaced in the headless footer).
    world.insert_resource(
        crate::resources::founder_dispersion::FounderDispersionStats {
            run_start_tick: start_tick,
            ..Default::default()
        },
    );
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
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
    world.insert_resource(crate::resources::ColonyReserves::default());
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
                (colony_site.x() + offset_x).clamp(0, map_ref.width - 1),
                (colony_site.y() + offset_y).clamp(0, map_ref.height - 1),
            )
        };

        let needs = Needs::staggered(i, cat_count);
        // Ticket 488 — founder Fulfillment uses the warm-floor variant
        // (social_warmth [0.85, 1.0]) so the day-1 GroomOther SELF
        // driver doesn't fire on a fictitious 30-50% spawn deficit.
        // Mirrors b24d333b's warm-floor Relationships init pattern.
        let fulfillment = Fulfillment::founder(i, cat_count);
        let position = Position::new(spawn_x, spawn_y);
        let entity = spawn_cat_from_blueprint(world, cat, position, needs, fulfillment);
        // Ticket 490 — instrumentation marker for the founder-dispersion
        // canary (read by `emit_cat_snapshots`' dispersion sampler).
        world
            .entity_mut(entity)
            .insert(crate::components::identity::Founder);
        entity_ids.push(entity);
    }

    // Initialize relationships between all pairs.
    {
        let rel_consts = world
            .resource::<crate::resources::SimConstants>()
            .relationships
            .clone();
        let mut relationships = Relationships::default();
        let mut rng = world.resource_mut::<SimRng>();
        for i in 0..entity_ids.len() {
            for j in (i + 1)..entity_ids.len() {
                relationships.init_pair(entity_ids[i], entity_ids[j], &mut rng.rng, &rel_consts);
            }
        }
        world.insert_resource(relationships);
    }

    // Spawn initial wildlife far from the colony.
    crate::systems::wildlife::spawn_initial_wildlife(world, colony_site);
    crate::systems::wildlife::spawn_initial_fox_dens(world, colony_site);

    // Insert fox scent map resource.
    world.insert_resource(crate::resources::FoxScentMap::default());

    // Insert per-prey-species scent maps (ticket 062 / §5.6.3 row #5).
    // Five `PreyScentMap` sub-maps keyed by `PreyKind`; reads via
    // `PreyScentMaps::get_any` / `highest_nearby_any` preserve aggregate
    // semantics for current consumers.
    world.insert_resource(crate::resources::PreyScentMaps::default_maps());

    // Insert tremor influence-map (ticket 100). Same 120×90 / bucket=3
    // shape as PreyScentMap; fast-decay (≈ 1-3 ticks for a full bucket
    // per `TremorConstants::decay_per_tick`) so the map reflects
    // current-tick motion, not residue.
    world.insert_resource(crate::resources::TremorMap::default_map());

    // 101: five-axis environmental quality influence maps. True
    // tile-resolution (`bucket_size = 1`) so the 1–3 tile stamping
    // radii produce meaningful spatial gradients. Rebuilt every tick
    // by `update_env_quality_maps` after `decay_building_condition`.
    world.insert_resource(crate::resources::ComfortMap::default_map());
    world.insert_resource(crate::resources::CleanlinessMap::default_map());
    world.insert_resource(crate::resources::BeautyMap::default_map());
    world.insert_resource(crate::resources::MysteryMap::default_map());
    world.insert_resource(crate::resources::CorruptionInfluenceMap::default_map());

    // Insert carcass scent map resource (ticket 048 — Phase 2C
    // §5.6.3 row #6).
    world.insert_resource(crate::resources::CarcassScentMap::default());

    // Insert cover-availability map (ticket 423). Tile-resolution
    // boolean influence map of "low-cover tile within sprint_radius".
    // Replaces the per-cat O(radius²) disc scan in
    // `update_hide_eligible_markers`. Cold-start `dirty = true` so
    // the first `update_cover_availability_map` tick stamps the
    // worldgen-established terrain before any HideEligible author
    // run. Terrain mutators (building completion, magic remedy) call
    // `mark_dirty()` to schedule re-stamping.
    world.insert_resource(crate::resources::CoverAvailabilityMap::default());

    // 312: fox-approach corridor map (perception axis for ward placement).
    // Populated by `update_fox_approach_corridor_map` reading
    // ShadowFox `Position` + `FoxAiPhase` each tick; exponential decay
    // runs in the same system. Dormant in scoring at land — the
    // `ward_fox_approach_corridor_weight` weight defaults to 0.0 so
    // `compute_ward_placement` short-circuits the lift. FO-1 scenario
    // (`chokepoint_defense_isthmus`) activates it at fixture level.
    world.insert_resource(crate::resources::FoxApproachCorridorMap::default());

    // Insert cat scent map resource.
    world.insert_resource(crate::resources::CatScentMap::default());

    // 256 R5: cat patrol deterrent map. Cats deposit when patrolling;
    // foxes read as routing cost in their A* via
    // `CatPatrolDeterrentOverlay`.
    world.insert_resource(crate::resources::CatPatrolDeterrentMap::default());

    // Insert ward coverage map resource (ticket 045 — substrate-refactor §5.6.3).
    world.insert_resource(crate::resources::WardCoverageMap::default());

    // 470 — per-tile siege-fear from besieged wards. Recomputed each
    // tick by `update_ward_siege_fear_map` from the live
    // `WildlifeAiState::EncirclingWard` set. Substrate ships active
    // at land (the producer always runs); consumer DSE weights stay
    // dormant (`ward_siege_fear_weight = 0.0`) per the 301 byte-
    // identical-at-land precedent. The (26,61) seed-42 death class
    // (Heron / Simba bleeding to death while reading safety=1.00 on
    // a besieged-ward tile) motivates this perception channel.
    world.insert_resource(crate::resources::WardSiegeFearMap::default());

    // 382: colony-district composite map. Populated each tick by
    // `update_colony_district_map`; consumed by
    // `compute_building_placement` to retire the radius-16 spiral search.
    world.insert_resource(crate::resources::ColonyDistrictMap::default());

    // 261: per-action success-affordance substrate. Allocated empty;
    // populated each tick by `affordance_writer` (ticket 261 C3). Lands
    // substrate-only — no DSE consumers wired at land, so the resource is
    // present but unread, and `just verdict` shows null behavioural
    // drift. Consumer tickets (263+) read via `read_affordance(...)`
    // inside their `fetch_target` closures.
    world.insert_resource(crate::resources::ActionAffordances::default());

    // 301: ward-placement intent map. Dormant at default
    // `SimConstants` (populator and reader both short-circuit on
    // their flags). Allocated unconditionally so the resource is
    // present for the L1 trace walker and so the populator can stamp
    // into it without an Option<ResMut> guard.
    world.insert_resource(crate::resources::WardIntentMap::default());

    // 035: Insert grave-aura map resource. Recomputed each tick by
    // `update_grave_aura_map` from live `Grave` entities; consumed
    // (in the foundation) only via the `InfluenceMapRegistry`'s
    // L1-trace surface.
    world.insert_resource(crate::resources::GraveAuraMap::default());

    // Insert food-location map resource (ticket 006 — §5.6.3 row #7).
    world.insert_resource(crate::resources::FoodLocationMap::default());

    // Insert garden-location map resource (ticket 006 — §5.6.3 row #10).
    world.insert_resource(crate::resources::GardenLocationMap::default());

    // Insert construction-site map resource (ticket 006 — §5.6.3 row #9).
    world.insert_resource(crate::resources::ConstructionSiteMap::default());

    // Insert kitten-cry map resource (ticket 006 — §5.6.3 row #13;
    // repurposed by ticket 156 from Sight to Hearing channel).
    world.insert_resource(crate::resources::KittenCryMap::default());

    // Insert herb-location map resource (ticket 061 — §5.6.3 row #8).
    world.insert_resource(crate::resources::HerbLocationMap::default());

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
    // 321: aspiration chains migrated from RON to code-defined const
    // data in `crate::ai::aspirations`; `build_static` wraps the const
    // `ALL_CHAINS` table behind the existing registry surface. No I/O.
    world.insert_resource(crate::resources::AspirationRegistry::build_static());
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
