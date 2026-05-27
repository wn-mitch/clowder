//! `CraftAtTanningFrame` — ticket 369, Phase 2b hide-armor crafting.
//!
//! Sibling resolver to [`resolve_craft_at_workshop`]. Picks any
//! `StationRequirement::TanningFrame` recipe (HideBracers,
//! HidePlatedWrap) whose full input set is in inventory, drains
//! the inputs, spawns the output. Delegates to the shared
//! `resolve_craft_at_station` helper.
//!
//! The DSE eligibility filter (`CraftAtTanningFrameDse`) already
//! gates on `HasFunctionalTanningFrame` (colony) +
//! `HasCraftInputInInventory` (per-cat), so the resolver's runtime
//! checks here are belt-and-braces: re-verify proximity and
//! satisfaction at execute time.

use super::craft_at_workshop::resolve_craft_at_station;
use crate::components::magic::Inventory;
use crate::components::physical::Position;
use crate::components::recipe::{RecipeId, StationRequirement};
use crate::resources::recipe_registry::RecipeRegistry;
use crate::steps::StepOutcome;

/// # GOAP step resolver: `CraftAtTanningFrame`
///
/// **Real-world effect** — consumes one TanningFrame recipe's full
/// input set from the actor's `Inventory` and adds the output
/// `Item` (Phase 2b hide armor: HideBracers / HidePlatedWrap; both
/// use `ItemDestination::Inventory`). Recipe selection is
/// deterministic — lexicographic by `RecipeId.0`, first satisfied
/// wins (same shape as `resolve_craft_at_workshop`).
///
/// **Plan-level preconditions** — emitted under
/// `StatePredicate::ZoneIs(PlannerZone::TanningFrame)` and
/// `StatePredicate::HasMarker(HasCraftInputInInventory::KEY)` by
/// `crafting_actions` in `src/ai/planner/actions.rs`. Cat
/// eligibility + station availability are gated upstream at
/// `CraftAtTanningFrameDse`.
///
/// **Runtime preconditions** — re-checks that a TanningFrame exists
/// within `proximity` tiles and that the cat's inventory satisfies
/// at least one TanningFrame recipe in full. Both can drift between
/// planning and execution — on either drift, returns
/// `unwitnessed(Fail)`.
///
/// **Witness** — `StepOutcome<Option<RecipeId>>`. Carries the chosen
/// recipe on success.
///
/// **Feature emission** — caller passes `Feature::ItemCrafted` to
/// `record_if_witnessed` (same feature variant as Workshop crafting
/// — the canary is "any item got crafted," not per-station).
pub fn resolve_craft_at_tanning_frame(
    recipe_id: Option<RecipeId>,
    cat_pos: Position,
    inventory: &mut Inventory,
    recipes: &RecipeRegistry,
    tanning_frame_positions: &[Position],
    proximity: i32,
) -> StepOutcome<Option<RecipeId>> {
    resolve_craft_at_station(
        recipe_id,
        cat_pos,
        inventory,
        recipes,
        tanning_frame_positions,
        StationRequirement::TanningFrame,
        "tanning frame",
        proximity,
    )
}
