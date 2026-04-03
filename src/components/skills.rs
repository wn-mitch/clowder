use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

/// Learned ability scores. All values are `f32` with no hard upper bound, but
/// conventionally start small and grow through use.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Skills {
    pub hunting: f32,
    pub foraging: f32,
    pub herbcraft: f32,
    pub building: f32,
    pub combat: f32,
    pub magic: f32,
}

impl Default for Skills {
    fn default() -> Self {
        Self {
            hunting: 0.1,
            foraging: 0.1,
            herbcraft: 0.05,
            building: 0.1,
            combat: 0.05,
            magic: 0.0,
        }
    }
}

impl Skills {
    /// Sum of all skill levels.
    pub fn total(&self) -> f32 {
        self.hunting + self.foraging + self.herbcraft + self.building + self.combat + self.magic
    }

    /// Diminishing-returns factor for skill growth.
    ///
    /// Returns a value in `(0, 1]`: high total skill → slower growth rate.
    pub fn growth_rate(&self) -> f32 {
        1.0 / (1.0 + self.total())
    }
}

// ---------------------------------------------------------------------------
// Magic-related components
// ---------------------------------------------------------------------------

/// Innate magical aptitude. Higher values mean the cat learns magic faster
/// and casts more powerfully.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MagicAffinity(pub f32);

/// Accumulated magical corruption. Excessive magic use or exposure to dark
/// sources raises this value.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Corruption(pub f32);

// ---------------------------------------------------------------------------
// Training
// ---------------------------------------------------------------------------

/// Tracks a teaching relationship. A cat may mentor at most one apprentice
/// and have at most one mentor at a time.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct Training {
    pub mentor: Option<Entity>,
    pub apprentice: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_sums_correctly() {
        let s = Skills::default();
        let expected = 0.1 + 0.1 + 0.05 + 0.1 + 0.05 + 0.0;
        let diff = (s.total() - expected).abs();
        assert!(diff < 1e-6, "expected {expected}, got {}", s.total());
    }

    #[test]
    fn growth_rate_diminishes_with_higher_total() {
        let low = Skills::default();
        let mut high = Skills::default();
        high.hunting = 5.0;
        high.foraging = 5.0;

        assert!(
            low.growth_rate() > high.growth_rate(),
            "low-skill cat should grow faster: {} vs {}",
            low.growth_rate(),
            high.growth_rate()
        );
    }

    #[test]
    fn growth_rate_at_zero_total_is_one() {
        let s = Skills {
            hunting: 0.0,
            foraging: 0.0,
            herbcraft: 0.0,
            building: 0.0,
            combat: 0.0,
            magic: 0.0,
        };
        assert_eq!(s.growth_rate(), 1.0);
    }

    #[test]
    fn growth_rate_always_positive() {
        let mut s = Skills::default();
        s.hunting = 100.0;
        assert!(s.growth_rate() > 0.0);
    }
}
