//! Farming cycle scenario — adult cat with diligent personality, food
//! stockpile low, and a Garden structure already built. Tests the
//! `Farm` DSE eligibility (`HasGarden` colony marker) and whether a
//! diligent cat with a depleted food stockpile actually picks Farming
//! over alternatives.
//!
//! Note: a complete plant→tend→harvest cycle spans ~120 ticks of sim
//! time (vs. soak ticks for emergent garden growth). Default ticks are
//! capped at 60 — long enough to observe a Farming pick + initial
//! commitment, but not the full crop maturation cycle. Use `--ticks 200`
//! for the full loop if needed.

use bevy_ecs::world::World;

use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat, spawn_garden_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "farming_cycle",
    default_focal: "Furrow",
    default_ticks: 60,
    setup,
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let _garden = spawn_garden_at(world, Position::new(22, 20));

    // Drain the food stockpile so the FoodScarcity scoring axis fires.
    {
        let mut food = world.resource_mut::<crate::resources::FoodStores>();
        *food = crate::resources::FoodStores::default();
    }

    let _furrow = spawn_cat(
        world,
        CatPreset::adult("Furrow", Position::new(20, 20))
            .with_personality(|p| {
                p.diligence = 0.95;
                p.patience = 0.85;
                p.tradition = 0.7;
            })
            .with_needs(|n| {
                // Healthy enough to commit to a multi-step plan, not
                // hungry enough that Eating dominates.
                n.hunger = 0.7;
                n.energy = 0.85;
                n.purpose = 0.2;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanForage),
    );
}
