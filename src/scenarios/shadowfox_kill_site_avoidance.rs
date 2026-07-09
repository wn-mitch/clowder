//! Ticket 310 S3 — kill-site avoidance (fished-out pond).
//!
//! A starving shadow-fox holds a fresh kill-site memory on top of the
//! nearer cat cluster. The hunger election must pass over that ground
//! and hunt the farther, clean-ground cat instead — and the passed-over
//! choice must be *named* (`Feature::ShadowFoxKillSiteAvoided`), not a
//! silent movement-layer artifact.
//!
//! Geometry: fox Waiting at (10,20) (the legacy roll only fires from
//! Patrolling/Circling, so the election is the only entry). Fished
//! cluster at (16,20)± — distance 6, inside both the motivation scan
//! (12) and the kill-site radius (6 of the stamped site). Clean cat
//! "Whisker" at (10,28) — distance 8: farther than the excluded
//! cluster (so the exclusion is memory, not geometry) but well inside
//! the scan.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::wildlife::{
    ShadowFoxBeliefs, ShadowFoxDrives, WildAnimal, WildSpecies, WildlifeAiState,
};
use crate::resources::time::TimeState;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::CatPreset;
use super::Scenario;

const FOX_POS: Position = Position::new(10, 20);
const KILL_SITE: (i32, i32) = (16, 20);
const CLEAN_CAT_POS: Position = Position::new(10, 28);

pub static SCENARIO: Scenario = Scenario {
    name: "shadowfox_kill_site_avoidance",
    default_focal: "Whisker",
    default_ticks: 40,
    setup,
    expected_features: &["ShadowFoxKillSiteAvoided"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        constants.wildlife.shadow_fox_motivation_jitter = 0.0;
        constants.wildlife.shadow_fox_motivation_softmax_temp = 0.001;
        constants.wildlife.shadow_fox_haunting_escalation_ticks = u64::MAX;
    }

    // Fished cluster — nearer than the clean cat, all within the
    // kill-site avoid radius of KILL_SITE.
    spawn_cat(world, CatPreset::adult("Bramble", Position::new(16, 20)));
    spawn_cat(world, CatPreset::adult("Sorrel", Position::new(16, 21)));
    spawn_cat(world, CatPreset::adult("Fen", Position::new(17, 20)));
    // Clean ground, farther out.
    spawn_cat(world, CatPreset::adult("Whisker", CLEAN_CAT_POS));

    let now = world.resource::<TimeState>().tick;
    world.spawn((
        WildAnimal::new(WildSpecies::ShadowFox),
        FOX_POS,
        WildlifeAiState::Waiting,
        ShadowFoxDrives::newly_manifested(0.9, 0.0),
        ShadowFoxBeliefs {
            den_position: Some((FOX_POS.x(), FOX_POS.y())),
            last_kill_site: Some(KILL_SITE),
            last_kill_tick: now,
        },
        crate::components::physical::Health::default(),
        crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
        crate::components::SensorySignature::WILDLIFE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::resources::system_activation::{Feature, SystemActivation};
    use crate::scenarios::runner::build_scenario_app;

    #[test]
    fn hunger_hunts_clean_ground_and_names_the_exclusion() {
        let mut app = build_scenario_app(42, &SCENARIO, SCENARIO.default_focal);

        let mut stalk_target: Option<(i32, i32)> = None;
        for _ in 0..SCENARIO.default_ticks {
            app.update();
            let world = app.world_mut();
            let mut q = world.query::<(&ShadowFoxDrives, &WildlifeAiState)>();
            let (_, state) = q
                .single(world)
                .expect("the scenario's lone shadow-fox should survive the run");
            if let WildlifeAiState::Stalking { target_x, target_y } = state {
                stalk_target = Some((*target_x, *target_y));
                break;
            }
        }

        let (tx, ty) =
            stalk_target.expect("the starving fox should elect a hunger hunt within the run");
        let clean = Position::new(tx, ty);
        assert!(
            clean.distance_to(&Position::new(KILL_SITE.0, KILL_SITE.1)) > 6.0,
            "the elected target must sit outside the fished-out radius; got ({tx},{ty})",
        );

        let world = app.world_mut();
        let avoided = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::ShadowFoxKillSiteAvoided)
            .copied()
            .unwrap_or(0);
        assert!(avoided >= 1, "the passed-over nearer cluster must be named");
    }
}
