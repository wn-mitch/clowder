use bevy_ecs::prelude::*;

use crate::components::building::ConstructionSite;
use crate::components::building::CropState;
use crate::components::building::Structure;
use crate::components::physical::Position;
use crate::components::task_chain::Material;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Deliver`
///
/// **Real-world effect** — calls `site.deliver(material, amount)`
/// on a target `ConstructionSite`, incrementing its delivered-
/// materials counter (tracked separately from the total required;
/// the site's `required - delivered` ledger decides when
/// Construct can begin).
///
/// **Plan-level preconditions** — this step is produced by
/// `src/systems/task_chains.rs` (the disposition-chain path), not
/// by the GOAP planner — `Construct` handles its own delivery
/// internally. The planner-side `GoapActionKind::DeliverMaterials`
/// is an enum-exhaustiveness fallback.
///
/// **Runtime preconditions** — requires `target_entity` to resolve
/// to a building that also carries a `ConstructionSite` component.
/// On a missing target or a building without `ConstructionSite`,
/// the step returns `unwitnessed(Advance)`: the chain moves on
/// (re-queueing the delivery in the next task-chain planning pass
/// is cheaper than a Fail, which would drop the whole chain).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the target
/// building had a `ConstructionSite` and `deliver(material,
/// amount)` was called.
///
/// **Feature emission** — caller passes
/// `Feature::MaterialsDelivered` (Positive) to
/// `record_if_witnessed`. Before §Phase 5a there was no Feature —
/// stuck construction sites couldn't be distinguished from
/// healthy delivery cadence by the Activation canary.
#[allow(clippy::type_complexity)]
pub fn resolve_deliver(
    material: Material,
    amount: u32,
    target_entity: Option<Entity>,
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
) -> StepOutcome<bool> {
    let Some(target) = target_entity else {
        return StepOutcome::unwitnessed(StepResult::Advance);
    };
    if let Ok((_, _, Some(mut site), _, _)) = buildings.get_mut(target) {
        site.deliver(material, amount);
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Advance)
    }
}
