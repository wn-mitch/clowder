//! Ticket 126 — actor-private BDI intention substrate.
//!
//! `HeldIntention` is a per-cat Component that records the goal-shaped
//! commitment the cat currently holds. Generalises `PairingActivity`'s
//! shape (one specific partner) into "the goal-shaped commitment I
//! currently hold." Authored by the L2 evaluator
//! (`evaluate_and_plan` in `src/systems/goap.rs`) on the softmax
//! winner; cleared by `resolve_goap_plans` on any drop trigger.
//!
//! # Actor-private by convention
//!
//! Sister DSEs **never** query other cats' `HeldIntention`. That would
//! be mind-reading: cats observe each other through body cues
//! (forthcoming §131 substrate), physical markers, audible cues
//! (§133), and spatial proximity — never internal state. The same rule
//! applies to `Disposition`. A helper cat that wants to bring soup to
//! an injured Resting cat authors its own `HeldIntention` from the
//! observable conjunction `target.Injured && target.HeadDownCurled`,
//! not from `target.HeldIntention.intention == Goal { rest }` and not
//! from `target.Disposition::Resting`.
//!
//! Two narrative sources fall out of this discipline, both
//! load-bearing: misreadings (the body-cue → Disposition mapping is
//! many-to-one, so `HeadDownCurled + Injured` cannot distinguish
//! "tired-from-wound" from "grieving") and unresolved conflicts (two
//! cats can independently form intentions toward the same target and
//! discover the conflict only at action time). Both are features, not
//! bugs.
//!
//! # Substrate placement
//!
//! `HeldIntention` is **substrate** per §4.7.2: no `StateEffect::Set*`
//! mutates it during A* expansion (A* operates on `PlannerState`, not
//! on cat components); external authorship by L2 is the §4.6 system.
//! Consumed via standard Bevy queries by the actor's own scoring
//! pipeline + drop triggers.
//!
//! # Coexistence with `PairingActivity`
//!
//! A cat in a Pairing disposition holds **both** `HeldIntention` and
//! `PairingActivity`. `PairingActivity` carries §7.M-specific state
//! (`last_interaction_tick`, the §7.M drop-branch vocabulary);
//! `HeldIntention` is the generic substrate. Drop gates run
//! independently. A future cleanup ticket may collapse them; not in
//! scope here.

use bevy_ecs::prelude::*;

use crate::ai::dse::Intention;

/// Where this intention came from. Self-formed dispositions carry
/// `SelfMotivated`; intentions adopted in response to a coordinator's
/// directive carry the coordinator's `Entity` so downstream consumers
/// (ticket 081 compliance demotion, ticket 130 trust-weighted
/// directive momentum) can read provenance. This ticket commits the
/// field and the read-site only — no live writer for
/// `CoordinatorDirective` ships in 126 (ticket 057 will).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub enum IntentionSource {
    SelfMotivated,
    CoordinatorDirective {
        #[serde(skip)]
        coordinator: Entity,
    },
}

impl IntentionSource {
    /// Stable ordinal for the scalar surface used by
    /// `IntentionMomentum`. `0` = `SelfMotivated`, `1` =
    /// `CoordinatorDirective`. Kept dense and small so the f32 round-
    /// trip through the scalar table preserves the discriminant
    /// exactly.
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::SelfMotivated => 0,
            Self::CoordinatorDirective { .. } => 1,
        }
    }

    /// Stable slug for trace serialization.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SelfMotivated => "self_motivated",
            Self::CoordinatorDirective { .. } => "coordinator_directive",
        }
    }
}

/// Why an intention was abandoned. Mirrors the trigger that fired the
/// drop. Recorded against the focal-cat trace's
/// `L3Commitment.abandon_reason` field; the
/// `Feature::IntentionAbandoned` activation counter is unparameterised
/// (the canary counts the variant, not the reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum IntentionAbandonReason {
    /// Trigger (3): a non-held DSE crossed the preempt margin.
    Preempted,
    /// §7.2 hard-fail: planner exhausted `max_replans`. Maps to
    /// `DropBranch::ReplanCap`.
    BecameImpossible,
    /// Trigger (4): target despawned, died, was banished, or
    /// incapacitated.
    TargetInvalid,
    /// `HeldIntention.expiry_tick` reached.
    Expired,
    /// §7.2 OpenMinded `still_goal == false`. Maps to
    /// `DropBranch::DroppedGoal`.
    DesireDrift,
}

impl IntentionAbandonReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preempted => "preempted",
            Self::BecameImpossible => "became_impossible",
            Self::TargetInvalid => "target_invalid",
            Self::Expired => "expired",
            Self::DesireDrift => "desire_drift",
        }
    }
}

/// Per-cat goal-shaped commitment. §126 substrate.
///
/// Only `Serialize` is derived (not `Deserialize`): `target: Option<Entity>`
/// has no `Default` and the component is runtime state — no save/load
/// path round-trips it. Mirrors `PairingActivity`'s precedent.
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct HeldIntention {
    /// What the cat is committed to. Reuses the existing `Intention`
    /// enum from `src/ai/dse.rs` (Goal | Activity); both carry a
    /// `CommitmentStrategy`. `Serialize`-skipped because `Intention`
    /// carries `fn(&World, Entity) -> bool` callbacks (`GoalState::achieved`,
    /// `Termination::UntilCondition`) that can't round-trip through serde.
    /// Trace consumers read the held DSE id via
    /// `MomentumSummary.held_dse` instead.
    #[serde(skip)]
    pub intention: Intention,
    /// The target entity (cat / building / tile-resident / wildlife)
    /// the intention is *toward*. `None` for self-state intentions
    /// (Rest, Idle, Wander). `Serialize`-skipped per `PairingActivity`
    /// — `Entity` has no `Default`.
    #[serde(skip)]
    pub target: Option<Entity>,
    /// Tick of adoption. Drives momentum decay and trace
    /// `ticks_held`.
    pub adopted_tick: u64,
    /// Per-intention strength, in `[0, 1]`. Derived at adoption time
    /// from the winning DSE's softmax-temperature-normalised margin
    /// over the runner-up. Constant for the intention's lifetime —
    /// refreshing per tick re-creates the per-tick churn problem this
    /// substrate exists to fix.
    pub commitment_strength: f32,
    /// Optional hard expiry tick. `None` for `Goal` intentions
    /// (which end on `achievement_believed`); `Some(t)` for
    /// `Activity::Termination::Ticks(n)`.
    pub expiry_tick: Option<u64>,
    /// Where this intention came from — `SelfMotivated` or
    /// `CoordinatorDirective(coord)`. The momentum modifier reads
    /// `source` so a future trust-weighted lift (ticket 130) can hook
    /// in without re-touching the modifier surface. 126 commits the
    /// field + a stub read-site; the trust axis itself ships in 130.
    pub source: IntentionSource,
}

impl HeldIntention {
    /// Convenience constructor used by the L2 author site.
    pub fn new(
        intention: Intention,
        target: Option<Entity>,
        tick: u64,
        commitment_strength: f32,
        expiry_tick: Option<u64>,
        source: IntentionSource,
    ) -> Self {
        Self {
            intention,
            target,
            adopted_tick: tick,
            commitment_strength: commitment_strength.clamp(0.0, 1.0),
            expiry_tick,
            source,
        }
    }

    /// `true` when `expiry_tick` is set and `now >= expiry_tick`.
    /// Trigger (3) — hard expiry — in `resolve_goap_plans`.
    pub fn is_expired(&self, now: u64) -> bool {
        self.expiry_tick.is_some_and(|t| now >= t)
    }
}

/// Temperature-normalised commitment-strength formulation. Locked in
/// the C3 design: `tanh(margin / softmax_temperature).clamp(0, 1)`.
/// Returns 0 when the margin is non-positive (no clear winner) and
/// asymptotes to 1 as the margin grows past several temperatures.
///
/// Centralised here — and not inline in the L2 author — so balance
/// tuning has a single edit site. The temperature normalisation
/// handles ticket 232's body-distress softmax-temperature coupling:
/// when the temperature is low the softmax is sharp and a small raw
/// margin still maps to a high strength, and vice versa.
pub fn commitment_strength_from_margin(margin: f32, softmax_temperature: f32) -> f32 {
    if margin <= 0.0 || softmax_temperature <= 0.0 {
        return 0.0;
    }
    (margin / softmax_temperature).tanh().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_round_trips_through_f32() {
        // The scalar surface stores ordinals as f32. Defend against
        // a future addition that adds a non-round-trippable variant.
        let cases = [(IntentionSource::SelfMotivated, 0u8)];
        for (source, expected) in cases {
            let f = source.ordinal() as f32;
            assert_eq!(f as u8, expected);
            assert_eq!(source.ordinal(), expected);
        }
    }

    #[test]
    fn commitment_strength_zero_when_no_margin() {
        assert_eq!(commitment_strength_from_margin(0.0, 0.5), 0.0);
        assert_eq!(commitment_strength_from_margin(-0.1, 0.5), 0.0);
    }

    #[test]
    fn commitment_strength_zero_when_temperature_zero() {
        // Defensive — softmax temperature should never reach zero in
        // production, but a divide-by-zero panic on a balance edit
        // would be a nasty surprise.
        assert_eq!(commitment_strength_from_margin(0.5, 0.0), 0.0);
    }

    #[test]
    fn commitment_strength_increases_with_margin() {
        let small = commitment_strength_from_margin(0.05, 0.5);
        let medium = commitment_strength_from_margin(0.20, 0.5);
        let large = commitment_strength_from_margin(0.80, 0.5);
        assert!(small < medium);
        assert!(medium < large);
        assert!(large <= 1.0);
    }

    #[test]
    fn commitment_strength_temperature_inversely_scales() {
        // Same margin, sharper softmax (lower temperature) → higher
        // strength. Confirms the §232 coupling.
        let sharp = commitment_strength_from_margin(0.10, 0.10);
        let diffuse = commitment_strength_from_margin(0.10, 1.00);
        assert!(sharp > diffuse);
    }

    #[test]
    fn is_expired_handles_unset() {
        let h = HeldIntention {
            intention: Intention::Activity {
                kind: crate::ai::dse::ActivityKind::Idle,
                termination: crate::ai::dse::Termination::UntilInterrupt,
                strategy: crate::ai::dse::CommitmentStrategy::OpenMinded,
            },
            target: None,
            adopted_tick: 100,
            commitment_strength: 0.5,
            expiry_tick: None,
            source: IntentionSource::SelfMotivated,
        };
        assert!(!h.is_expired(0));
        assert!(!h.is_expired(u64::MAX));
    }

    #[test]
    fn is_expired_fires_at_or_after_threshold() {
        let h = HeldIntention {
            intention: Intention::Activity {
                kind: crate::ai::dse::ActivityKind::Idle,
                termination: crate::ai::dse::Termination::Ticks(50),
                strategy: crate::ai::dse::CommitmentStrategy::OpenMinded,
            },
            target: None,
            adopted_tick: 100,
            commitment_strength: 0.5,
            expiry_tick: Some(150),
            source: IntentionSource::SelfMotivated,
        };
        assert!(!h.is_expired(149));
        assert!(h.is_expired(150));
        assert!(h.is_expired(200));
    }
}
