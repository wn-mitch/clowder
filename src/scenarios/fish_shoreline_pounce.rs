//! Fish shoreline-pounce scenario — ticket 467.
//!
//! A hungry hunter cat is placed west of a lake holding two fish: one
//! a single tile offshore (a passable bank tile exists inside the
//! cat's pounce band — catchable via the 467 shoreline vantage) and
//! one mid-lake (every tile within pounce range is Water — the 467
//! reachability gate must keep it out of the hunt-target candidate
//! set entirely).
//!
//! Expected on a healthy build: the cat elects Hunt on the shore fish,
//! navigates to the bank vantage (A* engages because the vantage is a
//! passable target), pounces across the water gap, and kills it; the
//! mid-lake fish survives untouched. Pre-467 this scenario locks up:
//! `find_path` refuses the impassable Water target, the greedy
//! fallback strands the cat at the shoreline, and the attempt burns
//! `chase_stuck_ticks` frozen before stuck-out ("stuck during
//! approach" — 93% of all hunt attempts in the 140 step-12 gate
//! soaks). Also the first scenario coverage for the Fish species row
//! (`prey_byproduct_spawn` excludes Fish for want of a water tile in
//! its all-Grass world).

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "fish_shoreline_pounce",
    default_focal: "Wren",
    default_ticks: 120,
    setup,
    expected_features: &["HuntAttempted"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Lake: x 26..=34, y 14..=26. Western bank at x=25.
    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        for y in 14..=26 {
            for x in 26..=34 {
                if map.in_bounds(x, y) {
                    map.get_mut(x, y).terrain = Terrain::Water;
                }
            }
        }
        // LightForest tile near the cat so `update_capability_markers`
        // keeps `CanHunt` asserted across replans (same pattern as
        // `hunt_deposit_chain` — all-Grass worlds strip the marker).
        if map.in_bounds(19, 20) {
            map.get_mut(19, 20).terrain = Terrain::LightForest;
        }
    }

    // Stores building west of the cat — the Hunting plan's terminal
    // `DepositPrey` step requires `ZoneIs(Stores)`; without a Stores
    // structure the zone never resolves and planning dies
    // `GoalUnreachable` before any step runs (this is also why
    // `hunt_acquisition`'s cat never hunts — observed while building
    // this scenario). The catch itself is eaten on the spot (hunger
    // below `production_self_eat_threshold`), so the deposit leg is
    // plan-shape scaffolding, not part of the assertion.
    {
        use crate::components::building::{StoredItems, Structure, StructureType};
        world.spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
            Position::new(16, 20),
        ));
    }

    // Hungry, bold hunter. Patience 0.5 → pounce_range_default (2).
    let _wren = spawn_cat(
        world,
        CatPreset::adult("Wren", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.85;
                p.diligence = 0.7;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                // Deep tier-1 hunger. The scenario world has no Stores
                // and no colony food economy, so Explore outbids a
                // 0.45-hunger Hunt every election (observed: even
                // `hunt_acquisition`'s cat never hunts at 0.45). At
                // 0.15 the Maslow physiological gate suppresses the
                // curiosity tier and the food-seeking family wins;
                // with zero stored food, Hunt is the only winnable
                // member. Below `production_self_eat_threshold` the
                // catch is eaten on the spot — fine, the assertion
                // counts corpses, not inventory.
                n.hunger = 0.15;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Shore fish: 1 tile into the lake at (26, 20). The bank column
    // x=25 sits at Chebyshev 1 — inside every personality's pounce
    // band, so `hunt_vantage` resolves and the fish is electable.
    let _shore_fish = spawn_prey_at(world, Position::new(26, 20), PreyKind::Fish);

    // Mid-lake fish at (30, 20): nearest land is x=25 (Chebyshev 5),
    // beyond even the patient pounce band (3). `hunt_vantage` returns
    // None → the 467 candidate gate must keep it un-elected and the
    // cat must never freeze at the shore trying.
    let _deep_fish = spawn_prey_at(world, Position::new(30, 20), PreyKind::Fish);
}
