//! Ticket 477 — equipment weapon-strike bonus scenario. A hungry skilled
//! hunter wielding a `BoneTipSpear` (Pierce class) closes on a mouse and
//! pounces. The hunt-strike read site in `resolve_engage_prey` should
//! lift the catch threshold by the class-keyed, quality-scaled weapon
//! bonus and surface it in the focal trace as a named `L4Resolver`
//! modifier (`weapon.pierce.bonus`) — never a hidden post-hoc bonus.
//!
//! This is the integration proof for the Piece-2 read site: the pure
//! `WeaponView::strike_bonus` math is unit-tested in
//! `equipment_effects`, but the wiring (resolver reads the aggregate,
//! applies it, emits the trace row) is verified here.

use bevy_ecs::world::World;

use crate::components::items::{ItemKind, ItemModifiers};
use crate::components::magic::ItemSlot;
use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "equipment_weapon_strike",
    default_focal: "Spear",
    default_ticks: 200,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Forest tile by the cat so `update_capability_markers` keeps
    // `CanHunt` asserted across replans (default scenario terrain is all
    // Grass, which strips CanHunt and sends the cat Idle — see
    // hunt_deposit_chain).
    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        if map.in_bounds(21, 20) {
            map.get_mut(21, 20).terrain = Terrain::LightForest;
        }
    }

    // Stores building west of the cat — the Hunt plan's DepositPrey leg
    // needs a deposit target to form, otherwise the planner falls through
    // and the cat idles (mirrors hunt_deposit_chain).
    {
        use crate::components::building::{StoredItems, Structure, StructureType};
        world.spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
            Position::new(18, 20),
        ));
    }

    let cat = spawn_cat(
        world,
        CatPreset::adult("Spear", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.85;
                p.diligence = 0.7;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                n.hunger = 0.55;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Wield a high-quality bone-tip spear (Pierce class, Fragile tier).
    // 017 — equip into the worn slot; equipment_modifiers_for reads worn
    // gear, not the pouch.
    if let Some(mut wearables) = world
        .entity_mut(cat)
        .get_mut::<crate::components::equipment::WearableSlots>()
    {
        let _ = wearables.equip(ItemSlot::with_quality(
            ItemKind::BoneTipSpear,
            0.9,
            ItemModifiers::default(),
        ));
    }

    // Cluster of mice east of the cat (mirrors hunt_deposit_chain's
    // proven kill setup) — repeated pounce opportunities so a strike
    // (and its weapon-bonus trace row) fires within the budget.
    spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(25, 21), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(25, 19), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(26, 20), PreyKind::Mouse);
    spawn_prey_at(world, Position::new(24, 22), PreyKind::Mouse);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_strike_bonus_surfaces_in_resolver_trace() {
        let report = crate::scenarios::runner::run(&SCENARIO, None, None, 42);
        // The pierce-class weapon bonus must appear as a named resolver
        // modifier at least once across the run (fires on the pounce
        // eval). If it's absent, the read site isn't wired or the cat
        // never reached a pounce.
        let saw_bonus = report.ticks.iter().any(|t| {
            t.resolver_modifiers
                .iter()
                .any(|(resolver, modifier, delta)| {
                    resolver == "resolve_engage_prey"
                        && modifier == "weapon.pierce.bonus"
                        && *delta > 0.0
                })
        });
        assert!(
            saw_bonus,
            "expected a weapon.pierce.bonus resolver-trace row; resolver \
             modifiers seen: {:?}",
            report
                .ticks
                .iter()
                .flat_map(|t| t.resolver_modifiers.iter())
                .collect::<Vec<_>>(),
        );
    }
}
