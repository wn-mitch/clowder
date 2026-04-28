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
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{hangry, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::ai::faction::StanceRequirement;

pub const HUNGER_INPUT: &str = "hunger_urgency";
pub const CUNNING_INPUT: &str = "cunning";

/// §L2.10.7 fox Raiding range — Manhattan tiles for the
/// nearest-visible-store anchor. 12 mirrors the fox_goap.rs
/// store-visibility radius (`store_visible` ≤ 12 tiles); foxes
/// outside this radius fail the visibility predicate entirely.
pub const FOX_RAIDING_STORE_RANGE: f32 = 12.0;

pub struct FoxRaidingDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl FoxRaidingDse {
    pub fn new() -> Self {
        // §L2.10.7 row Raiding: Composite{Logistic(8, 0.5), Invert}
        // over distance to nearest visible store. Spec line 5651:
        // 'Commute-to-target with guard-deterrent handled as a
        // separate scalar.' None when no store in range — CP gates
        // the DSE to 0.
        let store_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
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
                Consideration::Spatial(SpatialConsideration::new(
                    "fox_raiding_store_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestVisibleStore),
                    FOX_RAIDING_STORE_RANGE,
                    store_distance,
                )),
            ],
            // RtM weights: hunger via hangry() (≈0.88), cunning via
            // Linear identity, store distance via Logistic-Invert
            // (≈0.98 at zero distance). All three at 1.0 lets CP's
            // natural per-axis ceilings dominate.
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §9.3 DSE filter binding — FoxRaid treats colony stores /
            // cats as `Prey` per the fox→cat row (`StoreVisible` marker
            // refinement remains an outer gate — §4 port is Phase 3d).
            eligibility: EligibilityFilter::new().with_stance(StanceRequirement::hunt()),
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
    fn fox_raiding_has_three_axes() {
        // §L2.10.7: hunger + cunning + store_distance.
        assert_eq!(FoxRaidingDse::new().considerations().len(), 3);
    }

    #[test]
    fn fox_raiding_uses_visible_store_anchor() {
        let dse = FoxRaidingDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(sp) if sp.name == "fox_raiding_store_distance" => Some(sp),
                _ => None,
            })
            .expect("fox_raiding_store_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::NearestVisibleStore)
        ));
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
        assert!(dse
            .composition()
            .weights
            .iter()
            .all(|w| (0.0..=1.0).contains(w)));
    }

    #[test]
    fn fox_raiding_stance_requirement_is_prey() {
        use crate::ai::faction::FactionStance;
        let req = FoxRaidingDse::new()
            .eligibility()
            .required_stance
            .clone()
            .expect("§9.3 binding must populate required_stance");
        // `StanceRequirement::hunt()` (Prey) — §9.3's FoxRaidDse row.
        // The `StoreVisible` marker refinement lands with §4 in Phase 3d.
        assert!(req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Same));
    }
}
