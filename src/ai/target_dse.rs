//! `TargetTakingDse` — §6.3 of `docs/systems/ai-substrate-refactor.md`.
//!
//! The target-taking inner loop per Mark ch 13 §"Deciding on Dinner"
//! and ch 14 §"Which Dude to Kill?" — for each candidate target, score
//! a per-target consideration bundle, then aggregate the per-candidate
//! scores into a single action score + winning target.
//!
//! Why this is a distinct type from [`Dse`](super::dse::Dse):
//!
//! - Regular `Dse` scores once per cat-tick from state that lives on
//!   the scoring cat itself.
//! - `TargetTakingDse` scores once *per candidate* from state that
//!   blends the scoring cat's view (distance, relationship) with the
//!   target's own state (injury, fondness, fertility phase). The
//!   evaluator must iterate candidates; a single `ScalarConsideration`
//!   closure can't.
//!
//! Why this unifies scoring + target selection (§6.2):
//!
//! - Pre-refactor, `disposition.rs` and `goap.rs` each owned their own
//!   target-resolver (different tie-break formulas → silent divergence).
//! - Post-refactor, **one** `TargetTakingDse` owns both the score and
//!   the winning target. The emitted `Intention` carries the target
//!   entity forward to GOAP planning. Both scoring paths consume the
//!   same result.
//!
//! This module ships the type + evaluator + aggregation rules. Per-DSE
//! ports (Socialize, Mate, Mentor, Groom-other, Hunt, Fight,
//! ApplyRemedy, Build, Caretake per §6.4) are follow-on — each
//! requires a per-target consideration bundle from §6.5 and a cutover
//! of the legacy resolver.

use bevy::prelude::Entity;

use super::composition::Composition;
use super::considerations::{CenterPolicy, Consideration};
use super::dse::{DseId, EvalCtx, Intention};

// ---------------------------------------------------------------------------
// TargetAggregation
// ---------------------------------------------------------------------------

/// How per-candidate scores combine into one action score. §6.3:
///
/// - [`Best`](TargetAggregation::Best) — `score = max(per_target)`,
///   `winning_target = argmax`. Default for most target-taking DSEs
///   (Socialize, Mate, Mentor, Groom, Hunt, Build, Caretake).
/// - [`SumTopN`](TargetAggregation::SumTopN) — `score = sum(top N)`,
///   `winning_target = argmax`. Threat aggregation — Fight's response
///   scales with the number of hostiles, not just the worst one
///   (§6.5.9 / §6.6 note).
/// - [`WeightedAverage`](TargetAggregation::WeightedAverage) — rare;
///   decaying contribution of ranked targets. Used where the presence
///   of multiple mid-tier candidates matters more than the single
///   best. `winning_target = argmax`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetAggregation {
    #[default]
    Best,
    SumTopN(usize),
    WeightedAverage,
}

// ---------------------------------------------------------------------------
// TargetTakingDse
// ---------------------------------------------------------------------------

/// A DSE whose score depends on a set of candidate targets rather than
/// only on the scoring cat's own state. See module doc for scope and
/// motivation.
///
/// `candidate_query` is a documentation-shape field — it names where
/// candidates come from for grep-ability, but the runtime evaluator
/// accepts the candidate set as a direct argument because Bevy systems
/// can't freely access `&World` from inside a scoring tick. Concrete
/// target-taking DSEs author a corresponding `SystemParam` bundle that
/// produces the candidate slice each tick; the fn-pointer here is
/// conventional documentation of that contract.
///
/// `per_target_considerations` is a list of the same `Consideration`
/// enum regular `Dse`s consume — the difference is interpretation:
/// `Spatial` considerations with `CenterPolicy::TargetPosition` sample
/// the *candidate's* position, not `self.pos`. The evaluator switches
/// the per-target `EvalCtx.target_position` as it iterates candidates.
///
/// `composition` reduces per-target considerations to one per-candidate
/// score — typically `CompensatedProduct` (§3.1), matching the regular
/// `Dse` default.
///
/// `aggregation` controls how per-candidate scores combine into one
/// action-level score + winning target.
///
/// `intention` is a fn pointer that builds the `Intention` carrying the
/// winning target forward to the GOAP planner. Parameterized on the
/// target `Entity` so downstream planning has the commitment anchor.
pub struct TargetTakingDse {
    pub id: DseId,
    pub candidate_query: fn(Entity) -> &'static str,
    pub per_target_considerations: Vec<Consideration>,
    pub composition: Composition,
    pub aggregation: TargetAggregation,
    pub intention: fn(Entity) -> Intention,
}

impl TargetTakingDse {
    pub fn id(&self) -> DseId {
        self.id
    }

    pub fn per_target_considerations(&self) -> &[Consideration] {
        &self.per_target_considerations
    }

    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn aggregation(&self) -> TargetAggregation {
        self.aggregation
    }
}

// ---------------------------------------------------------------------------
// ScoredTargetTakingDse
// ---------------------------------------------------------------------------

/// Per-DSE target-taking evaluator output. Carries the same "enough to
/// reconstruct" audit detail as [`ScoredDse`](super::eval::ScoredDse):
/// every per-candidate score, the aggregated action score, the winning
/// target entity, and the emitted intention.
#[derive(Debug, Clone)]
pub struct ScoredTargetTakingDse {
    pub id: DseId,
    /// Per-candidate scores, unranked (preserving candidate order as
    /// supplied by the caller). Ranking utilities like
    /// `ranked_candidates` sort; raw output is unsorted to keep the
    /// trace reconstructable.
    pub per_target: Vec<(Entity, f32)>,
    pub winning_target: Option<Entity>,
    pub aggregated_score: f32,
    pub intention: Option<Intention>,
}

impl ScoredTargetTakingDse {
    /// Return per-candidate scores sorted descending. Copy-returned so
    /// the canonical output vector stays in the candidate-order the
    /// caller supplied (useful for deterministic-trace emission).
    pub fn ranked_candidates(&self) -> Vec<(Entity, f32)> {
        let mut v = self.per_target.clone();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v
    }
}

/// Build a §11.3 `TargetRanking` from a scored target-taking DSE for
/// focal-cat trace emission. Caller provides a `name_lookup` closure
/// because this module has no ECS access — in practice callers resolve
/// `Entity → Name` from a frame-local snapshot they've already built.
/// Returns `None` when the candidate set was empty (no ranking to
/// emit).
///
/// Wired by target-taking DSE resolvers at their call site: on a focal
/// cat tick, after `evaluate_target_taking` returns, the caller pushes
/// the resulting ranking into `FocalScoreCapture::set_target_ranking`
/// and `emit_focal_trace` merges it into the matching L2 record's
/// `targets` field at flush time.
pub fn target_ranking_from_scored(
    scored: &ScoredTargetTakingDse,
    aggregation: TargetAggregation,
    name_lookup: &dyn Fn(Entity) -> String,
) -> Option<crate::resources::trace_log::TargetRanking> {
    if scored.per_target.is_empty() {
        return None;
    }
    let ranked = scored.ranked_candidates();
    let (contributing_count, agg_slug) = match aggregation {
        TargetAggregation::Best => (1usize, "Best".to_string()),
        TargetAggregation::SumTopN(n) => (n.min(ranked.len()), format!("SumTopN({n})")),
        TargetAggregation::WeightedAverage => (ranked.len(), "WeightedAverage".to_string()),
    };
    let candidates: Vec<crate::resources::trace_log::TargetCandidate> = ranked
        .iter()
        .enumerate()
        .map(
            |(i, (entity, score))| crate::resources::trace_log::TargetCandidate {
                name: name_lookup(*entity),
                score: *score,
                contributed: i < contributing_count,
            },
        )
        .collect();
    let winner = scored.winning_target.map(name_lookup);
    Some(crate::resources::trace_log::TargetRanking {
        aggregation: agg_slug,
        candidates,
        winner,
    })
}

/// Focal-cat target-ranking capture hook — passed by callers that want
/// per-candidate scores emitted to the trace sidecar. Keeping this as
/// a dedicated struct (rather than two optional params) makes it
/// obvious at the call site that the resolver is operating in a
/// focal-tracing context.
///
/// Every target-taking DSE resolver accepts `Option<FocalTargetHook<'_>>`
/// as its trailing param; non-focal callers pass `None` and pay zero
/// cost. On a focal tick, the resolver calls
/// [`target_ranking_from_scored`] with `hook.name_lookup` and routes
/// the result into [`FocalScoreCapture::set_target_ranking`] under the
/// DSE's id.
///
/// [`FocalScoreCapture::set_target_ranking`]: crate::resources::FocalScoreCapture::set_target_ranking
pub struct FocalTargetHook<'a> {
    pub capture: &'a crate::resources::FocalScoreCapture,
    pub name_lookup: &'a dyn Fn(Entity) -> String,
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

/// Score a target-taking DSE against a candidate set. §6.3 contract.
///
/// - If `candidates` is empty, returns a `ScoredTargetTakingDse` with
///   `winning_target: None`, `aggregated_score: 0.0`, and no
///   intention. The caller treats this the same as a regular `Dse`
///   scoring 0 — skip, or push with score 0 if the caller wants
///   explicit tracking.
/// - Per-candidate scoring mirrors `evaluate_single` for regular DSEs:
///   score each consideration against the candidate-positioned
///   `EvalCtx`, compose, apply the cat's Maslow pre-gate (if the DSE
///   opts in via a non-`u8::MAX` tier — MVP defers to Best-aggregation
///   without pre-gate since target-taking DSEs are already filtered by
///   eligibility markers upstream).
/// - Aggregation follows `dse.aggregation`:
///   - `Best` → score = max; winner = argmax.
///   - `SumTopN(n)` → score = sum of top-n; winner = argmax (the top
///     candidate anchors the plan; the sum-of-tail expresses threat
///     concentration but the cat plans against its #1 target).
///   - `WeightedAverage` → score = Σ (rank_weight[i] × score[i])
///     over ranked candidates; winner = argmax.
///
/// `fetch_target_scalar` is the per-target analog of the regular
/// `fetch_scalar` closure — resolves named scalars against the
/// candidate target (not the scoring cat). Example: `target_fondness`
/// reads `Relationships::get(cat, target).fondness`, which only the
/// target-scoped closure can resolve.
pub fn evaluate_target_taking(
    dse: &TargetTakingDse,
    cat: Entity,
    candidates: &[Entity],
    candidate_positions: &[crate::components::physical::Position],
    ctx: &EvalCtx,
    fetch_self_scalar: &dyn Fn(&str, Entity) -> f32,
    fetch_target_scalar: &dyn Fn(&str, Entity, Entity) -> f32,
) -> ScoredTargetTakingDse {
    debug_assert_eq!(
        candidates.len(),
        candidate_positions.len(),
        "candidate/position slices must match length"
    );

    if candidates.is_empty() {
        return ScoredTargetTakingDse {
            id: dse.id,
            per_target: Vec::new(),
            winning_target: None,
            aggregated_score: 0.0,
            intention: None,
        };
    }

    let considerations = dse.per_target_considerations();
    let composition = dse.composition();

    // Score each candidate through the per-target consideration bundle.
    let mut per_target: Vec<(Entity, f32)> = Vec::with_capacity(candidates.len());
    for (target, target_pos) in candidates.iter().zip(candidate_positions.iter()) {
        let scores: Vec<f32> = considerations
            .iter()
            .map(|c| {
                score_target_consideration(
                    c,
                    cat,
                    *target,
                    *target_pos,
                    ctx,
                    fetch_self_scalar,
                    fetch_target_scalar,
                )
            })
            .collect();
        let composed = composition.compose(&scores);
        per_target.push((*target, composed));
    }

    // Aggregate per-target scores → (aggregated_score, winning_target).
    let (aggregated_score, winning_target) = aggregate(&per_target, dse.aggregation);

    let intention = winning_target.map(|t| (dse.intention)(t));

    ScoredTargetTakingDse {
        id: dse.id,
        per_target,
        winning_target,
        aggregated_score,
        intention,
    }
}

/// Score one per-target consideration against a specific candidate.
/// Matches `score_consideration` in [`super::eval`] but with `target`
/// + `target_pos` threaded in so `CenterPolicy::TargetPosition` resolves
///   against the candidate, and scalar-by-name considerations get the
///   target-scoped fetcher.
fn score_target_consideration(
    consideration: &Consideration,
    cat: Entity,
    target: Entity,
    target_pos: crate::components::physical::Position,
    ctx: &EvalCtx,
    fetch_self_scalar: &dyn Fn(&str, Entity) -> f32,
    fetch_target_scalar: &dyn Fn(&str, Entity, Entity) -> f32,
) -> f32 {
    match consideration {
        Consideration::Scalar(s) => {
            // Convention: scalar names prefixed `target_` resolve via the
            // target-scoped fetcher; everything else resolves against the
            // scoring cat. This convention matches §6.5's naming of
            // target-side axes (`target_fondness`, `target_injury`, etc.).
            if s.name.starts_with("target_") {
                s.score(fetch_target_scalar(s.name, cat, target))
            } else {
                s.score(fetch_self_scalar(s.name, cat))
            }
        }
        Consideration::Spatial(s) => {
            let pos = match s.center {
                CenterPolicy::SelfPosition => ctx.self_position,
                CenterPolicy::TargetPosition => target_pos,
            };
            s.score((ctx.sample_map)(s.map_key, pos))
        }
        Consideration::Marker(m) => {
            // Marker considerations on the target test the target's
            // marker, not the cat's. The `has_marker` closure is cat-
            // parameterized today; for target-taking we pass the target
            // entity directly.
            m.score((ctx.has_marker)(m.marker, target))
        }
    }
}

/// Aggregation dispatch — `(per_target, rule) → (score, winner)`.
fn aggregate(per_target: &[(Entity, f32)], rule: TargetAggregation) -> (f32, Option<Entity>) {
    match rule {
        TargetAggregation::Best => {
            let winner = per_target
                .iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .copied();
            match winner {
                Some((e, s)) => (s, Some(e)),
                None => (0.0, None),
            }
        }
        TargetAggregation::SumTopN(n) => {
            if per_target.is_empty() {
                return (0.0, None);
            }
            let mut ranked: Vec<(Entity, f32)> = per_target.to_vec();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let sum: f32 = ranked.iter().take(n).map(|(_, s)| *s).sum();
            let winner = ranked.first().map(|(e, _)| *e);
            (sum, winner)
        }
        TargetAggregation::WeightedAverage => {
            if per_target.is_empty() {
                return (0.0, None);
            }
            let mut ranked: Vec<(Entity, f32)> = per_target.to_vec();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            // Rank weights: 1/(rank+1) — first candidate full weight,
            // second half, third one-third, etc. Sum-of-weights
            // normalized to 1.0 to keep output bounded in [0, 1].
            let weights: Vec<f32> = (0..ranked.len()).map(|i| 1.0 / (i as f32 + 1.0)).collect();
            let w_sum: f32 = weights.iter().sum();
            let score: f32 = ranked
                .iter()
                .zip(weights.iter())
                .map(|((_, s), w)| *s * w)
                .sum::<f32>()
                / w_sum;
            let winner = ranked.first().map(|(e, _)| *e);
            (score, winner)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{CenterPolicy, ScalarConsideration, SpatialConsideration};
    use crate::ai::curves::Curve;
    use crate::ai::dse::{ActivityKind, CommitmentStrategy, Termination};
    use crate::components::physical::Position;

    fn linear_identity() -> Curve {
        Curve::Linear {
            slope: 1.0,
            intercept: 0.0,
        }
    }

    fn noop_intention(_target: Entity) -> Intention {
        Intention::Activity {
            kind: ActivityKind::Socialize,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        }
    }

    fn noop_candidate_query(_cat: Entity) -> &'static str {
        "stub"
    }

    fn test_ctx(entity: Entity) -> EvalCtx<'static> {
        static MARKER: fn(&str, Entity) -> bool = |_, _| false;
        static SAMPLE: fn(&str, Position) -> f32 = |_, _| 0.0;
        EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &SAMPLE,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        }
    }

    #[test]
    fn empty_candidates_return_zero_score_no_intention() {
        let dse = TargetTakingDse {
            id: DseId("socialize"),
            candidate_query: noop_candidate_query,
            per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                "target_fondness",
                linear_identity(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            aggregation: TargetAggregation::Best,
            intention: noop_intention,
        };
        let cat = Entity::from_raw_u32(1).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |_: &str, _: Entity, _: Entity| 0.0;
        let out = evaluate_target_taking(&dse, cat, &[], &[], &ctx, &fetch_self, &fetch_target);
        assert_eq!(out.aggregated_score, 0.0);
        assert!(out.winning_target.is_none());
        assert!(out.intention.is_none());
    }

    #[test]
    fn best_aggregation_picks_argmax() {
        let dse = TargetTakingDse {
            id: DseId("socialize"),
            candidate_query: noop_candidate_query,
            per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                "target_fondness",
                linear_identity(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            aggregation: TargetAggregation::Best,
            intention: noop_intention,
        };
        let cat = Entity::from_raw_u32(1).unwrap();
        let a = Entity::from_raw_u32(10).unwrap();
        let b = Entity::from_raw_u32(11).unwrap();
        let c = Entity::from_raw_u32(12).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |name: &str, _cat: Entity, target: Entity| match (name, target) {
            ("target_fondness", t) if t == a => 0.2,
            ("target_fondness", t) if t == b => 0.9,
            ("target_fondness", t) if t == c => 0.5,
            _ => 0.0,
        };
        let positions = vec![
            Position::new(1, 0),
            Position::new(2, 0),
            Position::new(3, 0),
        ];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[a, b, c],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(b));
        assert!((out.aggregated_score - 0.9).abs() < 1e-5);
        assert!(out.intention.is_some());
        // per_target preserves input order.
        assert_eq!(out.per_target[0].0, a);
        assert_eq!(out.per_target[1].0, b);
        assert_eq!(out.per_target[2].0, c);
    }

    #[test]
    fn sum_top_n_aggregates_threat() {
        // Fight-style: three hostiles, threat response sums top 2.
        let dse = TargetTakingDse {
            id: DseId("fight"),
            candidate_query: noop_candidate_query,
            per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                "target_threat",
                linear_identity(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            aggregation: TargetAggregation::SumTopN(2),
            intention: noop_intention,
        };
        let cat = Entity::from_raw_u32(1).unwrap();
        let a = Entity::from_raw_u32(10).unwrap();
        let b = Entity::from_raw_u32(11).unwrap();
        let c = Entity::from_raw_u32(12).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |name: &str, _: Entity, target: Entity| match (name, target) {
            ("target_threat", t) if t == a => 0.6,
            ("target_threat", t) if t == b => 0.4,
            ("target_threat", t) if t == c => 0.3,
            _ => 0.0,
        };
        let positions = vec![Position::new(1, 0); 3];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[a, b, c],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        // Top 2: 0.6 + 0.4 = 1.0; winner is the argmax (a).
        assert!(
            (out.aggregated_score - 1.0).abs() < 1e-5,
            "got {}",
            out.aggregated_score
        );
        assert_eq!(out.winning_target, Some(a));
    }

    #[test]
    fn weighted_average_decays_by_rank() {
        let dse = TargetTakingDse {
            id: DseId("mentor"),
            candidate_query: noop_candidate_query,
            per_target_considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                "target_gap",
                linear_identity(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            aggregation: TargetAggregation::WeightedAverage,
            intention: noop_intention,
        };
        let cat = Entity::from_raw_u32(1).unwrap();
        let a = Entity::from_raw_u32(10).unwrap();
        let b = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |name: &str, _: Entity, target: Entity| match (name, target) {
            ("target_gap", t) if t == a => 1.0,
            ("target_gap", t) if t == b => 0.5,
            _ => 0.0,
        };
        let positions = vec![Position::new(1, 0); 2];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[a, b],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        // Ranked: [(a, 1.0), (b, 0.5)]; weights [1, 0.5]; norm 1.5.
        // score = (1.0 × 1 + 0.5 × 0.5) / 1.5 = 1.25 / 1.5 ≈ 0.833
        assert!(
            (out.aggregated_score - (1.25 / 1.5)).abs() < 1e-4,
            "got {}",
            out.aggregated_score
        );
        assert_eq!(out.winning_target, Some(a));
    }

    #[test]
    fn spatial_consideration_uses_candidate_position() {
        // Spatial consideration with `TargetPosition` center must sample
        // at each candidate's tile, not the scoring cat's position.
        let dse = TargetTakingDse {
            id: DseId("hunt"),
            candidate_query: noop_candidate_query,
            per_target_considerations: vec![Consideration::Spatial(SpatialConsideration::new(
                "prey_map",
                "prey_map",
                CenterPolicy::TargetPosition,
                linear_identity(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            aggregation: TargetAggregation::Best,
            intention: noop_intention,
        };
        let cat = Entity::from_raw_u32(1).unwrap();
        let a = Entity::from_raw_u32(10).unwrap();
        let b = Entity::from_raw_u32(11).unwrap();
        let pos_a = Position::new(5, 0);
        let pos_b = Position::new(10, 0);
        let sample = move |key: &str, pos: Position| -> f32 {
            if key == "prey_map" {
                // Return a value that depends on position to verify
                // per-candidate sampling.
                pos.x as f32 / 10.0
            } else {
                0.0
            }
        };
        let has_marker = |_: &str, _: Entity| false;
        let ctx = EvalCtx {
            cat,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |_: &str, _: Entity, _: Entity| 0.0;
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[a, b],
            &[pos_a, pos_b],
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        // b at x=10 → 1.0, a at x=5 → 0.5. Winner = b.
        assert_eq!(out.winning_target, Some(b));
        assert!((out.aggregated_score - 1.0).abs() < 1e-5);
    }

    #[test]
    fn ranked_candidates_sorts_descending() {
        let scored = ScoredTargetTakingDse {
            id: DseId("socialize"),
            per_target: vec![
                (Entity::from_raw_u32(1).unwrap(), 0.3),
                (Entity::from_raw_u32(2).unwrap(), 0.9),
                (Entity::from_raw_u32(3).unwrap(), 0.6),
            ],
            winning_target: Some(Entity::from_raw_u32(2).unwrap()),
            aggregated_score: 0.9,
            intention: None,
        };
        let ranked = scored.ranked_candidates();
        assert!((ranked[0].1 - 0.9).abs() < 1e-5);
        assert!((ranked[1].1 - 0.6).abs() < 1e-5);
        assert!((ranked[2].1 - 0.3).abs() < 1e-5);
    }
}
