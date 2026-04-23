use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::ai::pathfinding::find_path;
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::physical::Position;
use crate::components::skills::Skills;
use crate::resources::colony_score::ColonyScore;
use crate::resources::map::TileMap;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Construct`
///
/// **Real-world effect** — paths the actor to a `ConstructionSite`,
/// then increments `site.progress` by a rate scaled by
/// `skills.building` + a group-build bonus for multiple concurrent
/// builders. On completion (`progress >= 1.0`): removes the
/// `ConstructionSite`, installs the final `Structure` and any
/// required auxiliary components (`StoredItems` for Stores,
/// `CropState` for Garden — missing-auxiliary was the §Phase 4c.4
/// farming repair), and increments `ColonyScore.structures_built`.
///
/// **Plan-level preconditions** — emitted by the builder-chain
/// planner in `src/systems/coordination.rs`. Requires that
/// `Deliver` steps have already fulfilled the site's materials
/// ledger — otherwise `Fail("materials not delivered")` drops the
/// plan.
///
/// **Runtime preconditions** — `target_entity` must resolve to a
/// site; if missing or not a ConstructionSite, returns Fail. The
/// ConstructionSite-already-removed case returns `Advance`
/// directly (idempotent completion).
///
/// **Witness** — `StepOutcome<()>`. The effect is either a real
/// progress increment or a Fail; no silent-advance surface.
///
/// **Feature emission** — caller records
/// `Feature::BuildingConstructed` (Positive) on `Advance` at
/// `src/systems/goap.rs`. Since the step can only Advance when
/// progress actually completed or the site was already removed,
/// gating on `Advance` is acceptable here — no witness-less
/// Advance path silently misfires.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn resolve_construct(
    target_entity: Option<Entity>,
    pos: &mut Position,
    cached_path: &mut Option<Vec<Position>>,
    skills: &mut Skills,
    workshop_bonus: f32,
    builders_per_site: &HashMap<Entity, usize>,
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
    map: &TileMap,
    commands: &mut Commands,
    colony_score: &mut Option<ResMut<ColonyScore>>,
) -> StepOutcome<()> {
    let Some(target) = target_entity else {
        return StepOutcome::bare(StepResult::Fail("no target for Construct".into()));
    };

    let Ok((_, _, maybe_site, _, building_pos)) = buildings.get_mut(target) else {
        return StepOutcome::bare(StepResult::Fail("construction site not found".into()));
    };

    if pos.manhattan_distance(building_pos) > 1 {
        if cached_path.is_none() {
            *cached_path = find_path(*pos, *building_pos, map);
        }
        if let Some(ref mut path) = cached_path {
            if !path.is_empty() {
                *pos = path.remove(0);
            }
        }
        return StepOutcome::bare(StepResult::Continue);
    }

    if let Some(mut site) = maybe_site {
        if !site.materials_complete() {
            return StepOutcome::bare(StepResult::Fail("materials not delivered".into()));
        }

        let other_builders = builders_per_site.get(&target).copied().unwrap_or(1).max(1);
        let rate = skills.building * 0.02 * (1.0 + 0.3 * (other_builders as f32 - 1.0));
        site.progress = (site.progress + rate).min(1.0);
        skills.building += skills.growth_rate() * 0.01 * workshop_bonus;

        if site.progress >= 1.0 {
            let blueprint = site.blueprint;
            commands.entity(target).remove::<ConstructionSite>();
            commands
                .entity(target)
                .remove::<crate::rendering::entity_sprites::EntitySpriteMarker>();
            commands.entity(target).insert(Structure::new(blueprint));
            if blueprint == StructureType::Stores {
                commands.entity(target).insert(StoredItems::default());
            }
            // §Phase 4c.4 farming repair: Gardens used to ship without a
            // `CropState` component, so `TendCrops`'s target-resolution
            // query (`has_crop` filter) never matched any Garden and
            // every Farming plan failed with "no target for Tend" —
            // 400+/soak silent failures, zero harvests ever logged.
            if blueprint == StructureType::Garden {
                commands
                    .entity(target)
                    .insert(crate::components::building::CropState::default());
            }
            if let Some(ref mut score) = colony_score {
                score.structures_built += 1;
            }
            StepOutcome::bare(StepResult::Advance)
        } else {
            StepOutcome::bare(StepResult::Continue)
        }
    } else {
        StepOutcome::bare(StepResult::Advance)
    }
}
