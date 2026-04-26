//! `PairingActivityTargetDse` — §7.M.1 L2 target-taking DSE for the
//! courtship arc.
//!
//! Pairs with [`PairingActivityDse`](super::pairing_activity::PairingActivityDse)
//! (this DSE owns *with whom*; the self-state owns *whether*).
//!
//! Sibling to `mate_target_dse` (L3): same three considerations, same
//! `Best` aggregation, same `WeightedSum` composition. The differences
//! are:
//!
//! - **Bond filter is `Friends`** (not `Partners`/`Mates`). L2's job is
//!   to escalate Friends → Partners by holding compatible adults
//!   colocated; L3 takes over once the Partners bond exists.
//! - **Wider weighting on fondness vs. romantic.** L3 picks the partner
//!   with the strongest existing romantic signal (already past
//!   threshold); L2 picks the most-fond Friends candidate so courtship
//!   directs at the partner with the most relationship momentum.
//! - **Orientation compatibility is in the candidate filter, not a
//!   consideration axis.** Both DSEs early-reject incompatible pairs;
//!   neither carries it as a curve since it's a binary gate.
//!
//! ### Four axes via the candidate filter ↦ three considerations
//!
//! The plan called for `target_compat` as a Cliff cliff axis but the
//! candidate filter (`Friends` bond + `are_orientation_compatible`)
//! already rejects incompatibles, so a `target_compat` axis would be
//! a constant 1.0 over the entire candidate pool — pure overhead.
//! Three considerations match the actual signal landscape.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::identity::{Gender, Orientation};
use crate::components::physical::Position;
use crate::resources::relationships::{BondType, Relationships};
use crate::systems::social::are_orientation_compatible;

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
pub const TARGET_ROMANTIC_INPUT: &str = "target_romantic";

/// Candidate-pool range for L2 partner selection. Matches
/// `MATE_TARGET_RANGE` and `SOCIALIZE_TARGET_RANGE` (10 tiles) — the
/// same broadcast range disposition.rs and goap.rs already use for
/// social outer-gating; per-target distance attenuation happens via
/// the `target_nearness` Quadratic.
pub const PAIRING_TARGET_RANGE: f32 = 10.0;

/// §7.M.1 L2 target-taking DSE factory.
pub fn pairing_activity_target_dse() -> TargetTakingDse {
    let nearness_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };

    TargetTakingDse {
        id: DseId("pairing_activity_target"),
        candidate_query: pairing_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_NEARNESS_INPUT,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_FONDNESS_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(TARGET_ROMANTIC_INPUT, linear)),
        ],
        // Weights: fondness and romantic each ~0.4; nearness ~0.2.
        // Distance only nudges — a beloved Friends partner across the
        // map should still beat a near-stranger. Romantic and fondness
        // both load similarly because L2 is the bridge that builds
        // *both* axes through colocated time.
        composition: Composition::weighted_sum(vec![0.20, 0.40, 0.40]),
        aggregation: TargetAggregation::Best,
        intention: pairing_intention,
    }
}

fn pairing_candidate_query_doc(_cat: Entity) -> &'static str {
    "cats within PAIRING_TARGET_RANGE with bond == Friends, orientation-compatible, excluding self"
}

fn pairing_intention(_target: Entity) -> Intention {
    Intention::Activity {
        kind: ActivityKind::Pairing,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::OpenMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best courtship partner for `cat` via the registered
/// [`pairing_activity_target_dse`]. Returns `None` iff no eligible
/// candidate exists (no Friends-bonded compat partner in range).
///
/// Bond filter: only cats whose `Relationships::get(cat, other).bond`
/// is `Some(Friends)` qualify. Partners/Mates route through
/// `mate_target` instead; the two DSEs partition the bond-tier space
/// cleanly.
///
/// Orientation gate: the candidate's gender + orientation must pass
/// `are_orientation_compatible(self, other)`. The caller threads
/// per-cat `(Gender, Orientation)` snapshots so this resolver doesn't
/// re-query components.
#[allow(clippy::too_many_arguments)]
pub fn resolve_pairing_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_gender: Gender,
    cat_orientation: Orientation,
    cat_positions: &[(Entity, Position)],
    orientations: &std::collections::HashMap<Entity, (Gender, Orientation)>,
    relationships: &Relationships,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "pairing_activity_target")?;

    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist > PAIRING_TARGET_RANGE {
            continue;
        }
        let bond = match relationships.get(cat, *other).and_then(|r| r.bond) {
            Some(b) => b,
            None => continue,
        };
        if !matches!(bond, BondType::Friends) {
            continue;
        }
        let (other_gender, other_orient) = match orientations.get(other) {
            Some(g) => *g,
            None => continue,
        };
        if !are_orientation_compatible(cat_gender, cat_orientation, other_gender, other_orient) {
            continue;
        }
        candidates.push(*other);
        positions.push(*other_pos);
    }

    if candidates.is_empty() {
        return None;
    }

    let pos_map: std::collections::HashMap<Entity, Position> = candidates
        .iter()
        .copied()
        .zip(positions.iter().copied())
        .collect();

    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_NEARNESS_INPUT => {
                let target_pos = match pos_map.get(&target) {
                    Some(p) => *p,
                    None => return 0.0,
                };
                let dist = cat_pos.manhattan_distance(&target_pos) as f32;
                (1.0 - dist / PAIRING_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_ROMANTIC_INPUT => relationships
                .get(cat, target)
                .map(|r| r.romantic)
                .unwrap_or(0.0),
            _ => 0.0,
        }
    };

    let sample_map = |_: &str, _: Position| -> f32 { 0.0 };
    let has_marker = |_: &str, _: Entity| -> bool { false };

    let ctx = EvalCtx {
        cat,
        tick,
        sample_map: &sample_map,
        has_marker: &has_marker,
        self_position: cat_pos,
        target: None,
        target_position: None,
    };

    let scored = evaluate_target_taking(
        dse,
        cat,
        &candidates,
        &positions,
        &ctx,
        &fetch_self,
        &fetch_target,
    );

    if let Some(hook) = focal_hook {
        if let Some(ranking) = crate::ai::target_dse::target_ranking_from_scored(
            &scored,
            dse.aggregation(),
            hook.name_lookup,
        ) {
            hook.capture
                .set_target_ranking("pairing_activity_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::relationships::BondType;
    use std::collections::HashMap;

    fn straight_queens() -> (Gender, Orientation) {
        (Gender::Queen, Orientation::Straight)
    }

    fn straight_tom() -> (Gender, Orientation) {
        (Gender::Tom, Orientation::Straight)
    }

    #[test]
    fn pairing_target_dse_id_stable() {
        assert_eq!(pairing_activity_target_dse().id().0, "pairing_activity_target");
    }

    #[test]
    fn pairing_target_has_three_axes() {
        assert_eq!(pairing_activity_target_dse().per_target_considerations().len(), 3);
    }

    #[test]
    fn pairing_target_weights_sum_to_one() {
        let sum: f32 = pairing_activity_target_dse()
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn pairing_target_uses_best_aggregation() {
        assert_eq!(
            pairing_activity_target_dse().aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn intention_is_pairing_activity_open_minded() {
        let cat = Entity::from_raw_u32(1).unwrap();
        let intention = pairing_intention(cat);
        match intention {
            Intention::Activity {
                kind,
                termination: _,
                strategy,
            } => {
                assert_eq!(kind, ActivityKind::Pairing);
                assert!(matches!(strategy, CommitmentStrategy::OpenMinded));
            }
            other => panic!("expected Activity intention, got {other:?}"),
        }
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let orientations = HashMap::new();
        let out = resolve_pairing_target(
            &registry,
            cat,
            Position::new(0, 0),
            Gender::Queen,
            Orientation::Straight,
            &[],
            &orientations,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_partners_bonded_candidates() {
        // Partners-tier bonds route to mate_target, not pairing_target.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(pairing_activity_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, partner).bond = Some(BondType::Partners);
        let mut orientations = HashMap::new();
        orientations.insert(partner, straight_tom());
        let cat_positions = vec![(partner, Position::new(2, 0))];
        let out = resolve_pairing_target(
            &registry,
            cat,
            Position::new(0, 0),
            Gender::Queen,
            Orientation::Straight,
            &cat_positions,
            &orientations,
            &relationships,
            0,
            None,
        );
        assert!(
            out.is_none(),
            "Partners-bonded cat must not appear in the L2 pairing candidate pool"
        );
    }

    #[test]
    fn resolver_excludes_orientation_incompatible_friends() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(pairing_activity_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let other_tom = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, other_tom).bond = Some(BondType::Friends);
        let mut orientations = HashMap::new();
        orientations.insert(other_tom, straight_tom());
        let cat_positions = vec![(other_tom, Position::new(2, 0))];
        let out = resolve_pairing_target(
            &registry,
            cat,
            Position::new(0, 0),
            Gender::Tom,
            Orientation::Straight,
            &cat_positions,
            &orientations,
            &relationships,
            0,
            None,
        );
        assert!(
            out.is_none(),
            "two straight Toms with a Friends bond are not orientation-compatible"
        );
    }

    #[test]
    fn resolver_picks_argmax_on_fondness_when_ties_elsewhere() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(pairing_activity_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let beloved = Entity::from_raw_u32(2).unwrap();
        let acquaintance = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        let r1 = relationships.get_or_insert(cat, beloved);
        r1.bond = Some(BondType::Friends);
        r1.fondness = 0.8;
        r1.romantic = 0.1;
        let r2 = relationships.get_or_insert(cat, acquaintance);
        r2.bond = Some(BondType::Friends);
        r2.fondness = 0.4;
        r2.romantic = 0.1;
        let mut orientations = HashMap::new();
        orientations.insert(beloved, straight_tom());
        orientations.insert(acquaintance, straight_tom());
        let cat_positions = vec![
            (beloved, Position::new(2, 0)),
            (acquaintance, Position::new(2, 1)),
        ];
        let (gender, orient) = straight_queens();
        let out = resolve_pairing_target(
            &registry,
            cat,
            Position::new(0, 0),
            gender,
            orient,
            &cat_positions,
            &orientations,
            &relationships,
            0,
            None,
        );
        assert_eq!(out, Some(beloved));
    }

    #[test]
    fn resolver_excludes_out_of_range_candidates() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(pairing_activity_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, far).bond = Some(BondType::Friends);
        let mut orientations = HashMap::new();
        orientations.insert(far, straight_tom());
        // 50 tiles away — beyond PAIRING_TARGET_RANGE.
        let cat_positions = vec![(far, Position::new(50, 0))];
        let (gender, orient) = straight_queens();
        let out = resolve_pairing_target(
            &registry,
            cat,
            Position::new(0, 0),
            gender,
            orient,
            &cat_positions,
            &orientations,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_self() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(pairing_activity_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let orientations = HashMap::new();
        let cat_positions = vec![(cat, Position::new(0, 0))];
        let (gender, orient) = straight_queens();
        let out = resolve_pairing_target(
            &registry,
            cat,
            Position::new(0, 0),
            gender,
            orient,
            &cat_positions,
            &orientations,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }
}
