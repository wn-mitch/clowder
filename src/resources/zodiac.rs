use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::*;

use crate::components::aspirations::AspirationDomain;
use crate::components::zodiac::ZodiacSign;

// ---------------------------------------------------------------------------
// SignData
// ---------------------------------------------------------------------------

/// Per-sign metadata loaded from RON.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SignData {
    pub compatible: Vec<ZodiacSign>,
    pub rival: Vec<ZodiacSign>,
    pub domain_affinities: Vec<AspirationDomain>,
    pub lore: String,
}

// ---------------------------------------------------------------------------
// ZodiacData resource
// ---------------------------------------------------------------------------

/// Colony-wide zodiac compatibility matrix loaded from `assets/data/zodiac.ron`.
#[derive(Resource, Debug, Clone, serde::Deserialize)]
pub struct ZodiacData {
    pub signs: HashMap<ZodiacSign, SignData>,
}

impl ZodiacData {
    /// Load zodiac data from a RON file.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let data: ZodiacData = ron::from_str(&contents)?;
        Ok(data)
    }

    /// Returns a compatibility score between two signs.
    ///
    /// - `+0.5` if `a` lists `b` as compatible.
    /// - `-0.5` if `a` lists `b` as a rival.
    /// - `0.0` otherwise.
    ///
    /// Checked from `a`'s perspective; the matrix is designed to be symmetric.
    pub fn compatibility(&self, a: ZodiacSign, b: ZodiacSign) -> f32 {
        let Some(data) = self.signs.get(&a) else {
            return 0.0;
        };
        if data.compatible.contains(&b) {
            0.5
        } else if data.rival.contains(&b) {
            -0.5
        } else {
            0.0
        }
    }

    /// Domain affinities for a given sign.
    pub fn domain_affinities(&self, sign: ZodiacSign) -> &[AspirationDomain] {
        self.signs
            .get(&sign)
            .map(|d| d.domain_affinities.as_slice())
            .unwrap_or(&[])
    }

    /// Lore text for a given sign.
    pub fn lore(&self, sign: ZodiacSign) -> &str {
        self.signs.get(&sign).map(|d| d.lore.as_str()).unwrap_or("")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_data() -> ZodiacData {
        ZodiacData::load(Path::new("assets/data/zodiac.ron")).expect("zodiac.ron should parse")
    }

    #[test]
    fn ron_loads_all_signs() {
        let data = load_test_data();
        for sign in ZodiacSign::ALL {
            assert!(data.signs.contains_key(&sign), "missing sign {sign:?}",);
        }
    }

    #[test]
    fn compatible_signs_return_positive() {
        let data = load_test_data();
        // LeapingFlame is compatible with StormFur and TallPine.
        assert_eq!(
            data.compatibility(ZodiacSign::LeapingFlame, ZodiacSign::StormFur),
            0.5
        );
        assert_eq!(
            data.compatibility(ZodiacSign::LeapingFlame, ZodiacSign::TallPine),
            0.5
        );
    }

    #[test]
    fn rival_signs_return_negative() {
        let data = load_test_data();
        // LeapingFlame rivals SilverPool and LongShadow.
        assert_eq!(
            data.compatibility(ZodiacSign::LeapingFlame, ZodiacSign::SilverPool),
            -0.5
        );
        assert_eq!(
            data.compatibility(ZodiacSign::LeapingFlame, ZodiacSign::LongShadow),
            -0.5
        );
    }

    #[test]
    fn neutral_signs_return_zero() {
        let data = load_test_data();
        // LeapingFlame and WarmDen are neither compatible nor rival.
        assert_eq!(
            data.compatibility(ZodiacSign::LeapingFlame, ZodiacSign::WarmDen),
            0.0
        );
    }

    #[test]
    fn domain_affinities_nonempty() {
        let data = load_test_data();
        for sign in ZodiacSign::ALL {
            assert!(
                !data.domain_affinities(sign).is_empty(),
                "sign {sign:?} has no domain affinities",
            );
        }
    }

    #[test]
    fn lore_nonempty() {
        let data = load_test_data();
        for sign in ZodiacSign::ALL {
            assert!(!data.lore(sign).is_empty(), "sign {sign:?} has no lore",);
        }
    }
}
