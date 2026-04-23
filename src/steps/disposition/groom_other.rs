use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::hunting_priors::HuntingPriors;
use crate::components::physical::Needs;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::{DispositionConstants, SocialConstants};
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `GroomOther`
///
/// **Real-world effect** — mutates relationships (fondness +
/// familiarity + last_interaction) between actor and target,
/// boosts actor's social + temperature needs, exchanges hunting
/// priors with the colony map. On completion (`ticks >=
/// groom_other_duration`), yields a deferred `(target, +0.12)`
/// grooming restoration that the caller applies in a post-loop
/// pass — `&mut Needs` on the target conflicts with the actor's
/// `&mut Needs` in the cats query.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::SocialTarget)` by
/// `src/ai/planner/actions.rs::grooming_actions`. `ZoneIs` does
/// not guarantee a target — `src/ai/dses/socialize_target.rs`
/// selects one, but a plan that predates the selection may
/// arrive with `target_entity == None`.
///
/// **Runtime preconditions** — relationship + colony-map mutations
/// only occur inside `if let Some(target) = target_entity`. On a
/// missing target the step still Continues / Advances over time
/// so the chain doesn't stall, but the witness stays `None`.
///
/// **Witness** — `StepOutcome<Option<(Entity, f32)>>`. `Some((t,
/// delta))` on completion when a target was present — the caller
/// applies `delta` to `t`'s grooming restoration deferred. `None`
/// while still walking / timing out with no target.
///
/// **Feature emission** — caller passes `Feature::GroomedOther`
/// (Positive) to `record_if_witnessed`.
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
) -> StepOutcome<Option<(Entity, f32)>> {
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
        needs.temperature = (needs.temperature + d.groom_other_temperature_gain).min(1.0);
        match target_entity {
            Some(t) => StepOutcome::witnessed_with(StepResult::Advance, (t, 0.12)),
            None => StepOutcome::unwitnessed(StepResult::Advance),
        }
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}
