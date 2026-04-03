use bevy_ecs::prelude::Resource;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

/// Deterministic RNG resource for the simulation.
///
/// All random decisions during a simulation run should draw from this single
/// resource so that results are fully reproducible given the same seed.
#[derive(Resource)]
pub struct SimRng {
    pub rng: ChaCha8Rng,
}

impl SimRng {
    /// Construct from a 64-bit seed. The seed is expanded to the 256-bit value
    /// expected by ChaCha8 by repeating its little-endian bytes across the
    /// full seed array.
    pub fn new(seed: u64) -> Self {
        let seed_bytes = seed.to_le_bytes(); // 8 bytes
        let mut full_seed = [0u8; 32];
        for (i, byte) in full_seed.iter_mut().enumerate() {
            *byte = seed_bytes[i % 8];
        }
        Self {
            rng: ChaCha8Rng::from_seed(full_seed),
        }
    }
}
