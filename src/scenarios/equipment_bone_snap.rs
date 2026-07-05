//! Ticket 477 — bone-weapon snap scenario. A hunter wielding a fragile
//! `BoneStiletto` works a cluster of mice. On a missed strike the bone
//! weapon may snap (`DurabilityTier::Fragile` gate), removing it from
//! inventory and firing `Feature::BoneWeaponSnapped`.
//!
//! Snapping is a low-probability per-miss event in production
//! (`bone_weapon_snap_chance_on_miss` defaults to 0.04 — far too rare to
//! reproduce in a seed-42 soak, which is why the canary is exempt). The
//! scenario raises the chance to near-certain so a single miss
//! deterministically exercises the snap branch + the durability canary.

use bevy_ecs::world::World;

use crate::components::items::{ItemKind, ItemModifiers};
use crate::components::magic::ItemSlot;
use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "equipment_bone_snap",
    default_focal: "Splinter",
    default_ticks: 200,
    setup,
    expected_features: &["BoneWeaponSnapped"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Near-certain snap on the first missed strike so the branch is
    // deterministically exercised (production default is 0.04).
    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        constants.combat.bone_weapon_snap_chance_on_miss = 0.95;
        // 493 follow-up: a missed strike must actually OCCUR for the
        // snap roll to run. Under A*-first chase stepping the seed-42
        // trajectory landed 5/5 clean catches and the snap branch was
        // never entered — the scenario rode hunt-outcome luck. Zero
        // the pounce skill terms so strikes reliably miss (same
        // load-the-dice philosophy as the 0.95 snap chance above; the
        // test asserts the snap + trace row, not catch success).
        constants.disposition.pounce_skill_base = 0.02;
        constants.disposition.pounce_skill_scale = 0.0;
    }

    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        if map.in_bounds(21, 20) {
            map.get_mut(21, 20).terrain = Terrain::LightForest;
        }
    }

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
        CatPreset::adult("Splinter", Position::new(20, 20))
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

    // Low-quality fragile bone stiletto — snaps on a missed strike.
    // 017 — equip into the worn slot (equipment_modifiers_for reads worn).
    if let Some(mut wearables) = world
        .entity_mut(cat)
        .get_mut::<crate::components::equipment::WearableSlots>()
    {
        let _ = wearables.equip(ItemSlot::with_quality(
            ItemKind::BoneStiletto,
            0.4,
            ItemModifiers::default(),
        ));
    }

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
    fn bone_weapon_snaps_and_leaves_inventory() {
        let report = crate::scenarios::runner::run(&SCENARIO, None, None, 42);
        // The durability canary must fire.
        assert!(
            report
                .feature_counts
                .get("BoneWeaponSnapped")
                .copied()
                .unwrap_or(0)
                >= 1,
            "expected BoneWeaponSnapped to fire; features: {:?}",
            report.feature_counts,
        );
        // The snap must surface in the focal resolver trace.
        let saw_snap = report.ticks.iter().any(|t| {
            t.resolver_modifiers.iter().any(|(resolver, modifier, _)| {
                resolver == "resolve_engage_prey" && modifier == "weapon.bone_snap"
            })
        });
        assert!(saw_snap, "expected a weapon.bone_snap resolver-trace row");
    }
}
