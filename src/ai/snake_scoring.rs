//! Snake utility scoring — Maslow-weighted action evaluation.
//!
//! Snakes have a 2-level Maslow hierarchy: Level 1 (survival — hunger,
//! safety) and Level 2 (thermoregulation). Four DSEs: Ambushing,
//! Foraging, Fleeing (L1), Basking (L2).

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;
use rand::Rng;

use crate::ai::dse::EvalCtx;
use crate::ai::considerations::LandmarkAnchor;
use crate::ai::eval::evaluate_single;
use crate::ai::scoring::EvalInputs;
use crate::ai::snake_planner::SnakeDispositionKind;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// SnakeNeeds — 2-level Maslow hierarchy
// ---------------------------------------------------------------------------

/// Level 1: survival (hunger, health). Level 2: thermoregulation (warmth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnakeNeeds {
    /// 1.0 = recently fed, 0.0 = starving.
    pub hunger: f32,
    /// Current health / max health.
    pub health_fraction: f32,
    /// 1.0 = optimal body temperature, 0.0 = dangerously cold.
    pub warmth: f32,
}

impl Default for SnakeNeeds {
    fn default() -> Self {
        Self {
            hunger: 0.8,
            health_fraction: 1.0,
            warmth: 0.7,
        }
    }
}

impl SnakeNeeds {
    /// Level 1 is never suppressed. Level 2 (basking) is suppressed when
    /// survival needs are critical.
    pub fn level_suppression(&self, tier: u8) -> f32 {
        match tier {
            1 => 1.0,
            _ => {
                // L2 suppressed when L1 satisfaction is low.
                let l1_satisfaction = (self.hunger + self.health_fraction) / 2.0;
                l1_satisfaction.clamp(0.0, 1.0)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SnakePersonality — 2-axis personality for snakes
// ---------------------------------------------------------------------------

/// Personality axes relevant to ambush predators.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnakePersonality {
    /// Active vs. passive hunter. High aggression favors Foraging over
    /// Ambushing.
    pub aggression: f32,
    /// Willingness to wait in ambush. Patience modulates ambush duration.
    pub patience: f32,
}

impl SnakePersonality {
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            aggression: rng.random_range(0.1..0.9_f32),
            patience: rng.random_range(0.1..0.9_f32),
        }
    }
}

impl Default for SnakePersonality {
    fn default() -> Self {
        Self {
            aggression: 0.4,
            patience: 0.6,
        }
    }
}

// ---------------------------------------------------------------------------
// SnakeScoringContext
// ---------------------------------------------------------------------------

/// Everything the snake scoring function needs to evaluate dispositions.
pub struct SnakeScoringContext<'a> {
    pub needs: &'a SnakeNeeds,
    pub personality: &'a SnakePersonality,
    /// Whether at least one prey animal is within detection range.
    pub prey_nearby: bool,
    /// Number of cats within threat range.
    pub cats_nearby: usize,
    /// Whether the snake is on warm terrain (rock, sun-exposed).
    pub on_warm_terrain: bool,
    pub self_position: Position,
    pub jitter_range: f32,
}

// ---------------------------------------------------------------------------
// SnakeScoringResult
// ---------------------------------------------------------------------------

pub struct SnakeScoringResult {
    pub scores: Vec<(SnakeDispositionKind, f32)>,
}

// ---------------------------------------------------------------------------
// Scalar inputs for DSE evaluation
// ---------------------------------------------------------------------------

fn snake_ctx_scalars(ctx: &SnakeScoringContext) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    m.insert("hunger_urgency", (1.0 - ctx.needs.hunger).clamp(0.0, 1.0));
    m.insert("hunger", ctx.needs.hunger.clamp(0.0, 1.0));
    m.insert("health_fraction", ctx.needs.health_fraction.clamp(0.0, 1.0));
    m.insert(
        "health_deficit",
        (1.0 - ctx.needs.health_fraction).clamp(0.0, 1.0),
    );
    m.insert("warmth", ctx.needs.warmth.clamp(0.0, 1.0));
    m.insert("warmth_deficit", (1.0 - ctx.needs.warmth).clamp(0.0, 1.0));
    m.insert("aggression", ctx.personality.aggression.clamp(0.0, 1.0));
    m.insert("patience", ctx.personality.patience.clamp(0.0, 1.0));
    m.insert("prey_nearby", if ctx.prey_nearby { 1.0 } else { 0.0 });
    m.insert("cats_nearby", ctx.cats_nearby as f32);
    m.insert(
        "on_warm_terrain",
        if ctx.on_warm_terrain { 1.0 } else { 0.0 },
    );
    m
}

// ---------------------------------------------------------------------------
// DSE dispatch
// ---------------------------------------------------------------------------

/// Score a registered snake DSE through the L2 evaluator.
pub fn score_snake_dse_by_id(dse_id: &str, ctx: &SnakeScoringContext, inputs: &EvalInputs) -> f32 {
    let Some(dse) = inputs.dse_registry.snake_dse(dse_id) else {
        return 0.0;
    };
    let scalars = snake_ctx_scalars(ctx);
    let fetch_scalar = |name: &str, _: Entity| -> f32 { scalars.get(name).copied().unwrap_or(0.0) };
    let has_marker = |_: &str, _: Entity| false;
    let entity_position = |_: Entity| -> Option<Position> { None };
    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    let needs_ref = ctx.needs;
    let maslow = |tier: u8| needs_ref.level_suppression(tier);

    let eval_ctx = EvalCtx {
        cat: inputs.cat,
        tick: inputs.tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
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

/// Score all available dispositions for a snake given its current state.
pub fn score_snake_dispositions(
    ctx: &SnakeScoringContext,
    inputs: &EvalInputs,
    rng: &mut impl Rng,
) -> SnakeScoringResult {
    let j = ctx.jitter_range;
    let mut scores = Vec::with_capacity(4);

    // Ambushing: default predation mode. Hunger-driven + patience.
    {
        let score = score_snake_dse_by_id("snake_ambushing", ctx, inputs);
        if score > 0.0 {
            scores.push((SnakeDispositionKind::Ambushing, score + jitter(rng, j)));
        }
    }

    // Foraging: active hunt when very hungry. Aggression-weighted.
    if ctx.needs.hunger < 0.4 {
        let score = score_snake_dse_by_id("snake_foraging", ctx, inputs);
        if score > 0.0 {
            scores.push((SnakeDispositionKind::Foraging, score + jitter(rng, j)));
        }
    }

    // Fleeing: health deficit + cats nearby.
    if ctx.needs.health_fraction < 0.5 || ctx.cats_nearby >= 1 {
        let score = score_snake_dse_by_id("snake_fleeing", ctx, inputs);
        if score > 0.0 {
            scores.push((SnakeDispositionKind::Fleeing, score + jitter(rng, j)));
        }
    }

    // Basking: thermoregulation. L2 — suppressed when L1 is critical.
    if !ctx.on_warm_terrain || ctx.needs.warmth < 0.6 {
        let score = score_snake_dse_by_id("snake_basking", ctx, inputs);
        if score > 0.0 {
            scores.push((SnakeDispositionKind::Basking, score + jitter(rng, j)));
        }
    }

    // If nothing scored, fall back to Ambushing as the default idle behavior.
    if scores.is_empty() {
        scores.push((SnakeDispositionKind::Ambushing, 0.1 + jitter(rng, j)));
    }

    SnakeScoringResult { scores }
}

/// Softmax disposition selection for snakes.
pub fn select_snake_disposition_softmax(
    result: &SnakeScoringResult,
    rng: &mut impl Rng,
    temperature: f32,
) -> Option<SnakeDispositionKind> {
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
    fn default_snake_needs() {
        let needs = SnakeNeeds::default();
        assert!(needs.hunger > 0.0);
        assert!(needs.warmth > 0.0);
    }

    #[test]
    fn snake_l2_suppressed_when_starving() {
        let mut needs = SnakeNeeds::default();
        needs.hunger = 0.0; // starving
        needs.health_fraction = 0.5;
        // L2 suppression = mean of hunger + health = 0.25
        let suppression = needs.level_suppression(2);
        assert!(suppression < 0.5);
    }

    #[test]
    fn snake_personality_random_in_range() {
        let mut rng = rand::rng();
        let p = SnakePersonality::random(&mut rng);
        assert!(p.aggression >= 0.1 && p.aggression <= 0.9);
        assert!(p.patience >= 0.1 && p.patience <= 0.9);
    }
}
