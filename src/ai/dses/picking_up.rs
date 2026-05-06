//! 176 `PickingUp` DSE — retrieve a desired item from the ground.
//! Inverse of Discarding; load-bearing for the
//! kill→carcass-on-ground→pick-up flow once `engage_prey` overflow
//! lands real entities (stage 2).
//!
//! **Eligibility.** `forbid(Incapacitated)` AND
//! `require(HasGroundCarcass)`. The colony-scoped marker is authored
//! by **ticket 185** (PickingUp + scavenging composition); pre-185
//! the marker is allowlisted in `scripts/substrate_stubs.allowlist`
//! and eligibility rejects every cat — keeping PickingUp dormant and
//! out of the L3 softmax pool. 178 leaves the curve at default-zero
//! so when 185 lifts both pieces the change is single-commit.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const ZERO_INPUT: &str = "one";

pub struct PickingUpDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PickingUpDse {
    pub fn new() -> Self {
        Self {
            id: DseId("pick_up"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                ZERO_INPUT,
                Curve::Linear {
                    slope: 0.0,
                    intercept: 0.0,
                },
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
    fn picking_up_default_zero_scoring() {
        let dse = PickingUpDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(0.0)).abs() < 1e-6);
        assert!((c.evaluate(1.0)).abs() < 1e-6);
    }

    #[test]
    fn picking_up_maslow_tier_is_one() {
        assert_eq!(PickingUpDse::new().maslow_tier(), 1);
    }
}
