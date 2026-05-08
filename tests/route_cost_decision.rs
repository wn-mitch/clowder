//! Ticket 228 microexperiment — bold-vs-timid route-cost suppression.
//!
//! Demonstrates the load-bearing claim of the cat-keyed route-cost
//! field: when a fox-scent corridor lies between cat and prey, a bold
//! cat's flood treats the corridor as cheap (low boldness-conditioned
//! overlay weight) and a timid cat's flood treats it as expensive
//! (high weight). The downstream consequence at L2 is that bold cats
//! score Hunt highly at prey behind the corridor while timid cats
//! suppress it — the substrate-grade fix that 222/223/224's path-cost
//! overlay alone couldn't deliver.
//!
//! This file asserts the L1 invariant directly via `flood_dijkstra`,
//! independent of the L2 evaluator and L3 softmax. The L2 conversion
//! is exercised by `src/scenarios/route_cost_decision.rs` running
//! through `just scenario`; the L1 substrate is what this test pins.

use clowder::ai::pathfinding::{
    cat_path_weight_from_boldness, CorruptionOverlay, FoxScentOverlay, WeightedOverlay,
};
use clowder::ai::route_cost::flood_dijkstra;
use clowder::components::physical::Position;
use clowder::resources::map::{Terrain, TileMap};
use clowder::resources::sim_constants::ScoringConstants;
use clowder::resources::FoxScentMap;

/// Build a world identical to the route_cost_decision scenario:
///   - 40×40 grass map.
///   - Bold + Timid spawn locations (used as flood origins).
///   - Single-bucket fox-scent corridor at world-x 17 (bucket index
///     `17 / 5 = 3`), saturating the bucket so all tiles in
///     `[15..=19] × [0..=39]` carry max scent.
fn corridor_world() -> (TileMap, FoxScentMap, ScoringConstants) {
    let map = TileMap::new(40, 40, Terrain::Grass);
    // FoxScentMap::default() is a 120×90 grid; we need one sized for
    // our 40×40 scenario, so build it explicitly.
    let mut fox = FoxScentMap::new(40, 40, 5);
    for y in 0..40 {
        // Saturate the bucket containing world-x 17. `deposit` clamps
        // to 1.0; depositing once at any tile in the bucket sets it.
        fox.deposit(17, y, 1.0);
    }
    let sc = ScoringConstants::default();
    (map, fox, sc)
}

const BOLD_START: Position = Position { x: 5, y: 20 };
const PREY_POS: Position = Position { x: 35, y: 20 };

/// L1 invariant — bold flood reaches the prey at strictly lower cost
/// than timid flood. The corridor's per-tile cost is
/// `round(scent * fox_scent_path_cost_max) = 8` for max-scent tiles;
/// the boldness factor scales the per-flood weight on the
/// `WeightedOverlay`, which the bucket-Dijkstra reads as edge cost.
#[test]
fn bold_route_cost_to_prey_is_strictly_lower_than_timid() {
    let (map, fox, sc) = corridor_world();
    let fox_overlay = FoxScentOverlay::new(&fox, &sc);
    // CorruptionOverlay reads tile.corruption; on a fresh grass map
    // every tile is uncorrupted, so this overlay contributes 0 — but
    // we wire it anyway to mirror the production replan-time flood.
    let corr_overlay = CorruptionOverlay::new(&map, &sc);

    let bold_w = cat_path_weight_from_boldness(0.9);
    let timid_w = cat_path_weight_from_boldness(0.1);

    let bold_overlays = [
        WeightedOverlay::new(&fox_overlay, bold_w),
        WeightedOverlay::new(&corr_overlay, bold_w),
    ];
    let timid_overlays = [
        WeightedOverlay::new(&fox_overlay, timid_w),
        WeightedOverlay::new(&corr_overlay, timid_w),
    ];

    let bold_field = flood_dijkstra(BOLD_START, &map, &bold_overlays, sc.route_cost_flood_budget, 0);
    let timid_field =
        flood_dijkstra(BOLD_START, &map, &timid_overlays, sc.route_cost_flood_budget, 0);

    let bold_cost = bold_field.cost_at(PREY_POS);
    let timid_cost = timid_field.cost_at(PREY_POS);

    assert!(
        bold_cost < timid_cost,
        "bold route-cost ({bold_cost}) must be strictly less than timid ({timid_cost}) — \
         the corridor is the only difference, and bold's overlay weight is the lower of the two"
    );
    // Sanity: bold's reach to the corridor's near edge should be cheap
    // grass (≈ chebyshev distance × 1).
    let near_edge = Position::new(14, 20);
    let bold_near = bold_field.cost_at(near_edge);
    assert!(
        bold_near <= 10,
        "bold reach to corridor near-edge ({bold_near}) should be ~chebyshev × terrain"
    );
}

/// Sanity: with no fox corridor at all, bold and timid floods produce
/// identical costs at the prey landmark. Confirms the cost difference
/// in the previous test is attributable to the corridor, not boldness
/// alone.
#[test]
fn no_corridor_means_no_bold_timid_difference() {
    let map = TileMap::new(40, 40, Terrain::Grass);
    let fox = FoxScentMap::new(40, 40, 5); // empty
    let sc = ScoringConstants::default();
    let fox_overlay = FoxScentOverlay::new(&fox, &sc);
    let corr_overlay = CorruptionOverlay::new(&map, &sc);
    let bold_w = cat_path_weight_from_boldness(0.9);
    let timid_w = cat_path_weight_from_boldness(0.1);
    let bold_overlays = [
        WeightedOverlay::new(&fox_overlay, bold_w),
        WeightedOverlay::new(&corr_overlay, bold_w),
    ];
    let timid_overlays = [
        WeightedOverlay::new(&fox_overlay, timid_w),
        WeightedOverlay::new(&corr_overlay, timid_w),
    ];
    let bold = flood_dijkstra(BOLD_START, &map, &bold_overlays, sc.route_cost_flood_budget, 0);
    let timid = flood_dijkstra(BOLD_START, &map, &timid_overlays, sc.route_cost_flood_budget, 0);
    assert_eq!(
        bold.cost_at(PREY_POS),
        timid.cost_at(PREY_POS),
        "with no corridor, boldness shouldn't change reach cost on uniform grass"
    );
}
