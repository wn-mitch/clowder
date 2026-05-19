use bevy_ecs::prelude::*;

use crate::components::building::StoredHerbs;
use crate::components::magic::{HerbKind, Inventory};
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveHerbs(kind)` (ticket 084)
///
/// **Real-world effect** — transfers one herb of `kind` from the
/// target Stores' `StoredHerbs` into the actor's `Inventory.slots`
/// via `Inventory::add_herb`. Decrements `StoredHerbs.counts[kind]`
/// by 1.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Stores) ∧
/// HasStoredThornbriar ∧ (HasFreeSlot ∨ HasFreeSlotThisPlan)` (only
/// thornbriar uses the stash retrieval path today; non-thornbriar
/// kinds compile-error if/when a future caller routes them through
/// here without authoring the corresponding marker). Wired in Commit 2
/// of ticket 084, into the `HerbcraftSetWard` template.
///
/// **Runtime preconditions** — `target_entity` may be `None` (no
/// Stores resolved) → `unwitnessed(Advance)`. If `StoredHerbs` has
/// zero of `kind` → also `unwitnessed(Advance)`: a marker→reality
/// race (the stash marker said yes, but another cat drained the
/// stash this same tick). The plan moves on and a follow-on
/// replanning tick re-evaluates. If inventory is full →
/// `unwitnessed(Fail("inventory full"))` mirroring
/// `resolve_retrieve_any_food_from_stores` from ticket 175 — the
/// store-side decrement must NOT happen if the cat-side add can't
/// happen, or the herb is silently destroyed.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff exactly one herb was
/// transferred this call.
///
/// **Feature emission** — caller passes `Feature::HerbsRetrieved`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_retrieve_herbs_from_stores(
    target_entity: Option<Entity>,
    kind: HerbKind,
    inventory: &mut Inventory,
    stored_herbs_query: &mut Query<&mut StoredHerbs>,
) -> StepOutcome<bool> {
    let Some(store_entity) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Ok(mut stash) = stored_herbs_query.get_mut(store_entity) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    if stash.count(kind) == 0 {
        return StepOutcome::unwitnessed(StepResult::Advance);
    }
    if inventory.is_full() {
        return StepOutcome::unwitnessed(StepResult::Fail("inventory full".into()));
    }

    // Inventory-side add first — if `add_herb` ever grows a stricter
    // gate (e.g. herb-slot-specific cap), we'd silently destroy the
    // herb on the cat-side fail without this ordering. Today `add_herb`
    // only checks `is_full()` which we pre-checked above; the ordering
    // is defense-in-depth, mirroring `transfer_item_stores_to_inventory`.
    if !inventory.add_herb(kind) {
        return StepOutcome::unwitnessed(StepResult::Fail("inventory full".into()));
    }
    let took = stash.take(kind);
    debug_assert!(took, "stash.count(kind) > 0 was checked above");
    StepOutcome::witnessed(StepResult::Advance)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stash_world_with(kind: HerbKind, n: u32) -> (World, Entity) {
        let mut world = World::new();
        let mut sh = StoredHerbs::default();
        sh.add(kind, n, 20);
        let e = world.spawn(sh).id();
        (world, e)
    }

    fn run_resolver(
        world: &mut World,
        target: Option<Entity>,
        kind: HerbKind,
        inventory: &mut Inventory,
    ) -> StepOutcome<bool> {
        let mut system_state: bevy_ecs::system::SystemState<Query<&mut StoredHerbs>> =
            bevy_ecs::system::SystemState::new(world);
        let mut q = system_state.get_mut(world);
        resolve_retrieve_herbs_from_stores(target, kind, inventory, &mut q)
    }

    #[test]
    fn witnessed_on_success() {
        let (mut world, store) = stash_world_with(HerbKind::Thornbriar, 3);
        let mut inv = Inventory::default();
        let outcome = run_resolver(&mut world, Some(store), HerbKind::Thornbriar, &mut inv);
        assert!(outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(inv.has_herb(HerbKind::Thornbriar));
        let stash = world.get::<StoredHerbs>(store).unwrap();
        assert_eq!(stash.count(HerbKind::Thornbriar), 2);
    }

    #[test]
    fn unwitnessed_on_empty_stash() {
        let (mut world, store) = stash_world_with(HerbKind::Thornbriar, 0);
        let mut inv = Inventory::default();
        let outcome = run_resolver(&mut world, Some(store), HerbKind::Thornbriar, &mut inv);
        assert!(!outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(!inv.has_herb(HerbKind::Thornbriar));
    }

    #[test]
    fn unwitnessed_on_no_target() {
        let mut world = World::new();
        let mut inv = Inventory::default();
        let outcome = run_resolver(&mut world, None, HerbKind::Thornbriar, &mut inv);
        assert!(!outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
    }

    #[test]
    fn fail_on_full_inventory_preserves_stash() {
        let (mut world, store) = stash_world_with(HerbKind::Thornbriar, 3);
        let mut inv = Inventory::default();
        for _ in 0..Inventory::MAX_SLOTS {
            assert!(inv.add_herb(HerbKind::Moonpetal));
        }
        assert!(inv.is_full());
        let outcome = run_resolver(&mut world, Some(store), HerbKind::Thornbriar, &mut inv);
        assert!(!outcome.witness);
        assert!(matches!(outcome.result, StepResult::Fail(_)));
        // Stash unchanged — items-are-real.
        let stash = world.get::<StoredHerbs>(store).unwrap();
        assert_eq!(stash.count(HerbKind::Thornbriar), 3);
    }
}
