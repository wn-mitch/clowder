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
    m.insert(
        "hunger_urgency",
        (1.0 - ctx.needs.hunger).clamp(0.0, 1.0),
    );
    m.insert("boldness", ctx.personality.boldness.clamp(0.0, 1.0));
    m.insert("cunning", ctx.personality.cunning.clamp(0.0, 1.0));
    m.insert("prey_nearby", if ctx.prey_nearby { 1.0 } else { 0.0 });
    m.insert(
        "prey_belief",
        ctx.local_prey_belief.clamp(0.0, 1.0),
    );
    m.insert("day_phase", day_phase_scalar(ctx.day_phase));
    m
}

/// Score a registered fox DSE through the L2 evaluator. Returns 0.0 if
/// the DSE is missing or ineligible — same contract as the cat-side
/// `score_dse_by_id`.
pub fn score_fox_dse_by_id(
    dse_id: &str,
    ctx: &FoxScoringContext,
    inputs: &EvalInputs,
) -> f32 {
    let Some(dse) = inputs.dse_registry.fox_dse(dse_id) else {
        return 0.0;
    };
    let scalars = fox_ctx_scalars(ctx);
    let fetch_scalar = |name: &str, _: Entity| -> f32 {
        scalars.get(name).copied().unwrap_or(0.0)
    };
    let has_marker = |_: &str, _: Entity| false;
    let sample_map = |_: &str, _: Position| 0.0_f32;
    let needs_ref = ctx.needs;
    let maslow = |tier: u8| needs_ref.level_suppression(tier);

    let eval_ctx = EvalCtx {
        cat: inputs.cat,
        tick: inputs.tick,
        sample_map: &sample_map,
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
    let p = ctx.personality;
    let j = ctx.jitter_range;
    let mut scores = Vec::with_capacity(8);

    // --- Lifecycle override: dispersing juveniles ---
    if ctx.is_dispersing_juvenile {
        // Dispersal bypasses Maslow — it's a survival-level instinct for juveniles.
        scores.push((FoxDispositionKind::Dispersing, 2.0 + jitter(rng, j)));

        // Still allow hunting if starving. Uses the ported `FoxHuntingDse`
        // so the juvenile path stays consistent with adult Hunting scoring
        // under the L2 evaluator.
        if needs.hunger < 0.3 {
            let urgency = score_fox_dse_by_id("fox_hunting", ctx, inputs);
            if urgency > 0.0 {
                scores.push((FoxDispositionKind::Hunting, urgency + jitter(rng, j)));
            }
        }

        // Allow fleeing if badly hurt.
        if needs.health_fraction < 0.4 {
            let urgency = (1.0 - needs.health_fraction) * (1.0 - p.boldness) * 2.0;
            scores.push((FoxDispositionKind::Fleeing, urgency + jitter(rng, j)));
        }

        return FoxScoringResult { scores };
    }

    // -----------------------------------------------------------------------
    // Level 1: Survival (never suppressed)
    // -----------------------------------------------------------------------
    let l1 = needs.level_suppression(1); // always 1.0

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
    // eligibility inside `EligibilityFilter`.
    if ctx.store_visible && !ctx.store_guarded {
        let score = score_fox_dse_by_id("fox_raiding", ctx, inputs);
        if score > 0.0 {
            scores.push((FoxDispositionKind::Raiding, score + jitter(rng, j)));
        }
    }

    // Resting: when well-fed and at/near den. Day phase bonus turns Day into
    // the default rest window — mirroring the Hunting bias at the opposite end
    // of the circadian cycle so foxes aren't forced to pick a survival-tier
    // action when neither hunger nor health is critical.
    if ctx.has_den && needs.hunger > 0.5 {
        let comfort = needs.hunger * needs.health_fraction;
        let phase_bonus = match ctx.day_phase {
            DayPhase::Dawn => ctx.scoring.fox_rest_dawn_bonus,
            DayPhase::Day => ctx.scoring.fox_rest_day_bonus,
            DayPhase::Dusk => ctx.scoring.fox_rest_dusk_bonus,
            DayPhase::Night => ctx.scoring.fox_rest_night_bonus,
        };
        let score = (comfort * 0.6 + phase_bonus) * l1;
        if score > 0.0 {
            scores.push((FoxDispositionKind::Resting, score + jitter(rng, j)));
        }
    }

    // Fleeing: when health is critical. Inversely weighted by boldness.
    if needs.health_fraction < 0.5 || ctx.cats_nearby >= 2 {
        let danger = (1.0 - needs.health_fraction) + if ctx.cats_nearby >= 2 { 0.5 } else { 0.0 };
        let score = danger * (1.0 - p.boldness * 0.5) * l1;
        if score > 0.0 {
            scores.push((FoxDispositionKind::Fleeing, score + jitter(rng, j)));
        }
    }

    // -----------------------------------------------------------------------
    // Level 2: Territory (suppressed by survival)
    // -----------------------------------------------------------------------
    let l2 = needs.level_suppression(2);

    // Patrolling: driven by low territory scent, weighted by territoriality.
    // Time-since-last-patrol adds a steady upward pressure — if the fox hasn't
    // patrolled in a while, this disposition eventually wins even when hunger
    // is slightly unsatisfied.
    if ctx.has_den {
        let scent_urgency = 1.0 - needs.territory_scent;
        let time_pressure = (ctx.ticks_since_patrol as f32 / 2000.0).min(1.0);
        let phase_bonus = match ctx.day_phase {
            DayPhase::Dawn => ctx.scoring.fox_patrol_dawn_bonus,
            DayPhase::Day => ctx.scoring.fox_patrol_day_bonus,
            DayPhase::Dusk => ctx.scoring.fox_patrol_dusk_bonus,
            DayPhase::Night => ctx.scoring.fox_patrol_night_bonus,
        };
        let score = (scent_urgency + time_pressure + phase_bonus) * p.territoriality * l2;
        if score > 0.0 {
            scores.push((FoxDispositionKind::Patrolling, score + jitter(rng, j)));
        }
    }

    // Avoiding: cats nearby and fox is at least partially fed — withdraw
    // rather than risk a skirmish. Empirically the 0.3 threshold produces the
    // best cat survival outcomes; tighter than that and foxes stay in fight
    // zones; looser and foxes spin in avoid loops and starve.
    if ctx.cats_nearby >= 1 && needs.hunger > 0.3 && needs.health_fraction > 0.5 {
        let urgency = (ctx.cats_nearby as f32) * (1.0 - p.boldness * 0.8);
        let score = urgency * l1;
        if score > 0.0 {
            scores.push((FoxDispositionKind::Avoiding, score + jitter(rng, j)));
        }
    }

    // -----------------------------------------------------------------------
    // Level 3: Offspring (suppressed by survival × territory)
    // -----------------------------------------------------------------------
    let l3 = needs.level_suppression(3);

    // Feeding: hunt prey and bring it back to cubs.
    if ctx.has_cubs && ctx.cubs_hungry {
        let urgency = (1.0 - needs.cub_satiation) * p.protectiveness * 1.5;
        let score = urgency * l3;
        if score > 0.0 {
            scores.push((FoxDispositionKind::Feeding, score + jitter(rng, j)));
        }
    }

    // Den defense: confront intruders near den with cubs.
    if ctx.cat_threatening_den && ctx.has_cubs {
        let urgency = (1.0 - needs.cub_safety) * p.protectiveness * 2.0;
        let score = urgency * l3;
        if score > 0.0 {
            scores.push((FoxDispositionKind::DenDefense, score + jitter(rng, j)));
        }
    }

    FoxScoringResult { scores }
}

/// Select the highest-scoring disposition. Returns `None` if no dispositions
/// scored above 0.
pub fn select_best_disposition(result: &FoxScoringResult) -> Option<FoxDispositionKind> {
    result
        .scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(kind, _)| *kind)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    use crate::ai::dses::{fox_hunting_dse, fox_raiding_dse};
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
        r
    }

    /// Build a throwaway `EvalInputs` bundle for fox tests — tests don't
    /// need a real entity, they just need the registry + modifier
    /// pipeline plumbed through. Entity::from_raw_u32(1) is a stable
    /// placeholder.
    fn test_eval_inputs<'a>(
        registry: &'a DseRegistry,
        modifiers: &'a ModifierPipeline,
    ) -> EvalInputs<'a> {
        EvalInputs {
            cat: Entity::from_raw_u32(1).unwrap(),
            position: Position::new(0, 0),
            tick: 0,
            dse_registry: registry,
            modifier_pipeline: modifiers,
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
        let inputs = test_eval_inputs(&registry, &modifiers);

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
        let inputs = test_eval_inputs(&registry, &modifiers);

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
        let inputs = test_eval_inputs(&registry, &modifiers);

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
        let inputs = test_eval_inputs(&registry, &modifiers);

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
        let inputs = test_eval_inputs(&registry, &modifiers);

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
        let inputs = test_eval_inputs(&registry, &modifiers);

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
        let inputs = test_eval_inputs(&registry, &modifiers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        // Should pick raiding over hunting (cunning + unguarded store).
        let has_raiding = result
            .scores
            .iter()
            .any(|(k, _)| *k == FoxDispositionKind::Raiding);
        assert!(has_raiding);
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
        let inputs = test_eval_inputs(&registry, &modifiers);

        let result = score_fox_dispositions(&ctx, &inputs, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Fleeing);
    }
}
