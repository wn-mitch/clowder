use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::beliefs::ColonyReservesBelief;
use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::{item_display_name, Item};
use crate::components::magic::{Inventory, ResourceKind};
use crate::components::markers::{
    HasCraftInputInInventory, HasCuriosInInventory, HasDryableInInventory, HasFreeSlot,
    HasFuelInInventory, HasHerbsInInventory, HasLowWardReserve, HasMaterialsInInventory,
    HasRawFishInInventory, HasRawMeatInInventory, HasRawOrganInInventory, HasRemedyHerbs,
    HasSmokeableInInventory, HasWardHerbs,
};
use crate::components::physical::Dead;
use crate::resources::colony_reserves::ColonyReserves;
use crate::resources::food::FoodStores;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// §4 per-cat inventory marker author
// ---------------------------------------------------------------------------

/// Author `HasHerbsInInventory`, `HasRemedyHerbs`, `HasWardHerbs`,
/// `HasFreeSlot` (231), and `HasMaterialsInInventory` /
/// `HasCuriosInInventory` (235) markers on living cats based on
/// their current inventory contents.
///
/// **Predicate fidelity.** The booleans authored here must match the inline
/// `ScoringContext` field computations in `goap.rs` / `disposition.rs`:
/// - `has_herbs_in_inventory` → `inventory.has_any_herb()`
/// - `has_remedy_herbs` → `inventory.has_remedy_herb()`
/// - `has_ward_herbs` → `inventory.has_ward_herb()`
/// - `has_free_slot` → `!inventory.is_full()` (231)
/// - `has_materials_in_inventory` → `inventory.has_any_material()` (235)
/// - `has_curios_in_inventory` → `inventory.has_any_curio()` (235)
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
            Has<HasFreeSlot>,
            Has<HasMaterialsInInventory>,
            Has<HasCuriosInInventory>,
            // 367: preservation inventory markers — bundled into a nested
            // tuple so the parent query stays under Bevy's 15-arity
            // `QueryData` ceiling (457 added `HasCraftInputInInventory`
            // and pushed the flat shape over).
            (
                Has<HasRawFishInInventory>,
                Has<HasRawOrganInInventory>,
                Has<HasRawMeatInInventory>,
                Has<HasFuelInInventory>,
                Has<HasDryableInInventory>,
                Has<HasSmokeableInInventory>,
            ),
            // 450: generic food-in-inventory marker.
            Has<crate::components::markers::HasFoodInInventory>,
            // 457: Workshop-craft input marker.
            Has<HasCraftInputInInventory>,
        ),
        Without<Dead>,
    >,
) {
    for (
        entity,
        inventory,
        has_herbs_marker,
        has_remedy_marker,
        has_ward_marker,
        has_free_slot_marker,
        has_materials_marker,
        has_curios_marker,
        (
            has_raw_fish_marker,
            has_raw_organ_marker,
            has_raw_meat_marker,
            has_fuel_marker,
            has_dryable_marker,
            has_smokeable_marker,
        ),
        has_food_marker,
        has_craft_input_marker,
    ) in cats.iter()
    {
        let has_herbs = inventory.has_any_herb();
        let has_remedy = inventory.has_remedy_herb();
        let has_ward = inventory.has_ward_herb();
        let has_free_slot = !inventory.is_full();
        let has_materials = inventory.has_any_material();
        let has_curios = inventory.has_any_curio();
        // 367 preservation inventory predicates.
        let has_raw_fish = inventory.has_raw_fish();
        let has_raw_organ = inventory.has_raw_organ();
        let has_raw_meat = inventory.has_raw_meat();
        let has_fuel = inventory.has_fuel();
        // 450: generic food-in-inventory predicate.
        let has_food = inventory.has_food();
        // 367 unified DSE-gate markers — `EligibilityFilter` lacks an OR
        // primitive, so the OR-of-{fish, organ} drying gate and the
        // AND-of-{meat, fuel} smoking gate live on these conjunction /
        // disjunction markers. Resolvers still read the more specific
        // single-kind markers above when they need to pick a particular
        // item to consume.
        let has_dryable = has_raw_fish || has_raw_organ;
        let has_smokeable = has_raw_meat && has_fuel;
        // 457: Workshop-craft input presence — fires when inventory
        // contains any Phase 2 recipe input (Twig / Bristle / Fiber /
        // Flower / Stone / Feather / PolishedStone). Recipe-agnostic;
        // the resolver picks the specific recipe at execute time.
        let has_craft_input = inventory.has_craft_input();

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
        match (has_free_slot, has_free_slot_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasFreeSlot);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasFreeSlot>();
            }
            _ => {}
        }
        match (has_materials, has_materials_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasMaterialsInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasMaterialsInInventory>();
            }
            _ => {}
        }
        match (has_curios, has_curios_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasCuriosInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasCuriosInInventory>();
            }
            _ => {}
        }
        // 367 — preservation inventory markers. Same toggle shape as
        // every sibling above; consolidating into a helper would be
        // cleaner but matches the explicit precedent.
        match (has_raw_fish, has_raw_fish_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasRawFishInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasRawFishInInventory>();
            }
            _ => {}
        }
        match (has_raw_organ, has_raw_organ_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasRawOrganInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasRawOrganInInventory>();
            }
            _ => {}
        }
        match (has_raw_meat, has_raw_meat_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasRawMeatInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasRawMeatInInventory>();
            }
            _ => {}
        }
        match (has_fuel, has_fuel_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasFuelInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasFuelInInventory>();
            }
            _ => {}
        }
        match (has_dryable, has_dryable_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasDryableInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasDryableInInventory>();
            }
            _ => {}
        }
        match (has_smokeable, has_smokeable_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasSmokeableInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasSmokeableInInventory>();
            }
            _ => {}
        }
        // 457: Workshop-craft input marker toggle.
        match (has_craft_input, has_craft_input_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasCraftInputInInventory);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasCraftInputInInventory>();
            }
            _ => {}
        }
        // 450: generic food-in-inventory marker.
        match (has_food, has_food_marker) {
            (true, false) => {
                commands
                    .entity(entity)
                    .insert(crate::components::markers::HasFoodInInventory);
            }
            (false, true) => {
                commands
                    .entity(entity)
                    .remove::<crate::components::markers::HasFoodInInventory>();
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

/// Recalculate `FoodStores` from actual food items across the colony (190).
///
/// **What this sets.**
/// - `current` / `capacity` describe `Stores` buildings only — the canonical
///   stockpile number the chronic-full latch, `HasStoredFood`, and the
///   coordinator's food-pressure assessment all reason about.
/// - `in_stores` / `in_dens` / `in_workshops` / `held` carry the per-source
///   breakdown the UI surfaces ("12 in stores · 3 in dens · 0 workshops ·
///   4 held"). These exist so the player can see where the colony's food
///   actually is without changing what `current` means for backend consumers.
pub fn sync_food_stores(
    mut food: ResMut<FoodStores>,
    stores_query: Query<(&Structure, &StoredItems)>,
    cats: Query<&crate::components::magic::Inventory, Without<crate::components::physical::Dead>>,
    items_query: Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
) {
    let mut in_stores = 0u32;
    let mut in_dens = 0u32;
    let mut in_workshops = 0u32;
    let mut total_capacity = 0.0f32;

    for (structure, stored) in stores_query.iter() {
        match structure.kind {
            StructureType::Stores => {
                total_capacity += StoredItems::effective_capacity_with_items(
                    StructureType::Stores,
                    &stored.items,
                    &items_query,
                ) as f32;
                for &item_entity in &stored.items {
                    if let Ok(item) = items_query.get(item_entity) {
                        if item.kind.is_food() {
                            in_stores += 1;
                        }
                    }
                }
            }
            StructureType::Den => {
                for &item_entity in &stored.items {
                    if let Ok(item) = items_query.get(item_entity) {
                        if item.kind.is_food() {
                            in_dens += 1;
                        }
                    }
                }
            }
            StructureType::Workshop => {
                for &item_entity in &stored.items {
                    if let Ok(item) = items_query.get(item_entity) {
                        if item.kind.is_food() {
                            in_workshops += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut held = 0u32;
    for inventory in cats.iter() {
        for slot in &inventory.pouch {
            if slot.kind.is_food() {
                held += 1;
            }
        }
    }

    food.in_stores = in_stores;
    food.in_dens = in_dens;
    food.in_workshops = in_workshops;
    food.held = held;
    food.current = in_stores as f32;
    food.capacity = total_capacity;
}

/// Recalculate `ColonyReserves` from actual herb items across cat inventories
/// and Stores buildings (ticket 308).
///
/// Ground truth for downstream observability and the per-cat
/// `ColonyReservesBelief` substrate. Mirrors `sync_food_stores`'s query shape.
pub fn sync_colony_reserves(
    mut reserves: ResMut<ColonyReserves>,
    cats: Query<&Inventory, Without<Dead>>,
    stores_query: Query<(&Structure, &StoredItems)>,
    items_query: Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
) {
    let mut thornbriar = 0u32;
    let mut remedy = 0u32;

    for inventory in cats.iter() {
        for slot in &inventory.pouch {
            match ResourceKind::from_item_kind(slot.kind) {
                Some(ResourceKind::Thornbriar) => thornbriar += 1,
                Some(ResourceKind::RemedyHerb) => remedy += 1,
                None => {}
            }
        }
    }

    for (structure, stored) in stores_query.iter() {
        if structure.kind == StructureType::Stores {
            for &item_entity in &stored.items {
                if let Ok(item) = items_query.get(item_entity) {
                    match ResourceKind::from_item_kind(item.kind) {
                        Some(ResourceKind::Thornbriar) => thornbriar += 1,
                        Some(ResourceKind::RemedyHerb) => remedy += 1,
                        None => {}
                    }
                }
            }
        }
    }

    reserves.thornbriar_count = thornbriar;
    reserves.remedy_herb_count = remedy;
}

/// Author the per-cat `HasLowWardReserve` marker from each cat's subjective
/// `ColonyReservesBelief` for `ResourceKind::Thornbriar` (ticket 308).
///
/// The marker fires iff the cat has any belief about thornbriar reserves
/// (entry exists with `strength > epsilon`) AND that belief's
/// `estimated_count <= low_ward_reserve_threshold`. Cats without belief data
/// (e.g. isolated or freshly spawned) don't fire the marker — substrate-honest:
/// you can't anticipate scarcity you haven't perceived.
///
/// Reader for this marker lands in ticket 309 (Herbcraft DSE consideration).
/// Allowlisted in `scripts/substrate_stubs.allowlist` until 309 lands.
pub fn update_low_ward_reserve_markers(
    mut commands: Commands,
    constants: Res<SimConstants>,
    cats: Query<(Entity, &ColonyReservesBelief, Has<HasLowWardReserve>), Without<Dead>>,
) {
    let threshold = constants.beliefs.low_ward_reserve_threshold;
    for (entity, belief, has_marker) in cats.iter() {
        let should_have = belief
            .reserves
            .get(&ResourceKind::Thornbriar)
            .is_some_and(|rb| rb.strength > f32::EPSILON && rb.estimated_count <= threshold);
        match (should_have, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(HasLowWardReserve);
            }
            (false, true) => {
                commands.entity(entity).remove::<HasLowWardReserve>();
            }
            _ => {}
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
    fn sync_food_stores_segregates_den_and_workshop_into_breakdown_fields() {
        let (mut world, mut schedule) = setup_sync();

        let den = world
            .spawn((Structure::new(StructureType::Den), StoredItems::default()))
            .id();
        let den_mouse = world
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
            .items = vec![den_mouse];

        let workshop = world
            .spawn((
                Structure::new(StructureType::Workshop),
                StoredItems::default(),
            ))
            .id();
        let workshop_fish = world
            .spawn(Item::new(
                ItemKind::RawFish,
                1.0,
                ItemLocation::StoredIn(workshop),
            ))
            .id();
        world
            .entity_mut(workshop)
            .get_mut::<StoredItems>()
            .unwrap()
            .items = vec![workshop_fish];

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert_eq!(
            food.current.round() as u32,
            0,
            "Stores-only `current` should ignore food in Dens/Workshops"
        );
        assert_eq!(food.in_stores, 0);
        assert_eq!(food.in_dens, 1, "Den food should populate in_dens");
        assert_eq!(
            food.in_workshops, 1,
            "Workshop food should populate in_workshops"
        );
        assert_eq!(food.total_accessible(), 2);
    }

    #[test]
    fn sync_food_stores_counts_cat_held_food() {
        let (mut world, mut schedule) = setup_sync();

        // A cat carrying one food item should populate `held`, not `current`.
        use crate::components::items::{ItemKind, ItemModifiers};
        use crate::components::magic::{Inventory, ItemSlot};

        world.spawn(Inventory {
            pouch: vec![ItemSlot::new(ItemKind::RawRabbit, ItemModifiers::default())],
            ..Default::default()
        });

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert_eq!(food.held, 1, "cat-held food should populate `held`");
        assert_eq!(food.in_stores, 0);
        assert_eq!(food.current.round() as u32, 0);
        assert_eq!(food.total_accessible(), 1);
    }

    #[test]
    fn sync_food_stores_skips_dead_cats_for_held_food() {
        let (mut world, mut schedule) = setup_sync();

        use crate::components::items::{ItemKind, ItemModifiers};
        use crate::components::magic::{Inventory, ItemSlot};
        use crate::components::physical::{Dead, DeathCause};

        world.spawn((
            Inventory {
                pouch: vec![ItemSlot::new(ItemKind::RawBird, ItemModifiers::default())],
                ..Default::default()
            },
            Dead {
                tick: 0,
                cause: DeathCause::Injury,
            },
        ));

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert_eq!(
            food.held, 0,
            "food on a corpse is not colony-accessible (the corpse decays as an item)"
        );
    }

    #[test]
    fn sync_food_stores_total_accessible_sums_all_sources() {
        let (mut world, mut schedule) = setup_sync();

        use crate::components::items::{ItemKind, ItemModifiers};
        use crate::components::magic::{Inventory, ItemSlot};

        let store = world
            .spawn((
                Structure::new(StructureType::Stores),
                StoredItems::default(),
            ))
            .id();
        let store_mouse = world
            .spawn(Item::new(
                ItemKind::RawMouse,
                1.0,
                ItemLocation::StoredIn(store),
            ))
            .id();
        world
            .entity_mut(store)
            .get_mut::<StoredItems>()
            .unwrap()
            .items = vec![store_mouse];

        let den = world
            .spawn((Structure::new(StructureType::Den), StoredItems::default()))
            .id();
        let den_fish = world
            .spawn(Item::new(
                ItemKind::RawFish,
                1.0,
                ItemLocation::StoredIn(den),
            ))
            .id();
        world
            .entity_mut(den)
            .get_mut::<StoredItems>()
            .unwrap()
            .items = vec![den_fish];

        world.spawn(Inventory {
            pouch: vec![ItemSlot::new(ItemKind::RawRabbit, ItemModifiers::default())],
            ..Default::default()
        });

        schedule.run(&mut world);

        let food = world.resource::<FoodStores>();
        assert_eq!(food.in_stores, 1);
        assert_eq!(food.in_dens, 1);
        assert_eq!(food.in_workshops, 0);
        assert_eq!(food.held, 1);
        assert_eq!(food.total_accessible(), 3);
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
        world
            .spawn(Inventory {
                pouch: slots,
                ..Default::default()
            })
            .id()
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
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::herb(HerbKind::HealingMoss)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasHerbsInInventory>(&world, cat));
        assert!(has_marker::<HasRemedyHerbs>(&world, cat));
        assert!(!has_marker::<HasWardHerbs>(&world, cat));
    }

    #[test]
    fn thornbriar_sets_herbs_and_ward() {
        let (mut world, mut schedule) = setup_inventory_markers();
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::herb(HerbKind::Thornbriar)]);
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
                ItemSlot::herb(HerbKind::Thornbriar),
                ItemSlot::herb(HerbKind::HealingMoss),
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
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::herb(HerbKind::HealingMoss)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasHerbsInInventory>(&world, cat));

        // Remove the herb.
        world.get_mut::<Inventory>(cat).unwrap().pouch.clear();
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
                    pouch: vec![ItemSlot::herb(HerbKind::HealingMoss)],
                    ..Default::default()
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
        let cat = spawn_cat_with_inventory(&mut world, vec![ItemSlot::herb(HerbKind::Thornbriar)]);
        schedule.run(&mut world);
        assert!(has_marker::<HasWardHerbs>(&world, cat));
        // Run again — should not flap.
        schedule.run(&mut world);
        assert!(has_marker::<HasWardHerbs>(&world, cat));
    }
}
