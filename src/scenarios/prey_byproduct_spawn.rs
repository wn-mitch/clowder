//! Prey-byproduct spawn scenario — ticket 375.
//!
//! Verifies that `resolve_engage_prey` emits the per-species byproduct
//! list from `SimConstants::prey_byproducts` alongside the meat on a
//! successful kill. Spawns one cat with a large inventory and one prey
//! of a single species at an adjacent tile; runs ~200 ticks; asserts
//! the post-run item-kind histogram contains the expected byproducts
//! for that species.
//!
//! Why this scenario exists: the substrate change in 375 is producer-
//! only (no L2 / L3 wiring), so a passing soak verdict alone doesn't
//! prove the *per-species* table mapped correctly. A typo in the
//! `PreyByproductConstants::default()` lists (e.g. Rabbit→[Hide, Bone,
//! Sinew] but the code reads Mouse→[Hide]) would slip through the
//! `ByproductSpawned` canary because the canary only counts firings,
//! not contents. Four species variants below pin each row of the table
//! to a deterministic kill at scenario time. Fish is excluded — Fish
//! has a water-habitat requirement that the all-Grass test world
//! doesn't satisfy cleanly; the Fish row is covered by the seed-42
//! soak and by the `prey_byproducts_table_default_matches_spec` unit
//! test on `PreyByproductConstants::default()` directly.
//!
//! The focal cat is given the `CanHunt` marker and a `LightForest`
//! tile nearby so the marker-author keeps it asserted across replans
//! (same pattern as `hunt_deposit_chain`). Hunger is 0.55, just above
//! `production_self_eat_threshold`, so the catch goes into inventory
//! rather than being consumed on the spot — preserves the multi-item
//! spawn in the inventory histogram.

use bevy_ecs::world::World;

use crate::components::physical::Position;
use crate::components::prey::PreyKind;

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO_MOUSE: Scenario = Scenario {
    name: "prey_byproduct_spawn_mouse",
    default_focal: "Stoat",
    default_ticks: 200,
    setup: setup_mouse,
    expected_features: &["ByproductSpawned"],
};

pub static SCENARIO_RAT: Scenario = Scenario {
    name: "prey_byproduct_spawn_rat",
    default_focal: "Stoat",
    default_ticks: 200,
    setup: setup_rat,
    expected_features: &["ByproductSpawned"],
};

pub static SCENARIO_RABBIT: Scenario = Scenario {
    name: "prey_byproduct_spawn_rabbit",
    default_focal: "Stoat",
    default_ticks: 200,
    setup: setup_rabbit,
    expected_features: &["ByproductSpawned"],
};

pub static SCENARIO_BIRD: Scenario = Scenario {
    name: "prey_byproduct_spawn_bird",
    default_focal: "Stoat",
    // 368: bumped 200 → 300. Seed-42 perturbation from the Phase 2
    // substrate (Bristle in mammal byproducts + forage-ingredient
    // drops on Grass/forest tiles) shifts bird-hunt timing past the
    // original 200-tick window. Mouse/Rat/Rabbit still land in time
    // (more guaranteed hits per pounce); Bird is more brittle.
    default_ticks: 300,
    setup: setup_bird,
    expected_features: &["ByproductSpawned"],
};

fn setup_common(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Forest tile keeps `CanHunt` asserted across replans (mirrors
    // hunt_deposit_chain).
    {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = world.resource_mut::<TileMap>();
        if map.in_bounds(21, 20) {
            map.get_mut(21, 20).terrain = Terrain::LightForest;
        }
    }

    // Stores building west of the cat — Hunt's eligibility filter wants
    // a deposit target nearby. Without it, the cat picks Forage / Rest /
    // Patrol instead and the byproduct loop never fires. Mirrors
    // hunt_deposit_chain.
    use crate::components::building::{StoredItems, Structure, StructureType};
    world.spawn((
        Structure::new(StructureType::Stores),
        StoredItems::default(),
        Position::new(18, 20),
    ));

    let _stoat = spawn_cat(
        world,
        CatPreset::adult("Stoat", Position::new(20, 20))
            .with_personality(|p| {
                p.boldness = 0.85;
                p.diligence = 0.7;
                p.patience = 0.7;
            })
            .with_needs(|n| {
                // 0.55 — just above `production_self_eat_threshold` so
                // the catch lands in inventory rather than getting
                // eaten in place; the histogram needs the meat + byproducts
                // to be discoverable post-run.
                n.hunger = 0.55;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );
}

fn setup_mouse(world: &mut World, seed: u64) {
    setup_common(world, seed);
    // Five mice — multiple kills probable within 200 ticks even if a
    // few pounces miss. Only one species per setup so the histogram
    // attributes byproducts unambiguously to that prey kind's table row.
    for offset in [(24, 20), (25, 21), (25, 19), (26, 20), (24, 22)] {
        spawn_prey_at(world, Position::new(offset.0, offset.1), PreyKind::Mouse);
    }
}

fn setup_rat(world: &mut World, seed: u64) {
    setup_common(world, seed);
    for offset in [(24, 20), (25, 21), (25, 19), (26, 20), (24, 22)] {
        spawn_prey_at(world, Position::new(offset.0, offset.1), PreyKind::Rat);
    }
}

fn setup_rabbit(world: &mut World, seed: u64) {
    setup_common(world, seed);
    for offset in [(24, 20), (25, 21), (25, 19), (26, 20), (24, 22)] {
        spawn_prey_at(world, Position::new(offset.0, offset.1), PreyKind::Rabbit);
    }
}

fn setup_bird(world: &mut World, seed: u64) {
    setup_common(world, seed);
    for offset in [(24, 20), (25, 21), (25, 19), (26, 20), (24, 22)] {
        spawn_prey_at(world, Position::new(offset.0, offset.1), PreyKind::Bird);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Mouse kills produce Bone + Sinew alongside the meat.
    /// Asserts each expected byproduct kind appears in the post-run
    /// item-kind histogram. Tolerates 0 meat (if all pounces miss) but
    /// not 0 byproducts when at least one kill landed.
    #[test]
    fn mouse_kills_produce_bone_and_sinew() {
        let report = run(&SCENARIO_MOUSE, None, Some(200), 42);
        // Derive kill count from the canary instead of `5 - final_prey_count`
        // because prey breed mid-run and external predators (Hawk dive,
        // ShadowFox ambush) can also reduce the count. Mouse drops 2
        // byproducts per kill, so `ByproductSpawned / 2` ≈ cat-kills.
        let bp_total = report
            .feature_counts
            .get("ByproductSpawned")
            .copied()
            .unwrap_or(0) as usize;
        // 368: Mouse drops 3 byproducts per kill (Bone + Sinew + Bristle).
        let kills = bp_total / 3;
        if kills == 0 {
            panic!(
                "expected ≥1 mouse kill within 200 ticks (ByproductSpawned canary fired {bp_total} times); \
                 inv={}, ground={}, kinds={:?}",
                report.final_focal_inventory_count,
                report.final_ground_item_count,
                report.final_item_kinds,
            );
        }
        for kind in ["bone", "sinew", "bristle"] {
            let n = report.final_item_kinds.get(kind).copied().unwrap_or(0);
            assert!(
                n >= kills,
                "{kills} mouse kill(s) should produce ≥{kills} {kind}; got {n}. \
                 histogram = {:?}",
                report.final_item_kinds
            );
        }
    }

    /// Rat kills add Whisker on top of Bone + Sinew.
    #[test]
    fn rat_kills_produce_bone_sinew_whisker() {
        let report = run(&SCENARIO_RAT, None, Some(200), 42);
        // 368: Rat drops 4 byproducts per kill (Bone + Sinew + Whisker + Bristle).
        let bp_total = report
            .feature_counts
            .get("ByproductSpawned")
            .copied()
            .unwrap_or(0) as usize;
        let kills = bp_total / 4;
        if kills == 0 {
            panic!("expected ≥1 rat kill within 200 ticks (ByproductSpawned fired {bp_total}×)");
        }
        for kind in ["bone", "sinew", "whisker", "bristle"] {
            let n = report.final_item_kinds.get(kind).copied().unwrap_or(0);
            assert!(
                n >= kills,
                "{kills} rat kill(s) should produce ≥{kills} {kind}; got {n}. \
                 histogram = {:?}",
                report.final_item_kinds
            );
        }
    }

    /// Rabbit kills produce Hide + Bone + Sinew (no Whisker).
    #[test]
    fn rabbit_kills_produce_hide_bone_sinew() {
        let report = run(&SCENARIO_RABBIT, None, Some(200), 42);
        // Derive *rabbit-specific* kill count from the meat histogram —
        // hunger=0.55 is calibrated above `production_self_eat_threshold`
        // to keep meat in inventory (see module docstring). Using the
        // global ByproductSpawned canary instead would conflate stray
        // ambient kills (rats from wildlife dens) with rabbit kills and
        // wrongly demand a hide per stray rat (ticket 464).
        let kills = report.final_item_kinds.get("rabbit").copied().unwrap_or(0);
        if kills == 0 {
            let bp_total = report
                .feature_counts
                .get("ByproductSpawned")
                .copied()
                .unwrap_or(0) as usize;
            panic!(
                "expected ≥1 rabbit kill within 200 ticks (rabbit meat histogram empty; \
                 global ByproductSpawned fired {bp_total}×; histogram = {:?})",
                report.final_item_kinds,
            );
        }
        for kind in ["hide", "bone", "sinew", "bristle"] {
            let n = report.final_item_kinds.get(kind).copied().unwrap_or(0);
            assert!(
                n >= kills,
                "{kills} rabbit kill(s) should produce ≥{kills} {kind}; got {n}. \
                 histogram = {:?}",
                report.final_item_kinds
            );
        }
        // Whisker is on the Rat row, not the Rabbit row — assert it
        // doesn't bleed in. Catches a typo where the lookup keyed off
        // the wrong PreyKind variant. Ambient wildlife dens can spawn
        // stray rats into this scenario; if the cat kills one, a single
        // legitimate whisker may appear. Bound the assertion to
        // "whisker count ≤ rat-meat count" so any whisker without a
        // corresponding rat is a real rabbit→rat lookup defect (ticket 464).
        let whisker = report.final_item_kinds.get("whisker").copied().unwrap_or(0);
        let rat_kills = report.final_item_kinds.get("rat").copied().unwrap_or(0);
        assert!(
            whisker <= rat_kills,
            "rabbit kills must not produce whisker (Rat-row item); \
             got whisker={whisker} but only {rat_kills} stray rat kill(s). \
             histogram = {:?}",
            report.final_item_kinds
        );
    }

    /// Bird kills produce Feather + Bone. Closes the long-standing
    /// `Feather` dormancy gap (existed as an ItemKind since pre-016
    /// with zero producers).
    ///
    /// 368: budget bumped from 200 → 300 ticks. Seed-42 perturbation
    /// from the Phase 2 substrate (Bristle in mammal byproducts +
    /// forage-ingredient drops) shifts the bird-hunt timing past the
    /// original 200-tick window; mouse/rat/rabbit kills still land in
    /// time but Bird is more brittle (longer pre-pounce stalk + lower
    /// catch chance per `PreyProfile`). The extended budget keeps the
    /// per-row coverage of the byproduct table intact.
    #[test]
    fn bird_kills_produce_feather_and_bone() {
        let report = run(&SCENARIO_BIRD, None, Some(300), 42);
        // Bird drops 2 byproducts per kill.
        let bp_total = report
            .feature_counts
            .get("ByproductSpawned")
            .copied()
            .unwrap_or(0) as usize;
        let kills = bp_total / 2;
        if kills == 0 {
            panic!("expected ≥1 bird kill within 200 ticks (ByproductSpawned fired {bp_total}×)");
        }
        for kind in ["feather", "bone"] {
            let n = report.final_item_kinds.get(kind).copied().unwrap_or(0);
            assert!(
                n >= kills,
                "{kills} bird kill(s) should produce ≥{kills} {kind}; got {n}. \
                 histogram = {:?}",
                report.final_item_kinds
            );
        }
    }
}
