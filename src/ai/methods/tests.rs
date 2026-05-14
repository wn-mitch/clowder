//! Tests for [`super::MethodRegistry`].
//!
//! Lives in a dedicated file (rather than `#[cfg(test)] mod tests`
//! inline) so `scripts/check_method_registry.sh` can exclude this
//! path while scanning `src/ai/methods/` for production `Method`
//! literals. Test fixtures construct `ApplicableWhen::PendingSubstrate`
//! variants with synthetic blocker ids; the lint must not see them.

use super::*;

fn always_true(_: &World, _: Entity) -> bool {
    true
}

fn always_false(_: &World, _: Entity) -> bool {
    false
}

#[test]
fn empty_registry_lookup_is_none() {
    let registry = MethodRegistry::default();
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    assert!(registry.lookup("anything", &world, entity).is_none());
    assert_eq!(registry.len(), 0);
    assert!(registry.is_empty());
}

#[test]
fn pending_substrate_method_is_filtered() {
    let mut registry = MethodRegistry::default();
    registry.push(Method {
        id: MethodId("dormant_example"),
        goal_label: "dormant_goal",
        applicable_when: ApplicableWhen::PendingSubstrate {
            blocker: "999",
            eventual: always_true,
        },
        sub_goals: &[],
        failure_strategy: MethodFailure::Abandon,
    });
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    // The dormant method registers and counts, but `lookup` skips it —
    // the L2 evaluator falls through to the 126 adoption path.
    assert_eq!(registry.len(), 1);
    assert!(registry.lookup("dormant_goal", &world, entity).is_none());
}

#[test]
fn live_method_matches_goal_label_first_applicable() {
    let mut registry = MethodRegistry::default();
    registry.push(Method {
        id: MethodId("inapplicable"),
        goal_label: "shared_goal",
        applicable_when: ApplicableWhen::Live(always_false),
        sub_goals: &[],
        failure_strategy: MethodFailure::Backtrack,
    });
    registry.push(Method {
        id: MethodId("applicable"),
        goal_label: "shared_goal",
        applicable_when: ApplicableWhen::Live(always_true),
        sub_goals: &[],
        failure_strategy: MethodFailure::Backtrack,
    });
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    let hit = registry.lookup("shared_goal", &world, entity);
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().id, MethodId("applicable"));
}
