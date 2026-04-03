use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// ItemKind
// ---------------------------------------------------------------------------

/// Every distinct type of physical item that can exist in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ItemKind {
    // --- Raw prey ---
    RawMouse,
    RawRat,
    RawFish,
    RawBird,

    // --- Foraged ---
    Berries,
    Nuts,
    Roots,
    WildOnion,
    Mushroom,
    Moss,
    DriedGrass,
    Feather,

    // --- Herbs (mirror HerbKind) ---
    HerbHealingMoss,
    HerbMoonpetal,
    HerbCalmroot,
    HerbThornbriar,
    HerbDreamroot,

    // --- Curiosities ---
    ShinyPebble,
    GlassShard,
    ColorfulShell,
}

impl ItemKind {
    /// Per-tick decay rate applied to `Item::condition`.
    ///
    /// - Raw prey: 0.01 (spoils in ~100 ticks)
    /// - Foraged organic: 0.005 (slower, but still perishable)
    /// - Herbs: 0.003 (preserved longer)
    /// - Inorganic / curiosities: 0.0 (no decay)
    pub fn decay_rate(self) -> f32 {
        match self {
            Self::RawMouse | Self::RawRat | Self::RawFish | Self::RawBird => 0.01,

            Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather => 0.005,

            Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot => 0.003,

            Self::ShinyPebble | Self::GlassShard | Self::ColorfulShell => 0.0,
        }
    }

    /// Returns true if this item can be eaten.
    pub fn is_food(self) -> bool {
        matches!(
            self,
            Self::RawMouse
                | Self::RawRat
                | Self::RawFish
                | Self::RawBird
                | Self::Berries
                | Self::Nuts
                | Self::Roots
                | Self::WildOnion
                | Self::Mushroom
        )
    }

    /// Hunger satisfaction provided when consumed (0.0–1.0 scale).
    /// Non-food items return 0.0.
    pub fn food_value(self) -> f32 {
        match self {
            Self::RawRat => 0.4,
            Self::RawMouse => 0.25,
            Self::RawFish => 0.35,
            Self::RawBird => 0.3,
            Self::Berries => 0.1,
            Self::Nuts => 0.15,
            Self::Roots | Self::WildOnion | Self::Mushroom => 0.1,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ItemLocation
// ---------------------------------------------------------------------------

/// Where an item currently resides.
///
/// Variants containing `Entity` are not serializable — entity handles are
/// runtime identifiers that cannot survive a save/load round-trip. The
/// `location` field in `Item` is therefore skipped during serialization and
/// defaults to `OnGround` on deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemLocation {
    /// Carried in a cat's inventory. The entity is the carrier.
    Carried(Entity),
    /// Lying on the ground; the item entity also has a `Position` component.
    OnGround,
    /// Stored inside a building. The entity is the containing structure.
    StoredIn(Entity),
}

impl ItemLocation {
    /// Default used by serde when deserializing items whose location cannot
    /// be restored from the save file.
    fn on_ground() -> Self {
        Self::OnGround
    }
}

// ---------------------------------------------------------------------------
// Item component
// ---------------------------------------------------------------------------

/// A physical item entity in the world.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    /// Overall quality, clamped to `[0.0, 1.0]` at construction.
    pub quality: f32,
    /// Current condition, starts at 1.0 and decays toward 0.0.
    pub condition: f32,
    /// Current location. Skipped during serialization because `Entity`
    /// handles are not stable across save/load boundaries; restored to
    /// `OnGround` on deserialization.
    #[serde(skip, default = "ItemLocation::on_ground")]
    pub location: ItemLocation,
}

impl Item {
    /// Create a new item with quality clamped to `[0.0, 1.0]`.
    pub fn new(kind: ItemKind, quality: f32, location: ItemLocation) -> Self {
        Self {
            kind,
            quality: quality.clamp(0.0, 1.0),
            condition: 1.0,
            location,
        }
    }

    /// Advance decay by one tick.
    ///
    /// Returns `true` if the item should be destroyed (condition has reached
    /// or dropped below 0.0).
    pub fn tick_decay(&mut self) -> bool {
        self.condition -= self.kind.decay_rate();
        self.is_destroyed()
    }

    /// True when condition has reached 0.0 or below.
    pub fn is_destroyed(&self) -> bool {
        self.condition <= 0.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_prey_is_food() {
        assert!(ItemKind::RawMouse.is_food());
        assert!(ItemKind::RawRat.is_food());
        assert!(ItemKind::RawFish.is_food());
        assert!(ItemKind::RawBird.is_food());
        assert!(ItemKind::Berries.is_food());
        assert!(ItemKind::Mushroom.is_food());

        assert!(!ItemKind::Moss.is_food());
        assert!(!ItemKind::Feather.is_food());
        assert!(!ItemKind::ShinyPebble.is_food());
        assert!(!ItemKind::HerbHealingMoss.is_food());
    }

    #[test]
    fn item_decays_over_time() {
        let mut item = Item::new(ItemKind::RawFish, 1.0, ItemLocation::OnGround);
        // RawFish decay_rate = 0.01; with f32 rounding the condition clears
        // slightly above 100 ticks. Allow up to 110 to be float-safe.
        let mut destroyed = false;
        for _ in 0..110 {
            if item.tick_decay() {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed, "RawFish should be destroyed within 110 ticks");
    }

    #[test]
    fn inorganic_items_do_not_decay() {
        let mut item = Item::new(ItemKind::ShinyPebble, 1.0, ItemLocation::OnGround);
        for _ in 0..1000 {
            assert!(!item.tick_decay(), "ShinyPebble should never decay");
        }
        assert_eq!(item.condition, 1.0);
    }

    #[test]
    fn quality_is_clamped() {
        let over = Item::new(ItemKind::Nuts, 5.0, ItemLocation::OnGround);
        assert_eq!(over.quality, 1.0, "quality above 1.0 should clamp to 1.0");

        let under = Item::new(ItemKind::Nuts, -3.0, ItemLocation::OnGround);
        assert_eq!(under.quality, 0.0, "quality below 0.0 should clamp to 0.0");

        let mid = Item::new(ItemKind::Nuts, 0.7, ItemLocation::OnGround);
        assert_eq!(mid.quality, 0.7, "quality in range should be unchanged");
    }

    #[test]
    fn food_values_are_positive_for_food_items() {
        let food_items = [
            ItemKind::RawMouse,
            ItemKind::RawRat,
            ItemKind::RawFish,
            ItemKind::RawBird,
            ItemKind::Berries,
            ItemKind::Nuts,
            ItemKind::Roots,
            ItemKind::WildOnion,
            ItemKind::Mushroom,
        ];
        for kind in food_items {
            assert!(
                kind.food_value() > 0.0,
                "{kind:?} is food but has food_value == 0.0"
            );
        }
    }
}
