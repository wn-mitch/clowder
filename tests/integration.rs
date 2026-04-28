//! End-to-end integration tests.
//!
//! Runs the actual `SimulationPlugin` + `HeadlessIoPlugin` against a Bevy
//! `App` rather than a hand-maintained scaffold. Earlier revisions duplicated
//! `setup_world_exclusive` and `SimulationPlugin::build` in this file, which
//! drifted whenever a new resource or system was added to the production app
//! (PE-001). This file no longer touches simulation construction directly —
//! everything goes through the canonical plugin path.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::MinimalPlugins;

use clowder::components::physical::Needs;
use clowder::plugins::headless_io::{emit_headless_footer, HeadlessConfig, HeadlessIoPlugin};
use clowder::plugins::setup::AppArgs;
use clowder::plugins::simulation::SimulationPlugin;
use clowder::resources::{SimConfig, TimeScale};

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

const TEST_GAME_DAY_SECONDS: f32 = 16.666_667;

/// Build a headless `App` writing logs into `output_dir`. Mirrors
/// `run_headless` in `src/main.rs` — same plugin order, same time
/// scaffolding — so the test exercises the real production graph.
fn build_test_app(seed: u64, output_dir: &Path) -> App {
    let preview_scale = TimeScale::from_config(&SimConfig::default(), TEST_GAME_DAY_SECONDS);
    let hz = preview_scale.tick_rate_hz() as f64;
    let fixed_timestep = Duration::from_secs_f64(1.0 / hz);

    let config = HeadlessConfig {
        seed,
        // Large wall-clock budget — the test loop terminates by tick count,
        // so we never want the wall-time gate to fire.
        duration_secs: 86_400,
        log_path: output_dir.join("narrative.jsonl"),
        event_log_path: output_dir.join("events.jsonl"),
        trace_log_path: None,
        focal_cat: None,
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
    app.add_plugins(SimulationPlugin);
    app.add_plugins(HeadlessIoPlugin);

    app.insert_resource(TimeUpdateStrategy::ManualDuration(fixed_timestep));
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep(fixed_timestep);

    app
}

/// Drive `app` for exactly `ticks` updates, then write the headless footer.
/// Stops early if the app has already signaled exit (e.g. wipeout).
fn drive_for_ticks(app: &mut App, ticks: u64) {
    for _ in 0..ticks {
        if app.should_exit().is_some() {
            break;
        }
        app.update();
    }
    emit_headless_footer(app.world_mut());
}

/// Allocate a fresh per-test temp directory under the system temp area.
/// Returns the path; cleanup is the caller's responsibility (use [`cleanup`]).
fn make_test_dir(label: &str) -> PathBuf {
    // Combine pid + nanos so concurrent test threads don't collide.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "clowder-test-{}-{}-{}",
        label,
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).expect("create test temp dir");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Same seed + same binary + same machine → byte-identical `events.jsonl`.
///
/// This is the core determinism gate that lets `just verdict` and the
/// regression-measurement tooling treat re-runs of the same code as a true
/// control. If this test ever fails, a new source of nondeterminism
/// (HashMap iteration in the sim path, parallel-scheduler reordering, a
/// wall-clock read in the tick loop) has crept in. Investigate before
/// shipping any balance change.
#[test]
fn simulation_is_deterministic() {
    let dir_a = make_test_dir("determinism-a");
    let dir_b = make_test_dir("determinism-b");

    let ticks = 600u64;
    let mut app_a = build_test_app(42, &dir_a);
    drive_for_ticks(&mut app_a, ticks);
    drop(app_a); // ensure writers flush + close

    let mut app_b = build_test_app(42, &dir_b);
    drive_for_ticks(&mut app_b, ticks);
    drop(app_b);

    let bytes_a = std::fs::read(dir_a.join("events.jsonl"))
        .expect("events.jsonl missing for run A");
    let bytes_b = std::fs::read(dir_b.join("events.jsonl"))
        .expect("events.jsonl missing for run B");

    let identical = bytes_a == bytes_b;

    if !identical {
        // Leave the temp dirs in place so the failure is debuggable.
        panic!(
            "events.jsonl bytes differ across same-seed runs:\n  A: {} bytes ({:?})\n  B: {} bytes ({:?})\n  diff with: cmp {:?} {:?}",
            bytes_a.len(),
            dir_a,
            bytes_b.len(),
            dir_b,
            dir_a.join("events.jsonl"),
            dir_b.join("events.jsonl"),
        );
    }

    cleanup(&dir_a);
    cleanup(&dir_b);
}

/// 1000 ticks, no panic. Smoke test that the canonical headless path stays
/// runnable end to end.
#[test]
fn simulation_runs_1000_ticks_without_panic() {
    let dir = make_test_dir("smoke-1000");
    let mut app = build_test_app(42, &dir);
    drive_for_ticks(&mut app, 1000);
    cleanup(&dir);
}

/// Drive cats to near-starvation, then run for long enough that at least one
/// of them eats. Verifies the eat plumbing (sense food → plan → travel →
/// EatAtStores → hunger restored) is wired up end to end. Targeted at the
/// "canary fired but cat starved anyway" failure mode: if no cat's hunger
/// recovers, the eating loop is broken regardless of what the activation
/// counter says.
#[test]
fn cats_eat_when_hungry() {
    let dir = make_test_dir("eat");
    let mut app = build_test_app(42, &dir);

    // One tick to let setup + sync_food_stores populate state.
    app.update();

    // Drain hunger on every cat so eating is the highest-utility action.
    {
        let world = app.world_mut();
        let entities: Vec<Entity> = world
            .query_filtered::<Entity, With<Needs>>()
            .iter(world)
            .collect();
        for entity in entities {
            if let Some(mut needs) = world.get_mut::<Needs>(entity) {
                needs.hunger = 0.1;
            }
        }
    }

    // Plenty of ticks for at least one cat to commit to Eat and finish the
    // travel + restore loop.
    for _ in 0..400 {
        if app.should_exit().is_some() {
            break;
        }
        app.update();
    }

    let max_hunger = {
        let world = app.world_mut();
        world
            .query::<&Needs>()
            .iter(world)
            .map(|n| n.hunger)
            .fold(0.0_f32, f32::max)
    };

    cleanup(&dir);

    // Threshold semantics: hunger decays ~0.002/tick. Starting from 0.1 over
    // 400 ticks, a cat that never eats lands at 0.0 (clamped). Any value
    // above the floor means at least one cat completed the
    // sense → plan → travel → eat → restore loop.
    assert!(
        max_hunger > 0.0,
        "no cat ate over 400 ticks (max={max_hunger}); eating loop is broken"
    );
}
