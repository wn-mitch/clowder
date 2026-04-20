//! Fox utility scoring — Maslow-weighted action evaluation.
//!
//! Parallel to `crate::ai::scoring` for cats, but using the fox's truncated
//! 3-level Maslow hierarchy and 4-axis personality.

use rand::Rng;

use crate::ai::fox_planner::FoxDispositionKind;
use crate::components::fox_personality::{FoxNeeds, FoxPersonality};
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
// Scoring
// ---------------------------------------------------------------------------

/// Score all available dispositions for a fox given its current state.
///
/// Uses the truncated 3-level Maslow hierarchy:
/// - Level 1 (Survival): Hunting, Raiding, Resting, Fleeing
/// - Level 2 (Territory): Patrolling, Dispersing
/// - Level 3 (Offspring): Feeding, DenDefense
pub fn score_fox_dispositions(ctx: &FoxScoringContext, rng: &mut impl Rng) -> FoxScoringResult {
    let needs = ctx.needs;
    let p = ctx.personality;
    let j = ctx.jitter_range;
    let mut scores = Vec::with_capacity(8);

    // --- Lifecycle override: dispersing juveniles ---
    if ctx.is_dispersing_juvenile {
        // Dispersal bypasses Maslow — it's a survival-level instinct for juveniles.
        scores.push((FoxDispositionKind::Dispersing, 2.0 + jitter(rng, j)));

        // Still allow hunting if starving.
        if needs.hunger < 0.3 {
            let urgency = (1.0 - needs.hunger) * p.boldness * 1.5;
            scores.push((FoxDispositionKind::Hunting, urgency + jitter(rng, j)));
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

    // Hunting: driven by hunger, modified by boldness and prey awareness.
    // Day-phase offset is additive so foxes still hunt when starving mid-Day,
    // but the default active window shifts toward Dusk/Night.
    {
        let hunger_urgency = 1.0 - needs.hunger; // higher when hungrier
        let prey_bonus = if ctx.prey_nearby { 0.3 } else { 0.0 };
        let belief_bonus = ctx.local_prey_belief * 0.2;
        let phase_bonus = match ctx.day_phase {
            DayPhase::Dawn => ctx.scoring.fox_hunt_dawn_bonus,
            DayPhase::Day => ctx.scoring.fox_hunt_day_bonus,
            DayPhase::Dusk => ctx.scoring.fox_hunt_dusk_bonus,
            DayPhase::Night => ctx.scoring.fox_hunt_night_bonus,
        };
        let score = (hunger_urgency + prey_bonus + belief_bonus + phase_bonus)
            * p.boldness.max(0.3) // even timid foxes hunt when hungry
            * l1;
        if score > 0.0 {
            scores.push((FoxDispositionKind::Hunting, score + jitter(rng, j)));
        }
    }

    // Raiding: hunger-driven but gated by cunning and store availability.
    if ctx.store_visible && !ctx.store_guarded {
        let hunger_urgency = 1.0 - needs.hunger;
        let score = hunger_urgency * p.cunning * 1.2 * l1;
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
            scoring,
            jitter_range: 0.0, // no jitter in tests
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
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

        let result = score_fox_dispositions(&ctx, &mut rand::rng());
        let best = select_best_disposition(&result).unwrap();
        assert_eq!(best, FoxDispositionKind::Fleeing);
    }
}
