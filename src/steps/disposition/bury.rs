//! Ticket 035 — `Bury` step resolver. Pairs with the
//! [`bury_dse`](crate::ai::dses::bury::bury_dse) and
//! [`bury_target_dse`](crate::ai::dses::bury_target::bury_target_dse).

use bevy_ecs::prelude::*;

use crate::components::fulfillment::Fulfillment;
use crate::components::physical::{DeathCause, Position};
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// Deferred outcome from a completed bury action. The caller drains
/// these in `goap.rs::resolve_goap_plans`'s post-loop pass and:
/// 1. Inserts `Buried` on the deceased entity (defensive against
///    same-tick double-fire by other cats — sensing's
///    `update_target_existence_markers` filters
///    `(With<Dead>, Without<Buried>)`).
/// 2. Despawns the deceased entity.
/// 3. Spawns a fresh entity carrying `Grave + Position` at the same
///    tile.
///
/// Why deferred: the resolver borrows `&mut Needs` / `&mut Fulfillment`
/// on the actor; the corpse despawn + grave spawn want
/// `commands.entity(deceased)` and a fresh `commands.spawn(...)`.
/// Routing through the post-loop drain keeps the borrow surface clean
/// and matches the precedent of `GroomOutcome` / `MentorEffect`.
#[derive(Debug, Clone)]
pub struct BuryOutcome {
    pub deceased: Entity,
    pub position: Position,
    pub deceased_name: String,
    pub cause: DeathCause,
    pub tick: u64,
}

/// # GOAP step resolver: `Bury`
///
/// **Real-world effect** — on completion (`ticks >= bury_ticks`),
/// boosts the actor's L3-fulfillment Belonging axis (small, by
/// `bury_belonging_gain`). Yields a deferred [`BuryOutcome`] that the
/// caller applies in a post-loop pass: insert `Buried` on the
/// deceased, despawn the deceased entity, spawn a `Grave` entity at
/// the same tile.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::CorpseTarget)` by
/// `src/ai/planner/actions.rs::burying_actions`. `ZoneIs` does not
/// guarantee a target — `src/ai/dses/bury_target.rs::resolve_bury_target`
/// selects one upstream, but a plan that predates the selection may
/// arrive with `target_entity == None`.
///
/// **Runtime preconditions** — the witness only fires inside
/// `if let Some(target) = target_entity`, AND only on the completion
/// tick. While ticking up, the step Continues without effect; on
/// completion without a target, the step Advances unwitnessed (the
/// chain doesn't stall, but no Grave is spawned and no event fires).
///
/// **Witness** — `StepOutcome<Option<BuryOutcome>>`. `Some(outcome)` on
/// completion when a target was present; `None` while still ticking
/// or when completion-time has no target. The caller drains witnesses
/// in the post-loop pass to mutate the world.
///
/// **Feature emission** — caller passes `Feature::BurialPerformed`
/// (Positive) to `record_if_witnessed`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_bury(
    ticks: u64,
    target_entity: Option<Entity>,
    target_position: Option<Position>,
    target_name: Option<String>,
    target_cause: Option<DeathCause>,
    fulfillment: &mut Fulfillment,
    tick: u64,
    d: &DispositionConstants,
) -> StepOutcome<Option<BuryOutcome>> {
    if ticks < d.bury_ticks {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }

    // Belonging-tier fulfillment lift on completion. Mirrors
    // grooming's social_warmth gain — burial is a witnessed
    // affiliative act of caring for the colony. (No dedicated
    // `belonging` axis on `Fulfillment` yet; lifting `social_warmth`
    // matches the existing fulfillment-axis surface and is the
    // closest available analog for "I am part of a colony that takes
    // care of its own.")
    fulfillment.social_warmth = (fulfillment.social_warmth + d.bury_belonging_gain).min(1.0);

    match (target_entity, target_position, target_name, target_cause) {
        (Some(deceased), Some(position), Some(deceased_name), Some(cause)) => {
            StepOutcome::witnessed_with(
                StepResult::Advance,
                BuryOutcome {
                    deceased,
                    position,
                    deceased_name,
                    cause,
                    tick,
                },
            )
        }
        _ => StepOutcome::unwitnessed(StepResult::Advance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::sim_constants::SimConstants;

    fn test_constants() -> DispositionConstants {
        SimConstants::default().disposition
    }

    #[test]
    fn bury_continues_until_duration() {
        let d = test_constants();
        let mut fulfillment = Fulfillment::default();
        let outcome = resolve_bury(
            0,
            Some(Entity::from_raw_u32(2).unwrap()),
            Some(Position::new(5, 5)),
            Some("Hazel".into()),
            Some(DeathCause::OldAge),
            &mut fulfillment,
            100,
            &d,
        );
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(outcome.witness.is_none());
    }

    #[test]
    fn bury_witness_emits_on_completion_with_target() {
        let d = test_constants();
        let mut fulfillment = Fulfillment::default();
        let initial = fulfillment.social_warmth;
        let deceased = Entity::from_raw_u32(2).unwrap();
        let outcome = resolve_bury(
            d.bury_ticks,
            Some(deceased),
            Some(Position::new(5, 5)),
            Some("Hazel".into()),
            Some(DeathCause::OldAge),
            &mut fulfillment,
            100,
            &d,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        let bury = outcome.witness.expect("expected BuryOutcome");
        assert_eq!(bury.deceased, deceased);
        assert_eq!(bury.position, Position::new(5, 5));
        assert_eq!(bury.deceased_name, "Hazel");
        assert!(fulfillment.social_warmth > initial);
    }

    #[test]
    fn bury_advances_unwitnessed_with_no_target() {
        let d = test_constants();
        let mut fulfillment = Fulfillment::default();
        let outcome = resolve_bury(
            d.bury_ticks,
            None,
            None,
            None,
            None,
            &mut fulfillment,
            100,
            &d,
        );
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(outcome.witness.is_none());
    }
}
