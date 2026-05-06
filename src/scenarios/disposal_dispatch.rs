//! Ticket 177 — dispatch-level tests for `Trash` / `Handoff` / `PickUp`.
//!
//! These tests exercise the GOAP dispatch arms wired in 177 by
//! hand-crafting a `GoapPlan` whose `current_step` is the disposal action
//! and asserting the post-tick world state. `evaluate_and_plan`'s
//! `Without<GoapPlan>` filter (`src/systems/goap.rs:860`) means a cat that
//! already has a plan is skipped by the planner — so the hand-crafted
//! plan reaches `resolve_goap_plans` intact and the dispatch arm runs.
//!
//! Disposal DSEs ship default-zero (`src/ai/dses/{drop,trashing,handing,
//! picking_up}.rs`), so the planner would never elect these actions
//! organically today; the hand-stamped plan is the only viable test
//! pattern until ticket 178 (Trash/Handoff/Drop weights) and ticket 185
//! (PickingUp weights gated on `HasGroundCarcass`) lift the scoring.

#![cfg(test)]

use bevy_ecs::prelude::*;
use bevy_ecs::world::World;

use crate::ai::planner::{GoapActionKind, PlannedStep};
use crate::ai::Action;
use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::disposition::DispositionKind;
use crate::components::goap_plan::GoapPlan;
use crate::components::items::{Item, ItemKind, ItemLocation, ItemModifiers};
use crate::components::magic::Inventory;
use crate::components::personality::Personality;
use crate::components::physical::Position;
use crate::resources::system_activation::{Feature, SystemActivation};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::runner::build_scenario_app;
use super::Scenario;

/// Stamp a single-step `GoapPlan` on `entity` whose only step is `action`
/// with `target_entity` set on the step state. Mirrors what the planner
/// would emit for a one-step disposal at a colocated target — minus
/// `TravelTo`, since these tests place the focal cat at the target's
/// position.
fn stamp_disposal_plan(
    world: &mut World,
    entity: Entity,
    kind: DispositionKind,
    action: Action,
    goap_action: GoapActionKind,
    target_entity: Option<Entity>,
) {
    let personality = world
        .entity(entity)
        .get::<Personality>()
        .expect("focal cat must have Personality")
        .clone();
    let mut plan = GoapPlan::new(
        kind,
        action,
        0,
        &personality,
        vec![PlannedStep {
            action: goap_action,
            cost: 1,
        }],
    );
    plan.step_state[0].target_entity = target_entity;
    world.entity_mut(entity).insert(plan);
}

/// Find an entity by `Name`. Used by the assertion phase to look up the
/// focal cat / midden / recipient after the dispatch tick.
fn entity_by_name(world: &mut World, name: &str) -> Option<Entity> {
    use crate::components::identity::Name;
    let mut q = world.query::<(Entity, &Name)>();
    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
}

// ----------------------------------------------------------------------
// Trash — focal cat at midden tile, 1 raw mouse in inventory.
// Expected: midden's StoredItems gains 1 entity; cat inventory empties;
// Feature::ItemTrashed records.
// ----------------------------------------------------------------------

fn setup_trash(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let midden = world
        .spawn((
            Structure::new(StructureType::Midden),
            StoredItems::default(),
            Position::new(20, 20),
        ))
        .id();

    let focal = spawn_cat(
        world,
        CatPreset::adult("TrashFocal", Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );

    let added = world
        .entity_mut(focal)
        .get_mut::<Inventory>()
        .expect("cat must have Inventory")
        .add_item(ItemKind::RawMouse);
    assert!(added, "fixture: focal cat inventory must accept a RawMouse");

    stamp_disposal_plan(
        world,
        focal,
        DispositionKind::Trashing,
        Action::Trash,
        GoapActionKind::TrashItemAtMidden,
        Some(midden),
    );
}

static TRASH_SCENARIO: Scenario = Scenario {
    name: "disposal_dispatch_trash",
    default_focal: "TrashFocal",
    default_ticks: 1,
    setup: setup_trash,
};

#[test]
fn trash_dispatch_moves_item_into_midden() {
    let mut app = build_scenario_app(42, &TRASH_SCENARIO, "TrashFocal");
    // First update runs Startup (scenario setup); subsequent updates run
    // FixedUpdate. The dispatch arm runs in the first FixedUpdate tick.
    app.update();
    for _ in 0..3 {
        app.update();
    }

    let world = app.world_mut();

    let focal = entity_by_name(world, "TrashFocal").expect("focal still alive");
    let inv = world
        .entity(focal)
        .get::<Inventory>()
        .expect("focal has Inventory");
    assert_eq!(
        inv.slots.len(),
        0,
        "trash dispatch should empty the actor's inventory; got {} slot(s)",
        inv.slots.len()
    );

    let mut q = world.query::<(&Structure, &StoredItems)>();
    let total_stored: usize = q
        .iter(world)
        .filter(|(s, _)| s.kind == StructureType::Midden)
        .map(|(_, st)| st.items.len())
        .sum();
    assert_eq!(
        total_stored, 1,
        "trash dispatch should add exactly one item to a midden's StoredItems; got {total_stored}"
    );

    let activation = world.resource::<SystemActivation>();
    let trashed = activation.counts.get(&Feature::ItemTrashed).copied().unwrap_or(0);
    assert!(
        trashed > 0,
        "Feature::ItemTrashed must record on a successful trash; counts={:?}",
        activation.counts
    );
}

// ----------------------------------------------------------------------
// PickUp — focal cat at ground item tile.
// Expected: ground item entity despawns; cat inventory gains 1 slot;
// Feature::ItemRetrieved records.
// ----------------------------------------------------------------------

fn setup_pick_up(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let item_entity = world
        .spawn((
            Item::with_modifiers(
                ItemKind::RawMouse,
                1.0,
                ItemLocation::OnGround,
                ItemModifiers::default(),
            ),
            Position::new(20, 20),
        ))
        .id();

    let focal = spawn_cat(
        world,
        CatPreset::adult("PickUpFocal", Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );

    stamp_disposal_plan(
        world,
        focal,
        DispositionKind::PickingUp,
        Action::PickUp,
        GoapActionKind::PickUpItemFromGround,
        Some(item_entity),
    );
}

static PICK_UP_SCENARIO: Scenario = Scenario {
    name: "disposal_dispatch_pick_up",
    default_focal: "PickUpFocal",
    default_ticks: 1,
    setup: setup_pick_up,
};

#[test]
fn pick_up_dispatch_brings_ground_item_into_inventory() {
    let mut app = build_scenario_app(42, &PICK_UP_SCENARIO, "PickUpFocal");
    app.update();
    for _ in 0..3 {
        app.update();
    }

    let world = app.world_mut();

    let focal = entity_by_name(world, "PickUpFocal").expect("focal still alive");
    let inv = world
        .entity(focal)
        .get::<Inventory>()
        .expect("focal has Inventory");
    assert_eq!(
        inv.slots.len(),
        1,
        "pick-up dispatch should add one slot to the actor's inventory; got {} slot(s)",
        inv.slots.len()
    );

    let mut items_q = world.query::<&Item>();
    let on_ground_count = items_q
        .iter(world)
        .filter(|item| matches!(item.location, ItemLocation::OnGround))
        .count();
    assert_eq!(
        on_ground_count, 0,
        "pick-up dispatch should despawn the ground item; got {on_ground_count} OnGround Item(s)"
    );

    let activation = world.resource::<SystemActivation>();
    let retrieved = activation
        .counts
        .get(&Feature::ItemRetrieved)
        .copied()
        .unwrap_or(0);
    assert!(
        retrieved > 0,
        "Feature::ItemRetrieved must record on a successful pick-up; counts={:?}",
        activation.counts
    );
}

// ----------------------------------------------------------------------
// Handoff — focal cat adjacent to recipient cat, 1 herb in actor inv.
// Expected: actor inventory empties; recipient inventory gains 1 slot;
// Feature::ItemHandedOff records.
// ----------------------------------------------------------------------

fn setup_handoff(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let recipient = spawn_cat(
        world,
        CatPreset::adult("HandoffRecipient", Position::new(21, 20)).with_marker(MarkerKind::Adult),
    );

    let focal = spawn_cat(
        world,
        CatPreset::adult("HandoffFocal", Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );

    let added = world
        .entity_mut(focal)
        .get_mut::<Inventory>()
        .expect("focal cat has Inventory")
        .add_item(ItemKind::RawMouse);
    assert!(added, "fixture: focal cat inventory must accept a RawMouse");

    stamp_disposal_plan(
        world,
        focal,
        DispositionKind::Handing,
        Action::Handoff,
        GoapActionKind::HandoffItem,
        Some(recipient),
    );

    // Stamp a no-op Resting plan on the recipient so they have a
    // `GoapPlan` and thus appear in the cats query at post-loop drain
    // time. Without this, `evaluate_and_plan` may not produce a viable
    // plan in tick 1 (default needs leave most dispositions zero-scored)
    // and the recipient falls out of the query — making
    // `cats.get_many_mut([actor, recipient])` Err and the deferred
    // handoff silently no-op.
    stamp_disposal_plan(
        world,
        recipient,
        DispositionKind::Resting,
        Action::Sleep,
        GoapActionKind::Sleep,
        None,
    );
}

static HANDOFF_SCENARIO: Scenario = Scenario {
    name: "disposal_dispatch_handoff",
    default_focal: "HandoffFocal",
    default_ticks: 1,
    setup: setup_handoff,
};

#[test]
fn handoff_dispatch_transfers_slot_to_recipient() {
    let mut app = build_scenario_app(42, &HANDOFF_SCENARIO, "HandoffFocal");
    app.update();
    for _ in 0..3 {
        app.update();
    }

    let world = app.world_mut();

    let focal = entity_by_name(world, "HandoffFocal").expect("focal still alive");
    let recipient = entity_by_name(world, "HandoffRecipient").expect("recipient still alive");

    let actor_slots = world
        .entity(focal)
        .get::<Inventory>()
        .expect("focal has Inventory")
        .slots
        .len();
    let recipient_slots = world
        .entity(recipient)
        .get::<Inventory>()
        .expect("recipient has Inventory")
        .slots
        .len();

    assert_eq!(
        actor_slots, 0,
        "handoff dispatch + post-loop drain should empty the actor's inventory; got {actor_slots} slot(s)"
    );
    assert_eq!(
        recipient_slots, 1,
        "handoff dispatch + post-loop drain should add one slot to the recipient; got {recipient_slots} slot(s)"
    );

    let activation = world.resource::<SystemActivation>();
    let handed = activation
        .counts
        .get(&Feature::ItemHandedOff)
        .copied()
        .unwrap_or(0);
    assert!(
        handed > 0,
        "Feature::ItemHandedOff must record on a successful handoff; counts={:?}",
        activation.counts
    );
}
