//! 176 `Trashing` DSE — carry surplus to the Midden. Sibling to
//! Discarding (drop-where-I-am) and Handing (give to peer).
//!
//! Stage 3 ships dormant via a default-zero Linear consideration.
//! Balance-tuning replaces the zero with an overflow consideration
//! gated on `ColonyStoresChronicallyFull` once the marker is
//! authored.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const ZERO_INPUT: &str = "one";

pub struct TrashingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl TrashingDse {
    pub fn new() -> Self {
        Self {
            id: DseId("trash"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                ZERO_INPUT,
                Curve::Linear {
                    slope: 0.0,
                    intercept: 0.0,
                },
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for TrashingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for TrashingDse {
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
                label: "trashed_surplus_at_midden",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn trashing_dse() -> Box<dyn Dse> {
    Box::new(TrashingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trashing_dse_id_stable() {
        assert_eq!(TrashingDse::new().id().0, "trash");
    }

    #[test]
    fn trashing_default_zero_scoring() {
        let dse = TrashingDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(0.0)).abs() < 1e-6);
        assert!((c.evaluate(1.0)).abs() < 1e-6);
    }

    #[test]
    fn trashing_maslow_tier_is_one() {
        assert_eq!(TrashingDse::new().maslow_tier(), 1);
    }
}
