use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::{item_display_name, Item};
use crate::components::magic::Inventory;
use crate::components::markers::{HasHerbsInInventory, HasRemedyHerbs, HasWardHerbs};
use crate::components::physical::Dead;
use crate::resources::food::FoodStores;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// §4 per-cat inventory marker author
// ---------------------------------------------------------------------------

/// Author `HasHerbsInInventory`, `HasRemedyHerbs`, and `HasWardHerbs`
/// markers on living cats based on their current inventory contents.
///
/// **Predicate fidelity.** The booleans authored here must match the inline
/// `ScoringContext` field computations in `goap.rs` / `disposition.rs`:
/// - `has_herbs_in_inventory` → `inventory.has_any_herb()`
/// - `has_remedy_herbs` → `inventory.has_remedy_herb()`
/// - `has_ward_herbs` → `inventory.has_ward_herb()`
///
/// **Ordering.** Runs in Chain 2a before the GOAP/disposition scoring
/// pipeline, so `MarkerSnapshot` population can read `Has<M>` booleans
/// from freshly-authored markers.
#[allow(clippy::type_complexity)]
pub fn update_inventory_markers(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            &Inventory,
            Has<HasHerbsInInventory>,
            Has<HasRemedyHerbs>,
            Has<HasWardHerbs>,
        ),
        Without<Dead>,
    >,
) {
    for (entity, inventory, has_herbs_marker, has_remedy_marker, has_ward_marker) in cats.iter() {
        let has_herbs = inventory.has_any_herb();
        let has_remedy = inventory.has_remedy_herb();
        let has_ward = inventory.has_ward_herb();

        match (has_herbs, has_herbs_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasHerbsInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasHerbsInInventory>();
            }
            _ => {}
        }
        match (has_remedy, has_remedy_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasRemedyHerbs);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasRemedyHerbs>();
            }
            _ => {}
        }
        match (has_ward, has_ward_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasWardHerbs);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasWardHerbs>();
            }
            _ => {}
        }
    }
}

/// Advance decay on every item entity. Despawn items whose condition has
/// reached zero or below. Narrates food spoilage at a low rate.
pub fn decay_items(
    mut commands: Commands,
    mut items: Query<(Entity, &mut Item)>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
) {
    for (entity, mut item) in &mut items {
        if item.tick_decay() {
            // Narrate food spoilage (~10% of destroyed food items).
            if item.kind.is_food() && rng.rng.random::<f32>() < 0.1 {
                let name = item_display_name(item.kind, item.quality, &item.modifiers);
                let verb = if item.kind.is_plural_name() {
                    "have"
                } else {
                    "has"
                };
                log.push(
                    time.tick,
                    format!("Some {name} in the stores {verb} gone off."),
                    NarrativeTier::Micro,
                );
            }
            commands.entity(entity).despawn();
        }
    }
}

/// Recalculate `FoodStores` from actual food items in Stores buildings.
///
/// This keeps `FoodStores` as a derived value for TUI, scoring, and
/// coordination while the real food economy runs on items.
/// Prune dead entity IDs from StoredItems so despawned items don't occupy capacity.
pub fn prune_stored_items(
    mut stores_query: Query<(&Structure, &mut StoredItems)>,
    items_query: Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
) {
    for (structure, mut stored) in stores_query.iter_mut() {
        if structure.kind == StructureType::Stores {
            stored.items.retain(|&e| items_query.contains(e));
        }
    }
}

/// Recalculate FoodStores from actual food items in Stores buildings.
pub fn sync_food_stores(
    mut food: ResMut<FoodStores>,
    stores_query: Query<(&Structure, &StoredItems)>,
    items_query: Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
) {
    let mut total_food_count = 0u32;
    let mut total_capacity = 0.0f32;

    for (structure, stored) in stores_query.iter() {
        if structure.kind == StructureType::Stores {
            total_capacity += StoredItems::effective_capacity_with_items(
                StructureType::Stores,
                &stored.items,
                &items_query,
            ) as f32;
            for &item_entity in &stored.items {
                if let Ok(item) = items_query.get(item_entity) {
                    if item.kind.is_food() {
                        total_food_count += 1;
                    }
                }
            }
        }
    }

    food.current = total_food_count as f32;
    food.capacity = total_capacity;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::items::{Item, ItemKind, ItemLocation};

    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimRng::new(42));
        world.insert_resource(TimeState::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_items);
        (world, schedule)
    }

    #[test]
    fn destroyed_items_are_despawned() {
        let (mut world, mut schedule) = setup();

        // RawFish decays at 0.0001/tick. Spawn with condition just above 0 so
        // a single tick drives it to <= 0.0.
        let mut item = Item::new(ItemKind::RawFish, 1.0, ItemLocation::OnGround);
        item.condition = 0.00009; // less than one decay step (0.0001)

        let entity = world.spawn(item).id();

        schedule.run(&mut world);

        assert!(
            world.get::<Item>(entity).is_none(),
            "item with condition <= 0.0 after tick should be despawned"
        );
    }

    #[test]
    fn healthy_items_survive() {
        let (mut world, mut schedule) = setup();

        let item = Item::new(ItemKind::RawFish, 1.0, ItemLocation::OnGround);
        let entity = world.spawn(item).id();

        schedule.run(&mut world);

        let item = world
            .get::<Item>(entity)
            .expect("fresh item should still exist after one tick");

        assert!(
            item.condition > 0.0,
            "condition should still be positive; got {}",
            item.condition
        );
        // Condition should have decreased by exactly the decay rate.
        let expected = 1.0 - ItemKind::RawFish.decay_rate();
        assert!(
            (item.condition - expected).abs() < f32::EPSILON,
            "condition should be {expected}, got {}",
            item.condition
        );
    }

    // --- sync_food_stores ---

    fn setup_sync() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(FoodStores::new(0.0, 50.0, 0.002));
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_food_stores);
        (world, schedule)
    }

    #[test]
    fn sync_food_stores_counts_food_items_in_stores() {
        let (mut world, mut schedule) = setup_sync();

        // Spawn a Stores building with two food items.
        let store = world
            .spawn((
                Structure::new(StructureType::Stores),
                StoredItems::default(),
            ))
            .id();
        let mouse = world
            .spawn(Item::new(
                ItemKind::RawMouse,
                1.0,
                ItemLocation::StoredIn(store),
            ))
            .id();
        let fish = world
            .spawn(Item::new(
                ItemKind::RawFish,
                1.0,
                ItemLocation::StoredIn(store),
            ))
            .id();
        world
            .entity_mut(store)
            .get_mut::<StoredItems>()
            .unwrap()
            .items = vec![mouse, fish];

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        let expected = 2.0f32; // 2 food items (mouse + fish)
        assert!(
            (food.current - expected).abs() < f32::EPSILON,
            "FoodStores.current should count {expected} food items; got {}",
            food.current
        );
    }

    #[test]
    fn sync_food_stores_ignores_non_food_items() {
        let (mut world, mut schedule) = setup_sync();

        let store = world
            .spawn((
                Structure::new(StructureType::Stores),
                StoredItems::default(),
            ))
            .id();
        let pebble = world
            .spawn(Item::new(
                ItemKind::ShinyPebble,
                1.0,
                ItemLocation::StoredIn(store),
            ))
            .id();
        world
            .entity_mut(store)
            .get_mut::<StoredItems>()
            .unwrap()
            .items = vec![pebble];

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert!(
            food.current.abs() < f32::EPSILON,
            "non-food items should not contribute to FoodStores; got {}",
            food.current
        );
    }

    #[test]
    fn sync_food_stores_ignores_non_stores_buildings() {
        let (mut world, mut schedule) = setup_sync();

        // A Den with a food item should not count.
        let den = world
            .spawn((Structure::new(StructureType::Den), StoredItems::default()))
            .id();
        let mouse = world
            .spawn(Item::new(
                ItemKind::RawMouse,
                1.0,
                ItemLocation::StoredIn(den),
            ))
            .id();
        world
            .entity_mut(den)
            .get_mut::<StoredItems>()
            .unwrap()
            .items = vec![mouse];

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert!(
            food.current.abs() < f32::EPSILON,
            "food in non-Stores buildings should not count; got {}",
            food.current
        );
    }

    #[test]
    fn sync_food_stores_updates_capacity_from_stores_count() {
        let (mut world, mut schedule) = setup_sync();

        // Spawn two Stores buildings.
        world.spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
        ));
        world.spawn((
            Structure::new(StructureType::Stores),
            StoredItems::default(),
        ));

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        let expected_capacity = (StoredItems::capacity(StructureType::Stores) * 2) as f32;
        assert!(
            (food.capacity - expected_capacity).abs() < f32::EPSILON,
            "capacity should be {expected_capacity}; got {}",
            food.capacity
        );
    }

    // --- update_inventory_markers ---

    use crate::components::magic::{HerbKind, ItemSlot};

    fn setup_inventory_markers() -> (World, bevy_ecs::schedule::Schedule) {
        let world = World::new();
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(update_inventory_markers);
        (world, schedule)
    }

    fn spawn_cat_with_inventory(world: &mut World, slots: Vec<ItemSlot>) -> Entity {
        world.spawn(Inventory { slots }).id()
    }

    fn has_marker<M: bevy_ecs::component::Component>(world: &World, entity: Entity) -> bool {
        world.get::<M>(entity).is_some()
    }

    #[test]
    fn empty_inventory_no_herb_markers() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(&mut world, vec![]);
        schedule.run(&mut world);
        assert!(!has_marker::<HasHerbsInInventory>(&world, cat));
        assert!(!has_marker::<HasRemedyHerbs>(&world, cat));
        assert!(!has_marker::<HasWardHerbs>(&world, cat));
    }

    #[test]
    fn healing_moss_sets_herbs_and_remedy() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::Herb(HerbKind::HealingMoss)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasHerbsInInventory>(&world, cat));
        assert!(has_marker::<HasRemedyHerbs>(&world, cat));
        assert!(!has_marker::<HasWardHerbs>(&world, cat));
    }

    #[test]
    fn thornbriar_sets_herbs_and_ward() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::Herb(HerbKind::Thornbriar)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasHerbsInInventory>(&world, cat));
        assert!(!has_marker::<HasRemedyHerbs>(&world, cat));
        assert!(has_marker::<HasWardHerbs>(&world, cat));
    }

    #[test]
    fn mixed_herbs_set_all_markers() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(
            &mut world,
            vec![
                ItemSlot::Herb(HerbKind::Thornbriar),
                ItemSlot::Herb(HerbKind::HealingMoss),
            ],
        );
        schedule.run(&mut world);
        assert!(has_marker::<HasHerbsInInventory>(&world, cat));
        assert!(has_marker::<HasRemedyHerbs>(&world, cat));
        assert!(has_marker::<HasWardHerbs>(&world, cat));
    }

    #[test]
    fn herb_removal_clears_markers() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::Herb(HerbKind::HealingMoss)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasHerbsInInventory>(&world, cat));

        // Remove the herb.
        world.get_mut::<Inventory>(cat).unwrap().slots.clear();
        schedule.run(&mut world);
        assert!(
            !has_marker::<HasHerbsInInventory>(&world, cat),
            "clearing inventory should remove HasHerbsInInventory"
        );
        assert!(!has_marker::<HasRemedyHerbs>(&world, cat));
    }

    #[test]
    fn dead_cats_skip_inventory_markers() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = world
            .spawn((
                Inventory {
                    slots: vec![ItemSlot::Herb(HerbKind::HealingMoss)],
                },
                crate::components::physical::Dead {
                    tick: 0,
                    cause: crate::components::physical::DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(
            !has_marker::<HasHerbsInInventory>(&world, cat),
            "dead cats should not receive herb markers"
        );
    }

    #[test]
    fn inventory_markers_idempotent() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::Herb(HerbKind::Thornbriar)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasWardHerbs>(&world, cat));
        // Run again — should not flap.
        schedule.run(&mut world);
        assert!(has_marker::<HasWardHerbs>(&world, cat));
    }
}
