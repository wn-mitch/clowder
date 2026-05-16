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
use crate::components::physical::Dead;

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
    cats: Query<
        (
            Entity,
            &crate::components::CatBodyModel,
            Has<Incapacitated>,
        ),
        Without<Dead>,
    >,
    constants: Res<crate::resources::sim_constants::SimConstants>,
) {
    // 095 Phase 1 Stage B — Incapacitated derives from anatomical pain
    // fraction (normalized total_pain / max_possible_pain) crossing the
    // configured threshold. Replaces the legacy "any Severe unhealed
    // injury" predicate.
    let weights = &constants.combat.body_zone_pain_weights;
    let max_pain: f32 = weights.iter().sum();
    let threshold = constants.combat.pain_incapacitation_threshold;
    for (entity, body_model, has_marker) in cats.iter() {
        let is_incapacitated =
            max_pain > 0.0 && (body_model.total_pain(weights) / max_pain) > threshold;
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
    use crate::components::body_zones::{BodyPart, CatBodyModel};
    use crate::components::physical::{DeathCause, Health};
    use crate::resources::sim_constants::SimConstants;
    use bevy_ecs::schedule::Schedule;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(update_incapacitation);
        (world, schedule)
    }

    /// 095 Phase 1 Stage B — construct a body model whose total_pain is
    /// at or above the configured incapacitation fraction. Saturates
    /// every part to ensure the predicate trips even with conservative
    /// weights.
    fn saturated_body_model() -> CatBodyModel {
        let c = SimConstants::default();
        let weights = &c.combat.body_zone_pain_weights;
        let thresholds = &c.combat.body_zone_condition_thresholds;
        let permanent = &c.combat.body_zone_permanent_at_destroyed;
        let mut model = CatBodyModel::default();
        for part in BodyPart::ALL {
            model.apply_damage(part, 1.0, thresholds, permanent);
            let _ = weights; // pain weights drive total_pain; saturating tissue is enough.
        }
        model
    }

    /// Body model with one part lightly damaged (Bruised tier) — below
    /// the incapacitation threshold even with the heaviest-weight part.
    fn lightly_damaged_body_model() -> CatBodyModel {
        let c = SimConstants::default();
        let mut model = CatBodyModel::default();
        model.apply_damage(
            BodyPart::Tail,
            0.1,
            &c.combat.body_zone_condition_thresholds,
            &c.combat.body_zone_permanent_at_destroyed,
        );
        model
    }

    fn spawn_cat_with_body_model(world: &mut World, model: CatBodyModel) -> Entity {
        world.spawn((Health::default(), model)).id()
    }

    fn has_incapacitated(world: &World, entity: Entity) -> bool {
        world.get::<Incapacitated>(entity).is_some()
    }

    #[test]
    fn empty_body_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_body_model(&mut world, CatBodyModel::default());
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "uninjured cat should leave no marker"
        );
    }

    #[test]
    fn saturated_body_inserts_marker() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_body_model(&mut world, saturated_body_model());
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "total_pain at max should insert marker"
        );
    }

    #[test]
    fn light_damage_no_marker() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_body_model(&mut world, lightly_damaged_body_model());
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "small tissue damage should not exceed incapacitation threshold"
        );
    }

    #[test]
    fn heal_transition_removes_marker() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_body_model(&mut world, saturated_body_model());
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "tick 1 should insert marker"
        );

        // Manually reset every part's tissue damage to 0 — simulates a
        // full per-part healing pass returning the cat to Healthy.
        let mut model = world.get_mut::<CatBodyModel>(cat).unwrap();
        for state in model.parts.iter_mut() {
            state.tissue_damage = 0.0;
            state.condition = crate::components::body_zones::PartCondition::Healthy;
        }
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "tick 2 should remove marker once body has healed"
        );
    }

    #[test]
    fn idempotent_no_flap_across_ticks() {
        let (mut world, mut schedule) = setup_world();
        let cat = spawn_cat_with_body_model(&mut world, saturated_body_model());
        schedule.run(&mut world);
        assert!(has_incapacitated(&world, cat));
        schedule.run(&mut world);
        assert!(
            has_incapacitated(&world, cat),
            "steady-state tick should not flap marker"
        );

        let healthy = spawn_cat_with_body_model(&mut world, CatBodyModel::default());
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
        let cat = world
            .spawn((
                Health::default(),
                saturated_body_model(),
                Dead {
                    tick: 0,
                    cause: DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(
            !has_incapacitated(&world, cat),
            "dead cats should not receive marker even when saturated"
        );
    }

    #[test]
    fn mixed_population_independent_authoring() {
        let (mut world, mut schedule) = setup_world();
        let downed = spawn_cat_with_body_model(&mut world, saturated_body_model());
        let wounded = spawn_cat_with_body_model(&mut world, lightly_damaged_body_model());
        let healthy = spawn_cat_with_body_model(&mut world, CatBodyModel::default());

        schedule.run(&mut world);

        assert!(has_incapacitated(&world, downed));
        assert!(!has_incapacitated(&world, wounded));
        assert!(!has_incapacitated(&world, healthy));
    }
}
