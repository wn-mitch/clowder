//! `ApplyRemedyTargetDse` — §6.5.7 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning patient selection for `ApplyRemedy`. The
//! action itself is surfaced through Herbcraft's `PrepareRemedy`
//! chain — this DSE picks *whom the poultice heals*, replacing the
//! legacy `injured_cats.iter().min_by_key(distance)` pick at
//! `disposition.rs::try_crafting_sub_mode::PrepareRemedy`.
//!
//! Phase 4c.9 scope — severity-aware triage. The legacy picker
//! chose nearest patient regardless of injury severity, so a cat at
//! health=0.95 next door would be preferred over a cat at health=0.3
//! across the colony. §6.1 Partial fix: the DSE scores distance,
//! injury severity, and kinship together; severity dominates via its
//! Quadratic amplification.
//!
//! Three per-target considerations per §6.5.7. The `remedy-match`
//! axis (Cliff gating HealingPoultice vs. mood-injury-remedy) is
//! deferred — remedies today are effectively single-class (the
//! HealingPoultice/EnergyTonic/MoodTonic switch in
//! `build_crafting_chain::PrepareRemedy` is remedy-kind selection
//! *at prepare time*, not a per-candidate match). Weights
//! renormalized from the spec's (0.15/0.40/0.30/0.15) by dropping
//! the 0.30 remedy-match row and dividing by 0.70:
//!
//! | # | Consideration      | Source                | Curve                                 | Spec weight | Renormalized |
//! |---|--------------------|-----------------------|---------------------------------------|-------------|--------------|
//! | 1 | distance           | `Spatial(target)`     | `Quadratic(exp=1.5, div=-1, shift=1)` | 0.15        | 0.214        |
//! | 2 | injury-severity    | `target_injury`       | `Quadratic(exp=2)`                    | 0.40        | 0.571        |
//! | 3 | kinship            | `target_kinship`      | `Linear(0.5, 0.5)`                    | 0.15        | 0.214        |
//!
//! **Distance curve.** Spec §6.4 row #7 specifies `Quadratic(exp=1.5),
//! range=15`. The 1.5 exponent sits between linear falloff and the
//! stronger Quadratic(2) used for adjacency-sensitive DSEs —
//! healers cross the colony, but a patient at range 15 still
//! deserves less attention than one at range 3. The distance axis
//! lands as a `SpatialConsideration` per the §L2.10.7 plan-cost
//! feedback design (ticket 052) — Manhattan distance to the patient
//! flows through `Quadratic(exp=1.5, divisor=-1, shift=1)` over
//! normalized cost, which evaluates `(1 - cost)^1.5` and exactly
//! preserves the legacy scalar `Quadratic(exp=1.5)` over `1 - dist/
//! range` shape.
//!
//! **Injury severity.** `1 − health.current / health.max` — the
//! standard deficit axis. Convex Quadratic amplifies desperate
//! need; a patient at health=0.3 (deficit=0.7) contributes ~0.49,
//! while a patient at health=0.95 (deficit=0.05) contributes
//! ~0.003. This is the axis the legacy picker could not see.
//!
//! **Kinship.** Linear(0.5, 0.5) per spec — non-kin scores 0.5
//! (signal=0), kin scores 1.0 (signal=1). Mild bias; the weight
//! (0.214) is intentionally small so colony-wide healing remains
//! the norm.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::Curve;
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::resources::action_affordances::{ActionAffordances, ActionKind};
use crate::resources::sim_constants::ScoringConstants;
use crate::systems::plan_substrate::perceived_injury_signal;

pub const TARGET_INJURY_INPUT: &str = "target_injury";
pub const TARGET_KINSHIP_INPUT: &str = "target_kinship";
/// 264 — actor's own `CatBeliefs[patient].perceived_injury_level`
/// (`[0, 1]`, 0.0 fail-open). ApplyRemedy is the `Care` consumer (261
/// estimator table: `perceived_injury_level + bond`); this belief axis
/// supersedes the raw-HP `target_injury` axis at activation (raw axis
/// retires second — pillar 2).
pub const TARGET_PERCEIVED_INJURY_INPUT: &str = "target_perceived_injury";
/// 264 — per-target `Affordance(Care, self, target)` read from
/// substrate 261. `target_`-prefixed for the
/// `score_target_consideration` routing reason (ticket 516).
pub const TARGET_CARE_AFFORDANCE_INPUT: &str = "target_affordance_care";

/// Candidate-pool range in Manhattan tiles. Matches spec §6.4 row #7
/// — healers cross the colony for severe injury. Outer cutoff
/// beyond which the caller doesn't bother building a candidate
/// snapshot.
pub const APPLY_REMEDY_TARGET_RANGE: f32 = 15.0;

/// Per-patient snapshot fed to `resolve_apply_remedy_target`.
#[derive(Clone, Copy, Debug)]
pub struct PatientCandidate {
    pub entity: Entity,
    pub position: Position,
    /// `health.current / health.max` — clamped to [0, 1].
    pub health_fraction: f32,
}

/// §6.5.7 `ApplyRemedy` target-taking DSE factory.
///
/// 264: takes `&ScoringConstants` so the conditional belief +
/// affordance axes (`target_perceived_injury`, `target_affordance_care`)
/// are added only when their weights are non-zero. Activated at
/// step 20 (2026-07-08): the belief axis holds the raw axis's full
/// 8/14 triage slot and the raw-HP `target_injury` axis is retired
/// from the default composition — it is only built when the belief
/// weight is zeroed (config-override escape hatch, byte-identical to
/// the pre-264 three-axis shape). Remaining base axes renormalize to
/// `(1 − Σ extras)` so the WeightedSum stays at 1.0.
pub fn apply_remedy_target_dse(scoring: &ScoringConstants) -> TargetTakingDse {
    // §L2.10.7 distance axis: `Quadratic(exp=1.5, divisor=-1, shift=1)`
    // evaluates `((cost - 1) / -1).max(0).powf(1.5) = (1 - cost)^1.5`,
    // exactly preserving the legacy `nearness^1.5` shape — same
    // explicit-inversion idiom as Mentor's port (Quadratic isn't
    // point-symmetric, so `Composite{Quadratic, Invert}` would give
    // a different shape).
    let nearness_curve = Curve::Quadratic {
        exponent: 1.5,
        divisor: -1.0,
        shift: 1.0,
    };
    let injury_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    // Kinship Linear(0.5, 0.5) — signal ∈ {0.0, 1.0}, curve output
    // ∈ {0.5, 1.0}. Non-kin still attended; kin biased mildly.
    let kinship_curve = Curve::Linear {
        slope: 0.5,
        intercept: 0.5,
    };

    let injury_belief_w = scoring.apply_remedy_injury_belief_weight.clamp(0.0, 1.0);
    let affordance_w = scoring.apply_remedy_affordance_weight.clamp(0.0, 1.0);

    let mut considerations: Vec<Consideration> =
        vec![Consideration::Spatial(SpatialConsideration::new(
            "apply_remedy_target_nearness",
            LandmarkSource::TargetPosition,
            APPLY_REMEDY_TARGET_RANGE,
            nearness_curve,
        ))];
    // Base weights are `[3, 8, 3] / 14` (distance / injury / kinship)
    // — the spec-renormalized distribution computed to f32 precision
    // so the RtEO invariant sum-to-1.0 assertion in
    // `Composition::compose` holds.
    let mut weights: Vec<f32> = vec![3.0 / 14.0];
    // 264 step-20 activation: the belief axis SUPERSEDES the raw-HP
    // `target_injury` read (pillar 2 — substrate first, hack second).
    // When `injury_belief_w > 0.0` the raw axis is not built at all;
    // the belief axis takes the triage slot at the configured weight
    // (shipped default: the raw axis's full 8/14). Zeroing the weight
    // is the config-override escape hatch that restores the legacy
    // three-axis god-eye composition byte-identically.
    if injury_belief_w <= 0.0 {
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            TARGET_INJURY_INPUT,
            injury_curve.clone(),
        )));
        weights.push(8.0 / 14.0);
    }
    considerations.push(Consideration::Scalar(ScalarConsideration::new(
        TARGET_KINSHIP_INPUT,
        kinship_curve,
    )));
    weights.push(3.0 / 14.0);
    // Renormalize whatever base remains to `1 − Σ extras` so the
    // WeightedSum stays at 1.0 across both shapes.
    let extra_w = (injury_belief_w + affordance_w).clamp(0.0, 1.0);
    if extra_w > 0.0 {
        let base_sum: f32 = weights.iter().sum();
        let scale = (1.0 - extra_w) / base_sum.max(f32::EPSILON);
        for w in &mut weights {
            *w *= scale;
        }
    }
    if injury_belief_w > 0.0 {
        // 264: perceived injury through the same convex Quadratic(2)
        // as the raw axis — the same "severity amplifies triage"
        // shape, sourced from the actor's belief instead of the
        // patient's HP bar. Unmodeled patients read 0.0: a healer
        // only triages injuries they have witnessed (or that gossip /
        // festering-wound cues have taught them about).
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            TARGET_PERCEIVED_INJURY_INPUT,
            injury_curve,
        )));
        weights.push(injury_belief_w);
    }
    if affordance_w > 0.0 {
        // 264: Affordance(Care, self, target) from substrate 261
        // (estimator: perceived_injury_level + bond + proximity + my
        // condition). Reads 0.0 for pairs the writer didn't populate
        // this tick — the substrate's gate signal.
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            TARGET_CARE_AFFORDANCE_INPUT,
            Curve::Linear {
                slope: 1.0,
                intercept: 0.0,
            },
        )));
        weights.push(affordance_w);
    }

    TargetTakingDse {
        id: DseId("apply_remedy_target"),
        candidate_query: apply_remedy_candidate_query_doc,
        per_target_considerations: considerations,
        composition: Composition::weighted_sum(weights),
        aggregation: TargetAggregation::Best,
        intention: apply_remedy_intention,
        required_stance: None,
        // Tickets 074 + 080 — gate dead/banished/incapacitated
        // candidates AND candidates already reserved by another
        // cat. Combined filter applied at the IAUS scoring layer.
        eligibility: crate::systems::plan_substrate::require_alive_and_unreserved_filter(),
    }
}

fn apply_remedy_candidate_query_doc(_cat: Entity) -> &'static str {
    "injured cats within APPLY_REMEDY_TARGET_RANGE (health.current < health.max)"
}

fn apply_remedy_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState::predicate("injury_healed", |_, _| false),
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best patient for `cat` via the registered
/// [`apply_remedy_target_dse`]. Returns `None` iff no eligible
/// candidate exists in range.
///
/// - `candidates` is the caller-built injured-cat snapshot.
/// - `is_kin(self, target)` — parent-child check (same shape as
///   Groom-other §6.5.4).
#[allow(clippy::too_many_arguments)]
pub fn resolve_apply_remedy_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    candidates: &[PatientCandidate],
    is_kin: &dyn Fn(Entity, Entity) -> bool,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
    // 264 — the actor's own belief-state about patients; the
    // `target_perceived_injury` axis reads the perceived_injury_level
    // facet (0.0 fail-open for unmodeled patients).
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    // 264 — ActionAffordances resource for the conditional
    // `target_affordance_care` axis; at dormant weight the axis is
    // absent and the arm is never queried.
    affordances: &ActionAffordances,
    // Ticket 427 Step 1 — pre-allocated scratch buffers.
    scratch: &mut crate::resources::DseTargetScratchpad,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "apply_remedy_target")?;

    if candidates.is_empty() {
        return None;
    }

    scratch.entities.clear();
    scratch.positions.clear();
    scratch.map_f32_a.clear();
    for c in candidates {
        let dist = cat_pos.distance_to(&c.position);
        if dist > APPLY_REMEDY_TARGET_RANGE {
            continue;
        }
        scratch.entities.push(c.entity);
        scratch.positions.push(c.position);
        scratch
            .map_f32_a
            .insert(c.entity, (1.0 - c.health_fraction).clamp(0.0, 1.0));
    }

    if scratch.entities.is_empty() {
        return None;
    }

    // Spatial nearness axis (`apply_remedy_target_nearness`) is
    // computed by the substrate from `EvalCtx::self_position` to each
    // candidate's tile per §L2.10.7.
    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    // Reborrow as `&` so the closure captures a shared reference;
    // disjoint from `&scratch.entities` / `&scratch.positions` below.
    let injury_map = &scratch.map_f32_a;
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_INJURY_INPUT => injury_map.get(&target).copied().unwrap_or(0.0),
            TARGET_KINSHIP_INPUT if is_kin(cat, target) => 1.0,
            // 264 — actor-subjective injury belief (0.0 fail-open).
            TARGET_PERCEIVED_INJURY_INPUT => perceived_injury_signal(cat_beliefs, target),
            // 264 — Affordance(Care) substrate read.
            TARGET_CARE_AFFORDANCE_INPUT => affordances.read(cat, target, ActionKind::Care),
            _ => 0.0,
        }
    };

    let entity_position = |_: Entity| -> Option<Position> { None };

    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat,
        tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
        has_marker: &has_marker,
        self_position: cat_pos,
        target: None,
        target_position: None,
        target_alive: None,
        field_cost: None,
    };

    let scored = evaluate_target_taking(
        dse,
        cat,
        &scratch.entities,
        &scratch.positions,
        &ctx,
        &fetch_self,
        &fetch_target,
    );

    // §11 focal-cat per-candidate ranking capture (§6.3). Emitted only
    // when the caller marks this resolve as the focal cat's tick.
    // Non-focal paths pass `focal_hook: None` and pay zero cost.
    if let Some(hook) = focal_hook {
        if let Some(ranking) = crate::ai::target_dse::target_ranking_from_scored(
            &scored,
            dse.aggregation(),
            hook.name_lookup,
        ) {
            hook.capture
                .set_target_ranking("apply_remedy_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patient(entity_id: u32, x: i32, y: i32, health_fraction: f32) -> PatientCandidate {
        PatientCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            health_fraction,
        }
    }

    /// Pre-264 constants: the two step-20-activated axes zeroed so
    /// raw-HP-axis behavioral tests and exact-shape assertions keep
    /// pinning the legacy three-axis god-eye composition (the
    /// config-override escape hatch).
    fn pre_264_scoring() -> ScoringConstants {
        let mut s = ScoringConstants::default();
        s.apply_remedy_injury_belief_weight = 0.0;
        s.apply_remedy_affordance_weight = 0.0;
        s
    }

    #[test]
    fn apply_remedy_target_dse_id_stable() {
        assert_eq!(
            apply_remedy_target_dse(&ScoringConstants::default()).id().0,
            "apply_remedy_target"
        );
    }

    #[test]
    fn apply_remedy_target_has_three_axes() {
        // Legacy shape — pinned via the zeroed-weights escape hatch;
        // the active default is the four-axis belief-triage shape
        // (see `active_default_is_belief_triage_shape`).
        assert_eq!(
            apply_remedy_target_dse(&pre_264_scoring())
                .per_target_considerations()
                .len(),
            3
        );
    }

    #[test]
    fn apply_remedy_target_weights_sum_to_one() {
        let sum: f32 = apply_remedy_target_dse(&ScoringConstants::default())
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn apply_remedy_target_uses_best_aggregation() {
        assert_eq!(
            apply_remedy_target_dse(&ScoringConstants::default()).aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn intention_is_injury_healed_goal() {
        let dse = apply_remedy_target_dse(&ScoringConstants::default());
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label(), "injury_healed");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    #[test]
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_filters_out_of_range() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = patient(2, 50, 0, 0.3);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[far],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn picks_more_injured_at_equal_distance() {
        // §6.1 Partial fix: severe patient (health=0.3, deficit=0.7)
        // wins over light-injury patient (health=0.95, deficit=0.05)
        // at equal distance. Weight ratio + Quadratic amplification
        // decides decisively.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&pre_264_scoring()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let severe = patient(2, 3, 0, 0.3);
        let mild = patient(3, 0, 3, 0.95);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[severe, mild],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(severe.entity));
    }

    #[test]
    fn severity_dominates_distance() {
        // A cat at health=0.2 (deficit=0.8, injury curve output ≈
        // 0.64) across the colony at dist=10 still beats a cat at
        // health=0.9 (deficit=0.1, injury curve ≈ 0.01) nearby at
        // dist=1, because severity's weight (0.571) × 0.64 ≈ 0.366
        // dominates nearness's weight (0.214) × (nearness
        // contribution at dist=10 range=15 ≈ 0.33²·⁵ ≈ 0.06) ≈ 0.013.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&pre_264_scoring()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let critical_far = patient(2, 10, 0, 0.2);
        let mild_near = patient(3, 1, 0, 0.9);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[critical_far, mild_near],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(critical_far.entity));
    }

    #[test]
    fn close_patient_outscores_distant_same_injury() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = patient(2, 2, 0, 0.5);
        let far = patient(3, 10, 0, 0.5);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[close, far],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn kin_beats_non_kin_when_other_axes_tied() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let kin = patient(2, 3, 0, 0.5);
        let stranger = patient(3, 0, 3, 0.5);
        let kin_e = kin.entity;
        let is_kin = move |_: Entity, target: Entity| -> bool { target == kin_e };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[kin, stranger],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(kin.entity));
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let patient = patient(2, 1, 0, 0.5);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[patient],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    // -----------------------------------------------------------------
    // 264 — conditional belief + affordance axes (dormant wire)
    // -----------------------------------------------------------------

    #[test]
    fn belief_affordance_axes_absent_when_zeroed() {
        // 264 conditional-axis contract: zeroing the belief weight
        // restores the legacy three-axis composition (raw target_injury
        // back in its 8/14 slot) byte-identically — the config-override
        // escape hatch and the shape the dormant-wire gate proved.
        let s = pre_264_scoring();
        let dse = apply_remedy_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 3);
        assert!(dse.per_target_considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc)
                if sc.name == TARGET_PERCEIVED_INJURY_INPUT
                    || sc.name == TARGET_CARE_AFFORDANCE_INPUT
        )));
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn belief_affordance_axes_present_and_raw_axis_retired_when_active() {
        // Step-20 supersession: any non-zero belief weight replaces the
        // raw target_injury axis entirely — four axes (nearness,
        // kinship, belief, affordance), base pair renormalized to
        // (1 − Σ extras).
        let mut s = ScoringConstants::default();
        s.apply_remedy_injury_belief_weight = 0.2;
        s.apply_remedy_affordance_weight = 0.1;
        let dse = apply_remedy_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 4);
        assert!(dse.per_target_considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == TARGET_INJURY_INPUT
        )));
        let weights = &dse.composition().weights;
        assert!((weights[2] - 0.2).abs() < 1e-6);
        assert!((weights[3] - 0.1).abs() < 1e-6);
        // Base pair: (3/14) × (0.7 / (6/14)) = 0.35 each.
        assert!((weights[0] - 0.35).abs() < 1e-5);
        assert!((weights[1] - 0.35).abs() < 1e-5);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "renormalized sum = {sum}");
    }

    #[test]
    fn active_default_is_belief_triage_shape() {
        // Shipped defaults (2026-07-08): belief axis at the raw axis\'s
        // full 8/14 triage slot, affordance at 0.10, raw axis retired.
        let s = ScoringConstants::default();
        assert!((s.apply_remedy_injury_belief_weight - 8.0 / 14.0).abs() < 1e-6);
        assert!((s.apply_remedy_affordance_weight - 0.10).abs() < 1e-6);
        let dse = apply_remedy_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 4);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn raw_hp_invisible_at_active_default() {
        // The supersession\'s behavioral contract: with no injury
        // beliefs about either patient, raw HP difference no longer
        // moves the pick. Tied positions; ties break toward the LATER
        // candidate — pre-supersession the severe patient won on the
        // raw axis, post-supersession the tie stands and the later
        // candidate wins. (Cats must WITNESS injury to triage it.)
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let severe = patient(2, 1, 0, 0.3);
        let mild_later = patient(3, 0, 1, 0.95);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[severe, mild_later],
            &is_kin,
            0,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(
            out,
            Some(mild_later.entity),
            "raw HP must be invisible without a belief — tie breaks to the later candidate"
        );
    }

    /// 264 — ticket microexperiment `care_targets_perceived_injury` at
    /// the resolver layer: with the belief axis active, two patients
    /// with IDENTICAL raw HP split on the actor's perceived injury —
    /// the Care DSE reads the belief, not (only) the HP bar. Tied
    /// positions; ties break toward the LATER candidate, so a dead
    /// fetch arm fails this test.
    #[test]
    fn care_targets_perceived_injury_when_axis_active() {
        let mut s = ScoringConstants::default();
        s.apply_remedy_injury_belief_weight = 0.3;
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&s));
        let cat = Entity::from_raw_u32(1).unwrap();
        // Same raw health fraction — the raw target_injury axis ties.
        let believed_hurt = patient(2, 1, 0, 0.6);
        let believed_fine = patient(3, 0, 1, 0.6);

        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let m = beliefs.models.entry(believed_hurt.entity).or_default();
        m.perceived_injury_level = crate::components::beliefs::Facet {
            value: 0.9,
            strength: 1.0,
            ..Default::default()
        };

        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[believed_hurt, believed_fine],
            &is_kin,
            0,
            None,
            Some(&beliefs),
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(believed_hurt.entity));
    }

    /// 264 — affordance arm verified live: the substrate-priced
    /// patient beats the unpriced one at tied positions and HP.
    #[test]
    fn apply_remedy_reads_affordance_when_axis_active() {
        let mut s = ScoringConstants::default();
        s.apply_remedy_affordance_weight = 0.2;
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(apply_remedy_target_dse(&s));
        let cat = Entity::from_raw_u32(1).unwrap();
        let afforded = patient(2, 1, 0, 0.6);
        let unpriced = patient(3, 0, 1, 0.6);
        let mut affordances = ActionAffordances::default();
        affordances.write(cat, afforded.entity, ActionKind::Care, 0.9);
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_apply_remedy_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[afforded, unpriced],
            &is_kin,
            0,
            None,
            None,
            &affordances,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(afforded.entity));
    }
}
