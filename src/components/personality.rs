use bevy_ecs::prelude::*;
use rand::Rng;

// ---------------------------------------------------------------------------
// Personality
// ---------------------------------------------------------------------------

/// 18-axis personality stored in three conceptual layers.
///
/// All axes are `f32` in `[0.0, 1.0]`. Values near 0 represent the low end
/// of the axis (e.g. cowardly, reclusive) and values near 1 represent the
/// high end (e.g. bold, sociable).
///
/// Generated with a 2-sample average which biases values toward 0.5 while
/// still allowing the full range.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Personality {
    // --- Core Drives (8) ---
    pub boldness: f32,
    pub sociability: f32,
    pub curiosity: f32,
    pub diligence: f32,
    pub warmth: f32,
    pub spirituality: f32,
    pub ambition: f32,
    pub patience: f32,

    // --- Temperament (5) ---
    pub anxiety: f32,
    pub optimism: f32,
    pub temper: f32,
    pub stubbornness: f32,
    pub playfulness: f32,

    // --- Values (5) ---
    pub loyalty: f32,
    pub tradition: f32,
    pub compassion: f32,
    pub pride: f32,
    pub independence: f32,
}

impl Personality {
    /// Generate a random personality using a bell-curve approximation.
    ///
    /// Each axis is the average of two independent uniform samples, producing
    /// a triangular distribution centred on 0.5 that still allows values near
    /// 0 and 1.
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        let mut bell = || -> f32 {
            let a: f32 = rng.random();
            let b: f32 = rng.random();
            (a + b) / 2.0
        };

        Self {
            boldness: bell(),
            sociability: bell(),
            curiosity: bell(),
            diligence: bell(),
            warmth: bell(),
            spirituality: bell(),
            ambition: bell(),
            patience: bell(),

            anxiety: bell(),
            optimism: bell(),
            temper: bell(),
            stubbornness: bell(),
            playfulness: bell(),

            loyalty: bell(),
            tradition: bell(),
            compassion: bell(),
            pride: bell(),
            independence: bell(),
        }
    }

    /// Iterate over all 18 axis values.
    pub fn all_values(&self) -> [f32; 18] {
        [
            self.boldness,
            self.sociability,
            self.curiosity,
            self.diligence,
            self.warmth,
            self.spirituality,
            self.ambition,
            self.patience,
            self.anxiety,
            self.optimism,
            self.temper,
            self.stubbornness,
            self.playfulness,
            self.loyalty,
            self.tradition,
            self.compassion,
            self.pride,
            self.independence,
        ]
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

    fn seeded_rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    #[test]
    fn all_values_in_range_over_100_generations() {
        let mut rng = seeded_rng(42);
        for _ in 0..100 {
            let p = Personality::random(&mut rng);
            for v in p.all_values() {
                assert!(
                    (0.0..=1.0).contains(&v),
                    "value {v} is out of [0.0, 1.0]"
                );
            }
        }
    }

    #[test]
    fn deterministic_with_same_seed() {
        let p1 = Personality::random(&mut seeded_rng(99));
        let p2 = Personality::random(&mut seeded_rng(99));
        assert_eq!(p1, p2);
    }

    #[test]
    fn bell_curve_mean_near_half_over_1000_samples() {
        let mut rng = seeded_rng(7);
        let mut sum = 0.0f64;
        let n = 1000;
        for _ in 0..n {
            let p = Personality::random(&mut rng);
            for v in p.all_values() {
                sum += v as f64;
            }
        }
        let mean = sum / (n as f64 * 18.0);
        // Triangular distribution centred on 0.5 — mean should be very close
        assert!(
            (0.45..=0.55).contains(&mean),
            "mean {mean} is not near 0.5"
        );
    }
}
