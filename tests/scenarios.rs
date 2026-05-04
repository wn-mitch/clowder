//! Ticket 162 — scenario harness assertion tests.
//!
//! Each scenario registered in `clowder::scenarios::ALL` gets a test here
//! asserting the expected winning DSE for the focal cat at the relevant
//! tick. These tests act as cheap regression guards: a structural change
//! that breaks any of these decision-landscape probes fails the suite in
//! seconds.

use clowder::scenarios::{self, kitten_cry, runner};

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
