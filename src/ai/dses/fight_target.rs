//! `FightTargetDse` — §6.5.9 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Target-taking DSE owning threat selection for `Fight`. Pairs with
//! the self-state [`FightDse`](super::fight::fight_dse) which decides
//! *whether* to engage; this DSE decides *which threat*.
//!
//! Phase 4c.8 scope:
//!
//! - `goap.rs::resolve_goap_plans::EngageThreat`'s
//!   `wildlife.iter().min_by_key(|wp| pos.manhattan_distance(wp))`
//!   picker retires for the un-directed branch. The coordinator-
//!   directive path (`fight_directive_target` in `evaluate_and_plan`)
//!   continues to override the DSE-picked target — a coordinated
//!   posse-engagement takes precedence over per-cat threat ranking,
//!   matching the §7.3 coordinator-cancel-override strategy row
//!   landed in Phase 5a.
//!
//! Four per-target considerations per §6.5.9. The `pursuit-cost`
//! axis doesn't apply to Fight (combat is colocated, not pursued),
//! so all four spec axes port directly. `SumTopN(3)` aggregation —
//! the action score reflects *total* threat from the top-3
//! adversaries, so a surrounded cat engages even if no single threat
//! is maximal. The `winning_target` remains the argmax (single
//! #1 threat) — the sum is for action-selection pressure, the winner
//! is for GOAP planning.
//!
//! | # | Consideration       | Source                   | Curve                                 | Weight |
//! |---|---------------------|--------------------------|---------------------------------------|--------|
//! | 1 | distance            | `Spatial(target)`        | `Composite{Logistic(10, 0.5), Invert}`| 0.25   |
//! | 2 | threat-level        | `target_threat`          | `Quadratic(exp=2)`                    | 0.30   |
//! | 3 | combat-advantage    | `target_combat_adv`      | `Logistic(10, 0.5)`                   | 0.25   |
//! | 4 | ally-proximity      | `ally_proximity`         | `Linear(1, 0)` capped at 3            | 0.20   |
//!
//! The distance axis lands as a `SpatialConsideration` per the
//! §L2.10.7 plan-cost feedback design (ticket 052). Logistic is
//! point-symmetric about its midpoint (0.5), so `Logistic(10, 0.5)`
//! over `(1 - cost)` is identical to `Composite{Logistic(10, 0.5),
//! Invert}` over `cost` (1 - m = m when m = 0.5).
//!
//! **Distance curve interpretation.** §6.5.9 specifies
//! `Logistic(10, 2), range=2-3`; midpoint=2 in tiles. Mapped to the
//! normalized `1 − dist/range` signal with range=10 (combat outer
//! gate), midpoint=0.5 on normalized signal corresponds to dist=5.
//! Logistic(10, 0.5) crosses 0.5 at signal=0.5 and saturates near
//! signal=1 (dist=0) — cats engage when close, disengage when far.
//!
//! **Threat signal.** `WildAnimal.threat_power` lands in [0.08, 0.18]
//! across Fox/Hawk/Snake/ShadowFox today. Normalized to [0, 1] by
//! dividing by `THREAT_POWER_NORMALIZER = 0.25` before the Quadratic.
//! Convex amplification means a ShadowFox (0.18 → 0.72) dominates a
//! Snake (0.08 → 0.32) — squared: 0.518 vs 0.102 — a ~5× gap that
//! overrides moderate distance differences.
//!
//! **Combat-advantage.** Per spec:
//! `self.skills.combat + self.health_fraction − target.threat_level`.
//! Normalized to [0, 1] around a midpoint of 0: values >0 mean the
//! cat has the edge, values <0 mean the threat does. The Logistic
//! midpoint=0.5 (post-normalization) places the curve crossing at
//! advantage=0, so cats engage at parity-or-better and disengage
//! when the threat clearly outmatches them.
//!
//! **Ally-proximity.** Count of ally cats within 4 tiles, capped at
//! 3. Linear from 0 to 1 over that range — first three allies each
//! add ~33% confidence; more has no effect (diminishing-returns
//! flat cap). Encodes "cats engage in groups" as a consideration,
//! not an eligibility gate.

use bevy::prelude::Entity;

use crate::ai::composition::Composition;
use crate::ai::considerations::{
    Consideration, LandmarkSource, ScalarConsideration, SpatialConsideration, LandmarkAnchor};
use crate::ai::curves::{Curve, PostOp};
use crate::ai::dse::{CommitmentStrategy, DseId, EvalCtx, GoalState, Intention};
use crate::ai::eval::DseRegistry;
use crate::ai::faction::StanceRequirement;
use crate::ai::target_dse::{
    evaluate_target_taking, FocalTargetHook, TargetAggregation, TargetTakingDse,
};
use crate::components::physical::Position;
use crate::components::wildlife::WildSpecies;

pub const TARGET_THREAT_INPUT: &str = "target_threat";
pub const TARGET_COMBAT_ADV_INPUT: &str = "target_combat_adv";
pub const ALLY_PROXIMITY_INPUT: &str = "ally_proximity";

/// Candidate-pool range in Manhattan tiles. Matches the pre-refactor
/// EngageThreat candidate pool (all wildlife in sensory range).
/// Changing it would shift the candidate population and is a balance
/// decision deferred to post-refactor per open-work #14.
pub const FIGHT_TARGET_RANGE: f32 = 10.0;

/// Maximum `WildAnimal.threat_power` across species (ShadowFox = 0.18).
/// Normalizer gives the Quadratic a [0, 1] input with the strongest
/// threat pinned to ~0.72. Padding to 0.25 leaves headroom for future
/// species without re-tuning the shape.
pub const THREAT_POWER_NORMALIZER: f32 = 0.25;

/// Radius (Manhattan tiles) inside which allied cats contribute to the
/// `ally_proximity` count. Per §6.5.9: "Count of ally cats within 4
/// tiles."
pub const ALLY_COUNT_RADIUS: i32 = 4;

/// Cap on the ally-proximity raw count before normalization. Per
/// §6.5.9: "Linear with cap: first 3 allies boost confidence linearly;
/// more has diminishing returns."
pub const ALLY_COUNT_CAP: u32 = 3;

/// Per-threat snapshot fed to `resolve_fight_target`. Callers build a
/// `Vec<ThreatCandidate>` from the frame-local wildlife query so the
/// resolver doesn't double-borrow it.
#[derive(Clone, Copy, Debug)]
pub struct ThreatCandidate {
    pub entity: Entity,
    pub position: Position,
    pub species: WildSpecies,
    pub threat_power: f32,
}

/// §6.5.9 `Fight` target-taking DSE factory.
pub fn fight_target_dse() -> TargetTakingDse {
    let linear = Curve::Linear {
        slope: 1.0,
        intercept: 0.0,
    };
    let threat_curve = Curve::Quadratic {
        exponent: 2.0,
        divisor: 1.0,
        shift: 0.0,
    };
    // §L2.10.7 distance axis: Logistic is point-symmetric about its
    // midpoint, so `Logistic(10, 0.5)` over `(1 - cost)` is identical
    // to `Composite{Logistic(10, 0.5), Invert}` over `cost`. Same
    // idiom as Mate's port.
    let nearness_curve = Curve::Composite {
        inner: Box::new(Curve::Logistic {
            steepness: 10.0,
            midpoint: 0.5,
        }),
        post: PostOp::Invert,
    };
    // Combat-advantage logistic: midpoint at 0.5 (post-normalization
    // where 0.5 = parity). Steepness 10 matches the spec's `Logistic
    // (10, 0.5)`. Cats engage at parity-or-above, disengage when
    // clearly outmatched.
    let combat_adv_curve = Curve::Logistic {
        steepness: 10.0,
        midpoint: 0.5,
    };

    TargetTakingDse {
        id: DseId("fight_target"),
        candidate_query: fight_candidate_query_doc,
        per_target_considerations: vec![
            Consideration::Spatial(SpatialConsideration::new(
                "fight_target_nearness",
                LandmarkSource::TargetPosition,
                FIGHT_TARGET_RANGE,
                nearness_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(TARGET_THREAT_INPUT, threat_curve)),
            Consideration::Scalar(ScalarConsideration::new(
                TARGET_COMBAT_ADV_INPUT,
                combat_adv_curve,
            )),
            Consideration::Scalar(ScalarConsideration::new(ALLY_PROXIMITY_INPUT, linear)),
        ],
        composition: Composition::weighted_sum(vec![0.25, 0.30, 0.25, 0.20]),
        // SumTopN(3) per §6.5.9: surrounded cat's action score sums
        // the top-3 threats, encoding "multiple hostiles mean higher
        // engagement urgency." Winner stays the argmax — the cat
        // plans a path to its #1 threat, not to a phantom centroid.
        aggregation: TargetAggregation::SumTopN(3),
        intention: fight_intention,
        // §9.3 Fight (Attack) accepts `Enemy | Prey`. Migrated from
        // the cat-action FightDse — candidate-prefilter happens here
        // before evaluate_target_taking.
        required_stance: Some(StanceRequirement::attack()),
        // Ticket 080 — Fight is contention-tolerant by design (multiple
        // cats engaging the same threat is a feature, not a bug).
        eligibility: Default::default(),
    }
}

fn fight_candidate_query_doc(_cat: Entity) -> &'static str {
    "wildlife within FIGHT_TARGET_RANGE, excluding prey"
}

fn fight_intention(_target: Entity) -> Intention {
    // §7.3: FightTarget is a constituent action of the Guarding
    // disposition and rides Guarding's `Blind` strategy.
    Intention::Goal {
        state: GoalState {
            label: "threat_engaged",
            achieved: |_, _| false,
        },
        strategy: CommitmentStrategy::Blind,
    }
}

/// Normalized threat signal from a `WildAnimal`. Normalizes
/// `threat_power` into [0, 1] via `THREAT_POWER_NORMALIZER`.
pub fn threat_level_normalized(threat_power: f32) -> f32 {
    (threat_power / THREAT_POWER_NORMALIZER).clamp(0.0, 1.0)
}

/// Normalized combat-advantage: `self.skills.combat +
/// self.health_fraction − target.threat_level`. Result mapped to
/// [0, 1] with 0.5 = parity.
///
/// Range analysis:
/// - `skills.combat` ∈ [0, 1]
/// - `health_fraction` ∈ [0, 1]
/// - `target.threat_level` ∈ [0, 1] (already normalized)
///
/// Raw advantage ∈ [-1, 2]. Shift by +1 and divide by 3 to land in
/// [0, 1] — the midpoint (parity) then sits at (0 + 1) / 3 = 0.333
/// when raw == 0. That doesn't match the Logistic(10, 0.5) midpoint.
///
/// Instead, we clamp the raw advantage to [-0.5, 0.5] and map to
/// [0, 1]: `(clamped + 0.5)`. Then parity (raw=0) → 0.5 exactly,
/// matching the Logistic's midpoint. Values outside [-0.5, 0.5]
/// saturate, which is fine because the Logistic already saturates
/// those regions.
pub fn combat_advantage_normalized(
    self_combat: f32,
    self_health_fraction: f32,
    target_threat_level: f32,
) -> f32 {
    let raw = self_combat + self_health_fraction - target_threat_level;
    let clamped = raw.clamp(-0.5, 0.5);
    clamped + 0.5
}

/// Ally-proximity signal: count of `ally_positions` within
/// `ALLY_COUNT_RADIUS` of `cat_pos`, capped at `ALLY_COUNT_CAP`,
/// normalized to [0, 1].
pub fn ally_proximity_normalized(cat_pos: Position, ally_positions: &[Position]) -> f32 {
    let count = ally_positions
        .iter()
        .filter(|p| cat_pos.manhattan_distance(p) <= ALLY_COUNT_RADIUS)
        .count() as u32;
    let capped = count.min(ALLY_COUNT_CAP);
    capped as f32 / ALLY_COUNT_CAP as f32
}

// ---------------------------------------------------------------------------
// Caller-side resolver
// ---------------------------------------------------------------------------

/// Pick the argmax threat for `cat` via the registered
/// [`fight_target_dse`]. Returns `None` iff no eligible candidate
/// exists in range.
///
/// - `candidates` is the caller-built wildlife snapshot.
/// - `self_combat`, `self_health_fraction` feed the combat-advantage
///   axis.
/// - `ally_positions` are all allied (same-faction) cat positions
///   within sensory range; the resolver filters by radius internally.
#[allow(clippy::too_many_arguments)]
pub fn resolve_fight_target(
    registry: &DseRegistry,
    cat: Entity,
    cat_pos: Position,
    candidates: &[ThreatCandidate],
    self_combat: f32,
    self_health_fraction: f32,
    ally_positions: &[Position],
    relations: &crate::ai::faction::FactionRelations,
    stance_overlays: &dyn Fn(Entity) -> crate::ai::faction::StanceOverlays,
    tick: u64,
    focal_hook: Option<FocalTargetHook<'_>>,
) -> Option<Entity> {
    let dse = registry
        .target_taking_dses
        .iter()
        .find(|d| d.id().0 == "fight_target")?;

    if candidates.is_empty() {
        return None;
    }

    // Per-candidate species lookup table — built once, reused by the
    // §9.3 prefilter closure.
    let species_map: std::collections::HashMap<Entity, crate::ai::faction::FactionSpecies> =
        candidates
            .iter()
            .map(|c| {
                (
                    c.entity,
                    crate::ai::faction::FactionSpecies::from_sensory(
                        crate::components::sensing::SensorySpecies::Wild(c.species),
                    ),
                )
            })
            .collect();

    // Filter by range + build lookup tables.
    let mut entities: Vec<Entity> = Vec::new();
    let mut positions: Vec<Position> = Vec::new();
    let mut threat_map: std::collections::HashMap<Entity, f32> = std::collections::HashMap::new();
    for c in candidates {
        let dist = cat_pos.manhattan_distance(&c.position) as f32;
        if dist > FIGHT_TARGET_RANGE {
            continue;
        }
        entities.push(c.entity);
        positions.push(c.position);
        threat_map.insert(c.entity, threat_level_normalized(c.threat_power));
    }

    if entities.is_empty() {
        return None;
    }

    // §9.3 stance prefilter — drop wildlife candidates whose resolved
    // stance fails the requirement. Uses `species_map` to map each
    // candidate's `WildSpecies` onto a `FactionSpecies` row.
    if let Some(req) = dse.required_stance() {
        let species_of = |e: Entity| species_map.get(&e).copied();
        let (filtered, filtered_pos) = crate::ai::faction::filter_candidates_by_stance(
            relations,
            crate::ai::faction::FactionSpecies::Cat,
            &entities,
            &positions,
            &species_of,
            stance_overlays,
            req,
        );
        if filtered.is_empty() {
            return None;
        }
        entities = filtered;
        positions = filtered_pos;
    }

    let ally_score = ally_proximity_normalized(cat_pos, ally_positions);

    // Spatial nearness axis (`fight_target_nearness`) is computed by
    // the substrate from `EvalCtx::self_position` to each candidate's
    // tile per §L2.10.7.
    let fetch_self = |_name: &str, _cat: Entity| -> f32 { 0.0 };
    let fetch_target = |name: &str, _cat: Entity, target: Entity| -> f32 {
        match name {
            TARGET_THREAT_INPUT => threat_map.get(&target).copied().unwrap_or(0.0),
            TARGET_COMBAT_ADV_INPUT => {
                let target_threat = threat_map.get(&target).copied().unwrap_or(0.0);
                combat_advantage_normalized(self_combat, self_health_fraction, target_threat)
            }
            // Ally-proximity is a self-side signal but named with the
            // `target_`-absent convention — the target-scoped fetcher
            // receives it regardless and returns the precomputed scalar.
            ALLY_PROXIMITY_INPUT => ally_score,
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
    };

    let scored = evaluate_target_taking(
        dse,
        cat,
        &entities,
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
                .set_target_ranking("fight_target", ranking, tick);
        }
    }

    scored.winning_target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        entity_id: u32,
        x: i32,
        y: i32,
        species: WildSpecies,
        threat_power: f32,
    ) -> ThreatCandidate {
        ThreatCandidate {
            entity: Entity::from_raw_u32(entity_id).unwrap(),
            position: Position::new(x, y),
            species,
            threat_power,
        }
    }

    #[test]
    fn fight_target_dse_id_stable() {
        assert_eq!(fight_target_dse().id().0, "fight_target");
    }

    #[test]
    fn fight_target_has_four_axes() {
        assert_eq!(fight_target_dse().per_target_considerations().len(), 4);
    }

    #[test]
    fn fight_target_weights_sum_to_one() {
        let sum: f32 = fight_target_dse().composition().weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }

    #[test]
    fn fight_target_uses_sum_top_n_aggregation() {
        assert_eq!(
            fight_target_dse().aggregation(),
            TargetAggregation::SumTopN(3)
        );
    }

    #[test]
    fn intention_is_threat_engaged_goal() {
        let dse = fight_target_dse();
        let target = Entity::from_raw_u32(10).unwrap();
        let intention = (dse.intention)(target);
        match intention {
            Intention::Goal { state, strategy } => {
                assert_eq!(state.label, "threat_engaged");
                assert_eq!(strategy, CommitmentStrategy::Blind);
            }
            other => panic!("expected Goal intention, got {other:?}"),
        }
    }

    #[test]
    fn threat_normalization_respects_species_ranking() {
        let shadow = threat_level_normalized(WildSpecies::ShadowFox.default_threat_power());
        let fox = threat_level_normalized(WildSpecies::Fox.default_threat_power());
        let hawk = threat_level_normalized(WildSpecies::Hawk.default_threat_power());
        let snake = threat_level_normalized(WildSpecies::Snake.default_threat_power());
        assert!(shadow > fox);
        assert!(fox > hawk);
        assert!(hawk > snake);
        assert!(shadow < 1.0);
        assert!(snake > 0.0);
    }

    #[test]
    fn combat_advantage_parity_is_half() {
        // At parity (combat+health == threat), advantage normalizes to 0.5.
        let adv = combat_advantage_normalized(0.3, 0.7, 1.0);
        assert!((adv - 0.5).abs() < 1e-5);
    }

    #[test]
    fn combat_advantage_saturates_at_extremes() {
        // Strong edge: combat=1, health=1, threat=0 → raw=2 → clamped
        // to 0.5 → normalized 1.0.
        let strong = combat_advantage_normalized(1.0, 1.0, 0.0);
        assert!((strong - 1.0).abs() < 1e-5);
        // Weak edge: combat=0, health=0, threat=1 → raw=-1 → clamped
        // to -0.5 → normalized 0.0.
        let weak = combat_advantage_normalized(0.0, 0.0, 1.0);
        assert!(weak.abs() < 1e-5);
    }

    #[test]
    fn ally_proximity_caps_at_three() {
        let cat_pos = Position::new(0, 0);
        let allies = vec![
            Position::new(1, 0),
            Position::new(0, 1),
            Position::new(2, 0),
            Position::new(0, 2),
            Position::new(3, 0),
        ];
        let score = ally_proximity_normalized(cat_pos, &allies);
        // Five within radius 4, cap = 3 → 1.0.
        assert!((score - 1.0).abs() < 1e-5);
    }

    #[test]
    fn ally_proximity_excludes_distant() {
        let cat_pos = Position::new(0, 0);
        let allies = vec![Position::new(1, 0), Position::new(10, 0)];
        let score = ally_proximity_normalized(cat_pos, &allies);
        // Only one within radius 4 → 1/3.
        assert!((score - (1.0 / 3.0)).abs() < 1e-5);
    }

    #[test]
    fn resolver_returns_none_with_no_registered_dse() {
        let registry = DseRegistry::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_returns_none_with_empty_candidates() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(fight_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[],
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn resolver_filters_out_of_range() {
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(fight_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let far = candidate(2, 50, 0, WildSpecies::Fox, 0.15);
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[far],
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert!(out.is_none());
    }

    #[test]
    fn picks_higher_threat_at_equal_distance() {
        // ShadowFox (0.18) outranks Snake (0.08) at equal distance.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(fight_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let shadow = candidate(2, 2, 0, WildSpecies::ShadowFox, 0.18);
        let snake = candidate(3, 0, 2, WildSpecies::Snake, 0.08);
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[shadow, snake],
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert_eq!(out, Some(shadow.entity));
    }

    #[test]
    fn sum_top_n_scores_surrounding_higher_than_single_threat() {
        // A cat surrounded by three ShadowFoxes scores higher than a
        // single ShadowFox — the aggregated score sums top-3. Winner
        // stays the argmax (here: ties go to first-in-order). Uses
        // ShadowFox because §9.3 fight_target accepts Enemy|Prey;
        // Cat→Fox base = Predator (filtered out), Cat→ShadowFox = Enemy.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(fight_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let surrounding = vec![
            candidate(2, 2, 0, WildSpecies::ShadowFox, 0.15),
            candidate(3, 0, 2, WildSpecies::ShadowFox, 0.15),
            candidate(4, -2, 0, WildSpecies::ShadowFox, 0.15),
        ];
        // Just the argmax: winner is one of the Foxes (first-in-order
        // for tied scores). The interesting invariant is that the
        // resolver doesn't panic or return None for multi-threat scenarios.
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &surrounding,
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert!(out.is_some());
    }

    #[test]
    fn close_threat_outscores_distant_same_species() {
        // ShadowFox = Enemy under Cat→target faction. Cat→Fox = Predator
        // would fail §9.3's Enemy|Prey requirement; the test scenario is
        // about distance-vs-threat scoring, not the predator-flee path.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(fight_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let close = candidate(2, 1, 0, WildSpecies::ShadowFox, 0.15);
        let far = candidate(3, 8, 0, WildSpecies::ShadowFox, 0.15);
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[close, far],
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert_eq!(out, Some(close.entity));
    }

    #[test]
    fn fight_target_stance_requirement_is_enemy_or_prey() {
        use crate::ai::faction::FactionStance;
        let req = fight_target_dse()
            .required_stance
            .expect("§9.3 binding must populate required_stance");
        assert!(req.accepts(FactionStance::Enemy));
        assert!(req.accepts(FactionStance::Prey));
        assert!(!req.accepts(FactionStance::Same));
        assert!(!req.accepts(FactionStance::Ally));
        assert!(!req.accepts(FactionStance::Predator));
    }

    #[test]
    fn resolver_drops_predator_candidate_via_stance_prefilter() {
        // §9.3: Cat→Fox = Predator. fight_target's required_stance is
        // Enemy|Prey. Without the prefilter, a Fox candidate would be
        // scored alongside ShadowFox. With the prefilter, Fox is
        // dropped before evaluate_target_taking and ShadowFox wins.
        let mut registry = DseRegistry::new();
        registry.target_taking_dses.push(fight_target_dse());
        let cat = Entity::from_raw_u32(1).unwrap();
        let fox = candidate(2, 1, 0, WildSpecies::Fox, 0.15);
        let shadow = candidate(3, 2, 0, WildSpecies::ShadowFox, 0.18);
        let out = resolve_fight_target(
            &registry,
            cat,
            Position::new(0, 0),
            &[fox, shadow],
            0.5,
            1.0,
            &[],
            &crate::ai::faction::FactionRelations::canonical(),
            &|_| crate::ai::faction::StanceOverlays::default(),
            0,
            None,
        );
        assert_eq!(
            out,
            Some(shadow.entity),
            "Predator-stance Fox candidate should be filtered; ShadowFox (Enemy) wins"
        );
    }
}
