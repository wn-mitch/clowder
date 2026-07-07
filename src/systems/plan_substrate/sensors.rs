//! IAUS sensors and maintenance systems for the planning substrate
//! (ticket 073, sub-epic 071).
//!
//! ## What's here
//!
//! - [`target_recent_failure_age_normalized`] — pure sensor that maps
//!   a `(now, last_failure_tick, cooldown_ticks)` tuple to a `[0, 1]`
//!   signal. Read by the `target_recent_failure` Consideration on the
//!   six target-taking DSEs.
//! - [`cooldown_curve`] — the canonical Piecewise curve consumed by
//!   the same Consideration. Knots `[(0.0, 0.1), (1.0, 1.0)]`: a fresh
//!   failure (signal 0.0) multiplies the candidate's product score by
//!   0.1, recovering linearly to 1.0 over the cooldown window.
//! - [`prune_recent_target_failures`] — chain-2a maintenance system
//!   that bounds per-cat map size by expiring entries older than
//!   the `belief_facets.predictability` decay tunables (292).
//!
//! ## Architectural guardrail
//!
//! The "machined gears" doctrine (sub-epic 071): cross-tick defenses
//! land *inside* the IAUS engine as a Consideration / Modifier /
//! EligibilityFilter. The cooldown is a `Consideration::Scalar` over
//! the `TARGET_RECENT_FAILURE_INPUT` key, **not** a post-hoc filter
//! in the resolver body. Each target DSE registers it with renormalized
//! weights so steady-state scores match pre-073 on cats with no
//! recent failures.

use bevy_ecs::prelude::*;

use crate::ai::curves::Curve;
use crate::components::beliefs::{ContextBeliefs, EnvironmentalContextKey};
#[cfg(test)]
use crate::components::beliefs::{Facet, MentalModel};
use crate::components::physical::Dead;
use crate::components::physical::Needs;
use crate::components::{DispositionKind, PrevSafetyDeficit};

/// 292 — belief-substrate target-cooldown signal: the successor to
/// [`target_recent_failure_age_normalized`]. Reads the actor's OWN
/// mental model of `target` — `CatBeliefs` for cat targets,
/// `PredatorBeliefs` for wildlife — whose `predictability` facet the
/// `belief_integrator` snaps to 0.0 on `TargetActionFailed` and
/// passively decays back toward `prior = 1.0`.
///
/// Semantics preserved from the legacy sensor: **1.0 = no penalty**,
/// **0.0 = fresh failure**, fail-open on every missing layer — no
/// beliefs component, no model of this target, or a predictability
/// facet with zero strength (a model created by other event arms
/// whose predictability was never observed) all return 1.0.
///
/// Action-agnostic by design (ticket 292 choice (a)): failing to
/// mentor cat-X now also cools socializing with cat-X for the
/// recovery window. The pre-registered pivot (b) re-keys on
/// `EnvironmentalContextKey::ActionExecution` if the hypothesize
/// pass shows the granularity loss matters. Prey / corpse /
/// structure targets have no belief home (505 ballast rule) and
/// read permanently fail-open — their churn-suppression is owned by
/// structural fixes (467 reachability gate, 514 eligibility) rather
/// than memory.
pub fn target_predictability_signal(
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    predator_beliefs: Option<&crate::components::beliefs::PredatorBeliefs>,
    target: bevy_ecs::entity::Entity,
) -> f32 {
    let model = cat_beliefs
        .and_then(|b| b.models.get(&target))
        .or_else(|| predator_beliefs.and_then(|b| b.models.get(&target)));
    let Some(model) = model else {
        return 1.0;
    };
    if model.predictability.strength <= 0.0 {
        return 1.0;
    }
    model.predictability.value.clamp(0.0, 1.0)
}

/// 264 — social belief-facet signal: the actor's own
/// `CatBeliefs[target].affiliation_history`, mapped from the facet's
/// native `[-1, 1]` range onto the consideration-curve `[0, 1]` domain
/// (`(v + 1) / 2`). Neutral-open on every missing layer — no beliefs
/// component, no model of this target, or an affiliation facet with
/// zero strength all return **0.5** (the mapped image of the 0.0
/// neutral prior), so unmodeled strangers are neither lifted nor
/// penalized relative to the axis midpoint.
pub fn affiliation_signal(
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    target: bevy_ecs::entity::Entity,
) -> f32 {
    let Some(model) = cat_beliefs.and_then(|b| b.models.get(&target)) else {
        return 0.5;
    };
    if model.affiliation_history.strength <= 0.0 {
        return 0.5;
    }
    ((model.affiliation_history.value + 1.0) * 0.5).clamp(0.0, 1.0)
}

/// 264 — social belief-facet signal: the actor's own
/// `CatBeliefs[target].perceived_hostility` (fast aggressive-intent
/// read, `[0, 1]`). Fail-open at **0.0** — no beliefs component, no
/// model, or a zero-strength facet mean "no perceived hostility", so
/// consumers with an inverted curve apply no penalty to unmodeled
/// targets.
pub fn perceived_hostility_signal(
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    target: bevy_ecs::entity::Entity,
) -> f32 {
    let Some(model) = cat_beliefs.and_then(|b| b.models.get(&target)) else {
        return 0.0;
    };
    if model.perceived_hostility.strength <= 0.0 {
        return 0.0;
    }
    model.perceived_hostility.value.clamp(0.0, 1.0)
}

/// 264 — social belief-facet signal: the actor's own
/// `CatBeliefs[target].perceived_receptivity` (is the partner open to
/// affiliative overtures *right now*, `[0, 1]`). Neutral-open at
/// **0.5** — no beliefs component, no model, or a zero-strength facet
/// mean "receptivity unknown", so unmodeled partners are neither
/// lifted nor penalized relative to the axis midpoint (a 0.0 default
/// would bias Mate away from never-observed partners, which is the
/// 027 supply-chain failure mode this axis exists to relieve).
pub fn perceived_receptivity_signal(
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    target: bevy_ecs::entity::Entity,
) -> f32 {
    let Some(model) = cat_beliefs.and_then(|b| b.models.get(&target)) else {
        return 0.5;
    };
    if model.perceived_receptivity.strength <= 0.0 {
        return 0.5;
    }
    model.perceived_receptivity.value.clamp(0.0, 1.0)
}

/// 264 — Care belief-facet signal: the actor's own
/// `CatBeliefs[target].perceived_injury_level` (`[0, 1]`). Fail-open
/// at **0.0** — no beliefs component, no model, or a zero-strength
/// facet mean "no belief of injury", so an unmodeled patient gets no
/// triage lift (the perceived-severity analog of the raw
/// `1 − health_fraction` deficit reading 0.0 for the unhurt).
pub fn perceived_injury_signal(
    cat_beliefs: Option<&crate::components::beliefs::CatBeliefs>,
    target: bevy_ecs::entity::Entity,
) -> f32 {
    let Some(model) = cat_beliefs.and_then(|b| b.models.get(&target)) else {
        return 0.0;
    };
    if model.perceived_injury_level.strength <= 0.0 {
        return 0.0;
    }
    model.perceived_injury_level.value.clamp(0.0, 1.0)
}

/// Build the canonical cooldown curve consumed by the
/// `target_recent_failure` Consideration. Knots
/// `[(0.0, 0.1), (1.0, 1.0)]`: a fresh failure scales the candidate's
/// product score by 0.1; recovery is linear over the cooldown window.
///
/// Construction returns a fresh `Curve` each call (no shared state) —
/// each DSE factory pulls its own copy when registering the
/// consideration.
pub fn cooldown_curve() -> Curve {
    crate::ai::curves::piecewise(vec![(0.0, 0.1), (1.0, 1.0)])
}

// ---------------------------------------------------------------------------
// prune_recent_target_failures — chain 2a decay-batch maintenance system
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// disposition_cooldown_signal — ticket 123 (290 reader cutover from RDF)
// ---------------------------------------------------------------------------

/// Compute the disposition-cooldown signal for a given `DispositionKind`
/// lookup. Reads `ContextBeliefs[DispositionExecution(kind)].predictability`,
/// the per-cat self-belief facet populated by `belief_integrator` on
/// `WitnessableEvent::SelfPlanFailed`.
///
/// Semantics: **1.0 = no penalty**, **0.0 = full penalty (just failed)**.
/// Scoring is fail-open — a missing `ContextBeliefs` component, a missing
/// `DispositionExecution(kind)` entry, or a predictability facet that has
/// fully decayed back to the prior all return 1.0.
///
/// The shape is an EMA of past failures (drop toward 0 on observed failure,
/// passive decay toward `prior = 1.0` between failures), not the legacy
/// linear age ramp. Tunables live at `sim_constants.belief_facets.predictability`:
/// `learning_rate = 1.0` preserves the legacy snap-to-0 contract on a single
/// failure (matches RDF's `age = 0 → 0.0` semantics); `decay_rate_to_prior`
/// scales the recovery window (~3000 ticks under default settings).
pub fn disposition_cooldown_signal(beliefs: Option<&ContextBeliefs>, kind: DispositionKind) -> f32 {
    let Some(beliefs) = beliefs else {
        return 1.0;
    };
    let key = EnvironmentalContextKey::DispositionExecution(kind);
    let Some(model) = beliefs.models.get(&key) else {
        return 1.0;
    };
    model.predictability.value.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// update_prev_safety_deficit — ticket 108 maintenance system
// ---------------------------------------------------------------------------

/// Snapshot the cat's current `safety_deficit = 1 - needs.safety` into
/// `PrevSafetyDeficit` so next tick's `evaluate_and_plan` /
/// `evaluate_dispositions` ScoringContext builders can compute
/// `threat_proximity_derivative = max(0, now - prev)` against last
/// tick's value.
///
/// **Schedule placement matters:** registered `.after(evaluate_and_plan)`
/// in `SimulationPlugin::build` so the writeback runs once the scoring
/// pass has read the prev-value. If this ran before scoring, the
/// derivative would always be `now - now = 0`.
///
/// **Lazy-insert path** for save-loaded cats (pre-108 saves) and any
/// other case where the bundle insert at `spawn_cat_from_blueprint`
/// didn't fire: when `Option<&mut PrevSafetyDeficit>` resolves to
/// `None`, we'd need a `Commands` write to insert. Phase 2 keeps the
/// system pure (no `Commands`) — save-loaded cats see one tick of
/// derivative = 0 (the ScoringContext's `prev = now` fallback) and
/// then get the component via the cat-spawn path on subsequent saves.
/// If the lazy-insert latency causes a measurable miss-window in
/// production, follow-up adds `Commands` here.
///
/// Skipped on `Dead` cats (the snapshot is a per-tick visit; a freshly-
/// dead cat's component will be cleaned up by `cleanup_dead`).
pub fn update_prev_safety_deficit(
    mut query: Query<(&Needs, &mut PrevSafetyDeficit), Without<Dead>>,
) {
    for (needs, mut prev) in &mut query {
        let now = (1.0 - needs.safety).clamp(0.0, 1.0);
        prev.0 = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::curves::Curve;

    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

    // -----------------------------------------------------------------
    // target_predictability_signal (292 — belief successor)
    // -----------------------------------------------------------------

    #[test]
    fn predictability_signal_fails_open_without_components() {
        assert_eq!(target_predictability_signal(None, None, entity(9)), 1.0);
    }

    #[test]
    fn predictability_signal_fails_open_without_model() {
        let beliefs = crate::components::beliefs::CatBeliefs::default();
        assert_eq!(
            target_predictability_signal(Some(&beliefs), None, entity(9)),
            1.0
        );
    }

    #[test]
    fn predictability_signal_fails_open_on_zero_strength_facet() {
        // A model created by other event arms (Attack/Groom) whose
        // predictability facet was never observed: strength 0 →
        // fail-open, NOT "value 0.0 = fresh failure".
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        beliefs.models.entry(entity(9)).or_default();
        assert_eq!(
            target_predictability_signal(Some(&beliefs), None, entity(9)),
            1.0
        );
    }

    // -----------------------------------------------------------------
    // affiliation_signal / perceived_hostility_signal (264 dormant wire)
    // -----------------------------------------------------------------

    #[test]
    fn affiliation_signal_neutral_open_without_model_or_strength() {
        assert_eq!(affiliation_signal(None, entity(9)), 0.5);
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        assert_eq!(affiliation_signal(Some(&beliefs), entity(9)), 0.5);
        // Model exists but affiliation facet never observed (strength 0).
        beliefs.models.entry(entity(9)).or_default();
        assert_eq!(affiliation_signal(Some(&beliefs), entity(9)), 0.5);
    }

    #[test]
    fn affiliation_signal_maps_native_range_onto_unit_interval() {
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let model = beliefs.models.entry(entity(9)).or_default();
        model.affiliation_history = Facet {
            value: -1.0,
            strength: 1.0,
            ..Default::default()
        };
        assert_eq!(affiliation_signal(Some(&beliefs), entity(9)), 0.0);
        let model = beliefs.models.entry(entity(9)).or_default();
        model.affiliation_history.value = 1.0;
        assert_eq!(affiliation_signal(Some(&beliefs), entity(9)), 1.0);
        let model = beliefs.models.entry(entity(9)).or_default();
        model.affiliation_history.value = 0.0;
        assert_eq!(affiliation_signal(Some(&beliefs), entity(9)), 0.5);
    }

    #[test]
    fn receptivity_signal_neutral_open_without_model_or_strength() {
        assert_eq!(perceived_receptivity_signal(None, entity(9)), 0.5);
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        beliefs.models.entry(entity(9)).or_default();
        assert_eq!(perceived_receptivity_signal(Some(&beliefs), entity(9)), 0.5);
    }

    #[test]
    fn receptivity_signal_reads_observed_facet() {
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let model = beliefs.models.entry(entity(9)).or_default();
        model.perceived_receptivity = Facet {
            value: 0.2,
            strength: 0.9,
            ..Default::default()
        };
        assert!((perceived_receptivity_signal(Some(&beliefs), entity(9)) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn injury_signal_fails_open_at_zero() {
        assert_eq!(perceived_injury_signal(None, entity(9)), 0.0);
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        beliefs.models.entry(entity(9)).or_default();
        assert_eq!(perceived_injury_signal(Some(&beliefs), entity(9)), 0.0);
    }

    #[test]
    fn injury_signal_reads_observed_facet() {
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let model = beliefs.models.entry(entity(9)).or_default();
        model.perceived_injury_level = Facet {
            value: 0.7,
            strength: 0.9,
            ..Default::default()
        };
        assert!((perceived_injury_signal(Some(&beliefs), entity(9)) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn hostility_signal_fails_open_at_zero() {
        assert_eq!(perceived_hostility_signal(None, entity(9)), 0.0);
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        beliefs.models.entry(entity(9)).or_default();
        assert_eq!(perceived_hostility_signal(Some(&beliefs), entity(9)), 0.0);
    }

    #[test]
    fn hostility_signal_reads_observed_facet() {
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let model = beliefs.models.entry(entity(9)).or_default();
        model.perceived_hostility = Facet {
            value: 0.8,
            strength: 0.9,
            ..Default::default()
        };
        assert!((perceived_hostility_signal(Some(&beliefs), entity(9)) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn predictability_signal_reads_snapped_facet_as_fresh_failure() {
        let mut beliefs = crate::components::beliefs::CatBeliefs::default();
        let model = beliefs.models.entry(entity(9)).or_default();
        model.predictability = crate::components::beliefs::Facet {
            value: 0.0,
            prior: 1.0,
            strength: 1.0,
            ..Default::default()
        };
        assert_eq!(
            target_predictability_signal(Some(&beliefs), None, entity(9)),
            0.0
        );
    }

    #[test]
    fn predictability_signal_reads_wildlife_from_predator_beliefs() {
        let mut preds = crate::components::beliefs::PredatorBeliefs::default();
        let model = preds.models.entry(entity(7)).or_default();
        model.predictability = crate::components::beliefs::Facet {
            value: 0.4,
            prior: 1.0,
            strength: 0.8,
            ..Default::default()
        };
        let signal = target_predictability_signal(None, Some(&preds), entity(7));
        assert!((signal - 0.4).abs() < 1e-6);
    }

    // -----------------------------------------------------------------
    // cooldown_curve
    // -----------------------------------------------------------------

    #[test]
    fn cooldown_curve_maps_zero_to_floor() {
        // Spec contract: curve maps sensor 0.0 → 0.1.
        let c = cooldown_curve();
        let y = c.evaluate(0.0);
        assert!((y - 0.1).abs() < 1e-6, "expected 0.1, got {}", y);
    }

    #[test]
    fn cooldown_curve_maps_one_to_one() {
        // Spec contract: curve maps sensor 1.0 → 1.0.
        let c = cooldown_curve();
        let y = c.evaluate(1.0);
        assert!((y - 1.0).abs() < 1e-6, "expected 1.0, got {}", y);
    }

    #[test]
    fn cooldown_curve_maps_half_to_linear_midpoint() {
        // Spec contract: curve maps sensor 0.5 → 0.55 (linear
        // interpolation between knots).
        let c = cooldown_curve();
        let y = c.evaluate(0.5);
        assert!((y - 0.55).abs() < 1e-6, "expected 0.55, got {}", y);
    }

    #[test]
    fn cooldown_curve_clamps_below_zero_to_floor() {
        let c = cooldown_curve();
        let y = c.evaluate(-1.0);
        assert!((y - 0.1).abs() < 1e-6);
    }

    #[test]
    fn cooldown_curve_clamps_above_one_to_one() {
        let c = cooldown_curve();
        let y = c.evaluate(2.0);
        assert!((y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cooldown_curve_is_piecewise() {
        // Sanity — the spec calls for `Piecewise` knots. Other curves
        // would silently break the sensor contract.
        let c = cooldown_curve();
        assert!(matches!(c, Curve::Piecewise { .. }));
    }

    // -----------------------------------------------------------------
    // disposition_cooldown_signal (ticket 290 — RDF reader cutover)
    //
    // The sensor is a pure projection of `ContextBeliefs[Disposition
    // Execution(kind)].predictability.value`. The EMA shape (failure →
    // value drop, passive decay back toward prior=1.0) is owned by
    // `belief_integrator` and tested there. These tests assert the
    // pass-through and fail-open contract only.
    // -----------------------------------------------------------------

    fn beliefs_with_predictability(kind: DispositionKind, value: f32) -> ContextBeliefs {
        let mut beliefs = ContextBeliefs::default();
        let model = MentalModel {
            predictability: Facet {
                value,
                ..Facet::from_prior(1.0)
            },
            ..Default::default()
        };
        beliefs
            .models
            .insert(EnvironmentalContextKey::DispositionExecution(kind), model);
        beliefs
    }

    #[test]
    fn disposition_sensor_no_component_returns_one() {
        let s = disposition_cooldown_signal(None, DispositionKind::Hunting);
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_no_model_entry_returns_one() {
        let beliefs = ContextBeliefs::default();
        let s = disposition_cooldown_signal(Some(&beliefs), DispositionKind::Hunting);
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_fresh_failure_returns_zero() {
        // After a single-snap EMA step (lr=1.0, OBSERVED_FAIL=0.0) the
        // integrator leaves predictability.value at 0.0 — sensor passes
        // it through.
        let beliefs = beliefs_with_predictability(DispositionKind::Herbalism, 0.0);
        let s = disposition_cooldown_signal(Some(&beliefs), DispositionKind::Herbalism);
        assert_eq!(s, 0.0);
    }

    #[test]
    fn disposition_sensor_full_recovery_returns_one() {
        // After Pass-B decay back to prior=1.0, sensor returns 1.0.
        let beliefs = beliefs_with_predictability(DispositionKind::Foraging, 1.0);
        let s = disposition_cooldown_signal(Some(&beliefs), DispositionKind::Foraging);
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_midpoint_passes_through() {
        let beliefs = beliefs_with_predictability(DispositionKind::Hunting, 0.5);
        let s = disposition_cooldown_signal(Some(&beliefs), DispositionKind::Hunting);
        assert!((s - 0.5).abs() < 1e-6);
    }

    #[test]
    fn disposition_sensor_distinguishes_kinds() {
        // A failure on Herbalism doesn't shadow Hunting's signal — each
        // disposition gets its own `DispositionExecution(kind)` model.
        let beliefs = beliefs_with_predictability(DispositionKind::Herbalism, 0.0);
        let s = disposition_cooldown_signal(Some(&beliefs), DispositionKind::Hunting);
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_clamps_out_of_range() {
        // Defensive: an out-of-range predictability.value (caller bug or
        // truncated save data) is clamped to [0, 1] at the read site.
        let low = beliefs_with_predictability(DispositionKind::Hunting, -0.5);
        assert_eq!(
            disposition_cooldown_signal(Some(&low), DispositionKind::Hunting),
            0.0,
        );
        let high = beliefs_with_predictability(DispositionKind::Hunting, 1.5);
        assert_eq!(
            disposition_cooldown_signal(Some(&high), DispositionKind::Hunting),
            1.0,
        );
    }
}
