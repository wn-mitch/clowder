//! `BegForFood` — kitten-side hunger-driven Activity DSE.
//!
//! Ticket 450 §4. Realizes the user-stated intent ("kittens want to
//! eat, GOAP eligibility prevents it so food falls onto the 'beg until
//! I get it' track") per `docs/systems/ai-substrate-refactor.md`
//! §L2.10.5: Begging is an `Intention::Activity(Begging,
//! UntilInterrupt)` — sibling-shape to Idle / Patrol / Socialize — not
//! a goal-state-achievement. The activity's drop trigger IS the
//! desire-drift condition (kitten gains `HasFoodInInventory` →
//! Eat-side DSE outscores; or kitten matures past Stage 2 →
//! eligibility filter excludes), so it pairs with `OpenMinded`
//! commitment per §L2.10.5's strategy-shape correlation.
//!
//! ## Two sibling registrations (no OR combinator per §4.7.3)
//!
//! Both registrations construct the same scoring shape and emit the
//! same Activity Intention; they differ only in their
//! `EligibilityFilter` require-marker (`NewbornKitten::KEY` vs
//! `EyesOpenKitten::KEY`). Both `.forbid(HasFoodInInventory::KEY)` so
//! a kitten that already carries food doesn't beg — the Eat-side
//! method (429 Phase 2) outscores instead.
//!
//! Stage 3 (`JuvenileKitten`) kittens are explicitly NOT eligible to
//! beg — by the time a kitten can forage, mentoring is the right
//! channel for learning provisioning. Two sibling DSEs (not three)
//! encode this.
//!
//! ## Scoring shape
//!
//! Single-axis on hunger urgency via the canonical
//! [`crate::ai::curves::hangry`] anchor (§2.3 Eat row). `WeightedSum`
//! composition (n=1 today; future axes — e.g. "is a parent in
//! earshot?" — would compose under WS additively, distinct from Eat's
//! `CompensatedProduct` shape because the spatial axis there gates
//! eligibility, whereas a parent-in-earshot axis here would *modulate*
//! the cry's urgency without gating).
//!
//! ## Spec rows
//!
//! | Axis | Shape | Rationale |
//! |---|---|---|
//! | §2.3 `BegForFood.hunger` | `Logistic(steepness=8, midpoint=0.5)` (`hangry()`) | Inherits the hangry anchor — a half-hungry kitten begs ~half-strongly. Reusing the curve keeps Stage 1/2 kittens' "I'm hungry" perception monotonically aligned with adults' Eat scoring. |
//! | §3.1.1 `BegForFood` | `WeightedSum(weights=[1.0])` | n=1 today; WS keeps the door open for future composition (parent-in-earshot, parent-grooming-me, etc.) under additive semantics. |
//! | §L2.10.5 Intention | `Activity(Begging, UntilInterrupt)` | Sustained signaling — the kitten cries continuously until something preempts (parent arrives, food appears, kitten matures past Stage 2). |
//! | Strategy | `OpenMinded` | §L2.10.5 strategy-shape correlation — Activity + UntilInterrupt → OpenMinded. |
//! | Maslow tier | 1 | Physiological-adjacent — driven by hunger; not satiated by the activity itself, but the *want* is hunger-driven. |

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::hangry;
use crate::ai::dse::{
    ActivityKind, CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention,
    Termination,
};
use crate::components::markers;

/// Shared scalar input name — read by the evaluator's `fetch_scalar`
/// closure against the kitten's hunger urgency (`1 - Needs.hunger`).
/// Matches Eat's HUNGER_INPUT key so both DSEs draw from the same
/// `ctx_scalars` slot — no duplicated scalar surface for the same
/// underlying signal.
pub const HUNGER_INPUT: &str = "hunger_urgency";

pub struct BegForFoodDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl BegForFoodDse {
    /// Stage 1 sibling — `NewbornKitten` ∧ ¬`HasFoodInInventory`.
    pub fn newborn() -> Self {
        Self::with_eligibility(
            EligibilityFilter::new()
                .require(markers::NewbornKitten::KEY)
                .forbid(markers::HasFoodInInventory::KEY),
        )
    }

    /// Stage 2 sibling — `EyesOpenKitten` ∧ ¬`HasFoodInInventory`.
    pub fn eyes_open() -> Self {
        Self::with_eligibility(
            EligibilityFilter::new()
                .require(markers::EyesOpenKitten::KEY)
                .forbid(markers::HasFoodInInventory::KEY),
        )
    }

    fn with_eligibility(eligibility: EligibilityFilter) -> Self {
        Self {
            id: DseId("beg_for_food"),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                HUNGER_INPUT,
                hangry(),
            ))],
            composition: Composition::weighted_sum(vec![1.0]),
            eligibility,
        }
    }
}

impl Dse for BegForFoodDse {
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
        // §L2.10.5 strategy-shape correlation: Activity + UntilInterrupt → OpenMinded.
        CommitmentStrategy::OpenMinded
    }

    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Begging,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }

    fn maslow_tier(&self) -> u8 {
        // Tier-1 physiological-adjacent — see DispositionKind::Begging doc
        // for the "hunger-driven want even though the activity itself
        // doesn't satiate" rationale.
        1
    }
}

impl crate::ai::dse::CatDse for BegForFoodDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::BegForFood
    }
}

/// Stage 1 constructor (used by the registry). Public so headless / scenario
/// harness code can build a fresh instance for assertion.
pub fn beg_for_food_newborn_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(BegForFoodDse::newborn())
}

/// Stage 2 constructor.
pub fn beg_for_food_eyes_open_dse() -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(BegForFoodDse::eyes_open())
}

// ---------------------------------------------------------------------------
// Distributed-slice registrations — two siblings, both keyed to the
// same DseId ("beg_for_food") with different eligibility markers.
// Order slots 3750 / 3760 sit after Idle (3700, the prior tail) so
// adult-cat seed-42 RNG draws are not perturbed (adult cats fail both
// eligibility filters on the marker check before any RNG is consumed).
// ---------------------------------------------------------------------------

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static BEG_FOR_FOOD_NEWBORN_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3750,
        construct: |_| beg_for_food_newborn_dse(),
    };

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static BEG_FOR_FOOD_EYES_OPEN_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 3760,
        construct: |_| beg_for_food_eyes_open_dse(),
    };

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beg_for_food_id_is_stable_across_siblings() {
        assert_eq!(BegForFoodDse::newborn().id().0, "beg_for_food");
        assert_eq!(BegForFoodDse::eyes_open().id().0, "beg_for_food");
    }

    #[test]
    fn newborn_sibling_requires_newborn_marker() {
        let dse = BegForFoodDse::newborn();
        assert_eq!(
            dse.eligibility().required,
            vec![markers::NewbornKitten::KEY]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::HasFoodInInventory::KEY]
        );
    }

    #[test]
    fn eyes_open_sibling_requires_eyes_open_marker() {
        let dse = BegForFoodDse::eyes_open();
        assert_eq!(
            dse.eligibility().required,
            vec![markers::EyesOpenKitten::KEY]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::HasFoodInInventory::KEY]
        );
    }

    #[test]
    fn emits_activity_intention_with_open_minded_strategy() {
        let dse = BegForFoodDse::newborn();
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| None;
        let anchor_position =
            |_: crate::ai::considerations::LandmarkAnchor| -> Option<crate::components::physical::Position> {
                None
            };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: crate::components::physical::Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };
        let intention = dse.emit(0.5, &ctx);
        assert!(intention.is_activity(), "expected Activity intention");
        assert_eq!(intention.strategy(), CommitmentStrategy::OpenMinded);
    }

    #[test]
    fn maslow_tier_is_one() {
        assert_eq!(BegForFoodDse::newborn().maslow_tier(), 1);
        assert_eq!(BegForFoodDse::eyes_open().maslow_tier(), 1);
    }

    #[test]
    fn cat_dse_action_is_beg_for_food() {
        use crate::ai::dse::CatDse;
        assert_eq!(
            BegForFoodDse::newborn().action(),
            crate::ai::Action::BegForFood
        );
        assert_eq!(
            BegForFoodDse::eyes_open().action(),
            crate::ai::Action::BegForFood
        );
    }
}
