use std::io;
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::{
    App, ButtonInput, ClearColor, Color, DefaultPlugins, Fixed, ImagePlugin, MessageWriter,
    PluginGroup, Res, ResMut, Time, Update, Window, WindowPlugin,
};
use bevy::window::WindowResolution;

use clowder::ui_data::{InspectionMode, InspectionState};

use bevy_ecs::prelude::*;

use clowder::persistence;
use clowder::plugins::setup::AppArgs;
use clowder::plugins::simulation::SimulationPlugin;
use clowder::rendering;

use clowder::resources::weather::Weather;
use clowder::resources::{SimConfig, TimeScale, TimeState};

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
    /// Wall-seconds-per-in-game-day peg (ticket 033). Default 16.6667
    /// preserves the historical headless 60 Hz tick rate at the
    /// canonical 1000 ticks/day. Honored only by `--headless` runs;
    /// the windowed build derives its peg from `SimSpeed`.
    game_day_seconds: f32,
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
            // Windowed default: SimSpeed::Normal at the canonical
            // 1000 ticks/day → 1000 wall-secs/day (= 1 Hz, matches
            // pre-ticket-033 behavior). `sync_sim_speed` updates this
            // when the player cycles speed presets.
            wall_seconds_per_game_day: clowder::resources::SimSpeed::Normal
                .wall_seconds_per_game_day(&clowder::resources::SimConfig::default()),
        })
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
///
/// Routes through [`TimeScale`] so the headless and windowed builds
/// share the same anchor. SimSpeed maps onto `wall_seconds_per_game_day`
/// via [`SimSpeed::wall_seconds_per_game_day`]; the FixedUpdate Hz then
/// derives from `TimeScale::tick_rate_hz()`. Preserves prior behavior:
/// Normal = 1 Hz, Fast = 5 Hz, VeryFast = 20 Hz at the default
/// 1000 ticks/day scale.
fn sync_sim_speed(
    time_state: Res<TimeState>,
    config: Res<SimConfig>,
    mut time_scale: ResMut<TimeScale>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    if !time_state.is_changed() {
        return;
    }
    let secs = time_state.speed.wall_seconds_per_game_day(&config);
    time_scale.set_wall_seconds_per_game_day(secs);
    let hz = time_scale.tick_rate_hz() as f64;
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
    // Default preserves 60 Hz at 1000 ticks/day (1000 / 60 ≈ 16.6667).
    let mut game_day_seconds: f32 = 16.666_667;
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
            "--game-day-seconds" => {
                let Some(val) = iter.next() else {
                    eprintln!("Error: --game-day-seconds requires a value");
                    std::process::exit(2);
                };
                let parsed: f32 = val.parse().unwrap_or_else(|_| {
                    eprintln!("Error: --game-day-seconds: cannot parse {val:?}");
                    std::process::exit(2);
                });
                if !(parsed > 0.0 && parsed.is_finite()) {
                    eprintln!("Error: --game-day-seconds must be > 0 and finite (got {parsed})");
                    std::process::exit(2);
                }
                game_day_seconds = parsed;
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
    if !headless && (game_day_seconds - 16.666_667).abs() > f32::EPSILON {
        eprintln!(
            "Warning: --game-day-seconds has no effect without --headless \
             (windowed builds derive the peg from SimSpeed)"
        );
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
        game_day_seconds,
    }
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

// ---------------------------------------------------------------------------
// Headless mode
// ---------------------------------------------------------------------------

fn run_headless(args: CliArgs) -> io::Result<()> {
    use bevy::time::TimeUpdateStrategy;
    use bevy::MinimalPlugins;
    use clowder::plugins::headless_io::{
        emit_headless_footer, EventJsonlWriter, HeadlessConfig, HeadlessIoPlugin,
        HeadlessTickCount, NarrativeJsonlWriter, TraceJsonlWriter,
    };

    // Resolve log paths with the legacy default fallbacks.
    let log_path = args
        .log_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("logs/narrative.jsonl"));
    let event_log_path = args
        .event_log_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("logs/events.jsonl"));
    let trace_log_path = args.focal_cat.as_ref().map(|name| {
        args.trace_log_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("logs/trace-{name}.jsonl")))
    });

    let headless_config = HeadlessConfig {
        seed: args.seed,
        duration_secs: args.duration_secs,
        log_path: log_path.clone(),
        event_log_path: event_log_path.clone(),
        trace_log_path: trace_log_path.clone(),
        focal_cat: args.focal_cat.clone(),
        force_weather: args.force_weather,
        snapshot_interval: args.snapshot_interval,
        trace_positions: args.trace_positions,
        load_log_path: args.load_log_path.clone(),
    };

    // Build the headless App. SimulationPlugin owns the simulation
    // graph (FixedUpdate systems, observers, messages, DSE registry);
    // HeadlessIoPlugin owns I/O (JSONL writers, header rows, per-tick
    // flush, tick-budget exit).
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(AppArgs {
        seed: args.seed,
        load_path: args.load_path.clone(),
        load_log_path: args.load_log_path.clone(),
        test_map: args.test_map,
        // Headless peg from `--game-day-seconds`. Default 16.6667
        // preserves the prior 60 Hz tick rate at 1000 ticks/day.
        // `setup_world_exclusive` reads this back to construct the
        // [`TimeScale`] resource during Startup.
        wall_seconds_per_game_day: args.game_day_seconds,
    });
    app.insert_resource(headless_config);
    app.add_plugins(SimulationPlugin);
    app.add_plugins(HeadlessIoPlugin);

    // FixedUpdate Hz must be set before `app.update()` runs, but
    // `TimeScale` itself doesn't land in the world until Startup.
    // Compute the same Hz here from the same inputs so the two stay
    // in lockstep — the canonical value will be re-derivable from
    // the world's `TimeScale` once Startup runs.
    let preview_scale = TimeScale::from_config(&SimConfig::default(), args.game_day_seconds);
    let hz = preview_scale.tick_rate_hz() as f64;

    // Drive Time<Virtual> manually so each app.update() advances Time
    // by exactly one fixed-timestep — one App update == one sim tick,
    // matching today's schedule.run() cadence and keeping the run
    // wall-clock-bounded by the duration_secs gate in
    // `tick_budget_check_and_exit`.
    let fixed_timestep = Duration::from_secs_f64(1.0 / hz);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(fixed_timestep));
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep(fixed_timestep);

    // Main loop. tick_budget_check_and_exit (in HeadlessIoPlugin)
    // writes AppExit::Success when wall-time elapses or every cat is
    // dead. Until then, app.update() advances the sim one tick.
    while app.should_exit().is_none() {
        app.update();
    }

    // Post-loop tail: write footer, pretty-print summary, autosave.
    let footer = emit_headless_footer(app.world_mut());

    let ticks = app.world().resource::<HeadlessTickCount>().0;
    let time_tick = app.world().resource::<TimeState>().tick;
    let day_number = {
        let sim_config = app.world().resource::<SimConfig>();
        TimeState::day_number(time_tick, sim_config)
    };
    let narrative_count = app
        .world()
        .get_resource::<NarrativeJsonlWriter>()
        .map(|w| w.last_flushed)
        .unwrap_or(0);
    let event_count = app
        .world()
        .get_resource::<EventJsonlWriter>()
        .map(|w| w.last_flushed)
        .unwrap_or(0);
    let trace_count = app
        .world()
        .get_resource::<TraceJsonlWriter>()
        .map(|w| w.last_flushed);

    eprintln!(
        "\nHeadless complete: {ticks} schedule runs, sim day {day_number}, {narrative_count} narrative / {event_count} event entries",
    );
    eprintln!("  narrative → {}", log_path.display());
    eprintln!("  events    → {}", event_log_path.display());
    if let (Some(tp), Some(records)) = (trace_log_path.as_ref(), trace_count) {
        eprintln!("  trace     → {}  ({records} records)", tp.display());
    }
    print_headless_summary(&footer);

    // Autosave.
    let save_path = std::path::Path::new(SAVE_PATH);
    if let Err(e) = persistence::save_to_file(app.world_mut(), save_path) {
        eprintln!("Warning: failed to autosave: {e}");
    }

    Ok(())
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
        "planning_failures_by_disposition",
        "planning_failures_by_reason",
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

// ---------------------------------------------------------------------------
// World construction helpers
// ---------------------------------------------------------------------------
