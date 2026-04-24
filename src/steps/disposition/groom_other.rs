use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::fulfillment::Fulfillment;
use crate::components::hunting_priors::HuntingPriors;
use crate::components::physical::Needs;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::{
    DispositionConstants, FulfillmentConstants, SocialConstants,
};
use crate::steps::{StepOutcome, StepResult};

/// Deferred outcome from a completed groom-other action. The caller
/// applies `grooming_delta` to the target's `GroomingCondition` and
/// `social_warmth_delta` to the target's `Fulfillment.social_warmth`
/// in a post-loop pass (because `&mut Needs`/`&mut Fulfillment` on
/// the target conflicts with the actor's mutable borrow in the cats
/// query).
#[derive(Debug, Clone, Copy)]
pub struct GroomOutcome {
    pub target: Entity,
    pub grooming_delta: f32,
    pub social_warmth_delta: f32,
}

/// # GOAP step resolver: `GroomOther`
///
/// **Real-world effect** — mutates relationships (fondness +
/// familiarity + last_interaction) between actor and target,
/// boosts actor's social need and both parties' `social_warmth`
/// fulfillment axis, exchanges hunting priors with the colony map.
/// On completion (`ticks >= groom_other_duration`), yields a
/// deferred `GroomOutcome` that the caller applies in a post-loop
/// pass — `&mut Fulfillment` on the target conflicts with the
/// actor's borrow in the cats query.
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
/// **Witness** — `StepOutcome<Option<GroomOutcome>>`. `Some(outcome)`
/// on completion when a target was present — the caller applies
/// `grooming_delta` to the target's GroomingCondition and
/// `social_warmth_delta` to the target's Fulfillment. `None` while
/// still walking / timing out with no target.
///
/// **Feature emission** — caller passes `Feature::GroomedOther`
/// (Positive) to `record_if_witnessed`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_groom_other(
    ticks: u64,
    cat_entity: Entity,
    target_entity: Option<Entity>,
    needs: &mut Needs,
    fulfillment: &mut Fulfillment,
    hunting_priors: &mut HuntingPriors,
    relationships: &mut Relationships,
    colony_map: &mut ColonyHuntingMap,
    grooming_snapshot: &HashMap<Entity, f32>,
    tick: u64,
    social: &SocialConstants,
    d: &DispositionConstants,
    fc: &FulfillmentConstants,
) -> StepOutcome<Option<GroomOutcome>> {
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
        // §7.W warmth split: grooming feeds social_warmth (fulfillment
        // axis), NOT needs.temperature. Both the groomer and the groomed
        // receive social_warmth — the groomer's is applied here, the
        // target's is deferred via GroomOutcome.
        fulfillment.social_warmth =
            (fulfillment.social_warmth + fc.social_warmth_groom_other_gain).min(1.0);

        match target_entity {
            Some(t) => StepOutcome::witnessed_with(
                StepResult::Advance,
                GroomOutcome {
                    target: t,
                    grooming_delta: 0.12,
                    social_warmth_delta: fc.social_warmth_groom_other_gain,
                },
            ),
            None => StepOutcome::unwitnessed(StepResult::Advance),
        }
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Needs;

    fn test_constants() -> (SocialConstants, DispositionConstants, FulfillmentConstants) {
        let sc = crate::resources::sim_constants::SimConstants::default();
        (
            sc.social.clone(),
            sc.disposition.clone(),
            sc.fulfillment.clone(),
        )
    }

    fn make_deps() -> (
        Relationships,
        ColonyHuntingMap,
        HuntingPriors,
        HashMap<Entity, f32>,
    ) {
        (
            Relationships::default(),
            ColonyHuntingMap::default(),
            HuntingPriors::default(),
            HashMap::new(),
        )
    }

    #[test]
    fn groom_other_feeds_social_warmth() {
        let (social, disp, fc) = test_constants();
        let (mut rels, mut colony_map, mut priors, snapshot) = make_deps();
        let mut needs = Needs::default();
        let mut fulfillment = Fulfillment::default();
        let initial_sw = fulfillment.social_warmth;

        // Complete the groom (ticks >= duration).
        let mut world = World::new();
        let cat = world.spawn_empty().id();

        let outcome = resolve_groom_other(
            disp.groom_other_duration, // enough ticks to complete
            cat,
            None, // no target — just verifying groomer's fulfillment
            &mut needs,
            &mut fulfillment,
            &mut priors,
            &mut rels,
            &mut colony_map,
            &snapshot,
            0,
            &social,
            &disp,
            &fc,
        );

        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(
            fulfillment.social_warmth > initial_sw,
            "groomer's social_warmth should rise: initial={initial_sw}, after={}",
            fulfillment.social_warmth
        );
    }

    #[test]
    fn groom_other_no_longer_feeds_temperature() {
        let (social, disp, fc) = test_constants();
        let (mut rels, mut colony_map, mut priors, snapshot) = make_deps();
        let mut needs = Needs::default();
        let mut fulfillment = Fulfillment::default();
        let initial_temp = needs.temperature;

        let mut world = World::new();
        let cat = world.spawn_empty().id();

        let _outcome = resolve_groom_other(
            disp.groom_other_duration,
            cat,
            None,
            &mut needs,
            &mut fulfillment,
            &mut priors,
            &mut rels,
            &mut colony_map,
            &snapshot,
            0,
            &social,
            &disp,
            &fc,
        );

        assert!(
            (needs.temperature - initial_temp).abs() < f32::EPSILON,
            "temperature should NOT change from groom_other: initial={initial_temp}, after={}",
            needs.temperature
        );
    }

    #[test]
    fn groom_other_target_receives_social_warmth_via_outcome() {
        let (social, disp, fc) = test_constants();
        let (mut rels, mut colony_map, mut priors, snapshot) = make_deps();
        let mut needs = Needs::default();
        let mut fulfillment = Fulfillment::default();

        let mut world = World::new();
        let cat = world.spawn_empty().id();
        let target = world.spawn_empty().id();

        let outcome = resolve_groom_other(
            disp.groom_other_duration,
            cat,
            Some(target),
            &mut needs,
            &mut fulfillment,
            &mut priors,
            &mut rels,
            &mut colony_map,
            &snapshot,
            0,
            &social,
            &disp,
            &fc,
        );

        let groom = outcome
            .witness
            .expect("should produce GroomOutcome with target");
        assert_eq!(groom.target, target);
        assert!(groom.grooming_delta > 0.0);
        assert!(groom.social_warmth_delta > 0.0);
    }
}
