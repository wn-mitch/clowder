---
id: 186
title: add_effective Bevy command-buffer race silently drops capacity_bonus on just-spawned items
status: ready
cluster: null
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`StoredItems::add_effective` in `src/components/building.rs:409-422`
calls `is_effectively_full` → `effective_capacity_with_items`
which queries `items_q.get(entity)` on every entity in
`self.items` (`src/components/building.rs:386-394`):

```rust
let bonus: usize = stored
    .iter()
    .filter_map(|&e| items_q.get(e).ok())
    .map(|item| item.kind.capacity_bonus())
    .sum();
```

When called from `deposit_at_stores.rs:140-184`, the calling
sequence is:

```rust
let item_entity = commands.spawn(Item::with_modifiers(...)).id();
if !stored.add_effective(item_entity, StructureType::Stores, items_query) {
    commands.entity(item_entity).despawn();
    rejected = true;
    break;
}
```

`commands.spawn(...).id()` allocates an entity ID but the
component bundle is **buffered in the commands queue** until the
next system flush. During the same tick's `add_effective` call,
`items_query.get(item_entity)` returns `Err` because the `Item`
component isn't yet visible to queries. The `filter_map(... .ok())`
silently drops the entity from the bonus calculation.

For **food items** this is harmless — `ItemKind::is_food()` items
have `capacity_bonus() == 0`, so the missed contribution is zero.
For **storage-upgrade items** (e.g., woven baskets, capacity_bonus > 0),
the just-deposited item's bonus is invisible to its own
fullness check on the **second** deposit in the same loop. If
`self.items.len() == base_capacity` after the basket push and a
food item follows, `is_effectively_full` returns true (because
the basket's bonus isn't counted), and the food item is rejected
+ despawned. **Silent loss path** for storage upgrades.

This was identified during ticket 184's diagnostic investigation
but is **orthogonal** to 184's stockpile observation (foods, not
upgrades). Tracked as a separate latent defect.

## Scope

- Repair `add_effective` to not silently miss the just-pushed
  item's `capacity_bonus`. Two viable shapes:
  1. **Eager cache**: `add_effective` accepts the new item's
     `Item` reference (or `ItemKind` + `quality` enough to compute
     `capacity_bonus`) and folds it into the capacity check
     directly, sidestepping the query lookup for the in-flight
     entity.
  2. **Apply commands first**: route deposit through a
     pre-flush helper that ensures the spawned entity is visible
     before `add_effective` runs. More invasive; would touch the
     commands queue contract.
- Update tests at `src/components/building.rs:625-700` to cover
  the basket-then-food deposit-loop scenario (currently no test
  exercises this race).
- Add a `cargo test --lib components::building::tests` regression
  case for the race specifically — assert that depositing a
  basket + food in one resolve_deposit_at_stores call lands both.

## Out of scope

- General audit of similar command-buffer races in other deposit
  / spawn / query patterns elsewhere in the codebase. If 186
  surfaces a pattern, open a follow-on for systematic review.
- Migrating deposit_at_stores to the typed
  `transfer_item_inventory_to_stored` primitive in
  `src/components/item_transfer.rs` — that's a separate
  refactor mentioned in 175's closeout.
- Changing the items-are-real contract or the
  StoredItems API shape.

## Current state

- Defect identified during 184 layer-walk audit
  ([`docs/open-work/tickets/184-kill-deposit-pipeline-regression-post-175.md`](184-kill-deposit-pipeline-regression-post-175.md)).
- No observed user-facing symptoms in the 184 soak
  (`StorageUpgraded == 0` in both pre-181 and post-181 runs,
  consistent with no baskets being deposited successfully — the
  race is masked by the absence of basket-deposit attempts).
- Will become user-visible the moment the colony starts
  consistently making baskets (which the
  build-stores-demand-wiring ticket 179 eventually enables).

## Approach

Preferred shape: **eager cache** (option 1 above). Modify
`add_effective` to accept the in-flight item's `capacity_bonus`
explicitly:

```rust
pub fn add_effective_with_bonus(
    &mut self,
    item: Entity,
    item_capacity_bonus: usize,
    kind: StructureType,
    items_q: &Query<&Item, ...>,
) -> bool {
    let base = Self::capacity(kind);
    let existing_bonus: usize = self.items.iter()
        .filter_map(|&e| items_q.get(e).ok())
        .map(|i| i.kind.capacity_bonus())
        .sum();
    let total_capacity = base + existing_bonus + item_capacity_bonus;
    if self.items.len() >= total_capacity {
        return false;
    }
    self.items.push(item);
    true
}
```

Caller in `deposit_at_stores.rs:140-184` provides
`kind.capacity_bonus()` from the inventory slot directly (no
query lookup needed; `ItemKind` is value-typed in `ItemSlot::Item`).

Keep `add_effective` (without the bonus parameter) as a
deprecated thin wrapper that forwards `0` for the in-flight bonus,
to avoid touching every callsite simultaneously. Migrate one
callsite per commit.

## Verification

- New unit test in `src/components/building.rs::tests`:
  - Setup: empty `StoredItems`, capacity 50
  - Spawn basket entity (capacity_bonus = 10) + 50 food items in
    one `add_effective_with_bonus` loop
  - Assertion: all 51 items land (basket + 50 food, total cap
    50 + 10 = 60)
- `just check` and `just test` pass.
- Soak-level: once 179 (build-stores-demand-wiring) lands, basket
  deposits will be observable. Confirm via `feature_count(StorageUpgraded) > 0`
  in a soak where buildings produce baskets.

## Log

- 2026-05-06: opened from ticket 184's audit. Real defect, no
  current user-visible symptom (no basket deposits in the soak
  data), but a known silent-loss path the moment basket deposits
  start. Tracked separately so 184 can close cleanly as
  "no defect for the food-stockpile observation."
