//! Hunt acquisition scenario — a hungry skilled hunter cat is placed
//! near a single mouse on flat terrain. Probes the Hunting decision
//! landscape: Hunt DSE eligibility (`CanHunt` marker), the
//! Hunting-vs-Foraging election under deep hunger, and (when Hunting
//! wins) the locate→stalk→pounce→kill chain.
//!
//! Doc-drift note (2026-07-06): the original "kills it within ~30
//! ticks" claim had silently rotted twice over — (a) without a Stores
//! structure, Hunting planning died `GoalUnreachable` at tick 1 and
//! the failure cooldown buried it (fixed below), and (b) with planning
//! fixed, Foraging still outbids Hunting for a mouse at every hunger
//! level tried (0.45/0.25/0.15) — grass forage is cheap and the mouse
//! yield doesn't beat it. The asserted end-to-end kill coverage lives
//! in `fish_shoreline_pounce` (ticket 467), where the higher fish
//! yield puts Hunting on top. This scenario remains the
//! election-landscape probe; use `--l2` style inspection, not a kill
//! assertion.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "hunt_acquisition_to_kill",
    default_focal: "Talon",
    default_ticks: 30,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Stores building — the Hunting plan's terminal `DepositPrey` step
    // requires `ZoneIs(Stores)`; without a Stores structure the zone
    // never resolves, planning dies `GoalUnreachable` at tick 1, and
    // the disposition-failure cooldown buries Hunt for the rest of the
    // run (found while building `fish_shoreline_pounce` — this
    // scenario's documented kill had silently drifted to never-hunts).
    {
        use crate::components::building::{StoredItems, Structure, StructureType};
        world.spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
            Position::new(16, 20),
        ));
    }

    // LightForest tile near the cat so `update_capability_markers`
    // keeps `CanHunt` asserted across replans (all-Grass worlds strip
    // the marker — same pattern as `hunt_deposit_chain`).
    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        if map.in_bounds(19, 20) {
            map.get_mut(19, 20).terrain = Terrain::LightForest;
        }
    }

    // Hungry, skilled, bold — should commit to Hunting.
    let _talon = spawn_cat(
        world,
        CatPreset::adult("Talon", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.85;
                p.diligence = 0.7;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                // Deep hunger: at 0.45 (and still at 0.25) Foraging
                // outbids Hunting every election (forage raw 0.875 vs
                // hunt 0.849, observed 2026-07-06) and the mouse
                // survives the whole run. 0.15 matches the calibration
                // `fish_shoreline_pounce` demonstrated wins for Hunt.
                // Eating stays unwinnable — the Stores holds no food.
                n.hunger = 0.15;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Mouse 4 tiles away — well within sense range, short pounce path.
    let _mouse = spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
}
