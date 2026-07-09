//! Prey `Bolt` — individual threat-response flight (266, first prey
//! DSE in the codebase).
//!
//! Elected by the alert-set-gated light dispatcher in
//! [`crate::ai::prey_scoring`] from inside `prey_ai` — prey never
//! enter `evaluate_and_plan`. The election preempts the legacy
//! freeze-timer `Alert → Fleeing` transition when the substrate says
//! *now is the moment*: the predator is committed (Chase affordance
//! high), believed dangerous, and an escape is actually afforded.
//! When no Bolt is elected, the legacy freeze → flee state machine
//! proceeds unchanged (additive; the hack retires only if the
//! substrate proves out — pillar 2 ordering).
//!
//! `WeightedSum` of three axes, all with live substrate writers:
//!
//! | # | Axis                      | Source                                   | Curve         | Weight |
//! |---|---------------------------|------------------------------------------|---------------|--------|
//! | 1 | `threat_chase_affordance` | `Affordance(Chase, threat, me)` (261)    | `Linear(1,0)` | 0.45   |
//! | 2 | `threat_violence_belief`  | `PredatorBeliefs[threat].violence` (314) | `Linear(1,0)` | 0.20   |
//! | 3 | `bolt_affordance`         | `Affordance(Bolt, me, threat)` (314)     | `Linear(1,0)` | 0.35   |
//!
//! Axis 1 is the ticket's named read — the predator's chase readiness
//! against *me* is mutually-perceivable body language (the
//! JointIntention doctrine: crouch, heading, closing speed). It
//! carries the urgency: the writer's heuristic composes proximity ×
//! target-alertness × predator-health × speed-advantage with a
//! min-eligibility gate, so an uncommitted or wounded predator reads
//! 0.0 and the prey holds its freeze.
//!
//! Axis 2 is the prey's own implanted species prior (`SpeciesViolence
//! Priors` prey-perceiver rows → Implant pass). The ticket also names
//! `recency_of_threat_cue`; that facet has NO prey-side writer today
//! (prey are not gossip witnesses), so it is deliberately not an axis
//! — a registered axis whose input can never be non-zero is the
//! silent-canary class 516 just retired. It joins when a prey-side
//! witness pass exists.
//!
//! Axis 3 is escape viability — 314's Bolt heuristic (head start +
//! believed lethality + real escape-speed ratio + reaction readiness).
//!
//! WS-conjunction check (310 S4 lesson): the required conjunction is
//! *detected AND dangerous*. Detection is the outer eligibility gate
//! (only prey in `Alert { threat }` are scored at all — the alert
//! set), and with axis 1 gated to 0.0 by the writer's min-eligibility
//! floor, axes 2+3 alone max out at 0.55 · saturated inputs — below
//! any sane election threshold, so a far/uncommitted threat cannot
//! elect a bolt on belief + head start alone.
//!
//! Maslow: prey have no ladder — the dispatcher scores flat tier 1.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::resources::sim_constants::ScoringConstants;

pub const THREAT_CHASE_AFFORDANCE_INPUT: &str = "threat_chase_affordance";
pub const THREAT_VIOLENCE_BELIEF_INPUT: &str = "threat_violence_belief";
pub const BOLT_AFFORDANCE_INPUT: &str = "bolt_affordance";

pub struct PreyBoltDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PreyBoltDse {
    pub fn new(_scoring: &ScoringConstants) -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        let considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(
                THREAT_CHASE_AFFORDANCE_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                THREAT_VIOLENCE_BELIEF_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(BOLT_AFFORDANCE_INPUT, linear)),
        ];
        let weights = vec![0.45, 0.20, 0.35];

        Self {
            id: DseId("prey_bolt"),
            considerations,
            composition: Composition::weighted_sum(weights),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Dse for PreyBoltDse {
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
        // Bolting is an Activity (flight until safe-distance/duration
        // termination in the prey state machine), not a Goal predicate
        // — §L2.10.5 Activity shape. `Avoid` is the closest existing
        // ActivityKind; the intention is score-only for prey (the
        // dispatcher reads final_score, the prey state machine owns
        // commitment) — same convention as the shadowfox DSEs' dummy
        // Goal predicates.
        Intention::Activity {
            kind: ActivityKind::Avoid,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn prey_bolt_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(PreyBoltDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prey_bolt_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(PreyBoltDse::new(&s).id().0, "prey_bolt");
    }

    #[test]
    fn prey_bolt_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = PreyBoltDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn prey_bolt_has_three_axes() {
        let s = ScoringConstants::default();
        assert_eq!(PreyBoltDse::new(&s).considerations().len(), 3);
    }

    #[test]
    fn belief_and_head_start_alone_stay_below_half() {
        // The WS-conjunction guard from the module doc: with the chase
        // axis at 0.0 (writer's min-eligibility gate — uncommitted
        // predator), even saturated belief + bolt-affordance inputs
        // compose to 0.55 · 1.0 = 0.55 of weight... which must stay
        // beneath the weight of a committed-chase composition. The
        // production election threshold lives in PreyConstants; this
        // pins the weight split so axis 1 stays load-bearing.
        let s = ScoringConstants::default();
        let dse = PreyBoltDse::new(&s);
        let w = &dse.composition().weights;
        assert!(
            w[1] + w[2] < w[0] + w[2],
            "chase axis must outweigh the belief axis"
        );
        assert!(
            w[0] >= 0.40,
            "chase-commitment axis carries the urgency; got {}",
            w[0]
        );
    }
}
