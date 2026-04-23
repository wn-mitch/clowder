//! §3.5 post-scoring modifiers (`docs/systems/ai-substrate-refactor.md`).
//!
//! A `ScoreModifier` is a pure post-composition pass: given a DSE's id,
//! its gated score, the cat's eval context, and the canonical scalar
//! fetcher, it returns a transformed score. The pipeline applies every
//! registered modifier in registration order — ch 13 §"Layered
//! Weighting Models / Propagation of Change" calls this the filter-stage
//! shape.
//!
//! Phase 4.2 ports the Herbcraft / PracticeMagic emergency-bonus
//! retargets named in `docs/open-work.md` #14 out of the inline
//! `score_actions` block at `scoring.rs:576–712` into first-class
//! `ScoreModifier` implementations:
//!
//! - [`WardCorruptionEmergency`] — boosts ward-setting DSEs when the
//!   colony has low ward coverage *and* territory corruption is
//!   detected. Applies to `herbcraft_gather` (pre-gather
//!   thornbriar), `herbcraft_ward`, and `magic_durable_ward`.
//! - [`CleanseEmergency`] — boosts corruption-cleansing DSEs when
//!   territory corruption is detected. Applies to `magic_cleanse`
//!   and `magic_colony_cleanse`.
//! - [`SensedRotBoost`] — proactive boost to ward / colony-cleanse
//!   DSEs when the cat smells corruption on nearby tiles even before
//!   standing on it. Applies to `magic_durable_ward` and
//!   `magic_colony_cleanse`.
//!
//! The six §3.5.1 foundational modifiers (Pride, Independence solo /
//! group, Patience, Tradition, Fox-suppression, Corruption-suppression)
//! remain inline in `score_actions` today — they are next on the §3.5
//! port ledger but not required for the Phase 3 exit-soak's
//! PracticeMagic regression fix.

use bevy::prelude::Entity;

use crate::ai::dse::{DseId, EvalCtx};
use crate::ai::eval::{ModifierPipeline, ScoreModifier};
use crate::resources::sim_constants::ScoringConstants;

// ---------------------------------------------------------------------------
// Scalar keys
// ---------------------------------------------------------------------------
//
// The three modifiers below read their trigger inputs through the
// canonical scalar surface (`ctx_scalars` in `scoring.rs`). Keys are
// duplicated here as `&'static str` constants so drift between modifier
// triggers and `ctx_scalars` producers is visible at grep time.

const WARD_DEFICIT: &str = "ward_deficit";
const TERRITORY_MAX_CORRUPTION: &str = "territory_max_corruption";
const NEARBY_CORRUPTION_LEVEL: &str = "nearby_corruption_level";
const MASLOW_L2_SUPPRESSION: &str = "maslow_level_2_suppression";
const HAS_HERBS_NEARBY: &str = "has_herbs_nearby";
const HAS_WARD_HERBS: &str = "has_ward_herbs";
const THORNBRIAR_AVAILABLE: &str = "thornbriar_available";

// ---------------------------------------------------------------------------
// DSE ids the modifiers target
// ---------------------------------------------------------------------------

const HERBCRAFT_GATHER: &str = "herbcraft_gather";
const HERBCRAFT_WARD: &str = "herbcraft_ward";
const MAGIC_DURABLE_WARD: &str = "magic_durable_ward";
const MAGIC_CLEANSE: &str = "magic_cleanse";
const MAGIC_COLONY_CLEANSE: &str = "magic_colony_cleanse";

// ---------------------------------------------------------------------------
// WardCorruptionEmergency
// ---------------------------------------------------------------------------

/// Port of `ward_corruption_emergency_bonus` from the retiring inline
/// Herbcraft/PracticeMagic blocks.
///
/// **Trigger:** `ward_deficit > 0 && territory_max_corruption > 0`.
///
/// **Transform:** additive `ward_corruption_emergency_bonus *
/// maslow_level_2_suppression`. For `herbcraft_gather` the trigger is
/// narrowed further to `has_herbs_nearby && !has_ward_herbs &&
/// thornbriar_available` — matching the inline `gather_emergency`
/// pre-gate that only boosted gather-for-ward when harvestable
/// thornbriar was reachable and no ward herbs were already held.
///
/// **Applies to:** `herbcraft_gather`, `herbcraft_ward`,
/// `magic_durable_ward`.
pub struct WardCorruptionEmergency {
    bonus: f32,
}

impl WardCorruptionEmergency {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.ward_corruption_emergency_bonus,
        }
    }
}

impl ScoreModifier for WardCorruptionEmergency {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        // Skip DSEs that aren't ward-related — no work to do.
        let targets_ward_action = matches!(
            dse_id.0,
            HERBCRAFT_GATHER | HERBCRAFT_WARD | MAGIC_DURABLE_WARD
        );
        if !targets_ward_action {
            return score;
        }
        // Score of zero means the outer scoring-layer gate suppressed
        // this DSE entirely (e.g. `has_herbs_nearby` false for gather).
        // Emergency bonuses don't resurrect a suppressed DSE — they
        // boost one that's already eligible to compete.
        if score <= 0.0 {
            return score;
        }

        let ward_deficit = fetch(WARD_DEFICIT, ctx.cat);
        let territory_max = fetch(TERRITORY_MAX_CORRUPTION, ctx.cat);
        if ward_deficit <= 0.0 || territory_max <= 0.0 {
            return score;
        }

        // For `herbcraft_gather`, apply the narrower pre-gate that the
        // inline block enforced: only boost if thornbriar is harvestable
        // and the cat doesn't already have ward herbs in hand.
        if dse_id.0 == HERBCRAFT_GATHER {
            let has_herbs = fetch(HAS_HERBS_NEARBY, ctx.cat) > 0.5;
            let has_ward_herbs = fetch(HAS_WARD_HERBS, ctx.cat) > 0.5;
            let thornbriar = fetch(THORNBRIAR_AVAILABLE, ctx.cat) > 0.5;
            if !(has_herbs && !has_ward_herbs && thornbriar) {
                return score;
            }
        }

        let suppression = fetch(MASLOW_L2_SUPPRESSION, ctx.cat);
        score + self.bonus * suppression
    }

    fn name(&self) -> &'static str {
        "ward_corruption_emergency"
    }
}

// ---------------------------------------------------------------------------
// CleanseEmergency
// ---------------------------------------------------------------------------

/// Port of `cleanse_corruption_emergency_bonus` from the retiring
/// inline PracticeMagic block.
///
/// **Trigger:** `territory_max_corruption > 0`.
///
/// **Transform:** additive `cleanse_corruption_emergency_bonus *
/// maslow_level_2_suppression`.
///
/// **Applies to:** `magic_cleanse`, `magic_colony_cleanse`. The inline
/// `magic_cleanse` path has an outer eligibility gate
/// (`on_corrupted_tile && tile_corruption > threshold`) that remains
/// in the scoring layer — a DSE suppressed by the gate scores 0 and
/// the modifier's `score <= 0.0` short-circuit skips it.
pub struct CleanseEmergency {
    bonus: f32,
}

impl CleanseEmergency {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.cleanse_corruption_emergency_bonus,
        }
    }
}

impl ScoreModifier for CleanseEmergency {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, MAGIC_CLEANSE | MAGIC_COLONY_CLEANSE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        if fetch(TERRITORY_MAX_CORRUPTION, ctx.cat) <= 0.0 {
            return score;
        }
        let suppression = fetch(MASLOW_L2_SUPPRESSION, ctx.cat);
        score + self.bonus * suppression
    }

    fn name(&self) -> &'static str {
        "cleanse_emergency"
    }
}

// ---------------------------------------------------------------------------
// SensedRotBoost
// ---------------------------------------------------------------------------

/// Port of `corruption_sensed_response_bonus` from the retiring inline
/// PracticeMagic block.
///
/// **Trigger:** `nearby_corruption_level > 0.1`. The 0.1 floor matches
/// the inline gate's lower bound; below that, scent is treated as
/// ambient drift rather than a response-worthy signal.
///
/// **Transform:** additive `corruption_sensed_response_bonus *
/// nearby_corruption_level * maslow_level_2_suppression`. Scales with
/// how much rot the cat can smell — the modifier reads the raw
/// `nearby_corruption_level` scalar (0-1 float) rather than a thresholded
/// gate, so the boost rises smoothly with nearby corruption intensity.
///
/// **Applies to:** `magic_durable_ward`, `magic_colony_cleanse`. Gives
/// the cat a proactive response trigger before stepping onto
/// corruption (which the standing-on-it `CleanseEmergency` would
/// require).
pub struct SensedRotBoost {
    scale: f32,
}

impl SensedRotBoost {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            scale: sc.corruption_sensed_response_bonus,
        }
    }
}

impl ScoreModifier for SensedRotBoost {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, MAGIC_DURABLE_WARD | MAGIC_COLONY_CLEANSE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        let nearby = fetch(NEARBY_CORRUPTION_LEVEL, ctx.cat);
        if nearby <= 0.1 {
            return score;
        }
        let suppression = fetch(MASLOW_L2_SUPPRESSION, ctx.cat);
        score + self.scale * nearby * suppression
    }

    fn name(&self) -> &'static str {
        "sensed_rot_boost"
    }
}

// ---------------------------------------------------------------------------
// Default pipeline builder
// ---------------------------------------------------------------------------

/// Build the Phase 4.2 modifier pipeline: the three corruption-response
/// emergency-bonus retargets ported out of the retiring inline
/// `score_actions` block. Registration order is fixed — emergency
/// bonuses compose by addition and are order-invariant, but pinning
/// order makes future audits grep-able.
///
/// Mirror sites — `src/plugins/simulation.rs`, `src/main.rs`
/// `setup_world` + `run_new_game`, save-load restore — each call
/// this helper to produce the same pipeline shape.
pub fn default_modifier_pipeline(sc: &ScoringConstants) -> ModifierPipeline {
    let mut pipeline = ModifierPipeline::new();
    pipeline.push(Box::new(WardCorruptionEmergency::new(sc)));
    pipeline.push(Box::new(CleanseEmergency::new(sc)));
    pipeline.push(Box::new(SensedRotBoost::new(sc)));
    pipeline
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Position;

    fn test_ctx() -> (Entity, EvalCtx<'static>) {
        // Static closures satisfy `EvalCtx<'ctx>` lifetime where we don't
        // need real per-tick access.
        static MARKER: fn(&str, Entity) -> bool = |_, _| false;
        static SAMPLE: fn(&str, Position) -> f32 = |_, _| 0.0;
        let entity = Entity::from_raw_u32(1).unwrap();
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &SAMPLE,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        (entity, ctx)
    }

    #[test]
    fn ward_emergency_skips_non_ward_dses() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        // Not a ward DSE — no transform.
        let out = modifier.apply(DseId("eat"), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ward_emergency_fires_on_herbcraft_ward_when_corruption_present() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 0.8,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_WARD), 0.3, &ctx, &fetch);
        // 0.3 + 1.0 × 0.8 = 1.1
        assert!((out - 1.1).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn ward_emergency_respects_gather_narrow_gate() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        // Gather fires only if has_herbs_nearby && !has_ward_herbs &&
        // thornbriar_available.
        let without_thornbriar = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            HAS_HERBS_NEARBY => 1.0,
            HAS_WARD_HERBS => 0.0,
            THORNBRIAR_AVAILABLE => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_GATHER), 0.4, &ctx, &without_thornbriar);
        assert!((out - 0.4).abs() < 1e-6, "no thornbriar ⇒ no boost");

        let with_gate_open = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            HAS_HERBS_NEARBY => 1.0,
            HAS_WARD_HERBS => 0.0,
            THORNBRIAR_AVAILABLE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_GATHER), 0.4, &ctx, &with_gate_open);
        assert!((out - 1.4).abs() < 1e-6, "gate open ⇒ boost; got {out}");
    }

    #[test]
    fn ward_emergency_skips_when_no_territory_corruption() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.0,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_WARD), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn cleanse_emergency_fires_on_both_cleanse_dses() {
        let modifier = CleanseEmergency { bonus: 0.6 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TERRITORY_MAX_CORRUPTION => 0.4,
            MASLOW_L2_SUPPRESSION => 0.5,
            _ => 0.0,
        };
        for dse in [MAGIC_CLEANSE, MAGIC_COLONY_CLEANSE] {
            let out = modifier.apply(DseId(dse), 0.2, &ctx, &fetch);
            assert!((out - (0.2 + 0.6 * 0.5)).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn cleanse_emergency_does_not_resurrect_suppressed_dse() {
        // Outer scoring gate suppressed the DSE (e.g. cleanse on non-
        // corrupted tile → score = 0). The modifier must not revive it.
        let modifier = CleanseEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(MAGIC_CLEANSE), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn sensed_rot_scales_with_nearby_level() {
        let modifier = SensedRotBoost { scale: 1.0 };
        let (_, ctx) = test_ctx();
        let make_fetch = |nearby: f32| {
            move |name: &str, _: Entity| match name {
                NEARBY_CORRUPTION_LEVEL => nearby,
                MASLOW_L2_SUPPRESSION => 1.0,
                _ => 0.0,
            }
        };
        // Below 0.1 floor → no boost.
        let f = make_fetch(0.05);
        let out = modifier.apply(DseId(MAGIC_DURABLE_WARD), 0.3, &ctx, &f);
        assert!((out - 0.3).abs() < 1e-6);

        // 0.5 nearby corruption → 0.3 + 1.0 × 0.5 × 1.0 = 0.8
        let f = make_fetch(0.5);
        let out = modifier.apply(DseId(MAGIC_DURABLE_WARD), 0.3, &ctx, &f);
        assert!((out - 0.8).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn sensed_rot_skips_unrelated_dses() {
        let modifier = SensedRotBoost { scale: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            NEARBY_CORRUPTION_LEVEL => 0.9,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId("hunt"), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }
}
