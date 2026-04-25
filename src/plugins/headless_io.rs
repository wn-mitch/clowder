//! Headless-mode I/O plugin (ticket 030 step 2).
//!
//! Owns every concern that distinguishes the headless run loop from the
//! windowed App: CLI args as a resource, the three JSONL writers
//! (events / narrative / optional focal trace), per-tick flush systems,
//! the wall-time + wipeout tick-budget exit, and the end-of-sim footer.
//!
//! Phase C is *additive*. The plugin compiles and exports its public
//! API but is not yet mounted by `run_headless` — phase D rewrites
//! `run_headless` to build an `App` with `MinimalPlugins +
//! SimulationPlugin + HeadlessIoPlugin` and a manual `app.update()`
//! loop. Until that lands, the legacy `build_schedule` /
//! `flush_*_entries` path in `src/main.rs` stays the active code path.
//!
//! The plugin assumes the host has already inserted [`HeadlessConfig`]
//! before calling [`App::add_plugins`]; the plugin's `build()` reads
//! the config from the world to open output files and seed writer
//! resources.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use bevy::app::AppExit;
use bevy::prelude::*;

use crate::components::identity::Species;
use crate::components::physical::Dead;
use crate::resources::event_log::EventLog;
use crate::resources::map::TileMap;
use crate::resources::narrative::NarrativeLog;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::{SimConfig, TimeState};
use crate::resources::trace_log::TraceLog;
use crate::resources::weather::Weather;
use crate::resources::{FocalScoreCapture, FocalTraceTarget};
use crate::resources::SystemActivation;
use crate::resources::system_activation::FeatureCategory;

/// Headless CLI args, threaded into the App as a resource.
///
/// The host (`run_headless` post-phase-D) parses argv, builds this
/// resource, and inserts it before adding [`HeadlessIoPlugin`]. The
/// plugin reads it during `build()` to know which file paths to open
/// and which optional trace sidecar to wire up.
#[derive(Resource, Clone, Debug)]
pub struct HeadlessConfig {
    pub seed: u64,
    pub duration_secs: u64,
    pub log_path: PathBuf,
    pub event_log_path: PathBuf,
    pub trace_log_path: Option<PathBuf>,
    pub focal_cat: Option<String>,
    pub force_weather: Option<Weather>,
    pub snapshot_interval: u64,
    pub trace_positions: u64,
    pub load_log_path: Option<PathBuf>,
}

/// Buffered writer for `events.jsonl` plus the monotonic flush cursor.
///
/// `last_flushed` mirrors the running counter the legacy
/// `flush_event_entries` helper carried in a stack-local variable; it
/// lives in a resource here so the per-tick flush system is plain
/// Bevy data-flow.
#[derive(Resource)]
pub struct EventJsonlWriter {
    pub writer: BufWriter<File>,
    pub last_flushed: u64,
}

/// Buffered writer for `narrative.jsonl` + flush cursor.
#[derive(Resource)]
pub struct NarrativeJsonlWriter {
    pub writer: BufWriter<File>,
    pub last_flushed: u64,
}

/// Buffered writer for the optional focal-cat trace sidecar
/// (`trace-<focal>.jsonl`). Inserted only when
/// [`HeadlessConfig::focal_cat`] is `Some`.
#[derive(Resource)]
pub struct TraceJsonlWriter {
    pub writer: BufWriter<File>,
    pub last_flushed: u64,
}

/// Wall-clock budget tracker. Inserted at plugin build time; the
/// tick-budget exit system reads this each `Last`-schedule pass.
#[derive(Resource)]
pub struct HeadlessRunStart(pub Instant);

/// Counts how many `Last`-schedule passes the run has executed —
/// printed as the `schedule runs` value in the operator-facing
/// completion summary.
#[derive(Resource, Default)]
pub struct HeadlessTickCount(pub u64);

/// Marker resource that the tick-budget exit system writes once,
/// the same tick the footer is emitted, so the post-loop helpers
/// in `run_headless` can detect "exit triggered".
#[derive(Resource, Default)]
pub struct HeadlessExitSignaled(pub bool);

/// The plugin itself. See module-level doc.
pub struct HeadlessIoPlugin;

impl Plugin for HeadlessIoPlugin {
    fn build(&self, app: &mut App) {
        let config = app
            .world()
            .get_resource::<HeadlessConfig>()
            .cloned()
            .expect(
                "HeadlessIoPlugin requires HeadlessConfig — \
                 host must `app.insert_resource(HeadlessConfig { … })` \
                 before `app.add_plugins(HeadlessIoPlugin)`.",
            );

        // Create parent directories for every output file. Failures
        // here surface as panics during App build, which is the right
        // behavior — there's no recovery if we can't write logs.
        if let Some(parent) = config.log_path.parent() {
            std::fs::create_dir_all(parent).expect("create narrative log parent dir");
        }
        if let Some(parent) = config.event_log_path.parent() {
            std::fs::create_dir_all(parent).expect("create event log parent dir");
        }
        if let Some(ref tp) = config.trace_log_path {
            if let Some(parent) = tp.parent() {
                std::fs::create_dir_all(parent).expect("create trace log parent dir");
            }
        }

        // Open writers. `BufWriter` buffers fine; per-tick flushes
        // call `writer.flush()` after each batch so consumers
        // (downstream tooling tailing the file mid-run) see fresh
        // content without waiting for buffer fill.
        let narrative_file =
            File::create(&config.log_path).expect("create narrative log file");
        let event_file =
            File::create(&config.event_log_path).expect("create event log file");

        app.insert_resource(NarrativeJsonlWriter {
            writer: BufWriter::new(narrative_file),
            last_flushed: 0,
        });
        app.insert_resource(EventJsonlWriter {
            writer: BufWriter::new(event_file),
            last_flushed: 0,
        });

        // Focal-cat trace path is opt-in — present only when the CLI
        // passed `--focal-cat`. Insert the trace writer + the three
        // gating resources together so all the trace systems
        // (`emit_focal_trace`, `score_dse_by_id` capture path, etc.)
        // either all see the focal infrastructure or none do.
        if let Some(ref focal_name) = config.focal_cat {
            if let Some(ref tp) = config.trace_log_path {
                let trace_file = File::create(tp).expect("create trace log file");
                app.insert_resource(TraceJsonlWriter {
                    writer: BufWriter::new(trace_file),
                    last_flushed: 0,
                });
            }
            app.insert_resource(FocalTraceTarget {
                name: focal_name.clone(),
                entity: None,
            });
            app.insert_resource(TraceLog::default());
            app.insert_resource(FocalScoreCapture::default());
        }

        // Wall-clock anchor + tick counter.
        app.insert_resource(HeadlessRunStart(Instant::now()));
        app.init_resource::<HeadlessTickCount>();
        app.init_resource::<HeadlessExitSignaled>();

        // Headers run once at Startup, ordered after
        // `setup_world_exclusive` so SimConstants / SimConfig /
        // TileMap are populated. The header rows are the
        // constants-hash anchor for cross-run reproducibility — see
        // CLAUDE.md "Simulation Verification → The constants-hash
        // header".
        app.add_systems(
            Startup,
            write_jsonl_headers.after(crate::plugins::setup::setup_world_exclusive),
        );

        // Per-tick flush — `Last` runs after every other system so
        // entries pushed by sim systems this tick land in the file
        // before the next iteration.
        app.add_systems(
            Last,
            (
                flush_narrative_jsonl,
                flush_events_jsonl,
                flush_trace_jsonl
                    .run_if(bevy::prelude::resource_exists::<TraceJsonlWriter>),
                bump_headless_tick_count,
                tick_budget_check_and_exit,
            ),
        );
    }
}

/// One-shot Startup system that writes the `_header` line to each
/// JSONL file. Mirrors the inline header build at
/// `run_headless` — same field names, same ordering, so downstream
/// header diffs continue to compare byte-for-byte across versions.
#[allow(clippy::too_many_arguments)]
pub fn write_jsonl_headers(
    config: Res<HeadlessConfig>,
    sim_constants: Res<SimConstants>,
    sim_config: Res<SimConfig>,
    tile_map: Res<TileMap>,
    mut narrative_writer: ResMut<NarrativeJsonlWriter>,
    mut event_writer: ResMut<EventJsonlWriter>,
    trace_writer: Option<ResMut<TraceJsonlWriter>>,
) {
    let commit_hash = env!("GIT_HASH");
    let commit_hash_short = env!("GIT_HASH_SHORT");
    let commit_time = env!("GIT_COMMIT_TIME");
    let commit_dirty = env!("GIT_DIRTY") == "true";

    let constants_json = serde_json::to_value((*sim_constants).clone()).unwrap_or_default();
    let sim_config_json = serde_json::to_value((*sim_config).clone()).unwrap_or_default();
    let forced_weather_json = config.force_weather.map(|w| w.label());
    let sensory_env_multipliers_json = sensory_env_multipliers_snapshot();

    // Narrative: lighter header (no constants block).
    let narrative_header = serde_json::json!({
        "_header": true,
        "seed": config.seed,
        "duration_secs": config.duration_secs,
        "commit_hash": commit_hash,
        "commit_hash_short": commit_hash_short,
        "commit_dirty": commit_dirty,
        "commit_time": commit_time,
    });
    if let Err(e) = writeln!(narrative_writer.writer, "{narrative_header}") {
        eprintln!("Warning: failed to write narrative header: {e}");
    }

    // Events: full header with constants + map size.
    let event_header = serde_json::json!({
        "_header": true,
        "seed": config.seed,
        "duration_secs": config.duration_secs,
        "commit_hash": commit_hash,
        "commit_hash_short": commit_hash_short,
        "commit_dirty": commit_dirty,
        "commit_time": commit_time,
        "sim_config": sim_config_json,
        "map_width": tile_map.width,
        "map_height": tile_map.height,
        "constants": constants_json,
        "forced_weather": forced_weather_json,
        "sensory_env_multipliers": sensory_env_multipliers_json,
    });
    if let Err(e) = writeln!(event_writer.writer, "{event_header}") {
        eprintln!("Warning: failed to write event header: {e}");
    }

    // Trace: events-flavored header + focal_cat field. The
    // joinability invariant in spec §11.4 requires the trace and
    // event headers carry matching `constants` / `sim_config` /
    // `commit_hash` fields, which they do.
    if let (Some(mut trace_writer), Some(focal_cat)) = (trace_writer, config.focal_cat.as_ref()) {
        let trace_header = serde_json::json!({
            "_header": true,
            "focal_cat": focal_cat,
            "seed": config.seed,
            "duration_secs": config.duration_secs,
            "commit_hash": commit_hash,
            "commit_hash_short": commit_hash_short,
            "commit_dirty": commit_dirty,
            "commit_time": commit_time,
            "sim_config": sim_config_json,
            "map_width": tile_map.width,
            "map_height": tile_map.height,
            "constants": constants_json,
            "forced_weather": forced_weather_json,
            "sensory_env_multipliers": sensory_env_multipliers_json,
        });
        if let Err(e) = writeln!(trace_writer.writer, "{trace_header}") {
            eprintln!("Warning: failed to write trace header: {e}");
        }
    }
}

/// Per-tick flush of new [`NarrativeLog`] entries. Mirrors the
/// `flush_new_entries` helper in `src/main.rs` — same ring-buffer
/// forward-walk, same `total_pushed` cursor.
pub fn flush_narrative_jsonl(
    log: Res<NarrativeLog>,
    sim_config: Res<SimConfig>,
    mut writer: ResMut<NarrativeJsonlWriter>,
) {
    use crate::resources::narrative::NarrativeTier;
    use crate::resources::time::DayPhase;

    let new_count = log.total_pushed.saturating_sub(writer.last_flushed);
    if new_count == 0 {
        return;
    }
    let capped = (new_count as usize).min(log.entries.len());
    let start = log.entries.len() - capped;
    for entry in log.entries.range(start..) {
        let day = TimeState::day_number(entry.tick, &sim_config);
        let phase = DayPhase::from_tick(entry.tick, &sim_config);
        let tier_label = match entry.tier {
            NarrativeTier::Micro => "Micro",
            NarrativeTier::Action => "Action",
            NarrativeTier::Significant => "Significant",
            NarrativeTier::Danger => "Danger",
            NarrativeTier::Nature => "Nature",
            NarrativeTier::Legend => "Legend",
        };
        let line = serde_json::json!({
            "tick": entry.tick,
            "day": day,
            "phase": phase.label(),
            "tier": tier_label,
            "text": entry.text,
        });
        if let Err(e) = writeln!(writer.writer, "{line}") {
            eprintln!("Warning: narrative flush failed: {e}");
            return;
        }
    }
    writer.last_flushed = log.total_pushed;
    let _ = writer.writer.flush();
}

/// Per-tick flush of new [`EventLog`] entries. Mirrors
/// `flush_event_entries`.
pub fn flush_events_jsonl(log: Res<EventLog>, mut writer: ResMut<EventJsonlWriter>) {
    let new_count = log.total_pushed.saturating_sub(writer.last_flushed);
    if new_count == 0 {
        return;
    }
    let capped = (new_count as usize).min(log.entries.len());
    let start = log.entries.len() - capped;
    for entry in log.entries.range(start..) {
        let line = serde_json::to_string(entry).unwrap_or_default();
        if let Err(e) = writeln!(writer.writer, "{line}") {
            eprintln!("Warning: event flush failed: {e}");
            return;
        }
    }
    writer.last_flushed = log.total_pushed;
    let _ = writer.writer.flush();
}

/// Per-tick flush of new [`TraceLog`] entries. Gated on the writer
/// resource existing — when `--focal-cat` is absent the system is
/// `run_if`'d off in the plugin's build.
pub fn flush_trace_jsonl(log: Option<Res<TraceLog>>, writer: Option<ResMut<TraceJsonlWriter>>) {
    let (Some(log), Some(mut writer)) = (log, writer) else {
        return;
    };
    let new_count = log.total_pushed.saturating_sub(writer.last_flushed);
    if new_count == 0 {
        return;
    }
    let capped = (new_count as usize).min(log.entries.len());
    let start = log.entries.len() - capped;
    for entry in log.entries.range(start..) {
        let line = serde_json::to_string(entry).unwrap_or_default();
        if let Err(e) = writeln!(writer.writer, "{line}") {
            eprintln!("Warning: trace flush failed: {e}");
            return;
        }
    }
    writer.last_flushed = log.total_pushed;
    let _ = writer.writer.flush();
}

/// Increments the per-tick run counter. Decoupled from
/// [`tick_budget_check_and_exit`] so the count survives even when
/// the exit system early-returns.
pub fn bump_headless_tick_count(mut count: ResMut<HeadlessTickCount>) {
    count.0 = count.0.saturating_add(1);
}

/// Wall-clock + wipeout exit gate. Writes [`AppExit::Success`] when
/// the configured `duration_secs` has elapsed *or* every cat is
/// dead, so the manual loop in `run_headless` can break.
///
/// Footer emission is the responsibility of [`emit_headless_footer`],
/// which the post-loop tail in `run_headless` calls once the loop
/// exits — this keeps the footer's `&mut World` access out of the
/// system graph.
pub fn tick_budget_check_and_exit(
    config: Res<HeadlessConfig>,
    start: Res<HeadlessRunStart>,
    mut signaled: ResMut<HeadlessExitSignaled>,
    mut app_exit: MessageWriter<AppExit>,
    cats: Query<(), (With<Species>, Without<Dead>)>,
) {
    if signaled.0 {
        return;
    }
    let elapsed = start.0.elapsed().as_secs();
    let alive = cats.iter().count();
    if elapsed >= config.duration_secs || alive == 0 {
        signaled.0 = true;
        app_exit.write(AppExit::Success);
    }
}

/// Build and write the end-of-sim diagnostic footer. Called once,
/// post-loop, by `run_headless` (phase D). Kept as a free function
/// rather than an exclusive system because it needs `&mut World`
/// access for the live `Ward` count and the same access pattern
/// today's `build_headless_footer` uses — easier to call from the
/// post-loop tail than to coerce into Bevy's system signature.
pub fn emit_headless_footer(world: &mut World) {
    use crate::components::magic::Ward;
    use crate::resources::system_activation::Feature;

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
    let positive_features_active = activation.features_active_in(FeatureCategory::Positive);
    let positive_features_total = SystemActivation::features_total_in(FeatureCategory::Positive);
    let negative_events_total = activation.negative_event_count();
    let neutral_features_active = activation.features_active_in(FeatureCategory::Neutral);
    let neutral_features_total = SystemActivation::features_total_in(FeatureCategory::Neutral);
    let never_fired_expected_positives = activation.never_fired_expected_positives();

    let event_log = world.resource::<EventLog>();
    let footer = serde_json::json!({
        "_footer": true,
        "wards_placed_total": feature_count(Feature::WardPlaced),
        "wards_despawned_total": feature_count(Feature::WardDespawned),
        "ward_count_final": ward_count_final,
        "ward_avg_strength_final": ward_avg_strength_final,
        "shadow_foxes_avoided_ward_total": feature_count(Feature::ShadowFoxAvoidedWard),
        "ward_siege_started_total": feature_count(Feature::WardSiegeStarted),
        "shadow_fox_spawn_total": feature_count(Feature::ShadowFoxSpawn),
        "anxiety_interrupt_total": feature_count(Feature::AnxietyInterrupt),
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

    let mut writer = world.resource_mut::<EventJsonlWriter>();
    if let Err(e) = writeln!(writer.writer, "{footer}") {
        eprintln!("Warning: failed to write headless footer: {e}");
    }
    let _ = writer.writer.flush();
}

/// Sensory-environment multiplier snapshot embedded in the events
/// header. Copied verbatim from `src/main.rs::sensory_env_multipliers_snapshot`
/// — kept here so phase D can delete the helper from main.rs.
fn sensory_env_multipliers_snapshot() -> serde_json::Value {
    use crate::resources::map::Terrain;
    use crate::resources::time::DayPhase;

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
