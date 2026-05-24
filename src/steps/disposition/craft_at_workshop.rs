//! `CraftAtWorkshop` — ticket 457, Phase 2 behavioral-tool crafting.
//!
//! Generalised Workshop-craft resolver. Iterates the recipe registry
//! for `StationRequirement::Workshop` recipes in lexicographic order
//! (deterministic across seeds), picks the first recipe whose full
//! input set is in the actor's `Inventory`, drains the inputs, spawns
//! the output item per `Recipe.output.destination`, and emits
//! `Feature::ItemCrafted`. Replaces the 322 / #334 dormant stub that
//! `craft.rs` carried — 457 is the first live user, with #334's
//! StealthCloak inheriting the same pipeline when it ships its recipe.

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
/// `proximity` tiles and that the cat's inventory satisfies at least
/// one Workshop recipe in full. Both can drift between planning and
/// execution (cat may have dropped an input en route, or never gathered
/// the full set despite carrying *some* input). On either drift,
/// returns `unwitnessed(Fail)` — the planner re-picks.
///
/// **Witness** — `StepOutcome<RecipeId>`. Carries the chosen recipe
/// on success so the caller can route narrative + canary emission
/// per-recipe. `unwitnessed(Fail)` paths return no witness — the
/// real-world effect didn't happen.
///
/// **Feature emission** — caller passes `Feature::ItemCrafted`
/// (Positive, expected_to_fire_per_soak = true — the first-light gate
/// for the 368 Phase 2 behavioral tools) to `record_if_witnessed`.
pub fn resolve_craft_at_workshop(
    cat_pos: Position,
    inventory: &mut Inventory,
    recipes: &RecipeRegistry,
    workshop_positions: &[Position],
    proximity: i32,
) -> StepOutcome<Option<RecipeId>> {
    resolve_craft_at_station(
        cat_pos,
        inventory,
        recipes,
        workshop_positions,
        StationRequirement::Workshop,
        "workshop",
        proximity,
    )
}

/// 369: shared station-craft resolver. Same shape as
/// `resolve_craft_at_workshop` was pre-369 — proximity check,
/// lex-order recipe pick, input drain, output add — parameterised by
/// the station whose recipes we're picking. Used by both
/// `resolve_craft_at_workshop` (Workshop station) and
/// `resolve_craft_at_tanning_frame` (TanningFrame station). The
/// `station_label` is for the failure-mode `Fail("no <label> in
/// range")` string; the `station_filter` discriminates the recipe set.
pub(crate) fn resolve_craft_at_station(
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

    let chosen = pick_satisfied_recipe(recipes, inventory, station_filter);
    let Some(recipe_id) = chosen else {
        return StepOutcome::unwitnessed(StepResult::Fail(format!(
            "no {station_label} recipe fully satisfied by inventory"
        )));
    };

    let recipe = recipes
        .get(recipe_id)
        .expect("RecipeId returned from registry must resolve")
        .clone();

    for input in &recipe.inputs {
        for _ in 0..input.count {
            let idx = inventory
                .slots
                .iter()
                .position(|s| s.kind == input.kind)
                .expect("input verified present by pick_satisfied_recipe");
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

    StepOutcome::witnessed_with(StepResult::Advance, recipe_id)
}

/// Walk the recipe registry in deterministic order (lexicographic by
/// RecipeId) and return the first recipe matching `station` whose
/// inputs are all in inventory at sufficient counts. Returns `None`
/// when no recipe is satisfied. Deterministic ordering matters for
/// seed-42 reproducibility — the registry's `HashMap` doesn't give us
/// that, so we sort here.
fn pick_satisfied_recipe(
    recipes: &RecipeRegistry,
    inventory: &Inventory,
    station: StationRequirement,
) -> Option<RecipeId> {
    let mut candidates: Vec<&crate::components::recipe::Recipe> =
        recipes.iter().filter(|r| r.station == station).collect();
    candidates.sort_by_key(|r| r.id.0);
    for recipe in candidates {
        if recipe_inputs_satisfied(recipe, inventory) {
            return Some(recipe.id);
        }
    }
    None
}

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
