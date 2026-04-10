use std::io::{self, BufRead, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::app::AppExit;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::{
    App, ButtonInput, ClearColor, Color, DefaultPlugins, ImagePlugin, MessageWriter, PluginGroup,
    Res, ResMut, Startup, Time, Fixed, Update, Window, WindowPlugin,
};
use bevy::window::WindowResolution;

use clowder::ui_data::{InspectionMode, InspectionState};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::CurrentAction;
use clowder::components::identity::{Age, Name, Species};
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Dead, Health, Needs, Position};
use clowder::components::magic::Inventory;
use clowder::components::skills::{Corruption, MagicAffinity, Training};
use clowder::persistence;
use clowder::plugins::setup::AppArgs;
use clowder::plugins::simulation::SimulationPlugin;
use clowder::rendering;

use clowder::resources::{
    EventLog, FoodStores, NarrativeLog, NarrativeTier, Relationships, SimConfig, SimRng,
    TemplateRegistry, TimeState, TileMap, WeatherState,
};
use clowder::resources::time::DayPhase;
use clowder::world_gen::colony::{find_colony_site, generate_starting_cats, spawn_starting_buildings};
use clowder::world_gen::terrain::generate_terrain;

/// Parsed CLI arguments.
struct CliArgs {
    seed: u64,
    load_path: Option<PathBuf>,
    headless: bool,
    duration_secs: u64,
    log_path: Option<PathBuf>,
    load_log_path: Option<PathBuf>,
    event_log_path: Option<PathBuf>,
    test_map: bool,
    trace_positions: u64,
    snapshot_interval: u64,
}

fn main() {
    let args = parse_args();

    if args.headless {
        if let Err(e) = run_headless(args) {
            eprintln!("Headless error: {e}");
            std::process::exit(1);
        }
        return;
    }

    App::new()
        .add_plugins(DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Clowder".into(),
                resolution: WindowResolution::new(1280, 720),
                ..Window::default()
            }),
            ..WindowPlugin::default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.15, 0.22, 0.12)))
        .insert_resource(AppArgs {
            seed: args.seed,
            load_path: args.load_path,
            load_log_path: args.load_log_path,
            test_map: args.test_map,
        })
        .add_systems(
            Startup,
            clowder::plugins::setup::setup_world_exclusive,
        )
        .add_plugins(SimulationPlugin)
        .add_plugins(rendering::RenderingPlugin)
        .add_plugins(rendering::CameraPlugin)
        .add_plugins(rendering::ui::UiPlugin)
        .add_systems(Update, (handle_input, sync_sim_speed))
        .run();
}

// ---------------------------------------------------------------------------
// Bevy systems for interactive mode
// ---------------------------------------------------------------------------

/// Basic input handling: quit, pause, speed cycle.
fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<TimeState>,
    mut app_exit: MessageWriter<AppExit>,
    mut inspection: ResMut<InspectionState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if inspection.mode != InspectionMode::None {
            // Dismiss open panels first.
            inspection.mode = InspectionMode::None;
        } else {
            app_exit.write(AppExit::Success);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        time.paused = !time.paused;
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        time.speed = time.speed.cycle();
    }
}

/// Keeps Bevy's FixedUpdate timestep in sync with the SimSpeed setting.
fn sync_sim_speed(
    time_state: Res<TimeState>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    if !time_state.is_changed() {
        return;
    }
    let hz = time_state.speed.ticks_per_second();
    fixed_time.set_timestep(Duration::from_secs_f64(1.0 / hz));
}

// ---------------------------------------------------------------------------
// CLI parsing
// ---------------------------------------------------------------------------

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut seed: u64 = rand::random();
    let mut load_path = None;
    let mut headless = false;
    let mut duration_secs = 60u64;
    let mut log_path = None;
    let mut load_log_path = None;
    let mut event_log_path = None;
    let mut test_map = false;
    let mut trace_positions = 0u64;
    let mut snapshot_interval = 100u64;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--seed" => {
                if let Some(val) = iter.next() {
                    if let Ok(n) = val.parse::<u64>() {
                        seed = n;
                    }
                }
            }
            "--load" => {
                if let Some(path) = iter.next() {
                    load_path = Some(PathBuf::from(path));
                }
            }
            "--headless" => {
                headless = true;
            }
            "--duration" => {
                if let Some(val) = iter.next() {
                    if let Ok(n) = val.parse::<u64>() {
                        duration_secs = n;
                    }
                }
            }
            "--log" => {
                if let Some(path) = iter.next() {
                    log_path = Some(PathBuf::from(path));
                }
            }
            "--load-log" => {
                if let Some(path) = iter.next() {
                    load_log_path = Some(PathBuf::from(path));
                }
            }
            "--event-log" => {
                if let Some(path) = iter.next() {
                    event_log_path = Some(PathBuf::from(path));
                }
            }
            "--test-map" => {
                test_map = true;
            }
            "--trace-positions" => {
                if let Some(val) = iter.next() {
                    if let Ok(n) = val.parse::<u64>() {
                        trace_positions = n;
                    }
                }
            }
            "--snapshot-interval" => {
                if let Some(val) = iter.next() {
                    if let Ok(n) = val.parse::<u64>() {
                        snapshot_interval = n;
                    }
                }
            }
            _ => {}
        }
    }

    if !headless && duration_secs != 60 {
        eprintln!("Warning: --duration has no effect without --headless");
    }

    eprintln!("seed: {seed}");

    CliArgs {
        seed,
        load_path,
        headless,
        duration_secs,
        log_path,
        load_log_path,
        event_log_path,
        test_map,
        trace_positions,
        snapshot_interval,
    }
}

// ---------------------------------------------------------------------------
// Legacy code kept for headless mode and tests
// ---------------------------------------------------------------------------

const SAVE_PATH: &str = "saves/autosave.json";

fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            // World simulation
            (
                clowder::systems::time::advance_time
                    .run_if(clowder::systems::time::not_paused),
                clowder::systems::weather::update_weather,
                clowder::systems::wind::update_wind,
                clowder::systems::time::emit_weather_transitions,
                clowder::systems::magic::corruption_spread,
                clowder::systems::magic::ward_decay,
                clowder::systems::magic::herb_seasonal_check,
                clowder::systems::magic::corruption_tile_effects,
                clowder::systems::magic::spawn_shadow_fox_from_corruption,
                clowder::systems::wildlife::spawn_wildlife,
                clowder::systems::wildlife::wildlife_ai,
                clowder::systems::wildlife::predator_hunt_prey,
                clowder::systems::prey::prey_population,
                clowder::systems::prey::prey_hunger,
                clowder::systems::prey::prey_ai,
                clowder::systems::wildlife::detect_threats,
                clowder::systems::buildings::apply_building_effects,
                clowder::systems::buildings::decay_building_condition,
                clowder::systems::items::decay_items,
            )
                .chain(),
            // Item pruning and food sync — split out to stay under Bevy's chain param limit.
            (
                clowder::systems::items::prune_stored_items,
                clowder::systems::items::sync_food_stores,
            )
                .chain(),
            // Cat needs, mood, and decision-making
            (
                clowder::systems::needs::decay_needs,
                clowder::systems::mood::update_mood,
                clowder::systems::mood::mood_contagion,
                clowder::systems::memory::decay_memories,
                clowder::systems::coordination::evaluate_coordinators,
                clowder::systems::coordination::assess_colony_needs,
                clowder::systems::coordination::accumulate_build_pressure,
            )
                .chain(),
            // Action resolution (disposition system handles all action selection/execution)
            (
                clowder::systems::task_chains::resolve_task_chains,
                clowder::systems::magic::resolve_magic_task_chains,
                clowder::systems::magic::apply_remedy_effects,
                clowder::systems::buildings::process_gates,
                clowder::systems::buildings::tidy_buildings,
            )
                .chain(),
            // Social, combat, death, cleanup, narrative
            (
                clowder::systems::social::passive_familiarity,
                clowder::systems::personality_friction::personality_friction,
                clowder::systems::social::check_bonds,
                clowder::systems::colony_knowledge::update_colony_knowledge,
                clowder::systems::combat::resolve_combat,
                clowder::systems::combat::heal_injuries,
                clowder::systems::magic::personal_corruption_effects,
                clowder::systems::death::check_death,
                clowder::systems::coordination::flag_coordinator_death,
                clowder::systems::coordination::expire_directives,
                clowder::systems::death::cleanup_dead,
                clowder::systems::wildlife::cleanup_wildlife,
                clowder::systems::narrative::generate_narrative,
            )
                .chain(),
        )
            .chain(),
    );
    // Disposition systems — must mirror SimulationPlugin ordering.
    schedule.add_systems(clowder::systems::disposition::check_anxiety_interrupts);
    schedule.add_systems(
        clowder::systems::disposition::evaluate_dispositions
            .after(clowder::systems::disposition::check_anxiety_interrupts),
    );
    // Flush commands so Disposition is visible to disposition_to_chain.
    schedule.add_systems(
        bevy_ecs::schedule::ApplyDeferred
            .after(clowder::systems::disposition::evaluate_dispositions)
            .before(clowder::systems::disposition::disposition_to_chain),
    );
    schedule.add_systems(
        clowder::systems::disposition::disposition_to_chain
            .after(clowder::systems::disposition::evaluate_dispositions),
    );
    // Flush commands so TaskChain is visible to resolve_disposition_chains.
    schedule.add_systems(
        bevy_ecs::schedule::ApplyDeferred
            .after(clowder::systems::disposition::disposition_to_chain)
            .before(clowder::systems::disposition::resolve_disposition_chains),
    );
    schedule.add_systems(
        clowder::systems::disposition::resolve_disposition_chains
            .after(clowder::systems::disposition::disposition_to_chain)
            .before(clowder::systems::task_chains::resolve_task_chains),
    );

    schedule.add_systems(clowder::systems::personality_events::emit_personality_events);
    schedule.add_systems(clowder::systems::ai::emit_periodic_events);
    schedule.add_systems(
        clowder::systems::snapshot::emit_cat_snapshots
            .after(clowder::systems::disposition::resolve_disposition_chains),
    );
    schedule.add_systems(
        clowder::systems::snapshot::emit_position_traces
            .after(clowder::systems::disposition::resolve_disposition_chains),
    );
    // Fate and aspiration lifecycle.
    schedule.add_systems(clowder::systems::fate::assign_fated_connections);
    schedule.add_systems(clowder::systems::fate::awaken_fated_connections);
    schedule.add_systems(clowder::systems::aspirations::select_aspirations);
    schedule.add_systems(clowder::systems::aspirations::check_second_aspiration_slot);
    schedule.add_systems(clowder::systems::aspirations::check_aspiration_abandonment);
    schedule.add_systems(clowder::systems::aspirations::track_milestones);
    schedule
}

fn setup_world(args: &CliArgs) -> io::Result<World> {
    let mut world = if let Some(ref load_path) = args.load_path {
        let save = persistence::load_from_file(load_path)?;
        let mut w = World::new();
        persistence::load_world(&mut w, save);
        w
    } else {
        build_new_world(args.seed, args.test_map)?
    };

    load_templates(&mut world);
    load_zodiac_data(&mut world);
    load_aspiration_data(&mut world);

    if args.load_path.is_none() {
        let current_tick = world.resource::<clowder::resources::TimeState>().tick;
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(
            current_tick,
            "A small group of cats settles in a clearing.".to_string(),
            NarrativeTier::Significant,
        );
    }

    if let Some(ref path) = args.load_log_path {
        load_log_file(&mut world, path)?;
    }

    // Always insert the event log for mechanical debugging.
    world.insert_resource(EventLog::default());

    // Wind state for scent-based hunting.
    if !world.contains_resource::<clowder::resources::wind::WindState>() {
        world.insert_resource(clowder::resources::wind::WindState::default());
    }

    // Snapshot configuration (overridden by CLI flags).
    world.insert_resource(clowder::resources::snapshot_config::SnapshotConfig {
        full_snapshot_interval: args.snapshot_interval,
        position_trace_interval: args.trace_positions,
        economy_interval: args.snapshot_interval,
    });

    // Ensure new resources exist (may be absent from older saves).
    if !world.contains_resource::<clowder::resources::ColonyKnowledge>() {
        world.insert_resource(clowder::resources::ColonyKnowledge::default());
    }
    if !world.contains_resource::<clowder::resources::ColonyPriority>() {
        world.insert_resource(clowder::resources::ColonyPriority::default());
    }
    if !world.contains_resource::<clowder::systems::wildlife::DetectionCooldowns>() {
        world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    }

    Ok(world)
}

// ---------------------------------------------------------------------------
// Headless mode
// ---------------------------------------------------------------------------

fn run_headless(args: CliArgs) -> io::Result<()> {
    let mut world = setup_world(&args)?;
    let mut schedule = build_schedule();

    // Ensure the simulation is unpaused.
    {
        let mut time = world.resource_mut::<TimeState>();
        time.paused = false;
    }

    // Resolve log paths.
    let log_path = args.log_path.unwrap_or_else(|| PathBuf::from("logs/narrative.jsonl"));
    let event_log_path = args.event_log_path.unwrap_or_else(|| PathBuf::from("logs/events.jsonl"));
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = event_log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(&log_path)?;
    let mut event_file = std::fs::File::create(&event_log_path)?;
    writeln!(
        file,
        "{}",
        serde_json::json!({"_header": true, "seed": args.seed, "duration_secs": args.duration_secs})
    )?;
    writeln!(
        event_file,
        "{}",
        serde_json::json!({"_header": true, "seed": args.seed, "duration_secs": args.duration_secs})
    )?;

    let duration = Duration::from_secs(args.duration_secs);
    let start = Instant::now();
    let mut ticks: u64 = 0;
    let mut last_flushed: u64 = 0;
    let mut last_events_flushed: u64 = 0;

    // Flush any entries already present (e.g. the initial narrative entry).
    flush_new_entries(&world, &mut file, &mut last_flushed)?;

    while start.elapsed() < duration {
        schedule.run(&mut world);
        ticks += 1;
        flush_new_entries(&world, &mut file, &mut last_flushed)?;
        flush_event_entries(&world, &mut event_file, &mut last_events_flushed)?;

        if ticks.is_multiple_of(1000) {
            let elapsed = start.elapsed().as_secs();
            let time = world.resource::<TimeState>();
            let config = world.resource::<SimConfig>();
            eprint!(
                "\r  [{elapsed}s] tick {} (sim day {})    ",
                time.tick,
                TimeState::day_number(time.tick, config),
            );
        }

        // Stop early if all cats are dead.
        let alive = world
            .query_filtered::<(), (
                bevy_ecs::prelude::With<Species>,
                bevy_ecs::prelude::Without<Dead>,
            )>()
            .iter(&world)
            .count();
        if alive == 0 {
            let time = world.resource::<TimeState>();
            eprintln!("\n  All cats dead at tick {}. Ending early.", time.tick);
            break;
        }
    }

    // Final flush.
    flush_new_entries(&world, &mut file, &mut last_flushed)?;
    flush_event_entries(&world, &mut event_file, &mut last_events_flushed)?;

    let time = world.resource::<TimeState>();
    let config = world.resource::<SimConfig>();
    eprintln!(
        "\nHeadless complete: {} schedule runs, sim day {}, {} narrative / {} event entries",
        ticks,
        TimeState::day_number(time.tick, config),
        last_flushed,
        last_events_flushed,
    );
    eprintln!("  narrative → {}", log_path.display());
    eprintln!("  events    → {}", event_log_path.display());

    // Autosave.
    let save_path = std::path::Path::new(SAVE_PATH);
    if let Err(e) = persistence::save_to_file(&mut world, save_path) {
        eprintln!("Warning: failed to autosave: {e}");
    }

    Ok(())
}

fn flush_new_entries(
    world: &World,
    file: &mut std::fs::File,
    last_flushed: &mut u64,
) -> io::Result<()> {
    let log = world.resource::<NarrativeLog>();
    let config = world.resource::<SimConfig>();
    let new_count = log.total_pushed.saturating_sub(*last_flushed);
    if new_count == 0 {
        return Ok(());
    }
    let start = log.entries.len().saturating_sub(new_count as usize);
    for entry in log.entries.range(start..) {
        let day = TimeState::day_number(entry.tick, config);
        let phase = DayPhase::from_tick(entry.tick, config);
        let tier_label = match entry.tier {
            NarrativeTier::Micro => "Micro",
            NarrativeTier::Action => "Action",
            NarrativeTier::Significant => "Significant",
        };
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "tick": entry.tick,
                "day": day,
                "phase": phase.label(),
                "tier": tier_label,
                "text": entry.text,
            })
        )?;
    }
    *last_flushed = log.total_pushed;
    Ok(())
}

fn flush_event_entries(
    world: &World,
    file: &mut std::fs::File,
    last_flushed: &mut u64,
) -> io::Result<()> {
    let log = world.resource::<EventLog>();
    let new_count = log.total_pushed.saturating_sub(*last_flushed);
    if new_count == 0 {
        return Ok(());
    }
    let start = log.entries.len().saturating_sub(new_count as usize);
    for entry in log.entries.range(start..) {
        writeln!(
            file,
            "{}",
            serde_json::to_string(entry).unwrap_or_default()
        )?;
    }
    *last_flushed = log.total_pushed;
    Ok(())
}

fn load_log_file(world: &mut World, path: &std::path::Path) -> io::Result<()> {
    let file = std::fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut loaded = 0u64;
    for line in reader.lines() {
        let line = line?;
        let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("bad JSON in log: {e}"))
        })?;
        // Skip header lines.
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

// ---------------------------------------------------------------------------
// World construction helpers
// ---------------------------------------------------------------------------

fn build_new_world(seed: u64, test_map: bool) -> io::Result<World> {
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let mut sim_rng = SimRng::new(seed);

    // Generate terrain.
    let mut map = if test_map {
        clowder::world_gen::test_map::generate_test_map()
    } else {
        generate_terrain(120, 90, &mut sim_rng.rng)
    };

    // Set initial corruption and mystery on special tiles.
    clowder::world_gen::herbs::initialize_tile_magic(&mut map, &mut sim_rng.rng);

    // Find colony site.
    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    // Start the clock high enough that cats can have varied ages.
    let start_tick: u64 = 100_000;

    let cat_blueprints = generate_starting_cats(
        8,
        start_tick,
        config.ticks_per_season,
        &mut sim_rng.rng,
    );

    // Build ECS world.
    let mut world = World::new();

    // Spawn starting buildings (sets terrain tiles and creates entities).
    spawn_starting_buildings(&mut world, colony_site, &mut map);

    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: clowder::resources::SimSpeed::Normal,
    });
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(clowder::resources::time::TransitionTracker::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(clowder::resources::ColonyKnowledge::default());
    world.insert_resource(clowder::resources::ColonyPriority::default());
    world.insert_resource(FoodStores::default());
    world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    // Spawn cats.
    let cat_count = cat_blueprints.len();
    let mut entity_ids: Vec<Entity> = Vec::with_capacity(cat_count);
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

        let entity = world.spawn((
            (
                Name(cat.name),
                Species,
                Age { born_tick: cat.born_tick },
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
                CurrentAction::default(),
                Inventory::default(),
            ),
        )).id();
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
    clowder::systems::wildlife::spawn_initial_wildlife(&mut world, colony_site);

    // Spawn initial prey animals across their habitats.
    clowder::systems::prey::spawn_initial_prey(&mut world);

    // Spawn herbs based on terrain and current season.
    let current_season = {
        let time = world.resource::<TimeState>();
        let config = world.resource::<SimConfig>();
        time.season(config)
    };
    clowder::world_gen::herbs::spawn_herbs(&mut world, current_season);

    Ok(world)
}

fn load_aspiration_data(world: &mut World) {
    let path = std::path::Path::new("assets/narrative/aspirations");
    match clowder::resources::AspirationRegistry::load_from_dir(path) {
        Ok(registry) => {
            world.insert_resource(registry);
        }
        Err(e) => {
            eprintln!("Warning: failed to load aspiration data: {e}");
        }
    }
}

fn load_zodiac_data(world: &mut World) {
    let path = std::path::Path::new("assets/data/zodiac.ron");
    match clowder::resources::ZodiacData::load(path) {
        Ok(data) => {
            world.insert_resource(data);
        }
        Err(e) => {
            eprintln!("Warning: failed to load zodiac data: {e}");
        }
    }
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
