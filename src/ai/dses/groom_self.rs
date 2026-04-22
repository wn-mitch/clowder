//! `Groom(self)` — sibling-DSE split from the retiring `Max`-composed
//! cat `Groom` inline block (§L2.10.10). Thermal-comfort grooming.
//!
//! Per §2.3 rows 984–985: `CompensatedProduct` of `thermal_deficit`
//! via `Logistic(7, 0.6)` (gentler than hangry — cats thermoregulate
//! passively) and an `affection_deficit` sibling axis via the
//! loneliness anchor. The affection axis is blocked on the
//! "split `needs.warmth`" substrate TODO — today the conflated
//! `needs.warmth` field doesn't separate thermal from affection, so
//! we port with the thermal axis only and leave the affection-axis
//! composition slot reserved.
//!
//! Not in any §3.3.2 peer group — Groom(self) stands alone, anchored
//! to thermal-need dynamics.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const THERMAL_DEFICIT_INPUT: &str = "thermal_deficit";

pub struct GroomSelfDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl GroomSelfDse {
    pub fn new() -> Self {
        Self {
            id: DseId("groom_self"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                THERMAL_DEFICIT_INPUT,
                Curve::Logistic {
                    steepness: 7.0,
                    midpoint: 0.6,
                },
            ))],
            // n=1 CP with weight 1.0 — the affection axis lands when
            // `needs.warmth` splits into thermal + affection (tracked
            // as the §2.3 post-split substrate TODO).
            composition: Composition::compensated_product(vec![1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for GroomSelfDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for GroomSelfDse {
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
                label: "groomed_self",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn groom_self_dse() -> Box<dyn Dse> {
    Box::new(GroomSelfDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groom_self_dse_id_stable() {
        assert_eq!(GroomSelfDse::new().id().0, "groom_self");
    }

    #[test]
    fn groom_self_maslow_tier_is_one() {
        assert_eq!(GroomSelfDse::new().maslow_tier(), 1);
    }

    #[test]
    fn groom_self_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            GroomSelfDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }
}
