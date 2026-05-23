//! Kinship domain — `RAISE_OFFSPRING_ASPIRATION` (ticket 398).
//!
//! Spec: `docs/systems/ai-substrate-refactor.md` §7.M.2 — post-partum,
//! a parent cat adopts `RaiseOffspringAspiration` (nested alongside
//! `ReproduceAspiration`). The aspiration emits Caretake Intentions
//! per tick the parent has a juvenile dependent; the §L2.10.6 unified
//! softmax + §7.4 persistence-bonus handle commitment.
//!
//! # Why this exists as a separate domain
//!
//! Kinship is distinct from `Social` because §7.M.2's spec-named
//! aspiration tracks a separate commitment axis (provisioning +
//! protection of dependents). The personality-alignment picks up via
//! `compassion`, not `warmth`. Mirrors the §7.4 base-tier table's
//! treatment of Caretaking as its own DispositionKind separate from
//! Socializing.
//!
//! # Inactive-substrate status at 398 Phase 1a
//!
//! The chain registers in [`crate::ai::aspirations::ALL_CHAINS`] and
//! the [`caretake_kitten`](crate::ai::methods::caretake_kitten)
//! method registers Live in `populate_method_registry`. The **emit
//! row's `applicable_when` is [`always_false`]** — the row is
//! structurally authored but does not fire until 398's later phases:
//!
//! - Phase 1c — `select_intention_softmax` generalises the L3
//!   softmax to take a union pool of DSE-default scores ∪ emitted
//!   Goal-Intentions.
//! - Phase 1d — `§7.4 per-tier persistence-bonus` encodes
//!   Caretaking = Medium (base ≈ 0.10) × compassion × Patience and
//!   wires it into the preempt-check at `goap.rs:3745-3779`.
//! - Phase 1e — retire the wrap-site Intention-author at
//!   `goap.rs:2759-2834` (the unified softmax replaces it).
//!
//! When those phases land, the emit row flips to
//! `has_juvenile_dependent` (or equivalent marker-gated predicate).
//! Until then, the row is silent and the substrate is genuinely
//! dormant: the picker's per-tick walk skips it, the domain-affinity
//! fallback (Tertiary) is dominated by the existing `kitten_reared`
//! REACTIVE_EMITS entry (Primary). Behavior is byte-identical to
//! pre-398. See [`docs/open-work/tickets/398-7m2-raiseoffspringaspiration-kitten-rearing-as-nested-intention-aspiration.md`](../../../../docs/open-work/tickets/398-7m2-raiseoffspringaspiration-kitten-rearing-as-nested-intention-aspiration.md).
//!
//! # §7.7.1 conflict class
//!
//! `incompatible_with` is empty at 398 Phase 1a. The spec at §7.M.2
//! notes the relationship to `ReproduceAspiration` ("nested inside or
//! adjacent to ... §7.7.1 conflict class TBD"). Filling the full
//! §7.7.1 hard-pair list against existing chains is follow-on work —
//! likely a separate ticket after `ReproduceAspiration` ships its
//! chain.

use super::{always_false, AspirationChain, Emit, Milestone, Priority, ProgressTracker};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// The single milestone's emit row. References the
/// [`caretake_kitten`](crate::ai::methods::caretake_kitten) HTN method
/// label. `applicable_when: always_false` keeps the L2 emission path
/// dormant — survival is achieved via the L1 layer alone:
/// event-driven adoption ([`crate::systems::aspirations::adopt_kinship_aspiration`])
/// makes parents carry `RAISE_OFFSPRING_ASPIRATION` in their active
/// list, which the existing
/// [`AspirationLift`](crate::ai::modifier::AspirationLift) modifier
/// reads via `compute_aspiration_action_counts` → adds
/// `count × aspiration_bonus` (≈ +0.2) to Caretake's score for
/// parents. That lift is sufficient to keep Caretake winning softmax
/// across the full kitten-dependency window without needing the
/// L2 emit path or L3 persistence-bonus.
///
/// The L2 emit row remains structurally authored so the §L2.10.6
/// activation (full §7.M.2 architecture: per-tier persistence-bonus
/// on a held Caretake-Intention) has a documented landing surface
/// for follow-on work. Activating the row requires:
/// (1) Wean/Teach/Release side-effects wired into the FeedKitten
///     resolver (so kitten_reared can retire),
/// (2) frame-pin support for single-primitive `caretake_kitten`
///     methods (so the frame's leaf primitive pins chosen_action),
/// (3) persistence-bonus tuning for AspirationEmitted intentions
///     (so the lift doesn't decay over the 600-tick window),
/// (4) retire `kitten_reared` REACTIVE_EMITS row.
/// All listed in `/Users/will.mitchell/.claude/plans/melodic-knitting-quiche.md`
/// Phase 3 plus tuning details.
const RAISE_OFFSPRING_EMITS: &[Emit] = &[Emit {
    label: "caretake_kitten",
    applicable_when: always_false,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// `RaiseOffspringAspiration` — §7.M.2's spec-named lifetime
/// kitten-rearing aspiration. Single milestone: "Raise the Kitten",
/// which progresses via `ActionCount` over `Caretake` actions.
///
/// **Progress-tracker choice.** `ActionCount { Caretake, count: 9999 }`
/// is the closest fit from the existing [`ProgressTracker`] variants
/// for the §7.M.2 lifetime-arc shape. The spec's natural completion
/// trigger is §7.7.a Elder life-stage transition (the cat ages out of
/// the reproductive window); ActionCount-completion is a placeholder
/// that effectively never fires (a 15-min soak contains ~18k ticks
/// and even the most prolific caregivers won't accumulate 9999
/// Caretake actions). Lifetime-arc completion via Elder transition
/// is follow-on work; until then the chain stays active across the
/// cat's reproductive window and re-emits per tick `Parent` holds.
pub const RAISE_OFFSPRING_ASPIRATION: AspirationChain = AspirationChain {
    name: "Raise Offspring",
    domain: AspirationDomain::Kinship,
    milestones: &[Milestone {
        name: "Raise the Kitten",
        gate: super::always_true,
        progress_tracker: ProgressTracker::ActionCount {
            actions: &[Action::Caretake],
            count: 9999,
        },
        emits: RAISE_OFFSPRING_EMITS,
        narrative_on_complete:
            "{name} sees {possessive} kitten standing on its own. The work was worth it.",
    }],
    completion_narrative:
        "{name} has shepherded a new generation into the colony. The line continues.",
    // §7.7.1 conflict class TBD (see module doc). Filling this list
    // against `ReproduceAspiration` and the lifetime-celibacy arcs
    // is follow-on.
    incompatible_with: &[],
    expected_valence_target: 0.40,
};
