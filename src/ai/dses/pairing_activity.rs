//! `PairingActivity` — §7.M.1 L2 of the three-layer Mating model.
//! OpenMinded sustained-Activity DSE driving courtship of a Friends-
//! bonded compatible partner.
//!
//! Pairs with the target-taking sibling
//! [`PairingActivityTargetDse`](super::pairing_activity_target::PairingActivityTargetDse)
//! (this DSE owns *whether* to court; the target-DSE owns *with whom*).
//!
//! ### Why a single-axis composition
//!
//! L1 ReproduceAspiration is always-on for fertile cats (no per-tick
//! gate); L3 MateWithGoal hard-gates on `HasEligibleMate` (Partners+
//! bond + season + sated/happy). L2 sits between them: the eligibility
//! marker `HasPairingCandidate` is what gates "is courting available
//! at all"; once available, the score should rise smoothly with
//! mating-deficit so the disposition fires before reproductive urgency
//! peaks. A single Logistic axis suffices — additional axes would
//! either duplicate the marker's gate or duplicate the target-DSE's
//! per-target scoring.
//!
//! ### Curve choice
//!
//! `Logistic(steepness=4, midpoint=0.3)` — lower midpoint than `MateDse`'s
//! 0.6 because courting starts well before mating-urgency peaks. Cats
//! with a Friends-bonded compat partner pursue them throughout normal
//! reproductive deficit; the L3 mating-event itself fires only at high
//! deficit + open fertility window.
//!
//! ### Eligibility
//!
//! - Forbid `Incapacitated/Kitten/Young` (mirrors `MateDse`'s §13.1
//!   floor + §4.3 LifeStage gate).
//! - Require `HasPairingCandidate` — orientation-compatible Friends-
//!   bonded partner exists in proximity (`pairing.rs::has_pairing_candidate`).

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::components::markers;

pub const MATING_DEFICIT_INPUT: &str = "mating_deficit";

pub struct PairingActivityDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PairingActivityDse {
    pub fn new() -> Self {
        Self {
            id: DseId("pairing_activity"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                MATING_DEFICIT_INPUT,
                Curve::Logistic {
                    steepness: 4.0,
                    midpoint: 0.3,
                },
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .forbid(markers::Kitten::KEY)
                .forbid(markers::Young::KEY)
                .require(markers::HasPairingCandidate::KEY),
        }
    }
}

impl Default for PairingActivityDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for PairingActivityDse {
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
        CommitmentStrategy::OpenMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Pairing,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        3
    }
}

pub fn pairing_activity_dse() -> Box<dyn Dse> {
    Box::new(PairingActivityDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_activity_dse_id_stable() {
        assert_eq!(PairingActivityDse::new().id().0, "pairing_activity");
    }

    #[test]
    fn pairing_activity_is_weighted_sum_single_axis() {
        use crate::ai::composition::CompositionMode;
        let dse = PairingActivityDse::new();
        assert_eq!(dse.composition().mode, CompositionMode::WeightedSum);
        assert_eq!(dse.considerations().len(), 1);
    }

    #[test]
    fn pairing_activity_requires_has_pairing_candidate_marker() {
        let dse = PairingActivityDse::new();
        assert!(
            dse.eligibility()
                .required
                .contains(&markers::HasPairingCandidate::KEY),
            "PairingActivityDse must require HasPairingCandidate"
        );
    }

    #[test]
    fn pairing_activity_forbids_juvenile_and_incapacitated() {
        let dse = PairingActivityDse::new();
        for forbidden in [
            markers::Incapacitated::KEY,
            markers::Kitten::KEY,
            markers::Young::KEY,
        ] {
            assert!(
                dse.eligibility().forbidden.contains(&forbidden),
                "PairingActivityDse should forbid {forbidden}"
            );
        }
    }

    #[test]
    fn pairing_activity_default_strategy_is_open_minded() {
        let dse = PairingActivityDse::new();
        assert!(matches!(
            dse.default_strategy(),
            CommitmentStrategy::OpenMinded
        ));
    }
}
