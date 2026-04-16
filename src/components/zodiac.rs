use bevy_ecs::prelude::*;
use rand::Rng;

// ---------------------------------------------------------------------------
// Zodiac Sign
// ---------------------------------------------------------------------------

/// One of eight celestial constellation signs — star patterns that elders teach
/// kits about in the nursery. Each sign has a domain affinity and personality
/// resonance that influence aspiration selection and fated connections.
#[derive(
    Component, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ZodiacSign {
    LeapingFlame,
    SilverPool,
    StormFur,
    WarmDen,
    LoneThorn,
    SwiftRiver,
    TallPine,
    LongShadow,
}

impl ZodiacSign {
    pub const ALL: [ZodiacSign; 8] = [
        Self::LeapingFlame,
        Self::SilverPool,
        Self::StormFur,
        Self::WarmDen,
        Self::LoneThorn,
        Self::SwiftRiver,
        Self::TallPine,
        Self::LongShadow,
    ];

    /// Assign a zodiac sign based on birth season.
    ///
    /// Each season maps to two primary signs (80% chance). There is a 20%
    /// chance of an off-season sign drawn uniformly from all eight.
    pub fn from_season(season: u64, rng: &mut impl Rng) -> Self {
        let primary = match season % 4 {
            0 => [Self::LeapingFlame, Self::StormFur], // Spring — fire and storms
            1 => [Self::SwiftRiver, Self::TallPine],   // Summer — rivers and growth
            2 => [Self::LoneThorn, Self::LongShadow],  // Autumn — solitude and patience
            3 => [Self::SilverPool, Self::WarmDen],    // Winter — reflection and warmth
            _ => unreachable!(),
        };
        if rng.random::<f32>() < 0.8 {
            primary[rng.random_range(0..2)]
        } else {
            Self::ALL[rng.random_range(0..8)]
        }
    }

    /// Human-readable constellation name for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::LeapingFlame => "The Leaping Flame",
            Self::SilverPool => "The Silver Pool",
            Self::StormFur => "The Storm Fur",
            Self::WarmDen => "The Warm Den",
            Self::LoneThorn => "The Lone Thorn",
            Self::SwiftRiver => "The Swift River",
            Self::TallPine => "The Tall Pine",
            Self::LongShadow => "The Long Shadow",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn all_array_has_all_variants() {
        assert_eq!(ZodiacSign::ALL.len(), 8);
        // Each variant appears exactly once.
        let mut seen = std::collections::HashSet::new();
        for sign in ZodiacSign::ALL {
            assert!(seen.insert(sign));
        }
    }

    #[test]
    fn from_season_produces_valid_signs() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for season in 0..100 {
            let sign = ZodiacSign::from_season(season, &mut rng);
            assert!(ZodiacSign::ALL.contains(&sign));
        }
    }

    #[test]
    fn from_season_is_deterministic() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(99);
        let mut rng2 = ChaCha8Rng::seed_from_u64(99);
        for season in 0..50 {
            assert_eq!(
                ZodiacSign::from_season(season, &mut rng1),
                ZodiacSign::from_season(season, &mut rng2),
            );
        }
    }

    #[test]
    fn spring_favours_flame_and_storm() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut flame = 0;
        let mut storm = 0;
        let n = 1000;
        for _ in 0..n {
            match ZodiacSign::from_season(0, &mut rng) {
                ZodiacSign::LeapingFlame => flame += 1,
                ZodiacSign::StormFur => storm += 1,
                _ => {}
            }
        }
        // ~80% should be primary signs (each ~40%).
        let primary_frac = (flame + storm) as f64 / n as f64;
        assert!(
            primary_frac > 0.7,
            "primary fraction {primary_frac} too low"
        );
    }

    #[test]
    fn label_returns_nonempty_string() {
        for sign in ZodiacSign::ALL {
            assert!(!sign.label().is_empty());
            assert!(sign.label().starts_with("The "));
        }
    }
}
