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
    FocalScoreCapture, FocalTraceTarget, ModifierApplication, TraceEntry, TraceLog, TraceRecord,
};
use crate::resources::{SimConfig, TimeScale};

use super::Scenario;

const TEST_GAME_DAY_SECONDS: f32 = 16.666_667;

/// Per-tick distillation of the focal cat's L3 record — what the cat
/// chose this tick, plus the ranked DSE table.
#[derive(Debug, Clone)]
pub struct TickReport {
    pub tick: u64,
    /// The action string that the resolver is executing this tick — read
    /// from `current.action` after `resolve_goap_plans`. Note: this is
    /// **not** the softmax winner. If a Hunt plan won softmax, its first
    /// GOAP step may be `Action::Explore` (move toward prey scent), so
    /// `chosen == "Explore"` while the actual disposition is Hunting.
    /// `None` if the focal cat hadn't been resolved yet.
    pub chosen: Option<String>,
    /// `(action_name, score)` rows from `TraceRecord::L3.ranked` — the
    /// post-Independence-penalty softmax pool, sorted descending. Empty
    /// when `chosen.is_none()`.
    pub ranked: Vec<(String, f32)>,
    /// Softmax probabilities parallel-indexed with `ranked`. Empty when
    /// the softmax-fallthrough path was taken (no rolled distribution).
    pub softmax_probs: Vec<f32>,
    /// Per-DSE L2 score breakdown captured this tick, projected from
    /// `TraceRecord::L2`. Critical for the L2-vs-L3 boundary investigation:
    /// `final_score` here is **pre-Independence-penalty**, while the
    /// `ranked` field above is **post-penalty**. A divergence between an
    /// L2 row's `final_score` and its corresponding pool entry in
    /// `ranked` is the Independence penalty showing itself.
    pub l2: Vec<L2RowSummary>,
    /// Action-keyed score Vec at `score_actions` exit. Empty when the
    /// focal cat hasn't been resolved yet or the softmax fell through.
    pub pre_bonus_pool: Vec<(String, f32)>,
    /// Post-filter, pre-Independence-penalty pool the softmax saw.
    pub pre_penalty_pool: Vec<(String, f32)>,
}

/// Compact per-DSE L2 row for the scenario report. Trims
/// `TraceRecord::L2`'s full shape (per-consideration breakdown, target
/// rankings) down to the score columns + modifier deltas needed for
/// boundary triage. If the per-consideration view is needed, read
/// `TraceLog` directly via the runner — this summary is for the CLI
/// and the assertion path.
#[derive(Debug, Clone)]
pub struct L2RowSummary {
    pub dse: String,
    pub eligible: bool,
    pub maslow_pregate: f32,
    /// Composition output before Maslow tier suppression.
    pub raw_score: f32,
    /// `raw_score * maslow_pregate` — the score the modifier pipeline
    /// receives. Derived (the trace record stores `raw_score` and
    /// `maslow_pregate` separately but never the product).
    pub gated_score: f32,
    /// After the modifier pipeline. **Pre-Independence-penalty** — the
    /// post-penalty value lives in `TickReport.ranked`.
    pub final_score: f32,
    /// `(modifier_name, delta_or_multiplier)` pairs. Modifiers in the
    /// live §3.5.1 catalog are additive-only today; multiplicative
    /// modifiers serialize as `(name, multiplier)` with the multiplier
    /// in the same slot for readability — callers that need to
    /// distinguish should consult the trace directly.
    pub modifier_deltas: Vec<(String, f32)>,
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
    // Pull the L3 record (one per tick), the L2 records (one per
    // captured DSE), and the ranked softmax pool out of whatever was
    // emitted; clear so the next tick starts fresh. L1 / L3Commitment /
    // L3PlanFailure variants flow through but aren't surfaced in the
    // CLI report — read `TraceLog` directly if a future investigation
    // needs them.
    let mut chosen: Option<String> = None;
    let mut ranked: Vec<(String, f32)> = Vec::new();
    let mut softmax_probs: Vec<f32> = Vec::new();
    let mut l2: Vec<L2RowSummary> = Vec::new();
    let mut pre_bonus_pool: Vec<(String, f32)> = Vec::new();
    let mut pre_penalty_pool: Vec<(String, f32)> = Vec::new();
    for entry in log.entries.drain(..) {
        let TraceEntry { record, .. } = entry;
        match record {
            TraceRecord::L3 {
                chosen: c,
                ranked: r,
                softmax,
                pre_bonus_pool: pb,
                pre_penalty_pool: pp,
                ..
            } => {
                chosen = Some(c);
                ranked = r;
                softmax_probs = softmax.probabilities;
                pre_bonus_pool = pb;
                pre_penalty_pool = pp;
            }
            TraceRecord::L2 {
                dse,
                eligibility,
                composition,
                maslow_pregate,
                modifiers,
                final_score,
                ..
            } => {
                l2.push(L2RowSummary {
                    dse,
                    eligible: eligibility.passed,
                    maslow_pregate,
                    raw_score: composition.raw,
                    gated_score: composition.raw * maslow_pregate,
                    final_score,
                    modifier_deltas: modifiers
                        .into_iter()
                        .map(|ModifierApplication { name, delta, multiplier }| {
                            // Additive modifiers carry `delta`; the few
                            // multiplicative ones surface their multiplier
                            // here so a future fox-territory suppression
                            // (or similar) is visible without the caller
                            // re-reading the trace. Drop rows where both
                            // are absent (shouldn't happen — guard rather
                            // than panic).
                            let value = delta.or(multiplier).unwrap_or(0.0);
                            (name, value)
                        })
                        .collect(),
                });
            }
            _ => {}
        }
    }
    TickReport {
        tick,
        chosen,
        ranked,
        softmax_probs,
        l2,
        pre_bonus_pool,
        pre_penalty_pool,
    }
}
