use crate::steps::{StepOutcome, StepResult};
use bevy_ecs::entity::Entity;

/// # GOAP step resolver: `Release`
///
/// Ticket 364 — final sub-goal of the `rear_kitten` HTN method. Pairs with
/// the `dependent_kitten_release_target` DSE that picks the kitten.
///
/// **Real-world effect** — removes the target kitten's
/// [`KittenDependency`](crate::components::KittenDependency) Component,
/// retiring the kitten to fully-independent colony-member status. The
/// existing [`update_parent_markers`](crate::systems::growth::update_parent_markers)
/// cascades the `Parent` marker removal on the mother next tick. Distinct
/// from `ReleaseGrief` (which retires a `mourn_at_grave` arc — different
/// real-world effect on a different Component).
///
/// **Plan-level preconditions** — emitted under a `ZoneIs(SocialTarget)`
/// precondition.
///
/// **Runtime preconditions** — accepts `kitten_has_dependency` from the
/// caller. Returns witnessed-`None` if the Component was already removed
/// (e.g., natural maturation at `maturity >= 1.0` retired it first).
///
/// **Witness** — `StepOutcome<Option<Entity>>`. The witness payload is the
/// kitten Entity that was released. The witness gates
/// `Feature::KittenReleased` emission via `record_if_witnessed`.
///
/// **Feature emission** — caller passes `Feature::KittenReleased`
/// (Positive) to `record_if_witnessed`.
pub fn resolve_release(
    target_kitten: Entity,
    kitten_has_dependency: bool,
) -> StepOutcome<Option<Entity>> {
    if !kitten_has_dependency {
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
    fn witnessed_when_dependency_present() {
        let outcome = resolve_release(ent(10), true);
        assert_eq!(outcome.witness, Some(ent(10)));
    }

    #[test]
    fn unwitnessed_when_dependency_absent() {
        let outcome = resolve_release(ent(10), false);
        assert_eq!(outcome.witness, None);
    }
}
