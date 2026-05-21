//! Ticket 382 — district placement under colony-crowd pressure.
//!
//! Sets up a tight founder cluster (6 buildings packed inside the
//! radius-16 spiral disc where the pre-382 spiral search saturates)
//! and pre-loads a `Build` directive for `Stores` onto a Coordinator's
//! queue. The 382 substrate replaces `find_building_placement`'s
//! spiral with an argmax over `ColonyDistrictMap`, so the new
//! directive should find a spot on the *expansion frontier* — outside
//! the founder cluster's saturated 1-tile-gap envelope — within a
//! handful of ticks.
//!
//! # Why a scenario, not the canonical soak
//!
//! The seed-42 deep soak surfaces this bug after 50k+ ticks of
//! chronic-full latch; the structural fix is testable in seconds with
//! a synthetic 6-building cluster + a pre-loaded directive. The
//! scenario harness asserts `Feature::ConstructionSiteSpawned` fires
//! — the L1-visible witness that the new placement function returned
//! `Some` and `spawn_construction_sites` consumed it.
//!
//! # Preloaded state
//!
//! - 6 founder structures (Den, Hearth, Kitchen, Workshop, Garden,
//!   Midden) packed at Manhattan-distance ≤ 4 from `colony_center =
//!   (60, 45)`. Every position in the radius-16 Manhattan disc that
//!   satisfies `footprint_valid` gets filled or apron-blocked by
//!   these — the spiral path would return `None`.
//! - 1 Coordinator cat (`Mocha`) at colony_center, marked Adult, with
//!   a `DirectiveQueue` pre-loaded with a `Build` directive for a new
//!   `Stores` building.
//! - 7 supporting cats spread around the cluster to populate the
//!   `CatScentMap` frontier signal so the influence-map composite has
//!   a positive lift outside the founder envelope.
//!
//! # Tick budget
//!
//! `update_colony_district_map` populates the L1 map on tick 0 (runs
//! in Chain 1 before `spawn_construction_sites` in Chain 2b);
//! `spawn_construction_sites` reads the directive on tick 0 or 1
//! depending on schedule order, calls `compute_building_placement`,
//! finds an argmax outside the founder cluster, spawns the
//! ConstructionSite. Budget 60 ticks leaves margin for startup
//! scheduling and the once-per-tick activation snapshot.

use bevy_ecs::world::World;

use crate::components::building::{Structure, StructureType};
use crate::components::coordination::{Coordinator, Directive, DirectiveKind, DirectiveQueue};
use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "district_placement_under_pressure",
    default_focal: "Mocha",
    default_ticks: 60,
    setup,
    expected_features: &["ConstructionSiteSpawned"],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Tight founder cluster around colony_center = (60, 45). Six
    // structures spaced at Manhattan-distance 2-4. The 1-tile-gap rule
    // means the radius-16 spiral disc is effectively saturated; the
    // pre-382 spiral returns None here.
    let cluster: &[(StructureType, Position)] = &[
        (StructureType::Den, Position::new(58, 43)),
        (StructureType::Hearth, Position::new(62, 43)),
        (StructureType::Kitchen, Position::new(58, 47)),
        (StructureType::Workshop, Position::new(62, 47)),
        (StructureType::Garden, Position::new(54, 45)),
        (StructureType::Midden, Position::new(66, 45)),
    ];
    for (kind, pos) in cluster {
        world.spawn((Structure::new(*kind), *pos));
    }

    // Coordinator with a pre-loaded Build directive for a NEW Stores
    // building. Placement should NOT find a spot inside the founder
    // cluster (every tile fails footprint_valid + crowding penalty)
    // and should pick a candidate on the expansion frontier.
    let mocha = spawn_cat(
        world,
        CatPreset::adult("Mocha", Position::new(60, 45))
            .with_personality(|p| {
                p.diligence = 0.9;
                p.boldness = 0.5;
                p.patience = 0.7;
            })
            .with_marker(MarkerKind::Adult),
    );
    world.entity_mut(mocha).insert((
        Coordinator,
        DirectiveQueue {
            directives: vec![Directive {
                kind: DirectiveKind::Build,
                priority: 0.9,
                target_entity: None,
                target_position: None,
                blueprint: Some(StructureType::Stores),
                placement_failure_count: 0,
            }],
        },
    ));

    // Seven supporting cats around the cluster to lift the
    // CatScentMap's frontier signal beyond raw wilderness. Spread
    // along the cardinal axes a few tiles outside the founder
    // envelope.
    let supports: &[(&str, Position)] = &[
        ("Bracken", Position::new(60, 38)),
        ("Sage", Position::new(60, 52)),
        ("Mallow", Position::new(50, 45)),
        ("Cedar", Position::new(70, 45)),
        ("Linden", Position::new(55, 40)),
        ("Sorrel", Position::new(65, 50)),
        ("Thistle", Position::new(53, 50)),
    ];
    for (name, pos) in supports {
        spawn_cat(
            world,
            CatPreset::adult(*name, *pos).with_marker(MarkerKind::Adult),
        );
    }
}
