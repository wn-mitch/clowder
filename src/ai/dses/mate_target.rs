//! `MateTargetDse` — §6.5.2 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning partner selection for `Mate`. Pairs with
//! the self-state [`MateDse`](super::mate::MateDse) (mating-deficit +
//! warmth desire) which decides *whether* to mate; this DSE decides
//! *with whom*.
//!
//! Phase 4c.2 scope: silent-divergence-fix + first-class replacement
//! of both legacy Mate target-pickers.
//!
//! - `disposition.rs::build_mating_chain`'s
//!   `romantic + fondness - 0.05 × distance` scorer retires.
//! - `goap.rs::resolve_goap_plans::MateWith`'s
//!   `find_social_target` call (fondness-only, **no bond filter**)
//!   retires — the silent divergence here was the more dangerous:
//!   goap could pick a non-partner as the mating target, then
//!   `resolve_mate_with` would score it as eligible because the
//!   upstream eligibility gate had already fired.
//!
//! Three per-target considerations per §6.5.2, with the
//! fertility-window axis deferred until §7.M.7.5's phase→scalar
//! signal mapping lands (Enumeration Debt). Weights renormalized
//! from the spec's (0.15/0.40/0.25/0.20) by dropping the 0.20 and
//! dividing the remaining three by 0.80:
//!
//! | # | Consideration | Scalar name       | Curve                            | Spec weight | Renormalized |
//! |---|---------------|-------------------|----------------------------------|-------------|--------------|
//! | 1 | distance      | `target_nearness` | `Logistic(20, 0.5)` near-step    | 0.15        | 0.1875       |
//! | 2 | romantic      | `target_romantic` | `Linear(1, 0)`                   | 0.40        | 0.5000       |
//! | 3 | fondness      | `target_fondness` | `Linear(1, 0)`                   | 0.25        | 0.3125       |
//!
//! Candidate filter: nearby cats within `MATE_TARGET_RANGE` tiles
//! whose bond is `Partners` or `Mates`. The bond filter is a
//! structural eligibility gate (§4 / §9.3), not a consideration —
//! matching `build_mating_chain`'s current behavior and closing the
//! bond-filter gap that `find_social_target` left open.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::resources::relationships::{BondType, Relationships};

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_ROMANTIC_INPUT: &str = "target_romantic";
pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";

/// Candidate-pool range for Mate partner selection. Mate's spec
/// template range is 1 (adjacency), but candidate gathering needs a
/// wider pool so near-but-not-adjacent Partners remain scoreable via
/// the Logistic distance curve. Matches the existing
/// `DispositionConstants::social_target_range` semantics.
pub const MATE_TARGET_RANGE: f32 = 10.0;

/// §6.5.2 `Mate` target-taking DSE factory.
pub fn mate_target_dse() -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // Logistic near-step: sharp drop-off as distance grows; max at
    // adjacency. `target_nearness = 1 - dist/range`, so nearness=1 at
    // adjacent, nearness=0.5 at half-range (dist=5). Logistic(20, 0.5)
    // crosses 0.5 at nearness=0.5, saturating near nearness=1.
    let nearness_curve = Curve::Logistic {
        steepness: 20.0,
        midpoint: 0.5,
    };

    TargetTakingDse {
        id: DseId("mate_target"),
        candidate_query: mate_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(TARGET_NEARNESS_INPUT, nearness_curve)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_ROMANTIC_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(TARGET_FONDNESS_INPUT, linear)),
        ],
        composition: Composition::weighted_sum(vec![0.1875, 0.5, 0.3125]),
        aggregation: TargetAggregation::Best,
        intention: mate_intention,
    }
}

fn mate_candidate_query_doc(_cat: Entity) -> &'static str {
    "cats within MATE_TARGET_RANGE with bond == Partners | Mates, excluding self"
}

fn mate_intention(_target: Entity) -> Intention {
    Intention::Activity {
        kind: ActivityKind::Pairing,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::SingleMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best mating partner for `cat` via the registered
/// [`mate_target_dse`]. Returns `None` iff no eligible candidate
/// exists (nobody in range OR no bonded partners in range).
///
/// Bond filter: only cats whose `Relationships::get(cat, other).bond`
/// is `Some(Partners)` or `Some(Mates)` are candidates. This closes
/// the gap where `goap.rs::find_social_target` picked targets
/// without a bond check, letting the MateWith step target non-mates
/// once the Mate disposition won selection.
pub fn resolve_mate_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "mate_target")?;

    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist > MATE_TARGET_RANGE {
            continue;
        }
        let bond = relationships
            .get(cat, *other)
            .and_then(|r| r.bond)
            .unwrap_or(BondType::Friends);
        if !matches!(bond, BondType::Partners | BondType::Mates) {
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
                (1.0 - dist / MATE_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_ROMANTIC_INPUT => relationships
                .get(cat, target)
                .map(|r| r.romantic)
                .unwrap_or(0.0),
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
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
                .set_target_ranking("mate_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::relationships::BondType;

    #[test]
    fn mate_target_dse_id_stable() {
        assert_eq!(mate_target_dse().id().0, "mate_target");
    }

    #[test]
    fn mate_target_has_three_axes() {
        assert_eq!(mate_target_dse().per_target_considerations().len(), 3);
    }

    #[test]
    fn mate_target_weights_sum_to_one() {
        let sum: f32 = mate_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_non_bonded_candidates() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mate_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend_not_partner = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, friend_not_partner).fondness = 0.9;
        relationships.get_or_insert(cat, friend_not_partner).romantic = 0.9;
        relationships.get_or_insert(cat, friend_not_partner).bond = Some(BondType::Friends);

        let cat_positions = vec![(friend_not_partner, Position::new(1, 0))];
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        // Friends bond doesn't pass the filter — even with romantic=0.9,
        // the resolver returns None. This is the bond-filter fix
        // that `find_social_target` left open.
        assert!(out.is_none());
    }

    #[test]
    fn resolver_picks_partners_bond_candidate() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mate_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, partner).fondness = 0.5;
        relationships.get_or_insert(cat, partner).romantic = 0.5;
        relationships.get_or_insert(cat, partner).bond = Some(BondType::Partners);

        let cat_positions = vec![(partner, Position::new(1, 0))];
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert_eq!(out, Some(partner));
    }

    #[test]
    fn resolver_picks_higher_romantic_when_both_partners() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mate_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let fond_partner = Entity::from_raw_u32(2).unwrap();
        let romantic_partner = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, fond_partner).fondness = 0.9;
        relationships.get_or_insert(cat, fond_partner).romantic = 0.2;
        relationships.get_or_insert(cat, fond_partner).bond = Some(BondType::Partners);
        relationships.get_or_insert(cat, romantic_partner).fondness = 0.3;
        relationships.get_or_insert(cat, romantic_partner).romantic = 0.9;
        relationships.get_or_insert(cat, romantic_partner).bond = Some(BondType::Partners);

        let cat_positions = vec![
            (fond_partner, Position::new(1, 0)),
            (romantic_partner, Position::new(1, 1)),
        ];
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        // Romantic weight (0.5) dominates fondness weight (0.3125),
        // so the more-romantic partner wins even with lower fondness.
        assert_eq!(out, Some(romantic_partner));
    }

    #[test]
    fn intention_is_pairing_activity() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mate_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let partner = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, partner).fondness = 0.5;
        relationships.get_or_insert(cat, partner).romantic = 0.5;
        relationships.get_or_insert(cat, partner).bond = Some(BondType::Mates);

        let cat_positions = vec![(partner, Position::new(1, 0))];
        let winner = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
        );
        assert_eq!(winner, Some(partner));
        // Verify Intention factory produces Pairing activity.
        let dse = mate_target_dse();
        let intention = (dse.intention)(partner);
        match intention {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Pairing),
            _ => panic!("expected Activity intention"),
        }
    }
}
