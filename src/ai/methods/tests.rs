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
        domain: None,
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
        domain: None,
    });
    registry.push(Method {
        id: MethodId("applicable"),
        goal_label: "shared_goal",
        applicable_when: ApplicableWhen::Live(always_true),
        sub_goals: &[],
        failure_strategy: MethodFailure::Backtrack,
        domain: None,
    });
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    let hit = registry.lookup("shared_goal", &world, entity);
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().id, MethodId("applicable"));
}

// -----------------------------------------------------------------
// 323 — courtship_method roundtrip
// -----------------------------------------------------------------

#[test]
fn courtship_method_has_four_sub_goals_one_per_practice_stage() {
    let m = super::courtship::courtship_method();
    assert_eq!(m.id, MethodId("courtship_method"));
    assert_eq!(m.goal_label, "courtship_completed");
    assert_eq!(m.sub_goals.len(), 4);
    assert_eq!(m.failure_strategy, MethodFailure::Abandon);
    // Courtship is reactive substrate (driven by JointIntention),
    // not aspirational achievement — `domain: None` keeps the
    // method out of the §H step-3 domain-affinity fallback. Matches
    // `rear_kitten` / `mourn_at_grave`. See `courtship.rs` for the
    // panic-class avoided by this discipline.
    assert!(m.domain.is_none());
    let labels: Vec<&'static str> = m
        .sub_goals
        .iter()
        .map(|sg| match sg {
            SubGoal::Primitive { label, .. } => *label,
            SubGoal::Goal(_) => "<goal>",
        })
        .collect();
    assert_eq!(
        labels,
        vec![
            "approach_partner",
            "allogroom_partner",
            "mate_with_partner",
            "consolidate_bonded",
        ]
    );
}

#[test]
fn courtship_predicate_gates_on_joint_intention_and_alive() {
    use crate::components::joint_intention::{JointIntention, PracticeKind};
    use crate::components::physical::{Dead, DeathCause};

    // Bare cat (no JointIntention) — predicate false.
    let mut world = World::new();
    let bare = world.spawn_empty().id();
    assert!(!super::courtship::has_active_courtship(&world, bare));

    // Cat with Courtship JointIntention — predicate true.
    let partner = world.spawn_empty().id();
    let courtier = world
        .spawn(JointIntention::new(PracticeKind::Courtship, partner, 100))
        .id();
    assert!(super::courtship::has_active_courtship(&world, courtier));

    // Dead courtier — predicate false (catches mid-tick deaths
    // before the despawn pass).
    world.entity_mut(courtier).insert(Dead {
        tick: 200,
        cause: DeathCause::OldAge,
    });
    assert!(!super::courtship::has_active_courtship(&world, courtier));
}

#[test]
fn courtship_method_registers_and_is_findable_via_registry_lookup() {
    use crate::components::joint_intention::{JointIntention, PracticeKind};

    let mut registry = MethodRegistry::default();
    registry.push(super::courtship::courtship_method());

    let mut world = World::new();
    let partner = world.spawn_empty().id();
    let courtier = world
        .spawn(JointIntention::new(PracticeKind::Courtship, partner, 100))
        .id();

    let hit = registry.lookup("courtship_completed", &world, courtier);
    assert!(
        hit.is_some(),
        "courtship_method must resolve for a courtier"
    );
    assert_eq!(hit.unwrap().id, MethodId("courtship_method"));

    // Empty registry semantics still hold for non-courtiers — the
    // predicate filters the cat out, not the label.
    let bare = world.spawn_empty().id();
    assert!(registry
        .lookup("courtship_completed", &world, bare)
        .is_none());
}
