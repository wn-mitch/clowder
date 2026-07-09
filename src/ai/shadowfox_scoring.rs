//! Shadow-fox DSE scoring (310 S4).
//!
//! Unlike the fox/hawk/snake stacks, the shadow-fox does NOT get its
//! own GOAP loop: the 023 motivation softmax stays the single election
//! (pillar 4 — commitment is one mechanism). This module scores the
//! three registry DSEs (`shadowfox_hunt` / `shadowfox_retreat` /
//! `shadowfox_patrol`) so they can stand as *candidates* in that
//! softmax alongside the four hand-scored corruption drives
//! (Coherence / Resonance / Dread / Entropy). Scoring through the L2
//! evaluator makes the hunt/retreat/patrol pressures trace-visible and
//! modifier-pipeline'd, replacing S1's hand-computed hunger pressure
//! and the legacy 5%/tick stalk roll.
//!
//! Dispatch is hand-written per DSE (the `score_actions` silent-inert
//! rule): registering a DSE without a matching dispatcher branch and
//! candidate-arm in `shadowfox_motivation_tick` leaves it inert, so
//! the registry entry, the dispatch branch, and the candidate arm ship
//! together.

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;

use crate::ai::considerations::LandmarkAnchor;
use crate::ai::dse::EvalCtx;
use crate::ai::eval::evaluate_single;
use crate::ai::scoring::EvalInputs;
use crate::components::physical::Position;
use crate::resources::time::DayPhase;

/// Everything the shadow-fox DSE scalars need, gathered by
/// `shadowfox_motivation_tick` from the same perception pass that
/// scores the corruption drives.
pub struct ShadowfoxScoringContext {
    /// `1.0 −` [`ShadowFoxDrives::satiation`](crate::components::wildlife::ShadowFoxDrives).
    pub hunger_urgency: f32,
    /// Raw satiation (the Retreat axis reads it directly).
    pub satiation: f32,
    /// Whether at least one eligible cat (kill-site filter applied)
    /// sits inside the motivation scan radius.
    pub cat_in_scan: bool,
    /// Corruption-born predators work in the dark: 1.0 night,
    /// 0.7 dusk/dawn, 0.2 day. See [`night_scalar`].
    pub night_scalar: f32,
    /// Max `Affordance(Ambush, fox, cat)` over eligible cats in scan
    /// (261 substrate; concealment-keyed estimator in
    /// `write_wildlife_vs_cat`).
    pub best_cat_ambush_affordance: f32,
    /// Distance to the den normalized by the scan radius, clamped to
    /// [0, 1]. 0.0 when the den is unknown (the Retreat candidate is
    /// gated out before scoring in that case).
    pub den_distance_norm: f32,
    pub self_position: Position,
}

/// Day-phase scalar for the nocturnal-predator axes. Inverse of the
/// diurnal `fox_scoring::day_phase_scalar` shape: the corruption hunts
/// when the light fails.
pub fn night_scalar(phase: DayPhase) -> f32 {
    match phase {
        DayPhase::Night => 1.0,
        DayPhase::Dusk | DayPhase::Dawn => 0.7,
        DayPhase::Day => 0.2,
    }
}

fn shadowfox_ctx_scalars(ctx: &ShadowfoxScoringContext) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    m.insert(
        crate::ai::dses::shadowfox_hunt::HUNGER_URGENCY_INPUT,
        ctx.hunger_urgency.clamp(0.0, 1.0),
    );
    m.insert(
        crate::ai::dses::shadowfox_hunt::CAT_IN_SCAN_INPUT,
        if ctx.cat_in_scan { 1.0 } else { 0.0 },
    );
    m.insert(
        crate::ai::dses::shadowfox_hunt::NIGHT_SCALAR_INPUT,
        ctx.night_scalar.clamp(0.0, 1.0),
    );
    m.insert(
        crate::ai::dses::shadowfox_hunt::CAT_AMBUSH_AFFORDANCE_INPUT,
        ctx.best_cat_ambush_affordance.clamp(0.0, 1.0),
    );
    m.insert(
        crate::ai::dses::shadowfox_retreat::SATIATION_INPUT,
        ctx.satiation.clamp(0.0, 1.0),
    );
    m.insert(
        crate::ai::dses::shadowfox_retreat::DEN_DISTANCE_INPUT,
        ctx.den_distance_norm.clamp(0.0, 1.0),
    );
    m
}

/// Score a registered shadow-fox DSE through the L2 evaluator.
/// Returns 0.0 for unknown ids (silent-inert rule: the dispatcher
/// branch is part of registering a DSE).
pub fn score_shadowfox_dse_by_id(
    dse_id: &str,
    ctx: &ShadowfoxScoringContext,
    inputs: &EvalInputs,
) -> f32 {
    let Some(dse) = inputs.dse_registry.shadowfox_dse(dse_id) else {
        return 0.0;
    };
    let scalars = shadowfox_ctx_scalars(ctx);
    let fetch_scalar = |name: &str, _: Entity| -> f32 { scalars.get(name).copied().unwrap_or(0.0) };
    let has_marker = |_: &str, _: Entity| false;
    let entity_position = |_: Entity| -> Option<Position> { None };
    let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
    // Shadow-foxes have no Maslow ladder — flat survival tier.
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

    #[test]
    fn starving_night_hunt_scores_high_fed_scores_low() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);

        let starving = ShadowfoxScoringContext {
            hunger_urgency: 1.0,
            satiation: 0.0,
            cat_in_scan: true,
            night_scalar: 1.0,
            best_cat_ambush_affordance: 0.0,
            den_distance_norm: 0.0,
            self_position: Position::new(10, 10),
        };
        let fed = ShadowfoxScoringContext {
            hunger_urgency: 0.0,
            satiation: 1.0,
            ..starving_clone(&starving)
        };
        let s_hunt = score_shadowfox_dse_by_id("shadowfox_hunt", &starving, &inputs);
        let f_hunt = score_shadowfox_dse_by_id("shadowfox_hunt", &fed, &inputs);
        assert!(
            s_hunt > 0.5,
            "starving night hunt should score decisively; got {s_hunt}"
        );
        assert!(
            s_hunt > f_hunt,
            "hunger must order the hunt score ({s_hunt} vs {f_hunt})"
        );
    }

    fn starving_clone(c: &ShadowfoxScoringContext) -> ShadowfoxScoringContext {
        ShadowfoxScoringContext {
            hunger_urgency: c.hunger_urgency,
            satiation: c.satiation,
            cat_in_scan: c.cat_in_scan,
            night_scalar: c.night_scalar,
            best_cat_ambush_affordance: c.best_cat_ambush_affordance,
            den_distance_norm: c.den_distance_norm,
            self_position: c.self_position,
        }
    }

    #[test]
    fn retreat_orders_by_satiation_and_distance() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);

        let fed_far = ShadowfoxScoringContext {
            hunger_urgency: 0.0,
            satiation: 1.0,
            cat_in_scan: false,
            night_scalar: 0.2,
            best_cat_ambush_affordance: 0.0,
            den_distance_norm: 1.0,
            self_position: Position::new(10, 10),
        };
        let hungry_near = ShadowfoxScoringContext {
            satiation: 0.0,
            hunger_urgency: 1.0,
            den_distance_norm: 0.0,
            ..starving_clone(&fed_far)
        };
        let high = score_shadowfox_dse_by_id("shadowfox_retreat", &fed_far, &inputs);
        let low = score_shadowfox_dse_by_id("shadowfox_retreat", &hungry_near, &inputs);
        assert!(
            high > 0.5,
            "fed-and-far retreat should be decisive; got {high}"
        );
        assert!(high > low);
    }

    #[test]
    fn patrol_scores_low_and_nocturnal() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);

        let night = ShadowfoxScoringContext {
            hunger_urgency: 0.0,
            satiation: 1.0,
            cat_in_scan: false,
            night_scalar: 1.0,
            best_cat_ambush_affordance: 0.0,
            den_distance_norm: 0.0,
            self_position: Position::new(10, 10),
        };
        let day = ShadowfoxScoringContext {
            night_scalar: 0.2,
            ..starving_clone(&night)
        };
        let n = score_shadowfox_dse_by_id("shadowfox_patrol", &night, &inputs);
        let d = score_shadowfox_dse_by_id("shadowfox_patrol", &day, &inputs);
        assert!(n > d, "patrol must be nocturnal ({n} vs {d})");
        assert!(n <= 0.15, "patrol is a weak default; got {n}");
    }

    #[test]
    fn unknown_dse_id_is_inert() {
        let r = registry();
        let pipeline = ModifierPipeline::default();
        let markers = MarkerSnapshot::new();
        let inputs = test_inputs!(&r, &pipeline, &markers);
        let ctx = ShadowfoxScoringContext {
            hunger_urgency: 1.0,
            satiation: 0.0,
            cat_in_scan: true,
            night_scalar: 1.0,
            best_cat_ambush_affordance: 1.0,
            den_distance_norm: 1.0,
            self_position: Position::new(10, 10),
        };
        assert_eq!(
            score_shadowfox_dse_by_id("shadowfox_nonexistent", &ctx, &inputs),
            0.0
        );
    }
}
