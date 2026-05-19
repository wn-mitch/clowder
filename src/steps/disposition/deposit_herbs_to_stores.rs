use bevy_ecs::prelude::*;

use crate::components::building::StoredHerbs;
use crate::components::magic::{HerbKind, Inventory};
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `DepositHerbs` (ticket 084)
///
/// **Real-world effect** — transfers every herb-kind slot from the
/// actor's `Inventory` into the target Stores' `StoredHerbs`. Slots
/// whose `kind.is_herb()` is true are removed from the inventory and
/// their `HerbKind` count is incremented in `StoredHerbs`. Per-kind
/// capacity is bounded by `stores_herb_capacity_per_kind` (default 20)
/// — herbs the stash can't absorb stay in the cat's inventory rather
/// than being silently destroyed (items-are-real discipline; see
/// `resolve_deposit_at_stores` for the food-side precedent established
/// in ticket 175).
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Stores) ∧
/// CarryingIs(Herbs)` by `src/ai/planner/actions.rs` (wired in Commit 2
/// of ticket 084, into the `HerbcraftGather` template after
/// `GatherHerb`).
///
/// **Runtime preconditions** — `target_entity` may be `None` (no Stores
/// resolved by the goap dispatch arm) → returns
/// `unwitnessed(Advance)`. If the inventory holds no herb slots → also
/// `unwitnessed(Advance)`: the precondition was satisfied at plan time
/// but the herbs were lost en route (e.g. swept by a Drop or eaten by
/// a hunger emergency); the plan moves on.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff ≥1 herb was actually
/// transferred from inventory to stash this call. False iff none of the
/// inventory's herb slots could be absorbed (stash already at per-kind
/// cap for every herb the cat carried).
///
/// **Feature emission** — caller passes `Feature::HerbsDeposited`
/// (Positive) to `record_if_witnessed`. Cap-rejected herbs that
/// remain in inventory do NOT count as a witnessed deposit — the
/// witness is gated on `transferred > 0`, mirroring the
/// `transfer_item_stores_to_inventory` discipline from
/// `resolve_retrieve_any_food_from_stores`.
pub fn resolve_deposit_herbs_to_stores(
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    stored_herbs_query: &mut Query<&mut StoredHerbs>,
    capacity_per_kind: u32,
) -> StepOutcome<bool> {
    let Some(store_entity) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Ok(mut stash) = stored_herbs_query.get_mut(store_entity) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };

    // Snapshot the herb slot indices BEFORE we start mutating: per-kind
    // capacity misses must leave the un-absorbed slot in place, so we
    // record per-index dispositions and apply the removals in
    // reverse-index order at the end (mirrors the
    // `resolve_deposit_at_stores` discipline from ticket 175).
    let herb_indices: Vec<(usize, HerbKind)> = inventory
        .slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            crate::components::magic::HerbKind::from_item_kind(slot.kind).map(|k| (i, k))
        })
        .collect();
    if herb_indices.is_empty() {
        return StepOutcome::unwitnessed(StepResult::Advance);
    }

    let mut transferred = 0u32;
    let mut absorbed_indices: Vec<usize> = Vec::with_capacity(herb_indices.len());
    for (idx, kind) in herb_indices {
        let added = stash.add(kind, 1, capacity_per_kind);
        if added == 1 {
            transferred += 1;
            absorbed_indices.push(idx);
        } else {
            // Cap miss — leave this slot in inventory; try the next
            // slot (it may be a different kind that fits).
        }
    }

    // Remove absorbed slots in reverse-index order to keep earlier
    // indices stable across `swap_remove` calls.
    absorbed_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in absorbed_indices {
        inventory.slots.swap_remove(idx);
    }

    if transferred > 0 {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::magic::HerbKind;

    /// Spawn a stash entity in a fresh world. Returns (world, entity).
    fn stash_world() -> (World, Entity) {
        let mut world = World::new();
        let e = world.spawn(StoredHerbs::default()).id();
        (world, e)
    }

    fn run_resolver(
        world: &mut World,
        target: Option<Entity>,
        inventory: &mut Inventory,
        cap: u32,
    ) -> StepOutcome<bool> {
        let mut system_state: bevy_ecs::system::SystemState<Query<&mut StoredHerbs>> =
            bevy_ecs::system::SystemState::new(world);
        let mut q = system_state.get_mut(world);
        resolve_deposit_herbs_to_stores(target, inventory, &mut q, cap)
    }

    #[test]
    fn witnessed_on_success() {
        let (mut world, store) = stash_world();
        let mut inv = Inventory::default();
        inv.add_herb(HerbKind::Thornbriar);
        inv.add_herb(HerbKind::Thornbriar);
        let outcome = run_resolver(&mut world, Some(store), &mut inv, 20);
        assert!(outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(inv.slots.is_empty());
        let stash = world.get::<StoredHerbs>(store).unwrap();
        assert_eq!(stash.count(HerbKind::Thornbriar), 2);
    }

    #[test]
    fn unwitnessed_on_no_target() {
        let (mut world, _store) = stash_world();
        let mut inv = Inventory::default();
        inv.add_herb(HerbKind::Thornbriar);
        let outcome = run_resolver(&mut world, None, &mut inv, 20);
        assert!(!outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
        // Herb stays in inventory (items-are-real).
        assert_eq!(inv.slots.len(), 1);
    }

    #[test]
    fn unwitnessed_on_no_herbs_in_inventory() {
        let (mut world, store) = stash_world();
        let mut inv = Inventory::default();
        let outcome = run_resolver(&mut world, Some(store), &mut inv, 20);
        assert!(!outcome.witness);
        assert!(matches!(outcome.result, StepResult::Advance));
    }

    #[test]
    fn cap_miss_leaves_remainder_in_inventory() {
        let (mut world, store) = stash_world();
        // Pre-fill stash to cap.
        {
            let mut sh = world.get_mut::<StoredHerbs>(store).unwrap();
            sh.add(HerbKind::Thornbriar, 5, 5);
        }
        let mut inv = Inventory::default();
        inv.add_herb(HerbKind::Thornbriar);
        inv.add_herb(HerbKind::HealingMoss);
        // Thornbriar slot bounces (cap), HealingMoss absorbs.
        let outcome = run_resolver(&mut world, Some(store), &mut inv, 5);
        assert!(outcome.witness);
        assert_eq!(inv.slots.len(), 1, "Thornbriar remainder stays carried");
        assert!(inv.has_herb(HerbKind::Thornbriar));
        assert!(!inv.has_herb(HerbKind::HealingMoss));
        let stash = world.get::<StoredHerbs>(store).unwrap();
        assert_eq!(stash.count(HerbKind::Thornbriar), 5);
        assert_eq!(stash.count(HerbKind::HealingMoss), 1);
    }
}
