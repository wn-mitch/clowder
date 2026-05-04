//! Grooming-other scenario — two high-warmth adults spawn one tile
//! apart, both with a high social deficit and adequate temperature.
//! The focal cat should pick `Action::GroomOther` (Maslow tier 2,
//! `groom_other_dse` saturating on `social_deficit` × warmth ×
//! `phys_satisfaction` × `social_warmth_deficit`) and commit to the
//! new `DispositionKind::Grooming` (158).
//!
//! Pre-158, this state would have been routed through Socializing's
//! `[SocializeWith (2), GroomOther (2)]` plan template, where A* at
//! `planner/mod.rs:437` pre-pruned `GroomOther` because both actions
//! produced the same `(SetInteractionDone(true), IncrementTrips)`
//! next-state — so `GroomedOther` never fired in soaks
//! (`logs/tuned-42`, seed 42, 8 sim years post-154).
//!
//! This scenario is the bugfix-discipline triage harness for that
//! defect class: a fast deterministic check that the structural split
//! actually surfaces `GroomOther` at the L3 softmax pick, and that the
//! GOAP planner emits a `[GroomOther]` chain rather than dropping it
//! in favor of `SocializeWith`.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::resources::Relationships;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "grooming_other",
    default_focal: "Affie",
    default_ticks: 20,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Two warm adults adjacent. `social = 0.4` produces a
    // `social_deficit` of 0.6 — past `groom_other_dse`'s
    // `Logistic(8, 0.3)` midpoint, so the deficit axis saturates
    // near 1.0. `temperature = 0.9` keeps `groom_self_dse`
    // (`thermal_deficit` × `Logistic(7, 0.6)`) near zero — without
    // this, a cold cat would prefer self-grooming and the test would
    // be measuring the wrong arm of the split.
    let affie = spawn_cat(
        world,
        CatPreset::adult("Affie", Position::new(20, 20))
            .with_personality(|p| {
                p.warmth = 0.9;
                p.sociability = 0.7;
                p.compassion = 0.7;
                // Low independence so the action-level penalty on
                // Coordinate / Socialize / Mentor doesn't accidentally
                // suppress Socialize's competitor and overstate
                // GroomOther's margin.
                p.independence = 0.2;
            })
            .with_needs(|n| {
                n.social = 0.4;
                n.temperature = 0.9;
            })
            .with_marker(MarkerKind::Adult),
    );

    let bondi = spawn_cat(
        world,
        CatPreset::adult("Bondi", Position::new(21, 20))
            .with_personality(|p| {
                p.warmth = 0.9;
                p.sociability = 0.7;
                p.compassion = 0.7;
                p.independence = 0.2;
            })
            .with_needs(|n| {
                n.social = 0.4;
                n.temperature = 0.9;
            })
            .with_marker(MarkerKind::Adult),
    );

    // Initialize the relationship pair so `groom_other_target_dse`'s
    // fondness axis has a row to read. Default-initialized fondness
    // sits in the neutral band (matches founder pairs); we don't
    // pre-bias it because the scenario should pin "GroomOther wins
    // even from a neutral starting fondness" — the pre-158 bug hid
    // this case completely.
    let mut rels = world.remove_resource::<Relationships>().unwrap_or_default();
    {
        let mut rng = world.resource_mut::<crate::resources::SimRng>();
        rels.init_pair(affie, bondi, &mut rng.rng);
    }
    world.insert_resource(rels);
}
