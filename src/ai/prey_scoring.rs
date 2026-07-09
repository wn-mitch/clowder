//! Prey-side DSE scoring (266) — the lightest scorer in the codebase.
//!
//! Prey do NOT get `evaluate_and_plan`: no GOAP, no markers, no
//! influence maps, no Maslow ladder (flat tier 1). The plan's
//! perf-pinned shape is **alert-set gating + cadence**: only prey
//! whose state machine holds a live detected threat (`Alert` — the
//! honest-perception gate; `try_detect_cat`'s stealth/tremor model
//! decides who is in the set) are scored, and only every
//! `PreyConstants::prey_ai_cadence_ticks` ticks. Idle/grazing prey pay
//! zero scoring cost — load-bearing because `prey_ai`/`try_detect_cat`
//! was already 8.0% inclusive at the 2026-06-09 flamegraph *before*
//! prey scoring existed.
//!
//! Dispatch is hand-written per DSE (the `score_actions` silent-inert
//! rule): registering a prey DSE without a matching dispatcher branch
//! and election arm in `prey_ai` leaves it inert, so the registry
//! entry, the `prey_ctx_scalars` population, and the election arm ship
//! together — three traps, one commit.

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;

use crate::ai::considerations::LandmarkAnchor;
use crate::ai::dse::EvalCtx;
use crate::ai::eval::evaluate_single;
use crate::ai::scoring::EvalInputs;
use crate::components::physical::Position;

/// Everything the prey DSE scalars need, gathered by `prey_ai`'s
/// election arm for one (prey, threat) pair on a cadence tick.
pub struct PreyScoringContext {
    /// `Affordance(Chase, threat, me)` — the threat's chase readiness
    /// against this prey (mutually-perceivable body language; the
    /// urgency carrier).
    pub threat_chase_affordance: f32,
    /// `PredatorBeliefs[threat].perceived_violence_capability` — the
    /// prey's implanted species prior (314 Implant pass), strength-
    /// gated to 0.0 when fully decayed or unmodeled.
    pub threat_violence_belief: f32,
    /// `Affordance(Bolt, me, threat)` — escape viability (head start +
    /// believed lethality + escape-speed ratio + alertness), from
    /// 314's prey-perceiver heuristic.
    pub bolt_affordance: f32,
    pub self_position: Position,
}

fn prey_ctx_scalars(ctx: &PreyScoringContext) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    m.insert(
        crate::ai::dses::prey_bolt::THREAT_CHASE_AFFORDANCE_INPUT,
        ctx.threat_chase_affordance.clamp(0.0, 1.0),
    );
    m.insert(
        crate::ai::dses::prey_bolt::THREAT_VIOLENCE_BELIEF_INPUT,
        ctx.threat_violence_belief.clamp(0.0, 1.0),
    );
    m.insert(
        crate::ai::dses::prey_bolt::BOLT_AFFORDANCE_INPUT,
        ctx.bolt_affordance.clamp(0.0, 1.0),
    );
    m
}

/// Score a registered prey DSE through the L2 evaluator. Returns 0.0
/// for unknown ids (silent-inert rule: the dispatcher branch is part
/// of registering a DSE).
pub fn score_prey_dse_by_id(dse_id: &str, ctx: &PreyScoringContext, inputs: &EvalInputs) -> f32 {
    let Some(dse) = inputs.dse_registry.prey_dse(dse_id) else {
        return 0.0;
    };
    let scalars = prey_ctx_scalars(ctx);
    let fetch_scalar = |name: &str, _: Entity| -> f32 { scalars.get(name).copied().unwrap_or(0.0) };
    let has_marker = |_: &str, _: Entity| false;
    let entity_position = |_: Entity| -> Option<Position> { None };
    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    // Prey have no Maslow ladder — flat survival tier.
    let maslow = |_tier: u8| 1.0;

    let eval_ctx = EvalCtx {
        cat: inputs.cat,
        tick: inputs.tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
        has_marker: &has_marker,
        self_position: inputs.position,
        target: None,
        target_position: None,
        target_alive: None,
        field_cost: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::eval::{DseRegistry, ModifierPipeline};
    use crate::ai::scoring::MarkerSnapshot;
    use crate::plugins::simulation::populate_dse_registry;
    use crate::resources::sim_constants::ScoringConstants;

    macro_rules! test_inputs {
        ($registry:expr, $pipeline:expr, $markers:expr) => {
            EvalInputs {
                cat: Entity::PLACEHOLDER,
                position: Position::new(10, 10),
                tick: 0,
                dse_registry: $registry,
                modifier_pipeline: $pipeline,
                markers: $markers,
                colony_landmarks: &Default::default(),
                exploration_map: &Default::default(),
                corruption_landmarks: &Default::default(),
                focal_cat: None,
                focal_capture: None,
            }
        };
    }

    fn registry() -> DseRegistry {
        let mut r = DseRegistry::new();
        populate_dse_registry(&mut r, &ScoringConstants::default());
        r
    }

    fn ctx(chase: f32, violence: f32, bolt: f32) -> PreyScoringContext {
        PreyScoringContext {
            threat_chase_affordance: chase,
            threat_violence_belief: violence,
            bolt_affordance: bolt,
            self_position: Position::new(10, 10),
        }
    }

    #[test]
    fn committed_chase_scores_decisively_above_uncommitted() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);

        // Cat committed to the chase, dangerous, escape afforded.
        let committed = score_prey_dse_by_id("prey_bolt", &ctx(0.8, 0.6, 0.5), &inputs);
        // Same beliefs + head start, but the writer's min-eligibility
        // gate zeroed the chase read (wounded / uncommitted predator)
        // — the ticket's `prey_no_bolt_at_low_affordance` shape.
        let uncommitted = score_prey_dse_by_id("prey_bolt", &ctx(0.0, 0.6, 0.5), &inputs);

        assert!(
            committed > 0.5,
            "committed chase should elect decisively; got {committed}"
        );
        assert!(
            uncommitted < 0.35,
            "belief + head start alone must stay below any sane \
             election threshold; got {uncommitted}"
        );
        assert!(committed > uncommitted);
    }

    #[test]
    fn bolt_score_orders_by_escape_viability() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);

        let good_escape = score_prey_dse_by_id("prey_bolt", &ctx(0.7, 0.6, 0.8), &inputs);
        let cornered = score_prey_dse_by_id("prey_bolt", &ctx(0.7, 0.6, 0.1), &inputs);
        assert!(
            good_escape > cornered,
            "escape viability must order the score ({good_escape} vs {cornered})"
        );
    }

    #[test]
    fn unknown_dse_id_is_inert() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);
        assert_eq!(
            score_prey_dse_by_id("prey_nonexistent", &ctx(1.0, 1.0, 1.0), &inputs),
            0.0
        );
    }
}
