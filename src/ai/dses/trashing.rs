//! 178 `Trashing` DSE — carry surplus food to a Midden building and
//! deposit it there. The first-choice disposal disposition: Middens
//! have unlimited capacity, so a colony with a Midden never needs
//! Discarding. Sibling to Discarding (drop-where-I-am, last-resort),
//! Handing (give-to-peer, deferred to 188), and PickingUp (retrieve
//! ground item, deferred to 185).
//!
//! **Composition.** Single `inventory_excess` axis through a Logistic
//! curve sourced from `ScoringConstants::disposal_inventory_excess_*`
//! (shared shape with Discarding so the two siblings score
//! symmetrically; eligibility differentiates them).
//!
//! **Eligibility.** `forbid(Incapacitated)` AND
//! `require(ColonyStoresChronicallyFull)` AND `require(HasMidden)`.
//! Both disposal siblings gate on `ColonyStoresChronicallyFull`
//! because trashing food the colony's Stores could accept
//! pre-empts the Eat → Cook → Mate Maslow ladder; the chronic-full
//! marker is the colony's "Stores can't take more — it's safe to
//! dispose" signal. `HasMidden` differentiates the *route* between
//! the two siblings (Midden vs ground), not whether disposal is
//! appropriate. When the colony has no Midden the disposition is
//! dormant; the cat falls back to Discarding.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub struct TrashingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl TrashingDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        Self {
            id: DseId("trash"),
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
                .require(markers::ColonyStoresChronicallyFull::KEY)
                .require(markers::HasMidden::KEY),
        }
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

pub fn trashing_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(TrashingDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ScoringConstants {
        ScoringConstants::default()
    }

    #[test]
    fn trashing_dse_id_stable() {
        assert_eq!(TrashingDse::new(&defaults()).id().0, "trash");
    }

    #[test]
    fn trashing_curve_lifts_with_inventory_excess() {
        let dse = TrashingDse::new(&defaults());
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        assert!(c.evaluate(0.0) < 0.05, "empty inventory → near-zero score");
        assert!((c.evaluate(0.5) - 0.5).abs() < 1e-3, "midpoint → 0.5");
        assert!(c.evaluate(1.0) > 0.95, "full inventory → near-one score");
    }

    #[test]
    fn trashing_eligibility_requires_midden_and_chronic_full() {
        let dse = TrashingDse::new(&defaults());
        let elig = dse.eligibility();
        assert!(
            elig.required.contains(&markers::HasMidden::KEY),
            "Trashing must require HasMidden",
        );
        assert!(
            elig.required
                .contains(&markers::ColonyStoresChronicallyFull::KEY),
            "Trashing must require ColonyStoresChronicallyFull \
             (otherwise cats trash food the colony's Stores could accept)",
        );
        assert!(
            elig.forbidden.contains(&markers::Incapacitated::KEY),
            "Trashing must forbid Incapacitated",
        );
    }

    #[test]
    fn trashing_maslow_tier_is_one() {
        assert_eq!(TrashingDse::new(&defaults()).maslow_tier(), 1);
    }
}
