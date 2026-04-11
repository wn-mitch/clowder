use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::hunting_priors::HuntingPriors;
use crate::components::physical::Needs;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::{DispositionConstants, SocialConstants};
use crate::steps::StepResult;

/// Returns the step result and an optional deferred grooming restoration
/// `(target_entity, delta)` to apply after the main loop.
#[allow(clippy::too_many_arguments)]
pub fn resolve_groom_other(
    ticks: u64,
    cat_entity: Entity,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    hunting_priors: &mut HuntingPriors,
    relationships: &mut Relationships,
    colony_map: &mut ColonyHuntingMap,
    grooming_snapshot: &HashMap<Entity, f32>,
    tick: u64,
    social: &SocialConstants,
    d: &DispositionConstants,
) -> (StepResult, Option<(Entity, f32)>) {
    if let Some(target) = target_entity {
        let target_grooming = grooming_snapshot.get(&target).copied().unwrap_or(0.8);
        let fondness_mod =
            social.fondness_grooming_floor + target_grooming * social.fondness_grooming_scale;

        needs.social = (needs.social + d.groom_other_social_per_tick).min(1.0);
        relationships.modify_fondness(
            cat_entity,
            target,
            d.groom_other_fondness_per_tick * fondness_mod,
        );
        relationships.modify_familiarity(cat_entity, target, d.groom_other_familiarity_per_tick);
        relationships
            .get_or_insert(cat_entity, target)
            .last_interaction = tick;
        colony_map.absorb(hunting_priors, d.groom_other_colony_absorb_rate);
        hunting_priors.learn_from(&colony_map.beliefs, d.groom_other_personal_learn_rate);
    }
    if ticks >= d.groom_other_duration {
        needs.warmth = (needs.warmth + d.groom_other_warmth_gain).min(1.0);
        let restoration = target_entity.map(|t| (t, 0.12));
        (StepResult::Advance, restoration)
    } else {
        (StepResult::Continue, None)
    }
}
