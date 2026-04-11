use bevy_ecs::prelude::*;

use crate::components::physical::Needs;
use crate::resources::relationships::Relationships;
use crate::steps::StepResult;

/// Returns the step result and an optional pregnancy trigger
/// `(partner_entity, litter_size)`.
pub fn resolve_mate_with(
    ticks: u64,
    cat_entity: Entity,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    relationships: &mut Relationships,
) -> (StepResult, Option<(Entity, u8)>) {
    if ticks >= 10 {
        let pregnancy = target_entity.map(|partner| {
            let mut litter_size: u8 = 1;
            if needs.hunger > 0.7 {
                litter_size += 1;
            }
            litter_size = litter_size.min(3);

            needs.mating = 1.0;
            needs.social = (needs.social + 0.15).min(1.0);
            relationships.modify_romantic(cat_entity, partner, 0.1);

            (partner, litter_size)
        });
        (StepResult::Advance, pregnancy)
    } else {
        (StepResult::Continue, None)
    }
}
