//! `Cook` — peer of Eat/Hunt/Forage in the Starvation-urgency group.
//!
//! Per §3.1.1: "Base rate and scarcity urgency trade off — cooking
//! is ongoing activity plus scarcity response, not strictly gated on
//! either." WeightedSum of `base_rate + food_scarcity + diligence`.
//!
//! Maslow tier 2 — comment at `scoring.rs:738` names Cook as a
//! food-buffer multiplier analogous to Farm, "Level 2 suppression
//! (phys only)."
//!
//! **Cook-specific eligibility** — §4 port (Phase 4b.5) carries two
//! colony-scoped markers: `.require("HasFunctionalKitchen")` and
//! `.require("HasRawFoodInStores")`. The third precondition —
//! `hunger > cook_hunger_gate` — is a scalar threshold, not a marker,
//! and stays as an inline caller-side wrap so Cook isn't scored while
//! the cat is too stuffed. The `wants_cook_but_no_kitchen` latent
//! signal read by BuildPressure in `goap.rs` is preserved by the
//! caller reading `ctx.has_raw_food_in_stores` / `has_functional_kitchen`
//! directly when the DSE's marker-gated score drops to zero.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{scarcity, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;

pub struct CookDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CookDse {
    pub fn new() -> Self {
        Self {
            id: DseId("cook"),
            considerations: vec![
                // base_rate: constant "cooking is always worth
                // something when eligible." Input is a dummy "one"
                // (always 1.0); the Linear slope carries the magnitude.
                Consideration::Scalar(ScalarConsideration::new(
                    "one",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Scalar(ScalarConsideration::new("food_scarcity", scarcity())),
                Consideration::Scalar(ScalarConsideration::new(
                    "diligence",
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            // RtEO sum = 1.0. Base rate carries the "always worth
            // doing" floor; scarcity escalates under colony stress;
            // diligence is the personality weight.
            composition: Composition::weighted_sum(vec![0.4, 0.3, 0.3]),
            // §4 batch 2: `.require(CanCook)` gates on Adult ∧ ¬Injured.
            // Colony-scoped kitchen/food markers stay here (not bundled
            // into CanCook) to preserve the `wants_cook_but_no_kitchen`
            // build-pressure signal in `scoring.rs`.
            // §4 Phase 4b.5: colony-scoped kitchen + raw food gates.
            // §13.1: `.forbid(Incapacitated)` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::CanCook::KEY)
                .require(markers::HasFunctionalKitchen::KEY)
                .require(markers::HasRawFoodInStores::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for CookDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CookDse {
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
                label: "food_cooked_at_kitchen",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn cook_dse() -> Box<dyn Dse> {
    Box::new(CookDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    #[test]
    fn cook_dse_id_stable() {
        assert_eq!(CookDse::new().id().0, "cook");
    }

    #[test]
    fn cook_weights_sum_to_one() {
        let dse = CookDse::new();
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn cook_is_maslow_tier_2() {
        assert_eq!(CookDse::new().maslow_tier(), 2);
    }

    #[test]
    fn cook_dse_requires_capability_and_colony_markers() {
        // §4 batch 2: Cook requires CanCook (Adult ∧ ¬Injured) plus
        // both colony markers.
        let dse = CookDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanCook::KEY,
                markers::HasFunctionalKitchen::KEY,
                markers::HasRawFoodInStores::KEY,
            ]
        );
        // §13.1: every non-Eat/Sleep/Idle cat DSE forbids Incapacitated.
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }

    fn evaluate_cook_with_markers(
        has_kitchen: bool,
        has_raw_food: bool,
    ) -> Option<crate::ai::eval::ScoredDse> {
        let dse = CookDse::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = move |name: &str, _: Entity| match name {
            n if n == markers::CanCook::KEY => true,
            n if n == markers::HasFunctionalKitchen::KEY => has_kitchen,
            n if n == markers::HasRawFoodInStores::KEY => has_raw_food,
            _ => false,
        };
        let entity_position = |_: Entity| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        // Supply non-zero scalars so the DSE would score > 0 if eligible.
        let fetch = |name: &str, _: Entity| match name {
            "one" => 1.0,
            "food_scarcity" => 0.8,
            "diligence" => 0.6,
            _ => 0.0,
        };
        evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch)
    }

    #[test]
    fn cook_dse_rejected_without_has_functional_kitchen_marker() {
        // Raw food present, kitchen absent — the marker-gate rejects.
        assert!(evaluate_cook_with_markers(false, true).is_none());
    }

    #[test]
    fn cook_dse_rejected_without_has_raw_food_in_stores_marker() {
        // Kitchen present, raw food absent — the marker-gate rejects.
        assert!(evaluate_cook_with_markers(true, false).is_none());
    }

    #[test]
    fn cook_dse_eligible_with_both_markers() {
        // Inverse of the two rejection tests — confirms the DSE scores
        // positively when both colony markers are set.
        let scored = evaluate_cook_with_markers(true, true).expect("eligible with both markers");
        assert!(scored.final_score > 0.0);
    }
}
