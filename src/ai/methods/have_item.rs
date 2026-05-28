//! HTN substrate for `GoalKind::HaveItem(ItemKind)` decomposition
//! (ticket 462 — child of the 462/463 item-aspiration arc).
//!
//! 462's substrate-only landing: ships the decomposition helper that
//! synthesizes a per-recipe craft plan from a held `HaveItem`
//! Intention. 463 lifts the dormant weight by emitting
//! `Intention::Goal(HaveItem(_))` from a `CraftItemAspiration` chain
//! and wiring the goal-advance hook to call this helper.
//!
//! ## Why a standalone helper instead of a `Method` registry entry
//!
//! `Method.sub_goals: &'static [SubGoal]` is structurally static —
//! a templated method whose decomposition varies per `ItemKind`
//! cannot fit that shape directly. Two alternatives — (a) a Method-
//! enum-variant carrying a `fn` decomposition, (b) one method per
//! ItemKind with a per-recipe `TargetHint::SpecificRecipe` — both
//! touch substrate-sensitive surfaces (the method registry or the
//! DSE target-resolution surface) and would violate the "ships
//! dormant, zero behavior change" invariant 462 holds.
//!
//! The standalone helper preserves substrate honesty: the per-
//! recipe decomposition is computed at decomposition time from
//! canonical RecipeRegistry data, with the L2 trace reading the
//! recipe identity directly. 463 picks the wiring shape (Method-
//! registry restructure OR goal-advance-hook interception OR
//! per-ItemKind static-method registration) when it lifts the
//! weight.

use crate::ai::planner::{GoapActionKind, PlannerZone};
use crate::components::items::ItemKind;
use crate::components::recipe::{Recipe, StationRequirement};
use crate::resources::recipe_registry::RecipeRegistry;

/// Map a recipe's `StationRequirement` to the planner zone the cat
/// must travel to to execute the craft action. Returns `None` for
/// `StationRequirement::None` (no station — current recipes that
/// fit are herbcraft-style, which run through their own DSE chain
/// rather than `Action::Craft`; HaveItem-style aspirations don't
/// currently target stationless recipes).
const fn station_to_zone(station: StationRequirement) -> Option<PlannerZone> {
    match station {
        StationRequirement::None => None,
        StationRequirement::Workshop => Some(PlannerZone::Workshop),
        StationRequirement::Kitchen => Some(PlannerZone::Kitchen),
        StationRequirement::DryingRack => Some(PlannerZone::DryingRack),
        StationRequirement::SmokingRack => Some(PlannerZone::SmokingRack),
        StationRequirement::TanningFrame => Some(PlannerZone::TanningFrame),
    }
}

/// Map a recipe's `StationRequirement` to the `GoapActionKind` that
/// executes the craft at that station. Returns `None` for stations
/// without a HaveItem-style craft action (`None`, `Kitchen`,
/// `DryingRack`, `SmokingRack` — these have their own dedicated
/// plan-template flows: `Cook`, `DryFood`, `SmokeMeat`).
///
/// 462 supports two HaveItem-decomposable stations:
/// - `Workshop` → `CraftAtWorkshop` (Phase 2 behavioral tools; 8 of 8
///   warrior's-kit items today route through Workshop or TanningFrame).
/// - `TanningFrame` → `CraftAtTanningFrame` (Phase 2b hide armor).
///
/// Future Phase ≥3 station-recipes (wearables / decorations / etc.)
/// extend this match arm when they author dedicated craft actions.
const fn station_to_craft_action(
    station: StationRequirement,
    recipe_id: crate::components::recipe::RecipeId,
) -> Option<GoapActionKind> {
    match station {
        StationRequirement::Workshop => Some(GoapActionKind::CraftAtWorkshop(recipe_id)),
        StationRequirement::TanningFrame => Some(GoapActionKind::CraftAtTanningFrame(recipe_id)),
        StationRequirement::None
        | StationRequirement::Kitchen
        | StationRequirement::DryingRack
        | StationRequirement::SmokingRack => None,
    }
}

/// Synthesize the per-recipe GOAP plan that satisfies
/// `Intention::Goal(GoalKind::HaveItem(item))`. The plan shape is:
///
/// ```text
/// [
///     RetrieveCraftInputs(recipe.id),
///     TravelTo(recipe.station_zone),
///     CraftAt<station>(recipe.id-implicit-via-station-resolver),
/// ]
/// ```
///
/// The retrieve step is parameterized by `recipe.id` so its resolver
/// pulls the recipe's specific inputs (preserving per-input counts).
/// The travel step picks the planner zone matching the recipe's
/// station. The craft step is the existing station-keyed action
/// (`CraftAtWorkshop` / `CraftAtTanningFrame`); both DSEs read the
/// inventory + recipe registry to pick the actual recipe at execute
/// time. The `RetrieveCraftInputs` prefix guarantees the inventory
/// carries the recipe-specific inputs *before* the craft action
/// scans for a satisfied recipe — so the lex-pick at the craft
/// resolver becomes a tautology once 463 emits HaveItem (only the
/// targeted recipe's inputs are present).
///
/// Returns `None` when:
/// - No recipe in the registry produces `item` (registration bug or
///   item is foraged / hunted / etc. rather than crafted).
/// - The recipe's station lacks a HaveItem-decomposable craft action
///   (Phase ≥3 stations whose dedicated craft actions haven't been
///   authored yet).
///
/// Dormant in 462 — no caller invokes this until 463 wires it into
/// the goal-advance hook (or, equivalently, the `Action::Craft` plan-
/// template builder reads the held `HaveItem` Intention and calls
/// this helper).
pub fn decompose_goal_have_item(
    item: ItemKind,
    recipes: &RecipeRegistry,
) -> Option<Vec<GoapActionKind>> {
    let recipe: &Recipe = recipes.recipe_producing(item)?;
    let zone = station_to_zone(recipe.station)?;
    let craft_action = station_to_craft_action(recipe.station, recipe.id)?;
    Some(vec![
        GoapActionKind::RetrieveCraftInputs(recipe.id),
        GoapActionKind::TravelTo(zone),
        craft_action,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::recipe::{
        DisciplineKind, ItemDestination, Recipe, RecipeDuration, RecipeId, RecipeInput,
        RecipeOutput, StationRequirement,
    };

    fn workshop_recipe(id: &'static str, output: ItemKind, inputs: Vec<RecipeInput>) -> Recipe {
        Recipe {
            id: RecipeId(id),
            discipline: DisciplineKind::BoneShellCraft,
            inputs,
            station: StationRequirement::Workshop,
            duration: RecipeDuration::Fixed { ticks: 10 },
            output: RecipeOutput {
                item_kind: output,
                destination: ItemDestination::Inventory,
            },
            skill_gate: None,
            is_warriors_kit: false,
            discipline_skill_affinity: None,
        }
    }

    #[test]
    fn bone_tip_spear_decomposes_to_per_recipe_plan() {
        // Synthetic recipe registry: one recipe producing
        // BoneTipSpear from one Bone + one Sinew at the Workshop.
        let mut recipes = RecipeRegistry::default();
        recipes.insert(workshop_recipe(
            "bone_tip_spear",
            ItemKind::BoneTipSpear,
            vec![
                RecipeInput {
                    kind: ItemKind::Bone,
                    count: 1,
                },
                RecipeInput {
                    kind: ItemKind::Sinew,
                    count: 1,
                },
            ],
        ));

        let plan = decompose_goal_have_item(ItemKind::BoneTipSpear, &recipes)
            .expect("BoneTipSpear has a recipe → decomposition must produce a plan");

        assert_eq!(plan.len(), 3, "plan is [retrieve, travel, craft]");
        assert_eq!(
            plan[0],
            GoapActionKind::RetrieveCraftInputs(RecipeId("bone_tip_spear")),
            "first step retrieves the specific recipe's inputs — NOT a generic any-craft-input set",
        );
        assert_eq!(
            plan[1],
            GoapActionKind::TravelTo(PlannerZone::Workshop),
            "second step travels to the Workshop (the recipe's station)",
        );
        assert_eq!(
            plan[2],
            GoapActionKind::CraftAtWorkshop(RecipeId("bone_tip_spear")),
            "third step executes the Workshop craft action parameterized \
             with the same RecipeId the retrieve step used",
        );
    }

    #[test]
    fn tanning_frame_recipe_routes_to_tanning_frame_craft() {
        let mut recipes = RecipeRegistry::default();
        recipes.insert(Recipe {
            id: RecipeId("hide_bracers"),
            discipline: DisciplineKind::BoneShellCraft,
            inputs: vec![RecipeInput {
                kind: ItemKind::Hide,
                count: 2,
            }],
            station: StationRequirement::TanningFrame,
            duration: RecipeDuration::Fixed { ticks: 10 },
            output: RecipeOutput {
                item_kind: ItemKind::HideBracers,
                destination: ItemDestination::EquippedSlot,
            },
            skill_gate: None,
            is_warriors_kit: false,
            discipline_skill_affinity: None,
        });

        let plan = decompose_goal_have_item(ItemKind::HideBracers, &recipes).unwrap();

        assert_eq!(
            plan[0],
            GoapActionKind::RetrieveCraftInputs(RecipeId("hide_bracers"))
        );
        assert_eq!(plan[1], GoapActionKind::TravelTo(PlannerZone::TanningFrame));
        assert_eq!(
            plan[2],
            GoapActionKind::CraftAtTanningFrame(RecipeId("hide_bracers"))
        );
    }

    #[test]
    fn missing_recipe_returns_none() {
        let recipes = RecipeRegistry::default();
        // No recipe produces BoneTipSpear in this empty registry.
        assert!(decompose_goal_have_item(ItemKind::BoneTipSpear, &recipes).is_none());
    }

    #[test]
    fn stationless_recipe_returns_none() {
        // A `StationRequirement::None` recipe (e.g. ward-setting
        // shape) has no HaveItem-decomposable plan — those run
        // through their own DSE chains, not Action::Craft.
        let mut recipes = RecipeRegistry::default();
        recipes.insert(Recipe {
            id: RecipeId("ward_setting_no_station"),
            discipline: DisciplineKind::BoneShellCraft,
            inputs: vec![RecipeInput {
                kind: ItemKind::HerbThornbriar,
                count: 1,
            }],
            station: StationRequirement::None,
            duration: RecipeDuration::Fixed { ticks: 1 },
            output: RecipeOutput {
                item_kind: ItemKind::ShinyPebble, // arbitrary non-Workshop output
                destination: ItemDestination::WorldPosition,
            },
            skill_gate: None,
            is_warriors_kit: false,
            discipline_skill_affinity: None,
        });

        assert!(decompose_goal_have_item(ItemKind::ShinyPebble, &recipes).is_none());
    }
}
