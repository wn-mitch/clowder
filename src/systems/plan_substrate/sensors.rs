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
//!   `target_failure_cooldown_ticks`.
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
use crate::ai::planner::GoapActionKind;
use crate::components::physical::Dead;
use crate::components::{DispositionKind, RecentDispositionFailures, RecentTargetFailures};
use crate::resources::sim_constants::SimConstants;

/// Compute the recently-failed-target signal for a given
/// `(action, target)` lookup.
///
/// Semantics: **1.0 = no penalty**, **0.0 = full penalty (just
/// failed)**. Scoring is fail-open — a missing `RecentTargetFailures`
/// component (cat that never hit a target failure) returns 1.0, as
/// does a missing entry or an expired one.
///
/// Linear ramp: at `age = 0` returns 0.0; at `age >= cooldown_ticks`
/// returns 1.0; otherwise `age / cooldown_ticks`. Defensive against
/// `cooldown_ticks == 0` (returns 1.0 — a zero-cooldown means "no
/// memory", and the consideration should be a no-op).
pub fn target_recent_failure_age_normalized(
    recent: Option<&RecentTargetFailures>,
    action: GoapActionKind,
    target: Entity,
    now: u64,
    cooldown_ticks: u64,
) -> f32 {
    if cooldown_ticks == 0 {
        return 1.0;
    }
    let Some(recent) = recent else {
        return 1.0;
    };
    let Some(failed_tick) = recent.last_failure_tick(action, target) else {
        return 1.0;
    };
    let age = now.saturating_sub(failed_tick);
    if age >= cooldown_ticks {
        return 1.0;
    }
    (age as f32 / cooldown_ticks as f32).clamp(0.0, 1.0)
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

/// Bound per-cat `RecentTargetFailures` map size by expiring entries
/// older than `target_failure_cooldown_ticks`. Slotted into chain 2a's
/// decay batch alongside `decay_grooming` / `decay_exploration` so the
/// substrate-owned per-cat data structures all share a single
/// passive-decay lane.
///
/// Skipped on `Dead` cats (the pruner is a per-tick visit; a freshly-
/// dead cat's component will be cleaned up by `cleanup_dead`).
pub fn prune_recent_target_failures(
    constants: Res<SimConstants>,
    time: Res<crate::resources::time::TimeState>,
    mut query: Query<&mut RecentTargetFailures, Without<Dead>>,
) {
    let cooldown = constants.planning_substrate.target_failure_cooldown_ticks;
    if cooldown == 0 {
        return;
    }
    let now = time.tick;
    for mut recent in &mut query {
        if recent.is_empty() {
            continue;
        }
        let _removed = recent.prune_expired(now, cooldown);
    }
}

// ---------------------------------------------------------------------------
// apply_disposition_failure_cooldown — ticket 123
// ---------------------------------------------------------------------------

/// Damp the IAUS scores of dispositions that recently hit
/// `make_plan → None`. Called by `evaluate_and_plan` after
/// `score_actions` and before disposition-softmax selection. Parallel
/// in shape to the existing `apply_memory_bonuses` /
/// `apply_priority_bonus` post-scoring score-list mutators in
/// `goap.rs`, but the policy lives in `plan_substrate` because the
/// recently-failed component itself is substrate-owned.
///
/// **Why a score-list mutator and not a per-DSE Consideration:** the
/// six failure-prone dispositions (Hunting, Foraging, Crafting,
/// Caretaking, Building, Mating) span 10+ DSEs (Crafting alone has 10
/// constituents — herbcraft × 3, magic × 6, cook). Registering a
/// `ScalarConsideration` on each, with renormalized weights, would
/// touch every Crafting / Caretaking / Building DSE file for the
/// same multiplicative effect that one score-list pass produces.
/// The score-list approach also gives the correct "true multiplier"
/// semantics — a `WeightedSum` axis at weight `1/N` could only
/// produce an `(N-1)/N`-floor attenuation, never a 0.1× damp.
///
/// **Mirrors `cooldown_curve()` shape:** at fresh failure (signal
/// 0.0) damp is 0.1×; at full recovery (signal 1.0) damp is 1.0×.
/// Linear interpolation inside the cooldown window.
///
/// **Resting and Guarding are exempt** per ticket 123 §Scope. Their
/// step graphs don't share the `_ => trips_done >= target_trips`
/// family; a `make_plan → None` outcome on those dispositions is
/// either rare or carries different semantics (e.g., Guard with no
/// threat → Idle).
pub fn apply_disposition_failure_cooldown(
    scores: &mut [(crate::ai::Action, f32)],
    recent: Option<&RecentDispositionFailures>,
    now: u64,
    cooldown_ticks: u64,
) {
    if cooldown_ticks == 0 {
        return;
    }
    for (action, score) in scores.iter_mut() {
        let Some(kind) = DispositionKind::from_action(*action) else {
            continue;
        };
        if !is_failure_prone_disposition(kind) {
            continue;
        }
        let signal = disposition_recent_failure_age_normalized(
            recent,
            kind,
            now,
            cooldown_ticks,
        );
        if signal >= 1.0 {
            continue;
        }
        // Damp curve mirrors `cooldown_curve()` exactly:
        // signal 0 → 0.1× (90% damp), signal 1 → 1.0× (no damp).
        let damp = 0.1 + 0.9 * signal;
        *score *= damp;
    }
}

/// Whitelist of dispositions whose IAUS score the disposition-failure
/// cooldown attenuates. Mirrors ticket 123 §Scope. Resting / Guarding
/// / Socializing / Farming / Coordinating / Exploring are exempt
/// because their step graphs don't share the
/// `make_plan → None` retry pattern (different completion semantics
/// or different planner-failure modes).
///
/// 154: Mentoring joins the whitelist alongside Mating — same shape
/// (single SocialTarget zone precondition; no reachable target →
/// `make_plan` fails).
fn is_failure_prone_disposition(kind: DispositionKind) -> bool {
    matches!(
        kind,
        DispositionKind::Hunting
            | DispositionKind::Foraging
            | DispositionKind::Crafting
            | DispositionKind::Caretaking
            | DispositionKind::Building
            | DispositionKind::Mating
            | DispositionKind::Mentoring
    )
}

// ---------------------------------------------------------------------------
// disposition_recent_failure_age_normalized — ticket 123
// ---------------------------------------------------------------------------

/// Compute the recently-failed-disposition signal for a given
/// `DispositionKind` lookup. Parallel in shape to
/// `target_recent_failure_age_normalized` — only the keying layer
/// differs (DispositionKind vs `(action, target)`).
///
/// Semantics: **1.0 = no penalty**, **0.0 = full penalty (just
/// failed)**. Scoring is fail-open — a missing
/// `RecentDispositionFailures` component (cat that never hit a
/// `make_plan → None` failure) returns 1.0, as does a missing entry
/// or an expired one.
///
/// Linear ramp: at `age = 0` returns 0.0; at `age >= cooldown_ticks`
/// returns 1.0; otherwise `age / cooldown_ticks`. Defensive against
/// `cooldown_ticks == 0` (returns 1.0 — a zero-cooldown means "no
/// memory", and the consideration should be a no-op).
pub fn disposition_recent_failure_age_normalized(
    recent: Option<&RecentDispositionFailures>,
    kind: DispositionKind,
    now: u64,
    cooldown_ticks: u64,
) -> f32 {
    if cooldown_ticks == 0 {
        return 1.0;
    }
    let Some(recent) = recent else {
        return 1.0;
    };
    let Some(failed_tick) = recent.last_failure_tick(kind) else {
        return 1.0;
    };
    let age = now.saturating_sub(failed_tick);
    if age >= cooldown_ticks {
        return 1.0;
    }
    (age as f32 / cooldown_ticks as f32).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// prune_recent_disposition_failures — chain 2a decay-batch maintenance system
// ---------------------------------------------------------------------------

/// Bound per-cat `RecentDispositionFailures` map size by expiring
/// entries older than `disposition_failure_cooldown_ticks`. Slotted
/// into chain 2a's decay batch next to `prune_recent_target_failures`
/// so the substrate-owned per-cat data structures all share a single
/// passive-decay lane.
///
/// Skipped on `Dead` cats (the pruner is a per-tick visit; a freshly-
/// dead cat's component will be cleaned up by `cleanup_dead`).
pub fn prune_recent_disposition_failures(
    constants: Res<SimConstants>,
    time: Res<crate::resources::time::TimeState>,
    mut query: Query<&mut RecentDispositionFailures, Without<Dead>>,
) {
    let cooldown = constants.planning_substrate.disposition_failure_cooldown_ticks;
    if cooldown == 0 {
        return;
    }
    let now = time.tick;
    for mut recent in &mut query {
        if recent.is_empty() {
            continue;
        }
        let _removed = recent.prune_expired(now, cooldown);
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
    // target_recent_failure_age_normalized
    // -----------------------------------------------------------------

    #[test]
    fn sensor_returns_one_when_no_recent_failures_component() {
        // Fail-open: a cat with no `RecentTargetFailures` component
        // gets the full no-penalty signal.
        let s = target_recent_failure_age_normalized(
            None,
            GoapActionKind::SocializeWith,
            entity(10),
            1000,
            8000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn sensor_returns_one_when_no_entry_for_pair() {
        let recent = RecentTargetFailures::default();
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::SocializeWith,
            entity(10),
            1000,
            8000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn sensor_returns_zero_at_fresh_failure() {
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        recent.record(GoapActionKind::SocializeWith, target, 1000);
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::SocializeWith,
            target,
            1000, // age = 0
            8000,
        );
        assert_eq!(s, 0.0);
    }

    #[test]
    fn sensor_returns_one_at_full_cooldown_age() {
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        recent.record(GoapActionKind::SocializeWith, target, 1000);
        // age = 8000 → fully expired → 1.0
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::SocializeWith,
            target,
            9000,
            8000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn sensor_returns_half_at_midpoint() {
        // Spec contract: sensor returns `(now - failed_tick) /
        // cooldown_ticks` clamped to `[0, 1]`.
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        recent.record(GoapActionKind::SocializeWith, target, 1000);
        // age = 4000, cooldown = 8000 → 0.5
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::SocializeWith,
            target,
            5000,
            8000,
        );
        assert!((s - 0.5).abs() < 1e-6, "expected 0.5, got {}", s);
    }

    #[test]
    fn sensor_clamps_age_beyond_cooldown_to_one() {
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        recent.record(GoapActionKind::SocializeWith, target, 1000);
        // age = 50_000, cooldown = 8000 → 1.0 (saturation)
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::SocializeWith,
            target,
            51_000,
            8000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn sensor_handles_zero_cooldown_defensively() {
        // Zero cooldown means "no memory" — sensor returns 1.0 (no
        // penalty) regardless of recorded failures, so the
        // consideration becomes a no-op.
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        recent.record(GoapActionKind::SocializeWith, target, 1000);
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::SocializeWith,
            target,
            1000,
            0,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn sensor_distinguishes_action_kinds() {
        let mut recent = RecentTargetFailures::default();
        let target = entity(10);
        recent.record(GoapActionKind::SocializeWith, target, 1000);
        // Same target, different action → no entry → 1.0
        let s = target_recent_failure_age_normalized(
            Some(&recent),
            GoapActionKind::GroomOther,
            target,
            1500,
            8000,
        );
        assert_eq!(s, 1.0);
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
    // disposition_recent_failure_age_normalized (ticket 123)
    // -----------------------------------------------------------------

    #[test]
    fn disposition_sensor_no_component_returns_one() {
        let s = disposition_recent_failure_age_normalized(
            None,
            DispositionKind::Hunting,
            1000,
            4000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_no_entry_returns_one() {
        let recent = RecentDispositionFailures::default();
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Hunting,
            1000,
            4000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_fresh_failure_returns_zero() {
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Crafting, 1000);
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Crafting,
            1000,
            4000,
        );
        assert_eq!(s, 0.0);
    }

    #[test]
    fn disposition_sensor_full_window_returns_one() {
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Foraging, 1000);
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Foraging,
            5000,
            4000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_midpoint_returns_half() {
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Hunting, 1000);
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Hunting,
            3000, // age 2000 of 4000 = 0.5
            4000,
        );
        assert!((s - 0.5).abs() < 1e-6);
    }

    #[test]
    fn disposition_sensor_zero_cooldown_returns_one() {
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Hunting, 1000);
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Hunting,
            1000,
            0,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_distinguishes_kinds() {
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Crafting, 1000);
        // Different kind → no entry → fail-open
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Hunting,
            1500,
            4000,
        );
        assert_eq!(s, 1.0);
    }

    #[test]
    fn disposition_sensor_handles_clock_rewind_defensively() {
        // saturating_sub keeps age at 0 when now < failed_tick — entry
        // is treated as fresh-failure (signal 0.0), not as expired.
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Foraging, 5000);
        let s = disposition_recent_failure_age_normalized(
            Some(&recent),
            DispositionKind::Foraging,
            100,
            4000,
        );
        assert_eq!(s, 0.0);
    }

    // -----------------------------------------------------------------
    // apply_disposition_failure_cooldown (ticket 123)
    // -----------------------------------------------------------------

    #[test]
    fn apply_cooldown_no_failures_leaves_scores_unchanged() {
        use crate::ai::Action;
        let mut scores = vec![(Action::Hunt, 0.7), (Action::Forage, 0.5)];
        apply_disposition_failure_cooldown(&mut scores, None, 1000, 4000);
        assert_eq!(scores[0].1, 0.7);
        assert_eq!(scores[1].1, 0.5);
    }

    #[test]
    fn apply_cooldown_zero_window_is_noop() {
        use crate::ai::Action;
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Hunting, 1000);
        let mut scores = vec![(Action::Hunt, 0.7)];
        apply_disposition_failure_cooldown(&mut scores, Some(&recent), 1000, 0);
        assert_eq!(scores[0].1, 0.7);
    }

    #[test]
    fn apply_cooldown_damps_freshly_failed_disposition_to_floor() {
        use crate::ai::Action;
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Hunting, 1000);
        let mut scores = vec![(Action::Hunt, 1.0)];
        apply_disposition_failure_cooldown(&mut scores, Some(&recent), 1000, 4000);
        // Fresh failure: signal = 0.0 → damp = 0.1 → score = 0.1.
        assert!((scores[0].1 - 0.1).abs() < 1e-6);
    }

    #[test]
    fn apply_cooldown_recovers_to_full_after_window() {
        use crate::ai::Action;
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Hunting, 1000);
        let mut scores = vec![(Action::Hunt, 1.0)];
        // age 5000 > cooldown 4000 → signal 1.0 → damp 1.0 → no change.
        apply_disposition_failure_cooldown(&mut scores, Some(&recent), 6000, 4000);
        assert!((scores[0].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn apply_cooldown_at_midpoint_damps_to_55_percent() {
        use crate::ai::Action;
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Crafting, 1000);
        let mut scores = vec![(Action::Herbcraft, 1.0)];
        // age 2000 of 4000 → signal 0.5 → damp = 0.1 + 0.45 = 0.55.
        apply_disposition_failure_cooldown(&mut scores, Some(&recent), 3000, 4000);
        assert!((scores[0].1 - 0.55).abs() < 1e-6);
    }

    #[test]
    fn apply_cooldown_skips_exempt_dispositions() {
        use crate::ai::Action;
        let mut recent = RecentDispositionFailures::default();
        // Even if Resting / Guarding were somehow keyed (the authoring
        // site only writes failure-prone dispositions, but a stale or
        // future-keyed entry shouldn't damp them), they pass through.
        recent.record(DispositionKind::Resting, 1000);
        let mut scores = vec![
            (Action::Sleep, 0.9),
            (Action::Patrol, 0.5),
            (Action::Coordinate, 0.4),
        ];
        apply_disposition_failure_cooldown(&mut scores, Some(&recent), 1000, 4000);
        assert_eq!(scores[0].1, 0.9, "Resting/Sleep is exempt");
        assert_eq!(scores[1].1, 0.5, "Guarding/Patrol is exempt");
        assert_eq!(scores[2].1, 0.4, "Coordinating is exempt");
    }

    #[test]
    fn apply_cooldown_damps_all_constituent_actions_of_failed_disposition() {
        use crate::ai::Action;
        let mut recent = RecentDispositionFailures::default();
        recent.record(DispositionKind::Crafting, 1000);
        // Crafting has 3 constituent Action enums: Herbcraft,
        // PracticeMagic, Cook. All three should damp on a Crafting
        // failure (the cooldown attaches at the disposition level,
        // not the action level).
        let mut scores = vec![
            (Action::Herbcraft, 1.0),
            (Action::PracticeMagic, 0.5),
            (Action::Cook, 0.8),
        ];
        apply_disposition_failure_cooldown(&mut scores, Some(&recent), 1000, 4000);
        assert!((scores[0].1 - 0.1).abs() < 1e-6);
        assert!((scores[1].1 - 0.05).abs() < 1e-6);
        assert!((scores[2].1 - 0.08).abs() < 1e-6);
    }
}
