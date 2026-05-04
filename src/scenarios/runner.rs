//! The scenario runner: build a headless `App` with the scenario's setup
//! closure injected, tick N times, return a `ScenarioReport` of per-tick
//! winning DSEs and the final L2 score table.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::MinimalPlugins;

use crate::plugins::headless_io::HeadlessConfig;
use crate::plugins::setup::{AppArgs, WorldSetup};
use crate::plugins::simulation::SimulationPlugin;
use crate::resources::trace_log::{
    FocalScoreCapture, FocalTraceTarget, TraceEntry, TraceLog, TraceRecord,
};
use crate::resources::{SimConfig, TimeScale};

use super::Scenario;

const TEST_GAME_DAY_SECONDS: f32 = 16.666_667;

/// Per-tick distillation of the focal cat's L3 record — what the cat
/// chose this tick, plus the ranked DSE table.
#[derive(Debug, Clone)]
pub struct TickReport {
    pub tick: u64,
    /// The DSE name that won at L3 (`TraceRecord::L3.chosen`). `None` if
    /// the focal cat hadn't been resolved yet on this tick (e.g.,
    /// pre-spawn) or the trace was missing for some other reason.
    pub chosen: Option<String>,
    /// `(dse_name, score)` rows from `TraceRecord::L3.ranked`, sorted
    /// descending by score. Empty if `chosen.is_none()`.
    pub ranked: Vec<(String, f32)>,
}

/// Output of [`run`]. Carries one row per tick plus convenience accessors
/// for the most common assertion shape: "what did the focal cat pick at
/// tick N?".
#[derive(Debug, Clone)]
pub struct ScenarioReport {
    pub scenario_name: &'static str,
    pub focal: String,
    pub seed: u64,
    pub ticks: Vec<TickReport>,
}

impl ScenarioReport {
    /// 1-indexed lookup: `tick(1)` returns the first tick's report. Panics
    /// if out of range so test assertions surface clearly.
    pub fn tick(&self, tick_1based: usize) -> &TickReport {
        &self.ticks[tick_1based - 1]
    }

    /// How often each DSE won across the run.
    pub fn winner_counts(&self) -> std::collections::BTreeMap<String, usize> {
        let mut counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for t in &self.ticks {
            if let Some(c) = &t.chosen {
                *counts.entry(c.clone()).or_insert(0) += 1;
            }
        }
        counts
    }
}

/// Run a scenario. `focal_override` defaults to the scenario's
/// `default_focal`; `ticks_override` to its `default_ticks`. `seed`
/// defaults to 42 — the standard reproducibility anchor used across
/// `just verdict` and the soak baselines.
pub fn run(
    scenario: &Scenario,
    focal_override: Option<&str>,
    ticks_override: Option<u32>,
    seed: u64,
) -> ScenarioReport {
    let focal = focal_override
        .unwrap_or(scenario.default_focal)
        .to_string();
    let ticks = ticks_override.unwrap_or(scenario.default_ticks);

    let mut app = build_scenario_app(seed, scenario, &focal);

    // Run one Startup-only update to materialize the world (the scenario
    // setup closure runs at Startup), then pre-resolve `FocalTraceTarget.entity`
    // so the very first FixedUpdate scoring pass already knows which entity
    // is focal. Without this, the first 1–2 ticks have empty capture
    // (scoring runs before `emit_focal_trace` resolves the entity by
    // name) and a cat that commits to a long plan in those ticks never
    // re-scores — making the scenario appear silent.
    app.update();
    pre_resolve_focal(&mut app, &focal);

    let mut tick_reports: Vec<TickReport> = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        app.update();
        tick_reports.push(drain_tick_report(&mut app));
    }

    ScenarioReport {
        scenario_name: scenario.name,
        focal,
        seed,
        ticks: tick_reports,
    }
}

fn build_scenario_app(seed: u64, scenario: &Scenario, focal: &str) -> App {
    let preview_scale = TimeScale::from_config(&SimConfig::default(), TEST_GAME_DAY_SECONDS);
    let hz = preview_scale.tick_rate_hz() as f64;
    let fixed_timestep = Duration::from_secs_f64(1.0 / hz);

    // Headless config — no log files; we read TraceLog in-memory instead.
    let config = HeadlessConfig {
        seed,
        duration_secs: 86_400, // tick-count gated, not wall-time
        log_path: std::path::PathBuf::from("/dev/null"),
        event_log_path: std::path::PathBuf::from("/dev/null"),
        trace_log_path: None,
        focal_cat: Some(focal.to_string()),
        force_weather: None,
        snapshot_interval: 100,
        trace_positions: 0,
        load_log_path: None,
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(AppArgs {
        seed,
        load_path: None,
        load_log_path: None,
        test_map: false,
        wall_seconds_per_game_day: TEST_GAME_DAY_SECONDS,
    });
    app.insert_resource(config);

    // Inject the scenario world-setup BEFORE adding SimulationPlugin so
    // setup_world_exclusive picks it up.
    let setup_fn = scenario.setup;
    app.insert_resource(WorldSetup::new(move |world, seed| {
        setup_fn(world, seed);
    }));

    // Trace surface — must be inserted before SimulationPlugin so the
    // scoring systems' `Option<Res<FocalScoreCapture>>` resolves Some.
    app.insert_resource(FocalTraceTarget {
        name: focal.to_string(),
        entity: None,
    });
    app.insert_resource(TraceLog::default());
    app.insert_resource(FocalScoreCapture::default());

    app.add_plugins(SimulationPlugin);
    // Note: HeadlessIoPlugin is NOT added — it would write to /dev/null
    // log paths but pulls in writing behavior we don't need. Scoring +
    // trace emission live on SimulationPlugin's schedule already.

    app.insert_resource(TimeUpdateStrategy::ManualDuration(fixed_timestep));
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep(fixed_timestep);

    app
}

fn pre_resolve_focal(app: &mut App, focal: &str) {
    use crate::components::identity::Name;
    use bevy_ecs::entity::Entity;
    let world = app.world_mut();
    // Walk all entities with a Name. Avoids importing Species (which might
    // be filtered) — names are unique enough for scenarios.
    let mut entity: Option<Entity> = None;
    let mut q = world.query::<(Entity, &Name)>();
    for (e, name) in q.iter(world) {
        if name.0 == focal {
            entity = Some(e);
            break;
        }
    }
    if let Some(e) = entity {
        let mut target = world.resource_mut::<FocalTraceTarget>();
        target.entity = Some(e);
    }
}

fn drain_tick_report(app: &mut App) -> TickReport {
    let world = app.world_mut();
    let tick = world.resource::<crate::resources::TimeState>().tick;
    let mut log = world.resource_mut::<TraceLog>();
    // Pull the L3 record (one per tick) and the ranked table out of
    // whatever was emitted; clear so the next tick starts fresh.
    let mut chosen: Option<String> = None;
    let mut ranked: Vec<(String, f32)> = Vec::new();
    for entry in log.entries.drain(..) {
        let TraceEntry { record, .. } = entry;
        if let TraceRecord::L3 {
            chosen: c, ranked: r, ..
        } = record
        {
            chosen = Some(c);
            ranked = r;
        }
    }
    TickReport {
        tick,
        chosen,
        ranked,
    }
}
