//! Ticket 288 microexperiment — `morale_break` releases the Guarding
//! commitment instead of triggering an in-disposition replan.
//!
//! Reproduces Cedar's death pattern at tick 1282037 in the post-271
//! verification soak. A wounded cat (HP below
//! `fight_bail_health_threshold`) inside a Guarding plan reaches
//! `EngageThreat`; the resolver returns `Fail("morale_break")`. Pre-fix
//! the GOAP dispatcher replanned inside Guarding to
//! `[TravelTo(PatrolZone), Survey]` and the cat walked back into ambush
//! range. Post-fix the dispatcher releases the commitment so L3 re-elects
//! on the next tick — wounded cats can drop to Sleep or Flee.
//!
//! ## Why the plan is pre-inserted
//!
//! Reaching `EngageThreat` through the natural play loop requires:
//! a wounded cat that elects Guarding at L3 *and* carries a Fight
//! directive that overrides the cheap-Survey plan template
//! (`src/systems/goap.rs:~2354`). The L3 softmax pool for a wounded
//! cat is dominated by Sleep (BodyDistress promotion lifts it onto
//! a wounded cat reliably), so the natural-play reproduction is
//! non-deterministic across short tick budgets — that's the
//! patrol-absorption-cascade dynamics the smoking-gun trace
//! captured (Cedar elected Guarding while healthy, took ambush
//! damage to HP=0.10, then a Fight directive landed mid-plan).
//!
//! This scenario short-circuits the long state-mutation chain by
//! pre-inserting a Guarding `GoapPlan` with one step
//! (`EngageThreat`) onto a wounded cat. `evaluate_and_plan` skips
//! the cat (query filter `Without<GoapPlan>`), and
//! `resolve_goap_plans` advances the step on the first tick — the
//! resolver's HP gate (`src/steps/disposition/fight_threat.rs:45`)
//! returns `Fail("morale_break")` immediately, exercising the new
//! dispatcher branch.
//!
//! ## Assertions
//!
//! - `expected_features: ["CommitmentDropMoraleBreak"]` — the integration
//!   gate from `scenario_feature_assertions` verifies the new Feature
//!   fires at least once during the run. The Feature variant is only
//!   bumped by `record_drop(_, _, DropBranch::MoraleBreak)`, which is
//!   only reachable through the ticket-288 short-circuit.

use bevy_ecs::world::World;

use crate::ai::planner::{GoapActionKind, PlannedStep};
use crate::ai::{Action, CurrentAction};
use crate::components::disposition::DispositionKind;
use crate::components::goap_plan::GoapPlan;
use crate::components::personality::Personality;
use crate::components::physical::{Health, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub const FOCAL_NAME: &str = "Watcher";
pub const FOCAL_START: Position = Position::new(30, 30);

pub static SCENARIO: Scenario = Scenario {
    name: "guarding_morale_break_releases",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup,
    expected_features: &["CommitmentDropMoraleBreak"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, FOCAL_START)
            .with_personality(|p| {
                p.boldness = 0.55;
                p.diligence = 0.85;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                // Neutral safety — keeps `safety_deficit` low so the
                // `ThreatProximityAdrenalineFlee` modifier's preempt
                // gate (rising derivative on `safety_deficit_now -
                // safety_deficit_prev`) doesn't strip the plan before
                // the executor runs. PrevSafetyDeficit defaults to 0.0
                // at spawn, so a low-safety cat produces a spurious
                // rising derivative on tick 1.
                n.safety = 0.7;
                n.hunger = 0.7;
                n.energy = 0.8;
            })
            .with_marker(MarkerKind::Adult),
    );

    // Wound the cat below `fight_bail_health_threshold = 0.35`.
    world.entity_mut(cat).insert(Health {
        current: 0.10,
        max: 1.0,
        total_starvation_damage: 0.0,
    });

    // Pre-populate PrevSafetyDeficit so threat_proximity_derivative is
    // zero on tick 1 — otherwise the rising-derivative modifier preempts
    // the plan before resolve_goap_plans can advance the EngageThreat
    // step. spawn_cat_from_blueprint defaults this to 0.0.
    let initial_safety_deficit = 1.0 - 0.7;
    world
        .entity_mut(cat)
        .insert(crate::components::PrevSafetyDeficit(initial_safety_deficit));

    // Pre-insert the Guarding plan with `[EngageThreat]` so the
    // dispatcher reaches the resolver on tick 1. `evaluate_and_plan`
    // queries `Without<GoapPlan>` so the L3 election is skipped for
    // this cat; `resolve_goap_plans` consumes the plan directly.
    let tick = world.resource::<crate::resources::TimeState>().tick;
    let personality = world
        .get::<Personality>(cat)
        .cloned()
        .expect("spawn_cat inserts Personality");
    let plan = GoapPlan::new(
        DispositionKind::Guarding,
        Action::Fight,
        tick,
        &personality,
        vec![PlannedStep {
            action: GoapActionKind::EngageThreat,
            cost: 1,
        }],
    );
    world.entity_mut(cat).insert(plan);

    // Sync CurrentAction so the executor's per-tick loop sees the
    // intent to act. `spawn_cat_from_blueprint` defaulted this to
    // `Idle` with `ticks_remaining = 0`.
    world.entity_mut(cat).insert(CurrentAction {
        action: Action::Fight,
        ticks_remaining: 1,
        target_position: None,
        target_entity: None,
        last_scores: Vec::new(),
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner;

    /// The load-bearing assertion: `Feature::CommitmentDropMoraleBreak`
    /// fires at least once during the run, witnessing that the wounded
    /// cat reached `EngageThreat`, returned `Fail("morale_break")`, and
    /// the new dispatcher short-circuit released the commitment instead
    /// of replanning inside Guarding.
    ///
    /// This is the same gate `scenario_feature_assertions` enforces via
    /// `expected_features`; the inline test makes the assertion local
    /// and survives changes to the global gate harness.
    #[test]
    fn morale_break_release_fires_commitment_drop_feature() {
        let report = runner::run(&SCENARIO, None, None, 42);
        let count = report
            .feature_counts
            .get("CommitmentDropMoraleBreak")
            .copied()
            .unwrap_or(0);
        assert!(
            count >= 1,
            "expected CommitmentDropMoraleBreak >= 1, got {count}. \
             Feature counts: {:?}",
            report.feature_counts
        );
    }
}
