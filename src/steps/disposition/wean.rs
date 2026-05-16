use crate::steps::{StepOutcome, StepResult};
use bevy_ecs::entity::Entity;

/// # GOAP step resolver: `Wean`
///
/// Ticket 364 — first sub-goal of the `rear_kitten` HTN method. Pairs with
/// the `dependent_kitten_wean_target` DSE that picks the kitten.
///
/// **Real-world effect** — bumps the target kitten's
/// [`KittenDependency.maturity`](crate::components::KittenDependency) to
/// `weaned_threshold` (a no-op if maturity is already past). The actual
/// mutation runs in the post-loop drain (mirrors `kitten_feedings` /
/// `bury_completions` accumulator pattern) so it doesn't conflict with
/// the outer `&mut Needs` cats query.
///
/// **Plan-level preconditions** — emitted under a `ZoneIs(SocialTarget)`
/// precondition: the cat must be co-located with the kitten (kittens are
/// alive cats in `cat_positions`).
///
/// **Runtime preconditions** — accepts `current_maturity` from the caller
/// (looked up via `ExecutorContext::kitten_parentage`). Returns
/// witnessed-`None` if maturity is already at or past `weaned_threshold`
/// (idempotent: no Feature emission, no accumulator push).
///
/// **Witness** — `StepOutcome<Option<Entity>>`. The witness payload is the
/// kitten Entity that advanced. The witness gates `Feature::KittenWeaned`
/// emission via `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::KittenWeaned` (Positive)
/// to `record_if_witnessed`.
pub fn resolve_wean(
    target_kitten: Entity,
    current_maturity: f32,
    weaned_threshold: f32,
) -> StepOutcome<Option<Entity>> {
    if current_maturity >= weaned_threshold {
        // Already past — idempotent no-op. Sub-goal completion is observed
        // via the maturity check; the advance hook will progress the
        // HeldGoalStack on the next IntentionFulfilled.
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
    fn witnessed_when_below_threshold() {
        let outcome = resolve_wean(ent(10), 0.1, 0.33);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert_eq!(outcome.witness, Some(ent(10)));
    }

    #[test]
    fn unwitnessed_when_at_threshold() {
        let outcome = resolve_wean(ent(10), 0.33, 0.33);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert_eq!(outcome.witness, None);
    }

    #[test]
    fn unwitnessed_when_above_threshold() {
        let outcome = resolve_wean(ent(10), 0.5, 0.33);
        assert_eq!(outcome.witness, None);
    }
}
