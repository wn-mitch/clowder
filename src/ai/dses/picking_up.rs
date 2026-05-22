//! 176 / 185 / 193 `PickingUp` DSE — retrieve a desired item from the
//! ground. Inverse of Discarding; load-bearing for the
//! kill→item-on-ground→pick-up flow.
//!
//! **Eligibility.** `forbid(Incapacitated)` AND
//! `require(HasGroundCarcass)`. The colony-scoped marker is authored
//! by `update_colony_building_markers` from any `Item` with
//! `location == OnGround` and `kind.is_food()` (ticket 193 re-wire of
//! the 185 author; pre-193 the marker latched on `Carcass` component
//! entities, which the resolver cannot consume — the planner routed
//! through `MaterialPile` and failed 1367/10kt on the seed-42
//! canonical soak). When no ground food-Item exists, eligibility
//! rejects every cat and PickingUp stays out of the L3 softmax pool.
//!
//! **Composition.** Two axes via pure product (CompensatedProduct
//! with `compensation_strength = 0`):
//! - `colony_food_security` (existing): inverted Logistic — scavenge
//!   urgency rises sharply as food-security drops.
//! - `health_deficit` (231 R3b): Linear(slope=-1, intercept=1) — damps
//!   to 1.0 at deficit=0 (full-HP cat ⇒ no suppression, parity with
//!   pre-231 score) and to 0.0 at deficit=1.0 (zero-HP cat ⇒ PickUp
//!   fully suppressed). Composition is multiplicative so the body
//!   axis multiplies (damps) the food axis; this matches the
//!   structural intent ("wounds reduce pickup viability") rather than
//!   additive (which would lift the score on its own).
//!
//!   `health_deficit` (rather than the broader `body_distress_composite`)
//!   was chosen so HUNGER doesn't suppress pickup — a hungry cat is
//!   exactly when pickup is most useful. The dying-arc evidence
//!   (Calcifer at HP=0.49, Cedar at HP=0.38) is wound-specific, not
//!   hunger-driven; `health_deficit` directly captures it.
//!
//! Pre-231 the DSE scored from one Consideration only and ignored the
//! cat's body state, leading to wounded cats picking PickUp over
//! Sleep/Flee in the seed-42 dying-arc analysis (Calcifer at HP=0.49
//! choosing PickUp 0.96 over Flee 0.95; Cedar at HP=0.38 choosing
//! PickUp 0.99 over Sleep 1.08). Per substrate-over-override
//! discipline, the fix is to *subscribe* the DSE to existing body-
//! state perception rather than adding an eligibility filter.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub const SCAVENGE_INPUT: &str = "colony_food_security";
pub const HEALTH_DEFICIT_INPUT: &str = "health_deficit";

pub struct PickingUpDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl PickingUpDse {
    pub fn new() -> Self {
        // Scavenge urgency = inverted Logistic over `colony_food_security`.
        // Logistic(8, 0.5) gives a steep transition around 0.5; Invert
        // flips so low food-security → high score, high → ~0.
        let scavenge_urgency = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        // 231 R3b: health-deficit damping. Linear(slope=-1, intercept=1)
        // evaluates to 1.0 at deficit=0 (full HP → no suppression) and
        // 0.0 at deficit=1.0 (zero HP → full suppression). Pure-product
        // composition (strength=0) makes the damping multiplicative.
        let health_damping = Curve::Linear {
            slope: -1.0,
            intercept: 1.0,
        };
        Self {
            id: DseId("pick_up"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SCAVENGE_INPUT, scavenge_urgency)),
                Consideration::Scalar(ScalarConsideration::new(
                    HEALTH_DEFICIT_INPUT,
                    health_damping,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0]).with_compensation(0.0),
            eligibility: EligibilityFilter::new()
                .forbid(markers::Incapacitated::KEY)
                .require(markers::HasGroundCarcass::KEY),
        }
    }
}

impl Default for PickingUpDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for PickingUpDse {
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
                label: "picked_up_ground_item",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}
impl crate::ai::dse::CatDse for PickingUpDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::PickUp
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn picking_up_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(PickingUpDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_up_dse_id_stable() {
        assert_eq!(PickingUpDse::new().id().0, "pick_up");
    }

    #[test]
    fn picking_up_scavenges_when_food_security_low() {
        // 185: replaced 178's default-zero curve with an inverted
        // Logistic. Food-security 0.0 (colony starving / cat hungry)
        // should score near 1.0; food-security 1.0 (sated + stockpiled)
        // should score near 0.0.
        let dse = PickingUpDse::new();
        let c = match &dse.considerations()[0] {
            Consideration::Scalar(sc) => &sc.curve,
            _ => panic!("expected scalar"),
        };
        let low = c.evaluate(0.0);
        let mid = c.evaluate(0.5);
        let high = c.evaluate(1.0);
        // High urgency at low food-security.
        assert!(
            low > 0.9,
            "expected scavenge urgency >0.9 at food_security=0, got {low}"
        );
        // Symmetric around midpoint 0.5.
        assert!(
            (mid - 0.5).abs() < 1e-3,
            "expected scavenge urgency ≈0.5 at food_security=0.5, got {mid}"
        );
        // Low urgency at high food-security.
        assert!(
            high < 0.1,
            "expected scavenge urgency <0.1 at food_security=1, got {high}"
        );
    }

    #[test]
    fn picking_up_axis_is_food_security() {
        let dse = PickingUpDse::new();
        match &dse.considerations()[0] {
            Consideration::Scalar(sc) => assert_eq!(sc.name, SCAVENGE_INPUT),
            _ => panic!("expected ScalarConsideration"),
        }
    }

    #[test]
    fn picking_up_eligibility_requires_ground_carcass() {
        let dse = PickingUpDse::new();
        assert!(dse
            .eligibility()
            .required
            .contains(&markers::HasGroundCarcass::KEY));
    }

    #[test]
    fn picking_up_maslow_tier_is_one() {
        assert_eq!(PickingUpDse::new().maslow_tier(), 1);
    }

    /// 231 R3b: health-deficit axis is present at index 1 with the
    /// damping Linear curve. At deficit=0 it evaluates to 1.0
    /// (full-HP cat → no suppression, parity with pre-231 score).
    #[test]
    fn picking_up_health_deficit_axis_present_with_damping_linear() {
        let dse = PickingUpDse::new();
        match &dse.considerations()[1] {
            Consideration::Scalar(sc) => {
                assert_eq!(sc.name, HEALTH_DEFICIT_INPUT);
                let healthy = sc.curve.evaluate(0.0);
                let deficit_full = sc.curve.evaluate(1.0);
                assert!(
                    (healthy - 1.0).abs() < 1e-6,
                    "damping curve must evaluate to 1.0 at deficit=0 (parity); got {healthy}"
                );
                assert!(
                    deficit_full.abs() < 1e-6,
                    "damping curve must evaluate to 0.0 at deficit=1.0 (full suppression); got {deficit_full}"
                );
            }
            _ => panic!("expected ScalarConsideration"),
        }
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static PICKING_UP_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3650,
        construct: |_| picking_up_dse(),
    };
