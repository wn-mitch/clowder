use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// FoodStores
// ---------------------------------------------------------------------------

/// Colony food supply. Cats deposit food from hunting/foraging and consume
/// it when eating. Spoils slowly each tick.
///
/// **Semantic note (190).** `current` / `capacity` describe food in `Stores`
/// buildings only — the historical resource the chronic-full latch
/// (`ColonyStoresChronicallyFull`), `HasStoredFood`, and the coordinator's
/// food-pressure assessment all reason about. The `in_stores` / `in_dens` /
/// `in_workshops` / `held` breakdown fields cover the broader UI question
/// "where is all the food right now?" without shifting the meaning of
/// `current` under existing backend consumers.
#[derive(Resource, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FoodStores {
    /// Food units in `Stores` buildings (the canonical "stockpile" number
    /// most backend systems reason about). Equal to `in_stores as f32` after
    /// each `sync_food_stores` tick.
    pub current: f32,
    /// Maximum capacity of all `Stores` buildings combined (including
    /// storage-upgrade bonuses).
    pub capacity: f32,
    /// Count of food items currently in `Stores` buildings.
    #[serde(default)]
    pub in_stores: u32,
    /// Count of food items currently in `Den` buildings (stashes).
    #[serde(default)]
    pub in_dens: u32,
    /// Count of food items currently in `Workshop` buildings (staging during
    /// crafting / cooking).
    #[serde(default)]
    pub in_workshops: u32,
    /// Count of food items currently carried in cat inventories.
    #[serde(default)]
    pub held: u32,
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
            in_stores: current as u32,
            in_dens: 0,
            in_workshops: 0,
            held: 0,
            spoilage_rate,
            spoilage_multiplier: 1.0,
        }
    }

    /// Deposit food, clamped to capacity. Updates `in_stores` to mirror.
    pub fn deposit(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.capacity);
        self.in_stores = self.current as u32;
    }

    /// Withdraw food. Returns the amount actually withdrawn (may be less if
    /// stores are nearly empty).
    pub fn withdraw(&mut self, amount: f32) -> f32 {
        let taken = amount.min(self.current);
        self.current -= taken;
        self.in_stores = self.current as u32;
        taken
    }

    /// Apply per-tick spoilage, scaled by `spoilage_multiplier`.
    pub fn spoil(&mut self) {
        self.current = (self.current - self.spoilage_rate * self.spoilage_multiplier).max(0.0);
        self.in_stores = self.current as u32;
    }

    /// Fraction of `Stores` capacity filled, in [0.0, 1.0].
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

    /// Total food accessible to the colony — Stores + Dens + Workshops +
    /// items carried by living cats. UI-facing display number; backend
    /// systems should use the specific breakdown field they care about.
    pub fn total_accessible(&self) -> u32 {
        self.in_stores + self.in_dens + self.in_workshops + self.held
    }
}

impl Default for FoodStores {
    fn default() -> Self {
        Self {
            current: 0.0,  // Recalculated by sync_food_stores from actual items.
            capacity: 0.0, // Recalculated by sync_food_stores from actual Stores buildings.
            in_stores: 0,
            in_dens: 0,
            in_workshops: 0,
            held: 0,
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

    #[test]
    fn total_accessible_sums_all_sources() {
        let mut fs = FoodStores::new(0.0, 50.0, 0.002);
        fs.in_stores = 12;
        fs.in_dens = 3;
        fs.in_workshops = 1;
        fs.held = 4;
        assert_eq!(fs.total_accessible(), 20);
    }

    #[test]
    fn total_accessible_zero_by_default() {
        let fs = FoodStores::default();
        assert_eq!(fs.total_accessible(), 0);
    }
}
