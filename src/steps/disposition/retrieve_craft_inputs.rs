use bevy_ecs::prelude::*;

use crate::components::building::StoredItems;
use crate::components::item_transfer::{transfer_item_stores_to_inventory, TransferError};
use crate::components::items::Item;
use crate::components::magic::Inventory;
use crate::components::recipe::RecipeId;
use crate::resources::recipe_registry::RecipeRegistry;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `RetrieveCraftInputs(RecipeId)`
///
/// **Real-world effect** — for the given recipe, transfers each
/// `RecipeInput { kind, count }` from the target Stores building into
/// the cat's `Inventory` until inventory carries at least `count`
/// copies of `kind` (or stores runs out). Items the cat already
/// carries are counted toward the requirement and skipped at
/// retrieve time. Sibling shape to `RetrieveSmokeable` (which
/// hard-codes the two-ingredient meat+fuel pair); this resolver
/// parameterizes over arbitrary recipe input sets.
///
/// **Plan-level preconditions** — emitted by `Action::Craft`'s plan
/// template when the cat holds an `Intention::Goal(HaveItem(_))`
/// Intention (462 substrate; emitted by 463+). The template
/// synthesizes `[RetrieveCraftInputs(recipe.id), TravelTo(station),
/// CraftAt<station>(recipe.id)]` so the retrieve step always runs
/// at a Stores zone. Dormant in 462 — no plan template emits this
/// variant yet.
///
/// **Runtime preconditions** — waits `ticks >= 5`. Requires the
/// recipe to resolve in `RecipeRegistry` (a missing recipe is a
/// registration bug, surfaced as `unwitnessed(Fail)` so the HTN
/// method abandons). Requires `target_entity` to resolve to a
/// `StoredItems`. For each input, transfer routes through
/// `components::item_transfer::transfer_item_stores_to_inventory`
/// per the items-are-real contract — capacity-checked before
/// `stored.remove` / `commands.entity(_).despawn()`. On inventory-
/// full: `unwitnessed(Fail)` so the cat re-plans rather than
/// silently destroying a real item entity. On no-target /
/// Stores-not-found: returns `unwitnessed(Advance)` — the chain
/// moves on (the target Stores went away mid-plan).
///
/// 468: at end-of-loop, if the pouch still doesn't satisfy the
/// recipe (Stores ran out of an input between the aspiration
/// picker's reachable-snapshot and this tick), returns
/// `unwitnessed(Fail("stores missing recipe input"))` rather than
/// `Advance`-ing into a downstream `CraftAt<station>` Fail. Forces
/// a clean re-plan via the aspiration picker's fresh Stores
/// snapshot.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff at least one
/// item was actually transferred from Stores to inventory this
/// call. Already-carried inputs do not witness (no real-world
/// effect this tick).
///
/// **Feature emission** — caller passes `Feature::ItemRetrieved`
/// (Positive) to `record_if_witnessed`. Same Feature as the
/// other retrieve resolvers (`RetrieveRawFood` /
/// `RetrieveSmokeable` / `RetrieveDryable`).
#[allow(clippy::too_many_arguments)]
pub fn resolve_retrieve_craft_inputs(
    recipe_id: RecipeId,
    recipes: &RecipeRegistry,
    ticks: u64,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    commands: &mut Commands,
) -> StepOutcome<bool> {
    if ticks < 5 {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    let Some(recipe) = recipes.get(recipe_id) else {
        // Registration bug — the planner emitted a recipe id with
        // no entry in the registry. HTN abandons rather than
        // silently no-op'ing on a broken plan.
        return StepOutcome::unwitnessed(StepResult::Fail(
            "unknown recipe id in RetrieveCraftInputs".into(),
        ));
    };
    let Some(store_entity) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    let Ok(mut stored) = stores_query.get_mut(store_entity) else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };

    let mut transferred = false;

    for input in &recipe.inputs {
        let required: usize = input.count as usize;
        let carried = inventory
            .pouch
            .iter()
            .filter(|s| s.kind == input.kind)
            .count();
        if carried >= required {
            continue;
        }
        let mut remaining = required - carried;
        while remaining > 0 {
            let candidate = stored
                .items
                .iter()
                .copied()
                .find(|&e| items_query.get(e).is_ok_and(|item| item.kind == input.kind));
            let Some(item_entity) = candidate else {
                break;
            };
            let Ok(item) = items_query.get(item_entity) else {
                break;
            };
            match transfer_item_stores_to_inventory(
                &mut stored,
                item_entity,
                item.kind,
                item.modifiers,
                inventory,
                commands,
            ) {
                Ok(()) => {
                    transferred = true;
                    remaining -= 1;
                }
                Err(TransferError::DestinationFull) => {
                    return StepOutcome::unwitnessed(StepResult::Fail(
                        "inventory full for craft input".into(),
                    ));
                }
            }
        }
    }

    // 468: surface partial-supply failures at this layer rather than
    // letting them cascade into a guaranteed downstream
    // `CraftAt<station>` Fail. If pouch still doesn't satisfy the
    // recipe, the aspiration picker's reachable-snapshot drifted
    // (another cat claimed the last Stones, item decayed, etc.) —
    // fail cleanly so the planner re-evaluates.
    if !inventory.satisfies_recipe(recipe) {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "stores missing recipe input for {}",
            recipe_id.0
        )));
    }
    if transferred {
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        // Pouch already satisfied the recipe at entry — retrieve had
        // nothing to do but the chain is still valid.
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}
