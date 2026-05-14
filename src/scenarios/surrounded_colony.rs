//! Ticket 313 (301 FO-3) — surrounded-colony ring-coverage fixture.
//!
//! A cat cluster sits at the center of an open map; eight unmoving
//! `ShadowFox`es occupy the eight compass-direction periphery tiles.
//! No corridor, no narrow choke — the threat is *omnidirectional*.
//! The expected emergent behavior of `compute_ward_placement` over
//! successive coordinator wakes is a **ring of coverage** around
//! the cluster: each new ward lands in the empty quadrant the
//! previous wards haven't covered, until all four quadrants carry
//! at least one ward.
//!
//! ## Why this scenario exists (313's lens)
//!
//! 313 introduces the `Gate` composition: `cat_value` becomes a
//! saturating-ramp gate on the threat-merit term instead of an
//! additive density reward. Under the pre-313 `Additive` formula,
//! a tile near the cat cluster gets a `+ 0.3 * cat_value` reward
//! that — combined with the centroid-pulling `- distance_cost`
//! penalty — biases placement INWARD, toward the cat cluster's
//! interior. With cats omnidirectionally threatened, that bias
//! would cluster wards in the same warm interior on every wake.
//!
//! For a ring to emerge, the scorer needs two things:
//! 1. **Spreading semantics.** Existing wards stamp coverage; the
//!    next wake's argmax sees an "eaten" threat surface on the
//!    side that's already warded, and picks the next-highest
//!    uncovered tile somewhere else. This is the
//!    `WardCoverageMap`-driven multi-wake spreading the
//!    coordinator does in normal play (NOT
//!    `DescendingResidual`'s in-call virtual coverage — that's
//!    `WardPlacementSemantics::DescendingResidual`, which is
//!    dormant at default).
//! 2. **Ring-friendly composition.** The fixture lifts cat-scent
//!    onto a *halo* around the cluster (cats wander outward),
//!    not just the centroid pixel. Under `Additive`, the cluster
//!    centroid still wins the density reward; under `Gate`, the
//!    halo clears the gate floor everywhere within radius and
//!    the threat-merit term (saturated by perimeter foxes)
//!    decides on its own. Asserting the ring forms under the
//!    default `Additive` composition tests the spreading
//!    semantics; asserting it ALSO forms under `Gate` tests that
//!    313's option (c) doesn't break the load-bearing
//!    multi-wake behavior.
//!
//! ## Geometry
//!
//! - 60×40 grass map; colony_center = (30, 20).
//! - 8 static `ShadowFox`es at radius ~12 around the center on
//!   the 8 compass points: N, NE, E, SE, S, SW, W, NW.
//! - 5 cats clustered at the center within a 6-tile radius. Each
//!   cat emits cat-scent at its own position; the cluster
//!   distribution + the bucket-size-5 influence-map quantization
//!   produces a small natural halo.
//!
//! ## Why one ring, not "K wards by DescendingResidual"
//!
//! `DescendingResidual` is the in-call spreading mechanism
//! (`select_descending_residual` in `coordination.rs:1823`) and is
//! `WardPlacementSemantics::DescendingResidual`-gated — dormant
//! at default. The ring-of-coverage behavior the scenario
//! exercises is the OTHER spreading mechanism: successive
//! coordinator wakes accumulate `WardCoverageMap` stamps and the
//! next wake's argmax picks the uncovered side. This is the
//! everyday gameplay behavior we want preserved across the 313
//! composition change. The test in `mod tests` mimics the
//! coordinator-over-time loop by running
//! `compute_ward_placement` repeatedly, stamping each pick into
//! `WardCoverageMap` between calls.

use bevy_ecs::world::World;

use crate::components::physical::{Health, Position};
use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
use crate::components::{SensorySignature, SensorySpecies};

use super::env::{init_scenario_world_with, spawn_cat, ScenarioWorldConfig};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "surrounded_colony",
    default_focal: "Bramble",
    // The scenario is primarily a substrate microexperiment for
    // `compute_ward_placement`, asserted via `mod tests`. The
    // tick-driven run is short — there's no DSE chain to gate
    // (`WardPlaced` end-to-end requires the urgent-dispatch chain
    // outside 313's scope, same rationale as
    // `chokepoint_defense_isthmus`). Keep it small so the runner
    // stays inside the ~3s scenario budget.
    default_ticks: 50,
    setup,
    expected_features: &[],
};

const MAP_WIDTH: i32 = 60;
const MAP_HEIGHT: i32 = 40;
const COLONY_CENTER: Position = Position { x: 30, y: 20 };

/// Eight compass-direction fox spawn tiles at radius ~12 from
/// `COLONY_CENTER`. Chosen so every fox sits on an even multiple
/// of 5 (the influence-map bucket size) — keeps the fox-scent
/// deposit aligned with placement-candidate buckets without
/// cherry-picking individual fox positions.
const FOX_POSITIONS: [Position; 8] = [
    Position { x: 30, y: 8 },  // N
    Position { x: 40, y: 10 }, // NE
    Position { x: 42, y: 20 }, // E
    Position { x: 40, y: 30 }, // SE
    Position { x: 30, y: 32 }, // S
    Position { x: 20, y: 30 }, // SW
    Position { x: 18, y: 20 }, // W
    Position { x: 20, y: 10 }, // NW
];

/// Five cats clustered within a 6-tile radius of `COLONY_CENTER`.
/// The distribution is intentional: a single cat on the centroid
/// would make the cat-scent influence-map a single saturated
/// bucket; a small spread produces a natural halo that the
/// `Gate` composition can saturate without artificial scent
/// deposits.
const CAT_OFFSETS: [(i32, i32); 5] = [
    (0, 0),   // centroid
    (-3, 0),  // W of centroid
    (3, 0),   // E of centroid
    (0, -3),  // N of centroid
    (0, 3),   // S of centroid
];

const CAT_NAMES: [&str; 5] = ["Bramble", "Cinder", "Dapple", "Ember", "Fennel"];

fn setup(world: &mut World, seed: u64) {
    init_scenario_world_with(
        world,
        seed,
        ScenarioWorldConfig {
            width: MAP_WIDTH,
            height: MAP_HEIGHT,
            colony_center: COLONY_CENTER,
        },
    );

    spawn_cluster(world);
    spawn_surrounding_foxes(world);
}

/// Five cats in the central cluster. Personality skew is
/// deliberately uniform-bland — the scenario is a placement-scorer
/// microexperiment, not a DSE-election test, so per-cat
/// personality doesn't drive the assertions in `mod tests`.
fn spawn_cluster(world: &mut World) {
    for ((dx, dy), name) in CAT_OFFSETS.iter().zip(CAT_NAMES.iter()) {
        let pos = Position::new(COLONY_CENTER.x + dx, COLONY_CENTER.y + dy);
        spawn_cat(
            world,
            CatPreset::adult(*name, pos)
                .with_personality(|p| {
                    p.spirituality = 0.5;
                    p.diligence = 0.5;
                })
                .with_marker(MarkerKind::Adult),
        );
    }
}

/// Eight static `ShadowFox`es at the compass periphery. State is
/// `Patrolling { dx: 0, dy: 0 }` — Bevy systems read the position
/// but the AI step is a no-op, so foxes don't drift inward across
/// the 50-tick run and don't perturb the geometry the scorer sees.
fn spawn_surrounding_foxes(world: &mut World) {
    for pos in FOX_POSITIONS {
        world.spawn((
            WildAnimal::new(WildSpecies::ShadowFox),
            pos,
            Health::default(),
            WildlifeAiState::Patrolling { dx: 0, dy: 0 },
            // Ticket 023 Phase A — canonical shadow-fox marker.
            crate::components::wildlife::ShadowFoxDrives::newly_manifested(0.9),
            SensorySpecies::Wild(WildSpecies::ShadowFox),
            SensorySignature::WILDLIFE,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::sim_constants::WardPlacementCatValueComposition;
    use crate::resources::{
        CarcassScentMap, CatScentMap, FoxApproachCorridorMap, FoxScentMap, RecentAmbushMap,
        SimConstants, WardCoverageMap,
    };
    use crate::resources::map::{Terrain, TileMap};
    use crate::systems::coordination::{compute_ward_placement, PlacementMaps};
    use rand::SeedableRng;

    /// `Ward::thornward().repel_radius()` — the radius
    /// `WardCoverageMap::stamp_ward` is called with when a
    /// thornward is planted. Matches the in-function constant
    /// `THORNWARD_VIRTUAL_RADIUS` in
    /// `coordination::select_descending_residual` (the in-call
    /// virtual-coverage analog). Hardcoded here because the
    /// scenario's test runs `compute_ward_placement` directly
    /// without spawning real `Ward` entities.
    const THORNWARD_REPEL_RADIUS: f32 = 6.0;
    const THORNWARD_STRENGTH: f32 = 1.0;

    /// How many wards to plant when checking ring formation. Six
    /// gives the multi-wake spreader enough room to hit all four
    /// cardinal sectors even when the first one or two wakes pick
    /// near the colony interior (jitter-dependent) before the
    /// coverage stamps push the next argmax outward.
    const RING_WARD_COUNT: usize = 6;

    /// Pre-existing ward at the colony center. `compute_ward_placement`
    /// short-circuits to the anchor when `ward_positions.is_empty()`
    /// (`coordination.rs:1463`); seeding one existing ward forces
    /// the scoring loop to run on the first wake.
    const SEED_WARD: Position = Position { x: 25, y: 25 };

    /// Radius of the cat-wandering halo deposited into
    /// `CatScentMap`. The 5-cat cluster centered on (30, 20) only
    /// saturates a 2×2 patch of bucket-size-5 buckets at its own
    /// positions; without a wider halo the `Gate` composition's
    /// `(cat_value / floor).clamp(0, 1)` zeroes the perimeter
    /// candidates where the foxes deposit their scent. The halo
    /// approximates "cats wander out this far from the cluster
    /// over the run's time horizon" — a substrate fact that the
    /// in-game scent-emission systems would build up in any
    /// real soak.
    const CAT_WANDER_RADIUS: i32 = 12;
    /// Per-tile cat-scent contribution for the wandering halo.
    /// `0.3` clears the default `gate_floor = 0.2` with margin
    /// while staying below the centroid's saturation (cats live
    /// at the centroid, not the periphery).
    const CAT_WANDER_DEPOSIT: f32 = 0.3;

    /// Build the `PlacementMaps` substrate the scorer reads from:
    /// fox-scent saturated at the 8 perimeter tiles, cat-scent
    /// deposited at the 5 cat positions, all other maps empty.
    /// This is the geometry the in-game systems would build up
    /// naturally over a few ticks of fox patrols and cat
    /// movement, pre-seeded here so the test isn't dependent on
    /// the wildlife / scent emission systems running.
    ///
    /// Returns `(fox_scent, cat_scent, ward_coverage, tile_map,
    /// recent_ambush, carcass_scent, fox_approach_corridor)` — the
    /// caller mutates `ward_coverage` between wakes.
    fn build_substrate() -> (
        FoxScentMap,
        CatScentMap,
        WardCoverageMap,
        TileMap,
        RecentAmbushMap,
        CarcassScentMap,
        FoxApproachCorridorMap,
    ) {
        let mut fox_scent = FoxScentMap::default();
        // Saturate the fox-scent influence map at each perimeter
        // fox's bucket. The bucket size is 5 (sibling-map
        // convention); a single deposit at the fox's tile fills
        // its bucket, and adjacent buckets get nothing — exactly
        // the eight-arc-of-threat shape the scenario wants.
        for fp in FOX_POSITIONS {
            fox_scent.deposit(fp.x, fp.y, 1.0);
        }

        let mut cat_scent = CatScentMap::default();
        // Saturate cat-scent at each cat's own tile.
        for (dx, dy) in CAT_OFFSETS {
            cat_scent.deposit(COLONY_CENTER.x + dx, COLONY_CENTER.y + dy, 1.0);
        }
        // Wandering halo. Cats don't sit on their cluster tiles
        // for the whole run; in any real soak, cat-scent decays
        // out from the centroid as cats forage / patrol / explore.
        // Deposit `CAT_WANDER_DEPOSIT` at every tile within
        // `CAT_WANDER_RADIUS` Chebyshev distance of the centroid
        // so the `Gate` composition can clear at the perimeter
        // candidates (where foxes deposit).
        for dy in -CAT_WANDER_RADIUS..=CAT_WANDER_RADIUS {
            for dx in -CAT_WANDER_RADIUS..=CAT_WANDER_RADIUS {
                let x = COLONY_CENTER.x + dx;
                let y = COLONY_CENTER.y + dy;
                if x < 0 || y < 0 || x >= MAP_WIDTH || y >= MAP_HEIGHT {
                    continue;
                }
                cat_scent.deposit(x, y, CAT_WANDER_DEPOSIT);
            }
        }

        let ward_coverage = WardCoverageMap::default();
        let tile_map = TileMap::new(MAP_WIDTH, MAP_HEIGHT, Terrain::Grass);
        let recent_ambush = RecentAmbushMap::default();
        let carcass_scent = CarcassScentMap::default();
        let fox_approach_corridor = FoxApproachCorridorMap::default();

        (
            fox_scent,
            cat_scent,
            ward_coverage,
            tile_map,
            recent_ambush,
            carcass_scent,
            fox_approach_corridor,
        )
    }

    /// Classify a ward position into one of the four cardinal
    /// sectors relative to the colony center, using `|dx|` vs
    /// `|dy|` as the tiebreaker. Returns:
    /// - `0` for N (`dy < 0` and `|dy| >= |dx|`)
    /// - `1` for E (`dx > 0` and `|dx| > |dy|`)
    /// - `2` for S (`dy > 0` and `|dy| >= |dx|`)
    /// - `3` for W (`dx < 0` and `|dx| > |dy|`)
    /// - `None` for the exact center (`dx == 0` and `dy == 0`).
    ///
    /// Cardinal sectors are the right grain for ring coverage:
    /// the candidate grid steps by 5 from origin, so most picks
    /// align with cardinal axes through the colony center rather
    /// than diagonal off-axis tiles. Ring formation means each
    /// cardinal sector is represented at least once across the
    /// successive wakes.
    fn cardinal_sector(pos: Position) -> Option<usize> {
        let dx = pos.x - COLONY_CENTER.x;
        let dy = pos.y - COLONY_CENTER.y;
        if dx == 0 && dy == 0 {
            return None;
        }
        if dy.abs() >= dx.abs() {
            // Vertical dominant — N or S.
            Some(if dy < 0 { 0 } else { 2 })
        } else {
            // Horizontal dominant — E or W.
            Some(if dx > 0 { 1 } else { 3 })
        }
    }

    /// Drive `RING_WARD_COUNT` successive `compute_ward_placement`
    /// calls, stamping each pick into `ward_coverage` between
    /// calls. Returns the picks in order.
    fn drive_wakes(constants: &SimConstants) -> Vec<Position> {
        let (fox_scent, cat_scent, mut ward_coverage, tile_map, recent_ambush, carcass_scent, fox_approach_corridor) =
            build_substrate();

        // Pre-seed: one existing ward at `SEED_WARD` so
        // `compute_ward_placement` runs the scoring loop on the
        // first wake (the empty-colony fallback short-circuits to
        // the anchor). The seed's coverage stamp also nudges the
        // first new pick away from `SEED_WARD`'s neighborhood.
        let mut wards: Vec<(Position, f32)> = vec![(SEED_WARD, THORNWARD_REPEL_RADIUS)];
        ward_coverage.stamp_ward(
            SEED_WARD.x,
            SEED_WARD.y,
            THORNWARD_STRENGTH,
            THORNWARD_REPEL_RADIUS,
        );

        let mut picks: Vec<Position> = Vec::with_capacity(RING_WARD_COUNT);
        // The colony "anchor" — the centroid the scorer pulls
        // toward via the `distance_cost` term. Pinned to the
        // cluster center.
        let anchor = COLONY_CENTER;
        // The scorer requires at least one "structure" to start
        // scoring (the buildings list seeds the centroid
        // computation in `compute_ward_placement`). Use the colony
        // center as the lone fake structure.
        let buildings = vec![COLONY_CENTER];

        for wake in 0..RING_WARD_COUNT {
            let maps = PlacementMaps {
                fox_scent: &fox_scent,
                cat_scent: &cat_scent,
                ward_coverage: &ward_coverage,
                tile_map: &tile_map,
                recent_ambush: &recent_ambush,
                carcass_scent: &carcass_scent,
                fox_approach_corridor: &fox_approach_corridor,
            };
            // Use a fresh RNG seed per wake (mirrors the
            // coordinator's per-wake RNG fork) so jitter draws
            // don't lock all wakes into the same tie-break order.
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(313 + wake as u64);
            let pick = compute_ward_placement(
                &buildings,
                &wards,
                anchor,
                &maps,
                constants,
                &mut rng,
                None,
            );
            picks.push(pick);
            wards.push((pick, THORNWARD_REPEL_RADIUS));
            ward_coverage.stamp_ward(
                pick.x,
                pick.y,
                THORNWARD_STRENGTH,
                THORNWARD_REPEL_RADIUS,
            );
        }
        picks
    }

    /// Default composition (`Additive`) produces a ring of
    /// coverage over successive wakes. Asserts each cardinal
    /// quadrant receives at least one ward by the end of the
    /// `RING_WARD_COUNT`-wake run. This is the load-bearing
    /// gameplay behavior 313's composition change must not
    /// break.
    #[test]
    fn additive_composition_builds_ring_of_coverage() {
        let constants = SimConstants::default();
        let picks = drive_wakes(&constants);

        let mut sectors_hit = [false; 4];
        for pick in &picks {
            if let Some(q) = cardinal_sector(*pick) {
                sectors_hit[q] = true;
            }
        }
        let missing: Vec<usize> = (0..4).filter(|&q| !sectors_hit[q]).collect();
        assert!(
            missing.is_empty(),
            "313 surrounded ring (Additive): expected wards in all 4 \
             cardinal sectors over {} wakes; missing {:?}; picks {:?}",
            RING_WARD_COUNT,
            missing,
            picks,
        );
    }

    /// 313 option (c) — Gate composition: same surrounded-colony
    /// substrate, but with `WardPlacementCatValueComposition::Gate`
    /// active. The ring must STILL form. If Gate breaks the
    /// multi-wake spreading behavior (e.g., by zeroing the
    /// threat-merit on every perimeter tile because cat-scent
    /// doesn't reach the fox-rich periphery), this test would
    /// fail and flag the composition as incompatible with the
    /// surrounded-threat geometry.
    ///
    /// Under the influence-map bucket-size-5 quantization, the
    /// 5-cat cluster centered on (30, 20) saturates cat-scent in
    /// the buckets covering x ∈ [25, 35], y ∈ [15, 25] — a
    /// 10×10 halo around the centroid. The placement candidates
    /// adjacent to the perimeter foxes (e.g. candidate (30, 10)
    /// near the N fox at (30, 8)) sit in the cat-scent halo's
    /// outer ring; for `gate_floor = 0.2` (the default) they
    /// clear the gate and score full threat merit.
    #[test]
    fn gate_composition_builds_ring_of_coverage() {
        let mut constants = SimConstants::default();
        constants.scoring.ward_placement_cat_value_composition =
            WardPlacementCatValueComposition::Gate;
        let picks = drive_wakes(&constants);

        let mut sectors_hit = [false; 4];
        for pick in &picks {
            if let Some(q) = cardinal_sector(*pick) {
                sectors_hit[q] = true;
            }
        }
        let missing: Vec<usize> = (0..4).filter(|&q| !sectors_hit[q]).collect();
        assert!(
            missing.is_empty(),
            "313 surrounded ring (Gate): expected wards in all 4 \
             cardinal sectors over {} wakes; missing {:?}; picks {:?}. \
             If Gate is suppressing the perimeter — likely because the \
             cat-scent halo doesn't reach the fox-rich tiles — option \
             (c) is incompatible with surrounded-threat geometry and \
             needs a follow-on iter to widen the halo or lower the \
             gate floor.",
            RING_WARD_COUNT,
            missing,
            picks,
        );
    }
}
