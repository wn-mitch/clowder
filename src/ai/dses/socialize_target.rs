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
//! four weights renormalized by ×0.8 to make room.
//!
//! **Pairing-intention axis** (ticket 078, backport of 027b's
//! `bond_score` pin). When the cat holds a `PairingActivity`
//! Intention, the candidate matching `pairing.partner` scores 1.0 on
//! this axis; everyone else scores 0.0. Replaces the post-IAUS pin
//! that 027b Commit B installed inline in `bond_score` — the
//! Intention partner's lift now flows through the score economy with
//! full traceability instead of branching on a special case in the
//! resolver body. See `docs/open-work/tickets/071-planning-substrate-hardening.md`
//! ("machined gears" doctrine) for the broader sub-epic. The five
//! pre-078 weights are scaled by `0.90` to make room for the new
//! axis at weight `0.10`; non-Intention picks rank identically (a
//! uniform scaling of all axes preserves argmax) so the candidate
//! winner is preserved on every non-Intention scoring tick.

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
/// Re-export of the canonical key from `plan_substrate` so call sites
/// in this module read a single name. Ticket 072 publishes the
/// canonical constant; ticket 078 wires the consideration here.
pub use crate::systems::plan_substrate::PAIRING_INTENTION_INPUT;

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
    // Ticket 078 — `target_pairing_intention` is a binary 0/1 sensor
    // (`1.0` iff the candidate is the cat's `PairingActivity`
    // partner). A Cliff at 0.5 promotes the Intention partner to a
    // full-axis-1.0 contribution and zeros every non-partner. Same
    // shape idiom as `species_cliff`.
    let intention_cliff = Curve::Piecewise {
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
            // candidate's contribution down to ~10% of no-failure value.
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_RECENT_FAILURE_INPUT,
                cooldown_curve(),
            )),
            // Ticket 078 — Pairing Intention coherence. Cliff curve
            // lifts the L2-elected partner's score to 1.0; non-partners
            // see 0.0. Replaces the 027b post-IAUS pin in `bond_score`.
            Consideration::Scalar(ScalarConsideration::new(
                PAIRING_INTENTION_INPUT,
                intention_cliff,
            )),
        ],
        // WeightedSum matches the pre-refactor resolver's linear mixer
        // (`fondness × w1 + (1 - familiarity) × w2`). CompensatedProduct
        // would gate any low axis (a 0.0 novelty nulls the candidate
        // entirely) which over-punishes familiar-but-beloved partners.
        // Original five weights [0.20, 0.28, 0.20, 0.12, 0.20] sum to
        // 1.0. Tickets 073 + 078 add two new axes (cooldown 1/6 ≈
        // 0.1667, pairing intention 0.10), so the originals scale by
        // (1 - 1/6 - 0.10) = 0.7333. Final 7-axis composition sums to
        // 1.0 within fp tolerance.
        composition: Composition::weighted_sum(vec![
            0.20 * 0.7333,
            0.28 * 0.7333,
            0.20 * 0.7333,
            0.12 * 0.7333,
            0.20 * 0.7333,
            1.0 / 6.0,
            0.10,
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

/// Map `Relationships::get(cat, target).bond` to a graduated scalar.
///
/// `None` → 0.0 (unbonded acquaintance), `Friends` → 0.5 (significant
/// boost — the courtship-drift loop in `social.rs::check_bonds` uses
/// this tier as the romantic-accumulation gate), `Partners` / `Mates`
/// → 1.0. Graduated rather than Cliff so a Mates-bonded partner
/// outscores a Friends-bonded one.
///
/// Ticket 078 — the post-IAUS pin that 027b Commit B installed here
/// (returning 1.0 when `pairing_partner == Some(target)`) was
/// backported to the dedicated `target_pairing_intention` axis on
/// `socialize_target_dse`. This function is now a pure tier→scalar
/// map again, with no knowledge of the Intention layer; the
/// Pairing-Intention lift flows through the IAUS score economy.
fn bond_score(relationships: &Relationships, cat: Entity, target: Entity) -> f32 {
    match relationships.get(cat, target).and_then(|r| r.bond) {
        Some(BondType::Mates) | Some(BondType::Partners) => 1.0,
        Some(BondType::Friends) => 0.5,
        None => 0.0,
    }
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
    // Sensor input for the `target_pairing_intention` IAUS axis
    // (ticket 078). `Some(p)` when the cat holds a `PairingActivity`
    // Intention with partner `p`; `None` for cats without an
    // Intention. Read by the per-target fetcher to produce a binary
    // 0/1 input that the cliff curve in `socialize_target_dse`
    // promotes into a flat 0.10 lift on the IAUS pick. Replaces
    // 027b Commit B's post-IAUS bond pin — the lift now flows
    // through the score economy with full traceability.
    pairing_partner: Option<Entity>,
    // Ticket 073 — per-cat recently-failed target memory. `None` for
    // cats without the component (lazy-inserted on first failure);
    // when `Some`, the `TARGET_RECENT_FAILURE_INPUT` axis penalizes
    // recently-failed candidates via the cooldown curve.
    recent: Option<&RecentTargetFailures>,
    cooldown_ticks: u64,
    // Activation tracker for `Feature::PairingBiasApplied` and
    // `Feature::TargetCooldownApplied`. Fires when the IAUS pick ==
    // `pairing_partner` AND the underlying bond score was < 1.0
    // (post-078, the `target_pairing_intention` axis was load-bearing);
    // or when at least one candidate's cooldown signal was < 1.0.
    // Pass `None` from dead-code disposition.rs paths and from tests
    // that don't care.
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
            TARGET_PARTNER_BOND_INPUT => bond_score(relationships, cat, target),
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
            // Ticket 078 — `target_pairing_intention` sensor:
            // `1.0` iff `target` is the cat's `PairingActivity`
            // partner; `0.0` otherwise. Backports 027b's `bond_score`
            // pin from a post-IAUS branch into a first-class IAUS
            // axis with a cliff curve.
            PAIRING_INTENTION_INPUT => {
                if pairing_partner == Some(target) {
                    1.0
                } else {
                    0.0
                }
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
        // Ticket 027b §7.M / 078 — fire `PairingBiasApplied` only when
        // the new `target_pairing_intention` axis was *load-bearing*:
        // the picked target is the Intention partner AND that partner's
        // bond score (now pure post-078) is < 1.0 (Friends-bonded or
        // unbonded; Partners/Mates get 1.0 with or without the Intention
        // lift). This is the same observability gate as the pre-078 pin.
        if let (Some(picked), Some(partner)) = (scored.winning_target, pairing_partner) {
            if picked == partner && bond_score(relationships, cat, partner) < 1.0 {
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
    fn socialize_target_dse_has_seven_axes() {
        // Tickets 073 + 078 — five legacy axes + cooldown (073) +
        // pairing intention (078) = seven.
        assert_eq!(socialize_target_dse().per_target_considerations().len(), 7);
    }

    #[test]
    fn socialize_target_weights_sum_to_one() {
        let sum: f32 = socialize_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn renormalization_preserves_no_failure_steady_state() {
        // Tickets 073 + 078 combined renormalization. The original
        // 5 axes summed to 1.0; we now have 7 axes total — 5 originals
        // scaled by (1 - 1/6 - 0.10) = 0.7333, the 073 cooldown at 1/6
        // (≈0.1667), and the 078 pairing intention at 0.10. Sum = 1.0.
        // At steady state (cooldown signal = 1.0, no Intention partner =
        // intention signal 0.0), the contribution is original weights
        // × 0.7333 × axis_score + 1/6 × 1.0 + 0.10 × 0.0 — i.e., a
        // shrunken fraction of the pre-073 score plus a constant 1/6.
        // The renormalization just shifts the dynamic range; argmax is
        // preserved (verified by the orthogonal pick-stability tests).
        let dse = socialize_target_dse();
        let weights = &dse.composition().weights;
        let pre_073 = [0.20_f32, 0.28, 0.20, 0.12, 0.20];
        for (i, &pre) in pre_073.iter().enumerate() {
            let expected = pre * 0.7333;
            assert!(
                (weights[i] - expected).abs() < 1e-3,
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
        let intention_weight = weights[6];
        assert!(
            (intention_weight - 0.10).abs() < 1e-6,
            "pairing-intention axis weight should be 0.10, got {}",
            intention_weight
        );
        // All 7 weights sum to ~1.0.
        let total: f32 = weights.iter().sum();
        assert!((total - 1.0).abs() < 1e-3);
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
        assert_eq!(bond_score(&relationships, cat, stranger), 0.0);
        relationships.get_or_insert(cat, friend).bond = Some(BondType::Friends);
        relationships.get_or_insert(cat, partner).bond = Some(BondType::Partners);
        relationships.get_or_insert(cat, mate).bond = Some(BondType::Mates);
        assert_eq!(bond_score(&relationships, cat, friend), 0.5);
        assert_eq!(bond_score(&relationships, cat, partner), 1.0);
        assert_eq!(bond_score(&relationships, cat, mate), 1.0);
    }

    #[test]
    fn pairing_intention_consideration_lifts_partner_via_axis() {
        // Ticket 078 — backport of 027b's `bond_score` pin. The
        // `target_pairing_intention` axis (cliff at 0.5, weight 0.10)
        // adds a flat 0.10 lift to the IAUS score of the cat's
        // `PairingActivity` partner. With Friends-bonded fondness/
        // novelty held equal between two candidates, the Intention
        // partner wins where the bond axis alone would have tied.
        //
        // Replaces `bond_score_pins_pairing_intention_partner_at_one`
        // — the pre-078 test asserted the pin's literal `1.0` return
        // value; post-078 we assert the consideration's contribution
        // through the score economy. The mechanism is preserved
        // (Intention partner wins ceteris paribus) but expressed as
        // an IAUS axis with full traceability.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let intended = Entity::from_raw_u32(10).unwrap();
        let other_friend = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => 0.5,
                TARGET_NOVELTY_INPUT => 0.5,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                // Both candidates are Friends-bonded — the bond axis
                // is tied at 0.5 across them.
                TARGET_PARTNER_BOND_INPUT => 0.5,
                // Only the Intention partner reads 1.0 on the new axis.
                PAIRING_INTENTION_INPUT => {
                    if target == intended {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            }
        };
        let positions = vec![Position::new(1, 0), Position::new(1, 0)];
        let out = evaluate_target_taking(
            &dse,
            cat,
            &[intended, other_friend],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_target,
        );
        assert_eq!(out.winning_target, Some(intended));
        // Score lift is `weight × cliff(1.0) = 0.10 × 1.0 = 0.10`.
        // Verify the axis is exactly load-bearing here: the
        // Intention partner's per-target score must exceed the
        // other Friends-bonded peer by exactly the new axis's
        // weighted contribution.
        let intended_score = out
            .per_target
            .iter()
            .find_map(|(e, s)| if *e == intended { Some(*s) } else { None })
            .expect("intended must be scored");
        let other_score = out
            .per_target
            .iter()
            .find_map(|(e, s)| if *e == other_friend { Some(*s) } else { None })
            .expect("other_friend must be scored");
        assert!(
            (intended_score - other_score - 0.10).abs() < 1e-6,
            "Intention partner lift must equal exactly 0.10 (axis weight × cliff(1.0)); \
             got intended={intended_score:.6}, other={other_score:.6}, \
             delta={:.6}",
            intended_score - other_score,
        );
    }

    #[test]
    fn pairing_intention_axis_matches_legacy_pin_lift_on_friends_partner() {
        // Ticket 078 — regression-on-purpose. The pre-078 pin
        // returned `1.0` from `bond_score` when `pairing_partner ==
        // Some(target)`, regardless of bond tier. For a Friends-
        // bonded Intention partner the pin's load-bearing lift was
        // exactly `partner_bond_weight × (1.0 - 0.5) = 0.20 × 0.5
        // = 0.10` against the same cat scored as a non-Intention
        // peer.
        //
        // The post-078 IAUS path produces the same delta via the new
        // `target_pairing_intention` axis (weight 0.10, cliff curve):
        // `intention_axis_weight × 1.0 = 0.10 × 1.0 = 0.10`. This
        // test asserts the lift matches within fp tolerance. The
        // five pre-078 axis weights are scaled by 0.90 to make room
        // for the new axis, so absolute scores shift uniformly; the
        // *delta* between Intention and non-Intention scores for the
        // same Friends-bonded peer is what the pin governed and is
        // what we preserve.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let intended = Entity::from_raw_u32(10).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;

        // Same fondness / novelty / species / Friends-bond state for
        // both runs — the only difference is whether the intention
        // axis returns 1.0 or 0.0 for the candidate.
        let fetch_with_intention = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => 0.6,
                TARGET_NOVELTY_INPUT => 0.4,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                TARGET_PARTNER_BOND_INPUT => 0.5,
                PAIRING_INTENTION_INPUT => {
                    if target == intended {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            }
        };
        let fetch_without_intention = move |name: &str, _: Entity, _: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => 0.6,
                TARGET_NOVELTY_INPUT => 0.4,
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                TARGET_PARTNER_BOND_INPUT => 0.5,
                // No Intention → axis stays 0 for everyone.
                PAIRING_INTENTION_INPUT => 0.0,
                _ => 0.0,
            }
        };

        let positions = vec![Position::new(1, 0)];
        let with_out = evaluate_target_taking(
            &dse,
            cat,
            &[intended],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_with_intention,
        );
        let without_out = evaluate_target_taking(
            &dse,
            cat,
            &[intended],
            &positions,
            &ctx,
            &fetch_self,
            &fetch_without_intention,
        );

        let with_score = with_out
            .per_target
            .first()
            .expect("intended scored")
            .1;
        let without_score = without_out
            .per_target
            .first()
            .expect("intended scored")
            .1;
        let delta = with_score - without_score;
        // The pin's load-bearing lift on a Friends-bonded Intention
        // partner was exactly 0.10; the new axis must produce the
        // same delta within fp tolerance.
        assert!(
            (delta - 0.10).abs() < 1e-6,
            "post-078 Intention lift must equal pre-078 pin lift (0.10) on a \
             Friends-bonded partner; got delta={delta:.9}"
        );
    }

    #[test]
    fn non_intention_pick_unchanged_by_axis_addition() {
        // Ticket 078 — the five pre-078 axes are uniformly scaled by
        // 0.90 to make room for the new `target_pairing_intention`
        // axis at weight 0.10. Uniform scaling preserves argmax
        // across candidates — non-Intention scoring ticks pick the
        // exact same winner as pre-078. This test exercises the §6.2
        // fondness-tied-novelty-different scenario (same as
        // `retires_silent_divergence_vs_fondness_only`) with the
        // intention axis stuck at 0 across all candidates: the
        // novel-stranger pick must hold.
        let dse = socialize_target_dse();
        let cat = Entity::from_raw_u32(1).unwrap();
        let novel_stranger = Entity::from_raw_u32(10).unwrap();
        let familiar_friend = Entity::from_raw_u32(11).unwrap();
        let ctx = test_ctx(cat);
        let fetch_self = |_: &str, _: Entity| 0.0;
        let fetch_target = move |name: &str, _: Entity, target: Entity| -> f32 {
            match name {
                TARGET_FONDNESS_INPUT => 0.5,
                TARGET_NOVELTY_INPUT => {
                    if target == novel_stranger {
                        0.9
                    } else {
                        0.1
                    }
                }
                TARGET_SPECIES_COMPAT_INPUT => 1.0,
                // No Intention held → axis is 0 for every candidate.
                PAIRING_INTENTION_INPUT => 0.0,
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
