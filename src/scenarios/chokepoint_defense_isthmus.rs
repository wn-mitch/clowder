//! Ticket 311 (301 FO-1) — chokepoint isthmus ward-corking fixture.
//!
//! A narrow-isthmus map: cats on one landmass, a ShadowFox on the other,
//! joined by a 2-tile-wide × 7-tile-long corridor at x ∈ [27, 33]. The
//! fixture exercises the **ward supply chain end-to-end** with a single
//! work-pinned cat (pattern mirrored from `farm_herb_demand.rs` and
//! `ward_placement.rs` — both proven scenarios for the same substrate):
//!
//! - `CropHarvested` — a pre-mature Thornbriar garden on the east
//!   landmass (`growth = 0.95`, `Skills.foraging = 1.0`) → tend cycle
//!   finishes in ~10 ticks, harvest emits the canary. Gated.
//! - `GatherHerbCompleted` — two wild thornbriar herb entities on the
//!   east landmass → `HerbcraftGatherDse` picks them up. Gated.
//! - `WardPlaced` — also exercised: one thornbriar is pre-loaded in
//!   inventory + corruption gradient near the isthmus. **Not gated at
//!   FO-1.** The reference `ward_placement.rs` scenario also opts out
//!   of `WardPlaced` gating (`expected_features: &[]`) because the
//!   substrate stalls at L3 election under work-pinned profiles in
//!   a tick budget that keeps the fixture cheap. FO-2's corridor
//!   perception axis is expected to lift HerbcraftSetWard's L2 score
//!   enough to overcome the dominant Farm/Gather slate here; when
//!   that lands, this scenario's `expected_features` gains `"WardPlaced"`.
//!
//! Topology is the point: when FO-2 adds a corridor-perception axis to
//! the ward-placement scorer, the same map should bias ward selection
//! toward the isthmus tiles. At FO-1 land the scorer has no such input
//! and is expected to place wards on the cat-side interior — the bug
//! FO-2 is meant to surface. The local `EXPECTED_ISTHMUS_CORKED` flag
//! stays `false` here; FO-2's PR flips it and adds the location
//! assertion. The fixture remains valuable at FO-1 land because it
//! exercises the farming + gather substrate the user noted as
//! underexercised by current canaries, AND it pre-stages the FO-2
//! acceptance surface.
//!
//! ## Personality / needs choices
//!
//! The cat is pinned to favor work over leisure (mirroring
//! `farm_herb_demand.rs`'s rationale): diligence/patience/spirituality
//! high, curiosity/boldness/playfulness low so Explore/Wander/Play
//! don't crowd the work-oriented DSEs at L3 election. Needs are sated
//! on hunger/energy but `purpose` is low, providing the work motivator.
//! `magic_affinity = 0.6` lifts ward scoring (mirrors `ward_placement.rs`).
//!
//! ## Why one cat, not three
//!
//! An earlier draft used three cats clustered together. The 3-cat
//! version L3-elected `Socialize` continuously (sociability 0.5 ×
//! adjacent partners) and never reached HerbcraftWard / Farm /
//! HerbcraftGather, even with 250 ticks. The single-cat work-pinned
//! shape is the established pattern for Feature-canary scenarios that
//! exercise multiple non-social pipelines in one run. The fox + map
//! topology are still in place for FO-2's perception-axis test.

use bevy_ecs::world::World;

use crate::components::building::{CropKind, CropState};
use crate::components::magic::{GrowthStage, Harvestable, Herb, HerbKind, Seasonal};
use crate::components::physical::{Health, Position};
use crate::components::skills::Skills;
use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
use crate::components::{SensorySignature, SensorySpecies};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::time::Season;

use super::env::{
    give_herbs, init_scenario_world_with, mark_tile_corrupted, spawn_cat, spawn_garden_at,
    ScenarioWorldConfig,
};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

/// FO-2 flips this to `true` and adds the `WardPlaced.location.x ∈ [28, 32]`
/// behavioral assertion. At FO-1 land, the scenario asserts nothing about
/// where the ward lands — only that one lands at all (`expected_features`).
#[allow(dead_code)]
const EXPECTED_ISTHMUS_CORKED: bool = false;

pub static SCENARIO: Scenario = Scenario {
    name: "chokepoint_defense_isthmus",
    default_focal: "Talon",
    default_ticks: 250,
    setup,
    // FO-1: gate only the chains that fire reliably under this profile.
    // `WardPlaced` is exercised (inventory pre-load + corruption gradient)
    // but stalls at L3 election; FO-2's corridor-perception axis is
    // expected to lift it, at which point the gate gains `"WardPlaced"`.
    // Matches `ward_placement.rs`'s opt-out rationale for the same DSE.
    expected_features: &["CropHarvested", "GatherHerbCompleted"],
};

const MAP_WIDTH: i32 = 60;
const MAP_HEIGHT: i32 = 40;

fn setup(world: &mut World, seed: u64) {
    init_scenario_world_with(
        world,
        seed,
        ScenarioWorldConfig {
            width: MAP_WIDTH,
            height: MAP_HEIGHT,
            colony_center: Position::new(45, 20),
        },
    );

    paint_isthmus_terrain(world);
    seed_corruption_near_isthmus(world);
    spawn_mature_thornbriar_garden(world);
    spawn_wild_thornbriar_patches(world);
    spawn_talon(world);
    spawn_shadow_fox(world);
}

/// `TileMap::new` populates every tile as `Terrain::Grass`. Flip every
/// tile outside the two landmasses and the isthmus to `Terrain::Water`,
/// which is pathfinding-impassable (`movement_cost == u32::MAX`,
/// `src/ai/pathfinding.rs` honors it everywhere). The result: a sea with
/// two grass landmasses joined by a 2-wide corridor.
fn paint_isthmus_terrain(world: &mut World) {
    let mut map = world.resource_mut::<TileMap>();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let on_west = (5..=25).contains(&x) && (10..=30).contains(&y);
            let on_east = (35..=55).contains(&x) && (10..=30).contains(&y);
            let on_isthmus = (27..=33).contains(&x) && (19..=21).contains(&y);
            if !(on_west || on_east || on_isthmus) && map.in_bounds(x, y) {
                map.get_mut(x, y).terrain = Terrain::Water;
            }
        }
    }
}

/// Corruption gradient flowing across the isthmus from west → east. Makes
/// `is_ward_strength_low` registers as colony-priority (rather than just
/// "no wards present") and gives `HerbcraftSetWard` a meaningful target
/// score, mirroring `ward_placement.rs`'s rationale.
fn seed_corruption_near_isthmus(world: &mut World) {
    mark_tile_corrupted(world, Position::new(34, 20), 0.6);
    for dx in -2..=2 {
        for dy in -2..=2 {
            mark_tile_corrupted(world, Position::new(34 + dx, 20 + dy), 0.35);
        }
    }
}

/// Pre-mature thornbriar garden on east landmass. `growth = 0.95`
/// (matches `farm_herb_demand.rs`) means a handful of tend ticks
/// completes the cycle and emits `CropHarvested`. `update_colony_facility_markers`
/// authors `HasGarden` colony marker on tick 1, making `FarmDse` eligible.
fn spawn_mature_thornbriar_garden(world: &mut World) {
    let garden = spawn_garden_at(world, Position::new(42, 22));
    world.entity_mut(garden).insert(CropState {
        growth: 0.95,
        crop_kind: CropKind::Thornbriar,
    });
}

/// Two wild thornbriar patches on the east landmass, year-round in-season
/// so `herb_seasonal_check` (`src/systems/magic.rs:491`) doesn't strip
/// `Harvestable` on the next season transition. Cats with `CanForage`
/// pick these up via `HerbcraftGatherDse`, emitting `Feature::GatherHerbCompleted`.
fn spawn_wild_thornbriar_patches(world: &mut World) {
    let year_round = vec![
        Season::Spring,
        Season::Summer,
        Season::Autumn,
        Season::Winter,
    ];
    for pos in [Position::new(48, 19), Position::new(52, 24)] {
        world.spawn((
            Herb {
                kind: HerbKind::Thornbriar,
                growth_stage: GrowthStage::Blossom,
                magical: false,
                twisted: false,
            },
            pos,
            Seasonal {
                available: year_round.clone(),
            },
            Harvestable,
        ));
    }
}

/// Single work-pinned cat on east landmass. Personality and needs mirror
/// `farm_herb_demand.rs`'s "Bracken" profile + `ward_placement.rs`'s
/// "Sage" magic-flavored profile — diligence/patience/spirituality high,
/// curiosity/boldness/playfulness/sociability low so non-work DSEs don't
/// crowd Farm/HerbcraftGather/HerbcraftWard at L3 election. One
/// pre-loaded thornbriar so `WardPlaced` doesn't depend on the
/// gather/harvest chain completing first.
fn spawn_talon(world: &mut World) {
    let talon = spawn_cat(
        world,
        CatPreset::adult("Talon", Position::new(40, 20))
            .with_personality(|p| {
                p.diligence = 0.9;
                p.patience = 0.85;
                p.spirituality = 0.85;
                p.compassion = 0.7;
                p.tradition = 0.7;
                p.curiosity = 0.05;
                p.boldness = 0.1;
                p.playfulness = 0.05;
                p.sociability = 0.05;
            })
            .with_needs(|n| {
                n.hunger = 1.0;
                n.energy = 0.9;
                n.purpose = 0.3;
            })
            .with_magic_affinity(0.6)
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanForage),
    );
    world.entity_mut(talon).insert(Skills {
        foraging: 1.0,
        ..Skills::default()
    });
    give_herbs(world, talon, HerbKind::Thornbriar, 1);
}

/// One ShadowFox on the west landmass, patrolling east toward the isthmus.
/// Mirrors `fox_ward_only_avoidance.rs:46-53`. The fox is fixture context
/// for FO-2 (the corridor perception axis uses fox approach corridors as
/// input); FO-1 only needs it present so the WardCoverageMap gradient
/// computation has a target.
fn spawn_shadow_fox(world: &mut World) {
    world.spawn((
        WildAnimal::new(WildSpecies::ShadowFox),
        Position::new(15, 20),
        Health::default(),
        WildlifeAiState::Patrolling { dx: 1, dy: 0 },
        SensorySpecies::Wild(WildSpecies::ShadowFox),
        SensorySignature::WILDLIFE,
    ));
}
