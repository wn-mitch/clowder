//! Snake `Basking` — thermoregulation. Ectothermic snakes must
//! bask to maintain body temperature; this drives them to warm
//! surfaces when warmth deficit rises.
//!
//! Single axis — `warmth_deficit` via `Logistic(5, 0.4)` (gradual
//! ramp as body temperature drops, inflection at 40% deficit).
//!
//! Maslow tier 2 — safety/comfort (not immediate survival, but
//! necessary for sustained function).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const WARMTH_DEFICIT_INPUT: &str = "warmth_deficit";

pub struct SnakeBaskingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl SnakeBaskingDse {
    pub fn new() -> Self {
        let warmth_curve = Curve::Logistic {
            steepness: 5.0,
            midpoint: 0.4,
        };

        Self {
            id: DseId("snake_basking"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                WARMTH_DEFICIT_INPUT,
                warmth_curve,
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for SnakeBaskingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for SnakeBaskingDse {
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
                label: "snake_warmed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn snake_basking_dse() -> Box<dyn Dse> {
    Box::new(SnakeBaskingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_basking_id_stable() {
        assert_eq!(SnakeBaskingDse::new().id().0, "snake_basking");
    }

    #[test]
    fn snake_basking_has_one_axis() {
        assert_eq!(SnakeBaskingDse::new().considerations().len(), 1);
    }

    #[test]
    fn snake_basking_weights_sum_to_one() {
        let sum: f32 = SnakeBaskingDse::new().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn snake_basking_maslow_tier_is_two() {
        assert_eq!(SnakeBaskingDse::new().maslow_tier(), 2);
    }
}
