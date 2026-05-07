//! Substrate-driven plan preemption scenario — tickets 118 + 119.
//!
//! A wounded cat already engaged in a Hunt plan should re-elect under
//! the substrate-driven preempt path (not the legacy CriticalHealth
//! interrupt — retired in 119) when the `AcuteHealthAdrenalineFlee`
//! modifier's lurch threshold is crossed. Closes the gap surfaced in
//! ticket 047 Phase 2: Sleep won the L2 softmax in 99.3% of injured-
//! window ticks but was the chosen action only 1.4% of them, because
//! plan-completion momentum gated behavior.
//!
//! Default expectation: `Feature::ModifierPreemption` fires ≥ 1 time
//! within 80 ticks under production-default constants — ticket 119
//! promoted 047's lifts from 0.0 to 0.60 (Flee) / 0.50 (Sleep), so
//! no in-test override is needed. Per the substrate-fires landing
//! gate (ticket 198), this scenario is a sibling assertion that the
//! preempt path is reachable end-to-end on stock config.

use bevy_ecs::world::World;

use crate::components::physical::{Health, Position};
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "modifier_preempts_hunt",
    default_focal: "Mallow",
    default_ticks: 80,
    setup,
    expected_features: &["ModifierPreemption"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Forest tile near the cat — the per-tick `update_capability_markers`
    // author gates `CanHunt` on forest-nearby. Default scenario
    // terrain is all Grass; without this the marker gets stripped on
    // tick 1 and Hunt's eligibility filter rejects every subsequent
    // score, so the cat never enters a Hunt plan and the scenario is
    // silent. Mirrors `hunt_deposit_chain::setup`.
    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        if map.in_bounds(21, 20) {
            map.get_mut(21, 20).terrain = Terrain::LightForest;
        }
    }

    // Stores building so the Hunt → DepositPrey plan template has a
    // valid PlannerZone::Stores resolution (otherwise the planner
    // refuses to make a Hunt plan).
    use crate::components::building::{StoredItems, Structure, StructureType};
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        Position::new(18, 20),
    ));

    // Spawn the focal cat. High hunger urgency + Hunt eligibility
    // biases the softmax toward Hunt despite the adrenaline Sleep
    // lift, so the cat actually enters a Hunt plan that can be
    // preempted on the next tick. (If Sleep wins immediately, the
    // cat goes to Resting which is exempt from preemption — no
    // Feature::ModifierPreemption fires, scenario silent.)
    let cat = spawn_cat(
        world,
        CatPreset::adult("Mallow", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.8;
                p.diligence = 0.8;
                // Low patience so commitment-tenure doesn't anchor the
                // cat into Hunt past the preempt point.
                p.patience = 0.3;
            })
            .with_needs(|n| {
                // Strong hunger urgency biases toward Hunt over Sleep.
                n.hunger = 0.30;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Wound the cat to deficit 0.6 (saturates 047's smoothstep above
    // threshold 0.4 + transition-width 0.1 = 0.5). With the modifier
    // activated above, preempts_in_flight returns true on every tick
    // the cat has a non-recovery plan.
    world.entity_mut(cat).insert(Health {
        current: 0.4,
        max: 1.0,
        injuries: Vec::new(),
        total_starvation_damage: 0.0,
    });

    // Prey east of the cat — gives Hunt a real target so the planner
    // produces a plan rather than failing to materialize one.
    spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(25, 21), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(25, 19), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(26, 20), PreyKind::Mouse);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Ticket 118 fix lock: a wounded cat with a non-recovery plan
    /// must trigger `Feature::ModifierPreemption` at least once. If
    /// this fails, the substrate-driven preempt path is structurally
    /// broken — `check_modifier_preemption` either isn't running, the
    /// trait override on `AcuteHealthAdrenalineFlee` isn't returning
    /// true, or `try_preempt`-equivalent isn't dropping the plan.
    /// (The `cargo test scenario_feature_assertions` integration test
    /// also enforces this via `expected_features`; the duplicate here
    /// is intentional — it captures the run report directly so a
    /// failure surfaces a richer diagnostic.)
    #[test]
    fn substrate_preempts_wounded_cats_hunt_plan() {
        let report = run(&SCENARIO, None, Some(80), 42);
        let preempt_count = report
            .feature_counts
            .get("ModifierPreemption")
            .copied()
            .unwrap_or(0);
        assert!(
            preempt_count >= 1,
            "expected substrate-driven preempt to fire ≥ 1× in 80 ticks; \
             got {preempt_count}. winner_counts: {:?}",
            report.winner_counts()
        );
    }
}
