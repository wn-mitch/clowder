//! `SocializeTargetDse` — §6.5.1 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning partner selection for `Socialize`. Pairs
//! with the self-state [`SocializeDse`](super::socialize::SocializeDse)
//! which owns the *desire* to socialize; this DSE owns *with whom*.
//!
//! Phase 4c.1 scope (silent-divergence-fix only):
//!
//! - The evaluator produces a `winning_target` that both
//!   `disposition.rs::build_socializing_chain` and
//!   `goap.rs::SocializeWith` consume. Replaces the divergent pickers
//!   at `disposition.rs:1348–1365` (fondness + novelty weighted sum)
//!   and `goap.rs:find_social_target` (fondness only).
//! - The `aggregated_score` is observable but does *not* modulate the
//!   `Action::Socialize` pool entry in this phase. The existing
//!   self-state [`SocializeDse`] continues to drive action selection;
//!   target-quality merging into the action pool is deferred until the
//!   scoring substrate's other §6 ports stabilize.
//!
//! Five per-target considerations (§6.5.1 + ticket 027 Bug 3 partner-bond
//! bias). The distance axis lands as a `SpatialConsideration` per the
//! §L2.10.7 plan-cost feedback design (ticket 052) — Manhattan
//! distance to the partner flows through `Quadratic(exp=2,
//! divisor=-1, shift=1)` over normalized cost, evaluating
//! `(1 - cost)²` and exactly preserving the legacy `nearness²` shape
//! (same explicit-inversion idiom as Mentor and ApplyRemedy ports):
//!
//! | # | Consideration          | Source                  | Curve                                | Weight |
//! |---|------------------------|-------------------------|--------------------------------------|--------|
//! | 1 | distance               | `Spatial(target)`       | `Quadratic(exp=2, div=-1, shift=1)`  | 0.20   |
//! | 2 | fondness               | `target_fondness`       | `Linear(1, 0)`                       | 0.28   |
//! | 3 | novelty (1-familiarity)| `target_novelty`        | `Linear(1, 0)`                       | 0.20   |
//! | 4 | species-compat         | `target_species_compat` | `Cliff(threshold=0.5)`               | 0.12   |
//! | 5 | partner bond           | `target_partner_bond`   | `Linear(1, 0)`                       | 0.20   |
//!
//! Novelty is stored pre-inverted (`1 - familiarity`) rather than via
//! a `Linear(-1, 1)` curve so the fetcher is the single source of
//! "novelty signal", matching the spec's naming of `target_novelty`
//! as a first-class axis.
//!
//! **Partner-bond axis** (ticket 027 Bug 3 partial implementation).
//! Maps `Relationships::get(cat, target).bond` to a graduated scalar
//! — `None`: 0.0, `Friends`: 0.5, `Partners`/`Mates`: 1.0. Biases
//! Socialize's target picker toward a bonded partner so repeated
//! socialization concentrates with the same cat, accelerating
//! fondness and familiarity past the Partners-bond gate that the
//! courtship-drift loop in `social.rs::check_bonds` cannot cross on
//! its own (only `romantic` accumulates passively). The original
//! four weights renormalized by ×0.8 to make room. The full L2
//! `PairingActivity` self-state DSE per §7.M is deferred — see
//! ticket 027 §"Out of scope".

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkSource, ScalarConsideration, SpatialConsideration, LandmarkAnchor};
use crate::ai::curves::Curve;
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::faction::StanceRequirement;
use crate::ai::planner::GoapActionKind;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::components::RecentTargetFailures;
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::systems::plan_substrate::{
    cooldown_curve, target_recent_failure_age_normalized, TARGET_RECENT_FAILURE_INPUT,
};

pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
pub const TARGET_NOVELTY_INPUT: &str = "target_novelty";
pub const TARGET_SPECIES_COMPAT_INPUT: &str = "target_species_compat";
pub const TARGET_PARTNER_BOND_INPUT: &str = "target_partner_bond";

/// Candidate-pool range in Manhattan tiles. Matches the existing
/// `DispositionConstants::social_target_range` outer-gate semantic —
/// changing it would shift the candidate population and is a balance
/// decision deferred to post-refactor per open-work #14.
pub const SOCIALIZE_TARGET_RANGE: f32 = 10.0;

/// §6.5.1 `Socialize` target-taking DSE factory. Produces a
/// [`TargetTakingDse`] consumable by `add_target_taking_dse`.
pub fn socialize_target_dse() -> TargetTakingDse {
    // §L2.10.7 distance axis: `Quadratic(exp=2, divisor=-1, shift=1)`
    // evaluates `(1 - cost)²`, preserving the legacy `nearness²`
    // shape — same explicit-inversion idiom as Mentor and ApplyRemedy
    // ports. Sharp falloff near the cat: at half-range score = 0.25.
    let nearness_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: -1.0,
        shift: 1.0,
    };
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // Cliff at 0.5 — species-compat scored as 1.0 above threshold, 0.0
    // below. Today only cat-on-cat socializing exists (the caller's
    // candidate filter already enforces species); under future visitor
    // / cross-species mechanics the fetcher returns the compatibility
    // class and the cliff gates it.
    let species_cliff = Curve::Piecewise {
        knots: vec![(0.0, 0.0), (0.499, 0.0), (0.5, 1.0), (1.0, 1.0)],
    };

    TargetTakingDse {
        id: DseId("socialize_target"),
        candidate_query: socialize_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Spatial(SpatialConsideration::new(
                "socialize_target_nearness",
                LandmarkSource::TargetPosition,
                SOCIALIZE_TARGET_RANGE,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_FONDNESS_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_NOVELTY_INPUT,
                linear.clone(),
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_SPECIES_COMPAT_INPUT,
                species_cliff,
            )),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_PARTNER_BOND_INPUT,
                linear,
            )),
            // Ticket 073 — recently-failed target cooldown. `Piecewise`
            // 0.0 → 0.1, 1.0 → 1.0 multiplies a fresh-failure
            // candidate's weighted-sum contribution down to ~10% of
            // its no-failure value. Existing five weights renormalized
            // by ×(5/6) so steady-state scores match pre-073 on cats
            // with no recent failures (the 1.0 sensor signal nulls the
            // axis to its full 1.0 contribution).
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_RECENT_FAILURE_INPUT,
                cooldown_curve(),
            )),
        ],
        // WeightedSum matches the pre-refactor resolver's linear mixer
        // (`fondness × w1 + (1 - familiarity) × w2`). CompensatedProduct
        // would gate any low axis (a 0.0 novelty nulls the candidate
        // entirely) which over-punishes familiar-but-beloved partners.
        // Original five weights (0.20/0.28/0.20/0.12/0.20) renormalized
        // by ×(5/6) to make room for the 073 cooldown axis at 1/6 ≈
        // 0.1667. Sums to 1.0 within fp tolerance.
        composition: Composition::weighted_sum(vec![
            0.20 * 5.0 / 6.0,
            0.28 * 5.0 / 6.0,
            0.20 * 5.0 / 6.0,
            0.12 * 5.0 / 6.0,
            0.20 * 5.0 / 6.0,
            1.0 / 6.0,
        ]),
        aggregation: TargetAggregation::Best,
        intention: socialize_intention,
        // §9.3 Socialize accepts `Same | Ally`. Migrated from the
        // cat-action SocializeDse (where it was metadata-only) — the
        // candidate-prefilter happens here before evaluate_target_taking.
        required_stance: Some(StanceRequirement::socialize()),
        // Ticket 074 — gate dead/banished/incapacitated candidates.
        // 080's reservation gate is intentionally not applied here:
        // multiple cats can socialize at the same partner.
        eligibility: crate::systems::plan_substrate::require_alive_filter(),
    }
}

/// Documentation-only candidate-query stub per §6.3 (the runtime
/// evaluator takes candidates as a direct argument; this fn-pointer
/// names where the caller's `SystemParam` bundle pulls them from).
fn socialize_candidate_query_doc(_cat: bevy::prelude::Entity) -> &'static str {
    "cats within social_target_range, excluding self, filtered by faction Same|Ally"
}

/// Intention factory threading the winning target forward. §L2.10
/// activity per `ActivityKind::Socialize`. The target entity rides on
/// the ScoredTargetTakingDse's `winning_target` field — this factory
/// produces the Intention shape that the commitment layer consumes;
/// the target itself is carried alongside.
fn socialize_intention(_target: Entity) -> Intention {
    Intention::Activity {
        kind: ActivityKind::Socialize,
        termination: Termination::UntilInterrupt,
        strategy: CommitmentStrategy::OpenMinded,
    }
}

/// Map `Relationships::get(cat, target).bond` to a graduated scalar,
/// with the L2 PairingActivity Intention partner pinned at 1.0.
///
/// **Without an active Intention** — `None` → 0.0 (unbonded
/// acquaintance), `Friends` → 0.5 (significant boost — the
/// courtship-drift loop in `social.rs::check_bonds` uses this tier
/// as the romantic-accumulation gate), `Partners` / `Mates` → 1.0.
/// Graduated rather than Cliff so a Mates-bonded partner outscores a
/// Friends-bonded one.
///
/// **With an active Intention** (§7.M, ticket 027b Commit B) — if
/// `pairing_partner == Some(target)`, the candidate is pinned at 1.0
/// regardless of bond tier. This is the structural-commitment
/// override: a cat that holds a Pairing Intention with a Friends-
/// bonded peer should preferentially socialize with *that* cat across
/// ticks, not drift toward a Partners-bonded acquaintance whose bond
/// tier is already maxed out. The pin only changes selection when the
/// underlying tier is Friends (or absent — defensive); a cat already
/// at Partners/Mates gets the same 1.0 either way, so the Intention
/// is a no-op there.
fn bond_score(
    relationships: &Relationships,
    pairing_partner: Option<Entity>,
    cat: Entity,
    target: Entity,
) -> f32 {
    // IAUS-COHERENCE-EXEMPT: 027b Commit B's MacGyvered Pairing-Intention pin; ticket 078 backports to a target_pairing_intention Consideration and removes this marker.
    if pairing_partner == Some(target) {
        return 1.0;
    }
    match relationships.get(cat, target).and_then(|r| r.bond) {
        Some(BondType::Mates) | Some(BondType::Partners) => 1.0,
        Some(BondType::Friends) => 0.5,
        None => 0.0,
    }
}

/// `bond_score` *without* the L2 Intention pin — the score the
/// candidate would have earned absent the Pairing. Used by the call
/// site to decide whether `Feature::PairingBiasApplied` should fire
/// (the bias is "load-bearing" only when the pin actually changed the
/// scalar — i.e., the underlying tier was Friends or absent).
pub fn unpinned_bond_score(
    relationships: &Relationships,
    cat: Entity,
    target: Entity,
) -> f32 {
    bond_score(relationships, None, cat, target)
}

// ---------------------------------------------------------------------------
// Caller-side resolver — the single entry point both disposition.rs and
// goap.rs invoke to pick a social partner. Retires the divergent pickers
// at `disposition.rs:1348` (weighted) and `goap.rs:find_social_target`
// (fondness-only) per §6.2.
// ---------------------------------------------------------------------------

/// Pick the best social partner for `cat` via the registered
/// [`socialize_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
///
/// `cat_positions` must be the frame-local snapshot of `(Entity,
/// Position)` for alive cats that disposition.rs / goap.rs already
/// build for their scoring ticks — this helper does not query the
/// world directly.
///
/// Per §6.2 this is the **only** sanctioned target-picker for
/// Socialize. Callers that currently call `find_social_target` or
/// `build_socializing_chain`'s inline picker must switch to this
/// helper; the legacy helpers stay behind for `GroomOther` /
/// `MentorCat` / `MateWith` only until their §6.5.2–§6.5.4 ports land.
#[allow(clippy::too_many_arguments)]
pub fn resolve_socialize_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    relations: &crate::ai::faction::FactionRelations,
    stance_overlays: &dyn Fn(Entity) -> crate::ai::faction::StanceOverlays,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
    // L2 PairingActivity partner (ticket 027b Commit B). `None` for
    // cats without an Intention; when `Some(p)`, the
    // `target_partner_bond` axis pins `p` at 1.0 via `bond_score`.
    pairing_partner: Option<Entity>,
    // Ticket 073 — per-cat recently-failed target memory. `None` for
    // cats without the component (lazy-inserted on first failure);
    // when `Some`, the `TARGET_RECENT_FAILURE_INPUT` axis penalizes
    // recently-failed candidates via the cooldown curve. The cooldown
    // window is `cooldown_ticks` (caller pulls
    // `SimConstants::planning_substrate::target_failure_cooldown_ticks`).
    recent: Option<&RecentTargetFailures>,
    cooldown_ticks: u64,
    // Activation tracker for `Feature::PairingBiasApplied` and
    // `Feature::TargetCooldownApplied`. Fires when the picked target
    // == `pairing_partner` and the unpinned bond score would have
    // been < 1.0; or when at least one candidate's cooldown signal
    // was < 1.0 this resolver call. Pass `None` from dead-code
    // disposition.rs paths and from tests that don't care.
    activation: Option<&mut SystemActivation>,
) -> Option<Entity> {
    // Find the registered factory output. Fall back to `None` if
    // registration was skipped (tests / partial bootstraps) — callers
    // interpret that as "no target picked" and the outer eligibility
    // gate short-circuits the action.
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "socialize_target")?;

    // Gather candidates + positions, excluding self and out-of-range
    // peers. `SOCIALIZE_TARGET_RANGE` matches today's
    // `DispositionConstants::social_target_range` (10) to preserve
    // outer-gate semantics; per-target distance attenuation happens
    // inside the DSE via the `socialize_target_nearness`
    // SpatialConsideration (§L2.10.7).
    let mut candidates: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos) as f32;
        if dist <= SOCIALIZE_TARGET_RANGE {
            candidates.push(*other);
            positions.push(*other_pos);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // §9.3 stance prefilter — drop candidates whose resolved stance
    // fails the DSE requirement. Socialize candidates are cats by
    // construction (cat_positions filter), so the species lookup is
    // a constant `Cat`.
    if let Some(req) = dse.required_stance() {
        let species_of = |_: Entity| Some(crate::ai::faction::FactionSpecies::Cat);
        let (filtered, filtered_pos) = crate::ai::faction::filter_candidates_by_stance(
            relations,
            crate::ai::faction::FactionSpecies::Cat,
            &candidates,
            &positions,
            &species_of,
            stance_overlays,
            req,
        );
        if filtered.is_empty() {
            return None;
        }
        candidates = filtered;
        positions = filtered_pos;
    }

    // Ticket 073 — track whether any candidate triggered the cooldown
    // penalty this call (signal < 1.0). Recorded against
    // `Feature::TargetCooldownApplied` after the resolver finishes if
    // the activation tracker is present. Cell so the closure can
    // mutate it without taking `&mut`.
    let cooldown_was_applied = std::cell::Cell::new(false);

    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_NOVELTY_INPUT => {
                let familiarity = relationships
                    .get(cat, target)
                    .map(|r| r.familiarity)
                    .unwrap_or(0.0);
                1.0 - familiarity
            }
            // Cat-on-cat social — candidate filter keeps this true today.
            // Future cross-species socializing routes a compatibility
            // class here; curve-cliff gates on ≥ 0.5.
            TARGET_SPECIES_COMPAT_INPUT => 1.0,
            TARGET_PARTNER_BOND_INPUT => bond_score(relationships, pairing_partner, cat, target),
            TARGET_RECENT_FAILURE_INPUT => {
                let signal = target_recent_failure_age_normalized(
                    recent,
                    GoapActionKind::SocializeWith,
                    target,
                    tick,
                    cooldown_ticks,
                );
                if signal < 1.0 {
                    cooldown_was_applied.set(true);
                }
                signal
            }
            _ => 0.0,
        }
    };

    // `entity_position` / `has_marker` are unused by Socialize's four
    // scalar considerations but required by `EvalCtx`. Stub with no-op
    // closures.
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
                .set_target_ranking("socialize_target", ranking, tick);
        }
    }

    if let Some(act) = activation {
        // Ticket 027b §7.M — fire `PairingBiasApplied` only when the
        // bias was *load-bearing*: the picked target is the Intention
        // partner AND that partner's unpinned bond score was < 1.0
        // (i.e., they were Friends-bonded or unbonded; a Partners/
        // Mates partner gets 1.0 with or without the pin, so the bias
        // didn't change anything).
        if let (Some(picked), Some(partner)) = (scored.winning_target, pairing_partner) {
            if picked == partner && unpinned_bond_score(relationships, cat, partner) < 1.0 {
                act.record(Feature::PairingBiasApplied);
            }
        }
        // Ticket 073 — fire `TargetCooldownApplied` once per resolver
        // call where at least one candidate's cooldown signal was
        // < 1.0 (i.e., the cooldown axis actually penalized somebody).
        if cooldown_was_applied.get() {
            act.record(Feature::TargetCooldownApplied);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::dse::EvalCtx;
    use crate::ai::target_dse::evaluate_target_taking;
    use crate::components::physical::Position;
    use bevy::prelude::Entity;

    fn test_ctx(entity: Entity) -> EvalCtx<'static> {
        static MARKER: fn(&str, Entity) -> bool = |_, _| false;
        static NO_ENTITY_POS: fn(Entity) -> Option<Position> = |_| None;
        static NO_ANCHOR_POS: fn(LandmarkAnchor) -> Option<Position> = |_| None;
        EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &NO_ENTITY_POS,
            anchor_position: &NO_ANCHOR_POS,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        }
    }

    #[test]
    fn socialize_target_dse_id_stable() {
        assert_eq!(socialize_target_dse().id().0, "socialize_target");
    }

    #[test]
    fn socialize_target_dse_has_six_axes() {
        // Ticket 073 — five legacy axes + the cooldown axis = six.
        assert_eq!(socialize_target_dse().per_target_considerations().len(), 6);
    }

    #[test]
    fn socialize_target_weights_sum_to_one() {
        let sum: f32 = socialize_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn renormalization_preserves_no_failure_steady_state() {
        // Ticket 073 acceptance gate: each renormalized DSE's no-failure
        // steady-state score equals its pre-073 score within fp tolerance.
        // Construction: with the cooldown signal pinned at 1.0 (no
        // recent failure), the WeightedSum should equal the pre-073
        // weighted sum with the original 5 weights × 5/6 + the cooldown
        // weight × 1.0 = exactly the original sum × 5/6 + 1/6.
        // Because the original five weights summed to 1.0, the
        // post-073 sum at signal=1.0 also sums to 1.0 — which is what
        // "no behavioral change at steady state" looks like.
        let dse = socialize_target_dse();
        let weights = &dse.composition().weights;
        // Pre-073 weights renormalized × 5/6, plus the cooldown axis
        // weight 1/6.
        let pre_073 = [0.20_f32, 0.28, 0.20, 0.12, 0.20];
        for (i, &pre) in pre_073.iter().enumerate() {
            let expected = pre * 5.0 / 6.0;
            assert!(
                (weights[i] - expected).abs() < 1e-6,
                "axis {} renormalized weight: expected {}, got {}",
                i,
                expected,
                weights[i]
            );
        }
        let cooldown_weight = weights[5];
        assert!(
            (cooldown_weight - 1.0 / 6.0).abs() < 1e-6,
            "cooldown axis weight should be 1/6, got {}",
            cooldown_weight
        );
        // Steady-state score with all axes at 1.0 = sum of weights = 1.0.
        let steady: f32 = weights.iter().sum();
        assert!((steady - 1.0).abs() < 1e-4);
    }

    #[test]
    fn socialize_target_uses_best_aggregation() {
        assert_eq!(
            socialize_target_dse().aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn picks_argmax_when_fondness_dominates() {
        // Two equally near, equally novel, equally cat candidates —
        // fondness decides the winner.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(10).unwrap();
        let acquaintance = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => {
                    if target == friend {
                        0.9
                    } else {
                        0.4
                    }
                }
                TARGET_NOVELTY_INPUT => 0.5,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(2, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[friend, acquaintance],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(friend));
    }

    #[test]
    fn retires_silent_divergence_vs_fondness_only() {
        // Legacy `find_social_target` picks by fondness alone. When
        // fondness is tied but novelty differs, the legacy path is
        // undefined (tie-break by iter order); the DSE picks the
        // novel partner, breaking the tie deterministically.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let novel_stranger = Entity::from_raw_u32(10).unwrap();
        let familiar_friend = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => 0.5, // tied
                TARGET_NOVELTY_INPUT => {
                    if target == novel_stranger {
                        0.9
                    } else {
                        0.1
                    }
                }
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(1, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[novel_stranger, familiar_friend],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(novel_stranger));
    }

    #[test]
    fn bond_score_maps_bond_tiers_to_graduated_scalar() {
        let cat = Entity::from_raw_u32(1).unwrap();
        let stranger = Entity::from_raw_u32(2).unwrap();
        let friend = Entity::from_raw_u32(3).unwrap();
        let partner = Entity::from_raw_u32(4).unwrap();
        let mate = Entity::from_raw_u32(5).unwrap();
        let mut relationships = Relationships::default();
        // No entry → bond=None → 0.0.
        assert_eq!(bond_score(&relationships, None, cat, stranger), 0.0);
        relationships.get_or_insert(cat, friend).bond = Some(BondType::Friends);
        relationships.get_or_insert(cat, partner).bond = Some(BondType::Partners);
        relationships.get_or_insert(cat, mate).bond = Some(BondType::Mates);
        assert_eq!(bond_score(&relationships, None, cat, friend), 0.5);
        assert_eq!(bond_score(&relationships, None, cat, partner), 1.0);
        assert_eq!(bond_score(&relationships, None, cat, mate), 1.0);
    }

    #[test]
    fn bond_score_pins_pairing_intention_partner_at_one() {
        // Ticket 027b §7.M — the L2 Intention partner is pinned at
        // 1.0 regardless of underlying bond tier (Friends-bonded or
        // even unbonded). This is the structural-commitment override.
        let cat = Entity::from_raw_u32(1).unwrap();
        let intended = Entity::from_raw_u32(2).unwrap();
        let unrelated = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, intended).bond = Some(BondType::Friends);
        // With the Intention: Friends → 1.0 (pinned).
        assert_eq!(
            bond_score(&relationships, Some(intended), cat, intended),
            1.0
        );
        // Without the Intention: Friends → 0.5 (graduated).
        assert_eq!(bond_score(&relationships, None, cat, intended), 0.5);
        // The pin is partner-specific — non-Intention candidates
        // resolve via the graduated map.
        assert_eq!(
            bond_score(&relationships, Some(intended), cat, unrelated),
            0.0
        );
    }

    #[test]
    fn unpinned_bond_score_ignores_intention() {
        // The unpinned helper is the "what would they score absent the
        // pin" reference used by the call site to decide whether
        // PairingBiasApplied is load-bearing.
        let cat = Entity::from_raw_u32(1).unwrap();
        let intended = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, intended).bond = Some(BondType::Friends);
        assert_eq!(unpinned_bond_score(&relationships, cat, intended), 0.5);
    }

    #[test]
    fn picks_friend_over_higher_fondness_unbonded_acquaintance() {
        // Ticket 027 Bug 3 partial — partner-bond bias must beat a
        // small fondness lead from an unbonded peer. Here the friend
        // has 0.4 fondness, the stranger has 0.6, both equally near /
        // novel / cat. Without the bond bias the stranger wins; with
        // the 0.20 weight × 0.5 (Friends scalar) the friend wins.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(10).unwrap();
        let stranger = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => {
                    if target == friend {
                        0.4
                    } else {
                        0.6
                    }
                }
                TARGET_NOVELTY_INPUT => 0.5,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                TARGET_PARTNER_BOND_INPUT => {
                    if target == friend {
                        0.5
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(2, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[friend, stranger],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(friend));
    }

    #[test]
    fn picks_partner_over_friend_when_both_bonded() {
        // Graduated bond scalar (Partners=1.0, Friends=0.5) means the
        // partners-bonded cat outscores the friends-bonded one ceteris
        // paribus — keeps a paired cat oriented toward the deeper bond.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(10).unwrap();
        let partner = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => 0.5,
                TARGET_NOVELTY_INPUT => 0.5,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                TARGET_PARTNER_BOND_INPUT => {
                    if target == partner {
                        1.0
                    } else {
                        0.5
                    }
                }
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(2, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[friend, partner],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(partner));
    }

    #[test]
    fn bond_bias_does_not_dominate_large_fondness_gap() {
        // Sanity: a +0.6 fondness gap on the unbonded candidate beats
        // the 0.20 × 0.5 = 0.10 bond bonus on the Friends-bonded peer.
        // Confirms the bond axis is a bias, not an override — a
        // beloved-but-unbonded peer still wins over a barely-liked
        // friend, matching the spec's "additive bias not gate" intent.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(10).unwrap();
        let beloved_stranger = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => {
                    if target == friend {
                        0.3
                    } else {
                        0.9
                    }
                }
                TARGET_NOVELTY_INPUT => 0.5,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                TARGET_PARTNER_BOND_INPUT => {
                    if target == friend {
                        0.5
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(2, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[friend, beloved_stranger],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(beloved_stranger));
    }

    #[test]
    fn empty_candidates_yield_no_target() {
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |_: &str, _: Entity, _: Entity| 0.0;
        let out = evaluate_target_taking(&dse, cat, &[], &[], &ctx, &fetch_self, &fetch_target);
        assert!(out.winning_target.is_none());
        assert!(out.intention.is_none());
        assert_eq!(out.aggregated_score, 0.0);
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let other = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        let cat_positions = vec![(other, Position::new(1, 0))];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
            None,
            None,
            8000,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_when_no_candidates_in_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = Entity::from_raw_u32(2).unwrap();
        let relationships = Relationships::default();
        // Position well beyond SOCIALIZE_TARGET_RANGE (10).
        let cat_positions = vec![(far, Position::new(50, 0))];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
            None,
            None,
            8000,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_picks_higher_fondness_all_else_equal() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend = Entity::from_raw_u32(2).unwrap();
        let stranger = Entity::from_raw_u32(3).unwrap();

        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, friend).fondness = 0.8;
        relationships.get_or_insert(cat, friend).familiarity = 0.5;
        relationships.get_or_insert(cat, stranger).fondness = 0.1;
        relationships.get_or_insert(cat, stranger).familiarity = 0.5;

        // Both within range, at equal distance.
        let cat_positions = vec![
            (friend, Position::new(3, 0)),
            (stranger, Position::new(3, 1)),
        ];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
            None,
            None,
            8000,
            None,
        );
        assert_eq!(out, Some(friend));
    }

    #[test]
    fn resolver_excludes_self() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        // Only self in the snapshot — must return None.
        let relationships = Relationships::default();
        let cat_positions = vec![(cat, Position::new(0, 0))];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
            None,
            None,
            8000,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_tiebreaks_fondness_ties_on_novelty() {
        // Exact §6.2 silent-divergence scenario: fondness tied across
        // candidates. Legacy `find_social_target` would tie-break by
        // iter order (nondeterministic); the DSE picks the novel
        // stranger deterministically via the novelty axis.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let familiar = Entity::from_raw_u32(2).unwrap();
        let novel = Entity::from_raw_u32(3).unwrap();

        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, familiar).fondness = 0.4;
        relationships.get_or_insert(cat, familiar).familiarity = 0.95;
        relationships.get_or_insert(cat, novel).fondness = 0.4;
        relationships.get_or_insert(cat, novel).familiarity = 0.05;

        let cat_positions = vec![
            (familiar, Position::new(3, 0)),
            (novel, Position::new(3, 1)),
        ];
        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
            None,
            None,
            8000,
            None,
        );
        assert_eq!(out, Some(novel));
    }

    #[test]
    fn intention_is_socialize_activity() {
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let target = Entity::from_raw_u32(10).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = |name: &str, _: Entity, _: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT
                | TARGET_NOVELTY_INPUT
                | TARGET_SPECIES_COMPAT_INPUT => 1.0,
                _ => 0.0,
            }
        };
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[target],
            &[Position::new(1, 0)],
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        match out.intention.expect("intention present for winning target") {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Socialize),
            other => panic!("expected Activity intention, got {other:?}"),
        }
    }

    #[test]
    fn socialize_target_stance_requirement_is_same_or_ally() {
        use crate::ai::faction::FactionStance;
        let req = socialize_target_dse()
            .required_stance
            .expect("§9.3 binding must populate required_stance");
        assert!(req.accepts(FactionStance::Same));
        assert!(req.accepts(FactionStance::Ally));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Predator));
        assert!(!req.accepts(FactionStance::Neutral));
    }

    #[test]
    fn resolver_drops_banished_candidate() {
        // §9.3 + §9.2: a Banished cat in the candidate set should be
        // filtered out before evaluate_target_taking. Resolver picks
        // the non-banished cat even when the banished cat would have
        // scored higher on every other axis.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(socialize_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let banished = Entity::from_raw_u32(2).unwrap();
        let normal = Entity::from_raw_u32(3).unwrap();

        let mut relationships = Relationships::default();
        // Banished cat would otherwise win on fondness.
        relationships.get_or_insert(cat, banished).fondness = 0.9;
        relationships.get_or_insert(cat, banished).familiarity = 0.5;
        relationships.get_or_insert(cat, normal).fondness = 0.3;
        relationships.get_or_insert(cat, normal).familiarity = 0.5;

        let cat_positions = vec![
            (banished, Position::new(2, 0)),
            (normal, Position::new(2, 1)),
        ];
        let stance_overlays = move |e: Entity| {
            if e == banished {
                crate::ai::faction::StanceOverlays {
                    banished: true,
                    ..Default::default()
                }
            } else {
                crate::ai::faction::StanceOverlays::default()
            }
        };

        let out = resolve_socialize_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            &crate::ai::faction::FactionRelations::canonical(),
            &stance_overlays,
            0,
            None,
            None,
            None,
            8000,
            None,
        );
        assert_eq!(
            out,
            Some(normal),
            "banished candidate should be filtered; resolver picks the non-banished cat"
        );
    }
}
