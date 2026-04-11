use bevy_ecs::prelude::*;

use crate::components::magic::{Harvestable, Herb, Inventory};
use crate::components::skills::Skills;
use crate::resources::sim_constants::MagicConstants;
use crate::steps::StepResult;

pub fn resolve_gather_herb(
    ticks: u64,
    target_entity: Option<Entity>,
    inventory: &mut Inventory,
    skills: &mut Skills,
    herb_entities: &Query<
        (Entity, &Herb, &crate::components::physical::Position),
        With<Harvestable>,
    >,
    commands: &mut Commands,
    m: &MagicConstants,
) -> StepResult {
    if ticks >= m.gather_herb_ticks {
        if let Some(herb_e) = target_entity {
            if let Ok((_, herb, _)) = herb_entities.get(herb_e) {
                if inventory.add_herb(herb.kind) {
                    commands.entity(herb_e).despawn();
                    skills.herbcraft += skills.growth_rate() * m.herbcraft_gather_skill_growth;
                    StepResult::Advance
                } else {
                    StepResult::Fail("inventory full".into())
                }
            } else {
                StepResult::Fail("herb already taken".into())
            }
        } else {
            StepResult::Fail("no herb target".into())
        }
    } else {
        StepResult::Continue
    }
}
