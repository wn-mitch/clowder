//! Ticket 477 — equipment cloak-mask scenario. The focal cat wears a
//! `WovenReedCloak` (Fiber, visual-mask) and sits within sight range of a
//! mouse. Each tick `prey_ai` runs `try_detect_cat` against the cloaked
//! cat; the cloak read multiplies the sight component of detection and
//! surfaces as a named `L4Resolver` modifier (`cloak.visual_mask`) in the
//! focal trace.
//!
//! Unlike the hunt-strike read (which needs a pounce), the detection read
//! fires every tick a prey is in sight band, so this is the reliable
//! integration proof that the resolver-trace pipeline carries an
//! equipment modifier end-to-end through a scenario run.

use bevy_ecs::world::World;

use crate::components::items::{ItemKind, ItemModifiers};
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "equipment_cloak_mask",
    default_focal: "Shroud",
    default_ticks: 20,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let cat = spawn_cat(
        world,
        CatPreset::adult("Shroud", Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );

    // Wear a full-quality woven reed cloak (Fiber, visual-mask class).
    if let Some(mut inv) = world.entity_mut(cat).get_mut::<Inventory>() {
        inv.slots.push(ItemSlot::with_quality(
            ItemKind::WovenReedCloak,
            1.0,
            ItemModifiers::default(),
        ));
    }

    // Mouse two tiles east — inside the rabbit/mouse sight band so
    // `try_detect_cat` evaluates the cloaked cat every tick.
    spawn_prey_at(world, Position::new(22, 20), PreyKind::Mouse);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloak_visual_mask_surfaces_in_resolver_trace() {
        let report = crate::scenarios::runner::run(&SCENARIO, None, None, 42);
        let saw_mask = report.ticks.iter().any(|t| {
            t.resolver_modifiers
                .iter()
                .any(|(resolver, modifier, delta)| {
                    resolver == "try_detect_cat"
                        && modifier == "cloak.visual_mask"
                        // delta = post - pre = (1 - mask) - 1 = -mask < 0
                        && *delta < 0.0
                })
        });
        assert!(
            saw_mask,
            "expected a cloak.visual_mask resolver-trace row; resolver \
             modifiers seen: {:?}",
            report
                .ticks
                .iter()
                .flat_map(|t| t.resolver_modifiers.iter())
                .collect::<Vec<_>>(),
        );
    }
}
