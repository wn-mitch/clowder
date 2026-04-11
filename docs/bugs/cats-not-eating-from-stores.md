# Bug: Cats starve with full food stores

## Symptoms

In a 15-minute headless sim (seed 42), all 8 cats starve despite food stores sitting at 94% capacity (47/50). Nourishment declines linearly from 0.9 to 0.0 over days 100-109 with zero recovery — no cat ever successfully eats. Prey populations stay at cap (245 total) throughout. Zero injury deaths (wards are working). This is purely a feeding failure.

## Timeline (seed 42, 15-min run)

- **Day 100**: 8 cats, nourishment 0.9, stores 47/50
- **Day 104**: nourishment 0.5, stores 47/50 (unchanged — nobody withdrawing)
- **Day 109**: nourishment ~0.08, stores 47/50
- **Day 110**: cascade — 4 cats starve in rapid succession
- **Day 111**: 3 more die, stores drop to 13 (item spoilage, not consumption)
- **Day 117**: last cat (Calcifer) starves

## Not the cause

- **Corruption prey suppression**: prey populations never dropped below 243. No corruption-related prey depletion.
- **Shadow fox ambushes**: 0 injury deaths. Wards are functional.
- **Food doesn't exist**: stores are 94% full. The food is there; cats aren't accessing it.

## Investigation starting points

### 1. EatAtStores resolver — does it complete?

`src/systems/goap.rs` around the `GoapActionKind::EatAtStores` dispatch:

```rust
GoapActionKind::EatAtStores => {
    if plan.step_state[step_idx].target_entity.is_none() {
        plan.step_state[step_idx].target_entity = stores_entities.iter()
            .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
            .map(|(e, _)| *e);
    }
    crate::steps::disposition::resolve_eat_at_stores(
        ticks,
        plan.step_state[step_idx].target_entity,
        &mut needs,
        &mut stores_query,
        &items_query,
        &mut commands,
        d,
    )
}
```

Questions:
- Is `stores_entities` populated? It's built from `building_snapshot` filtering `StructureType::Stores`. If no building has kind `Stores`, the vec is empty and target resolution fails.
- Does `resolve_eat_at_stores` actually advance? Check `src/steps/disposition/eat_at_stores.rs`.
- Does the cat ever get assigned `DispositionKind::Resting` with an `EatAtStores` step? The scoring system drives Eat via hunger urgency, but does it make it through disposition selection → plan generation → step execution?

### 2. Does Resting disposition ever win?

The nourishment decline is perfectly smooth — no blip where a cat briefly ate. This suggests cats never enter Resting disposition at all, or the EatAtStores action is never in their plans.

Check `evaluate_and_plan`: is `food_available` true? It reads `!res.food.is_empty()`. If `FoodStores` resource is empty (separate from the `StoredItems` component on the stores building), cats won't even score Eat.

Key question: **are `FoodStores` (the resource) and `StoredItems` (the component) in sync?** The stores building might have items but the `FoodStores` resource might not know about them.

### 3. Stores building existence

Does a `Structure` with `kind == StructureType::Stores` exist? The `building_snapshot` in `resolve_goap_plans` filters for it. If no stores structure spawned, `stores_entities` is empty and `stores_positions` is empty, so:
- Zone distances to `PlannerZone::Stores` are never set
- The planner can't route to stores
- EatAtStores is unreachable

### 4. Eat-from-inventory path

Cats can also eat carried prey directly when hunger is low (threshold `eat_from_inventory_threshold: 0.4`). Check if this path fires — it would show as nourishment recovery even without stores access. The fact that it doesn't fire either suggests cats aren't carrying food.

## Possible root causes (ranked by likelihood)

1. **FoodStores resource not synced with StoredItems** — resource says empty, component says full. Eat never scores.
2. **No Stores building exists** — stores structure never spawned or was despawned. Zone distances for Stores are missing, planner can't route.
3. **Eat scores but Resting disposition never wins** — other dispositions (Hunt, Socialize, Patrol) always outscore Resting in softmax. Check `deaths_starvation` climbing but `survival_floor` not kicking in.
4. **TravelTo(Stores) fails** — cat plans to eat, gets the step, but pathing to stores fails and plan is abandoned.

## Diagnostic instrumentation to add

- Feature tracking: `EatAtStoresCompleted`, `EatAtStoresFailed` in the dispatch
- Log `food_available` and `food_fraction` in the scoring context when hunger < 0.3
- Log when a cat's Resting plan includes EatAtStores vs only Sleep/Groom
