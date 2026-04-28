//! `Herbcraft::SetWard` — sibling-DSE split from the retiring cat
//! `Herbcraft` inline block.
//!
//! `CompensatedProduct` of spirituality + herbcraft_skill +
//! territory_max_corruption.
//! Eligibility: `.require(markers::WardStrengthLow::KEY)` per §4 port (Phase
//! 4b.5). The outer `ctx.has_ward_herbs` conjunct in
//! `scoring.rs::score_actions` stays inline until a per-cat inventory
//! marker port lands `HasWardHerbs` on a future batch. The
//! ward-siege bonus at the same site remains inline — it's an inner
//! additive on a different marker (`WardsUnderSiege`), not on this
//! DSE's eligibility. Maslow tier 2.
//!
//! The `territory_max_corruption` axis uses the §2.3 Logistic(8, 0.1)
//! shape — threshold-gated surge that rises steeply past 0.1
//! corruption. Absorbs the retiring
//! `ward_corruption_emergency_bonus` modifier contribution: the old
//! flat additive bonus-when-corruption-detected is now produced by
//! the axis curve itself as a natural threshold response.

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

pub const SPIRITUALITY_INPUT: &str = "spirituality";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";

/// §L2.10.7 HerbcraftWard range — Manhattan tiles for the
/// nearest-perimeter-tile anchor. 25 ≈ a colony-perimeter walk;
/// wards placed along the territory boundary.
pub const HERBCRAFT_WARD_PERIMETER_RANGE: f32 = 25.0;

pub struct HerbcraftWardDse {
    id: DseId,
    considerations: Vec<Consideration>,
    composition: Composition,
    eligibility: EligibilityFilter,
}

impl HerbcraftWardDse {
    pub fn new() -> Self {
        let linear = Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        };
        // §L2.10.7 row Herbcraft (Ward): Composite{Logistic(8, 0.5),
        // Invert} over distance to nearest territory perimeter tile.
        // Spec line 5635: 'Herb commute; emergency-corruption boost
        // handled by scalar, not spatial.' Replaces the retired
        // territory_max_corruption Logistic(8, 0.1) scalar — the
        // anchor IS the placement target (perimeter), not a corruption
        // signal. WardStrengthLow marker still gates eligibility.
        let perimeter_distance = Curve::Composite {
            inner: Box::new(Curve::Logistic {
                steepness: 8.0,
                midpoint: 0.5,
            }),
            post: PostOp::Invert,
        };
        Self {
            id: DseId("herbcraft_ward"),
            considerations: vec![
                Consideration::Scalar(ScalarConsideration::new(SPIRITUALITY_INPUT, linear.clone())),
                Consideration::Scalar(ScalarConsideration::new(HERBCRAFT_SKILL_INPUT, linear)),
                Consideration::Spatial(SpatialConsideration::new(
                    "herbcraft_ward_perimeter_distance",
                    LandmarkSource::Anchor(LandmarkAnchor::NearestPerimeterTile),
                    HERBCRAFT_WARD_PERIMETER_RANGE,
                    perimeter_distance,
                )),
            ],
            composition: Composition::compensated_product(vec![1.0, 1.0, 1.0]),
            // §4 batch 2: `.require(CanWard)` gates on Adult ∧ ¬Injured
            // ∧ HasWardHerbs. Retires the `ctx.has_ward_herbs` inline
            // gate at `scoring.rs:874`.
            // §4 Phase 4b.5: `.require(WardStrengthLow)` — colony gate.
            // §13.1: `.forbid(Incapacitated)` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::CanWard::KEY)
                .require(markers::WardStrengthLow::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for HerbcraftWardDse {
    fn default() -> Self {
        Self::new()
    }
}

impl Dse for HerbcraftWardDse {
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
                label: "ward_placed",
                achieved: |_, _| false,
            },
            strategy: CommitmentStrategy::SingleMinded,
        }
    }
    fn maslow_tier(&self) -> u8 {
        2
    }
}

pub fn herbcraft_ward_dse() -> Box<dyn Dse> {
    Box::new(HerbcraftWardDse::new())
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

    #[test]
    fn herbcraft_ward_id_stable() {
        assert_eq!(HerbcraftWardDse::new().id().0, "herbcraft_ward");
    }

    #[test]
    fn herbcraft_ward_has_three_axes() {
        // §L2.10.7: spirituality + herbcraft_skill + perimeter_distance.
        let dse = HerbcraftWardDse::new();
        assert_eq!(dse.considerations().len(), 3);
    }

    #[test]
    fn herbcraft_ward_uses_perimeter_anchor() {
        let dse = HerbcraftWardDse::new();
        let spatial = dse
            .considerations()
            .iter()
            .find_map(|c| match c {
                Consideration::Spatial(s) if s.name == "herbcraft_ward_perimeter_distance" => {
                    Some(s)
                }
                _ => None,
            })
            .expect("herbcraft_ward_perimeter_distance axis must exist");
        assert!(matches!(
            spatial.landmark,
            LandmarkSource::Anchor(LandmarkAnchor::NearestPerimeterTile)
        ));
        // Composite{Logistic(8, 0.5), Invert}: at cost 0 ≈ 0.98,
        // midpoint 0.5 ≈ 0.5, edge 1.0 ≈ 0.02.
        assert!(approx(spatial.curve.evaluate(0.0), 0.982, 1e-2));
        assert!(approx(spatial.curve.evaluate(0.5), 0.5, 1e-2));
        assert!(approx(spatial.curve.evaluate(1.0), 0.018, 1e-2));
    }

    #[test]
    fn herbcraft_ward_requires_can_ward_and_ward_strength_low() {
        // §4 batch 2: CanWard (Adult ∧ ¬Injured ∧ HasWardHerbs) + WardStrengthLow.
        let dse = HerbcraftWardDse::new();
        assert_eq!(
            dse.eligibility().required,
            vec![markers::CanWard::KEY, markers::WardStrengthLow::KEY]
        );
        // §13.1: every non-Eat/Sleep/Idle cat DSE forbids Incapacitated.
        assert_eq!(
            dse.eligibility().forbidden,
            vec![markers::Incapacitated::KEY]
        );
    }

    #[test]
    fn herbcraft_ward_rejected_without_ward_strength_low_marker() {
        // Marker absent → evaluator short-circuits to `None`, per §4's
        // "avoid computing a score that can't win" principle.
        let dse = HerbcraftWardDse::new();
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
        let fetch = |_: &str, _: Entity| 0.8_f32;
        assert!(evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).is_none());
    }
}
