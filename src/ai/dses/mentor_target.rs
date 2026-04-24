//! `MentorTargetDse` — §6.5.3 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning apprentice selection for `Mentor`. Pairs
//! with the self-state [`MentorDse`](super::mentor::MentorDse) which
//! decides *whether* to mentor (based on warmth, diligence, ambition);
//! this DSE decides *whom*.
//!
//! Phase 4c.5 scope: silent-divergence-fix for the MentorCat path.
//!
//! - `disposition.rs::build_socializing_chain`'s `can_mentor` branch
//!   today uses the `socialize_target` (fondness+novelty picked) as
//!   the apprentice, which means mentoring decisions ignore skill-gap
//!   entirely. §6.1 Critical.
//! - `goap.rs::resolve_goap_plans::MentorCat`'s `find_social_target`
//!   call (fondness-only, no skill-gap filter) retires for the
//!   MentorCat branch — the same legacy resolver stays in place for
//!   `GroomOther` until §6.5.4 ports.
//!
//! Three per-target considerations per §6.5.3, with the
//! `apprentice-receptivity` axis deferred until §4.3's `Apprentice`
//! marker author system lands (open-work #14 marker-roster second
//! bullet). Weights renormalized from the spec's
//! (0.20/0.20/0.40/0.20) by dropping the 0.20 and dividing the
//! remaining three by 0.80:
//!
//! | # | Consideration       | Scalar name          | Curve                   | Spec weight | Renormalized |
//! |---|---------------------|----------------------|-------------------------|-------------|--------------|
//! | 1 | distance            | `target_nearness`    | `Quadratic(exp=2)` r=8  | 0.20        | 0.25         |
//! | 2 | fondness            | `target_fondness`    | `Linear(1, 0)`          | 0.20        | 0.25         |
//! | 3 | skill-gap-magnitude | `target_skill_gap`   | `Logistic(8, 0.4)`      | 0.40        | 0.50         |
//!
//! No bond-tier eligibility filter — mentoring relationships grow *out
//! of* the act of mentoring, so bond is an output not an input
//! (contrast with Mate's `Partners|Mates` filter).
//!
//! Skill-gap signal: `max_k (self.skills[k] − target.skills[k]).max(0)`,
//! clamped to `[0, 1]` before the Logistic curve. Logistic(8, 0.4)
//! saturates near gap=0.8 and suppresses near gap=0, matching the
//! §6.5.3 design-intent that "gap-too-small (near peer) or gap-too-
//! large (overwhelming) both suppress via S-curve" — gap-too-large
//! case is already handled by clamp+Logistic upper-saturation, and
//! gap-too-small (near 0) sits below the midpoint.

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
use crate::components::skills::Skills;
use crate::resources::relationships::Relationships;

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
pub const TARGET_SKILL_GAP_INPUT: &str = "target_skill_gap";

/// Candidate-pool range in Manhattan tiles. Matches `SOCIALIZE_TARGET_RANGE`
/// / `MATE_TARGET_RANGE` (10) to preserve outer-gate semantics —
/// mentors find apprentices in the same colony cluster as social
/// partners. Changing it would shift the candidate population and is
/// a balance decision deferred to post-refactor per open-work #14.
pub const MENTOR_TARGET_RANGE: f32 = 10.0;

/// §6.5.3 `Mentor` target-taking DSE factory.
pub fn mentor_target_dse() -> TargetTakingDse {
    let nearness_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // Logistic on skill gap: midpoint 0.4 matches the prior
    // resolver's coarse threshold pair (high=0.6 / low=0.3 → ~0.3
    // gap minimum). Steepness 8 gives a sharp S-curve that peaks
    // above gap≈0.5 and zeroes below gap≈0.2, so near-peer pairs
    // don't score as apprentices.
    let skill_gap_curve = Curve::Logistic {
        steepness: 8.0,
        midpoint: 0.4,
    };

    TargetTakingDse {
        id: DseId("mentor_target"),
        candidate_query: mentor_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_NEARNESS_INPUT,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_FONDNESS_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_SKILL_GAP_INPUT,
                skill_gap_curve,
            )),
        ],
        // WeightedSum matches the social-family pattern (Socialize /
        // Mate). CompensatedProduct would gate any low axis — a
        // near-distance-but-low-fondness apprentice would score 0,
        // which over-punishes mentorship of strangers. The skill-gap
        // axis's weight (0.5) is dominant by design: gap is the
        // defining mentorship signal per §6.5.3.
        composition: Composition::weighted_sum(vec![0.25, 0.25, 0.5]),
        aggregation: TargetAggregation::Best,
        intention: mentor_intention,
    }
}

fn mentor_candidate_query_doc(_cat: Entity) -> &'static str {
    "cats within MENTOR_TARGET_RANGE, excluding self, no bond filter"
}

fn mentor_intention(_target: Entity) -> Intention {
    // §7.3: Mentor is a constituent action of the Socializing
    // disposition and rides Socializing's `OpenMinded` strategy.
    Intention::Activity {
        kind: ActivityKind::Mentor,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::OpenMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Maximum positive per-skill gap between `self_skills` and
/// `target_skills`, clamped to `[0, 1]`. The Logistic curve saturates
/// above ~0.8, so clamping to 1.0 preserves the S-curve shape without
/// letting very high-skill mentors dominate.
fn max_skill_gap(self_skills: &Skills, target_skills: &Skills) -> f32 {
    let pairs = [
        (self_skills.hunting, target_skills.hunting),
        (self_skills.foraging, target_skills.foraging),
        (self_skills.herbcraft, target_skills.herbcraft),
        (self_skills.building, target_skills.building),
        (self_skills.combat, target_skills.combat),
        (self_skills.magic, target_skills.magic),
    ];
    pairs
        .iter()
        .map(|(mine, other)| (mine - other).max(0.0))
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0)
}

/// Pick the best apprentice for `cat` via the registered
/// [`mentor_target_dse`]. Returns `None` iff no eligible candidate
/// exists (nobody in range, or no candidate has a positive skill
/// difference with `cat`).
///
/// `self_skills` is the scoring cat's Skills snapshot — needed for
/// skill-gap computation alongside each candidate's own Skills.
/// `skills_lookup` resolves a candidate's Skills; it returns `None`
/// for entities without the component (dead cats, non-cat entities
/// incorrectly in the candidate snapshot — the callers filter
/// upstream but the resolver is defensive).
#[allow(clippy::too_many_arguments)]
pub fn resolve_mentor_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_positions: &[(Entity, Position)],
    self_skills: &Skills,
    skills_lookup: &dyn Fn(Entity) -> Option<Skills>,
    relationships: &Relationships,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "mentor_target")?;

    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut skills_by_entity: std::collections::HashMap<Entity, Skills> =
        std::collections::HashMap::new();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist > MENTOR_TARGET_RANGE {
            continue;
        }
        let Some(other_skills) = skills_lookup(*other) else {
            continue;
        };
        candidates.push(*other);
        positions.push(*other_pos);
        skills_by_entity.insert(*other, other_skills);
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
                (1.0 - dist / MENTOR_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_SKILL_GAP_INPUT => skills_by_entity
                .get(&target)
                .map(|other| max_skill_gap(self_skills, other))
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
                .set_target_ranking("mentor_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills_with(hunting: f32, foraging: f32) -> Skills {
        Skills {
            hunting,
            foraging,
            herbcraft: 0.05,
            building: 0.1,
            combat: 0.05,
            magic: 0.0,
        }
    }

    #[test]
    fn mentor_target_dse_id_stable() {
        assert_eq!(mentor_target_dse().id().0, "mentor_target");
    }

    #[test]
    fn mentor_target_has_three_axes() {
        assert_eq!(mentor_target_dse().per_target_considerations().len(), 3);
    }

    #[test]
    fn mentor_target_weights_sum_to_one() {
        let sum: f32 = mentor_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn mentor_target_uses_best_aggregation() {
        assert_eq!(mentor_target_dse().aggregation(), TargetAggregation::Best);
    }

    #[test]
    fn max_skill_gap_picks_largest_positive_diff() {
        let mentor = skills_with(0.8, 0.2);
        let apprentice = skills_with(0.1, 0.1);
        // Max positive gap is hunting: 0.8 - 0.1 = 0.7.
        let gap = max_skill_gap(&mentor, &apprentice);
        assert!((gap - 0.7).abs() < 1e-5);
    }

    #[test]
    fn max_skill_gap_ignores_reverse_gaps() {
        // Apprentice outranks mentor on one axis; the resolver ignores
        // negative gaps (we're looking for *what the mentor can teach*).
        let mentor = skills_with(0.8, 0.2);
        let apprentice = skills_with(0.1, 0.9);
        let gap = max_skill_gap(&mentor, &apprentice);
        assert!((gap - 0.7).abs() < 1e-5);
    }

    #[test]
    fn max_skill_gap_clamps_to_one() {
        // Very high skill deltas saturate at 1.0 so the Logistic curve
        // doesn't get fed out-of-range inputs.
        let mentor = skills_with(5.0, 0.2);
        let apprentice = skills_with(0.0, 0.1);
        let gap = max_skill_gap(&mentor, &apprentice);
        assert!((gap - 1.0).abs() < 1e-5);
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let self_skills = Skills::default();
        let skills_lookup = |_: Entity| -> Option<Skills> { None };
        let out = resolve_mentor_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            &self_skills,
            &skills_lookup,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_when_no_candidates_in_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mentor_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let self_skills = skills_with(0.9, 0.2);
        let far_skills = skills_with(0.1, 0.1);
        let skills_lookup = move |e: Entity| -> Option<Skills> {
            if e == far {
                Some(far_skills.clone())
            } else {
                None
            }
        };
        let cat_positions = vec![(far, Position::new(50, 0))];
        let out = resolve_mentor_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &self_skills,
            &skills_lookup,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_self() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mentor_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let self_skills = skills_with(0.9, 0.2);
        let skills_lookup = |_: Entity| -> Option<Skills> { None };
        let cat_positions = vec![(cat, Position::new(0, 0))];
        let out = resolve_mentor_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &self_skills,
            &skills_lookup,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_skips_candidates_without_skills() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mentor_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let skillless = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let self_skills = skills_with(0.9, 0.2);
        let skills_lookup = |_: Entity| -> Option<Skills> { None };
        let cat_positions = vec![(skillless, Position::new(2, 0))];
        let out = resolve_mentor_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &self_skills,
            &skills_lookup,
            &relationships,
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_picks_larger_skill_gap_all_else_equal() {
        // Two candidates at equal distance, tied fondness. The one
        // with the bigger skill gap wins — this is the §6.1-Critical
        // fix: the legacy `find_social_target` path picked by fondness
        // only and ignored skill entirely.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mentor_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let novice = Entity::from_raw_u32(2).unwrap();
        let near_peer = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, novice).fondness = 0.5;
        relationships.get_or_insert(cat, near_peer).fondness = 0.5;

        let self_skills = skills_with(0.9, 0.2);
        let novice_skills = skills_with(0.1, 0.1); // gap=0.8 → Logistic~1
        let near_peer_skills = skills_with(0.8, 0.1); // gap=0.1 → Logistic~0.05
        let skills_lookup = move |e: Entity| -> Option<Skills> {
            if e == novice {
                Some(novice_skills.clone())
            } else if e == near_peer {
                Some(near_peer_skills.clone())
            } else {
                None
            }
        };

        // Both within range, at equal distance.
        let cat_positions = vec![
            (novice, Position::new(3, 0)),
            (near_peer, Position::new(3, 1)),
        ];
        let out = resolve_mentor_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &self_skills,
            &skills_lookup,
            &relationships,
            0,
            None,
        );
        assert_eq!(out, Some(novice));
    }

    #[test]
    fn resolver_skill_gap_dominates_fondness_bias() {
        // Even with a big fondness advantage, a near-peer (gap ≈ 0)
        // loses to a novice (gap ≈ 0.8) because skill-gap's weight
        // (0.5) plus Logistic saturation outweighs fondness's 0.25
        // contribution. Encodes the §6.5.3 design-intent that the
        // skill-gap axis is the dominant mentorship signal.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mentor_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let novice = Entity::from_raw_u32(2).unwrap();
        let dear_peer = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, novice).fondness = 0.1;
        relationships.get_or_insert(cat, dear_peer).fondness = 0.95;

        let self_skills = skills_with(0.9, 0.2);
        let novice_skills = skills_with(0.05, 0.1);
        let near_peer_skills = skills_with(0.85, 0.1);
        let skills_lookup = move |e: Entity| -> Option<Skills> {
            if e == novice {
                Some(novice_skills.clone())
            } else if e == dear_peer {
                Some(near_peer_skills.clone())
            } else {
                None
            }
        };

        let cat_positions = vec![
            (novice, Position::new(3, 0)),
            (dear_peer, Position::new(3, 1)),
        ];
        let out = resolve_mentor_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &self_skills,
            &skills_lookup,
            &relationships,
            0,
            None,
        );
        assert_eq!(out, Some(novice));
    }

    #[test]
    fn intention_is_mentor_activity() {
        let dse = mentor_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Mentor),
            other => panic!("expected Activity intention, got {other:?}"),
        }
    }
}
