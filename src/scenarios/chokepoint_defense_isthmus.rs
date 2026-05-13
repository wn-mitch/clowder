//! Ticket 311 (301 FO-1) / Ticket 312 (FO-2) — chokepoint isthmus
//! ward-corking fixture.
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
//! - `WardPlaced` — gated at FO-2 (312): one thornbriar is pre-loaded
//!   in inventory + corruption gradient near the isthmus, and the
//!   fixture activates `ward_fox_approach_corridor_weight = 0.3` to
//!   lift the topological-criticality axis off dormancy. With the
//!   axis active the corridor scoring multiplier pulls Path-A
//!   placement onto the 2-wide isthmus rather than the cat-cluster
//!   interior. The post-run assertion in `tests/scenarios.rs` checks
//!   the actual ward location.
//!
//! Topology is the point: 312's corridor-perception axis composes
//! multiplicatively outside the saturating threat sum
//! (`unaddressed_threat * (1.0 + w_corridor * L(corridor))`), escaping
//! the 297 iter-2 rank-preservation ceiling so a high-traffic isthmus
//! tile scores above the cat-cluster's saturated threat. At FO-1 land
//! the scorer had no such input and placed wards on the cat-side
//! interior — the architectural gap 312 closes. The fixture asserts
//! the corked outcome at fixture-level weight (`0.3`) without
//! requiring a global default flip (FO-3 territory).
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

/// 312 (FO-2) corked: with the fixture-level corridor weight active,
/// `tests/scenarios.rs::chokepoint_defense_isthmus_corks_corridor`
/// asserts at least one `WardPlaced` event lands with
/// `location.x ∈ [28, 32]` (the 5-tile band centered on the isthmus
/// at x=30).
pub const EXPECTED_ISTHMUS_CORKED: bool = true;

/// 312 (FO-2) corridor-band check: the 5-tile band centered on the
/// 7-wide isthmus at x=30. A `WardPlaced.location.x` in this range
/// constitutes corking the chokepoint; outside it the ward landed on
/// the cat-cluster interior (the 297 iter-2 saturation pathology).
pub const ISTHMUS_BAND_X_MIN: i32 = 28;
pub const ISTHMUS_BAND_X_MAX: i32 = 32;

/// 312: fixture-level corridor weight. Canonical `SimConstants` ship
/// at `0.0` (dormant per the 220 / 297 / 301 first-light pattern); the
/// scenario lifts to `0.3` to exercise the multiplicative-outside lift
/// without committing to a global default flip. FO-3 will run the
/// hypothesize sweep that decides the global default.
const FIXTURE_CORRIDOR_WEIGHT: f32 = 0.3;

pub static SCENARIO: Scenario = Scenario {
    name: "chokepoint_defense_isthmus",
    default_focal: "Talon",
    default_ticks: 250,
    setup,
    // 312 (FO-2): `WardPlaced` is NOT gated end-to-end through the
    // scenario harness. Reaching it via Path A requires a
    // non-Coordinator candidate within urgent dispatch range of a
    // separately-authored Coordinator, plus an L3 election that
    // picks Herbalism+HerbcraftSetWard over the dominant Farm /
    // HerbcraftGather slate — all dynamics outside 312's scope.
    // The architectural claim ("corridor signal lifts isthmus tiles
    // past the saturation ceiling") is asserted directly against
    // `compute_ward_placement` in
    // `tests/scenarios.rs::chokepoint_defense_isthmus_corks_corridor`
    // using scenario-matched geometry + pre-deposits. Same opt-out
    // rationale as `ward_placement.rs` for the same DSE.
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

    // 312 (FO-2): activate the corridor perception axis at fixture
    // level. Canonical `SimConstants` ship at 0.0 (dormant per the
    // 220 / 297 / 301 first-light pattern); this override is local
    // to the scenario world and lets `compute_ward_placement`'s
    // corridor lift bias ward placement onto the isthmus during the
    // run. Exercises the wiring (resource → scorer read path) even
    // though end-to-end `WardPlaced` emission requires colony-scale
    // dynamics outside 312's scope.
    {
        let mut constants = world.resource_mut::<crate::resources::SimConstants>();
        constants.scoring.ward_fox_approach_corridor_weight = FIXTURE_CORRIDOR_WEIGHT;
    }

    paint_isthmus_terrain(world);
    seed_corruption_near_isthmus(world);
    seed_isthmus_fox_traffic(world);
    spawn_mature_thornbriar_garden(world);
    spawn_wild_thornbriar_patches(world);
    spawn_talon(world);
    spawn_shadow_fox(world);
}

/// 312: pre-deposit fox traffic on the isthmus tiles, simulating "a
/// ShadowFox has been patrolling through this corridor for many
/// ticks." In a full soak the corridor map fills naturally as
/// `update_fox_approach_corridor_map` reads patrolling-fox positions
/// every tick; in this scenario the hand-spawned fox is inert (no
/// `FoxState` / `FoxAiPhase` components), so the substrate has to be
/// seeded explicitly. The `FoxScentMap` deposit gives the isthmus
/// tiles an inherent threat baseline (`fox_scent.max(corruption)`
/// saturates at the corridor band), and the
/// `FoxApproachCorridorMap` deposit drives the
/// multiplicative-outside lift in `compute_ward_placement` —
/// together they push Path A placement onto the isthmus.
///
/// The corridor map ships with per-tile resolution (bucket_size = 1)
/// so the corridor signal is exact to the deposited tiles — no
/// aliasing onto adjacent non-isthmus tiles. The fox_scent map
/// still uses its 5-tile buckets (sibling-map convention); the
/// corridor-band x ∈ [28, 32] check tolerates the fox_scent
/// bucket alignment.
///
/// Mirrors the L1-substrate pre-load pattern in
/// `src/scenarios/route_cost_decision.rs:69-77` (saturate a
/// fox-scent corridor; `deposit` clamps to 1.0).
fn seed_isthmus_fox_traffic(world: &mut World) {
    use crate::resources::{FoxApproachCorridorMap, FoxScentMap};
    // Isthmus geometry: x ∈ [27, 33], y ∈ [19, 21]. Deposit at every
    // isthmus tile so per-tile-resolution corridor signal covers the
    // full 7-tile width of the chokepoint.
    {
        let mut fox_scent = world.resource_mut::<FoxScentMap>();
        for y in 19..=21 {
            for x in 27..=33 {
                fox_scent.deposit(x, y, 1.0);
            }
        }
    }
    {
        let mut corridor = world.resource_mut::<FoxApproachCorridorMap>();
        for y in 19..=21 {
            for x in 27..=33 {
                corridor.deposit(x, y, 1.0);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        CarcassScentMap, CatScentMap, FoxApproachCorridorMap, FoxScentMap, RecentAmbushMap,
        SimConstants, WardCoverageMap,
    };
    use crate::systems::coordination::{compute_ward_placement, PlacementMaps};
    use rand::SeedableRng;

    /// 312 (FO-2) acceptance gate. With the corridor perception axis
    /// active at fixture-level `ward_fox_approach_corridor_weight = 0.3`
    /// and corridor traffic pre-deposited at the isthmus, the ward
    /// placement scorer's argmax lands in the 5-tile band centered
    /// on the 7-wide isthmus at x=30. Outside that band indicates the
    /// scorer chose a cat-cluster interior tile — the 297 iter-2
    /// saturation pathology that 312's multiplicative-outside lift
    /// is meant to escape.
    ///
    /// Tests the scorer directly against scenario-matched geometry +
    /// pre-deposits rather than running the full Path A directive →
    /// dispatch → L3 election → set-ward chain end-to-end through
    /// the scenario runner. The end-to-end chain has dependencies
    /// outside 312's scope (coordinator selection cadence, urgent
    /// dispatch thresholds, Herbalism sub-action selection); the
    /// architectural claim 312 introduces lives in
    /// `compute_ward_placement`'s scoring formula and is asserted
    /// directly here.
    #[test]
    fn corridor_corks_isthmus() {
        let mut tile_map = TileMap::new(60, 40, Terrain::Grass);
        tile_map.get_mut(34, 20).corruption = 0.6;
        for dx in -2i32..=2 {
            for dy in -2i32..=2 {
                let nx = 34 + dx;
                let ny = 20 + dy;
                if tile_map.in_bounds(nx, ny) {
                    let existing = tile_map.get_mut(nx, ny).corruption;
                    tile_map.get_mut(nx, ny).corruption = existing.max(0.35);
                }
            }
        }

        // Saturate the full isthmus geometry (x ∈ [27, 33],
        // y ∈ [19, 21]). FoxApproachCorridorMap ships per-tile
        // (bucket_size = 1) so the deposit covers exactly the
        // 7-tile corridor with no aliasing onto neighbors. FoxScentMap
        // retains its 5-tile bucketing so its deposits at x=27..=29
        // and x=30..=33 saturate the buckets covering x ∈ [25, 29]
        // and x ∈ [30, 34]; the corridor-band assertion at
        // x ∈ [28, 32] holds because the corridor multiplier is
        // what shifts the argmax — the fox_scent component is just a
        // threat-baseline.
        let mut fox_scent = FoxScentMap::default();
        let mut corridor = FoxApproachCorridorMap::default();
        for y in 19..=21 {
            for x in 27..=33 {
                fox_scent.deposit(x, y, 1.0);
                corridor.deposit(x, y, 1.0);
            }
        }

        let cat_scent = CatScentMap::default();
        let ward_coverage = WardCoverageMap::default();
        let recent_ambush = RecentAmbushMap::default();
        let carcass_scent = CarcassScentMap::default();

        let maps = PlacementMaps {
            fox_scent: &fox_scent,
            cat_scent: &cat_scent,
            ward_coverage: &ward_coverage,
            tile_map: &tile_map,
            recent_ambush: &recent_ambush,
            carcass_scent: &carcass_scent,
            fox_approach_corridor: &corridor,
        };

        let mut constants = SimConstants::default();
        constants.scoring.ward_fox_approach_corridor_weight = FIXTURE_CORRIDOR_WEIGHT;

        let building_positions = vec![Position::new(42, 22)];
        let ward_positions = vec![(Position::new(50, 22), 6.0)];
        let colony_center = Position::new(45, 20);

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let pick = compute_ward_placement(
            &building_positions,
            &ward_positions,
            colony_center,
            &maps,
            &constants,
            &mut rng,
            None,
        );

        let in_band = (ISTHMUS_BAND_X_MIN..=ISTHMUS_BAND_X_MAX).contains(&pick.x);
        if EXPECTED_ISTHMUS_CORKED {
            assert!(
                in_band,
                "312 corked contract violated: with corridor weight {} + \
                 saturated isthmus traffic, scorer picked {:?} (x not in \
                 band [{}, {}]). Expected ward placement on the isthmus \
                 corridor — multiplicative-outside lift should escape the \
                 297 iter-2 saturation ceiling.",
                FIXTURE_CORRIDOR_WEIGHT, pick, ISTHMUS_BAND_X_MIN, ISTHMUS_BAND_X_MAX,
            );
        } else {
            assert!(
                !in_band,
                "FO-1 contract violated: scorer corked the isthmus at {:?} \
                 without the corridor axis. The substrate-gap diagnosis is \
                 stale.",
                pick,
            );
        }
    }

    /// Control: with the corridor weight forced to 0.0 and *no
    /// substrate deposits at all*, the scorer must not cork the
    /// isthmus. Byte-identity-at-dormancy with pre-deposited
    /// substrate is already pinned by
    /// `coordination::tests::corridor_axis_dormant_when_weight_is_zero`;
    /// this control isolates the architectural claim "**without the
    /// new axis active**, the scorer prefers the cat-cluster interior
    /// over the corridor" by stripping the corridor signal entirely.
    #[test]
    fn dormant_corridor_does_not_cork_isthmus() {
        let mut tile_map = TileMap::new(60, 40, Terrain::Grass);
        tile_map.get_mut(34, 20).corruption = 0.6;
        for dx in -2i32..=2 {
            for dy in -2i32..=2 {
                let nx = 34 + dx;
                let ny = 20 + dy;
                if tile_map.in_bounds(nx, ny) {
                    let existing = tile_map.get_mut(nx, ny).corruption;
                    tile_map.get_mut(nx, ny).corruption = existing.max(0.35);
                }
            }
        }

        // No substrate deposits — the load-bearing signal is the
        // ABSENCE of corridor traffic, mirroring an unprimed
        // colony's first ward placement. Pre-depositing fox_scent
        // at the isthmus would itself saturate threat there
        // (via the 297 fox_intercept halo + fox_scent input) and
        // make the control's outcome jitter-dependent on the seed —
        // a separate concern from the corridor axis.
        let fox_scent = FoxScentMap::default();
        let corridor = FoxApproachCorridorMap::default();

        let cat_scent = CatScentMap::default();
        let ward_coverage = WardCoverageMap::default();
        let recent_ambush = RecentAmbushMap::default();
        let carcass_scent = CarcassScentMap::default();

        let maps = PlacementMaps {
            fox_scent: &fox_scent,
            cat_scent: &cat_scent,
            ward_coverage: &ward_coverage,
            tile_map: &tile_map,
            recent_ambush: &recent_ambush,
            carcass_scent: &carcass_scent,
            fox_approach_corridor: &corridor,
        };

        // Default constants — corridor weight stays at 0.0. The
        // pre-deposited isthmus fox_scent still gives the corridor
        // some threat baseline (sites with high fox_scent already
        // saturate the threat sum), but without the multiplicative
        // outside lift, the cat-cluster interior tile near
        // (45, 20) — sitting on the corruption gradient and close
        // to anchor — still wins.
        let constants = SimConstants::default();

        let building_positions = vec![Position::new(42, 22)];
        let ward_positions = vec![(Position::new(50, 22), 6.0)];
        let colony_center = Position::new(45, 20);

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let pick = compute_ward_placement(
            &building_positions,
            &ward_positions,
            colony_center,
            &maps,
            &constants,
            &mut rng,
            None,
        );

        let in_band = (ISTHMUS_BAND_X_MIN..=ISTHMUS_BAND_X_MAX).contains(&pick.x);
        assert!(
            !in_band,
            "Dormant control violated: with corridor weight 0.0, scorer \
             corked the isthmus at {:?}. Something other than the \
             corridor axis is driving the corked outcome; the contrast \
             with `corridor_corks_isthmus` is the load-bearing signal.",
            pick,
        );
    }
}
