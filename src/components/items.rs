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
    RawRabbit,
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
    HerbCatnip,
    HerbSlumbershade,
    HerbOracleOrchid,

    // --- Curiosities ---
    ShinyPebble,
    GlassShard,
    ColorfulShell,

    // --- Shadow materials ---
    ShadowBone,

    // --- Storage upgrades ---
    Barrel,
    Crate,
    Shelf,

    // --- Build materials (bridge into the construction `Material` enum;
    // physical-causality form of materials cats haul to ConstructionSites). ---
    Wood,
    Stone,
}

impl ItemKind {
    /// Per-tick decay rate applied to `Item::condition`.
    ///
    /// - Raw prey: 0.0005 (spoils in ~2000 ticks)
    /// - Foraged organic: 0.0005 (same rate as raw prey)
    /// - Herbs: 0.0003 (preserved longer)
    /// - Inorganic / curiosities: 0.0 (no decay)
    pub fn decay_rate(self) -> f32 {
        match self {
            Self::RawMouse | Self::RawRat | Self::RawRabbit | Self::RawFish | Self::RawBird => {
                0.0001
            }

            Self::Berries
            | Self::Nuts
            | Self::Roots
            | Self::WildOnion
            | Self::Mushroom
            | Self::Moss
            | Self::DriedGrass
            | Self::Feather => 0.0005,

            Self::HerbHealingMoss
            | Self::HerbMoonpetal
            | Self::HerbCalmroot
            | Self::HerbThornbriar
            | Self::HerbDreamroot
            | Self::HerbCatnip
            | Self::HerbSlumbershade
            | Self::HerbOracleOrchid => 0.0003,

            Self::ShinyPebble | Self::GlassShard | Self::ColorfulShell | Self::ShadowBone => 0.0,

            Self::Barrel | Self::Crate | Self::Shelf => 0.0,

            Self::Wood | Self::Stone => 0.0,
        }
    }

    /// Bridge to the construction `Material` enum. Returns `Some(_)` for the
    /// item kinds that can be delivered to a `ConstructionSite`. Used by
    /// `resolve_pickup_material` and `resolve_deliver` to identify carried
    /// build-material units.
    pub fn material(self) -> Option<crate::components::task_chain::Material> {
        use crate::components::task_chain::Material;
        match self {
            Self::Wood => Some(Material::Wood),
            Self::Stone => Some(Material::Stone),
            _ => None,
        }
    }

    /// Extra item capacity granted when this item is stored in a building.
    /// Most items provide no bonus; storage upgrades add slots.
    pub fn capacity_bonus(self) -> usize {
        match self {
            Self::Barrel => 10,
            Self::Crate => 8,
            Self::Shelf => 15,
            _ => 0,
        }
    }

    /// Returns true if this item can be eaten.
    pub fn is_food(self) -> bool {
        matches!(
            self,
            Self::RawMouse
                | Self::RawRat
                | Self::RawRabbit
                | Self::RawFish
                | Self::RawBird
                | Self::Berries
                | Self::Nuts
                | Self::Roots
                | Self::WildOnion
                | Self::Mushroom
        )
    }

    /// Human-readable name for narrative output.
    pub fn name(self) -> &'static str {
        match self {
            Self::RawMouse => "mouse",
            Self::RawRat => "rat",
            Self::RawRabbit => "rabbit",
            Self::RawFish => "fish",
            Self::RawBird => "bird",
            Self::Berries => "berries",
            Self::Nuts => "nuts",
            Self::Roots => "roots",
            Self::WildOnion => "wild onion",
            Self::Mushroom => "mushrooms",
            Self::Moss => "moss",
            Self::DriedGrass => "dried grass",
            Self::Feather => "feathers",
            Self::HerbHealingMoss => "healing moss",
            Self::HerbMoonpetal => "moonpetal",
            Self::HerbCalmroot => "calmroot",
            Self::HerbThornbriar => "thornbriar",
            Self::HerbDreamroot => "dreamroot",
            Self::HerbCatnip => "catnip",
            Self::HerbSlumbershade => "slumbershade",
            Self::HerbOracleOrchid => "oracle orchid",
            Self::ShinyPebble => "shiny pebble",
            Self::GlassShard => "glass shard",
            Self::ColorfulShell => "colorful shell",
            Self::ShadowBone => "shadow bone",
            Self::Barrel => "barrel",
            Self::Crate => "crate",
            Self::Shelf => "shelf",
            Self::Wood => "wood",
            Self::Stone => "stone",
        }
    }

    /// Whether `name()` returns a grammatically plural form.
    pub fn is_plural_name(self) -> bool {
        matches!(
            self,
            Self::Berries | Self::Nuts | Self::Roots | Self::Mushroom | Self::Feather
        )
    }

    /// Singular form of the item name for grammatical contexts like "every last X".
    pub fn singular_name(self) -> &'static str {
        match self {
            Self::Berries => "berry",
            Self::Nuts => "nut",
            Self::Roots => "root",
            Self::Mushroom => "mushroom",
            Self::Feather => "feather",
            _ => self.name(),
        }
    }

    /// Hunger satisfaction provided when consumed (0.0–1.0 scale).
    /// Non-food items return 0.0.
    ///
    /// Tuned so a single hunt feeds a cat for days. Hunted prey is a real
    /// meal (0.5–0.8); foraged plants are snacks (0.20).
    pub fn food_value(self) -> f32 {
        match self {
            Self::RawRat => 0.8,
            Self::RawRabbit => 0.65,
            Self::RawMouse => 0.5,
            Self::RawFish => 0.7,
            Self::RawBird => 0.6,
            Self::Berries | Self::Nuts | Self::Roots | Self::Mushroom | Self::WildOnion => 0.2,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Item modifiers
// ---------------------------------------------------------------------------

/// Modifiers stamped onto an item at creation time. Corruption is captured from
/// the source tile when the item is first produced (hunt catch, forage, den
/// raid). Future modifiers (blessed, poisoned, shadow-touched) add fields here.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemModifiers {
    /// Corruption level from the source tile, clamped to `[0.0, 1.0]`.
    pub corruption: f32,
    /// True if the item has been cooked at a Kitchen. Cooked food yields a
    /// hunger-restoration multiplier in `resolve_eat_at_stores`.
    #[serde(default)]
    pub cooked: bool,
}

impl ItemModifiers {
    pub fn with_corruption(corruption: f32) -> Self {
        Self {
            corruption: corruption.clamp(0.0, 1.0),
            ..Self::default()
        }
    }

    /// True when the item has no negative modifiers (i.e. is not corrupted).
    /// Cooked items are still "clean" — cooking is a positive modifier.
    pub fn is_clean(&self) -> bool {
        self.corruption == 0.0
    }
}

/// Returns a display name combining quality, modifiers, and kind.
/// Examples: `"corrupted rat"`, `"exceptional corrupted rat"`, `"fine rabbit"`.
pub fn item_display_name(kind: ItemKind, quality: f32, modifiers: &ItemModifiers) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(ql) = quality_label(quality) {
        parts.push(ql);
    }
    if modifiers.corruption > 0.3 {
        parts.push("corrupted");
    }
    parts.push(kind.name());
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Quality tiers for narrative
// ---------------------------------------------------------------------------

/// Returns a narrative label for notable item quality. Common quality returns
/// `None` — only poor and above-average items are worth mentioning.
pub fn quality_label(quality: f32) -> Option<&'static str> {
    if quality < 0.2 {
        Some("poor")
    } else if quality >= 0.8 {
        Some("exceptional")
    } else if quality >= 0.5 {
        Some("fine")
    } else {
        None // common quality — not worth narrating
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
// Build-material marker
// ---------------------------------------------------------------------------

/// Marker stamped on ground `Item` entities whose `kind` is a build
/// material (`Wood` / `Stone`). Used to make the planner's mutable
/// build-material query (`BuildingResolverParams::material_items`)
/// statically disjoint from the read-only `items_query` consumed by
/// food/herb resolvers (`eat_at_stores`, `deposit_at_stores`, etc).
/// Without it, both queries overlap on the same `Item` entities and
/// Bevy's borrow checker (B0001) rejects the system.
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct BuildMaterialItem;

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
    /// Modifiers stamped at creation (corruption, etc.). Defaults to clean
    /// for items created before this field existed.
    #[serde(default)]
    pub modifiers: ItemModifiers,
}

impl Item {
    /// Create a new item with quality clamped to `[0.0, 1.0]` and clean modifiers.
    pub fn new(kind: ItemKind, quality: f32, location: ItemLocation) -> Self {
        Self {
            kind,
            quality: quality.clamp(0.0, 1.0),
            condition: 1.0,
            location,
            modifiers: ItemModifiers::default(),
        }
    }

    /// Create a new item with explicit modifiers.
    pub fn with_modifiers(
        kind: ItemKind,
        quality: f32,
        location: ItemLocation,
        modifiers: ItemModifiers,
    ) -> Self {
        Self {
            kind,
            quality: quality.clamp(0.0, 1.0),
            condition: 1.0,
            location,
            modifiers,
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
        assert!(ItemKind::RawRabbit.is_food());
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
        // RawFish decay_rate = 0.0001; condition starts at 1.0, so ~10000 ticks
        // to fully decay. Allow up to 11000 to be float-safe.
        let mut destroyed = false;
        for _ in 0..11000 {
            if item.tick_decay() {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed, "RawFish should be destroyed within 11000 ticks");
    }

    #[test]
    fn inorganic_items_do_not_decay() {
        let mut item = Item::new(ItemKind::ShinyPebble, 1.0, ItemLocation::OnGround);
        for _ in 0..1000 {
            assert!(!item.tick_decay(), "ShinyPebble should never decay");
        }
        assert!((item.condition - 1.0).abs() < f32::EPSILON);
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
            ItemKind::RawRabbit,
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

    #[test]
    fn item_modifiers_default_is_clean() {
        let mods = ItemModifiers::default();
        assert!(mods.is_clean());
        assert_eq!(mods.corruption, 0.0);
    }

    #[test]
    fn with_corruption_clamps_to_unit_range() {
        let mods = ItemModifiers::with_corruption(2.5);
        assert_eq!(mods.corruption, 1.0);
        let mods = ItemModifiers::with_corruption(-0.3);
        assert_eq!(mods.corruption, 0.0);
    }

    #[test]
    fn corrupted_item_reduces_effective_food_value() {
        let penalty = 0.5;
        let base = ItemKind::RawRat.food_value(); // 0.8
        let mods = ItemModifiers::with_corruption(0.6);
        let freshness = 1.0 - mods.corruption * penalty;
        let effective = base * freshness;
        assert!(effective < base, "corrupted food should give less hunger");
        assert!((effective - 0.56).abs() < 0.01, "0.8 * 0.7 ≈ 0.56");
    }

    #[test]
    fn clean_item_gives_full_food_value() {
        let penalty = 0.5;
        let base = ItemKind::RawRat.food_value();
        let mods = ItemModifiers::default();
        let freshness = 1.0 - mods.corruption * penalty;
        assert_eq!(freshness, 1.0);
        assert_eq!(base * freshness, base);
    }

    #[test]
    fn item_display_name_reflects_corruption() {
        let mods = ItemModifiers::with_corruption(0.5);
        let name = item_display_name(ItemKind::RawRat, 0.4, &mods);
        assert_eq!(name, "corrupted rat");
    }

    #[test]
    fn item_display_name_clean_item() {
        let mods = ItemModifiers::default();
        let name = item_display_name(ItemKind::RawRat, 0.4, &mods);
        assert_eq!(name, "rat");
    }

    #[test]
    fn item_display_name_quality_and_corruption() {
        let mods = ItemModifiers::with_corruption(0.8);
        let name = item_display_name(ItemKind::RawRabbit, 0.85, &mods);
        assert_eq!(name, "exceptional corrupted rabbit");
    }

    #[test]
    fn item_new_has_clean_modifiers() {
        let item = Item::new(ItemKind::RawFish, 0.5, ItemLocation::OnGround);
        assert!(item.modifiers.is_clean());
    }

    #[test]
    fn item_with_modifiers_preserves_corruption() {
        let mods = ItemModifiers::with_corruption(0.7);
        let item = Item::with_modifiers(ItemKind::Berries, 0.5, ItemLocation::OnGround, mods);
        assert_eq!(item.modifiers.corruption, 0.7);
    }

    #[test]
    fn cooked_defaults_false_and_preserves_through_with_modifiers() {
        let default = ItemModifiers::default();
        assert!(!default.cooked);
        let with_corr = ItemModifiers::with_corruption(0.4);
        assert!(!with_corr.cooked);
    }

    #[test]
    fn cooked_item_yields_multiplier_on_hunger_math() {
        // Mirrors the formula in `resolve_eat_at_stores`.
        let cooked_food_multiplier = 1.3_f32;
        let penalty = 0.5_f32;
        let base = ItemKind::RawRat.food_value(); // 0.8
        let raw_mods = ItemModifiers::default();
        let cooked_mods = ItemModifiers {
            corruption: 0.0,
            cooked: true,
        };
        let freshness = 1.0 - raw_mods.corruption * penalty;
        let raw_value = base * freshness;
        let cooked_value = base
            * freshness
            * if cooked_mods.cooked {
                cooked_food_multiplier
            } else {
                1.0
            };
        assert!(
            (cooked_value - raw_value * 1.3).abs() < 1e-4,
            "cooked item should yield 1.3× the raw food value"
        );
    }
}
