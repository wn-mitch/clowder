use bevy_ecs::prelude::*;

use crate::components::building::ConstructionSite;
use crate::components::building::CropState;
use crate::components::building::Structure;
use crate::components::items::ItemKind;
use crate::components::magic::{Inventory, ItemSlot};
use crate::components::physical::Position;
use crate::components::task_chain::Material;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Deliver` (`GoapActionKind::DeliverMaterials`)
///
/// **Real-world effect** — consumes one carried unit of `material`
/// from the cat's `Inventory` (removing the `ItemSlot::Item` and
/// despawning the matching `Item` entity if `carried_item_entity`
/// is supplied) and calls `site.deliver(material, 1)` on the
/// targeted `ConstructionSite`. The single-unit-per-call shape
/// matches the founding wagon-dismantling flow: each pickup carries
/// one item, each delivery puts one item into the site's ledger.
/// Multi-trip delivery is handled by the planner via iterative
/// replanning until `materials_complete()` flips true.
///
/// **Plan-level preconditions** — emitted by
/// `src/ai/planner/actions.rs::building_actions` under
/// `ZoneIs(ConstructionSite) ∧ CarryingIs(BuildMaterials)` with
/// effects `SetCarrying(Nothing) ∧ SetMaterialsAvailable(true) ∧
/// IncrementTrips`. The planner's `materials_available` is
/// authored coarsely (any reachable site complete); the next
/// state-author tick re-reads from ECS, so a single Deliver that
/// doesn't fully fund the site triggers another haul cycle on
/// replan.
///
/// **Runtime preconditions** — requires `target_entity` to resolve
/// to a building with a `ConstructionSite`, AND requires the cat's
/// `Inventory` to contain at least one slot whose
/// `ItemKind::material()` matches the requested `material`. Either
/// failure returns `unwitnessed(Fail)` so the plan drops cleanly.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff inventory was
/// consumed AND `site.deliver(...)` was called this call. Pre-038,
/// the witness was `true` whenever the site existed (no inventory
/// check); the silent-Feature gap was the structural-prefunded
/// flow that meant `resolve_deliver` was never called at all (see
/// ticket 038 / landed entry for the full bug history).
///
/// **Feature emission** — caller passes `Feature::MaterialsDelivered`
/// (Positive) to `record_if_witnessed`. Each delivered unit
/// ratchets the count by one — a 6-Wood Stores founding site
/// produces 6 `MaterialsDelivered` events.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_deliver(
    material: Material,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    buildings: &mut Query<
        (
            Entity,
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ),
        Without<crate::components::task_chain::TaskChain>,
    >,
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Fail("no target for Deliver".into()));
    };

    // Find the inventory slot carrying the requested material. Wood/Stone
    // are the only `ItemKind::material()` returns; the slot lookup uses
    // the same bridge.
    let slot_idx = inventory.slots.iter().position(|s| {
        matches!(
            s,
            ItemSlot::Item(kind, _)
                if kind.material() == Some(material)
        )
    });
    let Some(slot_idx) = slot_idx else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "cat is not carrying the requested material".into(),
        ));
    };

    // Verify target site exists and has a ConstructionSite. Don't
    // mutate inventory until we know we can actually deliver.
    let Ok((_, _, Some(mut site), _, _)) = buildings.get_mut(target) else {
        return StepOutcome::unwitnessed(StepResult::Fail(
            "construction site missing or completed".into(),
        ));
    };

    // Both checks passed — consume the slot and bump the site.
    inventory.slots.swap_remove(slot_idx);
    site.deliver(material, 1);

    StepOutcome::witnessed(StepResult::Advance)
}

/// Helper for the legacy disposition-chain `StepKind::Deliver` path
/// (`src/systems/task_chains.rs`), which isn't currently produced by
/// any active system but is still wired. Bridges the chain's
/// `(material, amount)` shape to the new inventory-consuming
/// resolver by calling it `amount` times. Kept here rather than
/// inlined in `task_chains.rs` so the inventory-consumption
/// invariant lives next to its definition. Not a `pub fn resolve_*`
/// — it's a multi-call adapter, not a step resolver itself, so it's
/// exempt from the GOAP step-contract preamble check.
#[allow(clippy::type_complexity)]
pub fn deliver_legacy_chain_adapter(
    material: Material,
    amount: u32,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    buildings: &mut Query<
        (
            Entity,
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ),
        Without<crate::components::task_chain::TaskChain>,
    >,
) -> StepOutcome<bool> {
    let mut delivered_any = false;
    for _ in 0..amount {
        let outcome = resolve_deliver(material, target_entity, inventory, buildings);
        if outcome.witness {
            delivered_any = true;
        }
        if matches!(outcome.result, StepResult::Fail(_)) {
            // Stop on the first failure; the chain advances anyway
            // (legacy semantics) so the cat doesn't get stuck.
            break;
        }
    }
    if delivered_any {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}

// Suppress unused-import warning for ItemKind on builds that don't
// land a code path referencing it directly here. The import is kept
// because the rustdoc references `ItemKind::material()` and the
// helper above relies on that bridge through the inventory slots.
#[allow(dead_code)]
fn _kind_anchor() -> Option<ItemKind> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::building::StructureType;
    use crate::components::items::ItemModifiers;

    fn test_world() -> World {
        World::new()
    }

    fn spawn_site(world: &mut World, blueprint: StructureType) -> Entity {
        world
            .spawn((
                Structure::new(blueprint),
                ConstructionSite::new(blueprint),
                Position::new(5, 5),
            ))
            .id()
    }

    fn run(
        world: &mut World,
        material: Material,
        target: Option<Entity>,
        inventory: &mut Inventory,
    ) -> StepOutcome<bool> {
        let mut q = world.query_filtered::<(
            Entity,
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ), Without<crate::components::task_chain::TaskChain>>();
        let mut buildings = q.query_mut(world);
        resolve_deliver(material, target, inventory, &mut buildings)
    }

    #[test]
    fn carrying_matching_material_delivers_with_witness() {
        let mut world = test_world();
        let site = spawn_site(&mut world, StructureType::Stores);
        let mut inv = Inventory::default();
        inv.add_item(ItemKind::Wood);

        let outcome = run(&mut world, Material::Wood, Some(site), &mut inv);

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness, "real delivery must witness");
        assert!(
            !inv.has_item(ItemKind::Wood),
            "inventory must lose the carried wood"
        );
        let site_ref = world.get::<ConstructionSite>(site).unwrap();
        let wood_delivered = site_ref
            .materials_delivered
            .iter()
            .find(|(m, _)| *m == Material::Wood)
            .map(|(_, q)| *q)
            .unwrap_or(0);
        assert!(wood_delivered >= 1, "site must record one wood delivered");
    }

    #[test]
    fn empty_inventory_fails_unwitnessed() {
        let mut world = test_world();
        let site = spawn_site(&mut world, StructureType::Stores);
        let mut inv = Inventory::default();

        let outcome = run(&mut world, Material::Wood, Some(site), &mut inv);

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
        let site_ref = world.get::<ConstructionSite>(site).unwrap();
        let wood_delivered = site_ref
            .materials_delivered
            .iter()
            .find(|(m, _)| *m == Material::Wood)
            .map(|(_, q)| *q)
            .unwrap_or(99);
        assert_eq!(wood_delivered, 0, "site must not register any delivery");
    }

    #[test]
    fn carrying_wrong_material_fails_unwitnessed() {
        let mut world = test_world();
        let site = spawn_site(&mut world, StructureType::Stores);
        let mut inv = Inventory::default();
        inv.add_item(ItemKind::Stone);

        let outcome = run(&mut world, Material::Wood, Some(site), &mut inv);

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
        assert!(
            inv.has_item(ItemKind::Stone),
            "stone must not be consumed when wood is requested"
        );
    }

    #[test]
    fn missing_target_fails_unwitnessed() {
        let mut world = test_world();
        let mut inv = Inventory::default();
        inv.add_item(ItemKind::Wood);

        let outcome = run(&mut world, Material::Wood, None, &mut inv);

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
        assert!(
            inv.has_item(ItemKind::Wood),
            "no-target failure must preserve inventory"
        );
    }

    #[test]
    fn carries_modifiers_through_swap_remove() {
        // Slot ordering may rearrange after swap_remove — confirm the
        // remaining slots are still the cat's other items.
        let mut world = test_world();
        let site = spawn_site(&mut world, StructureType::Stores);
        let mut inv = Inventory::default();
        inv.add_item(ItemKind::Berries);
        inv.add_item_with_modifiers(ItemKind::Wood, ItemModifiers::default());
        inv.add_item(ItemKind::ShinyPebble);

        let outcome = run(&mut world, Material::Wood, Some(site), &mut inv);
        assert!(outcome.witness);
        assert!(!inv.has_item(ItemKind::Wood));
        assert!(inv.has_item(ItemKind::Berries));
        assert!(inv.has_item(ItemKind::ShinyPebble));
    }
}
