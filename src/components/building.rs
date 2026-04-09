use bevy_ecs::prelude::*;

use crate::components::physical::Position;
use crate::components::task_chain::{
    FailurePolicy, Material, StepKind, TaskChain, TaskStep,
};

// ---------------------------------------------------------------------------
// StructureType
// ---------------------------------------------------------------------------

/// The kind of building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StructureType {
    Den,
    Hearth,
    Stores,
    Workshop,
    Garden,
    Watchtower,
    WardPost,
    Wall,
    Gate,
}

impl StructureType {
    /// Default material cost for constructing this structure.
    pub fn material_cost(self) -> Vec<(Material, u32)> {
        match self {
            Self::Den => vec![(Material::Wood, 10), (Material::Stone, 6)],
            Self::Hearth => vec![(Material::Stone, 12), (Material::Wood, 5)],
            Self::Stores => vec![(Material::Wood, 10), (Material::Stone, 5)],
            Self::Workshop => vec![(Material::Wood, 7), (Material::Stone, 4), (Material::Herbs, 3)],
            Self::Garden => vec![(Material::Wood, 6)],
            Self::Watchtower => vec![(Material::Wood, 8), (Material::Stone, 8)],
            Self::WardPost => vec![(Material::Stone, 2), (Material::Herbs, 3)],
            Self::Wall => vec![(Material::Stone, 3)],
            Self::Gate => vec![(Material::Wood, 4), (Material::Stone, 2)],
        }
    }

    /// Default size in tiles (width, height).
    pub fn default_size(self) -> (i32, i32) {
        match self {
            Self::Den | Self::Workshop => (3, 3),
            Self::Hearth | Self::Stores => (4, 3),
            Self::Garden => (6, 5),
            Self::Watchtower => (2, 3),
            Self::Gate => (2, 1),
            Self::WardPost | Self::Wall => (1, 1),
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
                steps.push(
                    TaskStep::new(StepKind::MoveTo).with_position(rpos),
                );
                // Gather
                steps.push(
                    TaskStep::new(StepKind::Gather { material, amount })
                        .with_position(rpos),
                );
                // Move back to construction site
                steps.push(
                    TaskStep::new(StepKind::MoveTo).with_position(site_pos),
                );
                // Deliver
                steps.push(
                    TaskStep::new(StepKind::Deliver { material, amount })
                        .with_entity(site_entity),
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
        Position::new(
            anchor.x + self.size.0 / 2,
            anchor.y + self.size.1 / 2,
        )
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
        let materials_delivered = materials_needed
            .iter()
            .map(|(m, _)| (*m, 0u32))
            .collect();
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

/// Tracks crop growth on a `Garden` building entity.
#[derive(Component, Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct CropState {
    /// Growth progress: 0.0 (just planted) → 1.0 (ready to harvest).
    pub growth: f32,
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
    pub fn capacity(kind: StructureType) -> usize {
        match kind {
            StructureType::Stores => 50,
            StructureType::Den => 8,
            StructureType::Workshop => 15,
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

// ---------------------------------------------------------------------------
// GateState component
// ---------------------------------------------------------------------------

/// Tracks whether a gate is open or closed.
///
/// Open gates allow wildlife through. Cats can always pass regardless of state,
/// but personality (diligence) determines whether they close the gate behind them.
#[derive(
    Component, Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct GateState {
    pub open: bool,
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
            StructureType::Stores,
            StructureType::Workshop,
            StructureType::Garden,
            StructureType::Watchtower,
            StructureType::WardPost,
            StructureType::Wall,
            StructureType::Gate,
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
}
