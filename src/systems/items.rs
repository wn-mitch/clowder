use bevy_ecs::prelude::*;

use crate::components::items::Item;

/// Advance decay on every item entity. Despawn items whose condition has
/// reached zero or below.
pub fn decay_items(mut commands: Commands, mut items: Query<(Entity, &mut Item)>) {
    for (entity, mut item) in &mut items {
        if item.tick_decay() {
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::items::{Item, ItemKind, ItemLocation};

    fn setup() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_items);
        (world, schedule)
    }

    #[test]
    fn destroyed_items_are_despawned() {
        let (mut world, mut schedule) = setup();

        // RawFish decays at 0.01/tick. Spawn with condition just above 0 so
        // a single tick drives it to <= 0.0.
        let mut item = Item::new(ItemKind::RawFish, 1.0, ItemLocation::OnGround);
        item.condition = 0.005; // less than one decay step (0.01)

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
}
