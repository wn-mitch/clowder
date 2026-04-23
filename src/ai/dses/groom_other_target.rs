//! `GroomOtherTargetDse` — §6.5.4 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning allogrooming partner selection. Pairs with
//! the self-state [`GroomOtherDse`](super::groom_other::groom_other_dse)
//! which decides *whether* to allogroom; this DSE decides *whom*.
//!
//! Phase 4c.6 scope — full retirement of `find_social_target`:
//!
//! - `disposition.rs::build_socializing_chain`'s `GroomOther` branch
//!   reuses the `socialize_target`-picked cat today — fondness /
//!   novelty / species-compat mix. That ignores the core allogrooming
//!   signals: *adjacency* (grooming is a physical act) and
//!   *target-need-warmth* (the need the groomed cat has that you're
//!   answering). §6.5.4 names those as first-class axes; this port
//!   adds them.
//! - `goap.rs::resolve_goap_plans::GroomOther`'s `find_social_target`
//!   call (fondness-only, no warmth / no adjacency weight) retires —
//!   GroomOther is the last `find_social_target` caller after the
//!   Socialize / Mate / Mentor ports, so the legacy helper goes away
//!   with this port.
//!
//! Four per-target considerations per §6.5.4:
//!
//! | # | Consideration       | Scalar name              | Curve                          | Weight |
//! |---|---------------------|--------------------------|--------------------------------|--------|
//! | 1 | distance            | `target_nearness`        | `Logistic(15, 0.85)` near-step | 0.30   |
//! | 2 | fondness            | `target_fondness`        | `Linear(1, 0)`                 | 0.30   |
//! | 3 | target-need-warmth  | `target_warmth_deficit`  | `Quadratic(exp=2)`             | 0.30   |
//! | 4 | kinship             | `target_kinship`         | `Piecewise` (kin=1.0 / else=0.5) | 0.10  |
//!
//! **Distance curve interpretation.** The spec cell reads
//! `Logistic(steepness=15, midpoint=1), range=1–2`; the midpoint
//! parameter is reified here onto the normalized distance signal
//! `1 − dist/GROOM_OTHER_TARGET_RANGE`. With range = 10 (social
//! candidate pool) the midpoint lands at signal = 0.85, corresponding
//! to dist ≈ 1.5 — the spec's 1–2 tile design-intent band. The curve
//! saturates near dist = 0, crosses 0.5 at dist ≈ 1.5, and drops
//! below 0.1 by dist = 3. Allogrooming requires near-adjacency; the
//! curve enforces it without a hard candidate-pool cutoff that would
//! exclude the 2-tile partner the cat is about to walk one step to.
//!
//! **Warmth signal.** `target.needs.temperature` deficit = `1 −
//! temperature`. Convex `Quadratic(2)` amplifies desperate need
//! — matches the `Caretake` urgency axis shape (§6.5.6) and mirrors
//! the design-intent that outreach scales with unmet-need magnitude,
//! not linear proximity to crisis. Warmth deficit is not the *whole*
//! groom signal (fondness ties in the affective side) but it's the
//! axis the prior `find_social_target` path could not see.
//!
//! **Kinship signal.** Cliff-via-Piecewise matches §6.5.4's spec row:
//! parent-child pairs (determined from `KittenDependency.mother /
//! .father` in either direction) score 1.0; non-kin score 0.5. The
//! weight is intentionally small (0.10) — kin bias is a nudge, not a
//! gate, so colony-wide allogrooming remains the norm.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{Consideration, ScalarConsideration};
use crate::ai::curves::Curve;
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{evaluate_target_taking, TargetAggregation, TargetTakingDse};
use crate::components::physical::Position;
use crate::resources::relationships::Relationships;

pub const TARGET_NEARNESS_INPUT: &str = "target_nearness";
pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
pub const TARGET_WARMTH_DEFICIT_INPUT: &str = "target_warmth_deficit";
pub const TARGET_KINSHIP_INPUT: &str = "target_kinship";

/// Candidate-pool range in Manhattan tiles. Matches `SOCIALIZE_TARGET_RANGE`
/// / `MENTOR_TARGET_RANGE` (10) so Groom-other inherits the same social
/// outer gate. The Logistic distance curve then sharpens adjacency
/// preference inside the pool; only 1–2 tiles receive appreciable score.
pub const GROOM_OTHER_TARGET_RANGE: f32 = 10.0;

/// §6.5.4 `Groom` (other) target-taking DSE factory.
pub fn groom_other_target_dse() -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // Convex amplification — doubling the target's warmth deficit
    // quadruples the contribution. Mirrors Caretake's `kitten-hunger`
    // axis shape (§6.5.6) for consistency across outreach DSEs.
    let warmth_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    // Near-step distance curve. Midpoint 0.85 on the normalized
    // `1 − dist/range` signal crosses 0.5 at dist ≈ 1.5 with range=10,
    // within the spec's 1–2 tile range row. Steepness 15 gives a
    // sharp transition — by dist=3 the score is <0.1.
    let nearness_curve = Curve::Logistic {
        steepness: 15.0,
        midpoint: 0.85,
    };
    // Piecewise cliff: kin (signal=1.0) → 1.0; non-kin (signal=0.0) →
    // 0.5. The resolver collapses kin/non-kin to a boolean upstream,
    // so only two signal values are ever fed in.
    let kinship_curve = Curve::Piecewise {
        knots: vec![(0.0, 0.5), (0.999, 0.5), (1.0, 1.0)],
    };

    TargetTakingDse {
        id: DseId("groom_other_target"),
        candidate_query: groom_other_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Scalar(ScalarConsideration::new(TARGET_NEARNESS_INPUT, nearness_curve)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_FONDNESS_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_WARMTH_DEFICIT_INPUT,
                warmth_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_KINSHIP_INPUT,
                kinship_curve,
            )),
        ],
        // WeightedSum matches the social-family (Socialize / Mate /
        // Mentor) convention. CompensatedProduct would null a
        // non-kin-but-beloved target via the 0.5 kinship signal
        // passing through multiplicative gating — the spec intent is
        // a small additive bias, not a gate.
        composition: Composition::weighted_sum(vec![0.30, 0.30, 0.30, 0.10]),
        aggregation: TargetAggregation::Best,
        intention: groom_other_intention,
    }
}

fn groom_other_candidate_query_doc(_cat: Entity) -> &'static str {
    "cats within GROOM_OTHER_TARGET_RANGE, excluding self, no bond filter"
}

fn groom_other_intention(_target: Entity) -> Intention {
    Intention::Activity {
        kind: ActivityKind::Allogroom,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::OpenMinded,
    }
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best allogrooming target for `cat` via the registered
/// [`groom_other_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
///
/// The resolver needs two lookups that only the caller can provide:
/// - `temperature_lookup(target)` — target's `needs.temperature` in
///   `[0, 1]`. Returns `None` if the entity has no `Needs` (dead / non-
///   cat) — the resolver treats the target as skipped.
/// - `is_kin(self, target)` — bidirectional parent-child check via
///   `KittenDependency.mother / .father`. Returns `true` iff either
///   entity is the other's recorded parent. Cheap O(1) HashMap lookup
///   built once per tick at the call site.
#[allow(clippy::too_many_arguments)]
pub fn resolve_groom_other_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_positions: &[(Entity, Position)],
    temperature_lookup: &dyn Fn(Entity) -> Option<f32>,
    is_kin: &dyn Fn(Entity, Entity) -> bool,
    relationships: &Relationships,
    tick: u64,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "groom_other_target")?;

    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut temperatures: std::collections::HashMap<Entity, f32> =
        std::collections::HashMap::new();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist > GROOM_OTHER_TARGET_RANGE {
            continue;
        }
        let Some(temp) = temperature_lookup(*other) else {
            continue;
        };
        candidates.push(*other);
        positions.push(*other_pos);
        temperatures.insert(*other, temp);
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
                (1.0 - dist / GROOM_OTHER_TARGET_RANGE).clamp(0.0, 1.0)
            }
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_WARMTH_DEFICIT_INPUT => temperatures
                .get(&target)
                .map(|t| (1.0 - t).clamp(0.0, 1.0))
                .unwrap_or(0.0),
            TARGET_KINSHIP_INPUT => {
                if is_kin(cat, target) {
                    1.0
                } else {
                    0.0
                }
            }
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

    evaluate_target_taking(
        dse,
        cat,
        &candidates,
        &positions,
        &ctx,
        &fetch_self,
        &fetch_target,
    )
    .winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groom_other_target_dse_id_stable() {
        assert_eq!(groom_other_target_dse().id().0, "groom_other_target");
    }

    #[test]
    fn groom_other_target_has_four_axes() {
        assert_eq!(
            groom_other_target_dse().per_target_considerations().len(),
            4
        );
    }

    #[test]
    fn groom_other_target_weights_sum_to_one() {
        let sum: f32 = groom_other_target_dse()
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn groom_other_target_uses_best_aggregation() {
        assert_eq!(
            groom_other_target_dse().aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn intention_is_allogroom_activity() {
        let dse = groom_other_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Allogroom),
            other => panic!("expected Activity intention, got {other:?}"),
        }
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_when_no_candidates_in_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let cat_positions = vec![(far, Position::new(50, 0))];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_self() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let relationships = Relationships::default();
        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let cat_positions = vec![(cat, Position::new(0, 0))];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_skips_candidates_without_temperature() {
        // A non-cat entity (dead / no Needs component) that somehow
        // ended up in the candidate snapshot gets skipped rather than
        // scored. Matches the Mentor resolver's Skills-absence handling.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let orphan = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let temperature_lookup = |_: Entity| -> Option<f32> { None };
        let is_kin = |_: Entity, _: Entity| -> bool { false };
        let cat_positions = vec![(orphan, Position::new(1, 0))];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert!(out.is_none());
    }

    #[test]
    fn picks_lower_warmth_deficit_all_else_equal() {
        // Two candidates at equal distance with tied fondness + non-kin;
        // the colder cat (larger warmth deficit) wins — this is the
        // axis `find_social_target` could not see.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let cold = Entity::from_raw_u32(2).unwrap();
        let warm = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, cold).fondness = 0.5;
        relationships.get_or_insert(cat, warm).fondness = 0.5;

        let temperature_lookup = move |e: Entity| -> Option<f32> {
            if e == cold {
                Some(0.1) // deficit = 0.9
            } else if e == warm {
                Some(0.95) // deficit = 0.05
            } else {
                None
            }
        };
        let is_kin = |_: Entity, _: Entity| -> bool { false };

        // Both at same distance (1 tile, Manhattan).
        let cat_positions = vec![(cold, Position::new(1, 0)), (warm, Position::new(0, 1))];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert_eq!(out, Some(cold));
    }

    #[test]
    fn kin_beats_non_kin_when_other_axes_tied() {
        // Weight on kinship is only 0.10 so the kin bias is a nudge,
        // but with all other axes tied it's the decider.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let kin = Entity::from_raw_u32(2).unwrap();
        let stranger = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, kin).fondness = 0.5;
        relationships.get_or_insert(cat, stranger).fondness = 0.5;

        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = move |_: Entity, target: Entity| -> bool { target == kin };

        let cat_positions = vec![(kin, Position::new(1, 0)), (stranger, Position::new(0, 1))];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert_eq!(out, Some(kin));
    }

    #[test]
    fn adjacency_dominates_fondness_at_distance() {
        // Adjacent-but-less-liked vs. near-liked at distance:
        // allogrooming is physical, so adjacency must win when the
        // gap is large enough. At dist=1 the nearness signal is 0.9
        // (Logistic ≈ 0.68); at dist=5 signal is 0.5 (Logistic
        // ≈ 0.004). Adjacent cat wins decisively.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let near_acquaintance = Entity::from_raw_u32(2).unwrap();
        let far_dearest = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships
            .get_or_insert(cat, near_acquaintance)
            .fondness = 0.4;
        relationships.get_or_insert(cat, far_dearest).fondness = 0.95;

        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = |_: Entity, _: Entity| -> bool { false };

        let cat_positions = vec![
            (near_acquaintance, Position::new(1, 0)),
            (far_dearest, Position::new(5, 0)),
        ];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert_eq!(out, Some(near_acquaintance));
    }

    #[test]
    fn fondness_dominates_when_warmth_and_distance_tied() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(2).unwrap();
        let stranger = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, friend).fondness = 0.9;
        relationships.get_or_insert(cat, stranger).fondness = 0.1;

        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = |_: Entity, _: Entity| -> bool { false };

        let cat_positions = vec![
            (friend, Position::new(1, 0)),
            (stranger, Position::new(0, 1)),
        ];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert_eq!(out, Some(friend));
    }

    #[test]
    fn warmth_deficit_convexity_amplifies_severe_need() {
        // Two candidates tied on fondness (0.5) and distance (both
        // 1 tile); only warmth differs. Quadratic amplification means
        // the near-freezing cat (deficit=0.9 → curve 0.81) dominates
        // the chilly one (deficit=0.4 → curve 0.16) by far more than
        // their linear gap. Encodes the §6.5.4 design intent that
        // "desperate-need amplifies outreach."
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let freezing = Entity::from_raw_u32(2).unwrap();
        let chilly = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, freezing).fondness = 0.5;
        relationships.get_or_insert(cat, chilly).fondness = 0.5;

        let temperature_lookup = move |e: Entity| -> Option<f32> {
            if e == freezing {
                Some(0.1)
            } else if e == chilly {
                Some(0.6)
            } else {
                None
            }
        };
        let is_kin = |_: Entity, _: Entity| -> bool { false };

        let cat_positions = vec![
            (freezing, Position::new(1, 0)),
            (chilly, Position::new(0, 1)),
        ];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert_eq!(out, Some(freezing));
    }

    #[test]
    fn distance_curve_crosses_midpoint_near_two_tiles() {
        // Asserts the near-step shape of the Logistic(15, 0.85) curve
        // on the normalized `1 - dist/range` signal: adjacent wins
        // clearly over dist=2 when other axes are tied. The curve
        // contribution at dist=1 (signal=0.9) is ≈0.68, at dist=2
        // (signal=0.8) is ≈0.32 — a ~2× gap big enough to overcome
        // the 0.10 weight jitter room.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(groom_other_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let adjacent = Entity::from_raw_u32(2).unwrap();
        let two_tiles = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, adjacent).fondness = 0.5;
        relationships.get_or_insert(cat, two_tiles).fondness = 0.5;

        let temperature_lookup = |_: Entity| -> Option<f32> { Some(0.5) };
        let is_kin = |_: Entity, _: Entity| -> bool { false };

        let cat_positions = vec![
            (adjacent, Position::new(1, 0)),
            (two_tiles, Position::new(2, 0)),
        ];
        let out = resolve_groom_other_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &temperature_lookup,
            &is_kin,
            &relationships,
            0,
        );
        assert_eq!(out, Some(adjacent));
    }
}
