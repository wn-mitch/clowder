//! `StepOutcome<W>` — return type contract for every GOAP step resolver.
//!
//! # Why this exists
//!
//! Two silently-dead subsystems (Phase 4c.3 feed-kitten, Phase 4c.4 tend-
//! crops) were shipped by the same shape of bug: a step returned
//! `StepResult::Advance` even when its real-world effect didn't occur,
//! and the caller either emitted a `Feature::*` unconditionally on
//! `Advance` or emitted nothing at all. Either way the Activation canary
//! could not see the gap, so the pipelines stayed broken for weeks.
//!
//! This module makes that mistake a type error.
//!
//! Every resolver returns `StepOutcome<W>` where `W` is the *witness*:
//! the evidence that the real-world effect actually happened this call.
//! Common shapes:
//!
//! - `StepOutcome<()>` — unconditional: effect always occurs once the
//!   precondition holds (e.g., `resolve_move_to`, `resolve_sleep`). No
//!   witness. `()` deliberately does not implement [`Witnessed`], so
//!   `record_if_witnessed` is not callable on this shape.
//! - `StepOutcome<bool>` — effect may or may not occur this call (e.g.
//!   `resolve_tend` while still walking toward a garden; `resolve_cook`
//!   when inventory holds no raw food). Caller uses
//!   `record_if_witnessed` to gate the Feature emission.
//! - `StepOutcome<Option<T>>` — same as above, but the witness carries
//!   a payload the caller needs (e.g., the kitten entity to feed, the
//!   `Pregnancy` component to insert, the grooming restoration to
//!   apply in a deferred pass).
//!
//! # The gating rule
//!
//! Callers MUST NOT emit a positive `Feature::*` based on
//! `StepResult::Advance` alone. They MUST route Feature emission through
//! `record_if_witnessed`, which is only available when the witness type
//! implements [`Witnessed`] (i.e. `bool` or `Option<T>`). A resolver
//! that wants a Positive Feature is therefore forced to declare a
//! witness-bearing type at the signature level — no escape hatch short
//! of deleting this comment and writing bespoke emission code.
//!
//! # See also
//!
//! CLAUDE.md §"GOAP Step Resolver Contract" for the full docstring
//! preamble required on every `pub fn resolve_*`. Exemplars:
//! `src/steps/disposition/cook.rs`, `src/steps/disposition/feed_kitten.rs`,
//! `src/steps/building/tend.rs`.

use crate::resources::system_activation::{Feature, SystemActivation};
use crate::steps::StepResult;

/// Return type for every GOAP step resolver.
///
/// `result` drives the planner (Continue / Advance / Fail).
/// `witness` tells the caller whether the step's real-world effect
/// actually occurred this call. The two are independent — see the
/// module docs for the full rationale.
#[must_use = "a StepOutcome carries a witness — ignoring it defeats the Activation canary contract"]
#[derive(Debug)]
pub struct StepOutcome<W = ()> {
    pub result: StepResult,
    pub witness: W,
}

/// A witness type that can report whether it represents "real work
/// happened this call". Implemented for `bool` (true) and `Option<T>`
/// (is_some). Deliberately NOT implemented for `()` — witness-less
/// outcomes cannot emit Features.
pub trait Witnessed {
    fn is_witnessed(&self) -> bool;
}

impl Witnessed for bool {
    fn is_witnessed(&self) -> bool {
        *self
    }
}

impl<T> Witnessed for Option<T> {
    fn is_witnessed(&self) -> bool {
        self.is_some()
    }
}

// ---------------------------------------------------------------------------
// Generic constructors
// ---------------------------------------------------------------------------

impl<W: Default> StepOutcome<W> {
    /// An outcome whose witness is the type's default (no real-world
    /// effect this call). Usable on `()`, `bool`, and `Option<T>`.
    pub fn unwitnessed(result: StepResult) -> Self {
        Self {
            result,
            witness: W::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// StepOutcome<()> — witness-less (unconditional-effect) resolvers
// ---------------------------------------------------------------------------

impl StepOutcome<()> {
    /// Bare outcome with no witness. Use for resolvers whose effect is
    /// unconditional once the precondition holds (`resolve_move_to`,
    /// `resolve_sleep`, etc.). No `record_if_witnessed` is defined on
    /// this shape — Feature emission must route through a witness-
    /// carrying outcome.
    pub fn bare(result: StepResult) -> Self {
        Self {
            result,
            witness: (),
        }
    }
}

// ---------------------------------------------------------------------------
// StepOutcome<bool> — boolean witness
// ---------------------------------------------------------------------------

impl StepOutcome<bool> {
    /// The step's real-world effect occurred this call.
    pub fn witnessed(result: StepResult) -> Self {
        Self {
            result,
            witness: true,
        }
    }
}

// ---------------------------------------------------------------------------
// StepOutcome<Option<T>> — payload witness
// ---------------------------------------------------------------------------

impl<T> StepOutcome<Option<T>> {
    /// The step's real-world effect occurred this call, producing
    /// `payload`. Callers destructure the witness after the record
    /// call to consume the payload (e.g., append a kitten entity,
    /// insert a `Pregnancy` component, apply a grooming restoration).
    pub fn witnessed_with(result: StepResult, payload: T) -> Self {
        Self {
            result,
            witness: Some(payload),
        }
    }
}

// ---------------------------------------------------------------------------
// record_if_witnessed — the only sanctioned Feature-emission path
// ---------------------------------------------------------------------------

impl<W: Witnessed> StepOutcome<W> {
    /// Record a positive `Feature::*` ONLY if the witness reports that
    /// the real-world effect actually occurred this call. This is the
    /// only sanctioned way to emit a Feature from a step resolver —
    /// the method is not available on `StepOutcome<()>` because `()`
    /// does not implement [`Witnessed`], so the unsignalled-
    /// unconditional-Feature mistake is a type error.
    ///
    /// Typical call site:
    /// ```text
    /// let outcome = resolve_tend(...);
    /// outcome.record_if_witnessed(
    ///     narr.activation.as_deref_mut(),
    ///     Feature::CropTended,
    /// );
    /// outcome.result
    /// ```
    pub fn record_if_witnessed(&self, activation: Option<&mut SystemActivation>, feature: Feature) {
        if self.witness.is_witnessed() {
            if let Some(act) = activation {
                act.record(feature);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_outcome_has_unit_witness() {
        let o = StepOutcome::bare(StepResult::Advance);
        let _: () = o.witness;
        assert!(matches!(o.result, StepResult::Advance));
    }

    #[test]
    fn bool_witnessed_records_feature() {
        let mut sa = SystemActivation::default();
        let o = StepOutcome::<bool>::witnessed(StepResult::Advance);
        o.record_if_witnessed(Some(&mut sa), Feature::CropTended);
        assert_eq!(sa.counts.get(&Feature::CropTended).copied(), Some(1));
    }

    #[test]
    fn bool_unwitnessed_does_not_record_feature() {
        let mut sa = SystemActivation::default();
        let o = StepOutcome::<bool>::unwitnessed(StepResult::Advance);
        o.record_if_witnessed(Some(&mut sa), Feature::CropTended);
        assert_eq!(sa.counts.get(&Feature::CropTended).copied(), None);
    }

    #[test]
    fn option_witnessed_records_feature_and_carries_payload() {
        let mut sa = SystemActivation::default();
        let o = StepOutcome::<Option<u32>>::witnessed_with(StepResult::Advance, 42);
        o.record_if_witnessed(Some(&mut sa), Feature::KittenFed);
        assert_eq!(sa.counts.get(&Feature::KittenFed).copied(), Some(1));
        assert_eq!(o.witness, Some(42));
    }

    #[test]
    fn option_unwitnessed_does_not_record_feature() {
        let mut sa = SystemActivation::default();
        let o = StepOutcome::<Option<u32>>::unwitnessed(StepResult::Advance);
        o.record_if_witnessed(Some(&mut sa), Feature::KittenFed);
        assert_eq!(sa.counts.get(&Feature::KittenFed).copied(), None);
        assert_eq!(o.witness, None);
    }

    #[test]
    fn record_with_no_activation_is_noop() {
        let o = StepOutcome::<bool>::witnessed(StepResult::Advance);
        o.record_if_witnessed(None, Feature::CropTended);
        // Nothing to assert — the call must not panic when activation
        // tracking is disabled. Mirrors the release-build shape where
        // the `activation` resource may be absent.
    }
}
