//! Ticket 162 — scenario harness for fast deterministic AI decision triage.
//!
//! A scenario is a tiny preloaded world (1–5 cats with specific
//! needs/personality/markers/positions, optionally seeded influence-map
//! cells) that runs for a small number of ticks and reports which DSE the
//! focal cat picked at each tick. Wall-clock target ~3 seconds, vs. ~15
//! minutes for `just soak`.
//!
//! Scenarios bypass `build_new_world` via the [`crate::plugins::setup::WorldSetup`]
//! resource, so terrain and entity spawn are entirely under scenario
//! control. Helpers in [`env`] do the resource-init heavy lifting.
//!
//! # Bugfix discipline integration
//!
//! Per CLAUDE.md, the scenario harness is the **triage** tool: it answers
//! "given this state, which DSE wins?" cheaply. Reach for `just scenario
//! <name>` before `just soak` whenever a hypothesis names specific cat
//! state. `just soak` remains for whole-colony verification once a fix is
//! drafted.
//!
//! # Determinism
//!
//! `SimulationPlugin` already pins both Startup and FixedUpdate to the
//! single-threaded executor (`src/plugins/simulation.rs:115-132`), so
//! scenario runs are byte-deterministic per seed. The runner asserts this
//! invariant in tests via stdout-diff.

pub mod disposal_dispatch;
pub mod env;
pub mod exploration_ranging;
pub mod farming_cycle;
pub mod fondness_kitten_imprint;
pub mod grooming_other;
pub mod hunt_acquisition;
pub mod hunt_deposit_chain;
pub mod kitten_cry;
pub mod preset;
pub mod runner;
pub mod ward_placement;
pub mod wildlife_fight;

use bevy_ecs::world::World;

/// A scenario describes how to populate the world before tick 0 and how
/// long to run the focal-cat trace. Scenarios are static at-spawn state;
/// multi-step scripting (e.g., "drop hunger to 0.1 at tick 5") is
/// deliberately out of scope (see ticket 162 `## Out of scope`).
#[derive(Clone, Copy)]
pub struct Scenario {
    /// Stable identifier used by the CLI (`just scenario <name>`).
    pub name: &'static str,
    /// Default focal cat. The runner inserts `FocalTraceTarget { name }`;
    /// the trace-emit system at `src/systems/trace_emit.rs:99-116`
    /// resolves the entity by name on the first tick the cat exists.
    pub default_focal: &'static str,
    /// Per-scenario tick budget. Behaviors live on different timescales
    /// (kitten-cry triage settles in ~5 ticks; farming spans ~120). The
    /// CLI flag `--ticks N` overrides this.
    pub default_ticks: u32,
    /// Populate the world: terrain, resources, entities. Replaces
    /// `build_new_world` via the `WorldSetup` resource.
    pub setup: fn(&mut World, u64),
}

/// All scenarios known to the binary and the test suite. Adding a new
/// scenario means: write it under `src/scenarios/<name>.rs`, declare its
/// `pub static SCENARIO: Scenario = …`, append it here.
pub const ALL: &[&Scenario] = &[
    &kitten_cry::SCENARIO,
    &wildlife_fight::SCENARIO,
    &fondness_kitten_imprint::SCENARIO,
    &hunt_acquisition::SCENARIO,
    // 184 — kill→travel→DepositPrey pipeline regression triage.
    &hunt_deposit_chain::SCENARIO,
    // 184 — fix lock: injured cats can still elect Hunt.
    &hunt_deposit_chain::SCENARIO_INJURED,
    &exploration_ranging::SCENARIO,
    &ward_placement::SCENARIO,
    &farming_cycle::SCENARIO,
    // 158 — triage harness for the GroomedOther never-fired structural fix.
    &grooming_other::SCENARIO,
];

/// Look up a scenario by its `name` field.
pub fn by_name(name: &str) -> Option<&'static Scenario> {
    ALL.iter().copied().find(|s| s.name == name)
}
