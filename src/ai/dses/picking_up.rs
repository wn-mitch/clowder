//! 176 / 185 `PickingUp` DSE — retrieve a desired item from the ground.
//! Inverse of Discarding; load-bearing for the
//! kill→carcass-on-ground→pick-up flow.
//!
//! **Eligibility.** `forbid(Incapacitated)` AND
//! `require(HasGroundCarcass)`. The colony-scoped marker is authored
//! by `update_colony_building_markers` (ticket 185) from any
//! uncleansed/unharvested carcass in the colony — when no ground
//! carcass exists, eligibility rejects every cat and PickingUp stays
//! out of the L3 softmax pool.
//!
//! **Composition.** Single `colony_food_security` axis with an
//! inverted Logistic curve: scavenge urgency rises sharply as the
//! colony's food-security signal drops. `colony_food_security` is the
//! `min(food_fraction, hunger_satisfaction)` composite from
//! `scoring.rs:545`, so the inverse fires when *either* the colony
//! stockpile is low *or* the cat's own hunger is unsatisfied — which
//! is the right shape for "scavenge nearby carcass when food is
//! short, ignore it when sated and stockpiled." Plausibility curve;
//! balance follow-on tunes the Logistic params.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const SCAVENGE_INPUT: &str = "colony_food_security";

pub struct PickingUpDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PickingUpDse {
    pub fn new() -> Self {
        // Scavenge urgency = inverted Logistic over `colony_food_security`.
        // Logistic(8, 0.5) gives a steep transition around 0.5; Invert
        // flips so low food-security → high score, high → ~0.
        let scavenge_urgency = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("pick_up"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                SCAVENGE_INPUT,
                scavenge_urgency,
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .require(markers::HasGroundCarcass::KEY),
        }
    }
}

impl Default for PickingUpDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for PickingUpDse {
    fn id(&self) -> DseId {
        self.id
    }
    fn considerations(&self) -> &[Consideration] {
        &self.considerations
    }
    fn composition(&self) -> &Composition {
        &self.composition
    }
    fn eligibility(&self) -> &EligibilityFilter {
        &self.eligibility
    }
    fn default_strategy(&self) -> CommitmentStrategy {
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "picked_up_ground_item",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn picking_up_dse() -> Box<dyn Dse> {
    Box::new(PickingUpDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_up_dse_id_stable() {
        assert_eq!(PickingUpDse::new().id().0, "pick_up");
    }

    #[test]
    fn picking_up_scavenges_when_food_security_low() {
        // 185: replaced 178's default-zero curve with an inverted
        // Logistic. Food-security 0.0 (colony starving / cat hungry)
        // should score near 1.0; food-security 1.0 (sated + stockpiled)
        // should score near 0.0.
        let dse = PickingUpDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        let low = c.evaluate(0.0);
        let mid = c.evaluate(0.5);
        let high = c.evaluate(1.0);
        // High urgency at low food-security.
        assert!(low > 0.9, "expected scavenge urgency >0.9 at food_security=0, got {low}");
        // Symmetric around midpoint 0.5.
        assert!((mid - 0.5).abs() < 1e-3, "expected scavenge urgency ≈0.5 at food_security=0.5, got {mid}");
        // Low urgency at high food-security.
        assert!(high < 0.1, "expected scavenge urgency <0.1 at food_security=1, got {high}");
    }

    #[test]
    fn picking_up_axis_is_food_security() {
        let dse = PickingUpDse::new();
        match &dse.considerations()[0] {
            Consideration::Scalar(sc) => assert_eq!(sc.name, SCAVENGE_INPUT),
            _ => panic!("expected ScalarConsideration"),
        }
    }

    #[test]
    fn picking_up_eligibility_requires_ground_carcass() {
        let dse = PickingUpDse::new();
        assert!(dse
            .eligibility()
            .required
            .contains(&markers::HasGroundCarcass::KEY));
    }

    #[test]
    fn picking_up_maslow_tier_is_one() {
        assert_eq!(PickingUpDse::new().maslow_tier(), 1);
    }
}
