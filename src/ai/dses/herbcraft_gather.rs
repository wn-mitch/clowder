//! `Herbcraft::GatherHerbs` — sibling-DSE split from the retiring
//! cat `Herbcraft` inline block (§L2.10.10).
//!
//! `CompensatedProduct` of spirituality + herbcraft_skill +
//! territory_max_corruption. All three gate — collecting herbs is a
//! devotional/craft activity; neither pure spirituality nor pure
//! skill suffices alone, and the corruption axis only activates the
//! surge when territory corruption is actually present. Eligibility:
//! `has_herbs_nearby` (outer gate). Maslow tier 2.
//!
//! The `territory_max_corruption` axis uses the §2.3 Logistic(8, 0.1)
//! shape — threshold-gated surge that rises steeply past 0.1
//! corruption. Absorbs the retiring
//! `ward_corruption_emergency_bonus` modifier contribution: the old
//! flat additive bonus-when-corruption-detected is now produced by
//! the axis curve itself as a natural threshold response, consistent
//! with §2.3's retirement unify-shape pattern ("each retired constant
//! was a flat additive bonus gated by a compound threshold, used to
//! overcome the fact that the underlying axis was being scored
//! linearly").

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const SPIRITUALITY_INPUT: &str = "spirituality";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";
pub const TERRITORY_MAX_CORRUPTION_INPUT: &str = "territory_max_corruption";

pub struct HerbcraftGatherDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HerbcraftGatherDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // §2.3 Logistic(8, 0.1) — retires
        // `ward_corruption_emergency_bonus`'s flat additive bonus by
        // absorbing the emergency surge at the axis level.
        let territory_corruption = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.1,
        };
        Self {
            id: DseId("herbcraft_gather"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SPIRITUALITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HERBCRAFT_SKILL_INPUT, linear)),
                Consideration::Scalar(ScalarConsideration::new(
                    TERRITORY_MAX_CORRUPTION_INPUT,
                    territory_corruption,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid("Incapacitated"),
        }
    }
}

impl Default for HerbcraftGatherDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HerbcraftGatherDse {
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
                label: "herbs_in_inventory",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn herbcraft_gather_dse() -> Box<dyn Dse> {
    Box::new(HerbcraftGatherDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn herbcraft_gather_id_stable() {
        assert_eq!(HerbcraftGatherDse::new().id().0, "herbcraft_gather");
    }

    #[test]
    fn herbcraft_gather_has_territory_corruption_axis() {
        let dse = HerbcraftGatherDse::new();
        let names: Vec<&str> = dse
            .considerations()
            .iter()
            .map(|c| match c {
                Consideration::Scalar(s) => s.name,
                _ => "",
            })
            .collect();
        assert!(names.contains(&TERRITORY_MAX_CORRUPTION_INPUT));
        assert_eq!(dse.considerations().len(), 3);
    }

    #[test]
    fn territory_corruption_axis_is_logistic_8_01() {
        // §2.3 retired-constants row 4: Logistic(8, 0.1) absorbs the
        // retiring `ward_corruption_emergency_bonus`. Sample analytical
        // values to pin the curve shape.
        let curve = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.1,
        };
        // 0.0 → 1/(1+exp(0.8)) ≈ 0.3100
        assert!(approx(curve.evaluate(0.0), 0.3100, 1e-3));
        // 0.05 → 1/(1+exp(0.4)) ≈ 0.4013
        assert!(approx(curve.evaluate(0.05), 0.4013, 1e-3));
        // 0.1 (midpoint) → 0.5
        assert!(approx(curve.evaluate(0.1), 0.5, 1e-4));
        // 0.2 → 1/(1+exp(-0.8)) ≈ 0.6900
        assert!(approx(curve.evaluate(0.2), 0.6900, 1e-3));
        // 0.5 → 1/(1+exp(-3.2)) ≈ 0.9608
        assert!(approx(curve.evaluate(0.5), 0.9608, 1e-3));
        // 1.0 → 1/(1+exp(-7.2)) ≈ 0.99925
        assert!(approx(curve.evaluate(1.0), 0.9993, 1e-3));
    }

    #[test]
    fn territory_corruption_axis_present_in_factory() {
        // Guard against accidental curve-shape regression: confirm the
        // factory emits the Logistic(8, 0.1) curve on the axis.
        let dse = HerbcraftGatherDse::new();
        let corruption_curve = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Scalar(s) if s.name == TERRITORY_MAX_CORRUPTION_INPUT => {
                    Some(&s.curve)
                }
                _ => None,
            })
            .expect("territory_max_corruption axis must exist");
        match corruption_curve {
            Curve::Logistic { steepness, midpoint } => {
                assert!(approx(*steepness, 8.0, 1e-6));
                assert!(approx(*midpoint, 0.1, 1e-6));
            }
            other => panic!("expected Logistic(8, 0.1); got {other:?}"),
        }
    }
}
