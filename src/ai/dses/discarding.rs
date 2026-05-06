//! 178 `Discarding` DSE — drop a held food item on the ground when
//! the colony's Stores are chronically full and the cat's own
//! inventory is overstuffed. Sibling to Trashing (carry to Midden),
//! Handing (give to peer — deferred to 188), and PickingUp (retrieve
//! ground item — deferred to 185).
//!
//! **Composition.** Single `inventory_excess` axis through a Logistic
//! curve (slope/midpoint sourced from
//! `ScoringConstants::disposal_inventory_excess_*`). Per memory
//! feedback "single-axis perception scalars": colony state composes
//! at the eligibility-filter layer, not by folding into the scalar.
//!
//! **Eligibility.** `forbid(Incapacitated)` (Maslow-tier-1; injured
//! cats can't elect this) AND `require(ColonyStoresChronicallyFull)`
//! — Discarding is the *last-resort* disposal: only viable when the
//! colony has nowhere else to put the food. When the colony has a
//! Midden, Trashing's curve scores from the same `inventory_excess`
//! axis but routes through the unlimited-capacity Midden building
//! instead.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub struct DiscardingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl DiscardingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("discard"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                "inventory_excess",
                Curve::Logistic {
                    steepness: scoring.disposal_inventory_excess_slope,
                    midpoint: scoring.disposal_inventory_excess_midpoint,
                },
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .require(markers::ColonyStoresChronicallyFull::KEY),
        }
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

pub fn discarding_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(DiscardingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ScoringConstants {
        ScoringConstants::default()
    }

    #[test]
    fn discarding_dse_id_stable() {
        assert_eq!(DiscardingDse::new(&defaults()).id().0, "discard");
    }

    #[test]
    fn discarding_curve_lifts_with_inventory_excess() {
        // Logistic(8, 0.5) → ~0.018 at 0.0, 0.5 at midpoint, ~0.982 at 1.0.
        let dse = DiscardingDse::new(&defaults());
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!(c.evaluate(0.0) < 0.05, "empty inventory → near-zero score");
        assert!((c.evaluate(0.5) - 0.5).abs() < 1e-3, "midpoint → 0.5");
        assert!(c.evaluate(1.0) > 0.95, "full inventory → near-one score");
    }

    #[test]
    fn discarding_eligibility_requires_chronic_full() {
        let dse = DiscardingDse::new(&defaults());
        let elig = dse.eligibility();
        assert!(
            elig.required
                .contains(&markers::ColonyStoresChronicallyFull::KEY),
            "Discarding must require ColonyStoresChronicallyFull",
        );
        assert!(
            elig.forbidden.contains(&markers::Incapacitated::KEY),
            "Discarding must forbid Incapacitated",
        );
    }

    #[test]
    fn discarding_maslow_tier_is_one() {
        assert_eq!(DiscardingDse::new(&defaults()).maslow_tier(), 1);
    }
}
