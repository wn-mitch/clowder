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
//! dividing the remaining three by 0.80. The distance axis lands as
//! a `SpatialConsideration` per the §L2.10.7 plan-cost feedback
//! design (ticket 052) — Manhattan distance to the candidate tile
//! flows through `Composite { Logistic(20, 0.5), Invert }` over
//! `range = MATE_TARGET_RANGE`. Inverted-logistic on normalized cost
//! is mathematically equivalent to the prior `Logistic(20, 0.5)`
//! over `1 - dist/range` (logistic is point-symmetric about its
//! midpoint), so this port is behavior-neutral.
//!
//! | # | Consideration | Source              | Curve                                | Spec weight | Renormalized |
//! |---|---------------|---------------------|--------------------------------------|-------------|--------------|
//! | 1 | distance      | `Spatial(target)`   | `Logistic(20, 0.5, inverted)` over R | 0.15        | 0.1875       |
//! | 2 | romantic      | `target_romantic`   | `Linear(1, 0)`                       | 0.40        | 0.5000       |
//! | 3 | fondness      | `target_fondness`   | `Linear(1, 0)`                       | 0.25        | 0.3125       |
//!
//! Candidate filter: nearby cats within `MATE_TARGET_RANGE` tiles
//! whose bond is `Partners` or `Mates`. The bond filter is a
//! structural eligibility gate (§4 / §9.3), not a consideration —
//! matching `build_mating_chain`'s current behavior and closing the
//! bond-filter gap that `find_social_target` left open.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, EvalCtx, Intention, Termination};
use crate::ai::eval::DseRegistry;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::resources::action_affordances::{ActionAffordances, ActionKind};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::systems::plan_substrate::{
    cooldown_curve, perceived_receptivity_signal, target_predictability_signal,
    TARGET_PREDICTABILITY_INPUT,
};

pub const TARGET_ROMANTIC_INPUT: &str = "target_romantic";
pub const TARGET_FONDNESS_INPUT: &str = "target_fondness";
/// 264 — actor's own `CatBeliefs[target].perceived_receptivity`
/// (`[0, 1]`, 0.5 neutral-open for unmodeled partners). The downstream
/// lever on the 126/027 Mate supply-chain problem: low-receptivity
/// partners stop winning the pick and oscillating.
pub const TARGET_PERCEIVED_RECEPTIVITY_INPUT: &str = "target_perceived_receptivity";
/// 264 — per-target `Affordance(Mate, self, target)` read from
/// substrate 261. `target_`-prefixed for the
/// `score_target_consideration` routing reason (ticket 516).
pub const TARGET_MATE_AFFORDANCE_INPUT: &str = "target_affordance_mate";

/// Candidate-pool range for Mate partner selection. Mate's spec
/// template range is 1 (adjacency), but candidate gathering needs a
/// wider pool so near-but-not-adjacent Partners remain scoreable via
/// the Logistic distance curve. Matches the existing
/// `DispositionConstants::social_target_range` semantics.
pub const MATE_TARGET_RANGE: f32 = 10.0;

/// §6.5.2 `Mate` target-taking DSE factory.
///
/// 264: takes `&ScoringConstants` so the conditional belief +
/// affordance axes (`target_perceived_receptivity`, `affordance_mate`)
/// are added only when their weights are non-zero. At dormant defaults
/// the composition is byte-identical to pre-264 (four axes); at
/// non-zero weights the four base axes scale by `(1 − Σ extras)` so
/// the WeightedSum stays at 1.0. Activation must verify the 027
/// Mate-cadence canary.
pub fn mate_target_dse(scoring: &ScoringConstants) -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // §L2.10.7 distance axis: normalized cost (`dist/range`) flows
    // through an inverted logistic — adjacent cost (~0) saturates near
    // 1.0; cost beyond the midpoint (range/2 = 5 tiles) drops sharply
    // toward 0. Steepness 20 matches the spec catalog's "near-step"
    // anchor for Mate.
    let nearness_curve = Curve::Composite {
        inner: Box::new(Curve::Logistic {
            steepness: 20.0,
            midpoint: 0.5,
        }),
        post: PostOp::Invert,
    };

    let mut considerations: Vec<Consideration> = vec![
        Consideration::Spatial(SpatialConsideration::new(
            "mate_target_nearness",
            LandmarkSource::TargetPosition,
            MATE_TARGET_RANGE,
            nearness_curve,
        )),
        Consideration::Scalar(ScalarConsideration::new(
            TARGET_ROMANTIC_INPUT,
            linear.clone(),
        )),
        Consideration::Scalar(ScalarConsideration::new(
            TARGET_FONDNESS_INPUT,
            linear.clone(),
        )),
        // Ticket 073 — recently-failed target cooldown (audit gap #2).
        Consideration::Scalar(ScalarConsideration::new(
            TARGET_PREDICTABILITY_INPUT,
            cooldown_curve(),
        )),
    ];
    // Original three weights (0.1875/0.5/0.3125) renormalized
    // ×(3/4) to make room for the cooldown axis at 1/4. Sums to 1.0.
    let mut weights: Vec<f32> = vec![
        0.1875 * 3.0 / 4.0,
        0.5 * 3.0 / 4.0,
        0.3125 * 3.0 / 4.0,
        1.0 / 4.0,
    ];
    // 264: conditional belief + affordance axes, dormant at 0.0
    // (hunt_target's `hunt_best_predation_weight` shape). Base four
    // scale by `(1 − Σ extras)`; axes push in documented order
    // (receptivity, affordance).
    let receptivity_w = scoring.mate_receptivity_weight.clamp(0.0, 1.0);
    let affordance_w = scoring.mate_affordance_weight.clamp(0.0, 1.0);
    let extra_w = (receptivity_w + affordance_w).clamp(0.0, 1.0);
    if extra_w > 0.0 {
        let scale = 1.0 - extra_w;
        for w in &mut weights {
            *w *= scale;
        }
    }
    if receptivity_w > 0.0 {
        // 264: actor-subjective receptivity belief (0.5 neutral-open
        // — a 0.0 default would bias against never-observed partners,
        // the exact 027 failure mode this axis relieves).
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            TARGET_PERCEIVED_RECEPTIVITY_INPUT,
            linear.clone(),
        )));
        weights.push(receptivity_w);
    }
    if affordance_w > 0.0 {
        // 264: Affordance(Mate, self, target) from substrate 261
        // (estimator: fertility proxy + bond + receptivity +
        // proximity). Reads 0.0 for pairs the writer didn't populate
        // this tick — the substrate's gate signal.
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            TARGET_MATE_AFFORDANCE_INPUT,
            linear,
        )));
        weights.push(affordance_w);
    }

    TargetTakingDse {
        id: DseId("mate_target"),
        candidate_query: mate_candidate_query_doc,
        per_target_considerations: considerations,
        composition: Composition::weighted_sum(weights),
        aggregation: TargetAggregation::Best,
        intention: mate_intention,
        required_stance: None,
        // Tickets 074 + 080 — gate dead/banished/incapacitated
        // candidates AND candidates already reserved by another
        // cat. Combined filter applied at the IAUS scoring layer.
        eligibility: crate::systems::plan_substrate::require_alive_and_unreserved_filter(),
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
#[allow(clippy::too_many_arguments)]
pub fn resolve_mate_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
    // Ticket 073 — per-cat recently-failed target memory.
    // 292 — the actor's own belief-state about candidates; the
    // `target_predictability` axis penalizes targets whose
    // predictability facet a recent `TargetActionFailed` snapped low.
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    predator_beliefs: Option<&crate::components::beliefs::PredatorBeliefs>,
    // Activation tracker for `Feature::TargetCooldownApplied`.
    activation: Option<&mut SystemActivation>,
    // 264 — ActionAffordances resource for the conditional
    // `affordance_mate` axis. Reads return `0.0` for any pair the
    // writer didn't populate this tick; at dormant weight the axis is
    // absent and the arm is never queried.
    affordances: &ActionAffordances,
    // Ticket 427 Step 1 — pre-allocated scratch buffers.
    scratch: &mut crate::resources::DseTargetScratchpad,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "mate_target")?;

    scratch.entities.clear();
    scratch.positions.clear();
    for (other, other_pos) in cat_positions {
        if *other == cat {
            continue;
        }
        let dist = cat_pos.distance_to(other_pos);
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
        // Ticket 453: skip candidates already Mates-bonded to a third
        // party. The actor-side check is unnecessary here — the
        // Partners|Mates filter above already restricts the candidate
        // set to the actor's bonded partner when the actor itself is
        // Mates-locked.
        let other_mated_elsewhere = relationships
            .iter_for(*other)
            .any(|(third, third_rel)| third != cat && third_rel.bond == Some(BondType::Mates));
        if other_mated_elsewhere {
            continue;
        }
        scratch.entities.push(*other);
        scratch.positions.push(*other_pos);
    }

    if scratch.entities.is_empty() {
        return None;
    }

    // Spatial nearness axis (`mate_target_nearness`) is computed by
    // the substrate from `EvalCtx::self_position` to each candidate's
    // tile per §L2.10.7, so no nearness branch lives in `fetch_target`.
    let cooldown_was_applied = std::cell::Cell::new(false);
    let fetch_target = |name: &str, cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_ROMANTIC_INPUT => relationships
                .get(cat, target)
                .map(|r| r.romantic)
                .unwrap_or(0.0),
            TARGET_FONDNESS_INPUT => relationships
                .get(cat, target)
                .map(|r| r.fondness)
                .unwrap_or(0.0),
            TARGET_PREDICTABILITY_INPUT => {
                let signal = target_predictability_signal(cat_beliefs, predator_beliefs, target);
                if signal < 1.0 {
                    cooldown_was_applied.set(true);
                }
                signal
            }
            // 264 — actor-subjective receptivity belief (0.5 neutral
            // for unmodeled partners).
            TARGET_PERCEIVED_RECEPTIVITY_INPUT => perceived_receptivity_signal(cat_beliefs, target),
            // 264 — Affordance(Mate) substrate read.
            TARGET_MATE_AFFORDANCE_INPUT => affordances.read(cat, target, ActionKind::Mate),
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

    // Ticket 073 — record cooldown application once per resolver call.
    if let Some(act) = activation {
        if cooldown_was_applied.get() {
            act.record(Feature::TargetCooldownApplied);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::relationships::BondType;

    /// Pre-264 constants: the two step-20-activated axes zeroed so
    /// exact-shape assertions keep pinning the legacy four-axis
    /// composition. Behavioral argmax tests stay on
    /// `ScoringConstants::default()` — the active axes read uniform
    /// values through the test fetchers, and uniform scaling
    /// preserves argmax.
    fn pre_264_scoring() -> ScoringConstants {
        let mut s = ScoringConstants::default();
        s.mate_receptivity_weight = 0.0;
        s.mate_affordance_weight = 0.0;
        s
    }

    #[test]
    fn mate_target_dse_id_stable() {
        assert_eq!(
            mate_target_dse(&ScoringConstants::default()).id().0,
            "mate_target"
        );
    }

    #[test]
    fn mate_target_has_four_axes() {
        // Ticket 073 — three legacy axes + the cooldown axis = four.
        assert_eq!(
            mate_target_dse(&pre_264_scoring())
                .per_target_considerations()
                .len(),
            4
        );
    }

    #[test]
    fn mate_target_weights_sum_to_one() {
        let sum: f32 = mate_target_dse(&ScoringConstants::default())
            .composition()
            .weights
            .iter()
            .sum();
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
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_excludes_non_bonded_candidates() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let friend_not_partner = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        relationships
            .get_or_insert(cat, friend_not_partner)
            .fondness = 0.9;
        relationships
            .get_or_insert(cat, friend_not_partner)
            .romantic = 0.9;
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
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        // Friends bond doesn't pass the filter — even with romantic=0.9,
        // the resolver returns None. This is the bond-filter fix
        // that `find_social_target` left open.
        assert!(out.is_none());
    }

    #[test]
    fn resolver_picks_partners_bond_candidate() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
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
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(partner));
    }

    #[test]
    fn resolver_picks_higher_romantic_when_both_partners() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
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
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        // Romantic weight (0.5) dominates fondness weight (0.3125),
        // so the more-romantic partner wins even with lower fondness.
        assert_eq!(out, Some(romantic_partner));
    }

    #[test]
    fn nearness_attenuates_far_partner_smoothly() {
        // §L2.10.7 elastic-channel verification: across the
        // inverted-logistic midpoint (range/2 = 5 tiles) the spatial
        // axis suppresses score sharply. Two same-romantic, same-
        // fondness partners — the close one wins. Equal romantic +
        // fondness means the spatial axis is what separates them.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = Entity::from_raw_u32(2).unwrap();
        let far = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        relationships.get_or_insert(cat, close).fondness = 0.5;
        relationships.get_or_insert(cat, close).romantic = 0.5;
        relationships.get_or_insert(cat, close).bond = Some(BondType::Partners);
        relationships.get_or_insert(cat, far).fondness = 0.5;
        relationships.get_or_insert(cat, far).romantic = 0.5;
        relationships.get_or_insert(cat, far).bond = Some(BondType::Partners);

        let cat_positions = vec![
            (close, Position::new(1, 0)), // dist 1, well below midpoint
            (far, Position::new(9, 0)),   // dist 9, well past midpoint
        ];
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(close));
    }

    #[test]
    fn intention_is_pairing_activity() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
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
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(winner, Some(partner));
        // Verify Intention factory produces Pairing activity.
        let dse = mate_target_dse(&ScoringConstants::default());
        let intention = (dse.intention)(partner);
        match intention {
            Intention::Activity { kind, .. } => assert_eq!(kind, ActivityKind::Pairing),
            _ => panic!("expected Activity intention"),
        }
    }

    #[test]
    fn resolver_skips_candidate_mated_elsewhere() {
        // Ticket 453 — candidate-side exclusivity gate. Cat A is
        // Partners with B; B is Mates with C. The resolver must skip
        // B as a target so A doesn't poach a Mates-locked partner.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
        let cat_a = Entity::from_raw_u32(1).unwrap();
        let cat_b = Entity::from_raw_u32(2).unwrap();
        let cat_c = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        // A↔B Partners (would otherwise pass the resolver filter).
        let ab = relationships.get_or_insert(cat_a, cat_b);
        ab.fondness = 0.8;
        ab.romantic = 0.8;
        ab.bond = Some(BondType::Partners);
        // B↔C Mates — B is exclusively bonded elsewhere.
        let bc = relationships.get_or_insert(cat_b, cat_c);
        bc.fondness = 0.9;
        bc.romantic = 0.9;
        bc.bond = Some(BondType::Mates);

        let cat_positions = vec![(cat_b, Position::new(1, 0))];
        let out = resolve_mate_target(
            &registry,
            cat_a,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert!(
            out.is_none(),
            "B is Mates-bonded with C; must not be selected as A's target; got {out:?}"
        );
    }

    #[test]
    fn resolver_keeps_actors_own_mate_when_already_bonded() {
        // Ticket 453 — actor-side check: A is Mates with B. B is *not*
        // mated elsewhere. The existing Partners|Mates filter naturally
        // selects B (the actor's own mate), and the new third-party gate
        // doesn't fire because B's only Mates bond is with A itself.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(mate_target_dse(&ScoringConstants::default()));
        let cat_a = Entity::from_raw_u32(1).unwrap();
        let cat_b = Entity::from_raw_u32(2).unwrap();
        let mut relationships = Relationships::default();
        let ab = relationships.get_or_insert(cat_a, cat_b);
        ab.fondness = 0.8;
        ab.romantic = 0.8;
        ab.bond = Some(BondType::Mates);

        let cat_positions = vec![(cat_b, Position::new(1, 0))];
        let out = resolve_mate_target(
            &registry,
            cat_a,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(cat_b));
    }

    // -----------------------------------------------------------------
    // 264 — conditional belief + affordance axes (dormant wire)
    // -----------------------------------------------------------------

    #[test]
    fn belief_affordance_axes_absent_when_zeroed() {
        // 264 conditional-axis contract: at 0.0 the axes MUST NOT
        // appear and the four-axis composition is byte-identical to
        // pre-264 — the config-override escape hatch and the shape
        // the dormant-wire null-drift gate proved.
        let s = pre_264_scoring();
        let dse = mate_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 4);
        assert!(dse.per_target_considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc)
                if sc.name == TARGET_PERCEIVED_RECEPTIVITY_INPUT
                    || sc.name == TARGET_MATE_AFFORDANCE_INPUT
        )));
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn belief_affordance_axes_present_and_renormalized_when_active() {
        let mut s = ScoringConstants::default();
        s.mate_receptivity_weight = 0.15;
        s.mate_affordance_weight = 0.1;
        let dse = mate_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 6);
        assert_eq!(dse.composition().weights.len(), 6);
        assert!((dse.composition().weights[4] - 0.15).abs() < 1e-6);
        assert!((dse.composition().weights[5] - 0.1).abs() < 1e-6);
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "renormalized sum = {sum}");
    }

    #[test]
    fn belief_affordance_axes_active_at_default() {
        // Step-20 activation (2026-07-08): first-light 0.12 / 0.10 are
        // the shipped defaults — six axes, base four scaled ×0.78,
        // sum still 1.0.
        let s = ScoringConstants::default();
        assert_eq!(s.mate_receptivity_weight, 0.12);
        assert_eq!(s.mate_affordance_weight, 0.10);
        let dse = mate_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 6);
        let weights = &dse.composition().weights;
        assert!((weights[4] - 0.12).abs() < 1e-6);
        assert!((weights[5] - 0.10).abs() < 1e-6);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    /// 264 — ticket microexperiment `mate_skips_low_receptivity_partner`
    /// at the resolver layer: two Partners-bonded candidates tied on
    /// romantic/fondness/distance; the one the actor believes
    /// receptive wins over the one believed unreceptive. Kills the
    /// silent-inert trap on the receptivity fetch arm.
    #[test]
    fn mate_prefers_receptive_partner_when_axis_active() {
        let mut s = ScoringConstants::default();
        s.mate_receptivity_weight = 0.2;
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mate_target_dse(&s));
        let cat = Entity::from_raw_u32(1).unwrap();
        let receptive = Entity::from_raw_u32(2).unwrap();
        let unreceptive = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        for partner in [receptive, unreceptive] {
            let r = relationships.get_or_insert(cat, partner);
            r.fondness = 0.5;
            r.romantic = 0.5;
            r.bond = Some(BondType::Partners);
        }

        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let m = beliefs.models.entry(receptive).or_default();
        m.perceived_receptivity = crate::components::beliefs::Facet {
            value: 0.9,
            strength: 1.0,
            ..Default::default()
        };
        let m = beliefs.models.entry(unreceptive).or_default();
        m.perceived_receptivity = crate::components::beliefs::Facet {
            value: 0.1,
            strength: 1.0,
            ..Default::default()
        };

        let cat_positions = vec![
            (receptive, Position::new(1, 0)),
            (unreceptive, Position::new(0, 1)),
        ];
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
            Some(&beliefs),
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(
            out,
            Some(receptive),
            "believed-receptive partner must beat believed-unreceptive when the axis is live"
        );
    }

    /// 264 — affordance arm verified live: the substrate-priced
    /// partner beats the unpriced one when the axis is active.
    #[test]
    fn mate_reads_affordance_when_axis_active() {
        let mut s = ScoringConstants::default();
        s.mate_affordance_weight = 0.2;
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(mate_target_dse(&s));
        let cat = Entity::from_raw_u32(1).unwrap();
        let afforded = Entity::from_raw_u32(2).unwrap();
        let unpriced = Entity::from_raw_u32(3).unwrap();
        let mut relationships = Relationships::default();
        for partner in [afforded, unpriced] {
            let r = relationships.get_or_insert(cat, partner);
            r.fondness = 0.5;
            r.romantic = 0.5;
            r.bond = Some(BondType::Partners);
        }
        let mut affordances = ActionAffordances::default();
        affordances.write(cat, afforded, ActionKind::Mate, 0.9);

        let cat_positions = vec![
            (afforded, Position::new(1, 0)),
            (unpriced, Position::new(0, 1)),
        ];
        let out = resolve_mate_target(
            &registry,
            cat,
            Position::new(0, 0),
            &cat_positions,
            &relationships,
            0,
            None,
            None,
            None,
            None,
            &affordances,
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(afforded));
    }
}
