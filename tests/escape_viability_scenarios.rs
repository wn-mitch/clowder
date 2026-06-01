//! Ticket 103 + 138 — integration test for the `escape_viability`
//! perception scalar.
//!
//! Verifies the pure-fn `interoception::escape_viability` produces the
//! expected band across four synthetic scenarios (open meadow, walled
//! corner, open-with-dependent, no-threat short-circuit). Pure-function
//! unit tests live in `src/systems/interoception.rs`'s `mod tests`;
//! this file is the single-purpose integration test that proves the
//! helper is callable from a downstream-crate position with the same
//! `EscapeViabilityConstants` defaults the sim ships with.
//!
//! All scenarios use neutral cadence (own = threat = 1.0/tick) for
//! continuity with ticket 103's original assertions; the mobility
//! term contributes a fixed +0.1 (= `mobility_weight × 0.5`) in every
//! case. The mobility-differential dynamics themselves are exercised
//! in the unit-test mod.

use clowder::components::physical::Position;
use clowder::resources::map::{Terrain, TileMap};
use clowder::resources::sim_constants::EscapeViabilityConstants;
use clowder::systems::interoception::escape_viability;

const NEUTRAL_CADENCE: f32 = 1.0;

#[test]
fn open_meadow_produces_high_viability() {
    let map = TileMap::new(40, 40, Terrain::Grass);
    let constants = EscapeViabilityConstants::default();
    let v = escape_viability(
        Position::new(20, 20),
        Some(Position::new(5, 5)),
        NEUTRAL_CADENCE,
        NEUTRAL_CADENCE,
        &map,
        false,
        &constants,
    );
    // terrain_weight (0.6) × full openness (1.0) + mobility_weight
    // (0.2) × neutral (0.5) − dependent_term (0.0) = 0.7. Same band
    // as the original 103-era assertion.
    assert!(v > 0.5, "open meadow should score high; got {v}");
}

#[test]
fn walled_corner_produces_low_viability() {
    // 9×9 wall map with a tiny 3×3 grass pocket at the corner. Cat
    // sits at (1, 1) with most of its sprint box blocked.
    let mut map = TileMap::new(9, 9, Terrain::Wall);
    for x in 0..=2 {
        for y in 0..=2 {
            map.set(x, y, Terrain::Grass);
        }
    }
    let constants = EscapeViabilityConstants::default();
    let v = escape_viability(
        Position::new(1, 1),
        Some(Position::new(8, 8)),
        NEUTRAL_CADENCE,
        NEUTRAL_CADENCE,
        &map,
        false,
        &constants,
    );
    assert!(v < 0.3, "cornered cat should score low; got {v}");
}

#[test]
fn open_meadow_with_dependent_falls_below_no_dependent() {
    // Same open meadow, but cat is a parent / pair-bonded — the
    // dependent term subtracts from the terrain + mobility
    // composition.
    let map = TileMap::new(40, 40, Terrain::Grass);
    let constants = EscapeViabilityConstants::default();
    let pos = Position::new(20, 20);
    let threat = Some(Position::new(5, 5));
    let solo = escape_viability(
        pos,
        threat,
        NEUTRAL_CADENCE,
        NEUTRAL_CADENCE,
        &map,
        false,
        &constants,
    );
    let with_dependent = escape_viability(
        pos,
        threat,
        NEUTRAL_CADENCE,
        NEUTRAL_CADENCE,
        &map,
        true,
        &constants,
    );

    assert!(
        with_dependent < solo,
        "dependent must reduce viability ({with_dependent} >= {solo})"
    );
    // Penalty = dependent_weight (0.2) × dependent_penalty (1.0) = 0.2.
    // 0.7 − 0.2 = 0.5. Confirm magnitude.
    assert!(
        (with_dependent - 0.5).abs() < 1e-4,
        "dependent penalty magnitude wrong; got {with_dependent}"
    );
}

#[test]
fn no_threat_returns_one_regardless_of_terrain() {
    let walled = TileMap::new(20, 20, Terrain::Wall);
    let open = TileMap::new(20, 20, Terrain::Grass);
    let constants = EscapeViabilityConstants::default();
    let pos = Position::new(10, 10);
    assert_eq!(
        escape_viability(
            pos,
            None,
            NEUTRAL_CADENCE,
            NEUTRAL_CADENCE,
            &walled,
            true,
            &constants
        ),
        1.0,
        "no-threat short-circuit must ignore terrain (walled)"
    );
    assert_eq!(
        escape_viability(
            pos,
            None,
            NEUTRAL_CADENCE,
            NEUTRAL_CADENCE,
            &open,
            true,
            &constants
        ),
        1.0,
        "no-threat short-circuit must ignore terrain (open)"
    );
}
