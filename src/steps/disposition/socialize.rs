use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::hunting_priors::HuntingPriors;
use crate::components::physical::Needs;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::{DispositionConstants, SocialConstants};
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Socialize`
///
/// **Real-world effect** — mutates relationship state
/// (fondness, familiarity, last_interaction) between `cat_entity`
/// and `target_entity`, boosts the actor's `needs.social`, and
/// exchanges hunting-prior knowledge with the colony map.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::SocialTarget)` by
/// `src/ai/planner/actions.rs::socialize_actions`. Target selection
/// runs in `src/ai/dses/socialize_target.rs`; `ZoneIs` alone does
/// not prove a target exists.
///
/// **Runtime preconditions** — mutations only occur inside the
/// `if let Some(target) = target_entity` block. If `target_entity`
/// is `None`, no relationship state changes and the witness stays
/// `false` — the step still `Advance`s after `socialize_duration`
/// so a drifting plan doesn't loop forever.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff a real target was
/// present and the relationship mutations ran this call.
///
/// **Feature emission** — caller passes `Feature::Socialized`
/// (Positive) to `record_if_witnessed`. Before §Phase 5a no
/// Feature existed for Socialize — a blind spot that masked whether
/// the social pipeline was producing any real interactions.
#[allow(clippy::too_many_arguments)]
pub fn resolve_socialize(
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
) -> StepOutcome<bool> {
    let witnessed = if let Some(target) = target_entity {
        let target_grooming = grooming_snapshot.get(&target).copied().unwrap_or(0.8);
        let fondness_mod =
            social.fondness_grooming_floor + target_grooming * social.fondness_grooming_scale;

        needs.social = (needs.social + d.socialize_social_per_tick).min(1.0);
        relationships.modify_fondness(
            cat_entity,
            target,
            d.socialize_fondness_per_tick * fondness_mod,
        );
        relationships.modify_familiarity(cat_entity, target, d.socialize_familiarity_per_tick);
        relationships
            .get_or_insert(cat_entity, target)
            .last_interaction = tick;
        colony_map.absorb(hunting_priors, d.socialize_colony_absorb_rate);
        hunting_priors.learn_from(&colony_map.beliefs, d.socialize_personal_learn_rate);
        true
    } else {
        false
    };

    let result = if ticks >= d.socialize_duration {
        StepResult::Advance
    } else {
        StepResult::Continue
    };

    if witnessed {
        StepOutcome::witnessed(result)
    } else {
        StepOutcome::unwitnessed(result)
    }
}
