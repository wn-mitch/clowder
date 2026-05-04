//! Ticket 162 — scenario harness assertion tests.
//!
//! Each scenario registered in `clowder::scenarios::ALL` gets a test here
//! asserting the expected winning DSE for the focal cat at the relevant
//! tick. These tests act as cheap regression guards: a structural change
//! that breaks any of these decision-landscape probes fails the suite in
//! seconds.

use std::collections::HashMap;

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

/// Ticket-163 locked invariant: nothing mutates the action-keyed score
/// Vec between `score_actions` exit and softmax entry.
///
/// Two snapshots are captured per focal-cat tick:
/// - `pre_bonus_pool` — score Vec at `score_actions` exit, before any
///   bonus pass mutates it. Snapshotted in
///   `goap.rs::evaluate_and_plan` for the focal cat.
/// - `pre_penalty_pool` — post-filter, pre-Independence-penalty pool
///   the softmax saw, snapshotted in
///   `select_disposition_via_intention_softmax_with_trace`.
///
/// Both are action-keyed, both include jitter (jitter is added once
/// at `score_actions` push time and is therefore identical in both
/// snapshots — no jitter accounting needed). The softmax filter drops
/// `Flee` / `Idle` / zero-scoring actions; the test compares only the
/// actions present in `pre_penalty_pool`.
///
/// Today the §3.5.1 modifier pipeline ships with 19 modifiers but
/// `goap.rs::evaluate_and_plan` runs 9 additional `apply_*` passes
/// after `score_actions` returns. Each of those passes mutates the
/// score Vec, so this invariant fails on every focal cat where any
/// of the 9 fire. Ticket 163's full-batch migration ports each pass
/// to a registered `ScoreModifier`, retiring the imperative chain;
/// once landed, this test becomes a permanent CI guard against
/// re-introducing the antipattern.
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
                    "scenario `{}` tick {} action `{}`: pre-bonus {} vs pre-penalty {} (Δ={}). Some code mutates the score Vec between score_actions and softmax — see ticket 163.",
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
