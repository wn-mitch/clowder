use crate::steps::{StepOutcome, StepResult};
use bevy_ecs::entity::Entity;

/// # GOAP step resolver: `Teach`
///
/// Ticket 364 — middle sub-goal of the `rear_kitten` HTN method. Pairs with
/// the `dependent_kitten_teach_target` DSE that picks the kitten.
///
/// **Real-world effect** — advances the target kitten's
/// [`KittenDependency.maturity`](crate::components::KittenDependency) to
/// `teach_done_threshold` AND increments `skills_learned` (saturating at
/// the curriculum size). The actual mutation runs in the post-loop drain.
///
/// **Plan-level preconditions** — emitted under a `ZoneIs(SocialTarget)`
/// precondition: the cat must be co-located with the kitten.
///
/// **Runtime preconditions** — accepts `current_maturity` from the caller
/// (looked up via `ExecutorContext::kitten_parentage`). Returns witnessed-
/// `None` only when the kitten is already past `teach_done_threshold` AND
/// `current_skills_learned >= curriculum_size`. (When skills are not yet
/// exhausted but maturity is past — an edge case for now — the resolver
/// still witnesses so the skill increment can land.)
///
/// **Witness** — `StepOutcome<Option<Entity>>`. The witness payload is the
/// kitten Entity that advanced. The witness gates `Feature::SkillTaught`
/// emission via `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::SkillTaught` (Positive)
/// to `record_if_witnessed`.
pub fn resolve_teach(
    target_kitten: Entity,
    current_maturity: f32,
    current_skills_learned: u8,
    teach_done_threshold: f32,
    curriculum_size: u8,
) -> StepOutcome<Option<Entity>> {
    let maturity_past = current_maturity >= teach_done_threshold;
    let curriculum_complete = current_skills_learned >= curriculum_size;
    if maturity_past && curriculum_complete {
        return StepOutcome::unwitnessed(StepResult::Advance);
    }
    StepOutcome::witnessed_with(StepResult::Advance, target_kitten)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

    #[test]
    fn witnessed_when_in_teach_band() {
        let outcome = resolve_teach(ent(10), 0.4, 0, 0.66, 5);
        assert_eq!(outcome.witness, Some(ent(10)));
    }

    #[test]
    fn unwitnessed_when_both_past() {
        let outcome = resolve_teach(ent(10), 0.66, 5, 0.66, 5);
        assert_eq!(outcome.witness, None);
    }

    #[test]
    fn witnessed_when_maturity_past_but_skills_incomplete() {
        // Edge case — maturity raced ahead but curriculum still has slots.
        // Re-witnessing lets the skill increment land.
        let outcome = resolve_teach(ent(10), 0.7, 2, 0.66, 5);
        assert_eq!(outcome.witness, Some(ent(10)));
    }
}
