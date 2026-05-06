//! 176 `Discarding` DSE — one of four inventory-disposal DSEs that
//! give cats real reasoning over surplus inventory (alongside
//! `Trashing`, `Handing`, `PickingUp`).
//!
//! Stage 3 ships dormant: a single Linear consideration with
//! `slope: 0.0, intercept: 0.0` keeps the score at 0.0 regardless
//! of input, so the L3 softmax never elects this DSE. Balance-
//! tuning lifts the saturation surface in a follow-on by replacing
//! the zero curve with a real `inventory_overstuffed` consideration
//! gated on `ColonyStoresChronicallyFull` plus per-cat overflow
//! signals.
//!
//! Eligibility is `forbid(Incapacitated)` — a Maslow-tier-1
//! disposition; injured cats can't elect it.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const ZERO_INPUT: &str = "one";

pub struct DiscardingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl DiscardingDse {
    pub fn new() -> Self {
        Self {
            id: DseId("discard"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                ZERO_INPUT,
                // 176 stage 3: default-zero scoring — slope and
                // intercept both zero so the score collapses to 0.0
                // regardless of the "one" input. Balance-tuning
                // replaces this with a real overflow consideration.
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

impl Default for DiscardingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for DiscardingDse {
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
                label: "discarded_surplus_item",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn discarding_dse() -> Box<dyn Dse> {
    Box::new(DiscardingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discarding_dse_id_stable() {
        assert_eq!(DiscardingDse::new().id().0, "discard");
    }

    #[test]
    fn discarding_default_zero_scoring() {
        // Default-zero invariant: the Linear curve has slope=0,
        // intercept=0, so any input maps to 0.0. Balance-tuning
        // changes this to a real overflow consideration.
        let dse = DiscardingDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!((c.evaluate(0.0)).abs() < 1e-6);
        assert!((c.evaluate(1.0)).abs() < 1e-6);
    }

    #[test]
    fn discarding_maslow_tier_is_one() {
        assert_eq!(DiscardingDse::new().maslow_tier(), 1);
    }
}
