use bevy_ecs::prelude::*;

use crate::resources::map::Terrain;
use crate::resources::time::Season;

// ---------------------------------------------------------------------------
// Growth stages (shared by Herb and FlavorPlant)
// ---------------------------------------------------------------------------

/// Visual growth stage of a plant entity. Advances over time while in season.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GrowthStage {
    Sprout,
    Bud,
    Bloom,
    Blossom,
}

impl GrowthStage {
    /// Advance to the next stage. Returns None if already at Blossom.
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Sprout => Some(Self::Bud),
            Self::Bud => Some(Self::Bloom),
            Self::Bloom => Some(Self::Blossom),
            Self::Blossom => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Herb system
// ---------------------------------------------------------------------------

/// The species of herb that can be gathered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HerbKind {
    /// Healing poultice ingredient. Near water and forest.
    HealingMoss,
    /// Energy restorative ingredient. Light forest and grass.
    Moonpetal,
    /// Mood tonic ingredient. Any non-water terrain.
    Calmroot,
    /// Ward material. Forest edges.
    Thornbriar,
    /// Visions and narrative events. Fairy rings and standing stones only.
    Dreamroot,
    /// Playful mood herb. Open grass and clearings.
    Catnip,
    /// Rest and anxiety-easing herb. Forest shade.
    Slumbershade,
    /// Rare visions herb. Near standing stones and fairy rings.
    OracleOrchid,
}

impl HerbKind {
    /// Terrain types where this herb can spawn.
    pub fn spawn_terrains(self) -> &'static [Terrain] {
        match self {
            Self::HealingMoss => &[Terrain::Water, Terrain::LightForest, Terrain::DenseForest],
            Self::Moonpetal => &[Terrain::LightForest, Terrain::Grass],
            Self::Calmroot => &[
                Terrain::Grass,
                Terrain::LightForest,
                Terrain::DenseForest,
                Terrain::Rock,
                Terrain::Mud,
                Terrain::Sand,
            ],
            Self::Thornbriar => &[Terrain::LightForest, Terrain::DenseForest],
            Self::Dreamroot => &[Terrain::FairyRing, Terrain::StandingStone],
            Self::Catnip => &[Terrain::Grass, Terrain::Garden, Terrain::LightForest],
            Self::Slumbershade => &[Terrain::DenseForest, Terrain::LightForest],
            Self::OracleOrchid => &[
                Terrain::FairyRing,
                Terrain::StandingStone,
                Terrain::AncientRuin,
            ],
        }
    }

    /// Seasons during which this herb can be harvested.
    pub fn available_seasons(self) -> &'static [Season] {
        match self {
            Self::HealingMoss => &[Season::Spring, Season::Summer, Season::Autumn],
            Self::Moonpetal => &[Season::Summer],
            Self::Calmroot => &[Season::Spring, Season::Summer],
            Self::Thornbriar => &[
                Season::Spring,
                Season::Summer,
                Season::Autumn,
                Season::Winter,
            ],
            Self::Dreamroot => &[Season::Autumn, Season::Winter],
            Self::Catnip => &[Season::Spring, Season::Summer],
            Self::Slumbershade => &[Season::Autumn, Season::Winter],
            Self::OracleOrchid => &[Season::Summer, Season::Autumn],
        }
    }

    /// TUI map symbol for this herb.
    pub fn symbol(self) -> char {
        'h'
    }

    /// Spawn density: probability that an eligible tile actually gets this herb.
    pub fn spawn_density(self) -> f32 {
        match self {
            Self::HealingMoss => 0.15,
            Self::Moonpetal => 0.10,
            Self::Calmroot => 0.08,
            Self::Thornbriar => 0.12,
            Self::Dreamroot => 1.0,
            Self::Catnip => 0.12,
            Self::Slumbershade => 0.10,
            Self::OracleOrchid => 0.60,
        }
    }
}

/// An herb entity in the world.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Herb {
    pub kind: HerbKind,
    /// Visual growth stage. Advances while in season; resets to Sprout at season end.
    pub growth_stage: GrowthStage,
    /// True if growing on a tile with high mystery.
    pub magical: bool,
    /// True if corrupted — cannot be harvested and may cause negative effects.
    pub twisted: bool,
}

/// Marker: this herb can be harvested right now (correct season, not twisted).
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Harvestable;

/// Tracks which seasons an herb entity is available for harvest.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Seasonal {
    pub available: Vec<Season>,
}

// ---------------------------------------------------------------------------
// Flavor plants (non-harvestable world decoration)
// ---------------------------------------------------------------------------

/// Decorative plant species with no harvest use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FlavorKind {
    // Seasonal flowering plants
    Sunflower,
    Rose,
    // Stone decorations (spawned permanently, no seasonal cycle)
    Pebble,
    Rock,
    Stone,
    StoneChunk,
    StoneFlat,
    Boulder,
}

impl FlavorKind {
    /// Terrain types where this plant/decoration can spawn.
    pub fn spawn_terrains(self) -> &'static [Terrain] {
        match self {
            Self::Sunflower => &[Terrain::Grass, Terrain::Garden, Terrain::LightForest],
            Self::Rose => &[Terrain::Grass, Terrain::Garden],
            Self::Pebble
            | Self::Rock
            | Self::Stone
            | Self::StoneChunk
            | Self::StoneFlat
            | Self::Boulder => &[Terrain::Rock, Terrain::Sand],
        }
    }

    /// Seasons during which this flavor plant is visible. Rocks return all seasons.
    pub fn available_seasons(self) -> &'static [Season] {
        match self {
            Self::Sunflower => &[Season::Summer],
            Self::Rose => &[Season::Spring, Season::Summer],
            // Rocks are permanent — always present.
            Self::Pebble
            | Self::Rock
            | Self::Stone
            | Self::StoneChunk
            | Self::StoneFlat
            | Self::Boulder => &[
                Season::Spring,
                Season::Summer,
                Season::Autumn,
                Season::Winter,
            ],
        }
    }

    /// Spawn density.
    pub fn spawn_density(self) -> f32 {
        match self {
            Self::Sunflower => 0.06,
            Self::Rose => 0.05,
            Self::Pebble => 0.12,
            Self::Rock => 0.10,
            Self::Stone => 0.08,
            Self::StoneChunk => 0.08,
            Self::StoneFlat => 0.06,
            Self::Boulder => 0.04,
        }
    }

    /// Whether this kind participates in seasonal growth cycling.
    /// Rocks are permanent and skip the growth system.
    pub fn is_seasonal(self) -> bool {
        matches!(self, Self::Sunflower | Self::Rose)
    }
}

/// A non-harvestable decorative plant entity.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlavorPlant {
    pub kind: FlavorKind,
    /// Visual growth stage. Only meaningful for seasonal plants.
    pub growth_stage: GrowthStage,
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

/// A slot in a cat's inventory — either a herb or a generic item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ItemSlot {
    Herb(HerbKind),
    Item(crate::components::items::ItemKind),
}

/// A cat's carried inventory. Capacity-limited; holds both herbs and items.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub slots: Vec<ItemSlot>,
}

impl Inventory {
    pub const MAX_SLOTS: usize = 5;

    pub fn is_full(&self) -> bool {
        self.slots.len() >= Self::MAX_SLOTS
    }

    // --- Herb compatibility methods ---

    pub fn has_herb(&self, kind: HerbKind) -> bool {
        self.slots
            .iter()
            .any(|s| matches!(s, ItemSlot::Herb(h) if *h == kind))
    }

    /// Remove one instance of `kind` from inventory. Returns true if found.
    pub fn take_herb(&mut self, kind: HerbKind) -> bool {
        if let Some(idx) = self
            .slots
            .iter()
            .position(|s| matches!(s, ItemSlot::Herb(h) if *h == kind))
        {
            self.slots.swap_remove(idx);
            true
        } else {
            false
        }
    }

    /// Add a herb. Returns false if inventory is full.
    pub fn add_herb(&mut self, kind: HerbKind) -> bool {
        if self.is_full() {
            return false;
        }
        self.slots.push(ItemSlot::Herb(kind));
        true
    }

    /// Whether the inventory has any herb usable for a remedy.
    pub fn has_remedy_herb(&self) -> bool {
        self.slots.iter().any(|s| {
            matches!(
                s,
                ItemSlot::Herb(HerbKind::HealingMoss | HerbKind::Moonpetal | HerbKind::Calmroot)
            )
        })
    }

    /// Whether the inventory has thornbriar for ward-setting.
    pub fn has_ward_herb(&self) -> bool {
        self.has_herb(HerbKind::Thornbriar)
    }

    /// Return the first remedy kind that can be prepared from current herbs.
    pub fn first_remedy_kind(&self) -> Option<RemedyKind> {
        for slot in &self.slots {
            match slot {
                ItemSlot::Herb(HerbKind::HealingMoss) => return Some(RemedyKind::HealingPoultice),
                ItemSlot::Herb(HerbKind::Moonpetal) => return Some(RemedyKind::EnergyTonic),
                ItemSlot::Herb(HerbKind::Calmroot) => return Some(RemedyKind::MoodTonic),
                _ => {}
            }
        }
        None
    }

    // --- Item methods ---

    pub fn has_item(&self, kind: crate::components::items::ItemKind) -> bool {
        self.slots
            .iter()
            .any(|s| matches!(s, ItemSlot::Item(i) if *i == kind))
    }

    /// Add an item. Returns false if inventory is full.
    pub fn add_item(&mut self, kind: crate::components::items::ItemKind) -> bool {
        if self.is_full() {
            return false;
        }
        self.slots.push(ItemSlot::Item(kind));
        true
    }

    /// Remove one instance of `kind` from inventory. Returns true if found.
    pub fn take_item(&mut self, kind: crate::components::items::ItemKind) -> bool {
        if let Some(idx) = self
            .slots
            .iter()
            .position(|s| matches!(s, ItemSlot::Item(i) if *i == kind))
        {
            self.slots.swap_remove(idx);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Wards
// ---------------------------------------------------------------------------

/// The kind of magical ward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WardKind {
    /// Basic herb-based ward. ~200 tick lifespan.
    Thornward,
    /// Trained-magic ward. ~1000 tick lifespan.
    DurableWard,
}

/// A magical ward entity placed in the world.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ward {
    pub kind: WardKind,
    /// Current strength, 1.0 at creation. Decays per tick.
    pub strength: f32,
    /// Per-tick decay rate.
    pub decay_rate: f32,
    /// True for misfire-created wards that attract instead of repel.
    pub inverted: bool,
}

impl Ward {
    pub fn thornward() -> Self {
        Self {
            kind: WardKind::Thornward,
            strength: 1.0,
            decay_rate: 0.005,
            inverted: false,
        }
    }

    pub fn durable() -> Self {
        Self {
            kind: WardKind::DurableWard,
            strength: 1.0,
            decay_rate: 0.001,
            inverted: false,
        }
    }

    pub fn inverted_at(pos_kind: WardKind) -> Self {
        let mut w = match pos_kind {
            WardKind::Thornward => Self::thornward(),
            WardKind::DurableWard => Self::durable(),
        };
        w.inverted = true;
        w
    }

    /// Effective repulsion radius (tiles). Proportional to strength.
    pub fn repel_radius(&self) -> f32 {
        3.0 * self.strength
    }
}

// ---------------------------------------------------------------------------
// Remedies
// ---------------------------------------------------------------------------

/// The kind of herbal remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemedyKind {
    /// HealingMoss → +healing_poultice_rate health/tick for 20 ticks.
    HealingPoultice,
    /// Moonpetal → +0.03 energy/tick for 15 ticks.
    EnergyTonic,
    /// Calmroot → +0.2 mood modifier for 50 ticks.
    MoodTonic,
}

impl RemedyKind {
    /// Total ticks the remedy effect lasts.
    pub fn duration(self) -> u64 {
        match self {
            Self::HealingPoultice => 20,
            Self::EnergyTonic => 15,
            Self::MoodTonic => 50,
        }
    }

    /// Which herb is consumed to prepare this remedy.
    pub fn required_herb(self) -> HerbKind {
        match self {
            Self::HealingPoultice => HerbKind::HealingMoss,
            Self::EnergyTonic => HerbKind::Moonpetal,
            Self::MoodTonic => HerbKind::Calmroot,
        }
    }
}

/// Active remedy buff on a cat.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemedyEffect {
    pub kind: RemedyKind,
    pub ticks_remaining: u64,
    /// The cat who applied this remedy (for gratitude tracking).
    #[serde(skip)]
    pub healer: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Misfires
// ---------------------------------------------------------------------------

/// Possible outcomes when a magic attempt goes wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MisfireEffect {
    /// Nothing happens. Mild embarrassment.
    Fizzle,
    /// Caster gains +0.1 personal corruption.
    CorruptionBacksplash,
    /// Ward spawned with inverted effect (attracts predators).
    InvertedWard,
    /// Caster takes the injury instead of healing the target.
    WoundTransfer,
    /// Caster's position revealed to dark creatures.
    LocationReveal,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_add_take() {
        let mut inv = Inventory::default();
        assert!(inv.add_herb(HerbKind::HealingMoss));
        assert!(inv.has_herb(HerbKind::HealingMoss));
        assert!(!inv.has_herb(HerbKind::Thornbriar));

        assert!(inv.take_herb(HerbKind::HealingMoss));
        assert!(!inv.has_herb(HerbKind::HealingMoss));
        assert!(!inv.take_herb(HerbKind::HealingMoss)); // already taken
    }

    #[test]
    fn inventory_full() {
        let mut inv = Inventory::default();
        for _ in 0..Inventory::MAX_SLOTS {
            assert!(inv.add_herb(HerbKind::Calmroot));
        }
        assert!(inv.is_full());
        assert!(!inv.add_herb(HerbKind::Thornbriar));
    }

    #[test]
    fn inventory_has_remedy_herb() {
        let mut inv = Inventory::default();
        assert!(!inv.has_remedy_herb());
        inv.add_herb(HerbKind::Thornbriar);
        assert!(!inv.has_remedy_herb()); // thornbriar is ward material
        inv.add_herb(HerbKind::HealingMoss);
        assert!(inv.has_remedy_herb());
    }

    #[test]
    fn ward_constructors() {
        let thorn = Ward::thornward();
        assert_eq!(thorn.kind, WardKind::Thornward);
        assert_eq!(thorn.strength, 1.0);
        assert!(!thorn.inverted);

        let inv = Ward::inverted_at(WardKind::DurableWard);
        assert!(inv.inverted);
        assert_eq!(inv.kind, WardKind::DurableWard);
    }

    #[test]
    fn remedy_required_herbs() {
        assert_eq!(
            RemedyKind::HealingPoultice.required_herb(),
            HerbKind::HealingMoss
        );
        assert_eq!(RemedyKind::EnergyTonic.required_herb(), HerbKind::Moonpetal);
        assert_eq!(RemedyKind::MoodTonic.required_herb(), HerbKind::Calmroot);
    }

    #[test]
    fn herb_seasonal_availability() {
        // Thornbriar available all seasons
        assert_eq!(HerbKind::Thornbriar.available_seasons().len(), 4);
        // Moonpetal only summer
        assert_eq!(HerbKind::Moonpetal.available_seasons(), &[Season::Summer]);
        // Dreamroot autumn/winter
        assert_eq!(
            HerbKind::Dreamroot.available_seasons(),
            &[Season::Autumn, Season::Winter]
        );
    }

    #[test]
    fn new_herbs_have_terrains_and_seasons() {
        assert!(!HerbKind::Catnip.spawn_terrains().is_empty());
        assert!(!HerbKind::Slumbershade.spawn_terrains().is_empty());
        assert!(!HerbKind::OracleOrchid.spawn_terrains().is_empty());
        assert!(!HerbKind::Catnip.available_seasons().is_empty());
        assert!(!HerbKind::Slumbershade.available_seasons().is_empty());
        assert!(!HerbKind::OracleOrchid.available_seasons().is_empty());
    }

    #[test]
    fn growth_stage_advances_to_blossom() {
        assert_eq!(GrowthStage::Sprout.next(), Some(GrowthStage::Bud));
        assert_eq!(GrowthStage::Bud.next(), Some(GrowthStage::Bloom));
        assert_eq!(GrowthStage::Bloom.next(), Some(GrowthStage::Blossom));
        assert_eq!(GrowthStage::Blossom.next(), None);
    }

    #[test]
    fn rocks_are_not_seasonal() {
        assert!(!FlavorKind::Pebble.is_seasonal());
        assert!(!FlavorKind::Boulder.is_seasonal());
        assert!(FlavorKind::Sunflower.is_seasonal());
        assert!(FlavorKind::Rose.is_seasonal());
    }
}
