//! Hawk utility scoring — Maslow-weighted action evaluation.
//!
//! Hawks have a flat 1-level Maslow hierarchy (all survival). Three
//! DSEs: Hunting, Fleeing, Resting. Soaring is the default fallback
//! when no other disposition scores above threshold.

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;
use rand::Rng;

use crate::ai::dse::EvalCtx;
use crate::ai::eval::evaluate_single;
use crate::ai::hawk_planner::HawkDispositionKind;
use crate::ai::scoring::EvalInputs;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// HawkNeeds — truncated 1-level Maslow hierarchy
// ---------------------------------------------------------------------------

/// Hawks have only survival-tier needs. No territory, no offspring.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HawkNeeds {
    /// 1.0 = recently fed, 0.0 = starving.
    pub hunger: f32,
    /// Current health / max health.
    pub health_fraction: f32,
}

impl Default for HawkNeeds {
    fn default() -> Self {
        Self {
            hunger: 0.8,
            health_fraction: 1.0,
        }
    }
}

impl HawkNeeds {
    /// Hawks have no Maslow suppression — all dispositions are survival tier.
    pub fn level_suppression(&self, _tier: u8) -> f32 {
        1.0
    }
}

// ---------------------------------------------------------------------------
// HawkPersonality — 2-axis personality for hawks
// ---------------------------------------------------------------------------

/// Personality axes relevant to aerial predators.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HawkPersonality {
    /// Risk-taking: approach cats, dive in contested areas.
    pub boldness: f32,
    /// Willingness to wait for optimal dive opportunity vs. hasty strike.
    pub patience: f32,
}

impl HawkPersonality {
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            boldness: rng.random_range(0.1..0.9_f32),
            patience: rng.random_range(0.1..0.9_f32),
        }
    }
}

impl Default for HawkPersonality {
    fn default() -> Self {
        Self {
            boldness: 0.5,
            patience: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// HawkScoringContext
// ---------------------------------------------------------------------------

/// Everything the hawk scoring function needs to evaluate dispositions.
pub struct HawkScoringContext<'a> {
    pub needs: &'a HawkNeeds,
    pub personality: &'a HawkPersonality,
    /// Whether at least one prey animal is within detection range.
    pub prey_nearby: bool,
    /// Number of cats within threat range.
    pub cats_nearby: usize,
    /// Fox's current tile.
    pub self_position: Position,
    pub jitter_range: f32,
}

// ---------------------------------------------------------------------------
// HawkScoringResult
// ---------------------------------------------------------------------------

pub struct HawkScoringResult {
    pub scores: Vec<(HawkDispositionKind, f32)>,
}

// ---------------------------------------------------------------------------
// Scalar inputs for DSE evaluation
// ---------------------------------------------------------------------------

fn hawk_ctx_scalars(ctx: &HawkScoringContext) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    m.insert(
        "hunger_urgency",
        (1.0 - ctx.needs.hunger).clamp(0.0, 1.0),
    );
    m.insert("hunger", ctx.needs.hunger.clamp(0.0, 1.0));
    m.insert(
        "health_fraction",
        ctx.needs.health_fraction.clamp(0.0, 1.0),
    );
    m.insert(
        "health_deficit",
        (1.0 - ctx.needs.health_fraction).clamp(0.0, 1.0),
    );
    m.insert("boldness", ctx.personality.boldness.clamp(0.0, 1.0));
    m.insert("patience", ctx.personality.patience.clamp(0.0, 1.0));
    m.insert("prey_nearby", if ctx.prey_nearby { 1.0 } else { 0.0 });
    m.insert("cats_nearby", ctx.cats_nearby as f32);
    m
}

// ---------------------------------------------------------------------------
// DSE dispatch
// ---------------------------------------------------------------------------

/// Score a registered hawk DSE through the L2 evaluator.
pub fn score_hawk_dse_by_id(
    dse_id: &str,
    ctx: &HawkScoringContext,
    inputs: &EvalInputs,
) -> f32 {
    let Some(dse) = inputs.dse_registry.hawk_dse(dse_id) else {
        return 0.0;
    };
    let scalars = hawk_ctx_scalars(ctx);
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
// Jitter
// ---------------------------------------------------------------------------

fn jitter(rng: &mut impl Rng, range: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    rng.random_range(-range..range)
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score all available dispositions for a hawk given its current state.
pub fn score_hawk_dispositions(
    ctx: &HawkScoringContext,
    inputs: &EvalInputs,
    rng: &mut impl Rng,
) -> HawkScoringResult {
    let j = ctx.jitter_range;
    let mut scores = Vec::with_capacity(4);

    // Hunting: hunger-driven + prey presence.
    {
        let score = score_hawk_dse_by_id("hawk_hunting", ctx, inputs);
        if score > 0.0 {
            scores.push((HawkDispositionKind::Hunting, score + jitter(rng, j)));
        }
    }

    // Fleeing: health deficit + cats nearby.
    if ctx.needs.health_fraction < 0.5 || ctx.cats_nearby >= 2 {
        let score = score_hawk_dse_by_id("hawk_fleeing", ctx, inputs);
        if score > 0.0 {
            scores.push((HawkDispositionKind::Fleeing, score + jitter(rng, j)));
        }
    }

    // Resting: when not hungry and health is good — diurnal rest bias.
    if ctx.needs.hunger > 0.5 {
        let score = score_hawk_dse_by_id("hawk_resting", ctx, inputs);
        if score > 0.0 {
            scores.push((HawkDispositionKind::Resting, score + jitter(rng, j)));
        }
    }

    // Soaring is the default — scored as a constant baseline so hawks
    // have something to do when no other drive is pressing.
    scores.push((HawkDispositionKind::Soaring, 0.1 + jitter(rng, j)));

    HawkScoringResult { scores }
}

/// Softmax disposition selection for hawks.
pub fn select_hawk_disposition_softmax(
    result: &HawkScoringResult,
    rng: &mut impl Rng,
    temperature: f32,
) -> Option<HawkDispositionKind> {
    let scores = &result.scores;
    if scores.is_empty() {
        return None;
    }

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

    #[test]
    fn default_hawk_needs() {
        let needs = HawkNeeds::default();
        assert!(needs.hunger > 0.0);
        assert_eq!(needs.health_fraction, 1.0);
    }

    #[test]
    fn hawk_personality_random_in_range() {
        let mut rng = rand::rng();
        let p = HawkPersonality::random(&mut rng);
        assert!(p.boldness >= 0.1 && p.boldness <= 0.9);
        assert!(p.patience >= 0.1 && p.patience <= 0.9);
    }

    #[test]
    fn soaring_always_scored() {
        // Soaring is the fallback — it should always appear in results.
        let needs = HawkNeeds::default();
        let personality = HawkPersonality::default();
        let ctx = HawkScoringContext {
            needs: &needs,
            personality: &personality,
            prey_nearby: false,
            cats_nearby: 0,
            self_position: Position::new(0, 0),
            jitter_range: 0.0,
        };

        // Without DSE registry we can't score the DSE-backed dispositions,
        // but soaring should always be present.
        let registry = crate::ai::eval::DseRegistry::new();
        let modifier = crate::ai::eval::ModifierPipeline::default();
        let markers = crate::ai::scoring::MarkerSnapshot::new();
        let inputs = EvalInputs {
            cat: Entity::PLACEHOLDER,
            tick: 0,
            position: Position::new(0, 0),
            dse_registry: &registry,
            modifier_pipeline: &modifier,
            markers: &markers,
            focal_cat: None,
            focal_capture: None,
        };

        let mut rng = rand::rng();
        let result = score_hawk_dispositions(&ctx, &inputs, &mut rng);
        assert!(result.scores.iter().any(|(k, _)| *k == HawkDispositionKind::Soaring));
    }
}
