//! `PracticeMagic::*` — sibling-DSE split from the retiring cat
//! `PracticeMagic` inline block (§L2.10.10). Six sub-modes lifted
//! into six standalone DSEs; the outer hint-selection stays in the
//! scorer until §L2.10 closes out, but the three corruption-emergency
//! `ScoreModifier` bonuses (`WardCorruptionEmergency`,
//! `CleanseEmergency`, `SensedRotBoost`) retire in §13.1 — their flat
//! additive contributions are now produced at the axis level through
//! Logistic curves per §2.3's "Retired constants" subsection.
//!
//! All six share the PracticeMagic eligibility contract:
//! `magic_affinity > magic_affinity_threshold && magic_skill >
//! magic_skill_threshold` — handled by the outer gate in
//! `score_actions` until §4 markers port in Phase 3d.

use bevy::prelude::*;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{
    CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, GoalState, Intention,
};
use crate::components::markers;
use crate::resources::sim_constants::ScoringConstants;

fn linear() -> Curve {
    Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Scry — curiosity-driven divination
// ---------------------------------------------------------------------------

pub struct ScryDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl ScryDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("curiosity", linear())),
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for ScryDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ScryDse {
    fn id(&self) -> DseId {
        DseId("magic_scry")
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
        // §7.3: Scry is a constituent action of the Crafting
        // disposition and rides Crafting's `SingleMinded` strategy.
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "scried",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        5
    }
}

pub fn scry_dse() -> Box<dyn Dse> {
    Box::new(ScryDse::new())
}

// ---------------------------------------------------------------------------
// DurableWard — spirituality × magic_skill × ward_deficit ×
//               nearby_corruption_level
// ---------------------------------------------------------------------------

pub struct DurableWardDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl DurableWardDse {
    pub fn new() -> Self {
        // §2.3 row 6: Logistic(8, 0.1) on `nearby_corruption_level`
        // collapses the old threshold-gate + linear-scale pair
        // (`corruption_sensed_response_bonus` modifier) into one
        // axis-level primitive. The flat additive bonus retires.
        let nearby_corruption = Curve::Logistic {
            steepness: 8.0,
            midpoint: 0.1,
        };
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new("ward_deficit", linear())),
                Consideration::Scalar(ScalarConsideration::new(
                    "nearby_corruption_level",
                    nearby_corruption,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0, 1.0]),
            // §4 marker eligibility (Phase 4b.5): DurableWard only
            // scores when colony ward strength is low. Retires the
            // `ctx.ward_strength_low` conjunct from the outer gate at
            // `scoring.rs:775-780`. The
            // `magic_skill > magic_durable_ward_skill_threshold`
            // conjunct stays inline — magic_skill is a §4.5 scalar,
            // not a marker.
            // §13.1: `.forbid(markers::Incapacitated::KEY)` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::WardStrengthLow::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for DurableWardDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for DurableWardDse {
    fn id(&self) -> DseId {
        DseId("magic_durable_ward")
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
                label: "durable_ward_placed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn durable_ward_dse() -> Box<dyn Dse> {
    Box::new(DurableWardDse::new())
}

// ---------------------------------------------------------------------------
// Cleanse — spirituality × magic_skill × tile_corruption
// ---------------------------------------------------------------------------

pub struct CleanseDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CleanseDse {
    pub fn new(scoring: &ScoringConstants) -> Self {
        // §2.3 row 5: Logistic(8, magic_cleanse_corruption_threshold)
        // on `tile_corruption`. Threshold-gated cleansing — a
        // corrupted tile is a "now" problem, not a ramp. Absorbs the
        // retiring `cleanse_corruption_emergency_bonus` flat additive
        // bonus into the axis curve itself.
        let tile_corruption = Curve::Logistic {
            steepness: 8.0,
            midpoint: scoring.magic_cleanse_corruption_threshold,
        };
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new("tile_corruption", tile_corruption)),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for CleanseDse {
    fn default() -> Self {
        Self::new(&ScoringConstants::default())
    }
}

impl Dse for CleanseDse {
    fn id(&self) -> DseId {
        DseId("magic_cleanse")
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
                label: "tile_cleansed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn cleanse_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(CleanseDse::new(scoring))
}

// ---------------------------------------------------------------------------
// ColonyCleanse — large-scale corruption response
// ---------------------------------------------------------------------------

pub struct ColonyCleanseDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

/// Manhattan range over which the corruption-centroid distance is
/// normalized for ColonyCleanse. 30 tiles ≈ territory-wide; cats
/// farther than this rarely commute solely to ColonyCleanse.
pub const COLONY_CLEANSE_CENTROID_RANGE: f32 = 30.0;

impl ColonyCleanseDse {
    pub fn new() -> Self {
        // §L2.10.7 row 'PracticeMagic': Power curve over distance to
        // the territory-wide corruption centroid. `Composite {
        // Polynomial(exp=2, divisor=1), Invert }` evaluates
        // `1 - (cost)^2`: at the centroid score=1 (urgent cleanse),
        // half-distance → 0.75, edge → 0. CorruptionLandmarks (A1.2)
        // returns None when no tile is above the corruption floor;
        // the consideration scores 0.0 in that case via the substrate
        // dispatcher and the CP gate suppresses the DSE entirely.
        // Replaces the retired `territory_max_corruption` scalar
        // axis (Logistic(6, 0.3)) — the centroid is the §5.6
        // scent-like spread's geographic anchor.
        let centroid_distance = Curve::Composite {
            inner: Box::new(Curve::Polynomial {
                exponent: 2,
                divisor: 1.0,
            }),
            post: PostOp::Invert,
        };
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Spatial(SpatialConsideration::new(
                    "colony_cleanse_centroid_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::TerritoryCorruptionCentroid),
                    COLONY_CLEANSE_CENTROID_RANGE,
                    centroid_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for ColonyCleanseDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for ColonyCleanseDse {
    fn id(&self) -> DseId {
        DseId("magic_colony_cleanse")
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
                label: "colony_cleansed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn colony_cleanse_dse() -> Box<dyn Dse> {
    Box::new(ColonyCleanseDse::new())
}

// ---------------------------------------------------------------------------
// Harvest — carcass harvesting
// ---------------------------------------------------------------------------

pub struct HarvestDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HarvestDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("curiosity", linear())),
                Consideration::Scalar(ScalarConsideration::new("herbcraft_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new(
                    "carcass_count_saturated",
                    linear(),
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for HarvestDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HarvestDse {
    fn id(&self) -> DseId {
        DseId("magic_harvest")
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
                label: "carcass_harvested",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        3
    }
}

pub fn harvest_dse() -> Box<dyn Dse> {
    Box::new(HarvestDse::new())
}

// ---------------------------------------------------------------------------
// Commune — special-terrain communion
// ---------------------------------------------------------------------------

pub struct CommuneDse {
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl CommuneDse {
    pub fn new() -> Self {
        Self {
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new("spirituality", linear())),
                Consideration::Scalar(ScalarConsideration::new("magic_skill", linear())),
                Consideration::Scalar(ScalarConsideration::new("on_special_terrain", linear())),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §13.1: incapacitated cats can only Eat/Sleep/Idle.
            eligibility: EligibilityFilter::new().forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for CommuneDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for CommuneDse {
    fn id(&self) -> DseId {
        DseId("magic_commune")
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
        // §7.3: Commune is a constituent action of the Crafting
        // disposition and rides Crafting's `SingleMinded` strategy.
        CommitmentStrategy::SingleMinded
    }
    fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
        Intention::Goal {
            state: GoalState {
                label: "communed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        5
    }
}

pub fn commune_dse() -> Box<dyn Dse> {
    Box::new(CommuneDse::new())
}

#[cfg(test)]
mod tests {
    use crate::ai::considerations::LandmarkAnchor;
    use super::*;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    fn scalar_curve(dse: &dyn Dse, axis: &str) -> Option<Curve> {
        dse.considerations().iter().find_map(|c| match c {
            Consideration::Scalar(s) if s.name == axis => Some(s.curve.clone()),
            _ => None,
        })
    }

    #[test]
    fn all_six_practice_magic_ids_stable() {
        let sc = ScoringConstants::default();
        assert_eq!(scry_dse().id().0, "magic_scry");
        assert_eq!(durable_ward_dse().id().0, "magic_durable_ward");
        assert_eq!(cleanse_dse(&sc).id().0, "magic_cleanse");
        assert_eq!(colony_cleanse_dse().id().0, "magic_colony_cleanse");
        assert_eq!(harvest_dse().id().0, "magic_harvest");
        assert_eq!(commune_dse().id().0, "magic_commune");
    }

    #[test]
    fn durable_ward_requires_ward_strength_low() {
        // Phase 4b.5: the outer `ctx.ward_strength_low` conjunct at
        // `scoring.rs:775-780` retires; WardStrengthLow moves onto the
        // DSE's eligibility filter. §13.1: every sibling DSE carries
        // `.forbid(markers::Incapacitated::KEY)` (required emptiness asserted
        // separately below).
        let sc = ScoringConstants::default();
        let dse = DurableWardDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![markers::WardStrengthLow::KEY]
        );
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );

        // Guard against accidental spread of `require` to sibling DSEs
        // in this file — only DurableWard requires WardStrengthLow.
        assert!(ScryDse::new().eligibility().required.is_empty());
        assert!(CleanseDse::new(&sc).eligibility().required.is_empty());
        assert!(ColonyCleanseDse::new().eligibility().required.is_empty());
        assert!(HarvestDse::new().eligibility().required.is_empty());
        assert!(CommuneDse::new().eligibility().required.is_empty());
    }

    #[test]
    fn every_practice_magic_dse_forbids_incapacitated() {
        // §13.1: incapacitated cats retire the inline branch; the
        // `.forbid(markers::Incapacitated::KEY)` filter is the only remaining gate.
        let sc = ScoringConstants::default();
        assert_eq!(
            ScryDse::new().eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
        assert_eq!(
            DurableWardDse::new().eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
        assert_eq!(
            CleanseDse::new(&sc).eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
        assert_eq!(
            ColonyCleanseDse::new().eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
        assert_eq!(
            HarvestDse::new().eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
        assert_eq!(
            CommuneDse::new().eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }

    #[test]
    fn durable_ward_rejected_without_ward_strength_low_marker() {
        let dse = DurableWardDse::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.7_f32;
        assert!(evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).is_none());
    }

    // -----------------------------------------------------------------
    // §13.1 row 4/5/6 retired-constants curve witnesses.
    // -----------------------------------------------------------------

    #[test]
    fn durable_ward_has_nearby_corruption_axis_logistic_8_01() {
        // §2.3 row 6: Logistic(8, 0.1) on `nearby_corruption_level`
        // absorbs the retiring `corruption_sensed_response_bonus`
        // threshold-gate + linear-scale pair into one primitive.
        let dse = DurableWardDse::new();
        let names: Vec<&str> = dse
            .considerations()
            .iter()
            .map(|c| match c {
                Consideration::Scalar(s) => s.name,
                _ => "",
            })
            .collect();
        assert!(names.contains(&"nearby_corruption_level"));
        assert_eq!(dse.considerations().len(), 4);

        let curve = scalar_curve(&dse, "nearby_corruption_level")
            .expect("nearby_corruption_level axis must exist");
        // 0.0 → 0.3100; 0.1 (midpoint) → 0.5; 0.5 → 0.9608;
        // 1.0 → ≈ 0.9993.
        assert!(approx(curve.evaluate(0.0), 0.3100, 1e-3));
        assert!(approx(curve.evaluate(0.1), 0.5, 1e-4));
        assert!(approx(curve.evaluate(0.2), 0.6900, 1e-3));
        assert!(approx(curve.evaluate(0.5), 0.9608, 1e-3));
        assert!(approx(curve.evaluate(1.0), 0.9993, 1e-3));
        match curve {
            Curve::Logistic {
                steepness,
                midpoint,
            } => {
                assert!(approx(steepness, 8.0, 1e-6));
                assert!(approx(midpoint, 0.1, 1e-6));
            }
            other => panic!("expected Logistic(8, 0.1); got {other:?}"),
        }
    }

    #[test]
    fn cleanse_tile_corruption_axis_is_logistic_at_threshold() {
        // §2.3 row 5: Logistic(8, magic_cleanse_corruption_threshold).
        // Default threshold is 0.1 so the shape mirrors the
        // corruption-emergency midpoint anchor.
        let sc = ScoringConstants::default();
        let dse = CleanseDse::new(&sc);
        let curve = scalar_curve(&dse, "tile_corruption").expect("tile_corruption axis must exist");
        let midpoint = sc.magic_cleanse_corruption_threshold;
        assert!(approx(curve.evaluate(midpoint), 0.5, 1e-4));
        // Above the threshold the curve surges sharply.
        assert!(curve.evaluate(midpoint + 0.1) > 0.6);
        // Well below the threshold it is small but non-zero.
        assert!(curve.evaluate(0.0) < 0.5);
        match curve {
            Curve::Logistic {
                steepness,
                midpoint: m,
            } => {
                assert!(approx(steepness, 8.0, 1e-6));
                assert!(approx(m, midpoint, 1e-6));
            }
            other => panic!("expected Logistic(8, ·); got {other:?}"),
        }
    }

    #[test]
    fn cleanse_tile_corruption_uses_runtime_threshold() {
        // Confirm the factory reads the midpoint from
        // ScoringConstants rather than hardcoding — tuning the
        // constant shifts the axis shape.
        let mut sc = ScoringConstants::default();
        sc.magic_cleanse_corruption_threshold = 0.4;
        let dse = CleanseDse::new(&sc);
        let curve = scalar_curve(&dse, "tile_corruption").unwrap();
        assert!(approx(curve.evaluate(0.4), 0.5, 1e-4));
    }

    #[test]
    fn colony_cleanse_uses_corruption_centroid_anchor() {
        // §L2.10.7 row PracticeMagic (colony side): Power-Invert
        // curve (`Composite { Polynomial(exp=2, divisor=1), Invert }`)
        // over distance to the territory corruption centroid. At
        // cost=0 (cat at centroid) → 1.0; at cost=1 (range edge) → 0.
        // Replaces the retired `territory_max_corruption` scalar
        // axis (formerly Logistic(6, 0.3)).
        let dse = ColonyCleanseDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(s)
                    if s.name == "colony_cleanse_centroid_distance" =>
                {
                    Some(s)
                }
                _ => None,
            })
            .expect("colony_cleanse_centroid_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::TerritoryCorruptionCentroid)
        ));
        // (1 - cost^2): 0 → 1, 0.5 → 0.75, 1 → 0
        assert!(approx(spatial.curve.evaluate(0.0), 1.0, 1e-4));
        assert!(approx(spatial.curve.evaluate(0.5), 0.75, 1e-4));
        assert!(approx(spatial.curve.evaluate(1.0), 0.0, 1e-4));
    }
}
