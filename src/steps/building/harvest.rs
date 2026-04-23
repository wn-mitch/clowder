use bevy_ecs::prelude::*;

use crate::components::building::{
    ConstructionSite, CropKind, CropState, StoredItems, Structure, StructureType,
};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{GrowthStage, Harvestable, Herb, HerbKind, Seasonal};
use crate::components::physical::Position;
use crate::resources::time::Season;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `HarvestCrops`
///
/// **Real-world effect** — for a mature (`growth >= 1.0`) Garden:
/// FoodCrops spawn Berries + Roots as `Item` entities in the
/// nearest Stores building; Thornbriar spawns a harvestable
/// `Herb` entity at the garden tile. On success, resets
/// `CropState.growth` to 0.0 so the tending cycle can restart.
///
/// **Plan-level preconditions** — emitted after `TendCrops`
/// reaches `growth >= 1.0` (per
/// `src/ai/planner/actions.rs::farming_actions`); the planner
/// does not know about Stores availability.
///
/// **Runtime preconditions** — requires `target_entity` to
/// resolve to a building with `CropState`. For FoodCrops, also
/// requires at least one Stores building to exist — if no stores
/// are available, returns `Fail("no stores for harvest")` and
/// **does not** reset growth, so the ripe crop can be harvested
/// later once a Stores is built (previously the step silently
/// reset growth while dropping the harvest on the floor). For
/// Thornbriar there are no store preconditions — the herb spawns
/// at the garden tile.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff at least one
/// item successfully landed (FoodCrops reached Stores, or
/// Thornbriar spawned). On Fail the witness is `false`.
///
/// **Feature emission** — caller passes `Feature::CropHarvested`
/// (Positive) to `record_if_witnessed`. §Phase 5a gated this on
/// the witness: before, it fired on every `Advance` regardless of
/// whether food actually reached Stores.
#[allow(clippy::type_complexity)]
pub fn resolve_harvest(
    target_entity: Option<Entity>,
    pos: &Position,
    stores_list: &[(Entity, Position)],
    buildings: &mut Query<
        (
            Entity,
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ),
        Without<crate::components::task_chain::TaskChain>,
    >,
    stored_items: &mut Query<&mut StoredItems>,
    commands: &mut Commands,
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Fail("no target for Harvest".into()));
    };

    let Ok((_, _, _, maybe_crop, garden_pos)) = buildings.get_mut(target) else {
        return StepOutcome::unwitnessed(StepResult::Fail("garden not found".into()));
    };
    let garden_pos = *garden_pos;

    let Some(mut crop) = maybe_crop else {
        return StepOutcome::unwitnessed(StepResult::Fail("no CropState on garden".into()));
    };

    match crop.crop_kind {
        CropKind::FoodCrops => {
            let Some(store_entity) = stores_list
                .iter()
                .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
                .map(|(e, _)| *e)
            else {
                // No Stores exist — don't reset growth; fail so the
                // plan drops and the next plan can be re-evaluated
                // once a Stores is built.
                return StepOutcome::unwitnessed(StepResult::Fail(
                    "no stores for harvest".into(),
                ));
            };
            let mut items_placed = 0u32;
            for kind in [ItemKind::Berries, ItemKind::Roots] {
                let item_entity = commands
                    .spawn(Item::new(kind, 0.9, ItemLocation::StoredIn(store_entity)))
                    .id();
                if let Ok(mut stored) = stored_items.get_mut(store_entity) {
                    stored.add(item_entity, StructureType::Stores);
                    items_placed += 1;
                }
            }
            if items_placed == 0 {
                // Stores entity existed but didn't accept items — don't
                // credit the harvest or reset growth.
                return StepOutcome::unwitnessed(StepResult::Fail(
                    "stores rejected harvest items".into(),
                ));
            }
            crop.growth = 0.0;
            StepOutcome::witnessed(StepResult::Advance)
        }
        CropKind::Thornbriar => {
            let available = vec![
                Season::Spring,
                Season::Summer,
                Season::Autumn,
                Season::Winter,
            ];
            commands.spawn((
                Herb {
                    kind: HerbKind::Thornbriar,
                    growth_stage: GrowthStage::Blossom,
                    magical: false,
                    twisted: false,
                },
                garden_pos,
                Seasonal { available },
                Harvestable,
            ));
            crop.growth = 0.0;
            StepOutcome::witnessed(StepResult::Advance)
        }
    }
}
