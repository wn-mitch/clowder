//! `CommitmentStrategy` semantics, belief-proxy gate, and per-DispositionKind
//! strategy table — §7.1–§7.3 of `docs/systems/ai-substrate-refactor.md`.
//!
//! The `CommitmentStrategy` tag has ridden on every `Intention` since Phase 3a
//! (`src/ai/dse.rs::CommitmentStrategy`). Phase 6a wires up the consumer via
//! four pure helpers that `resolve_goap_plans` calls in its per-cat loop
//! prologue (`src/systems/goap.rs:~1652`). For each cat with a held
//! `GoapPlan`: look up the plan's strategy, compute belief proxies, and if
//! the strategy says drop, push the cat's entity onto the existing
//! `plans_to_remove` batch — the plan is removed at the tail of
//! `resolve_goap_plans`, and `evaluate_and_plan` picks a replacement next
//! tick.
//!
//! # Module boundaries
//!
//! - [`BeliefProxies`] — the three proxy booleans §7.2 consumes, computed
//!   upstream from ECS state. Plain struct so unit tests can script truth
//!   tables without a `World`.
//! - [`should_drop_intention`] — pure-function gate. One match arm per
//!   `CommitmentStrategy` variant; no I/O, no ECS access. Test-first.
//! - [`strategy_for_disposition`] — §7.3 per-`DispositionKind` strategy
//!   table as a single `match`. Authoritative mapping; DSE factories
//!   committing a strategy for their `Intention::emit` mirror this table
//!   per the class they serve.
//! - [`proxies_for_plan`] — reads `GoapPlan` + `Needs` + constants and
//!   builds the `BeliefProxies` struct. Proxy recipe lives next to the
//!   gate.
//! - [`record_drop`] — telemetry helper for the `Feature::CommitmentDropTriggered`
//!   activation record. Called from `resolve_goap_plans` via its existing
//!   `NarrativeEmitter::activation` channel so the merged integration
//!   introduces no new `ResMut<SystemActivation>` writer.
//!
//! # Why no stand-alone system
//!
//! Phase 6a shipped a `reconsider_held_intentions` system
//! (`.after(check_anxiety_interrupts).before(evaluate_and_plan)`) that
//! failed seed-42 canaries with a dead colony. The H2 bisection
//! (2026-04-23 PM; `docs/open-work.md` #5) showed the gate's schedule
//! *presence* — not its proxy logic — disrupted Bevy's ordering enough
//! that cats stopped being re-evaluated. Merging the logic into
//! `resolve_goap_plans`'s existing per-cat loop eliminates the new
//! system and the corresponding `&GoapPlan` reader +
//! `ResMut<SystemActivation>` writer + `.before(evaluate_and_plan)` edge.
//! The semantic shifts by one tick (replacement lands next tick instead
//! of same-tick) — ~0.7 ms at 1389 Hz, below decision cadence.
//!
//! # What §7.2's three proxies mean here (§12.3 cross-ref)
//!
//! Clowder has no formal belief layer (§12.2); the gate uses three proxy
//! signals derived from state the substrate already maintains:
//!
//! - `achievement_believed` — current percepts evaluated against the
//!   Intention's goal predicate. For held `GoapPlan`s, mirrors the
//!   per-`DispositionKind` completion check in `resolve_goap_plans`'s
//!   post-exhaustion block (`disposition_complete`, `goap.rs:~1672`),
//!   **including its implicit `trips_done > 0` guard for Resting** —
//!   lifting the three-need recipe without the guard caused the
//!   2026-04-23 regression.
//! - `achievable_believed` — two-channel per §L2.10.7:
//!   1. **Elastic channel (a)** — `SpatialConsideration` score
//!      attenuation + retention threshold. Deferred to §7.4 balance
//!      iteration; always-true today.
//!   2. **Hard-fail channel (b)** — `GoapPlan::replan_count <
//!      GoapPlan::max_replans` (`goap_plan.rs:103`). When the planner
//!      can't route after capped retries, achievability is lost.
//! - `still_goal` — DSE re-score above retention threshold. Load-bearing
//!   only under `OpenMinded`. Phase 6a wires a coarse `needs`-shaped
//!   proxy for Socializing (satiated-social drops) and Exploring
//!   (always-true until the curiosity-drift thread lands). Thresholds
//!   tune downstream.
//!
//! # Non-goals (deferred per scope)
//!
//! - **§7.4 persistence bonus.** Balance thread.
//! - **§7.5 Maslow interrupt pipeline.** Existing
//!   `check_anxiety_interrupts` bypasses the gate. No new code.
//! - **§7.6 monitoring cadence.** Already in place.
//! - **§7.7 aspiration-level commitment.** Phase 6b.
//! - **§8 softmax variation.** Phase 6c.

use crate::ai::dse::CommitmentStrategy;
use crate::components::disposition::DispositionKind;
use crate::components::goap_plan::GoapPlan;
use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;

// ---------------------------------------------------------------------------
// BeliefProxies — the three signals §7.2 consumes
// ---------------------------------------------------------------------------

/// Belief proxies fed to [`should_drop_intention`]. Each field is a pure
/// boolean derived from ECS state upstream of the gate; the gate itself
/// does no I/O.
///
/// Per §12.3 these are not formal beliefs (Clowder has no belief layer,
/// see §12.2 — that's Talk-of-the-Town work). They're named interfaces
/// derived from percepts, plan state, and need scalars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeliefProxies {
    /// `true` when the cat's held Intention is believed achieved — the
    /// goal predicate currently resolves true against the cat's world
    /// view. For `GoapPlan`-driven dispositions this mirrors the
    /// `disposition_complete` check in `resolve_goap_plans`.
    pub achievement_believed: bool,
    /// `true` when achievement is still believed reachable. Composed
    /// per §L2.10.7 + §12.3 from an elastic channel (spatial score
    /// retention — deferred) and a hard-fail channel
    /// (`replan_count < max_replans`). The gate consumes the AND.
    pub achievable_believed: bool,
    /// `true` when the cat still goals this Intention. For Goal
    /// Intentions this is coupled to `achievable_believed`; for
    /// Activity Intentions under `OpenMinded` this captures
    /// desire-drift (satiated social, drifted curiosity). Always
    /// `true` means "no drift"; `false` drops under `OpenMinded`
    /// regardless of achievability.
    pub still_goal: bool,
}

impl BeliefProxies {
    /// Proxies that never fire a drop — used by tests to confirm a
    /// `Blind`-strategy Intention survives unless `achievement_believed`
    /// is set.
    pub fn never_drop() -> Self {
        Self {
            achievement_believed: false,
            achievable_believed: true,
            still_goal: true,
        }
    }
}

// ---------------------------------------------------------------------------
// The drop-trigger gate — §7.2 pure-function core
// ---------------------------------------------------------------------------

/// Return `true` when the cat's currently-held Intention should be
/// dropped this tick per its `CommitmentStrategy` and the supplied
/// belief proxies. Canonical spec:
///
/// ```text
/// match strategy {
///     Blind        => achieved,
///     SingleMinded => achieved || unachievable,
///     OpenMinded   => achieved || dropped_goal,
/// }
/// ```
///
/// The AI8 persistence cap (`max_persistence_ticks`) is a separate
/// pre-gate the caller is responsible for — the spec names it as an
/// independent exit and per Phase 6a entry that cap is tracked against
/// `GoapPlan::max_replans`, not a fresh Intention-age counter.
///
/// This is a pure function by design so the 12-row strategy-truth-table
/// tests don't need a `World`.
pub fn should_drop_intention(strategy: CommitmentStrategy, proxies: BeliefProxies) -> bool {
    let achieved = proxies.achievement_believed;
    let unachievable = !proxies.achievable_believed;
    let dropped_goal = !proxies.still_goal;
    match strategy {
        CommitmentStrategy::Blind => achieved,
        CommitmentStrategy::SingleMinded => achieved || unachievable,
        CommitmentStrategy::OpenMinded => achieved || dropped_goal,
    }
}

// ---------------------------------------------------------------------------
// §7.3 per-DispositionKind strategy table
// ---------------------------------------------------------------------------

/// §7.3 12-row strategy assignment. Each `DispositionKind` maps to its
/// canonical `CommitmentStrategy` per the spec table. DSE factories that
/// emit Intentions against one of these classes are expected to carry
/// the matching strategy; this function is the single source of truth.
///
/// **Mating L-levels.** The spec's §7.3 splits Mating into three rows
/// (L1 ReproduceAspiration — OpenMinded; L2 PairingActivity —
/// OpenMinded; L3 MateWithGoal — SingleMinded). Today Clowder only has
/// `DispositionKind::Mating` (the L3 goal-event layer); L1 and L2 live
/// in the aspiration catalog and target-taking activity layer
/// respectively (Phase 6b / §7.M). This table returns the L3
/// `SingleMinded` for `Mating` since that's what the current layer
/// represents. L1/L2 strategies are carried inline on the emitting
/// aspiration DSE and pairing-activity Intention, not here.
///
/// **Coordinator-directive row.** §7.3's footer commits
/// `SingleMinded` with a coordinator-cancel override; the row currently
/// folds into the Coordinating DispositionKind. The override path
/// lands with the coordinator DSE (Phase 5a residue — see
/// `docs/open-work.md` §13.6).
pub fn strategy_for_disposition(kind: DispositionKind) -> CommitmentStrategy {
    use CommitmentStrategy::*;
    match kind {
        // Physiological completion; Maslow gate handles preemption already.
        // AI8 caps runaway sleeps via `resting_max_replans`.
        DispositionKind::Resting => Blind,
        // 150 R5a: Eating mirrors Resting's Blind. Single-trip,
        // physiological completion gated on hunger only.
        DispositionKind::Eating => Blind,
        // Territory defense shouldn't flinch mid-patrol. AI8 caps fixation.
        DispositionKind::Guarding => Blind,
        // Goal-shaped — flipper-proof without fanaticism.
        DispositionKind::Hunting => SingleMinded,
        DispositionKind::Foraging => SingleMinded,
        DispositionKind::Coordinating => SingleMinded,
        DispositionKind::Building => SingleMinded,
        DispositionKind::Farming => SingleMinded,
        DispositionKind::Crafting => SingleMinded,
        DispositionKind::Caretaking => SingleMinded,
        // L3 layer — goal-shaped single event.
        DispositionKind::Mating => SingleMinded,
        // Activity-shaped — desire drift should terminate.
        DispositionKind::Socializing => OpenMinded,
        DispositionKind::Exploring => OpenMinded,
    }
}

// ---------------------------------------------------------------------------
// Proxy computation from held plan + needs — §12.3 concrete recipe
// ---------------------------------------------------------------------------

/// Build [`BeliefProxies`] for a cat holding `plan` with current `needs`.
///
/// This is the concrete Phase 6a recipe that feeds the gate. Each proxy
/// leans on state already maintained by the substrate:
///
/// - **`achievement_believed`** — per-`DispositionKind` completion check
///   mirroring `resolve_goap_plans`'s `disposition_complete` arm. For
///   `Resting` this is the three-need recipe (hunger / energy /
///   temperature all above completion thresholds). For trip-driven
///   dispositions it's `trips_done >= target_trips`. For goal-event
///   `Mating` it's `trips_done >= 1`.
/// - **`achievable_believed`** —
///   `plan.replan_count < plan.max_replans`. The elastic channel (DSE
///   score retention) is deferred to §7.4 — wiring it here without the
///   persistence bonus risks OpenMinded activities thrashing under
///   noisy rescores, so we keep the hard-fail channel alone this phase.
/// - **`still_goal`** — OpenMinded satiation proxy:
///   `Socializing` drops when `needs.social` climbs above
///   `resting_complete_temperature` (we reuse the need-completion
///   constant to avoid introducing a new balance knob mid-refactor).
///   `Exploring` always-true today — curiosity drift is a mood-drift
///   follow-on (§7.7.d, `docs/open-work.md` §13.4) and wiring a
///   concrete threshold is balance-thread work. For goal-shaped
///   dispositions `still_goal` is uninteresting under `Blind` and
///   `SingleMinded` — reporting `true` is the safe default.
pub fn proxies_for_plan(
    plan: &GoapPlan,
    needs: &Needs,
    d: &DispositionConstants,
    unexplored_nearby: f32,
) -> BeliefProxies {
    let achievement_believed = match plan.kind {
        // Mirrors `resolve_goap_plans`'s post-trip `disposition_complete`
        // arm (`goap.rs:~1672`). The `trips_done > 0` guard is the
        // lifted-condition protection: pre-C the three-need check only
        // fired inside the `plan.trips_done += 1` block, so a cat whose
        // ambient needs happened to sit above the thresholds without
        // having rested read as *not* achieved. Dropping that guard
        // cascaded plan-churn (2026-04-23 PM regression) — keep both
        // arms together or the lifted condition answers a different
        // question than `disposition_complete` does.
        // 150 R5a: Resting now covers Sleep + SelfGroom only. Hunger
        // is owned by the new `Eating` disposition; gating Resting on
        // hunger would resurrect the multi-need plan-duration cost
        // asymmetry that R5a removes.
        DispositionKind::Resting => {
            plan.trips_done > 0
                && needs.energy >= d.resting_complete_energy
                && needs.temperature >= d.resting_complete_temperature
        }
        // 150 R5a: Eating completes as soon as one chain has run AND
        // hunger has climbed above the resting-complete threshold.
        // Reusing `resting_complete_hunger` (default 0.65) so the
        // existing Resting tests still characterize the same satiation
        // band; Eating doesn't introduce a new balance knob.
        DispositionKind::Eating => {
            plan.trips_done > 0 && needs.hunger >= d.resting_complete_hunger
        }
        DispositionKind::Mating => plan.trips_done >= 1,
        // Guarding is triggered by low safety (`CriticalSafety` urgency
        // fires when `needs.safety < critical_safety_threshold`; the
        // Patrol DSE's `safety_deficit` consideration gates on the same
        // signal). Achievement therefore means "safety has recovered",
        // not just "N patrol trips done". The gate is OR over two
        // conditions: either the trip-target is met (the legacy
        // completion predicate) OR safety has climbed past the exit
        // band and at least one patrol trip has run. The `trips_done
        // >= 1` guard is the §Resting-recipe lifted-condition protection
        // ported over — a cat entering Guarding with ambient safety
        // already above the exit band must not read as achieved before
        // any patrol action has run, or the plan fires and drops on
        // the same tick it was built.
        //
        // Guarding's strategy is `Blind` (see `strategy_for_disposition`),
        // so `achievable_believed` is ignored — only this branch fires
        // the drop. Without the safety-recovered arm the plan only drops
        // on trips completion, which meant safety-collapse-driven
        // Guarding plans could loop indefinitely on cats whose patrol
        // gains kept them just above critical but far below sated.
        DispositionKind::Guarding => {
            let trips_complete = plan.trips_done >= plan.target_trips;
            let safety_recovered = plan.trips_done >= 1
                && needs.safety >= d.critical_safety_threshold + d.guarding_exit_epsilon;
            trips_complete || safety_recovered
        }
        _ => plan.trips_done >= plan.target_trips,
    };

    // Hard-fail channel only (Phase 6a scope). The elastic (a) channel
    // lands with §7.4's persistence bonus balance thread.
    let achievable_believed = plan.replan_count < plan.max_replans;

    // Desire-drift proxy for OpenMinded activities. Other classes
    // return `true` — the strategy dispatch in `should_drop_intention`
    // ignores `still_goal` under Blind and SingleMinded so the value
    // is semantically inert there.
    let still_goal = match plan.kind {
        DispositionKind::Socializing => {
            // Satiated — social need above the social satiation band.
            // Seed-42 soaks show social need never drops below 0.54
            // (passive proximity restoration), so the original 0.3
            // threshold (reused from resting_complete_temperature)
            // prevented any Socializing plan from persisting — every
            // plan was dropped as "goal drifted" before TravelTo +
            // SocializeWith could complete.  Dedicated knob at 0.85
            // lets plans hold until the cat is genuinely sated.
            needs.social < d.social_satiation_threshold
        }
        DispositionKind::Exploring => {
            // Area-familiarity proxy — the cat still wants to explore
            // when its local area feels unfamiliar.  Threshold matches
            // the Logistic saturation curve midpoint (0.3) so the
            // commitment gate releases plans at the same point the
            // scoring layer suppresses the Explore DSE.
            unexplored_nearby >= d.explore_satiation_threshold
        }
        _ => true,
    };

    BeliefProxies {
        achievement_believed,
        achievable_believed,
        still_goal,
    }
}

// ---------------------------------------------------------------------------
// Integration — called from resolve_goap_plans's per-cat loop prologue
// ---------------------------------------------------------------------------

/// Telemetry helper for a §7.2 drop event. Emits the aggregate
/// `CommitmentDropTriggered` counter plus the branch-specific
/// counter (`CommitmentDropBlind` / `…SingleMinded` / `…OpenMinded`
/// / `…ReplanCap`) so colony-wide canaries can distinguish a
/// completion-driven drop from a hard-fail planner abandon.
///
/// The caller (`resolve_goap_plans`'s per-cat prologue, or the
/// de-facto commitment sites until Phase 6a's pluggable gate lands)
/// routes the plan removal through its existing `plans_to_remove`
/// batch. This helper writes only the telemetry.
///
/// # Why a helper instead of a standalone system
///
/// Phase 6a shipped a stand-alone `reconsider_held_intentions` system
/// (`.after(check_anxiety_interrupts).before(evaluate_and_plan)`) that
/// failed the seed-42 soak. The H2 bisection (2026-04-23 PM; see
/// `docs/open-work.md` #5) landed the diagnosis: the gate's schedule
/// *presence* — not its proxy logic — disrupted Bevy's system ordering
/// enough that cats stopped being re-evaluated by `evaluate_and_plan`.
/// `CommitmentDropTriggered` fired only 4× in 25k ticks yet cats
/// starved, proving the body was barely executing. Moving the logic
/// into `resolve_goap_plans`'s existing per-cat loop eliminates the
/// new system (no new `ResMut<SystemActivation>` writer, no new
/// `&GoapPlan` reader, no new `.before(evaluate_and_plan)` edge). The
/// gate's semantic shifts by one tick — a dropped plan's replacement
/// lands next tick instead of same-tick — which is ~0.7 ms at 1389 Hz,
/// below decision cadence.
///
/// The three pure helpers ([`should_drop_intention`],
/// [`strategy_for_disposition`], [`proxies_for_plan`]) remain the
/// single source of truth for §7.1–§7.3 semantics and are
/// unit-testable without a `World`.
pub fn record_drop(
    activation: Option<&mut crate::resources::system_activation::SystemActivation>,
    strategy: CommitmentStrategy,
    branch: DropBranch,
) {
    let Some(a) = activation else {
        return;
    };
    use crate::resources::system_activation::Feature;
    a.record(Feature::CommitmentDropTriggered);
    // The `ReplanCap` branch is the §7.2 `achievable_believed == false`
    // hard-fail — the strategy is always SingleMinded (Blind ignores
    // achievability by design, OpenMinded doesn't dispatch on it) but
    // the distinct counter lets canaries separate planner collapse
    // from legitimate completion under the same strategy.
    match branch {
        DropBranch::ReplanCap => a.record(Feature::CommitmentDropReplanCap),
        DropBranch::Achieved | DropBranch::DroppedGoal => match strategy {
            CommitmentStrategy::Blind => a.record(Feature::CommitmentDropBlind),
            CommitmentStrategy::SingleMinded => a.record(Feature::CommitmentDropSingleMinded),
            CommitmentStrategy::OpenMinded => a.record(Feature::CommitmentDropOpenMinded),
        },
    }
}

/// Which arm of the §7.2 strategy dispatch decided the drop.
/// `Retained` is not represented because `record_drop` is only
/// called when the gate says to drop. Focal-trace capture records
/// retained decisions via the separate `L3Commitment` row with
/// `dropped: false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropBranch {
    /// `achievement_believed == true`. Fires under any strategy.
    Achieved,
    /// `achievable_believed == false` hard-fail; only under
    /// `SingleMinded`. Distinct counter to separate planner
    /// collapse from completion.
    ReplanCap,
    /// `still_goal == false` (desire drift); only under `OpenMinded`.
    DroppedGoal,
}

impl DropBranch {
    /// Stable slug for trace-record serialization.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Achieved => "achieved",
            Self::ReplanCap => "unachievable",
            Self::DroppedGoal => "dropped_goal",
        }
    }
}

/// Capture a §7.2 commitment decision in the focal-cat trace without
/// depending on ECS state — the caller resolves the focal cat, plan,
/// strategy and proxies and hands them in. Emits nothing when there's
/// no active focal-capture resource or when the decision isn't for
/// the focal cat (both encoded by the caller passing `capture: None`).
///
/// `dropped == false` captures retained decisions — those are
/// valuable for answering "why did the gate evaluate and *not* drop"
/// questions, which matter more for the OpenMinded desire-drift
/// branch than for Blind.
#[allow(clippy::too_many_arguments)]
pub fn record_commitment_decision(
    capture: Option<&crate::resources::FocalScoreCapture>,
    tick: u64,
    plan: &GoapPlan,
    strategy: CommitmentStrategy,
    proxies: BeliefProxies,
    dropped: bool,
    branch: &'static str,
) {
    let Some(capture) = capture else { return };
    capture.push_commitment(
        crate::resources::trace_log::CommitmentCapture {
            disposition: format!("{:?}", plan.kind),
            strategy: strategy.as_str(),
            achievement_believed: proxies.achievement_believed,
            achievable_believed: proxies.achievable_believed,
            still_goal: proxies.still_goal,
            trips_done: plan.trips_done,
            target_trips: plan.target_trips,
            replan_count: plan.replan_count,
            max_replans: plan.max_replans,
            branch,
            dropped,
        },
        tick,
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::personality::Personality;

    fn test_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    // -----------------------------------------------------------------
    // §7.1 variant-semantics truth table — pure `should_drop_intention`
    // -----------------------------------------------------------------

    #[test]
    fn blind_retains_when_nothing_achieved() {
        let proxies = BeliefProxies::never_drop();
        assert!(!should_drop_intention(CommitmentStrategy::Blind, proxies));
    }

    #[test]
    fn blind_drops_only_on_achieved() {
        // Achieved fires drop regardless of other proxies.
        let achieved_only = BeliefProxies {
            achievement_believed: true,
            achievable_believed: true,
            still_goal: true,
        };
        assert!(should_drop_intention(
            CommitmentStrategy::Blind,
            achieved_only
        ));

        // Unachievable alone does NOT drop under Blind — this is the
        // "zealot posture" §7.1 explicitly calls out.
        let unachievable = BeliefProxies {
            achievement_believed: false,
            achievable_believed: false,
            still_goal: true,
        };
        assert!(!should_drop_intention(
            CommitmentStrategy::Blind,
            unachievable
        ));

        // Desire drift alone does NOT drop under Blind.
        let drifted = BeliefProxies {
            achievement_believed: false,
            achievable_believed: true,
            still_goal: false,
        };
        assert!(!should_drop_intention(CommitmentStrategy::Blind, drifted));
    }

    #[test]
    fn single_minded_drops_on_achieved() {
        let proxies = BeliefProxies {
            achievement_believed: true,
            achievable_believed: true,
            still_goal: true,
        };
        assert!(should_drop_intention(
            CommitmentStrategy::SingleMinded,
            proxies
        ));
    }

    #[test]
    fn single_minded_drops_on_unachievable() {
        let proxies = BeliefProxies {
            achievement_believed: false,
            achievable_believed: false,
            still_goal: true,
        };
        assert!(should_drop_intention(
            CommitmentStrategy::SingleMinded,
            proxies
        ));
    }

    #[test]
    fn single_minded_ignores_desire_drift() {
        // SingleMinded is "open-minded on ends, single-minded on means"
        // (Rao & Georgeff p.14). Desire drift — `still_goal == false` —
        // does not by itself terminate the Intention under this strategy.
        let drifted = BeliefProxies {
            achievement_believed: false,
            achievable_believed: true,
            still_goal: false,
        };
        assert!(!should_drop_intention(
            CommitmentStrategy::SingleMinded,
            drifted
        ));
    }

    #[test]
    fn single_minded_retains_when_achievable_and_unachieved() {
        let proxies = BeliefProxies::never_drop();
        assert!(!should_drop_intention(
            CommitmentStrategy::SingleMinded,
            proxies
        ));
    }

    #[test]
    fn open_minded_drops_on_achieved() {
        let proxies = BeliefProxies {
            achievement_believed: true,
            achievable_believed: true,
            still_goal: true,
        };
        assert!(should_drop_intention(
            CommitmentStrategy::OpenMinded,
            proxies
        ));
    }

    #[test]
    fn open_minded_drops_on_desire_drift() {
        let drifted = BeliefProxies {
            achievement_believed: false,
            achievable_believed: true,
            still_goal: false,
        };
        assert!(should_drop_intention(
            CommitmentStrategy::OpenMinded,
            drifted
        ));
    }

    #[test]
    fn open_minded_ignores_unachievable_without_drift() {
        // §7.1 semantics: OpenMinded drops on achieved OR dropped_goal.
        // An unachievable-but-still-wanted Intention does NOT drop
        // under OpenMinded (it would under SingleMinded). This is the
        // "desire sticks even if world resists" reading.
        let unachievable_but_wanted = BeliefProxies {
            achievement_believed: false,
            achievable_believed: false,
            still_goal: true,
        };
        assert!(!should_drop_intention(
            CommitmentStrategy::OpenMinded,
            unachievable_but_wanted
        ));
    }

    #[test]
    fn open_minded_retains_when_no_signals() {
        let proxies = BeliefProxies::never_drop();
        assert!(!should_drop_intention(
            CommitmentStrategy::OpenMinded,
            proxies
        ));
    }

    // -----------------------------------------------------------------
    // §7.3 strategy-table coverage — DispositionKind rows
    // 150 R5a: 12 → 13 with the addition of Eating.
    // -----------------------------------------------------------------

    #[test]
    fn strategy_table_covers_every_disposition() {
        use CommitmentStrategy::*;
        use DispositionKind::*;
        assert_eq!(strategy_for_disposition(Resting), Blind);
        assert_eq!(strategy_for_disposition(Eating), Blind);
        assert_eq!(strategy_for_disposition(Guarding), Blind);
        assert_eq!(strategy_for_disposition(Hunting), SingleMinded);
        assert_eq!(strategy_for_disposition(Foraging), SingleMinded);
        assert_eq!(strategy_for_disposition(Coordinating), SingleMinded);
        assert_eq!(strategy_for_disposition(Building), SingleMinded);
        assert_eq!(strategy_for_disposition(Farming), SingleMinded);
        assert_eq!(strategy_for_disposition(Crafting), SingleMinded);
        assert_eq!(strategy_for_disposition(Caretaking), SingleMinded);
        assert_eq!(strategy_for_disposition(Mating), SingleMinded);
        assert_eq!(strategy_for_disposition(Socializing), OpenMinded);
        assert_eq!(strategy_for_disposition(Exploring), OpenMinded);

        // Exhaustive-enum guard — if a new DispositionKind variant is
        // added without a row here, the `DispositionKind::ALL` constant
        // will diverge from the covered set below.
        let covered = [
            Resting,
            Eating,
            Guarding,
            Hunting,
            Foraging,
            Coordinating,
            Building,
            Farming,
            Crafting,
            Caretaking,
            Mating,
            Socializing,
            Exploring,
        ];
        assert_eq!(
            covered.len(),
            DispositionKind::ALL.len(),
            "strategy table must cover every DispositionKind variant"
        );
    }

    // -----------------------------------------------------------------
    // §12.3 proxies_for_plan recipe
    // -----------------------------------------------------------------

    fn default_needs() -> Needs {
        // Low-band values so `resting_complete_*` defaults
        // (hunger 0.5 / energy 0.3 / temperature 0.3) don't trip
        // achievement before tests flip the knobs deliberately.
        Needs {
            hunger: 0.2,
            energy: 0.2,
            temperature: 0.2,
            social: 0.2,
            ..Needs::default()
        }
    }

    fn default_d() -> DispositionConstants {
        crate::resources::sim_constants::SimConstants::default().disposition
    }

    fn test_plan(kind: DispositionKind, tick: u64) -> GoapPlan {
        let p = test_personality();
        GoapPlan::new(kind, tick, &p, vec![], None)
    }

    #[test]
    fn proxies_resting_achievement_tracks_three_need_recipe() {
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Resting, 0);
        plan.trips_done = 1; // past the guard; three-need recipe now active

        // Mid-band needs — not yet achieved.
        let proxies = proxies_for_plan(&plan, &default_needs(), &d, 1.0);
        assert!(!proxies.achievement_believed);
        assert!(proxies.achievable_believed);

        // All three needs above thresholds → achieved.
        let mut needs = default_needs();
        needs.hunger = 1.0;
        needs.energy = 1.0;
        needs.temperature = 1.0;
        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert!(proxies.achievement_believed);
    }

    #[test]
    fn proxies_resting_achievement_requires_trip_guard() {
        // Regression guard for the 2026-04-23 lifted-condition bug.
        // A fresh Resting plan (`trips_done == 0`) against a cat whose
        // ambient needs sit above the completion thresholds must NOT
        // read as achieved — the cat hasn't rested yet, and the gate
        // would fire at plan-birth causing the 1-tick Resting↔Exploring
        // oscillation that starved Calcifer on seed 42.
        let d = default_d();
        let plan = test_plan(DispositionKind::Resting, 0);
        assert_eq!(plan.trips_done, 0);

        let mut needs = default_needs();
        needs.hunger = 1.0;
        needs.energy = 1.0;
        needs.temperature = 1.0;
        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert!(
            !proxies.achievement_believed,
            "trips_done == 0 must gate the three-need recipe"
        );
    }

    #[test]
    fn proxies_trips_based_achievement_fires_on_target_hit() {
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Hunting, 0);
        plan.target_trips = 2;

        // 0/2 trips — not achieved.
        assert!(!proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievement_believed);

        // 2/2 trips — achieved.
        plan.trips_done = 2;
        assert!(proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievement_believed);
    }

    #[test]
    fn proxies_mating_achievement_fires_on_single_event() {
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Mating, 0);

        assert!(!proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievement_believed);
        plan.trips_done = 1;
        assert!(proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievement_believed);
    }

    #[test]
    fn proxies_achievable_flips_when_replan_cap_hit() {
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Hunting, 0);
        plan.max_replans = 3;
        plan.replan_count = 0;
        assert!(proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievable_believed);

        plan.replan_count = 2;
        assert!(proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievable_believed);

        plan.replan_count = 3;
        assert!(!proxies_for_plan(&plan, &default_needs(), &d, 1.0).achievable_believed);
    }

    #[test]
    fn proxies_still_goal_tracks_social_satiation_for_socializing() {
        let d = default_d();
        let plan = test_plan(DispositionKind::Socializing, 0);

        // Social below threshold → still goals.
        let mut needs = default_needs();
        needs.social = 0.1;
        assert!(proxies_for_plan(&plan, &needs, &d, 1.0).still_goal);

        // Social above `social_satiation_threshold` → drifted.
        needs.social = d.social_satiation_threshold + 0.05;
        assert!(!proxies_for_plan(&plan, &needs, &d, 1.0).still_goal);
    }

    #[test]
    fn proxies_still_goal_true_for_non_openminded_classes() {
        let d = default_d();
        for kind in [
            DispositionKind::Resting,
            DispositionKind::Hunting,
            DispositionKind::Foraging,
            DispositionKind::Building,
            DispositionKind::Guarding,
            DispositionKind::Farming,
            DispositionKind::Crafting,
            DispositionKind::Caretaking,
            DispositionKind::Coordinating,
            DispositionKind::Mating,
        ] {
            let plan = test_plan(kind, 0);
            assert!(
                proxies_for_plan(&plan, &default_needs(), &d, 1.0).still_goal,
                "{kind:?}: non-OpenMinded classes always report still_goal=true"
            );
        }
    }

    // -----------------------------------------------------------------
    // End-to-end gate behavior — plan + proxies + strategy
    // -----------------------------------------------------------------

    #[test]
    fn gate_drops_single_minded_hunting_on_replan_exhaustion() {
        // The canonical SingleMinded hard-fail case: the planner has
        // retried past the cap and the cat should re-evaluate.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Hunting, 0);
        plan.max_replans = 3;
        plan.replan_count = 3;

        let strategy = strategy_for_disposition(plan.kind);
        let proxies = proxies_for_plan(&plan, &default_needs(), &d, 1.0);
        assert_eq!(strategy, CommitmentStrategy::SingleMinded);
        assert!(!proxies.achievable_believed);
        assert!(should_drop_intention(strategy, proxies));
    }

    #[test]
    fn gate_retains_single_minded_hunting_with_retries_left() {
        // Mid-plan, replan cap not hit, no trips done yet. Plan holds.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Hunting, 0);
        plan.max_replans = 3;
        plan.replan_count = 1;

        let strategy = strategy_for_disposition(plan.kind);
        let proxies = proxies_for_plan(&plan, &default_needs(), &d, 1.0);
        assert!(!should_drop_intention(strategy, proxies));
    }

    #[test]
    fn gate_retains_blind_guarding_under_planner_hard_fail() {
        // Blind ignores `achievable_believed`. Even with replan cap hit,
        // a Blind Guarding plan holds until achieved (trip target met
        // or patrol loop closes). AI8 is the escape hatch, not §7.2.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Guarding, 0);
        plan.target_trips = 3;
        plan.max_replans = 3;
        plan.replan_count = 3;

        // Use low safety so the safety-recovered arm of the Guarding
        // achievement recipe doesn't fire — this test exercises the
        // "still replanning, still below exit band, trips incomplete"
        // holdfast case.
        let mut needs = default_needs();
        needs.safety = 0.1;

        let strategy = strategy_for_disposition(plan.kind);
        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert_eq!(strategy, CommitmentStrategy::Blind);
        assert!(!proxies.achievable_believed);
        assert!(!proxies.achievement_believed);
        // Blind ignores unachievable. Retain.
        assert!(!should_drop_intention(strategy, proxies));
    }

    // -----------------------------------------------------------------
    // Guarding safety-recovered recipe — seed-69 Patrol-loop fix.
    // See `docs/balance/guarding-exit-recipe.md` and the Thistle
    // diagnosis under `docs/open-work/landed/2026-04.md`.
    // -----------------------------------------------------------------

    #[test]
    fn proxies_guarding_achievement_requires_trip_guard() {
        // Regression guard for the lifted-condition pattern.
        // A fresh Guarding plan (`trips_done == 0`) against a cat whose
        // ambient safety already sits above the exit band must NOT
        // read as achieved — the cat hasn't patrolled yet, and the gate
        // would fire at plan-birth causing same-tick Guarding↔something
        // oscillation (the structural analog of the 2026-04-23 Resting
        // bug).
        let d = default_d();
        let plan = test_plan(DispositionKind::Guarding, 0);
        assert_eq!(plan.trips_done, 0);

        let mut needs = default_needs();
        // Well above exit band (0.2 + 0.15 = 0.35).
        needs.safety = 0.9;

        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert!(
            !proxies.achievement_believed,
            "trips_done == 0 must gate the safety-recovered recipe"
        );
    }

    #[test]
    fn proxies_guarding_unachieved_when_safety_below_exit_band() {
        // One patrol trip done but safety is still just above the
        // critical threshold — not yet past the exit band. Retain.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Guarding, 0);
        plan.target_trips = 3;
        plan.trips_done = 1;

        let mut needs = default_needs();
        // Exit band is `critical_safety_threshold + guarding_exit_epsilon`
        // = 0.2 + 0.15 = 0.35. Land just below.
        needs.safety = 0.25;

        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert!(!proxies.achievement_believed);
    }

    #[test]
    fn proxies_guarding_achieved_when_safety_above_exit_band_after_trip() {
        // Safety-recovered arm: ≥1 patrol trip has run AND safety has
        // climbed past the exit band. Fires the drop under Blind.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Guarding, 0);
        plan.target_trips = 3;
        plan.trips_done = 1;

        let mut needs = default_needs();
        // Clear the exit band (0.35) comfortably.
        needs.safety = 0.5;

        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert!(proxies.achievement_believed);
    }

    #[test]
    fn proxies_guarding_achieved_on_legacy_trips_target() {
        // Backward-compatible: even with safety still low, the plan
        // drops when the trip target is met. Preserves the existing
        // Guarding completion semantics from before the recipe change.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Guarding, 0);
        plan.target_trips = 2;
        plan.trips_done = 2;

        let mut needs = default_needs();
        needs.safety = 0.05; // well below exit band

        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert!(proxies.achievement_believed);
    }

    #[test]
    fn gate_drops_blind_guarding_when_safety_recovers_mid_plan() {
        // End-to-end: the loop-breaker. A Blind Guarding plan with
        // trips remaining drops via the safety-recovered arm once the
        // cat has patrolled at least once and safety has climbed past
        // the exit band. Without this arm Thistle-pattern cats would
        // loop indefinitely through Guarding/Patrol (seed-69 soak).
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Guarding, 0);
        plan.target_trips = 5;
        plan.trips_done = 1;

        let mut needs = default_needs();
        needs.safety = 0.6;

        let strategy = strategy_for_disposition(plan.kind);
        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert_eq!(strategy, CommitmentStrategy::Blind);
        assert!(proxies.achievement_believed);
        assert!(should_drop_intention(strategy, proxies));
    }

    #[test]
    fn gate_drops_open_minded_socializing_on_satiation() {
        // Social needs sated mid-activity. OpenMinded should drop.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Socializing, 0);
        plan.target_trips = 5;
        plan.replan_count = 0;

        let mut needs = default_needs();
        needs.social = d.social_satiation_threshold + 0.1;

        let strategy = strategy_for_disposition(plan.kind);
        let proxies = proxies_for_plan(&plan, &needs, &d, 1.0);
        assert_eq!(strategy, CommitmentStrategy::OpenMinded);
        assert!(!proxies.still_goal);
        assert!(should_drop_intention(strategy, proxies));
    }

    #[test]
    fn gate_retains_exploring_when_area_unfamiliar() {
        // Nearby area still mostly unexplored — cat wants to keep exploring.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Exploring, 0);
        plan.target_trips = 3;
        plan.trips_done = 1;

        let strategy = strategy_for_disposition(plan.kind);
        // 0.5 > threshold (0.3) → still_goal = true → retain.
        let proxies = proxies_for_plan(&plan, &default_needs(), &d, 0.5);
        assert_eq!(strategy, CommitmentStrategy::OpenMinded);
        assert!(proxies.still_goal);
        assert!(!should_drop_intention(strategy, proxies));
    }

    #[test]
    fn gate_drops_exploring_when_area_familiar() {
        // Nearby area well-explored — desire to explore fades.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Exploring, 0);
        plan.target_trips = 3;
        plan.trips_done = 1;

        let strategy = strategy_for_disposition(plan.kind);
        // 0.1 < threshold (0.3) → still_goal = false → drop.
        let proxies = proxies_for_plan(&plan, &default_needs(), &d, 0.1);
        assert_eq!(strategy, CommitmentStrategy::OpenMinded);
        assert!(!proxies.still_goal);
        assert!(should_drop_intention(strategy, proxies));
    }

    // -----------------------------------------------------------------
    // Integration test — full achievability-flip flow under the gate
    // -----------------------------------------------------------------

    #[test]
    fn integration_hunting_drops_mid_tick_when_achievability_flips() {
        // A Hunting plan with retries left holds. When the planner
        // exhausts its replan budget the same tick (simulating cascading
        // pathfinding failures against a moving prey), the gate drops.
        // Mirrors the `SingleMinded` flip the spec's worked example
        // in §7.1 describes.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Hunting, 0);
        plan.max_replans = 2;

        let strategy = strategy_for_disposition(plan.kind);

        // Pre-flip: achievable → retain.
        plan.replan_count = 1;
        assert!(!should_drop_intention(
            strategy,
            proxies_for_plan(&plan, &default_needs(), &d, 1.0)
        ));

        // Flip: cap reached.
        plan.replan_count = 2;
        assert!(should_drop_intention(
            strategy,
            proxies_for_plan(&plan, &default_needs(), &d, 1.0)
        ));
    }

    #[test]
    fn integration_resting_survives_planner_pressure_until_needs_sated() {
        // Blind strategy: planner hard-fails are ignored, only
        // `achievement_believed` (three-need completion after at least
        // one rest trip) drops the plan.
        let d = default_d();
        let mut plan = test_plan(DispositionKind::Resting, 0);
        plan.trips_done = 1; // past the `trips_done > 0` guard
        plan.max_replans = 3;
        plan.replan_count = 3; // achievable channel says "unachievable".

        let strategy = strategy_for_disposition(plan.kind);
        assert_eq!(strategy, CommitmentStrategy::Blind);

        // Needs unsated → retain.
        assert!(!should_drop_intention(
            strategy,
            proxies_for_plan(&plan, &default_needs(), &d, 1.0)
        ));

        // Needs sated → drop.
        let mut needs = default_needs();
        needs.hunger = 1.0;
        needs.energy = 1.0;
        needs.temperature = 1.0;
        assert!(should_drop_intention(
            strategy,
            proxies_for_plan(&plan, &needs, &d, 1.0)
        ));
    }
}
