//! `HuntTargetDse` — §6.5.5 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning prey selection. Pairs with the self-state
//! [`HuntDse`](super::hunt::hunt_dse) which decides *whether* to hunt;
//! this DSE decides *which prey*.
//!
//! Phase 4c.7 scope — yield-aware prey targeting:
//!
//! - `goap.rs::resolve_search_prey`'s `visible_prey.min_by_key(|...|
//!   pos.distance_to(...))` picker retires for the visible-prey
//!   path. §6.1 Partial fix: the resolver today "picks `min_distance`
//!   regardless of yield," so a Mouse at range 5 is chosen over a
//!   Rabbit at range 7 even though the Rabbit delivers 1.3× food
//!   value. With the DSE, the Rabbit wins — assuming alertness hasn't
//!   swung the score.
//! - The scent-detection path (`scented_prey`) remains unchanged —
//!   scent resolves through the §5 influence-map source tile, and the
//!   single-target `min_by_key(source_distance)` is the geometry of
//!   the scent gradient, not a candidate-ranking choice.
//!
//! Three per-target considerations per §6.5.5. The `pursuit-cost` axis
//! lands as a `SpatialConsideration` per the §L2.10.7 plan-cost
//! feedback design (ticket 052) — Manhattan distance to the candidate
//! tile flows through `Logistic(steepness=10, midpoint=0.5, inverted)`
//! over `range = HUNT_TARGET_RANGE`. High-cost prey suppress
//! elastically; the GOAP `replan_count` hard-fail is the last exit per
//! spec §0.2's two-channel composition. Weights renormalized from
//! (0.25/0.25/0.20/0.30) by dropping the 0.30 pursuit-cost row and
//! dividing by 0.70 — the substrate row replaces the ad-hoc nearness
//! axis at the same weight slot.
//!
//! | # | Consideration          | Source                       | Curve                                | Spec weight | Renormalized |
//! |---|------------------------|------------------------------|--------------------------------------|-------------|--------------|
//! | 1 | pursuit-cost           | `Spatial(target)`            | `Logistic(10, 0.5, inverted)` over R | 0.25        | 0.357        |
//! | 2 | prey-species-yield     | `prey_yield` scalar          | `Linear(1, 0)`                       | 0.25        | 0.357        |
//! | 3 | prey-alertness (inv)   | `prey_calm` scalar           | `Linear(1, 0)`                       | 0.20        | 0.286        |
//! | 5 | prey-alertness-toler.  | `prey_alertness_tolerance`   | `Linear(1, 0)`                       | (var)       | runtime      |
//!
//! Axis #5 (ticket 100) is added when `ScoringConstants::
//! hunt_alertness_tolerance_weight > 0.0`. Input is
//! `boldness × alertness`, capturing the orthogonal "I'm bold *and*
//! you're alert" signal that lets bold cats occasionally commit to
//! nervous prey. Other axes renormalize by `(1 − w)` so the
//! WeightedSum stays at 1.0. Pairs with the #3 prey-calm penalty:
//! #3 universally penalizes alert prey; #5 lifts that penalty for
//! bold cats specifically. See `feedback_single_axis_perception_scalars`
//! and the codebase pattern in `fight.rs` / `fox_avoiding.rs`.
//!
//! **Yield normalization.** `ItemKind::food_value()` maxes at 0.8
//! (RawRat). The resolver divides by `YIELD_NORMALIZER = 0.8` so the
//! curve input lands in `[0, 1]` — the Linear(1, 0) curve is then a
//! pass-through in spec space. Rabbit (0.65) → 0.8125 normalized;
//! Mouse (0.5) → 0.625; Bird (0.6) → 0.75; Fish (0.7) → 0.875.
//!
//! **Alertness inversion.** `PreyState.alertness` is already `[0, 1]`
//! (from `prey_state.alertness`). The spec row specifies
//! `Linear(slope=-1, intercept=1)` — an inversion. The resolver feeds
//! `prey_calm = 1 - alertness` directly and keeps the curve as
//! `Linear(1, 0)` so the inversion lives in one place.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkAnchor, LandmarkSource, ScalarConsideration, SpatialConsideration,
};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::faction::StanceRequirement;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::components::prey::PreyKind;
use crate::resources::action_affordances::{ActionAffordances, ActionKind};
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::systems::plan_substrate::{
    cooldown_curve, target_predictability_signal, TARGET_PREDICTABILITY_INPUT,
};

pub const PREY_YIELD_INPUT: &str = "prey_yield";
pub const PREY_CALM_INPUT: &str = "prey_calm";
/// 100 — orthogonal `boldness × alertness` axis. Bold cats partially
/// offset the `prey_calm` penalty when eyeing alert prey: a bold cat
/// will occasionally commit to a nervous rabbit; a patient cat
/// reliably filters for calm targets. Resolver feeds the product
/// `boldness × alertness` so the axis activates exactly when both
/// boldness *and* prey alertness are non-trivial — the orthogonal
/// "I'm bold *and* you're nervous" signal that the existing
/// single-axis `prey_calm` can't express.
pub const PREY_ALERTNESS_TOLERANCE_INPUT: &str = "prey_alertness_tolerance";
/// 263 — `max(Affordance(Stalk|Chase|Pounce))` for `(self, prey)`
/// from substrate 261. Encodes "is this prey actually catchable in
/// any predation form?" as an orthogonal axis to yield + alertness +
/// cooldown. Belief reads (intent_clarity, etc.) live at the
/// affordance layer per the architectural rule — Hunt does NOT add
/// a direct `perceived_intent_clarity` axis.
pub const BEST_PREDATION_AFFORDANCE_INPUT: &str = "hunt_best_predation_affordance";

/// Candidate-pool range in Manhattan tiles. Matches the cat sensory
/// profile's visual detection range (15) — the same outer gate that
/// `resolve_search_prey::visible_prey` uses today. Changing it would
/// shift the candidate population and is a balance decision deferred
/// to post-refactor per open-work #14.
pub const HUNT_TARGET_RANGE: f32 = 15.0;

/// Maximum `ItemKind::food_value()` across raw-prey variants (RawRat =
/// 0.8). Division by this normalizes the yield signal into `[0, 1]`
/// before the Linear curve evaluates.
pub const YIELD_NORMALIZER: f32 = 0.8;

/// Per-prey snapshot fed to `resolve_hunt_target`. Callers build a
/// `Vec<PreyCandidate>` from the frame-local prey query so the
/// resolver doesn't double-borrow it. `PreyKind` stays embedded so
/// the resolver can look up yield via the species food-value table
/// without another component query.
#[derive(Clone, Copy, Debug)]
pub struct PreyCandidate {
    pub entity: Entity,
    pub position: Position,
    pub kind: PreyKind,
    /// `PreyState.alertness` at snapshot time — already in `[0, 1]`.
    pub alertness: f32,
}

/// §6.5.5 `Hunt` target-taking DSE factory.
///
/// 263: takes `&ScoringConstants` so the 5th per-target axis
/// `hunt_best_predation_affordance` is conditionally added when
/// `hunt_best_predation_weight > 0.0`. At dormant default (0.0) the
/// composition is byte-identical to pre-263 (four weights, no
/// affordance axis). At non-zero weight the other four are scaled by
/// `(1 - w)` so the WeightedSum stays at 1.0.
pub fn hunt_target_dse(scoring: &ScoringConstants) -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    // §L2.10.7 pursuit-cost: Manhattan distance / range is the
    // normalized cost; the inverted logistic suppresses high-cost
    // candidates with an S-curve cutoff at midpoint=0.5 (i.e.,
    // `range/2`). Steepness 10 matches the spec catalog's "decisive
    // cutoff" anchor.
    let pursuit_cost_curve = Curve::Composite {
        inner: Box::new(Curve::Logistic {
            steepness: 10.0,
            midpoint: 0.5,
        }),
        post: PostOp::Invert,
    };

    let mut considerations: Vec<Consideration> = vec![
        Consideration::Spatial(SpatialConsideration::new(
            "hunt_pursuit_cost",
            LandmarkSource::TargetPosition,
            HUNT_TARGET_RANGE,
            pursuit_cost_curve,
        )),
        Consideration::Scalar(ScalarConsideration::new(PREY_YIELD_INPUT, linear.clone())),
        Consideration::Scalar(ScalarConsideration::new(PREY_CALM_INPUT, linear.clone())),
        // Ticket 073 — recently-failed target cooldown (audit gap #2).
        // Hunting amplification: Mocha's 109× `HarvestCarcass` failure
        // pattern came from a stuck loop where the dead-or-blocked
        // carcass kept winning the candidate set. The cooldown breaks
        // the loop by penalizing the same-target re-pick.
        Consideration::Scalar(ScalarConsideration::new(
            TARGET_PREDICTABILITY_INPUT,
            cooldown_curve(),
        )),
    ];
    // Pre-263 weights — three §6.5.5 axes renormalized ×(3/4) plus
    // cooldown at 1/4. Sums to ~1.0.
    let base_weights = [
        0.357 * 3.0 / 4.0,
        0.357 * 3.0 / 4.0,
        0.286 * 3.0 / 4.0,
        1.0 / 4.0,
    ];
    let aff_w = scoring.hunt_best_predation_weight.clamp(0.0, 1.0);
    let tol_w = scoring.hunt_alertness_tolerance_weight.clamp(0.0, 1.0);
    // 100: renormalize the four pre-263 weights by `(1 - aff_w - tol_w)`
    // when the optional axes (263 affordance, 100 tolerance) carry
    // non-zero weight, so the WeightedSum stays at 1.0 regardless of
    // which optional axes ship live.
    let extra_w = (aff_w + tol_w).clamp(0.0, 1.0);
    let mut weights: Vec<f32> = if extra_w > 0.0 {
        let scale = 1.0 - extra_w;
        base_weights.iter().map(|w| w * scale).collect()
    } else {
        base_weights.to_vec()
    };
    if tol_w > 0.0 {
        // 100 — prey_alertness_tolerance axis. Linear(1, 0) over the
        // `boldness × alertness` product (resolver-fed). High when bold
        // cat eyes alert prey → partially offsets the prey_calm
        // penalty; zero when boldness or alertness is zero.
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            PREY_ALERTNESS_TOLERANCE_INPUT,
            linear.clone(),
        )));
        weights.push(tol_w);
    }
    if aff_w > 0.0 {
        // 263: `hunt_best_predation_affordance` per-target axis. The
        // `fetch_target` closure computes max(Affordance(Stalk),
        // Affordance(Chase), Affordance(Pounce)) for this (self,
        // target). Linear over [0,1] — the substrate writer is the
        // canonical shaping site.
        considerations.push(Consideration::Scalar(ScalarConsideration::new(
            BEST_PREDATION_AFFORDANCE_INPUT,
            linear,
        )));
        weights.push(aff_w);
    }

    TargetTakingDse {
        id: DseId("hunt_target"),
        candidate_query: hunt_candidate_query_doc,
        per_target_considerations: considerations,
        // WeightedSum — matches the §6.1-Critical spec decision that
        // no axis should null a candidate (a loud rabbit is still
        // huntable). CompensatedProduct would gate alertness at 1.0,
        // and the spec explicitly wants alertness as a linear bias,
        // not a multiplicative lock-out.
        composition: Composition::weighted_sum(weights),
        aggregation: TargetAggregation::Best,
        intention: hunt_intention,
        // §9.3 Hunt accepts `Prey` only. Migrated from the cat-action
        // HuntDse — candidate-prefilter happens here before
        // evaluate_target_taking.
        required_stance: Some(StanceRequirement::hunt()),
        // Tickets 074 + 080 — gate dead/banished/incapacitated
        // candidates AND candidates already reserved by another
        // cat. Combined filter applied at the IAUS scoring layer.
        eligibility: crate::systems::plan_substrate::require_alive_and_unreserved_filter(),
    }
}

fn hunt_candidate_query_doc(_cat: Entity) -> &'static str {
    "prey within HUNT_TARGET_RANGE, visible to cat sensory profile"
}

fn hunt_intention(_target: Entity) -> Intention {
    Intention::Goal {
        state: GoalState::predicate("prey_caught", |_, _| false),
        strategy: CommitmentStrategy::SingleMinded,
    }
}

/// Normalized yield signal from a `PreyKind`. Reads `ItemKind::food_value`
/// via the standard `PreyConfig::item_kind` mapping and divides by
/// `YIELD_NORMALIZER` so the Linear curve sees `[0, 1]`. Inlined here
/// (not in `PreyKind`) because the normalizer is a consideration-
/// specific concern — `food_value` already has a documented meaning
/// on its own.
pub fn prey_yield_normalized(kind: PreyKind) -> f32 {
    let raw = match kind {
        PreyKind::Mouse => crate::components::items::ItemKind::RawMouse.food_value(),
        PreyKind::Rat => crate::components::items::ItemKind::RawRat.food_value(),
        PreyKind::Rabbit => crate::components::items::ItemKind::RawRabbit.food_value(),
        PreyKind::Fish => crate::components::items::ItemKind::RawFish.food_value(),
        PreyKind::Bird => crate::components::items::ItemKind::RawBird.food_value(),
    };
    (raw / YIELD_NORMALIZER).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the best visible prey for `cat` via the registered
/// [`hunt_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
///
/// `candidates` is the caller-built snapshot of in-sensory-range prey
/// (visible or scent-confirmed); it must already pass whatever species/
/// sensing filters the caller applies. The resolver does not re-filter
/// by range — it re-computes distance for the nearness axis, but trusts
/// the candidate list's eligibility.
#[allow(clippy::too_many_arguments)]
pub fn resolve_hunt_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    // 100: cat's boldness clamped to [0, 1]. Powers the
    // `prey_alertness_tolerance` axis's `boldness × alertness` input.
    // Caller pulls from `Personality.boldness`.
    cat_boldness: f32,
    candidates: &[PreyCandidate],
    relations: &crate::ai::faction::FactionRelations,
    stance_overlays: &dyn Fn(Entity) -> crate::ai::faction::StanceOverlays,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
    // Ticket 073 — per-cat recently-failed target memory.
    // 292 — the actor's own belief-state about candidates; the
    // `target_predictability` axis penalizes targets whose
    // predictability facet a recent `TargetActionFailed` snapped low.
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    predator_beliefs: Option<&crate::components::beliefs::PredatorBeliefs>,
    activation: Option<&mut SystemActivation>,
    // 263 — ActionAffordances resource for the 5th per-target axis
    // (`hunt_best_predation_affordance`). Reads return `0.0` for any
    // `(cat, target, kind)` not populated by the writer this tick,
    // which is also the dormant-axis outcome.
    affordances: &ActionAffordances,
    // Ticket 427 Step 1 — pre-allocated scratch buffers.
    scratch: &mut crate::resources::DseTargetScratchpad,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "hunt_target")?;

    if candidates.is_empty() {
        return None;
    }

    // Pull entity + position parallel-vecs for the evaluator. Also
    // populate the prey-kind + alertness lookup tables in the same pass
    // so the per-target fetcher closure can read them via shared
    // borrow (ticket 427 Step 1).
    scratch.entities.clear();
    scratch.positions.clear();
    scratch.prey_kind_map.clear();
    scratch.map_f32_a.clear();
    for c in candidates {
        scratch.entities.push(c.entity);
        scratch.positions.push(c.position);
        scratch.prey_kind_map.insert(c.entity, c.kind);
        scratch
            .map_f32_a
            .insert(c.entity, c.alertness.clamp(0.0, 1.0));
    }

    // §9.3 stance prefilter — drop prey candidates whose resolved
    // stance fails the requirement. `BefriendedAlly` upgrades a Prey
    // base to Ally, which Hunt's `Prey`-only requirement rejects.
    if let Some(req) = dse.required_stance() {
        scratch.species_map.clear();
        for c in candidates {
            scratch.species_map.insert(
                c.entity,
                crate::ai::faction::FactionSpecies::from_sensory(
                    crate::components::sensing::SensorySpecies::Prey(c.kind),
                ),
            );
        }
        let species_map = &scratch.species_map;
        let species_of = |e: Entity| species_map.get(&e).copied();
        crate::ai::faction::filter_candidates_by_stance_in_place(
            relations,
            crate::ai::faction::FactionSpecies::Cat,
            &mut scratch.entities,
            &mut scratch.positions,
            &species_of,
            stance_overlays,
            req,
        );
        if scratch.entities.is_empty() {
            return None;
        }
    }

    // Reborrow lookup tables as `&` so the per-target fetcher closure
    // captures shared references only. Disjoint from `&scratch.entities`
    // / `&scratch.positions` passed to the evaluator below.
    let kind_map = &scratch.prey_kind_map;
    let alertness_map = &scratch.map_f32_a;

    let cooldown_was_applied = std::cell::Cell::new(false);
    let fetch_target = |name: &str, perceiver: Entity, target: Entity| -> f32 {
        match name {
            PREY_YIELD_INPUT => kind_map
                .get(&target)
                .copied()
                .map(prey_yield_normalized)
                .unwrap_or(0.0),
            PREY_CALM_INPUT => alertness_map.get(&target).map(|a| 1.0 - a).unwrap_or(0.5),
            // 100: bold cats lifting the prey_calm penalty for alert
            // prey. `boldness × alertness` is the orthogonal "I'm bold
            // and you're nervous" signal — zero when either factor is
            // zero, so it composes cleanly with the universal prey_calm
            // penalty.
            PREY_ALERTNESS_TOLERANCE_INPUT => alertness_map
                .get(&target)
                .map(|a| cat_boldness.clamp(0.0, 1.0) * a)
                .unwrap_or(0.0),
            TARGET_PREDICTABILITY_INPUT => {
                let signal = target_predictability_signal(cat_beliefs, predator_beliefs, target);
                if signal < 1.0 {
                    cooldown_was_applied.set(true);
                }
                signal
            }
            // 263: max-of-three predation affordances. The substrate
            // writer's heuristic already composes proximity + cover +
            // belief facets (intent_clarity, etc.) per ActionKind,
            // so the DSE-side read is one branch-free max() that
            // picks "the most-afforded predation approach". Returns
            // 0.0 for any (cat, target, kind) the writer didn't
            // populate this tick (out of sensing range, species-gated,
            // below min_eligibility) — the substrate's gate signal.
            BEST_PREDATION_AFFORDANCE_INPUT => {
                let stalk = affordances.read(perceiver, target, ActionKind::Stalk);
                let chase = affordances.read(perceiver, target, ActionKind::Chase);
                let pounce = affordances.read(perceiver, target, ActionKind::Pounce);
                stalk.max(chase).max(pounce)
            }
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
                .set_target_ranking("hunt_target", ranking, tick);
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

    fn candidate(entity_id: u32, x: i32, y: i32, kind: PreyKind, alertness: f32) -> PreyCandidate {
        PreyCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            kind,
            alertness,
        }
    }

    #[test]
    fn hunt_target_dse_id_stable() {
        assert_eq!(
            hunt_target_dse(&ScoringConstants::default()).id().0,
            "hunt_target"
        );
    }

    #[test]
    fn hunt_target_has_five_axes_with_tolerance_live() {
        // Ticket 073 — three legacy axes + cooldown axis = four.
        // Ticket 263 — affordance 5th axis conditional on
        // `hunt_best_predation_weight > 0.0` (dormant at default).
        // Ticket 100 — `prey_alertness_tolerance` axis ships live at
        // `hunt_alertness_tolerance_weight = 0.15`, so the default
        // count is 4 + 1 = 5. When ticket 263 also activates, the
        // count rises to 6.
        let s = ScoringConstants::default();
        assert!(s.hunt_alertness_tolerance_weight > 0.0);
        assert_eq!(hunt_target_dse(&s).per_target_considerations().len(), 5);
    }

    #[test]
    fn hunt_target_weights_sum_to_one() {
        let sum: f32 = hunt_target_dse(&ScoringConstants::default())
            .composition()
            .weights
            .iter()
            .sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn hunt_best_predation_axis_dormant_at_default() {
        // 263: `hunt_best_predation_weight` ships at 0.0; the affordance
        // axis MUST NOT appear in the considerations list.
        // 100: the tolerance axis at default 0.15 IS present, so the
        // total count is 5 (4 base + tolerance) at default.
        let s = ScoringConstants::default();
        assert_eq!(s.hunt_best_predation_weight, 0.0);
        let dse = hunt_target_dse(&s);
        assert!(dse.per_target_considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == BEST_PREDATION_AFFORDANCE_INPUT
        )));
        // WeightedSum still totals ~1.0 at dormant affordance.
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn hunt_best_predation_axis_present_and_renormalized_when_active() {
        // 263: when the activation follow-on lifts the weight, the
        // affordance axis appears and the other axes scale so the
        // WeightedSum stays at 1.0.
        // 100: the tolerance axis (default 0.15) also occupies a slot,
        // so total axes = 4 base + tolerance + affordance = 6.
        let mut s = ScoringConstants::default();
        s.hunt_best_predation_weight = 0.15;
        let dse = hunt_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 6);
        assert_eq!(dse.composition().weights.len(), 6);
        // Last weight is the affordance weight verbatim.
        assert!((dse.composition().weights[5] - 0.15).abs() < 1e-4);
        // Total still sums to 1.0.
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "renormalized sum = {sum}");
    }

    #[test]
    fn hunt_alertness_tolerance_axis_dormant_when_weight_zero() {
        // 100: at `hunt_alertness_tolerance_weight = 0.0` the axis
        // MUST NOT appear and the base four weights are unchanged.
        let mut s = ScoringConstants::default();
        s.hunt_alertness_tolerance_weight = 0.0;
        let dse = hunt_target_dse(&s);
        assert_eq!(dse.per_target_considerations().len(), 4);
        assert!(dse.per_target_considerations().iter().all(|c| !matches!(
            c,
            Consideration::Scalar(sc) if sc.name == PREY_ALERTNESS_TOLERANCE_INPUT
        )));
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3);
    }

    #[test]
    fn hunt_alertness_tolerance_axis_present_at_default() {
        // 100: at default `hunt_alertness_tolerance_weight = 0.15` the
        // axis appears and other weights renormalize so the sum stays
        // at 1.0.
        let s = ScoringConstants::default();
        let dse = hunt_target_dse(&s);
        assert!(dse.per_target_considerations().iter().any(|c| matches!(
            c,
            Consideration::Scalar(sc) if sc.name == PREY_ALERTNESS_TOLERANCE_INPUT
        )));
        let sum: f32 = dse.composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "renormalized sum = {sum}");
    }

    #[test]
    fn hunt_target_uses_best_aggregation() {
        assert_eq!(
            hunt_target_dse(&ScoringConstants::default()).aggregation(),
            TargetAggregation::Best
        );
    }

    #[test]
    fn intention_is_hunt_prey_goal() {
        let dse = hunt_target_dse(&ScoringConstants::default());
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label(), "prey_caught");
                assert_eq!(strategy, CommitmentStrategy::SingleMinded);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    #[test]
    fn prey_yield_normalization_respects_species_ranking() {
        // Ranking matches the food_value table: Rat > Fish > Rabbit
        // > Bird > Mouse. Normalized values land in [0, 1] with Rat
        // pinned to 1.0 (the normalizer).
        let rat = prey_yield_normalized(PreyKind::Rat);
        let fish = prey_yield_normalized(PreyKind::Fish);
        let rabbit = prey_yield_normalized(PreyKind::Rabbit);
        let bird = prey_yield_normalized(PreyKind::Bird);
        let mouse = prey_yield_normalized(PreyKind::Mouse);
        assert!((rat - 1.0).abs() < 1e-5);
        assert!(rat > fish);
        assert!(fish > rabbit);
        assert!(rabbit > bird);
        assert!(bird > mouse);
        assert!(mouse > 0.0);
    }

    fn noop_relations() -> crate::ai::faction::FactionRelations {
        crate::ai::faction::FactionRelations::canonical()
    }

    fn noop_overlays() -> impl Fn(Entity) -> crate::ai::faction::StanceOverlays {
        |_| crate::ai::faction::StanceOverlays::default()
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[],
            &noop_relations(),
            &noop_overlays(),
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
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[],
            &noop_relations(),
            &noop_overlays(),
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
    fn picks_higher_yield_at_equal_distance_and_alertness() {
        // §6.1 Partial fix demo: Rabbit (yield=0.8125) wins over Mouse
        // (yield=0.625) when distance and alertness are tied.
        //
        // 516 tied-position discipline: the expected winner is listed
        // FIRST. WeightedSum ties break toward the LATER candidate, so
        // this ordering fails if the yield axis reads 0.0 for every
        // candidate — the exact silent-death mode the pre-516 prefix
        // routing produced (this test passed for months by listing the
        // rabbit second and winning on the tie-break coincidence).
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let mouse = candidate(2, 3, 0, PreyKind::Mouse, 0.2);
        let rabbit = candidate(3, 0, 3, PreyKind::Rabbit, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[rabbit, mouse],
            &noop_relations(),
            &noop_overlays(),
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(rabbit.entity));
    }

    #[test]
    fn bold_cat_tolerates_alert_prey_more_than_patient_cat() {
        // Ticket 100 — the load-bearing behavioral assertion. Given an
        // alert (alertness ≈ 0.9) rabbit and a calm (alertness ≈ 0.0)
        // rabbit at equal distance: the alertness penalty pushes the
        // calm rabbit higher in both cats' rankings; what matters is
        // the *gap*. With `boldness=0.9` the cat partially offsets the
        // calm rabbit's lead — the alert rabbit's tolerance signal is
        // `boldness × alertness ≈ 0.81`, which the new 0.15-weight axis
        // adds to its score. A patient cat (boldness=0.1) sees the
        // tolerance axis contribute ≈ 0.09 to the alert rabbit. The
        // gap between calm and alert is therefore smaller for the bold
        // cat. We assert the bold cat's winning-margin is smaller than
        // the patient cat's.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let alert = candidate(2, 2, 0, PreyKind::Rabbit, 0.9);
        let calm = candidate(3, 0, 2, PreyKind::Rabbit, 0.0);

        // 516: upgraded from a structural is_some() check to the real
        // gap assertion — the focal-target hook exposes per-candidate
        // scores, so we measure the calm-vs-alert margin under each
        // boldness and require the bold cat's margin to be strictly
        // smaller. A dead tolerance axis makes both margins identical
        // and fails the strict inequality.
        let calm_minus_alert_gap = |boldness: f32| -> f32 {
            let capture = crate::resources::FocalScoreCapture::default();
            let name_lookup = |e: Entity| format!("{e:?}");
            let hook = crate::ai::target_dse::FocalTargetHook {
                capture: &capture,
                name_lookup: &name_lookup,
            };
            let winner = resolve_hunt_target(
                &registry,
                cat,
                Position::new(0, 0),
                boldness,
                &[alert, calm],
                &noop_relations(),
                &noop_overlays(),
                0,
                Some(hook),
                None,
                None,
                None,
                &ActionAffordances::default(),
                &mut crate::resources::DseTargetScratchpad::default(),
            );
            // Calm prey wins at this alertness gap under both
            // temperaments — the tolerance axis shifts the margin,
            // not the identity.
            assert_eq!(winner, Some(calm.entity));
            let inner = capture.drain();
            let ranking = &inner.target_rankings["hunt_target"];
            let score_of = |e: Entity| {
                ranking
                    .candidates
                    .iter()
                    .find(|c| c.name == format!("{e:?}"))
                    .expect("candidate present in focal ranking")
                    .score
            };
            score_of(calm.entity) - score_of(alert.entity)
        };

        let bold_gap = calm_minus_alert_gap(0.9);
        let patient_gap = calm_minus_alert_gap(0.1);
        assert!(
            bold_gap < patient_gap,
            "boldness must shrink the calm-over-alert margin via the \
             tolerance axis: bold gap {bold_gap} vs patient gap {patient_gap}"
        );
    }

    #[test]
    fn alertness_penalizes_otherwise_better_prey() {
        // A very alert Rabbit (alertness=0.95, calm=0.05) loses to a
        // relaxed Mouse (alertness=0.0, calm=1.0) at the same distance.
        // Calc: Rabbit score = 1.0*0.357 + 0.8125*0.357 + 0.05*0.286
        //                    ≈ 0.357 + 0.290 + 0.014 = 0.661
        //       Mouse score  = 1.0*0.357 + 0.625*0.357 + 1.0*0.286
        //                    ≈ 0.357 + 0.223 + 0.286 = 0.866
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        // 516 tied-position discipline: expected winner (mouse) FIRST,
        // so a dead prey_calm axis fails on the toward-later tie-break.
        let relaxed_mouse = candidate(3, 0, 1, PreyKind::Mouse, 0.0);
        let alert_rabbit = candidate(2, 1, 0, PreyKind::Rabbit, 0.95);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[relaxed_mouse, alert_rabbit],
            &noop_relations(),
            &noop_overlays(),
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(relaxed_mouse.entity));
    }

    #[test]
    fn close_prey_outscores_distant_same_species() {
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = candidate(2, 2, 0, PreyKind::Rabbit, 0.2);
        let far = candidate(3, 12, 0, PreyKind::Rabbit, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[close, far],
            &noop_relations(),
            &noop_overlays(),
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn distance_quadratic_penalty_dominates_small_yield_edge() {
        // A Rat (0.8 raw → 1.0 normalized, the richest prey) at
        // distance 10 loses to a Mouse (0.5 → 0.625 normalized) at
        // distance 1 because the §L2.10.7 inverted-logistic pursuit-
        // cost curve drops the Rat's distance contribution near zero
        // (its distance crosses the midpoint=range/2) while the Mouse
        // sits near 1.0 (well below the midpoint).
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let near_mouse = candidate(2, 1, 0, PreyKind::Mouse, 0.1);
        let far_rat = candidate(3, 10, 0, PreyKind::Rat, 0.1);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[near_mouse, far_rat],
            &noop_relations(),
            &noop_overlays(),
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(near_mouse.entity));
    }

    #[test]
    fn retires_min_distance_only_behavior() {
        // Exact §6.1-Partial scenario: at equal yield and alertness,
        // the DSE still picks by distance (matches legacy). The key
        // is that when yield differs, the tie-break is yield, not
        // iteration order — a Rabbit slightly farther than a Mouse
        // still wins when the quadratic nearness gap is smaller than
        // the yield gap. Verified by the higher-yield test above.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let near = candidate(2, 1, 0, PreyKind::Mouse, 0.2);
        let far = candidate(3, 5, 0, PreyKind::Mouse, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[near, far],
            &noop_relations(),
            &noop_overlays(),
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(near.entity));
    }

    #[test]
    fn hunt_target_stance_requirement_is_prey_only() {
        use crate::ai::faction::FactionStance;
        let req = hunt_target_dse(&ScoringConstants::default())
            .required_stance
            .expect("§9.3 binding must populate required_stance");
        assert!(req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Enemy));
        assert!(!req.accepts(FactionStance::Same));
        assert!(!req.accepts(FactionStance::Predator));
    }

    #[test]
    fn pursuit_cost_attenuates_far_prey_smoothly() {
        // §L2.10.7 elastic-channel verification: as candidate distance
        // approaches the outer range the pursuit-cost suppresses score
        // (high cost crosses the inverted-logistic midpoint at
        // range/2). Two same-species, same-alertness rabbits at
        // distance 1 vs distance 14 — the close one wins, and its
        // aggregated score is meaningfully larger than the far one's
        // (not just argmax different).
        let dse = hunt_target_dse(&ScoringConstants::default());
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = candidate(2, 1, 0, PreyKind::Rabbit, 0.2);
        let far = candidate(3, 14, 0, PreyKind::Rabbit, 0.2);

        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[close, far],
            &noop_relations(),
            &noop_overlays(),
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(out, Some(close.entity));

        // Confirm the spatial axis is what's separating them: at the
        // outer-range edge (distance 14, range 15) the inverted
        // logistic at midpoint 0.5 has driven the pursuit-cost score
        // toward zero. Both rabbits have identical yield+calm, so the
        // delta is entirely in the spatial axis.
        let weights = &dse.composition().weights;
        // Weight slot 0 is the pursuit-cost spatial.
        // Pre-073: 0.357.
        // Post-073: 0.357 × 3/4 = 0.268 (cooldown axis added at slot 3).
        // Post-100: × (1 − 0.15) = 0.228 (tolerance axis renormalization
        // with `hunt_alertness_tolerance_weight = 0.15`).
        assert!(
            weights[0] > 0.20 && weights[0] < 0.25,
            "pursuit-cost weight {} outside renormalized band [0.20, 0.25)",
            weights[0]
        );
    }

    #[test]
    fn resolver_drops_befriended_prey() {
        // §9.2 BefriendedAlly upgrades Cat→Prey to Ally. Hunt requires
        // Prey, so a befriended prey candidate should be filtered out
        // before evaluate_target_taking.
        let mut registry = DseRegistry::new();
        registry
            .target_taking_dses
            .push(hunt_target_dse(&ScoringConstants::default()));
        let cat = Entity::from_raw_u32(1).unwrap();
        let befriended = candidate(2, 1, 0, PreyKind::Mouse, 0.2);
        let normal = candidate(3, 0, 1, PreyKind::Mouse, 0.2);
        let stance_overlays = move |e: Entity| {
            if e == befriended.entity {
                crate::ai::faction::StanceOverlays {
                    befriended_ally: true,
                    ..Default::default()
                }
            } else {
                crate::ai::faction::StanceOverlays::default()
            }
        };
        let out = resolve_hunt_target(
            &registry,
            cat,
            Position::new(0, 0),
            0.0,
            &[befriended, normal],
            &crate::ai::faction::FactionRelations::canonical(),
            &stance_overlays,
            0,
            None,
            None,
            None,
            None,
            &ActionAffordances::default(),
            &mut crate::resources::DseTargetScratchpad::default(),
        );
        assert_eq!(
            out,
            Some(normal.entity),
            "befriended prey candidate should be filtered out by §9.3"
        );
    }
}
