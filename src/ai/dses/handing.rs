//! 176 `Handing` DSE — hand surplus directly to a target cat.
//! Sibling to Discarding and Trashing.
//!
//! **Eligibility.** `forbid(Incapacitated)` AND
//! `require(HasHandoffRecipient)`. The recipient marker is authored
//! by **ticket 188** (Handoff target picker); pre-188 the marker is
//! allowlisted in `scripts/substrate_stubs.allowlist` and the
//! eligibility filter rejects every cat — keeping Handing dormant
//! and out of the L3 softmax pool while the curve waits for 188's
//! companion lift. 178 leaves the curve at default-zero so when 188
//! lifts both pieces the change is single-commit.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const ZERO_INPUT: &str = "one";

pub struct HandingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HandingDse {
    pub fn new() -> Self {
        Self {
            id: DseId("handoff"),
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
                .require(markers::HasHandoffRecipient::KEY),
        }
    }
}

impl Default for HandingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HandingDse {
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
                label: "handed_off_surplus",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn handing_dse() -> Box<dyn Dse> {
    Box::new(HandingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handing_dse_id_stable() {
        assert_eq!(HandingDse::new().id().0, "handoff");
    }

    #[test]
    fn handing_default_zero_scoring() {
        let dse = HandingDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(0.0)).abs() < 1e-6);
        assert!((c.evaluate(1.0)).abs() < 1e-6);
    }

    #[test]
    fn handing_maslow_tier_is_one() {
        assert_eq!(HandingDse::new().maslow_tier(), 1);
    }
}
