//! Fox personality and needs — truncated Maslow hierarchy for wildlife.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::physical::smoothstep;

// ---------------------------------------------------------------------------
// FoxPersonality — 4 axes, randomized per individual
// ---------------------------------------------------------------------------

/// Individual personality for a fox. Influences GOAP scoring weights.
///
/// Values are in `[0.0, 1.0]`. Generated at spawn with controlled variance
/// so each fox develops distinct behavioral patterns.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxPersonality {
    /// Willingness to take risks: approach cats, push through wards, confront.
    pub boldness: f32,
    /// Preference for raiding over hunting, ambush over confrontation.
    pub cunning: f32,
    /// Weight on cub safety, urgency of den defense and feeding.
    pub protectiveness: f32,
    /// Weight on scent marking, boundary patrol, territory maintenance.
    pub territoriality: f32,
}

impl FoxPersonality {
    /// Generate a random personality with controlled variance.
    ///
    /// Each axis is drawn from a normal-ish distribution centered at 0.5
    /// with standard deviation ~0.15, clamped to [0.1, 0.9].
    pub fn random(rng: &mut impl Rng) -> Self {
        fn axis(rng: &mut dyn rand::RngCore) -> f32 {
            let noise = rng.random_range(-0.3..0.3_f32);
            (0.5 + noise).clamp(0.1, 0.9)
        }

        Self {
            boldness: axis(rng),
            cunning: axis(rng),
            protectiveness: axis(rng),
            territoriality: axis(rng),
        }
    }

    /// Default personality (used for tests).
    pub fn balanced() -> Self {
        Self {
            boldness: 0.5,
            cunning: 0.5,
            protectiveness: 0.5,
            territoriality: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// FoxNeeds — truncated Maslow (3 levels, 6 fields)
// ---------------------------------------------------------------------------

/// Truncated Maslow hierarchy for foxes.
///
/// | Level | Name       | Fields                        |
/// |-------|------------|-------------------------------|
/// | 1     | Survival   | hunger, health_fraction        |
/// | 2     | Territory  | territory_scent, den_security  |
/// | 3     | Offspring  | cub_satiation, cub_safety      |
///
/// Lower levels suppress higher levels when critical, just like cat needs.
/// All values in `[0.0, 1.0]` where 1.0 = fully satisfied.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxNeeds {
    // Level 1: Survival
    /// How fed the fox is. 1.0 = full, 0.0 = starving. Decays over time.
    pub hunger: f32,
    /// Current health as fraction of max. Derived from Health component.
    pub health_fraction: f32,

    // Level 2: Territory
    /// Average scent strength across territory. 1.0 = fully marked.
    pub territory_scent: f32,
    /// Security of the den. 1.0 = no threats nearby, 0.0 = under attack.
    pub den_security: f32,

    // Level 3: Offspring
    /// How recently cubs have been fed. 1.0 = well fed, 0.0 = starving.
    /// Stays at 1.0 if no cubs exist (satisfied by default).
    pub cub_satiation: f32,
    /// Whether cubs are safe from threats. 1.0 = safe, 0.0 = under threat.
    /// Stays at 1.0 if no cubs exist.
    pub cub_safety: f32,
}

impl Default for FoxNeeds {
    fn default() -> Self {
        Self {
            hunger: 0.5,
            health_fraction: 1.0,
            territory_scent: 0.0,
            den_security: 1.0,
            cub_satiation: 1.0,
            cub_safety: 1.0,
        }
    }
}

impl FoxNeeds {
    /// Satisfaction of the survival level (minimum of hunger and health).
    pub fn survival_satisfaction(&self) -> f32 {
        let min = self.hunger.min(self.health_fraction);
        smoothstep(0.15, 0.65, min)
    }

    /// Satisfaction of the territory level (minimum of scent and den security).
    pub fn territory_satisfaction(&self) -> f32 {
        let min = self.territory_scent.min(self.den_security);
        smoothstep(0.1, 0.5, min)
    }

    /// Satisfaction of the offspring level (minimum of cub satiation and safety).
    pub fn offspring_satisfaction(&self) -> f32 {
        let min = self.cub_satiation.min(self.cub_safety);
        smoothstep(0.15, 0.6, min)
    }

    /// How freely a given Maslow level can be pursued.
    ///
    /// Level 1 is never suppressed. Each higher level is the product of
    /// all lower-level satisfactions.
    ///
    /// | level | suppression value                |
    /// |-------|----------------------------------|
    /// | 1     | 1.0 (always)                     |
    /// | 2     | survival satisfaction             |
    /// | 3     | survival × territory satisfaction |
    pub fn level_suppression(&self, level: u8) -> f32 {
        match level {
            1 => 1.0,
            2 => self.survival_satisfaction(),
            3 => self.survival_satisfaction() * self.territory_satisfaction(),
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_needs_are_reasonable() {
        let n = FoxNeeds::default();
        assert!(n.survival_satisfaction() > 0.0);
        assert_eq!(n.cub_satiation, 1.0); // no cubs = satisfied
    }

    #[test]
    fn level_1_never_suppressed() {
        let n = FoxNeeds {
            hunger: 0.0,
            health_fraction: 0.0,
            ..FoxNeeds::default()
        };
        assert_eq!(n.level_suppression(1), 1.0);
    }

    #[test]
    fn starving_fox_suppresses_territory() {
        let n = FoxNeeds {
            hunger: 0.0,
            health_fraction: 0.0,
            territory_scent: 1.0,
            den_security: 1.0,
            ..FoxNeeds::default()
        };
        // Survival satisfaction is 0 → territory suppressed
        assert_eq!(n.level_suppression(2), 0.0);
    }

    #[test]
    fn healthy_fed_fox_can_pursue_territory() {
        let n = FoxNeeds {
            hunger: 0.8,
            health_fraction: 0.9,
            territory_scent: 0.2,
            den_security: 0.8,
            ..FoxNeeds::default()
        };
        assert!(n.level_suppression(2) > 0.5);
    }

    #[test]
    fn offspring_suppressed_by_both_lower_levels() {
        // Survival ok but territory failing
        let n = FoxNeeds {
            hunger: 0.8,
            health_fraction: 0.9,
            territory_scent: 0.0,
            den_security: 0.0,
            cub_satiation: 0.0,
            cub_safety: 0.0,
        };
        assert!(n.level_suppression(3) < 0.1);
    }

    #[test]
    fn personality_random_stays_in_bounds() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            let p = FoxPersonality::random(&mut rng);
            assert!(p.boldness >= 0.1 && p.boldness <= 0.9);
            assert!(p.cunning >= 0.1 && p.cunning <= 0.9);
            assert!(p.protectiveness >= 0.1 && p.protectiveness <= 0.9);
            assert!(p.territoriality >= 0.1 && p.territoriality <= 0.9);
        }
    }
}
