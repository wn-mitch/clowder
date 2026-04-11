use bevy_ecs::prelude::*;

use crate::components::building::ConstructionSite;
use crate::components::building::CropState;
use crate::components::building::Structure;
use crate::components::physical::Position;
use crate::components::task_chain::Material;
use crate::steps::StepResult;

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
) -> StepResult {
    if let Some(target) = target_entity {
        if let Ok((_, _, Some(mut site), _, _)) = buildings.get_mut(target) {
            site.deliver(material, amount);
        }
    }
    StepResult::Advance
}
