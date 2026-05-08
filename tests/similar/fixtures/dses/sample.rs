//! Sample DSE for similar.py test fixtures.
//!
//! Mirrors the shape of real DSEs in src/ai/dses/ — a module-level
//! doc comment plus per-item doc comments on the scoring functions.

/// Score the Patrol disposition for a given cat.
///
/// Patrol elevates when Hunt and Forage are saturation-suppressed,
/// because L3 freed bandwidth flows downward through the disposition
/// stack. Currently does NOT price predator-exposure cost — see
/// ticket 194 for the structural extension.
pub fn score_patrol(cat: &Cat, world: &World) -> f32 {
    0.0
}

/// Score the Mate disposition. Gated by the courtship marker; does not
/// fire for cats without the marker.
pub fn score_mate(cat: &Cat, world: &World) -> f32 {
    0.0
}
