//! Ticket 310 S1 — shadow-fox hunger-hunt cycle.
//!
//! One starving shadow-fox (satiation 0.0) ten tiles from a four-cat
//! cluster on clean ground — outside `base_detection_range` (8), so
//! the legacy 5%/tick stalk roll cannot see the cats, but inside the
//! motivation scan radius (12), so the hunger drive can. With Coherence/Resonance/Entropy at zero
//! (full coherence, no corruption, no wards) and Dread group-suppressed
//! (every cat has ≥ 2 allies in isolation radius), the hunger drive
//! `(1 − satiation)² × shadow_fox_hunger_drive_weight` is the only live
//! pressure: the motivation softmax elects Stalking
//! (`Feature::ShadowFoxHungerHuntEntered`), the fox closes and ambushes,
//! the kill feeds it past `shadow_fox_stalk_satiation_threshold`, and no
//! further Stalking occurs for the rest of the run — the hunt-feed-rest
//! cycle the ticket's Verification section names.
//!
//! Confound pins: softmax jitter 0 + near-argmax temperature make the
//! election deterministic; `shadow_fox_haunting_escalation_ticks` is
//! pinned to `u64::MAX` so a post-ambush Haunting election (Dread is
//! alive once the victim's safety drops) can never promote itself to
//! Stalking through the 023 psychological-escalation path — any
//! Stalking observed after the ambush would be a satiation-gate defect.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::wildlife::{ShadowFoxDrives, WildAnimal, WildSpecies, WildlifeAiState};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::CatPreset;
use super::Scenario;

/// Ten tiles from the cluster: beyond the legacy roll's sight range
/// (`base_detection_range` 8.0), within the hunger scan (12) — the
/// only path into Stalking from here is the motivation election.
const FOX_POS: Position = Position::new(10, 20);
/// Cluster anchor; the four cats sit in a 2×2 block so every one has
/// ≥ 2 allies within `shadow_fox_dread_isolation_radius` (8) and Dread
/// stays group-suppressed (×0.2) below the hunger pressure.
const VICTIM_POS: Position = Position::new(20, 20);

pub static SCENARIO: Scenario = Scenario {
    name: "shadowfox_hunger_hunt_cycle",
    default_focal: "Whisker",
    default_ticks: 120,
    setup,
    expected_features: &["ShadowFoxHungerHuntEntered"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        // Deterministic election: no jitter, near-argmax temperature.
        constants.wildlife.shadow_fox_motivation_jitter = 0.0;
        constants.wildlife.shadow_fox_motivation_softmax_temp = 0.001;
        // Pin off the 023 Haunting → Stalking escalation so the only
        // paths into Stalking are the two satiation-gated ones under
        // test (hunger election + legacy 5%/tick roll).
        constants.wildlife.shadow_fox_haunting_escalation_ticks = u64::MAX;
    }

    // 2×2 cat cluster — Whisker is the nearest cat (the hunger target).
    spawn_cat(world, CatPreset::adult("Whisker", VICTIM_POS));
    spawn_cat(world, CatPreset::adult("Bramble", Position::new(20, 21)));
    spawn_cat(world, CatPreset::adult("Sorrel", Position::new(21, 20)));
    spawn_cat(world, CatPreset::adult("Fen", Position::new(21, 21)));

    // Starving shadow-fox on clean ground: full coherence (no
    // Reconstituting pull), satiation 0.0 (hunger pressure 0.10 at the
    // first-light weight). Bundle mirrors the production spawn in
    // `spawn_shadow_fox_from_corruption`; the `OnAdd<WildAnimal>`
    // observer authors the movement components.
    world.spawn((
        WildAnimal::new(WildSpecies::ShadowFox),
        FOX_POS,
        WildlifeAiState::Patrolling { dx: 1, dy: 0 },
        ShadowFoxDrives::newly_manifested(0.9, 0.0),
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
    fn hunger_hunt_feeds_then_suppresses() {
        let mut app = build_scenario_app(42, &SCENARIO, SCENARIO.default_focal);

        let threshold = crate::resources::SimConstants::default()
            .wildlife
            .shadow_fox_stalk_satiation_threshold;

        let mut fed_at: Option<u32> = None;
        for tick in 0..SCENARIO.default_ticks {
            app.update();

            let world = app.world_mut();
            let (drives, state) = {
                let mut q = world.query::<(&ShadowFoxDrives, &WildlifeAiState)>();
                let (d, s) = q
                    .single(world)
                    .expect("the scenario's lone shadow-fox should survive the run");
                (d.clone(), s.clone())
            };

            if fed_at.is_none() && drives.satiation >= threshold {
                fed_at = Some(tick);
            }
            if let Some(fed_tick) = fed_at {
                // Post-kill: both Stalking entries (hunger election,
                // legacy roll) are satiation-gated, and escalation is
                // pinned off — Stalking here is a gate defect.
                if tick > fed_tick {
                    assert!(
                        !matches!(state, WildlifeAiState::Stalking { .. }),
                        "fed shadow-fox (satiation {:.2} ≥ {threshold}) re-entered Stalking at tick {tick}",
                        drives.satiation,
                    );
                }
            }
        }

        let fed_tick = fed_at.expect(
            "the starving shadow-fox should have elected the hunger hunt, ambushed, and fed within the run",
        );
        assert!(
            fed_tick < SCENARIO.default_ticks - 10,
            "ambush landed too late ({fed_tick}) to observe the suppressed window",
        );

        let world = app.world_mut();
        let hunger_hunts = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::ShadowFoxHungerHuntEntered)
            .copied()
            .unwrap_or(0);
        assert!(
            hunger_hunts >= 1,
            "the hunt must have entered through the hunger election, not the legacy roll",
        );
    }
}
