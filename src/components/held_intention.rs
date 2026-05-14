//! Ticket 126 — actor-private BDI intention substrate.
//!
//! `HeldIntention` is a per-cat Component that records the goal-shaped
//! commitment the cat currently holds. Generalises the prior
//! `PairingActivity` shape (one specific partner) into "the
//! goal-shaped commitment I currently hold." Ticket 127 retired
//! `PairingActivity` in favor of `JointIntention { practice: Courtship,
//! .. }` (mutually-public substrate) — `HeldIntention` remains the
//! actor-private counterpart. Authored by the L2 evaluator
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
//! # Coexistence with `JointIntention`
//!
//! A cat in a joint practice (e.g., Courtship) holds **both**
//! `HeldIntention` and `JointIntention { practice, .. }`. JointIntention
//! is *mutually-public substrate* (ticket 127) — it carries the
//! practice-state observable to both partners (stage, partner,
//! tick markers, drop-branch vocabulary). HeldIntention is the
//! *actor-private* counterpart (commitment strength, source, expiry).
//! Drop gates run independently. The two substrate categories are
//! intentional and load-bearing per ticket 127's §Semantic category.

use bevy_ecs::prelude::*;

use crate::ai::dse::Intention;
use crate::ai::Action;

/// Where this intention came from. Self-formed dispositions carry
/// `SelfMotivated`; intentions adopted in response to a coordinator's
/// directive carry the coordinator's `Entity`; intentions emitted by
/// the L1→L2 aspiration picker (ticket 321) carry the aspiration's
/// chain name. Downstream consumers (ticket 081 compliance demotion,
/// ticket 130 trust-weighted directive momentum, the 320 HTN goal-
/// stack's `GoalFrame.source`) read provenance through this enum.
/// 126 committed the field + `SelfMotivated` / `CoordinatorDirective`
/// variants; 320 added the `AspirationEmitted` variant for the HTN
/// emission path; 321 wires the picker that authors it.
///
/// `chain` carries a `&'static str` (post-321) — every aspiration
/// chain's name lives in `crate::ai::aspirations`'s const
/// [`crate::ai::aspirations::AspirationChain::name`] field, so the
/// emitter site never allocates. Pre-321 the variant carried `String`;
/// flipping to `&'static str` restores `Copy`-derivability for the
/// non-`CoordinatorDirective` variants (the enum itself isn't `Copy`
/// because `CoordinatorDirective` carries `Entity` which is `Copy` —
/// but the variant constructor takes `serde(skip)`, so for the trace
/// emitter's purposes the enum-level `Clone` suffices).
#[derive(Debug, Clone, serde::Serialize)]
pub enum IntentionSource {
    SelfMotivated,
    CoordinatorDirective {
        #[serde(skip)]
        coordinator: Entity,
    },
    /// Ticket 320 (variant) / 321 (writer) — emitted by the L1→L2
    /// picker from an aspiration's per-milestone `emits` table.
    /// `chain` matches `ActiveAspiration.chain_name` for the source
    /// aspiration (e.g. `"Master of the Hunt"`); the picker copies
    /// the `&'static str` from the matched
    /// `crate::ai::aspirations::AspirationChain.name`.
    AspirationEmitted {
        chain: &'static str,
    },
}

impl IntentionSource {
    /// Stable ordinal for the scalar surface used by
    /// `IntentionMomentum`. `0` = `SelfMotivated`, `1` =
    /// `CoordinatorDirective`, `2` = `AspirationEmitted`. Kept dense
    /// and small so the f32 round-trip through the scalar table
    /// preserves the discriminant exactly.
    pub fn ordinal(&self) -> u8 {
        match self {
            Self::SelfMotivated => 0,
            Self::CoordinatorDirective { .. } => 1,
            Self::AspirationEmitted { .. } => 2,
        }
    }

    /// Stable slug for trace serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SelfMotivated => "self_motivated",
            Self::CoordinatorDirective { .. } => "coordinator_directive",
            Self::AspirationEmitted { .. } => "aspiration_emitted",
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
/// path round-trips it. Mirrors `JointIntention`'s precedent.
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
    /// The cat's held DSE encoded as an `Action`. Round-trips 1:1 to
    /// the codebase's DSE id strings via
    /// `crate::ai::modifier::dse_id_for_action`; the
    /// `IntentionMomentum` modifier reads it through the
    /// `INTENTION_HELD_ACTION_ORDINAL` scalar. Stored separately from
    /// `intention` because `Intention::Activity { kind: ActivityKind }`
    /// and `Intention::Goal { state: GoalState }` don't map cleanly
    /// 1:1 to `Action` (e.g., `Resting` covers `Sleep` and `GroomSelf`,
    /// and the held DSE within that disposition is what gets the
    /// margin-weighted lift).
    pub held_action: Action,
    /// The target entity (cat / building / tile-resident / wildlife)
    /// the intention is *toward*. `None` for self-state intentions
    /// (Rest, Idle, Wander). `Serialize`-skipped per `JointIntention`
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
        held_action: Action,
        target: Option<Entity>,
        tick: u64,
        commitment_strength: f32,
        expiry_tick: Option<u64>,
        source: IntentionSource,
    ) -> Self {
        Self {
            intention,
            held_action,
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

    /// Linear-decay factor at `now`. Returns `1.0` at `adopted_tick`,
    /// ramps down to `0.0` at the decay window's edge:
    ///
    /// - `Goal` intentions (no `expiry_tick`) use `decay_window_ticks`
    ///   (the `intention_momentum_decay_ticks` constant from
    ///   `DispositionConstants`).
    /// - `Activity` intentions with `expiry_tick = Some(t)` use
    ///   `t - adopted_tick` as the window — the lift fades to zero
    ///   at the activity's natural termination.
    ///
    /// Returns `0.0` when `now < adopted_tick` (clock-skew defense)
    /// or when the window has elapsed. The `IntentionMomentum`
    /// scalar is `commitment_strength × intention_momentum_lift ×
    /// decay_factor`, so a 0.0 here makes the modifier short-circuit.
    pub fn decay_factor(&self, now: u64, decay_window_ticks: u64) -> f32 {
        if now < self.adopted_tick {
            return 0.0;
        }
        let window = self
            .expiry_tick
            .map(|t| t.saturating_sub(self.adopted_tick))
            .filter(|w| *w > 0)
            .unwrap_or(decay_window_ticks);
        if window == 0 {
            return 0.0;
        }
        let elapsed = now - self.adopted_tick;
        if elapsed >= window {
            return 0.0;
        }
        1.0 - (elapsed as f32 / window as f32)
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
        let cases: &[(IntentionSource, u8)] = &[
            (IntentionSource::SelfMotivated, 0),
            (
                IntentionSource::AspirationEmitted {
                    chain: "Master of the Hunt",
                },
                2,
            ),
        ];
        for (source, expected) in cases {
            let f = source.ordinal() as f32;
            assert_eq!(f as u8, *expected);
            assert_eq!(source.ordinal(), *expected);
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
            held_action: Action::Idle,
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
            held_action: Action::Idle,
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

    #[test]
    fn decay_factor_starts_at_one_and_ramps_to_zero() {
        let h = HeldIntention {
            intention: Intention::Activity {
                kind: crate::ai::dse::ActivityKind::Idle,
                termination: crate::ai::dse::Termination::UntilInterrupt,
                strategy: crate::ai::dse::CommitmentStrategy::OpenMinded,
            },
            held_action: Action::Hunt,
            target: None,
            adopted_tick: 100,
            commitment_strength: 1.0,
            expiry_tick: None,
            source: IntentionSource::SelfMotivated,
        };
        // Goal-shape window: uses passed-in decay_window_ticks.
        assert!((h.decay_factor(100, 600) - 1.0).abs() < 1e-6);
        assert!((h.decay_factor(400, 600) - 0.5).abs() < 1e-6);
        assert!((h.decay_factor(700, 600) - 0.0).abs() < 1e-6);
        assert!((h.decay_factor(700, 600) - 0.0).abs() < 1e-6);
        // Clock-skew defense: now < adopted_tick → 0.0.
        assert_eq!(h.decay_factor(99, 600), 0.0);
    }

    #[test]
    fn decay_factor_uses_expiry_window_for_activity_with_expiry() {
        // Activity intention with explicit expiry — decay window is
        // (expiry - adopted), not the passed-in default.
        let h = HeldIntention {
            intention: Intention::Activity {
                kind: crate::ai::dse::ActivityKind::Rest,
                termination: crate::ai::dse::Termination::Ticks(200),
                strategy: crate::ai::dse::CommitmentStrategy::OpenMinded,
            },
            held_action: Action::Sleep,
            target: None,
            adopted_tick: 100,
            commitment_strength: 1.0,
            expiry_tick: Some(300),
            source: IntentionSource::SelfMotivated,
        };
        assert!((h.decay_factor(100, 600) - 1.0).abs() < 1e-6);
        // 100 ticks elapsed of a 200-tick window → 0.5.
        assert!((h.decay_factor(200, 600) - 0.5).abs() < 1e-6);
        // At expiry → 0.0.
        assert!((h.decay_factor(300, 600) - 0.0).abs() < 1e-6);
    }
}
