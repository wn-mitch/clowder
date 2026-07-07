//! Ticket 162 — scenario harness assertion tests.
//!
//! Each scenario registered in `clowder::scenarios::ALL` gets a test here
//! asserting the expected winning DSE for the focal cat at the relevant
//! tick. These tests act as cheap regression guards: a structural change
//! that breaks any of these decision-landscape probes fails the suite in
//! seconds.

use std::collections::HashMap;

use clowder::scenarios::{
    self, colony_knowledge_false_belief, fish_shoreline_pounce, kitten_cry, runner,
};

/// Drift-control smoke test: every registered scenario must run for at
/// least one tick without panicking. Catches the "build_new_world starts
/// inserting a new resource that init_scenario_world misses" failure
/// mode before that drift propagates to a per-tick system that reads
/// the resource via `world.resource::<T>()` and crashes.
#[test]
fn all_scenarios_smoke_run_one_tick() {
    for scenario in scenarios::ALL {
        let report = runner::run(scenario, None, Some(1), 42);
        assert_eq!(
            report.ticks.len(),
            1,
            "scenario `{}` did not produce a single-tick report",
            scenario.name
        );
    }
}

/// Ticket 198 — substrate-fires landing gate. Every scenario whose
/// `expected_features` is non-empty must actually emit each declared
/// `Feature::*` at least once during its default-tick run. A scenario
/// that lies about which Features it exercises masks the failure mode
/// the gate exists to prevent (curve-lifted-without-resolver-wiring,
/// the 185-shape regression).
///
/// Empty `expected_features: &[]` opts out — appropriate for L2/L3
/// election-triage scenarios that don't reach Feature emission, and
/// for scenarios exercising rare-tier outcomes whose absence is the
/// contract.
#[test]
fn declared_expected_features_all_fire() {
    let mut failures: Vec<String> = Vec::new();
    for scenario in scenarios::ALL {
        if scenario.expected_features.is_empty() {
            continue;
        }
        let report = runner::run(scenario, None, None, 42);
        for &feature in scenario.expected_features {
            let count = report.feature_counts.get(feature).copied().unwrap_or(0);
            if count == 0 {
                failures.push(format!(
                    "scenario `{}` declared Feature::{} but it fired 0× across {} ticks",
                    scenario.name,
                    feature,
                    report.ticks.len(),
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} scenario(s) failed the substrate-fires gate:\n  - {}",
        failures.len(),
        failures.join("\n  - "),
    );
}

#[test]
fn kitten_cry_basic_emits_focal_trace_with_caretake_in_ranked_list() {
    let report = runner::run(&kitten_cry::SCENARIO, None, None, 42);

    // Invariant 1: focal-cat name resolution produces at least one tick
    // with a chosen value. If this fails, either the spawn isn't
    // creating a cat named "Mallow" or `emit_focal_trace`'s lazy entity
    // resolution broke.
    let any_chosen = report.ticks.iter().any(|t| t.chosen.is_some());
    assert!(
        any_chosen,
        "no tick emitted an L3 record for focal cat — focal name resolution failed. Report: {:#?}",
        report
    );

    // Invariant 2: at least one of Mallow's L3 records ranks Caretake
    // with a non-zero score. This is the cry-broadcast architecture
    // smoke test — the IsParentOfHungryKitten marker + the cry-map
    // both feed the Caretake DSE; if both wires are dead, Caretake
    // never enters the ranked list at all.
    //
    // Note: this test deliberately does NOT assert Caretake *wins*.
    // The harness exists to surface and investigate score-distribution
    // questions like "why does Wander beat Caretake here?" — encoding
    // a hard "Caretake must win" assertion would block users from
    // observing real regressions during bugfix loops.
    let caretake_present_with_score = report.ticks.iter().any(|t| {
        t.ranked
            .iter()
            .any(|(name, score)| name == "Caretake" && *score > 0.0)
    });
    assert!(
        caretake_present_with_score,
        "Caretake never appeared in the ranked DSE list with a positive score across {} ticks — cry-broadcast architecture (ticket 156) appears broken. Report: {:#?}",
        report.ticks.len(),
        report
    );
}

/// Locked §11.3 invariant: nothing mutates the action-keyed score Vec
/// between `score_actions` exit and softmax entry. The two snapshots
/// (`pre_bonus_pool` from goap.rs, `pre_penalty_pool` from the softmax
/// helper) include jitter identically and must agree per-action.
#[test]
fn pre_bonus_equals_pre_penalty_across_all_scenarios() {
    const EPSILON: f32 = 1e-4;

    for scenario in scenarios::ALL {
        let report = runner::run(scenario, None, None, 42);
        for tick in &report.ticks {
            if tick.pre_penalty_pool.is_empty() {
                // Softmax fall-through (empty filtered pool) — nothing
                // to compare. The runner emits these as legitimate
                // empty captures, not regressions.
                continue;
            }
            let pre_bonus: HashMap<&str, f32> = tick
                .pre_bonus_pool
                .iter()
                .map(|(a, s)| (a.as_str(), *s))
                .collect();
            for (action, pre_penalty_score) in &tick.pre_penalty_pool {
                let pre_penalty_score = *pre_penalty_score;
                let Some(&pre_bonus_score) = pre_bonus.get(action.as_str()) else {
                    panic!(
                        "scenario `{}` tick {}: action `{}` appears in pre_penalty_pool but not in pre_bonus_pool — a bonus pass appears to *introduce* a row",
                        scenario.name, tick.tick, action
                    );
                };
                assert!(
                    (pre_bonus_score - pre_penalty_score).abs() < EPSILON,
                    "scenario `{}` tick {} action `{}`: pre-bonus {} vs pre-penalty {} (Δ={}). Some code mutates the score Vec between score_actions and softmax.",
                    scenario.name,
                    tick.tick,
                    action,
                    pre_bonus_score,
                    pre_penalty_score,
                    pre_penalty_score - pre_bonus_score,
                );
            }
        }
    }
}

/// Ticket 467 — shoreline-pounce structural verification. The scenario
/// world holds exactly two fish: one a tile offshore (bank vantage
/// inside the pounce band → catchable) and one mid-lake (no passable
/// tile within any pounce band → must never be elected). A healthy
/// build kills exactly the shore fish; a pre-467 build kills neither
/// (the approach freezes at the shoreline until the stuck watchdog
/// aborts, on repeat), and an over-eager fix that drops the
/// reachability gate would produce visible churn on the mid-lake fish
/// without ever changing the count — which is why the count-based
/// assertion pairs with the `hunt_vantage` unit tests rather than
/// replacing them.
#[test]
fn fish_shoreline_pounce_kills_shore_fish_and_spares_mid_lake_fish() {
    let report = runner::run(&fish_shoreline_pounce::SCENARIO, None, None, 42);
    assert_eq!(
        report.final_prey_count, 1,
        "expected exactly the shore fish dead and the mid-lake fish alive; \
         2 = shoreline freeze regressed (no kill), 0 = the mid-lake fish was \
         somehow reachable (reachability gate broken)"
    );
}

/// Ticket 291 — false-belief promotion with a citable witness chain,
/// at full-app integration (real schedule; belief decay running).
/// The three-cat false consensus about a safe meadow MUST promote
/// (substrate does not gate truth) and carry all three believers as
/// witnesses; the contested bucket (one alarmed cat vs two calm)
/// MUST stay out and register measured divergence.
#[test]
fn colony_knowledge_false_belief_promotes_with_witness_chain() {
    use clowder::components::mental::MemoryType;
    use clowder::resources::colony_knowledge::ColonyKnowledge;

    let mut app = runner::build_scenario_app(42, &colony_knowledge_false_belief::SCENARIO, "Rumor");
    app.update(); // Startup — scenario setup plants the beliefs.
    for _ in 0..colony_knowledge_false_belief::SCENARIO.default_ticks {
        app.update();
    }

    let world = app.world_mut();
    let knowledge = world.resource::<ColonyKnowledge>();
    let false_bucket =
        ColonyKnowledge::bucket_position(&clowder::components::physical::Position::new(
            colony_knowledge_false_belief::FALSE_THREAT_POS.0,
            colony_knowledge_false_belief::FALSE_THREAT_POS.1,
        ));
    let entry = knowledge
        .entries
        .iter()
        .find(|e| e.event_type == MemoryType::ThreatSeen && e.location == Some(false_bucket))
        .expect("the false consensus must promote — substrate does not gate truth");
    assert_eq!(
        entry.witnesses.len(),
        3,
        "the promoted entry must cite its three believers"
    );

    let contested_bucket =
        ColonyKnowledge::bucket_position(&clowder::components::physical::Position::new(
            colony_knowledge_false_belief::CONTESTED_POS.0,
            colony_knowledge_false_belief::CONTESTED_POS.1,
        ));
    assert!(
        !knowledge
            .entries
            .iter()
            .any(|e| e.location == Some(contested_bucket)),
        "the contested bucket must not promote"
    );
    assert!(
        knowledge.divergence_duration_ticks > 0,
        "the contested bucket must register measured divergence"
    );
}
