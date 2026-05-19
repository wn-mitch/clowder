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
use crate::resources::sim_constants::ScoringConstants;

pub const SPIRITUALITY_INPUT: &str = "spirituality";
pub const HERBCRAFT_SKILL_INPUT: &str = "herbcraft_skill";
/// 301 — substrate-dormant scalar input. Sampled from `WardIntentMap`
/// at the cat's current position by `ctx_scalars` and exposed here
/// only when `ScoringConstants::ward_intent_dse_weight > 0.0`. When
/// the weight is `0.0` (default) the DSE is constructed with the
/// original 3-axis CompensatedProduct — byte-identical pre-301.
pub const WARD_INTENT_AT_POSITION_INPUT: &str = "ward_intent_at_position";

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
    /// 301: construction takes `&ScoringConstants` so the ward-intent
    /// consideration can be conditionally appended. At default
    /// `ward_intent_dse_weight == 0.0` the DSE is the original 3-axis
    /// composition (spirituality + herbcraft_skill + perimeter
    /// distance) — byte-identical pre-301. When the weight is lifted
    /// above 0 a 4th `Consideration::Scalar` reading the
    /// `WardIntentMap`-sourced scalar is appended, biasing the DSE
    /// score upward for cats standing on coordinator-stamped intent
    /// tiles. The conditional-axis pattern is required because
    /// `CompensatedProduct`'s geometric-mean compensation depends on
    /// `n` (axis count): adding a no-op 4th axis would still shift
    /// the compensation exponent `1/n` and perturb scores.
    pub fn new(scoring: &ScoringConstants) -> Self {
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
            inner: Box::new(Curve::Composite {
                inner: Box::new(Curve::Logistic {
                    steepness: 8.0,
                    midpoint: 0.5,
                }),
                post: PostOp::Invert,
            }),
            post: PostOp::ClampMin(0.1),
        };
        let mut considerations = vec![
            Consideration::Scalar(ScalarConsideration::new(SPIRITUALITY_INPUT, linear.clone())),
            Consideration::Scalar(ScalarConsideration::new(
                HERBCRAFT_SKILL_INPUT,
                linear.clone(),
            )),
            Consideration::Spatial(SpatialConsideration::new(
                "herbcraft_ward_perimeter_distance",
                LandmarkSource::Anchor(LandmarkAnchor::NearestPerimeterTile),
                HERBCRAFT_WARD_PERIMETER_RANGE,
                perimeter_distance,
            )),
        ];
        let mut weights = vec![1.0, 1.0, 1.0];

        // 301: conditional 4th axis. Active only when the weight is
        // lifted off 0.0. Curve `slope=w, intercept=1-w` maps the
        // `[0, 1]` intent scalar to `[1-w, 1]` — on-intent cats
        // retain a full multiplier (1.0) while off-intent cats
        // receive a `1-w` suppression. The conditional-add preserves
        // byte-identity at default by leaving the 3-axis composition
        // shape unchanged when dormant.
        let w = scoring.ward_intent_dse_weight;
        if w > 0.0 {
            considerations.push(Consideration::Scalar(ScalarConsideration::new(
                WARD_INTENT_AT_POSITION_INPUT,
                Curve::Linear {
                    slope: w,
                    intercept: 1.0 - w,
                },
            )));
            weights.push(1.0);
        }

        Self {
            id: DseId("herbcraft_ward"),
            considerations,
            composition: Composition::compensated_product(weights),
            // §4 batch 2: original `.require(CanWard)` gated on Adult ∧
            // ¬Injured ∧ HasWardHerbs.
            // 084 Commit 2: swapped to `CanWardFromSupply`, which fires
            // when the cat either carries thornbriar OR the colony has
            // ≥1 stashed (HasStoredThornbriar). The GOAP plan branches
            // into carry-direct or retrieve-first chains naturally via
            // `CarryingIs(Herbs)` precondition matching at plan time.
            // §4 Phase 4b.5: `.require(WardStrengthLow)` — colony gate.
            // §13.1: `.forbid(Incapacitated)` blocks downed cats.
            eligibility: EligibilityFilter::new()
                .require(markers::CanWardFromSupply::KEY)
                .require(markers::WardStrengthLow::KEY)
                .forbid(markers::Incapacitated::KEY),
        }
    }
}

impl Default for HerbcraftWardDse {
    /// Default uses `ScoringConstants::default()` — 4th-axis dormant
    /// (`ward_intent_dse_weight == 0.0`). Test-harness convenience;
    /// production goes through `herbcraft_ward_dse(scoring)`.
    fn default() -> Self {
        Self::new(&ScoringConstants::default())
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

pub fn herbcraft_ward_dse(scoring: &ScoringConstants) -> Box<dyn Dse> {
    Box::new(HerbcraftWardDse::new(scoring))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::considerations::LandmarkAnchor;
    use crate::ai::eval::{evaluate_single, ModifierPipeline};
    use crate::components::physical::Position;

    fn approx(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn herbcraft_ward_id_stable() {
        assert_eq!(HerbcraftWardDse::default().id().0, "herbcraft_ward");
    }

    #[test]
    fn herbcraft_ward_has_three_axes_at_dormant_default() {
        // §L2.10.7: spirituality + herbcraft_skill + perimeter_distance.
        // 301 byte-identity invariant: at the default
        // `ward_intent_dse_weight = 0.0` the 4th axis is *not*
        // appended, preserving the 3-axis CompensatedProduct shape
        // (and its `1/3` compensation exponent) pre-301.
        let dse = HerbcraftWardDse::default();
        assert_eq!(dse.considerations().len(), 3);
    }

    #[test]
    fn herbcraft_ward_uses_perimeter_anchor() {
        let dse = HerbcraftWardDse::default();
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
        // Composite{Composite{Logistic(8, 0.5), Invert}, ClampMin(0.1)}:
        // at cost 0 ≈ 0.98, midpoint 0.5 ≈ 0.5, edge 1.0 floored at 0.1.
        assert!(approx(spatial.curve.evaluate(0.0), 0.982, 1e-2));
        assert!(approx(spatial.curve.evaluate(0.5), 0.5, 1e-2));
        assert!(approx(spatial.curve.evaluate(1.0), 0.1, 1e-2));
    }

    #[test]
    fn herbcraft_ward_requires_can_ward_from_supply_and_ward_strength_low() {
        // 084 Commit 2: CanWardFromSupply (Adult ∧ ¬Injured ∧
        // (HasWardHerbs ∨ HasStoredThornbriar)) + WardStrengthLow.
        let dse = HerbcraftWardDse::default();
        assert_eq!(
            dse.eligibility().required,
            vec![
                markers::CanWardFromSupply::KEY,
                markers::WardStrengthLow::KEY
            ]
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
        let dse = HerbcraftWardDse::default();
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
            target_alive: None,
            field_cost: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.8_f32;
        assert!(evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).is_none());
    }

    /// 301 dormancy invariant: at `ward_intent_dse_weight == 0.0`
    /// the DSE has exactly 3 considerations (no `ward_intent_at_position`
    /// axis), so its composition shape and score arithmetic match
    /// pre-301 byte-for-byte. This is the structural guarantee that
    /// the default-flag soak's `WardPlaced` event stream remains
    /// byte-identical.
    #[test]
    fn ward_intent_axis_absent_at_dormant_weight() {
        let mut scoring = ScoringConstants::default();
        scoring.ward_intent_dse_weight = 0.0;
        let dse = HerbcraftWardDse::new(&scoring);
        assert_eq!(dse.considerations().len(), 3);
        let has_intent_axis = dse.considerations().iter().any(|c| match c {
            Consideration::Scalar(s) => s.name == WARD_INTENT_AT_POSITION_INPUT,
            _ => false,
        });
        assert!(
            !has_intent_axis,
            "intent axis must be absent at dormant weight"
        );
    }

    /// 301 activation: with `ward_intent_dse_weight > 0.0` the DSE
    /// appends a 4th `ward_intent_at_position` scalar axis. Score
    /// lifts when the cat stands on an intent tile (scalar = 1.0)
    /// vs the same cat off-intent (scalar = 0.0).
    #[test]
    fn ward_intent_axis_lifts_score_on_intent_tile() {
        let mut scoring = ScoringConstants::default();
        scoring.ward_intent_dse_weight = 0.5;
        let dse = HerbcraftWardDse::new(&scoring);
        assert_eq!(
            dse.considerations().len(),
            4,
            "4th axis must be present when weight is lifted"
        );

        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |key: &str, _: Entity| {
            // Grant the marker set required by the eligibility filter
            // so `evaluate_single` doesn't short-circuit to None.
            key == markers::CanWardFromSupply::KEY || key == markers::WardStrengthLow::KEY
        };
        let entity_position = |_: Entity| Some(Position::new(0, 0));
        let anchor_position = |_: LandmarkAnchor| Some(Position::new(0, 0));
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();

        // On-intent cat: ward_intent_at_position fetches 1.0.
        let fetch_on_intent = |name: &str, _: Entity| -> f32 {
            if name == WARD_INTENT_AT_POSITION_INPUT {
                1.0
            } else {
                0.8
            }
        };
        let on_intent = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch_on_intent)
            .expect("eligible cat scores")
            .final_score;

        // Off-intent cat: ward_intent_at_position fetches 0.0.
        let fetch_off_intent = |name: &str, _: Entity| -> f32 {
            if name == WARD_INTENT_AT_POSITION_INPUT {
                0.0
            } else {
                0.8
            }
        };
        let off_intent =
            evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch_off_intent)
                .expect("eligible cat scores")
                .final_score;

        assert!(
            on_intent > off_intent,
            "on-intent score {on_intent} must exceed off-intent score \
             {off_intent} when ward_intent_dse_weight = 0.5"
        );
    }
}
