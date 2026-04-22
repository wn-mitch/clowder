//! Fox `Raiding` — fox-side peer of cat `Hunt`/`Forage` and fox
//! `Hunting` in the Starvation-urgency peer group (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 fox rows: `CompensatedProduct` of two axes —
//! `hunger_urgency` (via `hangry()` anchor) and `cunning` (Linear
//! identity). Maslow tier 1.
//!
//! Design intent per §3.1.1: "Both gate: raiding requires cleverness;
//! no hunger ⇒ no reason to risk colony contact." CP with n=2 means
//! either axis at 0 zeroes the DSE — a cunning but well-fed fox won't
//! raid; a starving but dim fox won't raid either.
//!
//! **Eligibility gate.** The old inline block requires
//! `store_visible && !store_guarded`. §2.3 formalizes this as a
//! context-tag filter (§4) — markers like `HasVisibleStore` +
//! `!StoreGuarded`. Phase 3c.1b keeps the gate at the outer
//! `score_fox_dispositions` level (same pattern as Eat's
//! `food_available` outer gate through 3c.1a); Phase 3d flips it to
//! marker-driven eligibility when the authoring systems land.
//!
//! **Magnitude delta vs. inline.** Old formula:
//! `hunger_urgency × cunning × 1.2 × l1` — the `× 1.2` amplifier
//! could push the score above 1.0. Under CP with `hangry()` ceiling
//! ≈ 0.88 and cunning ≤ 1.0, the CP peak is ≈ 0.88 before
//! compensation. With `DEFAULT_COMPENSATION_STRENGTH = 0.75` and n=2,
//! the compensated peak lands near the geometric mean — still bounded
//! by 1.0, matching §3.3.2's peer-group anchor.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::{hangry, Curve};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};

pub const HUNGER_INPUT: &str = "hunger_urgency";
pub const CUNNING_INPUT: &str = "cunning";

pub struct FoxRaidingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxRaidingDse {
    pub fn new() -> Self {
        Self {
            id: DseId("fox_raiding"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(HUNGER_INPUT, hangry())),
                Consideration::Scalar(ScalarConsideration::new(
                    CUNNING_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
            ],
            // RtM weights: both axes at their natural ceiling. Hunger
            // saturates via hangry() (asymptote ≈ 0.88); cunning is a
            // `[0, 1]` personality coefficient. Both=1.0 means CP's
            // per-axis ceilings apply unscaled.
            composition: Composition::compensated_product(vec![1.0, 1.0]),
            eligibility: EligibilityFilter::new(),
        }
    }
}

impl Default for FoxRaidingDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for FoxRaidingDse {
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
                label: "food_from_store",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        1
    }
}

pub fn fox_raiding_dse() -> Box<dyn Dse> {
    Box::new(FoxRaidingDse::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fox_raiding_dse_id_stable() {
        assert_eq!(FoxRaidingDse::new().id().0, "fox_raiding");
    }

    #[test]
    fn fox_raiding_has_two_axes() {
        assert_eq!(FoxRaidingDse::new().considerations().len(), 2);
    }

    #[test]
    fn fox_raiding_is_compensated_product() {
        use crate::ai::composition::CompositionMode;
        assert_eq!(
            FoxRaidingDse::new().composition().mode,
            CompositionMode::CompensatedProduct
        );
    }

    #[test]
    fn fox_raiding_maslow_tier_is_one() {
        assert_eq!(FoxRaidingDse::new().maslow_tier(), 1);
    }

    #[test]
    fn fox_raiding_cp_weights_in_unit_interval() {
        let dse = FoxRaidingDse::new();
        assert!(dse.composition().weights.iter().all(|w| (0.0..=1.0).contains(w)));
    }
}
