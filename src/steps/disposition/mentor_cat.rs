use bevy_ecs::prelude::*;

use crate::components::physical::Needs;
use crate::components::skills::Skills;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

/// Returns the step result and an optional deferred mentor effect
/// `(apprentice_entity, mentor_skills_snapshot)` to apply after the main loop.
pub fn resolve_mentor_cat(
    ticks: u64,
    cat_entity: Entity,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    skills: &Skills,
    relationships: &mut Relationships,
    tick: u64,
    d: &DispositionConstants,
) -> (StepResult, Option<(Entity, Skills)>) {
    if let Some(target) = target_entity {
        needs.mastery = (needs.mastery + d.mentor_mastery_per_tick).min(1.0);
        needs.social = (needs.social + d.mentor_social_per_tick).min(1.0);
        needs.respect = (needs.respect + d.mentor_respect_per_tick).min(1.0);
        relationships.modify_fondness(cat_entity, target, d.mentor_fondness_per_tick);
        relationships.modify_familiarity(cat_entity, target, d.mentor_familiarity_per_tick);
        relationships
            .get_or_insert(cat_entity, target)
            .last_interaction = tick;
    }
    if ticks >= d.mentor_duration {
        let effect = target_entity.map(|t| (t, skills.clone()));
        (StepResult::Advance, effect)
    } else {
        (StepResult::Continue, None)
    }
}
