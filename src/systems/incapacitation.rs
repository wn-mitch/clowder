//! §4.3 `Incapacitated` marker author.
//!
//! This module hosts the per-tick author system for the `Incapacitated`
//! marker ZST (`src/components/markers.rs`). Per §4 of
//! `docs/systems/ai-substrate-refactor.md`, context-tag markers collapse
//! Mark-style context tags, Bevy ECS component filters, and the scoring
//! substrate's `MarkerSnapshot` bitset into a single concept: author
//! systems insert/remove a ZST per-tick, and DSEs gate eligibility via
//! `Query<With<Marker>, Without<OtherMarker>>` or
//! `EligibilityFilter::{require, forbid}` (`src/ai/eval.rs`).
//!
//! `update_incapacitation` owns the author half of the `Incapacitated`
//! lifecycle. The consumer half — adding `.forbid("Incapacitated")` to
//! every non-Eat/Sleep/Idle DSE and retiring the
//! `if ctx.is_incapacitated` early-return in `src/ai/scoring.rs` — is
//! tracked as §13.1 rows 1–3 in `docs/open-work.md` and lands in a
//! separate commit together with the `incapacitated_*` constant
//! retirements.
//!
//! **Predicate fidelity.** The boolean authored here must match
//! `ScoringContext.is_incapacitated` bit-for-bit so that when §13.1
//! retires the inline branch, behaviour is preserved modulo Bevy's
//! parallel-scheduler noise. The inline expression today lives in
//! `src/systems/goap.rs::evaluate_and_plan` and
//! `src/systems/disposition.rs::evaluate_dispositions`; it reads
//! `health.injuries.iter().any(|i| i.kind == Severe && !i.healed)`.
//! Both scoring systems also populate
//! `MarkerSnapshot::set_entity("Incapacitated", entity, is_incapacitated)`
//! so `EligibilityFilter::{require,forbid}` resolves identically once a
//! consumer is wired.

use bevy_ecs::prelude::*;

use crate::components::markers::Incapacitated;
use crate::components::physical::{Dead, Health, InjuryKind};

/// Author the `Incapacitated` ZST on living cats with at least one
/// unhealed `InjuryKind::Severe` injury; remove it otherwise.
///
/// **Predicate** — `health.injuries.iter().any(|inj| inj.kind ==
/// InjuryKind::Severe && !inj.healed)`. Bit-for-bit mirror of the
/// inline `is_incapacitated` computations in
/// `goap.rs::evaluate_and_plan` and
/// `disposition.rs::evaluate_dispositions`.
///
/// **Ordering** — registered in Chain 2 (cat-needs / decision-prep)
/// before the GOAP scoring pipeline runs, matching the per-tick
/// timing of today's inline consumers. Injury writes (combat
/// resolution, heal ticks) land in Chain 4 at end-of-tick, so the
/// author observes the same end-of-previous-tick state that the
/// inline predicate reads today.
///
/// **Lifecycle** — only transitions insert/remove; idempotent when
/// `is_incapacitated == has_marker`. `Dead` cats are filtered out so
/// markers are not authored on corpses during the narrative
/// grace-period window before `cleanup_dead`.
pub fn update_incapacitation(
    mut commands: Commands,
    cats: Query<(Entity, &Health, Has<Incapacitated>), Without<Dead>>,
) {
    for (entity, health, has_marker) in cats.iter() {
        let is_incapacitated = health
            .injuries
            .iter()
            .any(|inj| inj.kind == InjuryKind::Severe && !inj.healed);
        match (is_incapacitated, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(Incapacitated);
            }
            (false, true) => {
                commands.entity(entity).remove::<Incapacitated>();
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::{DeathCause, Injury, InjurySource, Position};
    use bevy_ecs::schedule::Schedule;

    fn setup_world() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_incapacitation);
        (world, schedule)
    }

    fn spawn_cat_with_health(world: &mut World, health: Health) -> Entity {
        world.spawn(health).id()
    }

    fn injury(kind: InjuryKind, healed: bool) -> Injury {
        Injury {
            kind,
            tick_received: 0,
            healed,
            source: InjurySource::Unknown,
            at: Position::new(0, 0),
        }
    }

    fn has_incapacitated(world: &World, entity: Entity) -> bool {
        world.get::<Incapacitated>(entity).is_some()
    }

    #[test]
    fn empty_injuries_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_health(&mut world, Health::default());
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "no injuries should leave no marker"
        );
    }

    #[test]
    fn single_unhealed_severe_inserts_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Severe, false)],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "unhealed severe should insert marker"
        );
    }

    #[test]
    fn healed_severe_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Severe, true)],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "healed severe should not insert marker"
        );
    }

    #[test]
    fn unhealed_minor_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Minor, false)],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "minor injury alone should not insert marker"
        );
    }

    #[test]
    fn unhealed_moderate_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Moderate, false)],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "moderate injury alone should not insert marker"
        );
    }

    #[test]
    fn unhealed_severe_plus_moderate_inserts_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![
                injury(InjuryKind::Moderate, false),
                injury(InjuryKind::Severe, false),
            ],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "any unhealed severe wins even when mixed with moderate"
        );
    }

    #[test]
    fn healed_severe_plus_unhealed_moderate_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![
                injury(InjuryKind::Severe, true),
                injury(InjuryKind::Moderate, false),
            ],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "healed severe + unhealed moderate should not insert marker"
        );
    }

    #[test]
    fn multiple_unhealed_severe_inserts_once() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![
                injury(InjuryKind::Severe, false),
                injury(InjuryKind::Severe, false),
                injury(InjuryKind::Severe, false),
            ],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        // Component presence is a set — multiple severes still collapse
        // to a single marker ZST.
        assert!(
            has_incapacitated(&world, cat),
            "multiple unhealed severes should still insert marker"
        );
    }

    #[test]
    fn heal_transition_removes_marker() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Severe, false)],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "tick 1 should insert marker"
        );

        // Simulate combat::heal_injuries flipping the severe to healed.
        world.get_mut::<Health>(cat).unwrap().injuries[0].healed = true;
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "tick 2 should remove marker once injury is healed"
        );
    }

    #[test]
    fn new_injury_inserts_marker_next_tick() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_health(&mut world, Health::default());
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "tick 1 should not insert marker on uninjured cat"
        );

        world
            .get_mut::<Health>(cat)
            .unwrap()
            .injuries
            .push(injury(InjuryKind::Severe, false));
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "tick 2 should insert marker after severe injury added"
        );
    }

    #[test]
    fn idempotent_no_flap_across_ticks() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Severe, false)],
            ..Health::default()
        };
        let cat = spawn_cat_with_health(&mut world, health);

        schedule.run(&mut world);
        assert!(has_incapacitated(&world, cat));
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "steady-state tick should not flap marker"
        );

        // And the steady-state uninjured case: no flap either.
        let healthy = spawn_cat_with_health(&mut world, Health::default());
        schedule.run(&mut world);
        assert!(!has_incapacitated(&world, healthy));
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, healthy),
            "steady-state uninjured tick should not flap marker"
        );
    }

    #[test]
    fn dead_cats_are_skipped() {
        let (mut world, mut schedule) = setup_world();
        let health = Health {
            injuries: vec![injury(InjuryKind::Severe, false)],
            ..Health::default()
        };
        let cat = world
            .spawn((
                health,
                Dead {
                    tick: 0,
                    cause: DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "dead cats should not receive marker even with severe injury"
        );
    }

    #[test]
    fn mixed_population_independent_authoring() {
        let (mut world, mut schedule) = setup_world();
        let downed = spawn_cat_with_health(
            &mut world,
            Health {
                injuries: vec![injury(InjuryKind::Severe, false)],
                ..Health::default()
            },
        );
        let wounded = spawn_cat_with_health(
            &mut world,
            Health {
                injuries: vec![injury(InjuryKind::Moderate, false)],
                ..Health::default()
            },
        );
        let healthy = spawn_cat_with_health(&mut world, Health::default());

        schedule.run(&mut world);

        assert!(
            has_incapacitated(&world, downed),
            "severe-injury cat should have marker"
        );
        assert!(
            !has_incapacitated(&world, wounded),
            "moderate-injury cat should not have marker"
        );
        assert!(
            !has_incapacitated(&world, healthy),
            "healthy cat should not have marker"
        );
    }
}
