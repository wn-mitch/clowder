use bevy_ecs::prelude::*;

use crate::components::physical::Needs;
use crate::components::skills::Skills;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `MentorCat`
///
/// **Real-world effect** — boosts mentor's mastery/social/respect
/// needs, shifts relationships (fondness + familiarity +
/// last_interaction) with the apprentice, and on completion yields
/// a deferred `(apprentice_entity, mentor_skills_snapshot)` the
/// caller uses to transfer skill XP to the apprentice outside the
/// mentor's `&mut` borrow.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::SocialTarget)` by
/// `src/ai/planner/actions.rs::mentor_actions`. Apprentice
/// selection happens at disposition-scoring time; a plan can still
/// arrive with `target_entity == None` if the apprentice was lost
/// mid-plan.
///
/// **Runtime preconditions** — all mentor-side mutations are
/// gated on `if let Some(target) = target_entity`. No target
/// means no real mentoring happened: the step still Advances on
/// time-out but the witness stays `None`.
///
/// **Witness** — `StepOutcome<Option<(Entity, Skills)>>`. `Some`
/// iff the mentor-side mutations ran AND the step completed
/// (`ticks >= mentor_duration`) — the caller uses the snapshot to
/// apply the cross-entity skill transfer.
///
/// **Feature emission** — caller passes `Feature::MentoredCat`
/// (Positive) to `record_if_witnessed`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_mentor_cat(
    ticks: u64,
    cat_entity: Entity,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    skills: &Skills,
    relationships: &mut Relationships,
    tick: u64,
    d: &DispositionConstants,
) -> StepOutcome<Option<(Entity, Skills)>> {
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
        match target_entity {
            Some(t) => StepOutcome::witnessed_with(StepResult::Advance, (t, skills.clone())),
            None => StepOutcome::unwitnessed(StepResult::Advance),
        }
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}
