use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::items::Item;
use crate::components::magic::HerbKind;
use crate::components::physical::Position;
use crate::components::task_chain::{FailurePolicy, Material, StepKind, TaskChain, TaskStep};

/// Decorative marker for the colony well entity at the colony center.
#[derive(Component)]
pub struct ColonyWell;

// ---------------------------------------------------------------------------
// StructureType
// ---------------------------------------------------------------------------

/// The kind of building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StructureType {
    Den,
    Hearth,
    /// Cooking workstation — cats bring raw food here, transform it into
    /// cooked items, and return it to Stores. Cooked items grant a
    /// hunger-restoration multiplier when eaten (`cooked_food_multiplier`).
    Kitchen,
    Stores,
    Workshop,
    Garden,
    Watchtower,
    WardPost,
    Wall,
    Gate,
    /// 176: refuse pile. Cats carry inventory items they can't usefully
    /// deposit here; the building's `StoredItems` has unlimited capacity
    /// so deposits never fail on capacity grounds. Distinct from Stores
    /// because retrievals are conceptually disallowed (no
    /// `RetrieveFromMidden` chain — items go to die here, even if they
    /// remain real entities). Future scope: items at the midden decay
    /// faster via the existing rot ecology.
    Midden,
    /// 367: open-air drying rack. Sun-powered, weather-sensitive. Cats
    /// load raw fish or raw organ + herb here; per-tick advance only
    /// when `Weather::Clear`. Output ItemKind (DriedFish / PreservedOrgan)
    /// spawns onto the rack tile when progress reaches 1.0.
    /// State lives on a sibling `DryingRackState` Component.
    DryingRack,
    /// 367: covered smoking rack. Requires raw meat + 1 fuel load;
    /// progress advances only via discrete tend-cycles (3 visits per
    /// craft), with a per-rack cooldown that forces interleaving with
    /// other actions. State lives on a sibling `SmokingRackState`
    /// Component.
    SmokingRack,
    /// 369: tanning frame for cured-hide work. Hosts the Hide
    /// Bracers / Hide-Plated Wrap recipes (016 Phase 2b). Single-pass
    /// craft (no per-rack progress state) — the recipe duration is a
    /// `RecipeDuration::AtStationFaster` like the Workshop entries,
    /// resolved by `resolve_craft_at_tanning_frame` (mirror of
    /// `resolve_craft_at_workshop`). Construction is light wood +
    /// stake-driven hide stretching; cheaper than the Smoking Rack
    /// because there's no flame containment.
    TanningFrame,
}

impl StructureType {
    /// Default material cost for constructing this structure.
    pub fn material_cost(self) -> Vec<(Material, u32)> {
        match self {
            Self::Den => vec![(Material::Wood, 10), (Material::Stone, 6)],
            Self::Hearth => vec![(Material::Stone, 12), (Material::Wood, 5)],
            Self::Kitchen => vec![(Material::Stone, 6), (Material::Wood, 6)],
            Self::Stores => vec![(Material::Wood, 10), (Material::Stone, 5)],
            Self::Workshop => vec![
                (Material::Wood, 7),
                (Material::Stone, 4),
                (Material::Herbs, 3),
            ],
            Self::Garden => vec![(Material::Wood, 6)],
            Self::Watchtower => vec![(Material::Wood, 8), (Material::Stone, 8)],
            Self::WardPost => vec![(Material::Stone, 2), (Material::Herbs, 3)],
            Self::Wall => vec![(Material::Stone, 3)],
            Self::Gate => vec![(Material::Wood, 4), (Material::Stone, 2)],
            // 176: Midden is a refuse pile, not a built-up structure;
            // a single load of wood marks out the spot. Cheap so the
            // colony-founding wagon-dismantle haul can fund it without
            // blocking other infrastructure.
            Self::Midden => vec![(Material::Wood, 1)],
            // 367: Drying Rack is a light wood frame in open ground.
            // No stone — preservation rack, not flame-handling. Cheap
            // enough that a labor-flush founder colony can stand one
            // up alongside the kitchen.
            Self::DryingRack => vec![(Material::Wood, 5)],
            // 367: Smoking Rack handles smoldering fuel — wants stone
            // base for flame containment plus wood for the rack frame.
            // Pricier than the drying rack to match the multi-cycle
            // labor cost of tending it.
            Self::SmokingRack => vec![(Material::Stone, 3), (Material::Wood, 4)],
            // 369: Tanning Frame is a light wood frame with stake-
            // driven hide stretching. No flame containment, so
            // cheaper than the Smoking Rack; slightly heavier than
            // the Drying Rack because the stakes need to be ground-
            // anchored. The sinew lashing the hides to the frame is
            // implicit (cat labor, not a delivered material).
            Self::TanningFrame => vec![(Material::Wood, 4)],
        }
    }

    /// Default size in tiles (width, height).
    pub fn default_size(self) -> (i32, i32) {
        match self {
            Self::Den | Self::Workshop | Self::Kitchen => (3, 3),
            Self::Hearth | Self::Stores => (4, 3),
            Self::Garden => (6, 5),
            Self::Watchtower => (2, 3),
            Self::Gate => (2, 1),
            Self::WardPost | Self::Wall => (1, 1),
            Self::Midden => (2, 2),
            // 367: both preservation stations occupy a 2×2 footprint —
            // a rack frame and a workspace tile alongside it.
            // 369: Tanning Frame uses the same 2×2 footprint pattern —
            // the frame + working tile for the cat stretching hide.
            Self::DryingRack | Self::SmokingRack | Self::TanningFrame => (2, 2),
        }
    }

    /// The `Terrain` tile type used to render this building's footprint.
    pub fn terrain(self) -> crate::resources::map::Terrain {
        use crate::resources::map::Terrain;
        match self {
            Self::Den => Terrain::Den,
            Self::Hearth => Terrain::Hearth,
            Self::Kitchen => Terrain::Kitchen,
            Self::Stores => Terrain::Stores,
            Self::Workshop => Terrain::Workshop,
            Self::Garden => Terrain::Garden,
            Self::Watchtower => Terrain::Watchtower,
            Self::WardPost => Terrain::WardPost,
            Self::Wall => Terrain::Wall,
            Self::Gate => Terrain::Gate,
            // 176: Midden visually reuses the Stores terrain for
            // stage-1 atomicity — adding a dedicated Terrain variant
            // requires also updating the autotile / palette pipeline,
            // which is out of scope here. Worldgen places the Midden
            // away from Stores so the rendering overlap is minimal.
            // A future visual-polish ticket can add `Terrain::Midden`.
            Self::Midden => Terrain::Stores,
            // 367: same stage-1 reuse precedent as Midden — preservation
            // stations borrow Workshop / Hearth terrain visually until a
            // dedicated autotile + palette pipeline ticket lands.
            // Drying Rack reads as a workshop (open frame structure);
            // Smoking Rack reads as a hearth (manages combustion).
            Self::DryingRack => Terrain::Workshop,
            Self::SmokingRack => Terrain::Hearth,
            // 369: Tanning Frame reuses Workshop terrain visually
            // (same open-frame structure idiom as Drying Rack); a
            // dedicated Terrain variant + autotile pass is a
            // visual-polish follow-on, not a substrate concern.
            Self::TanningFrame => Terrain::Workshop,
        }
    }

    /// Generate a `TaskChain` for constructing this structure at the given position.
    ///
    /// The chain gathers each required material (move to resource, gather, move
    /// back, deliver), then constructs. The `site_entity` should be the
    /// `ConstructionSite` entity.
    pub fn build_chain(
        self,
        site_pos: Position,
        site_entity: Entity,
        resource_positions: &[(Material, Position)],
    ) -> TaskChain {
        let mut steps = Vec::new();

        for (material, amount) in self.material_cost() {
            // Find nearest resource position for this material
            let resource_pos = resource_positions
                .iter()
                .find(|(m, _)| *m == material)
                .map(|(_, pos)| *pos);

            if let Some(rpos) = resource_pos {
                // Move to resource
                steps.push(TaskStep::new(StepKind::MoveTo).with_position(rpos));
                // Gather
                steps
                    .push(TaskStep::new(StepKind::Gather { material, amount }).with_position(rpos));
                // Move back to construction site
                steps.push(TaskStep::new(StepKind::MoveTo).with_position(site_pos));
                // Deliver
                steps.push(
                    TaskStep::new(StepKind::Deliver { material, amount }).with_entity(site_entity),
                );
            }
        }

        // Final construction step
        steps.push(
            TaskStep::new(StepKind::Construct)
                .with_position(site_pos)
                .with_entity(site_entity),
        );

        TaskChain::new(steps, FailurePolicy::AbortChain)
    }

    /// Generate a repair chain for a damaged building.
    pub fn repair_chain(building_pos: Position, building_entity: Entity) -> TaskChain {
        TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(building_pos),
                TaskStep::new(StepKind::Repair).with_entity(building_entity),
            ],
            FailurePolicy::AbortChain,
        )
    }

    /// Generate a farming chain for a garden.
    pub fn farm_chain(garden_pos: Position, garden_entity: Entity) -> TaskChain {
        TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(garden_pos),
                TaskStep::new(StepKind::Tend)
                    .with_position(garden_pos)
                    .with_entity(garden_entity),
                TaskStep::new(StepKind::Harvest)
                    .with_position(garden_pos)
                    .with_entity(garden_entity),
            ],
            FailurePolicy::AbortChain,
        )
    }
}

// ---------------------------------------------------------------------------
// Structure component
// ---------------------------------------------------------------------------

fn default_cleanliness() -> f32 {
    1.0
}

/// A completed (or decaying) building in the world.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Structure {
    pub kind: StructureType,
    /// Structural integrity: 1.0 = pristine, 0.0 = ruins.
    pub condition: f32,
    /// Cleanliness: 1.0 = tidy, 0.0 = filthy.
    #[serde(default = "default_cleanliness")]
    pub cleanliness: f32,
    /// Tile footprint.
    pub size: (i32, i32),
}

impl Structure {
    pub fn new(kind: StructureType) -> Self {
        Self {
            kind,
            condition: 1.0,
            cleanliness: 1.0,
            size: kind.default_size(),
        }
    }

    /// Center tile position given the building's anchor (top-left) position.
    pub fn center(&self, anchor: &Position) -> Position {
        Position::new(anchor.x + self.size.0 / 2, anchor.y + self.size.1 / 2)
    }

    /// Effectiveness multiplier based on condition.
    ///
    /// - condition > 0.5 → 1.0 (full effect)
    /// - 0.2 < condition ≤ 0.5 → linear falloff
    /// - condition ≤ 0.2 → 0.0 (non-functional)
    pub fn effectiveness(&self) -> f32 {
        if self.condition > 0.5 {
            1.0
        } else if self.condition > 0.2 {
            (self.condition - 0.2) / 0.3
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// ConstructionSite component
// ---------------------------------------------------------------------------

/// Marks an entity as an in-progress construction project.
///
/// Removed when construction completes (progress reaches 1.0), at which point
/// the entity gets a `Structure` component with full condition.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConstructionSite {
    pub blueprint: StructureType,
    pub progress: f32,
    pub materials_needed: Vec<(Material, u32)>,
    pub materials_delivered: Vec<(Material, u32)>,
}

impl ConstructionSite {
    pub fn new(blueprint: StructureType) -> Self {
        let materials_needed = blueprint.material_cost();
        let materials_delivered = materials_needed.iter().map(|(m, _)| (*m, 0u32)).collect();
        Self {
            blueprint,
            progress: 0.0,
            materials_needed,
            materials_delivered,
        }
    }

    /// Create a construction site with all materials already delivered.
    ///
    /// Used for founding buildings where the colony pools resources they
    /// brought with them (analogous to Dwarf Fortress wagon disassembly).
    pub fn new_prefunded(blueprint: StructureType) -> Self {
        let materials_needed = blueprint.material_cost();
        let materials_delivered = materials_needed.clone();
        Self {
            blueprint,
            progress: 0.0,
            materials_needed,
            materials_delivered,
        }
    }

    /// Construct a site with a custom (non-blueprint-default) materials
    /// requirement. Used by the founding wagon-dismantling spawn (ticket
    /// 038) — the founding act ships a smaller-than-default materials
    /// load so cats can finish hauling in the first few in-game days
    /// without starving while the long-term build economy comes online.
    pub fn new_with_custom_cost(
        blueprint: StructureType,
        materials_needed: Vec<(Material, u32)>,
    ) -> Self {
        let materials_delivered = materials_needed.iter().map(|(m, _)| (*m, 0u32)).collect();
        Self {
            blueprint,
            progress: 0.0,
            materials_needed,
            materials_delivered,
        }
    }

    /// Whether all required materials have been delivered.
    pub fn materials_complete(&self) -> bool {
        self.materials_needed
            .iter()
            .zip(self.materials_delivered.iter())
            .all(|((_, needed), (_, delivered))| delivered >= needed)
    }

    /// Deliver materials of the given type, clamping to what's needed.
    pub fn deliver(&mut self, material: Material, amount: u32) {
        for ((mat_needed, qty_needed), (_, qty_delivered)) in self
            .materials_needed
            .iter()
            .zip(self.materials_delivered.iter_mut())
        {
            if *mat_needed == material {
                *qty_delivered = (*qty_delivered + amount).min(*qty_needed);
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CropState component
// ---------------------------------------------------------------------------

/// What kind of crop a garden is growing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CropKind {
    /// Standard food crops: produces Berries + Roots.
    #[default]
    FoodCrops,
    /// Thornbriar herb: produces a harvestable Thornbriar entity.
    Thornbriar,
}

/// Tracks crop growth on a `Garden` building entity.
#[derive(Component, Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct CropState {
    /// Growth progress: 0.0 (just planted) → 1.0 (ready to harvest).
    pub growth: f32,
    /// What kind of crop is being grown.
    pub crop_kind: CropKind,
}

// ---------------------------------------------------------------------------
// StoredItems component
// ---------------------------------------------------------------------------

/// Tracks items stored inside a building. Capacity depends on building type.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredItems {
    #[serde(skip, default)]
    pub items: Vec<Entity>,
}

impl StoredItems {
    /// Maximum number of items this building type can hold.
    ///
    /// 176: `Midden` is unlimited — refuse piles can grow indefinitely.
    /// Cats trash overflow here; items at the midden are still real
    /// entities (items-are-real invariant) but conceptually go to die.
    pub fn capacity(kind: StructureType) -> usize {
        match kind {
            StructureType::Stores => 50,
            StructureType::Den => 8,
            StructureType::Workshop => 15,
            StructureType::Midden => usize::MAX,
            _ => 0,
        }
    }

    /// Whether this building is at capacity.
    pub fn is_full(&self, kind: StructureType) -> bool {
        self.items.len() >= Self::capacity(kind)
    }

    /// Attempt to add an item. Returns `false` if at capacity.
    pub fn add(&mut self, item: Entity, kind: StructureType) -> bool {
        if self.is_full(kind) {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Effective capacity accounting for storage-upgrade items in the Vec.
    /// Requires an `Item` query to inspect stored items for `capacity_bonus()`.
    pub fn effective_capacity_with_items(
        kind: StructureType,
        stored: &[Entity],
        items_q: &Query<
            &Item,
            bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
        >,
    ) -> usize {
        let base = Self::capacity(kind);
        let bonus: usize = stored
            .iter()
            .filter_map(|&e| items_q.get(e).ok())
            .map(|item| item.kind.capacity_bonus())
            .sum();
        base + bonus
    }

    /// Whether this building is at effective capacity (accounting for storage upgrades).
    pub fn is_effectively_full(
        &self,
        kind: StructureType,
        items_q: &Query<
            &Item,
            bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
        >,
    ) -> bool {
        self.items.len() >= Self::effective_capacity_with_items(kind, &self.items, items_q)
    }

    /// Attempt to add an item, using effective capacity. Returns `false` if full.
    pub fn add_effective(
        &mut self,
        item: Entity,
        kind: StructureType,
        items_q: &Query<
            &Item,
            bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
        >,
    ) -> bool {
        if self.is_effectively_full(kind, items_q) {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Remove an item by entity. Returns `false` if not found.
    pub fn remove(&mut self, item: Entity) -> bool {
        if let Some(pos) = self.items.iter().position(|&e| e == item) {
            self.items.swap_remove(pos);
            true
        } else {
            false
        }
    }
}

/// Ticket 084: per-Stores aggregate count of stashed herbs, keyed by
/// `HerbKind`. Sibling to `StoredItems` (food/material Entities), but
/// herbs stash as a lightweight count rather than spawned Item entities
/// — matches the existing `Inventory.slots` herb representation, where
/// herb slots carry no Entity identity and no per-instance modifiers.
/// Capacity is per-kind and provided by the caller (sourced from
/// `ScoringConstants::stores_herb_capacity_per_kind`).
///
/// Lifecycle:
/// - Inserted on every `StructureType::Stores` at construction
///   (`steps/building/construct.rs`).
/// - Mutated by `resolve_deposit_herbs_to_stores` (add) and
///   `resolve_retrieve_herbs_from_stores(kind)` (take).
/// - Aggregated by `update_colony_building_markers` to author
///   `HasStoredThornbriar` and (Commit 3) `ColonyThornbriarChronicallyLow`.
#[derive(Component, Debug, Clone, Default)]
pub struct StoredHerbs {
    pub counts: HashMap<HerbKind, u32>,
}

impl StoredHerbs {
    /// Count of stashed herbs of one kind.
    pub fn count(&self, kind: HerbKind) -> u32 {
        self.counts.get(&kind).copied().unwrap_or(0)
    }

    /// Attempt to add `n` herbs of `kind`, capped at `capacity_per_kind`.
    /// Returns the number actually added (0 if already at cap, `n` if
    /// fully absorbed, partial otherwise — items-are-real discipline
    /// applies: the caller MUST retain the un-added remainder rather
    /// than silently destroying it).
    pub fn add(&mut self, kind: HerbKind, n: u32, capacity_per_kind: u32) -> u32 {
        let current = self.count(kind);
        let room = capacity_per_kind.saturating_sub(current);
        let added = n.min(room);
        if added > 0 {
            *self.counts.entry(kind).or_insert(0) += added;
        }
        added
    }

    /// Remove one herb of `kind`. Returns true if one was present.
    pub fn take(&mut self, kind: HerbKind) -> bool {
        let entry = self.counts.entry(kind).or_insert(0);
        if *entry == 0 {
            return false;
        }
        *entry -= 1;
        true
    }
}

// ---------------------------------------------------------------------------
// GateState component
// ---------------------------------------------------------------------------

/// Tracks whether a gate is open or closed.
///
/// Open gates allow wildlife through. Cats can always pass regardless of state,
/// but personality (diligence) determines whether they close the gate behind them.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GateState {
    pub open: bool,
}

// ---------------------------------------------------------------------------
// Preservation station state (ticket 367 — 016 Phase 1b)
// ---------------------------------------------------------------------------

/// Which recipe is currently loaded on a Drying Rack. Determines the
/// output `ItemKind` when progress reaches 1.0.
///
/// Local enum (rather than a `RecipeId`) so the state Component is
/// serde-deserializable — `RecipeId` carries `&'static str` keys
/// that can't round-trip. The two-variant shape mirrors the two
/// Phase 1b recipes that share the Drying Rack station.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DryingRecipe {
    /// `preserve.dried_fish` — input RawFish, ~3 days Clear weather.
    #[default]
    DriedFish,
    /// `preserve.preserved_organ` — input RawOrgan + 1 herb, ~2 days.
    /// Herb is consumed from cat inventory at load time; quality and
    /// `from_organ` ride through to the output via `source_modifiers`.
    PreservedOrgan,
}

/// Per-load captured state for a Drying Rack. Copied off the source
/// item at load time; the source `Item` entity is despawned in the same
/// tick (matches the precedent set by `eat_from_inventory` —
/// fungible-grade consumption rather than a Source/Transfer/Sink
/// routed step, see ticket 429).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DryingLoad {
    pub recipe: DryingRecipe,
    /// Source item's quality at load time (RimWorld/Factorio-style
    /// substrate for the output). Combined with `crafter_skill` via
    /// `CraftingConstants::preservation_quality_*` at output-spawn
    /// time to produce the final `Item::quality`.
    pub source_quality: f32,
    /// 367-4b: loader's normalised crafter skill at load time. For
    /// drying, the loader is the substrate-correct "crafter" — sun
    /// does the rest of the work, no per-tend cat exists. Combined
    /// with `source_quality` at output-spawn time. Default 0.4
    /// (matches `CraftingConstants::preservation_skill_baseline`)
    /// for serde back-compat on pre-4b save data.
    #[serde(default = "default_crafter_skill")]
    pub crafter_skill: f32,
    /// Corruption + `from_organ` ride through to the output's modifiers.
    pub source_modifiers: crate::components::items::ItemModifiers,
}

fn default_crafter_skill() -> f32 {
    0.4
}

/// Per-Drying-Rack runtime state (ticket 367). Inserted on
/// construction completion (see `src/steps/building/construct.rs`).
/// `progress` advances per tick by `preservation` system only when
/// `Weather::Clear`; output `Item` spawns on completion.
#[derive(Component, Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DryingRackState {
    /// `None` when idle. `Some(_)` after a cat loads raw food (and
    /// the optional herb).
    pub loaded: Option<DryingLoad>,
    /// 0.0 at load time; 1.0 = ready to spawn output.
    pub progress: f32,
}

/// Per-load captured state for a Smoking Rack. Same Source/Transfer
/// caveats as `DryingLoad`. `fuel_loaded` is a boolean — wood is
/// fungible material with no per-instance provenance, so we don't
/// track a fuel entity handle (matches the construction-material
/// consumption pattern).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SmokingLoad {
    /// Which raw meat is smoking. Stored so the output recipe id can
    /// be reconstructed for `CraftedItem` provenance even though all
    /// four smoking recipes share `ItemKind::SmokedMeat` as their
    /// output.
    pub source_kind: crate::components::items::ItemKind,
    /// Source meat's quality at load time. See `DryingLoad::source_quality`.
    pub source_quality: f32,
    /// 367-4b: loader's normalised crafter skill at load time. Smoking
    /// has a per-tend cat (the cooking happens in discrete visits) so
    /// arguably the "last tender" is the substrate-correct crafter.
    /// 4b ships the simpler convention — the loader's skill carries
    /// through every tend — to avoid stamping skill on every visit.
    /// If gameplay observation says the closing tender should matter
    /// more, a follow-on can extend `SmokingRackState` with a
    /// `tend_skills: Vec<f32>` and weight the closing tend higher.
    #[serde(default = "default_crafter_skill")]
    pub crafter_skill: f32,
    pub source_modifiers: crate::components::items::ItemModifiers,
}

/// Per-Smoking-Rack runtime state (ticket 367). Inserted on
/// construction completion. Smoking progress does NOT advance per
/// tick — it's driven entirely by discrete tend-cycle completions.
///
/// Tend cycles are the novel substrate for 367: each tend (a
/// short, single-tick resolver) increments `tends_completed` and
/// advances `progress` by `1.0 / tends_needed`. The per-rack
/// `last_tended_at_tick` + `CraftingConstants::smoking_tend_cooldown_ticks`
/// cooldown forces interleaving — the cat must do something else for
/// ~2 sim-hours between tends, producing the "tend, walk away, come
/// back, tend, ..." rhythm the design doc calls for.
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SmokingRackState {
    pub loaded: Option<SmokingLoad>,
    /// True once a cat has burned a fuel load onto the rack. A loaded
    /// meat without burning fuel still needs a fuel-load action before
    /// tend cycles begin.
    pub fuel_loaded: bool,
    /// 0.0 at load time; 1.0 = ready to spawn `SmokedMeat`.
    pub progress: f32,
    /// Absolute tick of the most recent tend; gates the cooldown.
    /// `0` sentinel means "no tend yet this craft" (the rack-load
    /// resolver leaves this at 0 so the first tend can fire as soon
    /// as a cat reaches the rack).
    pub last_tended_at_tick: u64,
    /// How many tends have completed this craft.
    pub tends_completed: u32,
    /// Total tends required (defaulted to
    /// `CraftingConstants::smoking_tends_needed`, kept on the state so
    /// future recipes can declare a variant smoking duration).
    pub tends_needed: u32,
}

impl Default for SmokingRackState {
    fn default() -> Self {
        Self {
            loaded: None,
            fuel_loaded: false,
            progress: 0.0,
            last_tended_at_tick: 0,
            tends_completed: 0,
            // Default tends_needed — overridden at load time by the
            // CraftingConstants knob if it differs.
            tends_needed: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Structure::effectiveness ---

    #[test]
    fn effectiveness_full_above_half() {
        let s = Structure {
            kind: StructureType::Den,
            condition: 0.8,
            cleanliness: 1.0,
            size: StructureType::Den.default_size(),
        };
        assert_eq!(s.effectiveness(), 1.0);
    }

    #[test]
    fn effectiveness_at_half() {
        let s = Structure {
            kind: StructureType::Den,
            condition: 0.5,
            cleanliness: 1.0,
            size: StructureType::Den.default_size(),
        };
        assert_eq!(s.effectiveness(), 1.0);
    }

    #[test]
    fn effectiveness_linear_falloff() {
        let s = Structure {
            kind: StructureType::Den,
            condition: 0.35,
            cleanliness: 1.0,
            size: StructureType::Den.default_size(),
        };
        let expected = (0.35 - 0.2) / 0.3;
        assert!((s.effectiveness() - expected).abs() < 1e-6);
    }

    #[test]
    fn effectiveness_at_lower_bound() {
        let s = Structure {
            kind: StructureType::Den,
            condition: 0.2,
            cleanliness: 1.0,
            size: StructureType::Den.default_size(),
        };
        assert_eq!(s.effectiveness(), 0.0);
    }

    #[test]
    fn effectiveness_below_lower_bound() {
        let s = Structure {
            kind: StructureType::Den,
            condition: 0.1,
            cleanliness: 1.0,
            size: StructureType::Den.default_size(),
        };
        assert_eq!(s.effectiveness(), 0.0);
    }

    #[test]
    fn effectiveness_at_zero() {
        let s = Structure {
            kind: StructureType::Den,
            condition: 0.0,
            cleanliness: 1.0,
            size: StructureType::Den.default_size(),
        };
        assert_eq!(s.effectiveness(), 0.0);
    }

    #[test]
    fn effectiveness_pristine() {
        let s = Structure::new(StructureType::Hearth);
        assert_eq!(s.effectiveness(), 1.0);
        assert_eq!(s.condition, 1.0);
    }

    // --- StructureType::material_cost ---

    #[test]
    fn all_types_have_material_costs() {
        let types = [
            StructureType::Den,
            StructureType::Hearth,
            StructureType::Kitchen,
            StructureType::Stores,
            StructureType::Workshop,
            StructureType::Garden,
            StructureType::Watchtower,
            StructureType::WardPost,
            StructureType::Wall,
            StructureType::Gate,
            StructureType::Midden,
            StructureType::DryingRack,
            StructureType::SmokingRack,
        ];
        for kind in types {
            let cost = kind.material_cost();
            assert!(!cost.is_empty(), "{kind:?} should have material costs");
            for (_, amount) in &cost {
                assert!(*amount > 0, "{kind:?} has zero-amount material");
            }
        }
    }

    // --- ConstructionSite ---

    #[test]
    fn construction_site_starts_incomplete() {
        let site = ConstructionSite::new(StructureType::Den);
        assert!(!site.materials_complete());
        assert_eq!(site.progress, 0.0);
    }

    #[test]
    fn deliver_fills_materials() {
        let mut site = ConstructionSite::new(StructureType::Garden);
        // Garden needs Wood × 6
        assert!(!site.materials_complete());
        site.deliver(Material::Wood, 6);
        assert!(site.materials_complete());
    }

    #[test]
    fn deliver_clamps_to_needed() {
        let mut site = ConstructionSite::new(StructureType::Garden);
        site.deliver(Material::Wood, 100);
        assert_eq!(site.materials_delivered[0].1, 6); // only needed 6
    }

    // --- build_chain ---

    #[test]
    fn build_chain_has_correct_structure() {
        use bevy_ecs::world::World;
        let mut world = World::new();
        let site = world.spawn_empty().id();

        let chain = StructureType::Garden.build_chain(
            Position::new(5, 5),
            site,
            &[(Material::Wood, Position::new(10, 5))],
        );

        // Garden needs Wood × 2: MoveTo + Gather + MoveTo + Deliver + Construct = 5 steps
        assert_eq!(chain.steps.len(), 5);
        assert!(matches!(chain.steps[0].kind, StepKind::MoveTo));
        assert!(matches!(
            chain.steps[1].kind,
            StepKind::Gather {
                material: Material::Wood,
                ..
            }
        ));
        assert!(matches!(chain.steps[2].kind, StepKind::MoveTo));
        assert!(matches!(
            chain.steps[3].kind,
            StepKind::Deliver {
                material: Material::Wood,
                ..
            }
        ));
        assert!(matches!(chain.steps[4].kind, StepKind::Construct));
    }

    // --- GateState ---

    #[test]
    fn gate_defaults_closed() {
        let gate = GateState::default();
        assert!(!gate.open);
    }

    // --- StoredItems ---

    #[test]
    fn stores_has_capacity_50() {
        assert_eq!(StoredItems::capacity(StructureType::Stores), 50);
    }

    #[test]
    fn den_has_capacity_8() {
        assert_eq!(StoredItems::capacity(StructureType::Den), 8);
    }

    #[test]
    fn wall_has_no_storage() {
        assert_eq!(StoredItems::capacity(StructureType::Wall), 0);
    }

    #[test]
    fn add_respects_capacity() {
        use bevy_ecs::world::World;
        let mut world = World::new();
        let e = world.spawn_empty().id();

        // Wall has 0 capacity — add should fail immediately.
        let mut wall_storage = StoredItems::default();
        assert!(!wall_storage.add(e, StructureType::Wall));

        // Stores has 30 capacity — first add should succeed.
        let mut stores_storage = StoredItems::default();
        assert!(stores_storage.add(e, StructureType::Stores));
        assert_eq!(stores_storage.items.len(), 1);
    }

    #[test]
    fn remove_returns_false_for_missing() {
        use bevy_ecs::world::World;
        let mut world = World::new();
        let e = world.spawn_empty().id();
        let mut storage = StoredItems::default();
        assert!(!storage.remove(e));
    }

    // --- StoredHerbs (ticket 084) ---

    #[test]
    fn stored_herbs_round_trip() {
        let mut sh = StoredHerbs::default();
        assert_eq!(sh.count(HerbKind::Thornbriar), 0);
        assert_eq!(sh.add(HerbKind::Thornbriar, 3, 20), 3);
        assert_eq!(sh.count(HerbKind::Thornbriar), 3);
        assert!(sh.take(HerbKind::Thornbriar));
        assert_eq!(sh.count(HerbKind::Thornbriar), 2);
        assert!(sh.take(HerbKind::Thornbriar));
        assert!(sh.take(HerbKind::Thornbriar));
        assert_eq!(sh.count(HerbKind::Thornbriar), 0);
        assert!(!sh.take(HerbKind::Thornbriar));
    }

    #[test]
    fn stored_herbs_respects_capacity() {
        let mut sh = StoredHerbs::default();
        // Capacity 5, attempt to add 8 — accepts 5, signals 3 unabsorbed
        // remain in caller's possession (items-are-real).
        assert_eq!(sh.add(HerbKind::Thornbriar, 8, 5), 5);
        assert_eq!(sh.count(HerbKind::Thornbriar), 5);
        // Already at cap — further adds return 0.
        assert_eq!(sh.add(HerbKind::Thornbriar, 1, 5), 0);
        assert_eq!(sh.count(HerbKind::Thornbriar), 5);
    }

    #[test]
    fn stored_herbs_independent_per_kind() {
        let mut sh = StoredHerbs::default();
        sh.add(HerbKind::Thornbriar, 4, 20);
        sh.add(HerbKind::HealingMoss, 2, 20);
        assert_eq!(sh.count(HerbKind::Thornbriar), 4);
        assert_eq!(sh.count(HerbKind::HealingMoss), 2);
        // Taking Thornbriar doesn't affect HealingMoss.
        sh.take(HerbKind::Thornbriar);
        assert_eq!(sh.count(HerbKind::Thornbriar), 3);
        assert_eq!(sh.count(HerbKind::HealingMoss), 2);
    }
}
