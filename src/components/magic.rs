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

    pub fn to_item_kind(self) -> crate::components::items::ItemKind {
        use crate::components::items::ItemKind;
        match self {
            Self::HealingMoss => ItemKind::HerbHealingMoss,
            Self::Moonpetal => ItemKind::HerbMoonpetal,
            Self::Calmroot => ItemKind::HerbCalmroot,
            Self::Thornbriar => ItemKind::HerbThornbriar,
            Self::Dreamroot => ItemKind::HerbDreamroot,
            Self::Catnip => ItemKind::HerbCatnip,
            Self::Slumbershade => ItemKind::HerbSlumbershade,
            Self::OracleOrchid => ItemKind::HerbOracleOrchid,
        }
    }

    pub fn from_item_kind(kind: crate::components::items::ItemKind) -> Option<Self> {
        use crate::components::items::ItemKind;
        Some(match kind {
            ItemKind::HerbHealingMoss => Self::HealingMoss,
            ItemKind::HerbMoonpetal => Self::Moonpetal,
            ItemKind::HerbCalmroot => Self::Calmroot,
            ItemKind::HerbThornbriar => Self::Thornbriar,
            ItemKind::HerbDreamroot => Self::Dreamroot,
            ItemKind::HerbCatnip => Self::Catnip,
            ItemKind::HerbSlumbershade => Self::Slumbershade,
            ItemKind::HerbOracleOrchid => Self::OracleOrchid,
            _ => return None,
        })
    }
}

/// Resource category tracked by the `ColonyReserves` aggregator and per-cat
/// `ColonyReservesBelief` substrate (ticket 308). Distinct from `HerbKind`:
/// `RemedyHerb` collapses the three remedy ingredients (`HealingMoss` /
/// `Moonpetal` / `Calmroot`) into a single bucket because the supply-chain
/// signal cats anticipate is "do we have remedy material at all," not which
/// specific remedy. Mirror of `inventory.has_remedy_herb()` classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ResourceKind {
    /// Ward material. Consumed by `resolve_set_ward` (Thornward branch).
    Thornbriar,
    /// Aggregate of HealingMoss / Moonpetal / Calmroot. Consumed by
    /// `resolve_prepare_remedy`.
    RemedyHerb,
}

impl ResourceKind {
    /// Classify a herb kind into the coarser `ResourceKind` reserve bucket,
    /// or `None` if the herb is not tracked by the reserves substrate.
    pub fn from_herb_kind(kind: HerbKind) -> Option<Self> {
        match kind {
            HerbKind::Thornbriar => Some(Self::Thornbriar),
            HerbKind::HealingMoss | HerbKind::Moonpetal | HerbKind::Calmroot => {
                Some(Self::RemedyHerb)
            }
            _ => None,
        }
    }

    /// Classify a world `ItemKind` into a reserve bucket. Used by the
    /// `ColonyReserves` aggregator when summing herbs across cat inventories
    /// and Stores building contents.
    pub fn from_item_kind(kind: crate::components::items::ItemKind) -> Option<Self> {
        use crate::components::items::ItemKind;
        match kind {
            ItemKind::HerbThornbriar => Some(Self::Thornbriar),
            ItemKind::HerbHealingMoss | ItemKind::HerbMoonpetal | ItemKind::HerbCalmroot => {
                Some(Self::RemedyHerb)
            }
            _ => None,
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

/// A slot in a cat's inventory. The 5-slot pool is unified: any slot
/// can hold any `ItemKind`, herbs included (the `ItemKind` enum has herb
/// variants — `HerbHealingMoss` etc. — that map 1:1 to `HerbKind`).
/// Ticket 231 collapsed the prior `enum ItemSlot { Herb, Item }` split,
/// which created a representational asymmetry but no semantic difference
/// (`Inventory::is_full()` was always variant-agnostic).
///
/// Ticket 367 Commit 4b added `quality`. Picked-up items propagate
/// their source `Item.quality` through the slot (RimWorld/Factorio-
/// style: input quality is the substrate for output quality at any
/// downstream craft station). Pre-4b callers (hunt drops, remedy
/// preparation, herb gather, etc.) default to `1.0` via
/// `ItemSlot::new`. The `#[serde(default = "default_one_quality")]`
/// attribute keeps pre-4b save-game JSON deserializable.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemSlot {
    pub kind: crate::components::items::ItemKind,
    pub modifiers: crate::components::items::ItemModifiers,
    /// `[0.0, 1.0]`. Default `1.0` ("unknown / full quality") when
    /// the upstream caller doesn't track per-instance quality. The
    /// cat-picks-up-from-ground path (`resolve_pick_up`) captures
    /// the source `Item.quality`; downstream crafting reads it via
    /// the load resolver and combines it with crafter skill at the
    /// output-spawn site (see `tend_smoking_rack.rs` and the
    /// `preservation` system).
    #[serde(default = "default_one_quality")]
    pub quality: f32,
}

fn default_one_quality() -> f32 {
    1.0
}

impl ItemSlot {
    /// Construct a slot with default quality (`1.0`). Pre-367-4b
    /// API surface — every existing caller (hunt drops, prepared
    /// remedies, herb gather, magic) keeps working without explicit
    /// quality plumbing. New callers that *do* track quality should
    /// use `ItemSlot::with_quality` instead.
    pub fn new(
        kind: crate::components::items::ItemKind,
        modifiers: crate::components::items::ItemModifiers,
    ) -> Self {
        Self {
            kind,
            modifiers,
            quality: 1.0,
        }
    }

    /// Construct a slot with explicit quality. Used by the
    /// `resolve_pick_up` path (367-4b) to propagate the source
    /// `Item.quality` into the inventory representation. Clamps to
    /// `[0.0, 1.0]` matching the `Item::quality` invariant.
    pub fn with_quality(
        kind: crate::components::items::ItemKind,
        quality: f32,
        modifiers: crate::components::items::ItemModifiers,
    ) -> Self {
        Self {
            kind,
            modifiers,
            quality: quality.clamp(0.0, 1.0),
        }
    }

    pub fn herb(kind: HerbKind) -> Self {
        Self {
            kind: kind.to_item_kind(),
            modifiers: crate::components::items::ItemModifiers::default(),
            quality: 1.0,
        }
    }
}

/// A cat's carry bag (pouch) — the OSRS-style "backpack" half of the
/// slot-inventory model (ticket 017). Holds consumables (herbs, food,
/// remedies, materials, curios, craft inputs) and any *unworn* / carried
/// items. Worn gear lives in the sibling [`WearableSlots`] component
/// (`src/components/equipment.rs`), and `equipment_modifiers_for` reads
/// only those equipped slots — presence in the pouch no longer counts as
/// "worn." Capacity-limited via `pouch_capacity`.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    #[serde(alias = "slots")]
    pub pouch: Vec<ItemSlot>,
    /// Max items the pouch holds. Defaults to [`Inventory::MAX_SLOTS`];
    /// a future Crafted Bag (370/Phase 3) raises it via `bag_capacity_bonus`.
    #[serde(default = "default_pouch_capacity")]
    pub pouch_capacity: u16,
}

fn default_pouch_capacity() -> u16 {
    Inventory::MAX_SLOTS as u16
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            pouch: Vec::new(),
            pouch_capacity: Self::MAX_SLOTS as u16,
        }
    }
}

impl Inventory {
    pub const MAX_SLOTS: usize = 5;

    pub fn is_full(&self) -> bool {
        self.pouch.len() >= self.pouch_capacity as usize
    }

    // --- Herb compatibility methods ---

    pub fn has_herb(&self, kind: HerbKind) -> bool {
        let target = kind.to_item_kind();
        self.pouch.iter().any(|s| s.kind == target)
    }

    /// Remove one instance of `kind` from inventory. Returns true if found.
    pub fn take_herb(&mut self, kind: HerbKind) -> bool {
        let target = kind.to_item_kind();
        if let Some(idx) = self.pouch.iter().position(|s| s.kind == target) {
            self.pouch.swap_remove(idx);
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
        self.pouch.push(ItemSlot::herb(kind));
        true
    }

    /// Whether the inventory contains any herb at all.
    pub fn has_any_herb(&self) -> bool {
        self.pouch.iter().any(|s| s.kind.is_herb())
    }

    /// Whether the inventory has any herb usable for a remedy.
    pub fn has_remedy_herb(&self) -> bool {
        use crate::components::items::ItemKind;
        self.pouch.iter().any(|s| {
            matches!(
                s.kind,
                ItemKind::HerbHealingMoss | ItemKind::HerbMoonpetal | ItemKind::HerbCalmroot
            )
        })
    }

    /// Whether the inventory has thornbriar for ward-setting.
    pub fn has_ward_herb(&self) -> bool {
        self.has_herb(HerbKind::Thornbriar)
    }

    /// Whether the inventory contains any build material (Wood, Stone,
    /// Moss, DriedGrass, Feather, ShadowBone). Mirrors `has_any_herb`'s
    /// shape; reads `ItemKind::category()` for a single source of truth.
    /// Ticket 235.
    pub fn has_any_material(&self) -> bool {
        use crate::components::items::ItemCategory;
        self.pouch
            .iter()
            .any(|s| s.kind.category() == ItemCategory::Material)
    }

    /// Whether the inventory contains any curio (ShinyPebble, GlassShard,
    /// ColorfulShell). Ticket 235.
    pub fn has_any_curio(&self) -> bool {
        use crate::components::items::ItemCategory;
        self.pouch
            .iter()
            .any(|s| s.kind.category() == ItemCategory::Curiosity)
    }

    /// 450: whether the inventory contains *any* food item (raw, cooked,
    /// preserved). Reader: `HasFoodInInventory` writer in
    /// `items::update_inventory_markers`. Consulted by the HTN method
    /// `[BegForFood]`'s `ApplicableWhen` and (429 Phase 2) the
    /// `EatFromOwnInventoryDse` eligibility filter. Distinct from the
    /// narrower preservation-input markers (`has_raw_fish`,
    /// `has_raw_meat`, …) which gate the drying/smoking chains on
    /// specific raw inputs.
    pub fn has_food(&self) -> bool {
        self.pouch.iter().any(|s| s.kind.is_food())
    }

    /// 367: whether the inventory contains at least one Raw Fish.
    /// Reader: `HasRawFishInInventory` writer in
    /// `items::update_inventory_markers`.
    pub fn has_raw_fish(&self) -> bool {
        use crate::components::items::ItemKind;
        self.pouch.iter().any(|s| s.kind == ItemKind::RawFish)
    }

    /// 367: whether the inventory contains at least one Raw Organ.
    /// Reader: `HasRawOrganInInventory` writer.
    pub fn has_raw_organ(&self) -> bool {
        use crate::components::items::ItemKind;
        self.pouch.iter().any(|s| s.kind == ItemKind::RawOrgan)
    }

    /// 367: whether the inventory contains at least one Raw Meat
    /// (mammals + birds — fish goes through drying, not smoking).
    /// Reader: `HasRawMeatInInventory` writer.
    pub fn has_raw_meat(&self) -> bool {
        self.pouch.iter().any(|s| s.kind.is_raw_meat())
    }

    /// 367: whether the inventory contains at least one Fuel item
    /// (currently only `ItemKind::Wood`). Semantic-narrower sibling of
    /// `has_any_material` — Stone is a build material but not a fuel.
    /// Reader: `HasFuelInInventory` writer.
    pub fn has_fuel(&self) -> bool {
        self.pouch.iter().any(|s| s.kind.is_fuel())
    }

    /// 457: whether the inventory contains at least one Phase 2 Workshop
    /// recipe input — Phase 2 (368): `Twig`, `Bristle`, `Fiber`,
    /// `Flower`, `Stone`, `Feather`, `PolishedStone`; Phase 2b (369):
    /// `Bone`, `Sinew`, `Whisker`, `Hide`. Reader:
    /// `HasCraftInputInInventory` writer in
    /// `items::update_inventory_markers`. Recipe-agnostic: the marker
    /// fires when ANY craft input is present, and the resolver
    /// (`resolve_craft_at_workshop` or `resolve_craft_at_tanning_frame`)
    /// picks the specific recipe whose full input set is satisfied at
    /// execute time.
    pub fn has_craft_input(&self) -> bool {
        use crate::components::items::ItemKind;
        self.pouch.iter().any(|s| {
            matches!(
                s.kind,
                // 368 Phase 2 inputs.
                ItemKind::Twig
                    | ItemKind::Bristle
                    | ItemKind::Fiber
                    | ItemKind::Flower
                    | ItemKind::Stone
                    | ItemKind::Feather
                    | ItemKind::PolishedStone
                    // 369 Phase 2b inputs (prey byproducts).
                    | ItemKind::Bone
                    | ItemKind::Sinew
                    | ItemKind::Whisker
                    | ItemKind::Hide,
            )
        })
    }

    /// Whether the inventory holds a specific prepared remedy.
    /// Ticket 365 — Phase 1a real-items migration.
    pub fn has_remedy(&self, kind: RemedyKind) -> bool {
        let target = kind.to_item_kind();
        self.pouch.iter().any(|s| s.kind == target)
    }

    /// Remove one instance of a prepared remedy. Returns true
    /// if found.
    pub fn take_remedy(&mut self, kind: RemedyKind) -> bool {
        let target = kind.to_item_kind();
        if let Some(idx) = self.pouch.iter().position(|s| s.kind == target) {
            self.pouch.swap_remove(idx);
            true
        } else {
            false
        }
    }

    /// Return the first remedy kind that can be prepared from current herbs.
    pub fn first_remedy_kind(&self) -> Option<RemedyKind> {
        use crate::components::items::ItemKind;
        for slot in &self.pouch {
            match slot.kind {
                ItemKind::HerbHealingMoss => return Some(RemedyKind::HealingPoultice),
                ItemKind::HerbMoonpetal => return Some(RemedyKind::EnergyTonic),
                ItemKind::HerbCalmroot => return Some(RemedyKind::MoodTonic),
                _ => {}
            }
        }
        None
    }

    // --- Item methods ---

    pub fn has_item(&self, kind: crate::components::items::ItemKind) -> bool {
        self.pouch.iter().any(|s| s.kind == kind)
    }

    /// Add an item with default (clean) modifiers. Returns false if inventory is full.
    pub fn add_item(&mut self, kind: crate::components::items::ItemKind) -> bool {
        self.add_item_with_modifiers(kind, crate::components::items::ItemModifiers::default())
    }

    /// Add an item with explicit modifiers (default quality `1.0`).
    /// Returns false if inventory is full.
    pub fn add_item_with_modifiers(
        &mut self,
        kind: crate::components::items::ItemKind,
        modifiers: crate::components::items::ItemModifiers,
    ) -> bool {
        if self.is_full() {
            return false;
        }
        self.pouch.push(ItemSlot::new(kind, modifiers));
        true
    }

    /// 367-4b: add an item with full provenance (explicit quality +
    /// modifiers). The cat-picks-up-from-ground path uses this so the
    /// source `Item.quality` rides into inventory and onward into any
    /// downstream craft station. Returns false if inventory is full.
    pub fn add_item_with_quality(
        &mut self,
        kind: crate::components::items::ItemKind,
        quality: f32,
        modifiers: crate::components::items::ItemModifiers,
    ) -> bool {
        if self.is_full() {
            return false;
        }
        self.pouch
            .push(ItemSlot::with_quality(kind, quality, modifiers));
        true
    }

    /// Remove one instance of `kind` from inventory. Returns true if found.
    pub fn take_item(&mut self, kind: crate::components::items::ItemKind) -> bool {
        if let Some(idx) = self.pouch.iter().position(|s| s.kind == kind) {
            self.pouch.swap_remove(idx);
            true
        } else {
            false
        }
    }

    /// Take the first food item, returning its kind and modifiers.
    pub fn take_food(
        &mut self,
    ) -> Option<(
        crate::components::items::ItemKind,
        crate::components::items::ItemModifiers,
    )> {
        let idx = self.pouch.iter().position(|s| s.kind.is_food())?;
        let slot = self.pouch.remove(idx);
        Some((slot.kind, slot.modifiers))
    }

    /// Number of food slots currently held. Mirrors the predicate
    /// `take_food` uses (`ItemKind::is_food`); 178 reads this for the
    /// per-cat `inventory_excess` scoring axis.
    pub fn food_count(&self) -> usize {
        self.pouch.iter().filter(|s| s.kind.is_food()).count()
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
            decay_rate: 0.002,
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
    /// Durable wards project a wider aura than thornwards, reflecting the
    /// deeper magical anchor of the spell.
    pub fn repel_radius(&self) -> f32 {
        let base = match self.kind {
            WardKind::Thornward => 6.0,
            WardKind::DurableWard => 9.0,
        };
        base * self.strength
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

    /// The `ItemKind` slot a prepared remedy occupies in
    /// inventory. Ticket 365 — prepared remedies are real inventory
    /// items (Phase 1a substrate), not a search-state-only virtual
    /// carry. Symmetric with `HerbKind::to_item_kind`.
    pub fn to_item_kind(self) -> crate::components::items::ItemKind {
        use crate::components::items::ItemKind;
        match self {
            Self::HealingPoultice => ItemKind::RemedyHealingPoultice,
            Self::EnergyTonic => ItemKind::RemedyEnergyTonic,
            Self::MoodTonic => ItemKind::RemedyMoodTonic,
        }
    }

    /// Recipe id (ticket 365 — 016 Phase 1a). One recipe per
    /// remedy. Used by `resolve_prepare_remedy` to attach a
    /// `CraftedItem` provenance record to inventory and by
    /// `populate_recipe_registry` to register the catalog entry.
    pub fn recipe_id(self) -> crate::components::recipe::RecipeId {
        use crate::components::recipe::RecipeId;
        match self {
            Self::HealingPoultice => RecipeId("remedy.healing_poultice"),
            Self::EnergyTonic => RecipeId("remedy.energy_tonic"),
            Self::MoodTonic => RecipeId("remedy.mood_tonic"),
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
///
/// Ticket 471 rename: this was `MisfireEffect`. The bare enum now lives at
/// `MisfireEffectKind` because the Message type (which carries the
/// per-event identity + position + tick) is named `MisfireEffect` and
/// references this enum as its `kind` discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MisfireEffectKind {
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
    fn inventory_food_count() {
        use crate::components::items::ItemKind;
        let mut inv = Inventory::default();
        assert_eq!(inv.food_count(), 0);
        inv.add_item(ItemKind::RawMouse);
        inv.add_item(ItemKind::RawRat);
        assert_eq!(inv.food_count(), 2);
        inv.add_herb(HerbKind::HealingMoss);
        assert_eq!(inv.food_count(), 2, "herbs are not food");
        inv.take_food();
        assert_eq!(inv.food_count(), 1);
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
