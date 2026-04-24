//! `Hunt` — peer of Eat/Forage in the Starvation-urgency group.
//!
//! Per §2.3 + §3.1.1: WeightedSum of 4 axes — `hunger_urgency`,
//! `food_scarcity`, `boldness`, `prey_nearby`.
//!
//! Design intent per §3.1.1: "Bold cat spotting prey ⇒ Hunt even on
//! full stomach; starving timid cat ⇒ Hunt out of need. No single
//! axis is a gate."
//!
//! **Prey-proximity shape.** §2.3 specifies the end-state as a
//! `SpatialConsideration` sampling the Prey-location map with
//! `Quadratic(exponent=2)` falloff. Today's code uses
//! `bool prey_nearby + additive hunt_prey_bonus` — a binary
//! presence signal. For Phase 3c.1a we keep the binary signal as
//! a scalar (0 or 1) fed through `Linear(slope=1, intercept=0)`,
//! preserving the old behavior. The spatial upgrade moves to Phase
//! 4 alongside `TargetTakingDse` / `SpatialConsideration` wiring
//! per §6.3 / §L2.10.7.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{hangry, scarcity, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::ai::faction::StanceRequirement;
use crate::components::markers;

pub struct HuntDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HuntDse {
    pub fn new() -> Self {
        Self {
            id: DseId("hunt"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("hunger_urgency", hangry())),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "boldness",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new(
                    "prey_nearby",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            // RtEO weights sum to 1.0. Hunger dominates so starving
            // cats converge on food-acquisition DSEs (peer-group
            // anchor §3.3.2). Scarcity and boldness follow; prey_nearby
            // held to 0.10 — it's an *opportunity* axis, not a gate,
            // so when a cat with food available sees prey it shouldn't
            // out-score `Eat`'s direct path. The binary 0/1 prey_nearby
            // value otherwise dominates the sum disproportionately.
            composition: Composition::weighted_sum(vec![0.5, 0.25, 0.15, 0.10]),
            // §4 batch 2: `.require(CanHunt)` gates on (Adult ∨ Young)
            // ∧ ¬Injured ∧ ¬InCombat ∧ forest nearby. Retires the
            // inline `ctx.can_hunt` guard in `scoring.rs`.
            // §9.3 DSE filter binding — Hunt targets `Prey` only.
            // §13.1: `.forbid(Incapacitated)` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::CanHunt::KEY)
                .with_stance(StanceRequirement::hunt())
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for HuntDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HuntDse {
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
                label: "prey_caught",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn hunt_dse() -> Box<dyn Dse> {
    Box::new(HuntDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunt_dse_id_stable() {
        assert_eq!(HuntDse::new().id().0, "hunt");
    }

    #[test]
    fn hunt_has_four_axes() {
        assert_eq!(HuntDse::new().considerations().len(), 4);
    }

    #[test]
    fn hunt_weights_sum_to_one() {
        let dse = HuntDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {sum}");
    }

    #[test]
    fn hunt_stance_requirement_is_prey_only() {
        use crate::ai::faction::FactionStance;
        let req = HuntDse::new()
            .eligibility()
            .required_stance
            .clone()
            .expect("§9.3 binding must populate required_stance");
        assert!(req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Predator));
        assert!(!req.accepts(FactionStance::Same));
    }
}
