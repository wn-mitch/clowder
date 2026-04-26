use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::Structure;
use crate::components::goap_plan::GoapPlan;
use crate::components::items::{BuildMaterialItem, Item, ItemLocation};
use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `PickupMaterial` (`GoapActionKind::GatherMaterials`)
///
/// **Real-world effect** — picks up a build-material `Item` entity
/// (kind `Wood` or `Stone`, location `OnGround`) adjacent to the cat.
/// Transitions the item's `ItemLocation` to `Carried(cat)` and adds
/// a matching slot to the cat's `Inventory`. Founding wagon-
/// dismantling pipeline: ground items spawned next to the founding
/// site become carried by the first cats whose plan picks them up.
///
/// **Plan-level preconditions** — emitted by
/// `src/ai/planner/actions.rs::building_actions` under
/// `ZoneIs(MaterialPile) ∧ CarryingIs(Nothing)` with effect
/// `SetCarrying(BuildMaterials)`. The planner does NOT verify the
/// pile still has a free item or that inventory has space — both
/// runtime checks happen here.
///
/// **Runtime preconditions** — requires `target_entity` to resolve
/// to an `Item` whose `kind.material()` is `Some(_)`, whose
/// `location` is `OnGround`, and that sits adjacent (manhattan ≤ 1)
/// to the cat. If the cat is further away, paths toward the pile
/// and returns `unwitnessed(Continue)`. If the inventory is full,
/// returns `unwitnessed(Fail)` so the planner drops the pickup leg.
/// If the item was already taken (location no longer OnGround) or
/// despawned, returns `unwitnessed(Fail)` — another cat got there
/// first; replan to a different pile.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the item's location
/// flipped to `Carried(cat)` AND a slot was successfully inserted
/// into `Inventory` this call.
///
/// **Feature emission** — caller passes `Feature::MaterialPickedUp`
/// (Positive) to `record_if_witnessed`. Before this resolver, the
/// founding site spawned with `materials_delivered = materials_needed`
/// (prefunded), so no pickup ever happened and the Feature did not
/// exist; this resolver brings physical-causality back to the
/// founding build economy.
#[allow(clippy::type_complexity)]
pub fn resolve_pickup_material(
    target_entity: Option<Entity>,
    cat_entity: Entity,
    pos: &mut Position,
    cached_path: &mut Option<Vec<Position>>,
    inventory: &mut Inventory,
    items: &mut Query<
        (Entity, &'static mut Item, &'static Position),
        (
            Without<GoapPlan>,
            Without<Structure>,
            With<BuildMaterialItem>,
        ),
    >,
    map: &TileMap,
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Fail("no target for PickupMaterial".into()));
    };

    // Snapshot the item's current state — verifies it still exists, is
    // still on the ground, and is still a build material.
    let (item_kind, item_pos) = match items.get(target) {
        Ok((_, item, item_pos)) => {
            if !matches!(item.location, ItemLocation::OnGround) {
                return StepOutcome::unwitnessed(StepResult::Fail("pile already taken".into()));
            }
            if item.kind.material().is_none() {
                return StepOutcome::unwitnessed(StepResult::Fail(
                    "item is not a build material".into(),
                ));
            }
            (item.kind, *item_pos)
        }
        Err(_) => {
            return StepOutcome::unwitnessed(StepResult::Fail("pile despawned".into()));
        }
    };

    // Walk to the pile if not adjacent yet.
    if pos.manhattan_distance(&item_pos) > 1 {
        if cached_path.is_none() {
            *cached_path = find_path(*pos, item_pos, map);
        }
        if let Some(ref mut path) = cached_path {
            if !path.is_empty() {
                *pos = path.remove(0);
            }
        }
        return StepOutcome::unwitnessed(StepResult::Continue);
    }

    // Adjacent — try the pickup. Inventory full is a soft fail; the
    // planner can drop the leg and the cat will deposit elsewhere
    // before retrying.
    if inventory.is_full() {
        return StepOutcome::unwitnessed(StepResult::Fail("inventory full".into()));
    }
    if !inventory.add_item(item_kind) {
        return StepOutcome::unwitnessed(StepResult::Fail("inventory rejected item".into()));
    }

    // Inventory accepted — flip the item's location. If the mutable
    // lookup fails (entity went away between the read above and now),
    // back the inventory change out so we don't leak a phantom
    // material slot into the cat's hands.
    match items.get_mut(target) {
        Ok((_, mut item, _)) => {
            item.location = ItemLocation::Carried(cat_entity);
        }
        Err(_) => {
            inventory.take_item(item_kind);
            return StepOutcome::unwitnessed(StepResult::Fail("pile despawned mid-pickup".into()));
        }
    }

    StepOutcome::witnessed(StepResult::Advance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::ItemKind;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, crate::resources::map::Terrain::Grass));
        world
    }

    fn spawn_ground_item(world: &mut World, kind: ItemKind, at: Position) -> Entity {
        // Tests stamp the BuildMaterialItem marker on every ground item
        // (regardless of kind) so the resolver's `With<BuildMaterialItem>`
        // filter sees the entity. The resolver then returns Fail for
        // non-build-material kinds (validating the runtime check).
        world
            .spawn((
                Item::new(kind, 1.0, ItemLocation::OnGround),
                at,
                BuildMaterialItem,
            ))
            .id()
    }

    fn run_with_resolver(
        world: &mut World,
        cat_entity: Entity,
        cat_pos: &mut Position,
        cached_path: &mut Option<Vec<Position>>,
        inventory: &mut Inventory,
        target: Option<Entity>,
    ) -> StepOutcome<bool> {
        // Build a fresh map for the resolver call — the World's
        // `Res<TileMap>` borrow conflicts with the mutable items query
        // we need to construct, so the test pre-instantiates a separate
        // identical map (all-grass 20×20) instead of cloning.
        let map = TileMap::new(20, 20, crate::resources::map::Terrain::Grass);
        let mut q = world.query_filtered::<(Entity, &mut Item, &Position), (
            Without<GoapPlan>,
            Without<Structure>,
            With<BuildMaterialItem>,
        )>();
        let mut items = q.query_mut(world);
        resolve_pickup_material(
            target,
            cat_entity,
            cat_pos,
            cached_path,
            inventory,
            &mut items,
            &map,
        )
    }

    #[test]
    fn adjacent_ground_item_picks_up() {
        let mut world = test_world();
        let item = spawn_ground_item(&mut world, ItemKind::Wood, Position::new(5, 5));
        let cat = world.spawn(Inventory::default()).id();
        let mut cat_pos = Position::new(5, 6);
        let mut cached: Option<Vec<Position>> = None;
        let mut inv = Inventory::default();

        let outcome = run_with_resolver(
            &mut world,
            cat,
            &mut cat_pos,
            &mut cached,
            &mut inv,
            Some(item),
        );

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness, "witness must flip true on real pickup");
        assert!(
            inv.has_item(ItemKind::Wood),
            "inventory must hold the picked-up wood"
        );
        let item_ref = world.get::<Item>(item).unwrap();
        assert!(
            matches!(item_ref.location, ItemLocation::Carried(c) if c == cat),
            "item location must flip to Carried(cat), got {:?}",
            item_ref.location
        );
    }

    #[test]
    fn distant_item_continues_with_no_witness() {
        let mut world = test_world();
        let item = spawn_ground_item(&mut world, ItemKind::Wood, Position::new(15, 15));
        let cat = world.spawn(Inventory::default()).id();
        let mut cat_pos = Position::new(2, 2);
        let mut cached: Option<Vec<Position>> = None;
        let mut inv = Inventory::default();

        let outcome = run_with_resolver(
            &mut world,
            cat,
            &mut cat_pos,
            &mut cached,
            &mut inv,
            Some(item),
        );

        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(!outcome.witness, "no witness while still walking");
        assert!(!inv.has_item(ItemKind::Wood));
    }

    #[test]
    fn full_inventory_fails_unwitnessed() {
        let mut world = test_world();
        let item = spawn_ground_item(&mut world, ItemKind::Wood, Position::new(5, 5));
        let cat = world.spawn(Inventory::default()).id();
        let mut cat_pos = Position::new(5, 5);
        let mut cached: Option<Vec<Position>> = None;
        let mut inv = Inventory {
            slots: (0..Inventory::MAX_SLOTS)
                .map(|_| {
                    crate::components::magic::ItemSlot::Item(
                        ItemKind::ShinyPebble,
                        crate::components::items::ItemModifiers::default(),
                    )
                })
                .collect(),
        };

        let outcome = run_with_resolver(
            &mut world,
            cat,
            &mut cat_pos,
            &mut cached,
            &mut inv,
            Some(item),
        );

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
        let item_ref = world.get::<Item>(item).unwrap();
        assert!(
            matches!(item_ref.location, ItemLocation::OnGround),
            "item must remain on ground when pickup fails"
        );
    }

    #[test]
    fn already_carried_item_fails_unwitnessed() {
        let mut world = test_world();
        let dummy_carrier = world.spawn(()).id();
        let item = world
            .spawn((
                Item::new(ItemKind::Wood, 1.0, ItemLocation::Carried(dummy_carrier)),
                Position::new(5, 5),
                BuildMaterialItem,
            ))
            .id();
        let cat = world.spawn(Inventory::default()).id();
        let mut cat_pos = Position::new(5, 5);
        let mut cached: Option<Vec<Position>> = None;
        let mut inv = Inventory::default();

        let outcome = run_with_resolver(
            &mut world,
            cat,
            &mut cat_pos,
            &mut cached,
            &mut inv,
            Some(item),
        );

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
        assert!(!inv.has_item(ItemKind::Wood));
    }

    #[test]
    fn non_material_kind_fails_unwitnessed() {
        let mut world = test_world();
        let item = spawn_ground_item(&mut world, ItemKind::ShinyPebble, Position::new(5, 5));
        let cat = world.spawn(Inventory::default()).id();
        let mut cat_pos = Position::new(5, 5);
        let mut cached: Option<Vec<Position>> = None;
        let mut inv = Inventory::default();

        let outcome = run_with_resolver(
            &mut world,
            cat,
            &mut cat_pos,
            &mut cached,
            &mut inv,
            Some(item),
        );

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
    }

    #[test]
    fn missing_target_fails_with_no_target() {
        let mut world = test_world();
        let cat = world.spawn(Inventory::default()).id();
        let mut cat_pos = Position::new(5, 5);
        let mut cached: Option<Vec<Position>> = None;
        let mut inv = Inventory::default();

        let outcome = run_with_resolver(&mut world, cat, &mut cat_pos, &mut cached, &mut inv, None);

        assert!(matches!(outcome.result, StepResult::Fail(_)));
        assert!(!outcome.witness);
    }
}
