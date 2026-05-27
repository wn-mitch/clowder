//! `CraftAtWorkshop` — ticket 457 / parameterized in 463 commit 8.
//!
//! Workshop-craft resolver. Takes a `RecipeId` from the plan step
//! (the recipe identity flows from the held `Intention::Goal(HaveItem(_))`
//! through `craft_have_item_actions`'s plan template). Looks the
//! recipe up in the registry, drains its inputs from the actor's
//! `Inventory`, spawns the output per `Recipe.output.destination`, and
//! emits `Feature::ItemCrafted`. Retired the pre-463 lex-pick — the
//! resolver no longer chooses "best satisfied" recipe; the choice
//! happens upstream in the aspiration picker.

use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::components::recipe::{ItemDestination, RecipeId, StationRequirement};
use crate::resources::recipe_registry::RecipeRegistry;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `CraftAtWorkshop`
///
/// **Real-world effect** — consumes one Workshop recipe's full input
/// set from the actor's `Inventory` and adds the output item (Phase 2:
/// all six recipes use `ItemDestination::Inventory`). Recipe selection
/// is deterministic — lexicographic by `RecipeId.0`, first satisfied
/// wins.
///
/// **Plan-level preconditions** — emitted under `StatePredicate::ZoneIs(
/// PlannerZone::Workshop)` and `StatePredicate::HasMarker(
/// HasCraftInputInInventory::KEY)` by `crafting_actions` in
/// `src/ai/planner/actions.rs`. Cat eligibility + station availability
/// are gated upstream at `CraftAtWorkshopDse` (`CanCraft` +
/// `HasFunctionalWorkshop` + `HasCraftInputInInventory` + forbid
/// `Incapacitated`).
///
/// **Runtime preconditions** — re-checks that a Workshop exists within
/// `proximity` tiles AND that the actor's inventory satisfies *the
/// named recipe* in full. Both can drift between planning and
/// execution (cat may have dropped an input en route, or never gathered
/// the full set despite the `HasCraftInputInInventory` marker firing
/// on a single matching input). On either drift, returns
/// `unwitnessed(Fail)` — the planner re-picks.
///
/// **Witness** — `StepOutcome<RecipeId>`. Carries the parameterized
/// `recipe_id` on success (always equal to the `recipe_id` input —
/// kept for narrative + canary parity with the pre-463 lex-pick
/// witness shape). `unwitnessed(Fail)` paths return no witness — the
/// real-world effect didn't happen.
///
/// **Feature emission** — caller passes `Feature::ItemCrafted`
/// (Positive, expected_to_fire_per_soak = true — the first-light gate
/// for the 368 Phase 2 behavioral tools) to `record_if_witnessed`.
pub fn resolve_craft_at_workshop(
    recipe_id: Option<RecipeId>,
    cat_pos: Position,
    inventory: &mut Inventory,
    recipes: &RecipeRegistry,
    workshop_positions: &[Position],
    proximity: i32,
) -> StepOutcome<Option<RecipeId>> {
    resolve_craft_at_station(
        recipe_id,
        cat_pos,
        inventory,
        recipes,
        workshop_positions,
        StationRequirement::Workshop,
        "workshop",
        proximity,
    )
}

/// 369 / 463 commit 8: shared station-craft resolver. Takes a
/// `RecipeId` from the plan step (the recipe identity flows from the
/// held HaveItem Intention through `craft_have_item_actions`'s
/// templated plan). Verifies station proximity + the named recipe's
/// station + the named recipe's full input set, drains, spawns,
/// witnesses. Used by both `resolve_craft_at_workshop` (Workshop) and
/// `resolve_craft_at_tanning_frame` (TanningFrame). The
/// `station_label` is for failure-mode strings; the `station_filter`
/// is a defensive check that the named recipe's station matches the
/// arm — a registration mistake (e.g. naming a Kitchen recipe on the
/// Workshop arm) returns `Fail`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_craft_at_station(
    recipe_id: Option<RecipeId>,
    cat_pos: Position,
    inventory: &mut Inventory,
    recipes: &RecipeRegistry,
    station_positions: &[Position],
    station_filter: StationRequirement,
    station_label: &'static str,
    proximity: i32,
) -> StepOutcome<Option<RecipeId>> {
    let near_station = station_positions
        .iter()
        .any(|sp| cat_pos.manhattan_distance(sp) <= proximity);
    if !near_station {
        return StepOutcome::unwitnessed(StepResult::Fail(format!("no {station_label} in range")));
    }

    // 463 commit 8: `Some(id)` = HaveItem path (recipe pinned by the
    // held Intention). `None` = legacy fallback path (no held HaveItem)
    // — lex-pick the first inventory-satisfied recipe at this station.
    let chosen_id = match recipe_id {
        Some(id) => id,
        None => match pick_satisfied_recipe(recipes, inventory, station_filter) {
            Some(id) => id,
            None => {
                return StepOutcome::unwitnessed(StepResult::Fail(format!(
                    "no {station_label} recipe fully satisfied by inventory"
                )));
            }
        },
    };
    let Some(recipe) = recipes.get(chosen_id).cloned() else {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "{station_label}: recipe {} not in registry",
            chosen_id.0
        )));
    };
    if recipe.station != station_filter {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "{station_label}: recipe {} targets a different station ({:?})",
            chosen_id.0, recipe.station
        )));
    }
    if !recipe_inputs_satisfied(&recipe, inventory) {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "{station_label}: recipe {} inputs not satisfied by inventory",
            chosen_id.0
        )));
    }

    for input in &recipe.inputs {
        for _ in 0..input.count {
            let idx = inventory
                .slots
                .iter()
                .position(|s| s.kind == input.kind)
                .expect("input verified present by recipe_inputs_satisfied");
            inventory.slots.swap_remove(idx);
        }
    }

    match recipe.output.destination {
        ItemDestination::Inventory => {
            if !inventory.add_item(recipe.output.item_kind) {
                return StepOutcome::unwitnessed(StepResult::Fail(
                    "inventory full at output add (shouldn't happen post-consume)".into(),
                ));
            }
        }
        ItemDestination::EquippedSlot | ItemDestination::WorldPosition => {
            return StepOutcome::unwitnessed(StepResult::Fail(format!(
                "{station_label} recipe output destination not yet supported \
                 (Phase 2/2b are Inventory-only)"
            )));
        }
    }

    StepOutcome::witnessed_with(StepResult::Advance, chosen_id)
}

/// Lex-pick fallback for the legacy `CraftAt<Station>(None)` path —
/// the cat elected Crafting without a held HaveItem Intention. Walks
/// the recipe registry in lexicographic order (deterministic across
/// seeds), filters to the named station, returns the first recipe
/// whose inputs are all in inventory. Returns `None` when no recipe
/// is fully satisfied — the caller emits `Fail`.
fn pick_satisfied_recipe(
    recipes: &RecipeRegistry,
    inventory: &Inventory,
    station: StationRequirement,
) -> Option<RecipeId> {
    let mut candidates: Vec<&crate::components::recipe::Recipe> =
        recipes.iter().filter(|r| r.station == station).collect();
    candidates.sort_by_key(|r| r.id.0);
    candidates
        .into_iter()
        .find(|r| recipe_inputs_satisfied(r, inventory))
        .map(|r| r.id)
}

/// Helper: returns `true` iff every `RecipeInput { kind, count }` is
/// present in `inventory.slots` at sufficient count. Kept after the
/// 463 commit 8 lex-pick retirement because the resolver still
/// defensively re-checks before draining — the held Intention's
/// recipe may not match the inventory (inputs dropped en route, or
/// inventory shifted between plan + execute).
fn recipe_inputs_satisfied(
    recipe: &crate::components::recipe::Recipe,
    inventory: &Inventory,
) -> bool {
    for input in &recipe.inputs {
        let have = inventory
            .slots
            .iter()
            .filter(|s| s.kind == input.kind)
            .count() as u32;
        if have < input.count {
            return false;
        }
    }
    true
}
