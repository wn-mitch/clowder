//! `Build` — Work-urgency peer (§3.3.2 anchor = 1.0).
//!
//! Per §2.3 + §3.1.1 row 1493 (post-§L2.10.7): `WeightedSum` of 3
//! axes — diligence (Linear), site_distance
//! (`Composite{Logistic(8, 0.5), Invert}` over distance to nearest
//! construction site, replacing the retired binary `has_construction_site`
//! Piecewise axis), repair_presence (Piecewise `(0, 0),
//! (1, build_repair_bonus)`).
//! RtEO: site proximity drives even low-diligence cats ("there's
//! literally a half-built wall here"); repair need drives build
//! independently.
//!
//! Maslow tier 2 — Build is a safety-infrastructure action that
//! shouldn't be gated on pre-existing safety (chicken-and-egg per
//! the old inline comment), but a hungry cat still shouldn't build.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, MarkerConsideration, ScalarConsideration,
    SpatialConsideration,
};
use crate::ai::curves::{piecewise, Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

pub const DILIGENCE_INPUT: &str = "diligence";
pub const SITE_PRESENCE_INPUT: &str = "has_construction_site";
pub const REPAIR_PRESENCE_INPUT: &str = "has_damaged_building";
pub const CHRONIC_FULL_INPUT: &str = "colony_stores_chronically_full";
pub const SURPLUS_FOOD_INPUT: &str = "surplus_food_perceptible";

/// §L2.10.7 Build range — Manhattan tiles for the
/// nearest-construction-site anchor. 25 ≈ a long colony walk;
/// matches Cook/Eat/Farm range cluster.
pub const BUILD_SITE_RANGE: f32 = 25.0;

pub struct BuildDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl BuildDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // §L2.10.7 row Build: Composite{Logistic(8, 0.5), Invert} over
        // distance to the nearest construction site. Replaces the
        // binary `has_construction_site` Piecewise axis — distance to
        // the work IS the presence signal (None when no site nearby
        // → CP/WS gate suppresses the build score). The `build_site_bonus`
        // tunable retires; the curve's plateau gives the same "literally
        // a half-built wall here" pull at close range.
        let site_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        // Ethological colony-start: surplus-food demand axis. Nested via a
        // `(1 - w)` remainder so the RtEO sum stays 1.0 at any weight; ships
        // dormant at `build_surplus_food_weight = 0.0` → the four canonical
        // axes keep their full weights and this axis contributes nothing
        // (byte-identical to the pre-feature composition).
        let surplus_weight = scoring.build_surplus_food_weight.clamp(0.0, 1.0);
        let remainder = 1.0 - surplus_weight;
        Self {
            id: DseId("build"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(
                    DILIGENCE_INPUT,
                    Curve::Linear {
                        slope: 1.0,
                        intercept: 0.0,
                    },
                )),
                Consideration::Spatial(SpatialConsideration::new(
                    "build_site_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestConstructionSite),
                    BUILD_SITE_RANGE,
                    site_distance,
                )),
                // `has_damaged_building` retains its binary Piecewise
                // shape today: §L2.10.7's roster commits one landmark
                // per row (Site position), and damaged-building repair
                // is a distinct repair-pull signal that isn't named
                // separately in the spec. Future audit may split this
                // into a NearestDamagedBuilding anchor.
                Consideration::Scalar(ScalarConsideration::new(
                    REPAIR_PRESENCE_INPUT,
                    piecewise(vec![(0.0, 0.0), (1.0, scoring.build_repair_bonus)]),
                )),
                // 179: chronic-full demand axis. The
                // `ColonyStoresChronicallyFull` marker latches when
                // `DepositRejected` events have been chronic over a
                // window (authored by `update_colony_building_markers`,
                // wired through `colony_state_query` → `MarkerSnapshot`).
                // Reading it here gives the Build DSE a colony-demand
                // pull on Stores expansion that's distinct from the
                // instantaneous `stores_full` signal that
                // `assess_build_pressure` already tracks: the chronic
                // signal captures "cats keep trying to deposit and
                // failing," not just "Stores happens to be full this
                // tick." Tunable via `build_chronic_full_weight` —
                // ships at plausibility (`default_build_chronic_full_weight`).
                Consideration::Marker(MarkerConsideration::new(
                    CHRONIC_FULL_INPUT,
                    markers::ColonyStoresChronicallyFull::KEY,
                    scoring.build_chronic_full_weight,
                )),
                // Ethological colony-start: "there is ungathered food nearby
                // — build a larder to hold it." High belief → high score, no
                // invert (unlike Forage's saturation axis, this is a demand
                // signal, not a satiation one). Dormant at land.
                Consideration::Scalar(ScalarConsideration::new(
                    SURPLUS_FOOD_INPUT,
                    Curve::Logistic {
                        steepness: 8.0,
                        midpoint: 0.5,
                    },
                )),
            ],
            // RtEO sum = 1.0. Diligence is primary; spatial axis pulls
            // toward the site; repair-presence and chronic-full demand
            // are auxiliary pull signals (each smaller than spatial so
            // diligence + site dominate when no repair / chronic demand
            // exists). The surplus-food axis is nested via `(1 - surplus_weight)`
            // on the four canonical weights so the sum stays 1.0; dormant at 0.0.
            composition: Composition::weighted_sum(vec![
                0.4 * remainder,
                0.25 * remainder,
                0.20 * remainder,
                0.15 * remainder,
                surplus_weight,
            ]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Dse for BuildDse {
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
            state: GoalState::predicate("built_or_repaired", |_, _| false),
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}
impl crate::ai::dse::CatDse for BuildDse {
    fn action(&self) -> crate::ai::Action {
        crate::ai::Action::Build
    }

    fn life_stages(&self) -> crate::ai::dse::LifeStageSet {
        crate::ai::dse::LifeStageSet::adults_young_elder()
    }
}

pub fn build_dse(scoring: &ScoringConstants) -> Box<dyn crate::ai::dse::CatDse> {
    Box::new(BuildDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dse_id_stable() {
        let s = ScoringConstants::default();
        assert_eq!(BuildDse::new(&s).id().0, "build");
    }

    #[test]
    fn build_weights_sum_to_one() {
        let s = ScoringConstants::default();
        let sum: f32 = BuildDse::new(&s).composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn build_consideration_count_is_five() {
        let s = ScoringConstants::default();
        // diligence + site_distance + repair_presence + chronic_full
        // + surplus_food (ethological colony-start)
        assert_eq!(BuildDse::new(&s).considerations().len(), 5);
    }

    #[test]
    fn build_surplus_axis_dormant_at_default_zero() {
        let s = ScoringConstants::default();
        assert_eq!(s.build_surplus_food_weight, 0.0);
        let dse = BuildDse::new(&s);
        let weights = &dse.composition().weights;
        // Five axes; the surplus axis (last) is 0.0 at default, and the
        // four canonical weights are their original values (× remainder=1).
        assert_eq!(weights.len(), 5);
        assert_eq!(weights[4], 0.0);
        assert!((weights[0] - 0.4).abs() < 1e-6);
        assert!((weights[1] - 0.25).abs() < 1e-6);
        assert!((weights[2] - 0.20).abs() < 1e-6);
        assert!((weights[3] - 0.15).abs() < 1e-6);
    }

    #[test]
    fn build_surplus_axis_preserves_sum_when_active() {
        let mut s = ScoringConstants::default();
        s.build_surplus_food_weight = 0.3;
        let dse = BuildDse::new(&s);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-4,
            "RtEO sum must stay 1.0; got {sum}"
        );
        assert!((dse.composition().weights[4] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn build_chronic_full_axis_reads_colony_marker() {
        let s = ScoringConstants::default();
        let dse = BuildDse::new(&s);
        let chronic = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Marker(m) if m.name == CHRONIC_FULL_INPUT => Some(m),
                _ => None,
            })
            .expect("Build DSE must include the chronic-full MarkerConsideration");
        assert_eq!(chronic.marker, markers::ColonyStoresChronicallyFull::KEY);
        // Plausibility default — ships nonzero so the marker actually
        // lifts Build score when set.
        assert!(chronic.present_score > 0.0);
    }
}

#[linkme::distributed_slice(crate::ai::dses::CAT_DSE_REGISTRY)]
static BUILD_REGISTRATION: crate::ai::dses::CatDseRegistration =
    crate::ai::dses::CatDseRegistration {
        order: 1500,
        construct: |s| build_dse(s),
    };
