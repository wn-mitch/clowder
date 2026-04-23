use std::io::{self, BufRead, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::app::AppExit;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::{
    App, ButtonInput, ClearColor, Color, DefaultPlugins, Fixed, ImagePlugin, MessageWriter,
    PluginGroup, Res, ResMut, Startup, Time, Update, Window, WindowPlugin,
};
use bevy::window::WindowResolution;

use clowder::ui_data::{InspectionMode, InspectionState};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::CurrentAction;
use clowder::components::hunting_priors::HuntingPriors;
use clowder::components::identity::{Age, Name, Species};
use clowder::components::magic::{Inventory, Ward};
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Dead, Health, Needs, Position};
use clowder::components::skills::{Corruption, MagicAffinity, Training};
use clowder::persistence;
use clowder::plugins::setup::AppArgs;
use clowder::plugins::simulation::SimulationPlugin;
use clowder::rendering;

use clowder::resources::system_activation::{Feature, FeatureCategory, SystemActivation};
use clowder::resources::time::DayPhase;
use clowder::resources::weather::Weather;
use clowder::resources::{
    ColonyHuntingMap, EventLog, FoodStores, ForcedConditions, NarrativeLog, NarrativeTier,
    Relationships, SimConfig, SimRng, TemplateRegistry, TileMap, TimeState, WeatherState,
};
use clowder::world_gen::colony::{
    find_colony_site, generate_starting_cats, spawn_starting_buildings,
};
use clowder::world_gen::custom_cats::load_custom_cats;
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
    force_weather: Option<Weather>,
    /// Per §11.5 — name of the focal cat for trace-record emission.
    /// When `Some`, a `FocalTraceTarget` resource is inserted and
    /// `logs/trace-<focal>.jsonl` receives layer-by-layer records.
    /// Default focal cat is resolved deterministically from seed on
    /// the first tick if the flag is omitted.
    focal_cat: Option<String>,
    trace_log_path: Option<PathBuf>,
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
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Clowder".into(),
                        resolution: WindowResolution::new(1280, 720),
                        ..Window::default()
                    }),
                    ..WindowPlugin::default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.15, 0.22, 0.12)))
        .insert_resource(AppArgs {
            seed: args.seed,
            load_path: args.load_path,
            load_log_path: args.load_log_path,
            test_map: args.test_map,
        })
        .add_systems(Startup, clowder::plugins::setup::setup_world_exclusive)
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
fn sync_sim_speed(time_state: Res<TimeState>, mut fixed_time: ResMut<Time<Fixed>>) {
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
    let mut duration_secs = 600u64;
    let mut log_path = None;
    let mut load_log_path = None;
    let mut event_log_path = None;
    let mut test_map = false;
    let mut trace_positions = 0u64;
    let mut snapshot_interval = 100u64;
    let mut force_weather: Option<Weather> = None;
    let mut focal_cat: Option<String> = None;
    let mut trace_log_path: Option<PathBuf> = None;
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
            "--force-weather" => {
                let Some(val) = iter.next() else {
                    eprintln!("Error: --force-weather requires a value");
                    std::process::exit(2);
                };
                force_weather = Some(parse_weather(val).unwrap_or_else(|| {
                    eprintln!(
                        "Error: --force-weather: unknown variant {val:?}. \
                         Expected one of: clear, overcast, light-rain, heavy-rain, \
                         snow, fog, wind, storm"
                    );
                    std::process::exit(2);
                }));
            }
            "--focal-cat" => {
                if let Some(name) = iter.next() {
                    focal_cat = Some(name.clone());
                }
            }
            "--trace-log" => {
                if let Some(path) = iter.next() {
                    trace_log_path = Some(PathBuf::from(path));
                }
            }
            _ => {}
        }
    }

    if !headless && duration_secs != 600 {
        eprintln!("Warning: --duration has no effect without --headless");
    }
    if !headless && force_weather.is_some() {
        eprintln!("Warning: --force-weather has no effect without --headless");
    }
    if !headless && (focal_cat.is_some() || trace_log_path.is_some()) {
        eprintln!("Warning: --focal-cat / --trace-log have no effect without --headless");
    }

    eprintln!("seed: {seed}");
    if let Some(w) = force_weather {
        eprintln!("forced weather: {}", w.label());
    }

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
        force_weather,
        focal_cat,
        trace_log_path,
    }
}

/// Serialize every (weather × phase × terrain) × channel multiplier into a
/// structured header block. Two sweeps are semantically comparable iff their
/// diff against this block is understood. Without this snapshot a Phase 5b
/// activation (which only edits inline `1.0` returns) would be invisible in
/// the event-log header — the constants-hash rule wouldn't flag it because
/// the values live in enum methods, not `SimConstants`.
fn sensory_env_multipliers_snapshot() -> serde_json::Value {
    use clowder::resources::map::Terrain;

    let weather_variants = [
        Weather::Clear,
        Weather::Overcast,
        Weather::LightRain,
        Weather::HeavyRain,
        Weather::Snow,
        Weather::Fog,
        Weather::Wind,
        Weather::Storm,
    ];
    let phase_variants = [DayPhase::Dawn, DayPhase::Day, DayPhase::Dusk, DayPhase::Night];

    let weather_block: serde_json::Map<String, serde_json::Value> = weather_variants
        .iter()
        .map(|w| {
            (
                w.label().to_string(),
                serde_json::json!({
                    "sight": w.sight_multiplier(),
                    "hearing": w.hearing_multiplier(),
                    "scent": w.scent_multiplier(),
                    "tremor": w.tremor_multiplier(),
                }),
            )
        })
        .collect();

    let phase_block: serde_json::Map<String, serde_json::Value> = phase_variants
        .iter()
        .map(|p| {
            (
                p.label().to_string(),
                serde_json::json!({
                    "sight": p.sight_multiplier(),
                    "hearing": p.hearing_multiplier(),
                    "scent": p.scent_multiplier(),
                    "tremor": p.tremor_multiplier(),
                }),
            )
        })
        .collect();

    let terrain_block: serde_json::Map<String, serde_json::Value> = Terrain::ALL
        .iter()
        .map(|t| {
            (
                format!("{t:?}"),
                serde_json::json!({
                    "occludes_sight": t.occludes_sight(),
                    "tremor_transmission": t.tremor_transmission(),
                }),
            )
        })
        .collect();

    serde_json::json!({
        "weather": weather_block,
        "day_phase": phase_block,
        "terrain": terrain_block,
    })
}

/// Accept kebab-case or space-case weather names.
fn parse_weather(s: &str) -> Option<Weather> {
    let key = s.to_ascii_lowercase().replace('_', "-");
    match key.as_str() {
        "clear" => Some(Weather::Clear),
        "overcast" => Some(Weather::Overcast),
        "light-rain" | "lightrain" => Some(Weather::LightRain),
        "heavy-rain" | "heavyrain" => Some(Weather::HeavyRain),
        "snow" => Some(Weather::Snow),
        "fog" => Some(Weather::Fog),
        "wind" => Some(Weather::Wind),
        "storm" => Some(Weather::Storm),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Legacy code kept for headless mode and tests
// ---------------------------------------------------------------------------

const SAVE_PATH: &str = "saves/autosave.json";

fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    // Message buffer flush — must run before systems that read messages.
    schedule.add_systems(bevy_ecs::message::message_update_system);
    schedule.add_systems(
        (
            // World simulation
            (
                clowder::systems::time::advance_time.run_if(clowder::systems::time::not_paused),
                clowder::systems::weather::update_weather,
                clowder::systems::wind::update_wind,
                clowder::systems::time::emit_weather_transitions,
                clowder::systems::magic::corruption_spread,
                clowder::systems::magic::ward_decay,
                (
                    clowder::systems::magic::herb_seasonal_check,
                    clowder::systems::magic::advance_herb_growth,
                    clowder::systems::magic::advance_flavor_growth,
                    clowder::systems::magic::herb_regrowth,
                )
                    .chain(),
                clowder::systems::magic::corruption_tile_effects,
                clowder::systems::magic::apply_corruption_pushback,
                clowder::systems::magic::spawn_shadow_fox_from_corruption,
                (
                    clowder::systems::wildlife::spawn_wildlife,
                    clowder::systems::wildlife::wildlife_ai,
                    clowder::systems::wildlife::fox_movement,
                    clowder::systems::wildlife::fox_needs_tick,
                    clowder::systems::fox_goap::sync_fox_needs,
                    clowder::systems::fox_goap::fox_evaluate_and_plan,
                    clowder::systems::fox_goap::fox_resolve_goap_plans,
                    clowder::systems::fox_goap::feed_cubs_at_dens,
                    clowder::systems::fox_goap::resolve_paired_confrontations,
                    clowder::systems::wildlife::fox_ai_decision,
                    clowder::systems::wildlife::fox_scent_tick,
                    clowder::systems::wildlife::predator_hunt_prey,
                    clowder::systems::wildlife::carcass_decay,
                    clowder::systems::wildlife::predator_stalk_cats,
                )
                    .chain(),
                clowder::systems::prey::prey_population,
                clowder::systems::prey::prey_hunger,
                clowder::systems::prey::prey_ai,
                clowder::systems::prey::prey_scent_tick,
                clowder::systems::prey::prey_den_lifecycle,
                clowder::systems::wildlife::detect_threats,
                clowder::systems::buildings::apply_building_effects,
                clowder::systems::buildings::decay_building_condition,
                clowder::systems::items::decay_items,
            )
                .chain(),
            // Item pruning, food sync, den pressure — split to stay under chain param limit.
            (
                clowder::systems::items::prune_stored_items,
                clowder::systems::items::sync_food_stores,
                clowder::systems::prey::update_den_pressure,
                clowder::systems::prey::apply_den_raids,
                clowder::systems::prey::orphan_prey_adopt_or_found,
            )
                .chain(),
            // Cat needs, mood, and decision-making
            (
                clowder::systems::needs::decay_needs,
                // §4.3 Incapacitated marker author — mirror of
                // SimulationPlugin::build Chain 2.
                clowder::systems::incapacitation::update_incapacitation,
                clowder::systems::needs::decay_grooming,
                clowder::systems::needs::eat_from_inventory,
                clowder::systems::needs::decay_exploration,
                clowder::systems::needs::bond_proximity_social,
                clowder::systems::pregnancy::tick_pregnancy,
                // Fertility transitions (§7.M.7) — mirrored from
                // SimulationPlugin::build.
                clowder::systems::fertility::handle_post_partum_reinsert,
                clowder::systems::fertility::update_fertility_phase,
                clowder::systems::growth::tick_kitten_growth,
                clowder::systems::growth::kitten_mood_aura,
                clowder::systems::mood::update_mood,
                clowder::systems::mood::mood_contagion,
                clowder::systems::mood::bond_proximity_mood,
                clowder::systems::memory::decay_memories,
                clowder::systems::coordination::evaluate_coordinators,
                clowder::systems::coordination::assess_colony_needs,
                clowder::systems::coordination::dispatch_urgent_directives,
                clowder::systems::coordination::accumulate_build_pressure,
                clowder::systems::coordination::spawn_construction_sites,
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
                clowder::systems::wildlife::fox_lifecycle_tick,
                clowder::systems::wildlife::fox_confrontation_tick,
                clowder::systems::wildlife::fox_store_raid_tick,
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
    // GOAP systems — must mirror SimulationPlugin ordering.
    // check_anxiety_interrupts and evaluate_and_plan run after sync_food_stores
    // so food_available reflects actual item state this tick, not the default 0.0.
    schedule.add_systems(
        clowder::systems::goap::check_anxiety_interrupts
            .after(clowder::systems::items::sync_food_stores),
    );
    schedule.add_systems(
        clowder::systems::goap::evaluate_and_plan
            .after(clowder::systems::goap::check_anxiety_interrupts)
            .after(clowder::systems::items::sync_food_stores),
    );
    schedule.add_systems(
        bevy_ecs::schedule::ApplyDeferred
            .after(clowder::systems::goap::evaluate_and_plan)
            .before(clowder::systems::goap::resolve_goap_plans),
    );
    schedule.add_systems(
        clowder::systems::goap::resolve_goap_plans
            .after(clowder::systems::goap::evaluate_and_plan)
            .before(clowder::systems::task_chains::resolve_task_chains),
    );
    schedule.add_systems(
        clowder::systems::goap::emit_plan_narrative
            .after(clowder::systems::goap::resolve_goap_plans),
    );

    schedule.add_systems(clowder::systems::disposition::cat_presence_tick);
    schedule.add_systems(clowder::systems::personality_events::emit_personality_events);
    schedule.add_systems(clowder::systems::ai::emit_periodic_events);
    schedule.add_systems(
        clowder::systems::snapshot::emit_cat_snapshots
            .after(clowder::systems::goap::resolve_goap_plans),
    );
    // §11 trace emitter — headless-only. Run_if gate keeps the system
    // dormant unless FocalTraceTarget is inserted (interactive builds
    // never insert it). Runs after resolve_goap_plans so last_scores
    // reflects the current tick's evaluation.
    schedule.add_systems(
        clowder::systems::trace_emit::emit_focal_trace
            .after(clowder::systems::goap::resolve_goap_plans)
            .run_if(bevy_ecs::prelude::resource_exists::<
                clowder::resources::FocalTraceTarget,
            >)
            .run_if(bevy_ecs::prelude::resource_exists::<
                clowder::resources::TraceLog,
            >)
            .run_if(bevy_ecs::prelude::resource_exists::<
                clowder::resources::FocalScoreCapture,
            >),
    );
    schedule.add_systems(
        clowder::systems::snapshot::emit_position_traces
            .after(clowder::systems::goap::resolve_goap_plans),
    );
    schedule.add_systems(clowder::systems::snapshot::emit_spatial_snapshots);
    schedule.add_systems(clowder::systems::colony_score::emit_colony_score);
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
        ..Default::default()
    });

    // Ensure new resources exist (may be absent from older saves).
    if !world.contains_resource::<clowder::resources::ColonyKnowledge>() {
        world.insert_resource(clowder::resources::ColonyKnowledge::default());
    }
    if !world.contains_resource::<clowder::resources::ColonyPriority>() {
        world.insert_resource(clowder::resources::ColonyPriority::default());
    }
    if !world.contains_resource::<ColonyHuntingMap>() {
        world.insert_resource(ColonyHuntingMap::default());
    }
    if !world.contains_resource::<clowder::resources::ExplorationMap>() {
        world.insert_resource(clowder::resources::ExplorationMap::default());
    }
    if !world.contains_resource::<clowder::systems::wildlife::DetectionCooldowns>() {
        world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    }
    if !world.contains_resource::<clowder::species::SpeciesRegistry>() {
        world.insert_resource(clowder::species::build_registry());
    }
    if !world.contains_resource::<clowder::components::prey::PreyDensity>() {
        world.insert_resource(clowder::components::prey::PreyDensity::default());
    }
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
    if !world.contains_resource::<clowder::resources::ColonyScore>() {
        world.insert_resource(clowder::resources::ColonyScore::default());
    }
    if !world.contains_resource::<clowder::resources::SimConstants>() {
        world.insert_resource(clowder::resources::SimConstants::default());
    }
    if !world.contains_resource::<clowder::resources::SystemActivation>() {
        world.insert_resource(clowder::resources::SystemActivation::default());
    }
    if !world.contains_resource::<clowder::resources::ForcedConditions>() {
        world.insert_resource(clowder::resources::ForcedConditions::default());
    }
    // L2 substrate resources + DSE registrations. Manual mirror of
    // `SimulationPlugin::build()` — both load paths (fresh world below
    // and save-load path here) must match.
    if !world.contains_resource::<clowder::ai::faction::FactionRelations>() {
        world.insert_resource(clowder::ai::faction::FactionRelations::canonical());
    }
    if !world.contains_resource::<clowder::ai::eval::DseRegistry>() {
        let scoring = world
            .resource::<clowder::resources::SimConstants>()
            .scoring
            .clone();
        let mut registry = clowder::ai::eval::DseRegistry::new();
        registry.cat_dses.push(clowder::ai::dses::eat_dse());
        registry.cat_dses.push(clowder::ai::dses::hunt_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::hunt_target_dse());
        registry.cat_dses.push(clowder::ai::dses::forage_dse());
        registry.cat_dses.push(clowder::ai::dses::cook_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::flee_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::fight_dse(&scoring));
        registry
            .target_taking_dses
            .push(clowder::ai::dses::fight_target_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::sleep_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::idle_dse(&scoring));
        registry.cat_dses.push(clowder::ai::dses::socialize_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::socialize_target_dse());
        registry.cat_dses.push(clowder::ai::dses::groom_self_dse());
        registry.cat_dses.push(clowder::ai::dses::groom_other_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::groom_other_target_dse());
        registry.cat_dses.push(clowder::ai::dses::mentor_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::mentor_target_dse());
        registry.cat_dses.push(clowder::ai::dses::caretake_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::caretake_target_dse());
        registry.cat_dses.push(clowder::ai::dses::mate_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::mate_target_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::patrol_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::build_dse(&scoring));
        registry
            .target_taking_dses
            .push(clowder::ai::dses::build_target_dse());
        registry.cat_dses.push(clowder::ai::dses::farm_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::coordinate_dse(&scoring));
        registry.cat_dses.push(clowder::ai::dses::explore_dse());
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
            .target_taking_dses
            .push(clowder::ai::dses::apply_remedy_target_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::herbcraft_ward_dse());
        registry.cat_dses.push(clowder::ai::dses::scry_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::durable_ward_dse());
        registry.cat_dses.push(clowder::ai::dses::cleanse_dse());
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
        registry.fox_dses.push(clowder::ai::dses::fox_avoiding_dse());
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
    // §3.5 modifier pipeline — headless runs with live `SimConstants`
    // values loaded from TOML, so pass them in rather than using
    // `ScoringConstants::default()`. Unconditional insert: if an older
    // pipeline resource exists (e.g. from a save-load hand-off), this
    // replaces it with the current-constants build.
    {
        let scoring = world
            .resource::<clowder::resources::SimConstants>()
            .scoring
            .clone();
        world.insert_resource(clowder::ai::modifier::default_modifier_pipeline(&scoring));
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

    // Apply diagnostic overrides before the first schedule tick runs.
    if let Some(w) = args.force_weather {
        let mut forced = world.resource_mut::<ForcedConditions>();
        forced.weather = Some(w);
        // Pin the current weather immediately so tick-0 readers don't see the default.
        let mut weather = world.resource_mut::<WeatherState>();
        weather.current = w;
    }

    // Resolve log paths.
    let log_path = args
        .log_path
        .unwrap_or_else(|| PathBuf::from("logs/narrative.jsonl"));
    let event_log_path = args
        .event_log_path
        .unwrap_or_else(|| PathBuf::from("logs/events.jsonl"));
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = event_log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Trace sidecar is gated on --focal-cat per §11.5 — headless-only,
    // opt-in. Path defaults to logs/trace-<focal>.jsonl; --trace-log
    // overrides. When --focal-cat is absent, no file is opened and no
    // FocalTraceTarget resource is inserted, so trace systems remain
    // dormant.
    let trace_log_path = args.focal_cat.as_ref().map(|name| {
        args.trace_log_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("logs/trace-{name}.jsonl")))
    });
    if let Some(ref tp) = trace_log_path {
        if let Some(parent) = tp.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut file = std::fs::File::create(&log_path)?;
    let mut event_file = std::fs::File::create(&event_log_path)?;
    let mut trace_file = trace_log_path
        .as_ref()
        .map(std::fs::File::create)
        .transpose()?;
    let commit_hash = env!("GIT_HASH");
    let commit_hash_short = env!("GIT_HASH_SHORT");
    let commit_time = env!("GIT_COMMIT_TIME");
    let commit_dirty = env!("GIT_DIRTY") == "true";
    writeln!(
        file,
        "{}",
        serde_json::json!({
            "_header": true,
            "seed": args.seed,
            "duration_secs": args.duration_secs,
            "commit_hash": commit_hash,
            "commit_hash_short": commit_hash_short,
            "commit_dirty": commit_dirty,
            "commit_time": commit_time,
        })
    )?;
    let constants_json = {
        let c = world.resource::<clowder::resources::SimConstants>();
        serde_json::to_value(c.clone()).unwrap_or_default()
    };
    let sim_config_json = serde_json::to_value(world.resource::<SimConfig>().clone())
        .unwrap_or_default();
    let forced_weather_json = args.force_weather.map(|w| w.label());
    let sensory_env_multipliers_json = sensory_env_multipliers_snapshot();
    let (map_width, map_height) = {
        let tm = world.resource::<clowder::resources::map::TileMap>();
        (tm.width, tm.height)
    };
    writeln!(
        event_file,
        "{}",
        serde_json::json!({
            "_header": true,
            "seed": args.seed,
            "duration_secs": args.duration_secs,
            "commit_hash": commit_hash,
            "commit_hash_short": commit_hash_short,
            "commit_dirty": commit_dirty,
            "commit_time": commit_time,
            "sim_config": sim_config_json,
            "map_width": map_width,
            "map_height": map_height,
            "constants": constants_json,
            "forced_weather": forced_weather_json,
            "sensory_env_multipliers": sensory_env_multipliers_json,
        })
    )?;

    // Trace sidecar header — §11.4 joinability invariant: shares
    // commit_hash + sim_config + constants fields with events.jsonl so
    // the two files diff-lock as a pair. A trace from one run is
    // comparable to another only when both sidecar and events headers
    // agree.
    if let (Some(ref mut trace_file), Some(ref focal_name)) =
        (trace_file.as_mut(), args.focal_cat.as_ref())
    {
        writeln!(
            trace_file,
            "{}",
            serde_json::json!({
                "_header": true,
                "focal_cat": focal_name,
                "seed": args.seed,
                "duration_secs": args.duration_secs,
                "commit_hash": commit_hash,
                "commit_hash_short": commit_hash_short,
                "commit_dirty": commit_dirty,
                "commit_time": commit_time,
                "sim_config": sim_config_json,
                "map_width": map_width,
                "map_height": map_height,
                "constants": constants_json,
                "forced_weather": forced_weather_json,
                "sensory_env_multipliers": sensory_env_multipliers_json,
            })
        )?;

        // Insert the FocalTraceTarget resource. Entity resolution is
        // lazy — the trace emitters look up the cat by name on each
        // tick until they find it (or the cat dies / never existed).
        world.insert_resource(clowder::resources::FocalTraceTarget {
            name: focal_name.to_string(),
            entity: None,
        });
        world.insert_resource(clowder::resources::TraceLog::default());
        // §11 rich-trace capture sink. Populated by `score_dse_by_id`
        // + `select_disposition_via_intention_softmax_with_trace` for
        // the focal cat each tick; drained by `emit_focal_trace` at
        // the end of the tick. Insertion gates the `evaluate_and_plan`
        // / `cat_presence_tick` trace code paths via `Option<Res<_>>`
        // checks — when the resource is absent those paths take the
        // zero-capture branch unconditionally.
        world.insert_resource(clowder::resources::FocalScoreCapture::default());
    }

    let duration = Duration::from_secs(args.duration_secs);
    let start = Instant::now();
    let mut ticks: u64 = 0;
    let mut last_flushed: u64 = 0;
    let mut last_events_flushed: u64 = 0;
    let mut last_trace_flushed: u64 = 0;

    // Flush any entries already present (e.g. the initial narrative entry).
    flush_new_entries(&world, &mut file, &mut last_flushed)?;

    while start.elapsed() < duration {
        schedule.run(&mut world);
        ticks += 1;
        flush_new_entries(&world, &mut file, &mut last_flushed)?;
        flush_event_entries(&world, &mut event_file, &mut last_events_flushed)?;
        if let Some(ref mut tf) = trace_file {
            flush_trace_entries(&world, tf, &mut last_trace_flushed)?;
        }

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
    if let Some(ref mut tf) = trace_file {
        flush_trace_entries(&world, tf, &mut last_trace_flushed)?;
    }

    // Emit end-of-sim diagnostic footer to the event log. This is the
    // machine-readable summary that downstream tooling (baseline diffs,
    // tuning reports) reads to compare runs. Keep the schema stable.
    let footer = build_headless_footer(&mut world);
    writeln!(event_file, "{footer}")?;

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
    if let Some(ref tp) = trace_log_path {
        eprintln!(
            "  trace     → {}  ({} records)",
            tp.display(),
            last_trace_flushed
        );
    }
    print_headless_summary(&footer);

    // Autosave.
    let save_path = std::path::Path::new(SAVE_PATH);
    if let Err(e) = persistence::save_to_file(&mut world, save_path) {
        eprintln!("Warning: failed to autosave: {e}");
    }

    Ok(())
}

/// Build the end-of-sim diagnostic footer as a JSON string.
///
/// Pulls cumulative tallies from [`EventLog`] and [`SystemActivation`], plus a
/// live query of surviving [`Ward`] entities for count and average strength.
/// This is the structured summary that gets appended to `events.jsonl` and
/// is the source of truth for cross-run comparisons.
fn build_headless_footer(world: &mut World) -> String {
    let (ward_count_final, ward_avg_strength_final) = {
        let mut q = world.query::<&Ward>();
        let (count, sum) = q
            .iter(world)
            .fold((0u64, 0.0f32), |(c, s), w| (c + 1, s + w.strength));
        let avg = if count == 0 { 0.0 } else { sum / count as f32 };
        (count, avg)
    };

    let activation = world.resource::<SystemActivation>();
    let feature_count = |f: Feature| activation.counts.get(&f).copied().unwrap_or(0);
    let wards_placed_total = feature_count(Feature::WardPlaced);
    let wards_despawned_total = feature_count(Feature::WardDespawned);
    let shadow_foxes_avoided_ward_total = feature_count(Feature::ShadowFoxAvoidedWard);
    let ward_siege_started_total = feature_count(Feature::WardSiegeStarted);
    let shadow_fox_spawn_total = feature_count(Feature::ShadowFoxSpawn);
    let anxiety_interrupt_total = feature_count(Feature::AnxietyInterrupt);

    let positive_features_active = activation.features_active_in(FeatureCategory::Positive);
    let positive_features_total = SystemActivation::features_total_in(FeatureCategory::Positive);
    let negative_events_total = activation.negative_event_count();
    let neutral_features_active = activation.features_active_in(FeatureCategory::Neutral);
    let neutral_features_total = SystemActivation::features_total_in(FeatureCategory::Neutral);
    // §Phase 5a never-fired canary: Positive features that a
    // canonical soak is *expected* to fire but didn't. Empty list
    // on a healthy run; populated list means silently-dead
    // subsystems (the farming bug's diagnostic blind spot before
    // §Phase 4c.4).
    let never_fired_expected_positives = activation.never_fired_expected_positives();

    let event_log = world.resource::<EventLog>();
    let footer = serde_json::json!({
        "_footer": true,
        "wards_placed_total": wards_placed_total,
        "wards_despawned_total": wards_despawned_total,
        "ward_count_final": ward_count_final,
        "ward_avg_strength_final": ward_avg_strength_final,
        "shadow_foxes_avoided_ward_total": shadow_foxes_avoided_ward_total,
        "ward_siege_started_total": ward_siege_started_total,
        "shadow_fox_spawn_total": shadow_fox_spawn_total,
        "anxiety_interrupt_total": anxiety_interrupt_total,
        "positive_features_active": positive_features_active,
        "positive_features_total": positive_features_total,
        "negative_events_total": negative_events_total,
        "neutral_features_active": neutral_features_active,
        "neutral_features_total": neutral_features_total,
        "never_fired_expected_positives": never_fired_expected_positives,
        "deaths_by_cause": event_log.deaths_by_cause,
        "plan_failures_by_reason": event_log.plan_failures_by_reason,
        "interrupts_by_reason": event_log.interrupts_by_reason,
        "continuity_tallies": event_log.continuity_tallies,
    });
    footer.to_string()
}

/// Pretty-print the footer summary to stderr for the operator running the sim.
///
/// Reads the JSON footer back out as a Value so the set of printed fields
/// stays aligned with the file schema — if a new field is added to
/// [`build_headless_footer`], it shows up here automatically (for maps) or
/// gets a one-liner (for scalars).
fn print_headless_summary(footer: &str) {
    let v: serde_json::Value = match serde_json::from_str(footer) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("  (footer parse failed: {e})");
            return;
        }
    };
    eprintln!("\n== Diagnostic footer ==");
    let scalar_fields = [
        "wards_placed_total",
        "wards_despawned_total",
        "ward_count_final",
        "ward_avg_strength_final",
        "shadow_foxes_avoided_ward_total",
        "ward_siege_started_total",
        "shadow_fox_spawn_total",
        "anxiety_interrupt_total",
        "positive_features_active",
        "positive_features_total",
        "negative_events_total",
        "neutral_features_active",
        "neutral_features_total",
    ];
    for key in scalar_fields {
        if let Some(val) = v.get(key) {
            eprintln!("  {key}: {val}");
        }
    }
    for key in [
        "deaths_by_cause",
        "plan_failures_by_reason",
        "interrupts_by_reason",
        "continuity_tallies",
    ] {
        let Some(map) = v.get(key).and_then(|x| x.as_object()) else {
            continue;
        };
        if map.is_empty() {
            eprintln!("  {key}: (none)");
            continue;
        }
        eprintln!("  {key}:");
        let mut entries: Vec<_> = map.iter().collect();
        // continuity_tallies prints by fixed key order so readers can
        // eyeball the canary set at a glance; others sort by descending count.
        if key == "continuity_tallies" {
            let order = [
                "grooming",
                "play",
                "mentoring",
                "burial",
                "courtship",
                "mythic-texture",
            ];
            let mut idx: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for (i, k) in order.iter().enumerate() {
                idx.insert(k, i);
            }
            entries.sort_by_key(|(k, _)| idx.get(k.as_str()).copied().unwrap_or(usize::MAX));
        } else {
            entries.sort_by(|a, b| b.1.as_u64().unwrap_or(0).cmp(&a.1.as_u64().unwrap_or(0)));
        }
        for (k, count) in entries.iter().take(10) {
            eprintln!("    {count}× {k}");
        }
        if entries.len() > 10 {
            eprintln!("    … ({} more)", entries.len() - 10);
        }
    }
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
    // Cap at buffer size — if entries were evicted before this flush,
    // they're lost rather than replayed from the ring buffer tail.
    let capped = (new_count as usize).min(log.entries.len());
    let start = log.entries.len() - capped;
    for entry in log.entries.range(start..) {
        let day = TimeState::day_number(entry.tick, config);
        let phase = DayPhase::from_tick(entry.tick, config);
        let tier_label = match entry.tier {
            NarrativeTier::Micro => "Micro",
            NarrativeTier::Action => "Action",
            NarrativeTier::Significant => "Significant",
            NarrativeTier::Danger => "Danger",
            NarrativeTier::Nature => "Nature",
            NarrativeTier::Legend => "Legend",
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
    let capped = (new_count as usize).min(log.entries.len());
    let start = log.entries.len() - capped;
    for entry in log.entries.range(start..) {
        writeln!(file, "{}", serde_json::to_string(entry).unwrap_or_default())?;
    }
    *last_flushed = log.total_pushed;
    Ok(())
}

/// Flushes new `TraceLog` entries to `logs/trace-<focal>.jsonl`. Called
/// every tick when a `FocalTraceTarget` is active. Ring-buffer + forward-
/// walk semantics match [`flush_event_entries`] — if emission ever
/// outpaces the flush cadence the ring evicts oldest entries rather
/// than growing unbounded.
fn flush_trace_entries(
    world: &World,
    file: &mut std::fs::File,
    last_flushed: &mut u64,
) -> io::Result<()> {
    let Some(log) = world.get_resource::<clowder::resources::TraceLog>() else {
        return Ok(());
    };
    let new_count = log.total_pushed.saturating_sub(*last_flushed);
    if new_count == 0 {
        return Ok(());
    }
    let capped = (new_count as usize).min(log.entries.len());
    let start = log.entries.len() - capped;
    for entry in log.entries.range(start..) {
        writeln!(file, "{}", serde_json::to_string(entry).unwrap_or_default())?;
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
            "Danger" => NarrativeTier::Danger,
            "Nature" => NarrativeTier::Nature,
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

    // Find colony site first (read-only) so special tiles can respect colony distance.
    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    // Place special terrain tiles (ruins, fairy rings, standing stones, deep pools).
    let constants = clowder::resources::SimConstants::default();
    clowder::world_gen::special_tiles::place_special_tiles(
        &mut map,
        colony_site,
        &mut sim_rng.rng,
        &constants.world_gen,
    );

    // Set initial corruption and mystery on special tiles (must be after placement).
    clowder::world_gen::herbs::initialize_tile_magic(&mut map, &mut sim_rng.rng);

    // Start the clock high enough that cats can have varied ages. Must exceed
    // the maximum rolled age in ticks (see `FounderAgeConstants::elder_max_seasons`)
    // — otherwise `born_tick = start_tick.saturating_sub(age_ticks)` clamps to 0
    // and every cat reads back as the age of start_tick itself (Young by
    // default), which silently blocks mating eligibility.
    let ticks_per_season = config.ticks_per_season;
    let start_tick: u64 = 60 * ticks_per_season;

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

    // Build ECS world.
    let mut world = World::new();

    // Spawn starting buildings (sets terrain tiles and creates entities).
    spawn_starting_buildings(&mut world, colony_site, &mut map);

    // Persist colony center (no well entity in headless — no rendering).
    world.insert_resource(clowder::resources::ColonyCenter(colony_site));

    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: clowder::resources::SimSpeed::Normal,
    });
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(clowder::resources::ForcedConditions::default());
    world.insert_resource(clowder::resources::time::TransitionTracker::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(clowder::resources::ColonyKnowledge::default());
    world.insert_resource(clowder::resources::ColonyPriority::default());
    world.insert_resource(ColonyHuntingMap::default());
    world.insert_resource(clowder::resources::ExplorationMap::default());
    world.insert_resource(FoodStores::default());
    world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    world.insert_resource(clowder::resources::SimConstants::default());
    world.insert_resource(clowder::resources::SystemActivation::default());
    world.insert_resource(clowder::species::build_registry());
    world.insert_resource(clowder::components::prey::PreyDensity::default());
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

    // L2 substrate resources + DSE registrations. Manual mirror of
    // `SimulationPlugin::build()` — both paths must register the same
    // DSE set or headless and interactive builds diverge silently.
    world.insert_resource(clowder::ai::faction::FactionRelations::canonical());
    {
        let scoring = world
            .resource::<clowder::resources::SimConstants>()
            .scoring
            .clone();
        let mut registry = clowder::ai::eval::DseRegistry::new();
        registry.cat_dses.push(clowder::ai::dses::eat_dse());
        registry.cat_dses.push(clowder::ai::dses::hunt_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::hunt_target_dse());
        registry.cat_dses.push(clowder::ai::dses::forage_dse());
        registry.cat_dses.push(clowder::ai::dses::cook_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::flee_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::fight_dse(&scoring));
        registry
            .target_taking_dses
            .push(clowder::ai::dses::fight_target_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::sleep_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::idle_dse(&scoring));
        registry.cat_dses.push(clowder::ai::dses::socialize_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::socialize_target_dse());
        registry.cat_dses.push(clowder::ai::dses::groom_self_dse());
        registry.cat_dses.push(clowder::ai::dses::groom_other_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::groom_other_target_dse());
        registry.cat_dses.push(clowder::ai::dses::mentor_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::mentor_target_dse());
        registry.cat_dses.push(clowder::ai::dses::caretake_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::caretake_target_dse());
        registry.cat_dses.push(clowder::ai::dses::mate_dse());
        registry
            .target_taking_dses
            .push(clowder::ai::dses::mate_target_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::patrol_dse(&scoring));
        registry
            .cat_dses
            .push(clowder::ai::dses::build_dse(&scoring));
        registry
            .target_taking_dses
            .push(clowder::ai::dses::build_target_dse());
        registry.cat_dses.push(clowder::ai::dses::farm_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::coordinate_dse(&scoring));
        registry.cat_dses.push(clowder::ai::dses::explore_dse());
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
            .target_taking_dses
            .push(clowder::ai::dses::apply_remedy_target_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::herbcraft_ward_dse());
        registry.cat_dses.push(clowder::ai::dses::scry_dse());
        registry
            .cat_dses
            .push(clowder::ai::dses::durable_ward_dse());
        registry.cat_dses.push(clowder::ai::dses::cleanse_dse());
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
        registry.fox_dses.push(clowder::ai::dses::fox_avoiding_dse());
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
    // §3.5 modifier pipeline — mirrors simulation.rs registration but
    // with live `SimConstants` values since SimConstants is already
    // inserted at this point.
    {
        let scoring = world
            .resource::<clowder::resources::SimConstants>()
            .scoring
            .clone();
        world.insert_resource(clowder::ai::modifier::default_modifier_pipeline(&scoring));
    }

    // Seed `last_recorded_season` to the current season so `seasons_survived`
    // starts at 0 and counts real elapsed seasons, not the start_tick offset.
    let initial_season = start_tick / ticks_per_season;
    let initial_score = clowder::resources::ColonyScore {
        last_recorded_season: initial_season,
        ..Default::default()
    };
    world.insert_resource(initial_score);
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
                    CurrentAction::default(),
                    Inventory::default(),
                    HuntingPriors::default(),
                    clowder::components::grooming::GroomingCondition::default(),
                    clowder::components::goap_plan::PendingUrgencies::default(),
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
    clowder::systems::wildlife::spawn_initial_wildlife(&mut world, colony_site);
    clowder::systems::wildlife::spawn_initial_fox_dens(&mut world, colony_site);

    // Insert fox scent map resource.
    world.insert_resource(clowder::resources::FoxScentMap::default());

    // Insert prey scent map resource (Phase 2B).
    world.insert_resource(clowder::resources::PreyScentMap::default());

    // Insert cat presence map resource.
    world.insert_resource(clowder::resources::CatPresenceMap::default());

    // Insert unmet-demand ledger (mirrors SimulationPlugin).
    world.insert_resource(clowder::resources::UnmetDemand::default());

    // Spawn initial prey animals across their habitats.
    clowder::world_gen::prey_ecosystem::seed_prey_ecosystem(&mut world);

    // Spawn herbs based on terrain and current season.
    let current_season = {
        let time = world.resource::<TimeState>();
        let config = world.resource::<SimConfig>();
        time.season(config)
    };
    clowder::world_gen::herbs::spawn_herbs(&mut world, current_season);
    clowder::world_gen::herbs::spawn_flavor_plants(&mut world, current_season);

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
