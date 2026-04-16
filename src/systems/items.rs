use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::{item_display_name, Item};
use crate::resources::food::FoodStores;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

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
                log.push(
                    time.tick,
                    format!("Some {name} in the stores has gone off."),
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
    items_query: Query<&Item>,
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
    items_query: Query<&Item>,
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
}
