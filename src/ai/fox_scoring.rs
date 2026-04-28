//! Fox utility scoring — Maslow-weighted action evaluation.
//!
//! Parallel to `crate::ai::scoring` for cats, but using the fox's truncated
//! 3-level Maslow hierarchy and 4-axis personality.
//!
//! Phase 3c.1b ports `Hunting` and `Raiding` through the L2 evaluator.
//! The inline scoring blocks for those two dispositions are retired in
//! favor of `score_fox_dse_by_id`, which dispatches to
//! `FoxHuntingDse`/`FoxRaidingDse` in `src/ai/dses/`. The remaining
//! dispositions still use their hand-authored formulas; they port in
//! Phase 3c.2+.

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;
use rand::Rng;

use crate::ai::dse::EvalCtx;
use crate::ai::eval::evaluate_single;
use crate::ai::fox_planner::FoxDispositionKind;
use crate::ai::scoring::EvalInputs;
use crate::components::fox_personality::{FoxNeeds, FoxPersonality};
use crate::components::physical::Position;
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::time::DayPhase;

// ---------------------------------------------------------------------------
// Jitter
// ---------------------------------------------------------------------------

fn jitter(rng: &mut impl Rng, range: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    rng.random_range(-range..range)
}

// ---------------------------------------------------------------------------
// FoxScoringContext
// ---------------------------------------------------------------------------

/// Everything the fox scoring function needs to evaluate dispositions.
pub struct FoxScoringContext<'a> {
    // --- Core state ---
    pub needs: &'a FoxNeeds,
    pub personality: &'a FoxPersonality,

    // --- World perception ---
    /// Whether at least one prey animal is within detection range.
    pub prey_nearby: bool,
    /// Prey belief at the fox's current location (from FoxHuntingBeliefs).
    pub local_prey_belief: f32,
    /// Whether a colony store is visible and within raid range.
    pub store_visible: bool,
    /// Whether the visible store is guarded by cats.
    pub store_guarded: bool,
    /// Number of cats within avoidance range.
    pub cats_nearby: usize,
    /// Whether a cat is within den defense range (and cubs are present).
    pub cat_threatening_den: bool,
    /// Whether a ward is within detection radius.
    pub ward_nearby: bool,
    /// Threat level at the fox's current location (from FoxThreatMemory).
    pub local_threat_level: f32,
    /// Exploration coverage around current position (from FoxExplorationMap).
    pub local_exploration_coverage: f32,

    // --- Offspring state ---
    /// Whether this fox has cubs at its den.
    pub has_cubs: bool,
    /// Whether cubs are hungry (cub_satiation < 0.4).
    pub cubs_hungry: bool,

    // --- Lifecycle state ---
    /// Whether this fox is a homeless juvenile (forces Dispersing).
    pub is_dispersing_juvenile: bool,
    /// Whether this fox has a home den.
    pub has_den: bool,

    // --- §9.2 faction overlay ---
    /// Whether this fox carries the `BefriendedAlly` marker. Suppresses
    /// the §9.3 fox-raiding gate so a befriended fox does not raid.
    /// Per-fox coarsening of the spec's per-pair befriending — see
    /// ticket 049 D5 / §9.2.
    pub befriended_ally: bool,

    // --- Patrolling pressure ---
    /// Ticks since this fox last patrolled. Provides a steady upward pressure
    /// so Patrolling wins periodically even when hunger is never fully sated.
    pub ticks_since_patrol: u64,

    // --- Circadian ---
    /// Current phase of the day. Drives crepuscular/nocturnal bias on Hunting,
    /// Patrolling, and Resting so foxes concentrate hunts at Dusk/Night and
    /// rest at den during Day.
    pub day_phase: DayPhase,

    // --- Self position ---
    /// Fox's current tile. Plumbs through to `EvalCtx.self_position` for
    /// any future spatial consideration; today the ported fox DSEs are
    /// scalar-only.
    pub self_position: Position,

    // --- Tuning ---
    pub scoring: &'a ScoringConstants,
    pub jitter_range: f32,
}

// ---------------------------------------------------------------------------
// FoxScoringResult
// ---------------------------------------------------------------------------

pub struct FoxScoringResult {
    pub scores: Vec<(FoxDispositionKind, f32)>,
}

// ---------------------------------------------------------------------------
// L2 evaluator plumbing (Phase 3c.1b)
// ---------------------------------------------------------------------------

/// Day-phase scalar knots, keyed to the Piecewise curve in
/// `FoxHuntingDse`. Keep in sync with `dses::fox_hunting::{DAWN_KNOT,
/// DAY_KNOT, DUSK_KNOT, NIGHT_KNOT}`.
fn day_phase_scalar(phase: DayPhase) -> f32 {
    use crate::ai::dses::fox_hunting;
    match phase {
        DayPhase::Dawn => fox_hunting::DAWN_KNOT,
        DayPhase::Day => fox_hunting::DAY_KNOT,
        DayPhase::Dusk => fox_hunting::DUSK_KNOT,
        DayPhase::Night => fox_hunting::NIGHT_KNOT,
    }
}

/// Build the scalar-input map for fox DSE consideration dispatch.
///
/// Parallels `scoring::ctx_scalars`. Needs are inverted to urgency form
/// (§2.3: "hunger" = deficit), personality coefficients flow through as
/// `[0, 1]` inputs, `prey_nearby` is a binary 0/1, `day_phase` is the
/// Piecewise knot encoding.
fn fox_ctx_scalars(ctx: &FoxScoringContext) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    m.insert("hunger_urgency", (1.0 - ctx.needs.hunger).clamp(0.0, 1.0));
    // Raw satiation/health scalars for fox Resting's WS axes.
    m.insert("hunger", ctx.needs.hunger.clamp(0.0, 1.0));
    m.insert("health_fraction", ctx.needs.health_fraction.clamp(0.0, 1.0));
    m.insert("boldness", ctx.personality.boldness.clamp(0.0, 1.0));
    m.insert("cunning", ctx.personality.cunning.clamp(0.0, 1.0));
    m.insert(
        "protectiveness",
        ctx.personality.protectiveness.clamp(0.0, 1.0),
    );
    m.insert("prey_nearby", if ctx.prey_nearby { 1.0 } else { 0.0 });
    m.insert("prey_belief", ctx.local_prey_belief.clamp(0.0, 1.0));
    m.insert("day_phase", day_phase_scalar(ctx.day_phase));
    // Fatal-threat peer (3c.2): health deficit and raw cats_nearby
    // count. cats_nearby flows as raw f32 because the consuming DSEs
    // (FoxFleeing Piecewise step, FoxAvoiding saturating Linear)
    // encode their own normalization through their curves.
    m.insert(
        "health_deficit",
        (1.0 - ctx.needs.health_fraction).clamp(0.0, 1.0),
    );
    m.insert("cats_nearby", ctx.cats_nearby as f32);
    // DenDefense axis — cub safety deficit.
    m.insert(
        "cub_safety_deficit",
        (1.0 - ctx.needs.cub_safety).clamp(0.0, 1.0),
    );
    // Fox Patrolling axes.
    m.insert(
        "territory_scent_deficit",
        (1.0 - ctx.needs.territory_scent).clamp(0.0, 1.0),
    );
    m.insert("ticks_since_patrol", ctx.ticks_since_patrol as f32);
    m.insert(
        "territoriality",
        ctx.personality.territoriality.clamp(0.0, 1.0),
    );
    // Offspring axis for fox Feeding.
    m.insert(
        "cub_satiation_deficit",
        (1.0 - ctx.needs.cub_satiation).clamp(0.0, 1.0),
    );
    // Dummy "one" scalar for the fox Dispersing lifecycle-intercept
    // axis. Matches the cat-side convention.
    m.insert("one", 1.0);
    m
}

/// Score a registered fox DSE through the L2 evaluator. Returns 0.0 if
/// the DSE is missing or ineligible — same contract as the cat-side
/// `score_dse_by_id`.
pub fn score_fox_dse_by_id(dse_id: &str, ctx: &FoxScoringContext, inputs: &EvalInputs) -> f32 {
    let Some(dse) = inputs.dse_registry.fox_dse(dse_id) else {
        return 0.0;
    };
    let scalars = fox_ctx_scalars(ctx);
    let fetch_scalar = |name: &str, _: Entity| -> f32 { scalars.get(name).copied().unwrap_or(0.0) };
    let has_marker = |_: &str, _: Entity| false;
    let entity_position = |_: Entity| -> Option<Position> { None };
    let needs_ref = ctx.needs;
    let maslow = |tier: u8| needs_ref.level_suppression(tier);

    let eval_ctx = EvalCtx {
        cat: inputs.cat,
        tick: inputs.tick,
        entity_position: &entity_position,
        has_marker: &has_marker,
        self_position: inputs.position,
        target: None,
        target_position: None,
    };

    evaluate_single(
        dse,
        inputs.cat,
        &eval_ctx,
        &maslow,
        inputs.modifier_pipeline,
        &fetch_scalar,
    )
    .map(|s| s.final_score)
    .unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score all available dispositions for a fox given its current state.
///
/// Uses the truncated 3-level Maslow hierarchy:
/// - Level 1 (Survival): Hunting, Raiding, Resting, Fleeing
/// - Level 2 (Territory): Patrolling, Dispersing
/// - Level 3 (Offspring): Feeding, DenDefense
pub fn score_fox_dispositions(
    ctx: &FoxScoringContext,
    inputs: &EvalInputs,
    rng: &mut impl Rng,
) -> FoxScoringResult {
    let needs = ctx.needs;
    let j = ctx.jitter_range;
    let mut scores = Vec::with_capacity(8);

    // --- Lifecycle override: dispersing juveniles ---
    // §2.3 row 1140: Dispersing is a Linear(intercept=2.0) lifecycle
    // override — intentionally above every other disposition's 1.0
    // peer-group ceiling so it cannot be outvoted.
    if ctx.is_dispersing_juvenile {
        let score = score_fox_dse_by_id("fox_dispersing", ctx, inputs);
        scores.push((FoxDispositionKind::Dispersing, score + jitter(rng, j)));

        // Still allow hunting if starving.
        if needs.hunger < 0.3 {
            let urgency = score_fox_dse_by_id("fox_hunting", ctx, inputs);
            if urgency > 0.0 {
                scores.push((FoxDispositionKind::Hunting, urgency + jitter(rng, j)));
            }
        }

        // Allow fleeing if badly hurt — uses the ported FoxFleeingDse
        // so the juvenile branch stays consistent with adult Fleeing.
        if needs.health_fraction < 0.4 {
            let urgency = score_fox_dse_by_id("fox_fleeing", ctx, inputs);
            if urgency > 0.0 {
                scores.push((FoxDispositionKind::Fleeing, urgency + jitter(rng, j)));
            }
        }

        return FoxScoringResult { scores };
    }

    // -----------------------------------------------------------------------
    // Level 1: Survival (never suppressed)
    // -----------------------------------------------------------------------
    // Every L1 DSE now routes through `score_fox_dse_by_id`, which
    // applies `needs.level_suppression(tier)` inside `evaluate_single`.
    // The old explicit `let l1 = needs.level_suppression(1)` binding
    // retired with the last inline L1 branch in Phase 3c.3.

    // Hunting: §2.3 fox row — WS of 5 axes (hunger_urgency, prey_nearby,
    // prey_belief, day_phase, boldness) via `FoxHuntingDse`. Maslow
    // pre-gate applied inside `evaluate_single` through `l1 = 1.0`.
    {
        let score = score_fox_dse_by_id("fox_hunting", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Hunting, score + jitter(rng, j)));
        }
    }

    // Raiding: §2.3 fox row — CP of (hunger_urgency, cunning) via
    // `FoxRaidingDse`. The `store_visible && !store_guarded` gate stays
    // outer (same pattern as cat `Eat`'s `food_available` outer gate
    // through Phase 3c.1a); Phase 3d flips it to marker-driven
    // eligibility inside `EligibilityFilter`. §9.2 BefriendedAlly
    // suppresses the gate — a befriended fox does not raid.
    if ctx.store_visible && !ctx.store_guarded && !ctx.befriended_ally {
        let score = score_fox_dse_by_id("fox_raiding", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Raiding, score + jitter(rng, j)));
        }
    }

    // Resting: §2.3 WS of hunger + health_fraction + day_phase
    // (Piecewise on fox_rest_*_bonus knots). Diurnal foxes rest by
    // day even when comfort is low — the day_phase axis carries
    // that signal independently (§3.1.1 row 1518 design note).
    if ctx.has_den && needs.hunger > 0.5 {
        let score = score_fox_dse_by_id("fox_resting", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Resting, score + jitter(rng, j)));
        }
    }

    // Fleeing: §2.3 WS of health_deficit + cats_nearby (Piecewise
    // step at 2+) + boldness (damped invert). Outer gate preserves
    // the original `health < 0.5 || cats_nearby >= 2` precondition.
    if needs.health_fraction < 0.5 || ctx.cats_nearby >= 2 {
        let score = score_fox_dse_by_id("fox_fleeing", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Fleeing, score + jitter(rng, j)));
        }
    }

    // -----------------------------------------------------------------------
    // Level 2: Territory (suppressed by survival)
    // -----------------------------------------------------------------------
    // Patrolling: §2.3 WS of territory_scent_deficit (Logistic(5,
    // 0.5)) + ticks_since_patrol (saturating) + day_phase
    // (Piecewise on fox_patrol_*_bonus) + territoriality. Maslow
    // tier 2 pre-gate applied inside `evaluate_single` — the old
    // `let l2 = needs.level_suppression(2)` binding retires.
    if ctx.has_den {
        let score = score_fox_dse_by_id("fox_patrolling", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Patrolling, score + jitter(rng, j)));
        }
    }

    // Avoiding: §2.3 CP of cats_nearby (saturating) + boldness
    // damped-invert. Outer gate preserves the original `cats_nearby
    // >= 1 && hunger > 0.3 && health > 0.5` empirical precondition —
    // tighter than that and foxes stay in fight zones; looser and
    // foxes spin in avoid loops and starve.
    if ctx.cats_nearby >= 1 && needs.hunger > 0.3 && needs.health_fraction > 0.5 {
        let score = score_fox_dse_by_id("fox_avoiding", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Avoiding, score + jitter(rng, j)));
        }
    }

    // -----------------------------------------------------------------------
    // Level 3: Offspring (suppressed by survival × territory)
    // -----------------------------------------------------------------------
    // Maslow tier 3 pre-gate applied inside `evaluate_single` for
    // Feeding + DenDefense — the explicit `let l3 =
    // needs.level_suppression(3)` binding retires with the last
    // inline L3 branch in Phase 3c.8.

    // Feeding: §2.3 CP of cub_satiation_deficit (Logistic(7, 0.6))
    // + protectiveness (Linear). Maslow tier 3 pre-gate applied
    // inside evaluate_single.
    if ctx.has_cubs && ctx.cubs_hungry {
        let score = score_fox_dse_by_id("fox_feeding", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Feeding, score + jitter(rng, j)));
        }
    }

    // Den defense: §2.3 CP of cub_safety_deficit (flee_or_fight
    // Logistic) + protectiveness Linear. Outer gate preserves
    // `cat_threatening_den && has_cubs`.
    if ctx.cat_threatening_den && ctx.has_cubs {
        let score = score_fox_dse_by_id("fox_den_defense", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::DenDefense, score + jitter(rng, j)));
        }
    }

    FoxScoringResult { scores }
}

/// Select the highest-scoring disposition (argmax). Returns `None` if no
/// dispositions scored above 0. Retained for tests and diagnostic paths;
/// the production fox-planner hot path uses `select_fox_disposition_softmax`
/// below, per §L2.10.6.
pub fn select_best_disposition(result: &FoxScoringResult) -> Option<FoxDispositionKind> {
    result
        .scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(kind, _)| *kind)
}

/// Softmax-over-Intentions selection for fox dispositions (§L2.10.6).
/// The cat-side hot path moved to softmax-over-actions in Phase 4.1; the
/// fox side mirrors that at disposition granularity since fox DSEs are
/// 1:1 with dispositions (there is no peer-group MAX-aggregation to
/// dissolve). Temperature is `ScoringConstants::fox_softmax_temperature`.
///
/// Replaces the argmax-only `select_best_disposition` for the live
/// planner, addressing the fox Hunting 0-plan regression documented in
/// `docs/balance/substrate-phase-3.md` where Hunting was consistently
/// outranked by Fleeing / Avoiding and argmax never let it win.
pub fn select_fox_disposition_softmax(
    result: &FoxScoringResult,
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> Option<FoxDispositionKind> {
    let scores = &result.scores;
    if scores.is_empty() {
        return None;
    }

    let temperature = sc.fox_softmax_temperature;
    let max_score = scores
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = scores
        .iter()
        .map(|(_, s)| ((s - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let mut roll: f32 = rng.random::<f32>() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return Some(scores[i].0);
        }
    }
    scores.last().map(|(k, _)| *k)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    use crate::ai::dses::{
        fox_avoiding_dse, fox_den_defense_dse, fox_dispersing_dse, fox_feeding_dse,
        fox_fleeing_dse, fox_hunting_dse, fox_patrolling_dse, fox_raiding_dse, fox_resting_dse,
    };
    use crate::ai::eval::{DseRegistry, ModifierPipeline};

    /// Shared default ScoringConstants for tests — avoids threading a local
    /// `scoring` binding through every test body. Static lifetime satisfies
    /// the `&'a ScoringConstants` field on FoxScoringContext.
    static SCORING: LazyLock<ScoringConstants> = LazyLock::new(ScoringConstants::default);

    fn default_context<'a>(
        needs: &'a FoxNeeds,
        personality: &'a FoxPersonality,
        scoring: &'a ScoringConstants,
    ) -> FoxScoringContext<'a> {
        FoxScoringContext {
            needs,
            personality,
            prey_nearby: true,
            local_prey_belief: 0.5,
            store_visible: false,
            store_guarded: false,
            cats_nearby: 0,
            cat_threatening_den: false,
            ward_nearby: false,
            local_threat_level: 0.0,
            local_exploration_coverage: 0.0,
            has_cubs: false,
            cubs_hungry: false,
            is_dispersing_juvenile: false,
            has_den: true,
            befriended_ally: false,
            ticks_since_patrol: 0,
            day_phase: DayPhase::Night, // hunt-favorable default; tests override
            self_position: Position::new(0, 0),
            scoring,
            jitter_range: 0.0, // no jitter in tests
        }
    }

    /// Build a fox-only DSE registry with `fox_hunting` + `fox_raiding`
    /// registered. Parallels the 4 mirror sites but self-contained for
    /// tests.
    fn test_fox_registry(scoring: &ScoringConstants) -> DseRegistry {
        let mut r = DseRegistry::new();
        r.fox_dses.push(fox_hunting_dse(scoring));
        r.fox_dses.push(fox_raiding_dse());
        r.fox_dses.push(fox_fleeing_dse());
        r.fox_dses.push(fox_avoiding_dse());
        r.fox_dses.push(fox_den_defense_dse());
        r.fox_dses.push(fox_resting_dse(scoring));
        r.fox_dses.push(fox_patrolling_dse(scoring));
        r.fox_dses.push(fox_feeding_dse());
        r.fox_dses.push(fox_dispersing_dse());
        r
    }

    /// Build a throwaway `EvalInputs` bundle for fox tests — tests don't
    /// need a real entity, they just need the registry + modifier
    /// pipeline plumbed through. Entity::from_raw_u32(1) is a stable
    /// placeholder.
    fn test_eval_inputs<'a>(
        registry: &'a DseRegistry,
        modifiers: &'a ModifierPipeline,
        markers: &'a crate::ai::scoring::MarkerSnapshot,
    ) -> EvalInputs<'a> {
        EvalInputs {
            cat: Entity::from_raw_u32(1).unwrap(),
            position: Position::new(0, 0),
            tick: 0,
            dse_registry: registry,
            modifier_pipeline: modifiers,
            markers,
            focal_cat: None,
            focal_capture: None,
        }
    }

    #[test]
    fn starving_fox_prioritizes_hunting() {
        let needs = FoxNeeds {
            hunger: 0.1,
            health_fraction: 0.8,
            territory_scent: 0.0,
            den_security: 1.0,
            cub_satiation: 1.0,
            cub_safety: 1.0,
        };
        let personality = FoxPersonality::balanced();
        let ctx = default_context(&needs, &personality, &SCORING);
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Hunting);
    }

    #[test]
    fn well_fed_fox_with_low_scent_patrols() {
        let needs = FoxNeeds {
            hunger: 0.9,
            health_fraction: 0.9,
            territory_scent: 0.1,
            den_security: 0.9,
            cub_satiation: 1.0,
            cub_safety: 1.0,
        };
        let personality = FoxPersonality {
            territoriality: 0.9,
            ..FoxPersonality::balanced()
        };
        let ctx = FoxScoringContext {
            prey_nearby: false,
            ..default_context(&needs, &personality, &SCORING)
        };
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Patrolling);
    }

    #[test]
    fn hungry_cubs_trigger_feeding() {
        let needs = FoxNeeds {
            hunger: 0.7,
            health_fraction: 0.9,
            territory_scent: 0.8,
            den_security: 0.9,
            cub_satiation: 0.2,
            cub_safety: 0.9,
        };
        let personality = FoxPersonality {
            protectiveness: 0.9,
            ..FoxPersonality::balanced()
        };
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.has_cubs = true;
        ctx.cubs_hungry = true;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Feeding);
    }

    #[test]
    fn starvation_suppresses_offspring_care() {
        let needs = FoxNeeds {
            hunger: 0.05, // nearly starving
            health_fraction: 0.3,
            territory_scent: 0.8,
            den_security: 0.9,
            cub_satiation: 0.1, // cubs are hungry too
            cub_safety: 0.9,
        };
        let personality = FoxPersonality {
            protectiveness: 0.9,
            ..FoxPersonality::balanced()
        };
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.has_cubs = true;
        ctx.cubs_hungry = true;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        // Survival suppresses offspring — fox hunts for itself first.
        assert_eq!(best, FoxDispositionKind::Hunting);
    }

    #[test]
    fn cat_near_den_with_cubs_triggers_defense() {
        let needs = FoxNeeds {
            hunger: 0.7,
            health_fraction: 0.9,
            territory_scent: 0.8,
            den_security: 0.9,
            cub_satiation: 0.8,
            cub_safety: 0.1, // cubs under threat!
        };
        let personality = FoxPersonality {
            protectiveness: 0.9,
            boldness: 0.8,
            ..FoxPersonality::balanced()
        };
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.has_cubs = true;
        ctx.cubs_hungry = false;
        ctx.cat_threatening_den = true;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::DenDefense);
    }

    #[test]
    fn juvenile_always_disperses() {
        let needs = FoxNeeds::default();
        let personality = FoxPersonality::balanced();
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.is_dispersing_juvenile = true;
        ctx.has_den = false;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Dispersing);
    }

    #[test]
    fn cunning_fox_raids_when_store_available() {
        let needs = FoxNeeds {
            hunger: 0.2,
            health_fraction: 0.9,
            territory_scent: 0.8,
            den_security: 0.9,
            cub_satiation: 1.0,
            cub_safety: 1.0,
        };
        let personality = FoxPersonality {
            cunning: 0.9,
            boldness: 0.3,
            ..FoxPersonality::balanced()
        };
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.store_visible = true;
        ctx.store_guarded = false;
        ctx.prey_nearby = false;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        // Should pick raiding over hunting (cunning + unguarded store).
        let has_raiding = result
            .scores
            .iter()
            .any(|(k, _)| *k == FoxDispositionKind::Raiding);
        assert!(has_raiding);
    }

    #[test]
    fn befriended_fox_does_not_raid() {
        // §9.2 ticket 049: a fox carrying `BefriendedAlly` skips the
        // §9.3 raiding gate, even with hunger high and store visible.
        let needs = FoxNeeds {
            hunger: 0.9,
            health_fraction: 0.9,
            territory_scent: 0.8,
            den_security: 0.9,
            cub_satiation: 1.0,
            cub_safety: 1.0,
        };
        let personality = FoxPersonality {
            cunning: 0.9,
            boldness: 0.3,
            ..FoxPersonality::balanced()
        };
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.store_visible = true;
        ctx.store_guarded = false;
        ctx.prey_nearby = false;
        ctx.befriended_ally = true;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let has_raiding = result
            .scores
            .iter()
            .any(|(k, _)| *k == FoxDispositionKind::Raiding);
        assert!(
            !has_raiding,
            "befriended fox should not raise Raiding even with store visible"
        );
    }

    #[test]
    fn injured_outnumbered_fox_flees() {
        let needs = FoxNeeds {
            hunger: 0.5,
            health_fraction: 0.3,
            territory_scent: 0.5,
            den_security: 0.5,
            cub_satiation: 1.0,
            cub_safety: 1.0,
        };
        let personality = FoxPersonality {
            boldness: 0.2, // timid
            ..FoxPersonality::balanced()
        };
        let mut ctx = default_context(&needs, &personality, &SCORING);
        ctx.cats_nearby = 3;
        let registry = test_fox_registry(&SCORING);
        let modifiers = ModifierPipeline::new();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = test_eval_inputs(&registry, &modifiers, &markers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Fleeing);
    }
}
