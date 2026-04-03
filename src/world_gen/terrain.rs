use noise::{NoiseFn, Perlin};
use rand::Rng;

use crate::resources::map::{Terrain, TileMap};

/// Noise scale — smaller values produce broader, smoother features.
const SCALE: f64 = 0.05;

/// Generate a terrain map using two-octave Perlin noise.
///
/// - First noise function (elevation) drives the water/rock boundary.
/// - Second noise function (moisture) drives forest/sand/mud variation.
/// - The seed for each Perlin function is drawn from `rng` so the caller
///   controls determinism via the RNG.
pub fn generate_terrain(width: i32, height: i32, rng: &mut impl Rng) -> TileMap {
    let elev_seed: u32 = rng.random();
    let moist_seed: u32 = rng.random();

    let elevation = Perlin::new(elev_seed);
    let moisture = Perlin::new(moist_seed);

    let mut map = TileMap::new(width, height, Terrain::Grass);

    for y in 0..height {
        for x in 0..width {
            let ex = x as f64 * SCALE;
            let ey = y as f64 * SCALE;

            let e = elevation.get([ex, ey]);
            let m = moisture.get([ex, ey]);

            let terrain = classify(e, m);
            map.set(x, y, terrain);
        }
    }

    map
}

/// Classify a tile based on elevation and moisture noise values.
///
/// Both inputs are Perlin noise in approximately `[-1, 1]`.
fn classify(e: f64, m: f64) -> Terrain {
    if e < -0.3 {
        Terrain::Water
    } else if e > 0.6 {
        Terrain::Rock
    } else if m > 0.3 && e > 0.2 {
        Terrain::DenseForest
    } else if m > 0.3 {
        Terrain::LightForest
    } else if m < -0.2 && e < 0.0 {
        Terrain::Mud
    } else if m < -0.2 {
        Terrain::Sand
    } else {
        Terrain::Grass
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_chacha::rand_core::SeedableRng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    #[test]
    fn correct_dimensions() {
        let mut r = rng(1);
        let map = generate_terrain(80, 60, &mut r);
        assert_eq!(map.width, 80);
        assert_eq!(map.height, 60);
    }

    #[test]
    fn terrain_variety_present() {
        let mut r = rng(42);
        let map = generate_terrain(120, 120, &mut r);

        let mut has_grass = false;
        let mut has_forest = false;
        let mut has_water = false;

        for y in 0..map.height {
            for x in 0..map.width {
                match map.get(x, y).terrain {
                    Terrain::Grass => has_grass = true,
                    Terrain::LightForest | Terrain::DenseForest => has_forest = true,
                    Terrain::Water => has_water = true,
                    _ => {}
                }
            }
        }

        assert!(has_grass, "no grass tiles generated");
        assert!(has_forest, "no forest tiles generated");
        assert!(has_water, "no water tiles generated");
    }

    #[test]
    fn generation_is_deterministic() {
        let map1 = generate_terrain(80, 60, &mut rng(77));
        let map2 = generate_terrain(80, 60, &mut rng(77));

        for y in 0..map1.height {
            for x in 0..map1.width {
                assert_eq!(
                    map1.get(x, y).terrain,
                    map2.get(x, y).terrain,
                    "mismatch at ({x}, {y})"
                );
            }
        }
    }
}
