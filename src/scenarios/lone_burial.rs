//! 035: lone-burial scenario — one adult adjacent to a freshly-dead
//! colony-mate. The focal cat should pick `Action::Bury`, commit to
//! `DispositionKind::Burying`, walk to the corpse (already adjacent),
//! complete `bury_ticks` of work, emit `Feature::BurialPerformed` +
//! `EventKind::BurialFired`, and on the post-loop drain the corpse is
//! despawned and a `Grave` entity is spawned at its position.
//!
//! Why this scenario exists: seed-42 deep soaks on the post-230
//! healthy-colony regime have **zero deaths** in 15 minutes (cats
//! reliably flee shadow-fox ambushes), so the burial continuity canary
//! can't fire end-to-end in the canonical soak. This scenario exercises
//! the foundation deterministically by spawning the prerequisite (a
//! Dead colony-mate) directly. If it fires, the Bury chain is wired
//! correctly and the soak-side canary is gated only by death-rate
//! dynamics — a separate balance concern.

use bevy_ecs::world::World;

use crate::components::physical::{DeathCause, Dead, Position};
use crate::resources::Relationships;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "lone_burial",
    default_focal: "Mira",
    default_ticks: 200,
    setup,
    // 035: foundation gate — `Feature::BurialPerformed` must fire
    // ≥ 1× during the run. The scenario is sized so a single chain
    // (Bury at adjacent zone, no travel, then 60-tick bury_ticks)
    // resolves comfortably inside `default_ticks`.
    expected_features: &["BurialPerformed"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // The mourner — high warmth so the burial DSE saturates on the
    // `warmth` axis and beats the small idle/wander competitors.
    // Adjacent to the corpse so the planner doesn't need to resolve a
    // long path.
    let mira = spawn_cat(
        world,
        CatPreset::adult("Mira", Position::new(20, 20))
            .with_personality(|p| {
                p.warmth = 0.95;
                p.compassion = 0.9;
                // Low sociability so Socialize doesn't crowd out Bury at
                // the L3 softmax (Mira has no social peers anyway, but
                // keep the axis quiet defensively).
                p.sociability = 0.3;
                p.independence = 0.2;
            })
            .with_needs(|n| {
                // Above all the body-distress thresholds — a cat in
                // body distress would route to Eat/Sleep/Flee instead.
                n.hunger = 0.9;
                n.energy = 0.9;
                n.temperature = 0.9;
                n.social = 0.7;
                n.safety = 0.9;
            })
            .with_marker(MarkerKind::Adult),
    );

    // The deceased — adjacent to Mira at (21, 20). Spawned as a
    // normal adult, then immediately tagged `Dead` so sensing's
    // `dead_cats_q` query sees it on the first tick. The scenario
    // bypasses the natural death path because we want a deterministic
    // pre-existing corpse, not a side-effect of starvation/injury.
    let deceased = spawn_cat(
        world,
        CatPreset::adult("Hazel", Position::new(21, 20))
            .with_personality(|p| {
                p.warmth = 0.5;
            })
            .with_marker(MarkerKind::Adult),
    );
    let death_tick = world
        .get_resource::<crate::resources::TimeState>()
        .map(|t| t.tick)
        .unwrap_or(0);
    world.entity_mut(deceased).insert(Dead {
        // Scenario start_tick comes from `init_scenario_world`; the
        // cleanup_grace_period default of 500 ticks is plenty for a
        // 200-tick scenario, so the corpse persists through the run.
        tick: death_tick,
        cause: DeathCause::OldAge,
    });

    // Initialize the relationship pair so `bury_target_dse`'s
    // bond/kinship axes have a row to read. Mira and Hazel start
    // neutral — burial fires even without a pre-existing bond, by
    // design (community duty, not personal grief).
    let mut rels = world.remove_resource::<Relationships>().unwrap_or_default();
    {
        let mut rng = world.resource_mut::<crate::resources::SimRng>();
        rels.init_pair(mira, deceased, &mut rng.rng);
    }
    world.insert_resource(rels);
}
