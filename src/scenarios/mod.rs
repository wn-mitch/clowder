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
pub mod disposal_election;
pub mod dying_arc_softmax;
pub mod env;
pub mod exploration_ranging;
pub mod farm_herb_demand;
pub mod farming_cycle;
pub mod flee_commitment;
pub mod fondness_kitten_imprint;
pub mod grooming_other;
pub mod hunt_acquisition;
pub mod hunt_deposit_chain;
pub mod inventory_full_no_pickup;
pub mod kitten_cry;
pub mod lone_burial;
pub mod modifier_preempts_hunt;
pub mod picking_up_scavenging;
pub mod preset;
pub mod route_cost_decision;
pub mod runner;
pub mod ward_placement;
pub mod wildlife_fight;
pub mod wounded_cat_no_pickup;

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
    /// Ticket 198 — substrate-fires landing gate. `Feature::*` variants
    /// (by name) the scenario expects to emit ≥ 1× during the run. The
    /// `cargo test scenario_feature_assertions` integration test runs
    /// every scenario with a non-empty `expected_features` and asserts
    /// each declared Feature actually fired in `SystemActivation.counts`.
    /// Empty `&[]` opts out — legitimate for scenarios whose only
    /// purpose is L2/L3 election triage (no Feature emission expected).
    /// Scenarios exercising rare-tier outcomes (legend events, etc.)
    /// also opt out — the absence is the contract.
    pub expected_features: &'static [&'static str],
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
    // 086 — Farm DSE herb-pressure axis (084) integration gate.
    // Deterministic Thornbriar tend→harvest cycle that asserts both
    // `Feature::CropTended` and `Feature::CropHarvested` fire. The
    // soak canary stays demoted; this scenario is the structural-
    // correctness surface. See module rustdoc for the empirical
    // reframe (085 evidence, sociology/economy follow-on parked).
    &farm_herb_demand::SCENARIO,
    // 158 — triage harness for the GroomedOther never-fired structural fix.
    &grooming_other::SCENARIO,
    // 178 — election-side scenarios for the lifted disposal DSEs.
    &disposal_election::SCENARIO_TRASHING,
    &disposal_election::SCENARIO_DISCARDING,
    &disposal_election::SCENARIO_IDLE,
    &disposal_election::SCENARIO_DISCARDING_BLOCKED,
    // 193 — election-side scenario for the rerouted PickingUp plan
    // template (PlannerZone::CarcassPile).
    &picking_up_scavenging::SCENARIO,
    // 118 — substrate-driven plan preemption for acute-class lurch
    // modifiers. Asserts Feature::ModifierPreemption fires when a
    // wounded cat is mid-Hunt and the AcuteHealthAdrenaline lurch
    // crosses its threshold.
    &modifier_preempts_hunt::SCENARIO,
    // 228 — bold-vs-timid route-cost suppression microexperiment.
    // Lifts hunt_route_cost_weight to 1.0 locally; canonical soak
    // constants ship at 0.0 (substrate-dormant).
    &route_cost_decision::SCENARIO,
    // 230 — substrate-aware Fleeing chain end-to-end smoke. Wounded
    // cat with adjacent fox + saturated naive-projection corridor.
    // Asserts `FleeTargetPicked` fires; the picker is reachable
    // through the `Action::Flee → DispositionKind::Fleeing` route
    // closing the last anxiety-interrupt arm migration.
    &flee_commitment::SCENARIO,
    // 231 — capacity-aware pickup pipeline. Three sister scenarios:
    // full-of-curios + adjacent food drops the curio first then picks
    // up; full-of-herbs validates the ItemSlot collapse; empty cat
    // takes the substrate path with no DropItem prefix.
    &inventory_full_no_pickup::SCENARIO_FULL_CURIOS,
    &inventory_full_no_pickup::SCENARIO_FULL_HERBS,
    &inventory_full_no_pickup::SCENARIO_EMPTY_PICKUP,
    // 231 R3b — wounded cat L2 score regression. Reproduces the
    // dying-arc analysis: HP=0.49 + adjacent food → PickUp's L2
    // final_score is multiplicatively damped by `health_deficit`
    // (post-R3b) instead of scoring near 1.0 (pre-R3b).
    &wounded_cat_no_pickup::SCENARIO,
    // 232 — body-state-coupled L3 softmax temperature triage harness.
    // Wounded cat (HP=0.49) + adjacent fox saturates body_distress
    // and threat_proximity, driving softmax_temperature to the floor.
    // Fix-lock requires 231 + 232 together; load-bearing assertions
    // live in src/ai/scoring.rs unit tests.
    &dying_arc_softmax::SCENARIO,
    // 035 — lone-burial foundation gate. One adult adjacent to a
    // freshly-tagged `Dead` colony-mate; the chain must run end-to-end
    // and emit `Feature::BurialPerformed` so the burial canary's
    // mechanism is independently verified from the soak-side death-
    // rate dynamics.
    &lone_burial::SCENARIO,
];

/// Look up a scenario by its `name` field.
pub fn by_name(name: &str) -> Option<&'static Scenario> {
    ALL.iter().copied().find(|s| s.name == name)
}
