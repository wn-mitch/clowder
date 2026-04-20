use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// FoodStores
// ---------------------------------------------------------------------------

/// Colony food supply. Cats deposit food from hunting/foraging and consume
/// it when eating. Spoils slowly each tick.
#[derive(Resource, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FoodStores {
    /// Current food units available.
    pub current: f32,
    /// Maximum storage capacity.
    pub capacity: f32,
    /// Food lost per tick to spoilage.
    pub spoilage_rate: f32,
    /// Per-tick multiplier applied to spoilage rate.
    ///
    /// Set by the building effects system (e.g. functional Stores halves this
    /// to 0.5). Reset to 1.0 each tick before building effects run.
    pub spoilage_multiplier: f32,
}

impl FoodStores {
    pub fn new(current: f32, capacity: f32, spoilage_rate: f32) -> Self {
        Self {
            current,
            capacity,
            spoilage_rate,
            spoilage_multiplier: 1.0,
        }
    }

    /// Deposit food, clamped to capacity.
    pub fn deposit(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.capacity);
    }

    /// Withdraw food. Returns the amount actually withdrawn (may be less if
    /// stores are nearly empty).
    pub fn withdraw(&mut self, amount: f32) -> f32 {
        let taken = amount.min(self.current);
        self.current -= taken;
        taken
    }

    /// Apply per-tick spoilage, scaled by `spoilage_multiplier`.
    pub fn spoil(&mut self) {
        self.current = (self.current - self.spoilage_rate * self.spoilage_multiplier).max(0.0);
    }

    /// Fraction of capacity filled, in [0.0, 1.0].
    pub fn fraction(&self) -> f32 {
        if self.capacity <= 0.0 {
            0.0
        } else {
            (self.current / self.capacity).clamp(0.0, 1.0)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.current <= 0.0
    }
}

impl Default for FoodStores {
    fn default() -> Self {
        Self {
            current: 0.0,  // Recalculated by sync_food_stores from actual items.
            capacity: 0.0, // Recalculated by sync_food_stores from actual Stores buildings.
            spoilage_rate: 0.002,
            spoilage_multiplier: 1.0,
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
    fn deposit_clamps_to_capacity() {
        let mut fs = FoodStores::new(45.0, 50.0, 0.002);
        fs.deposit(10.0);
        assert_eq!(fs.current, 50.0);
    }

    #[test]
    fn withdraw_returns_available() {
        let mut fs = FoodStores::new(1.0, 50.0, 0.002);
        let taken = fs.withdraw(5.0);
        assert_eq!(taken, 1.0);
        assert_eq!(fs.current, 0.0);
    }

    #[test]
    fn spoilage_reduces_current() {
        let mut fs = FoodStores::new(10.0, 50.0, 0.5);
        fs.spoil();
        assert!((fs.current - 9.5).abs() < 1e-6);
    }

    #[test]
    fn spoilage_does_not_go_negative() {
        let mut fs = FoodStores::new(0.001, 50.0, 0.01);
        fs.spoil();
        assert_eq!(fs.current, 0.0);
    }

    #[test]
    fn fraction_reflects_fill_level() {
        let fs = FoodStores::new(25.0, 50.0, 0.002);
        assert!((fs.fraction() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn empty_when_zero() {
        let fs = FoodStores::new(0.0, 50.0, 0.002);
        assert!(fs.is_empty());
    }
}
