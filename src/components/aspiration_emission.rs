//! Ticket 321 — per-cat ephemeral substrate carrying the L1→L2 picker's
//! emitted `Intention::Goal` candidates for this tick.
//!
//! Substrate per §4.7.2 of `docs/systems/ai-substrate-refactor.md`:
//! (a) no `StateEffect::Set*` mutates `AspirationEmissions` during A*
//! expansion (the planner runs over `PlannerState`, not on cat
//! components); (b) external authorship by
//! `crate::systems::aspiration_picker::pick_aspiration_emissions`.
//!
//! # Lifecycle
//!
//! The picker is the sole writer. Per its contract:
//! - At the start of each picker run the existing `AspirationEmissions`
//!   on a cat is dropped.
//! - For each `ActiveAspiration` on that cat, the picker walks the
//!   current milestone's `emits[]` table and produces zero or one
//!   `EmissionRow`. The cat's combined `rows: Vec<EmissionRow>` is
//!   inserted (replacing any prior) when non-empty; the Component is
//!   removed entirely when the cat has no live emission this tick.
//!
//! Read by `evaluate_and_plan` at the `Intention::Activity { Idle }`
//! wrap site — when `rows.is_empty()` is false, the highest-`Priority`
//! row replaces the default Activity wrap with `Intention::Goal {
//! state: { label, achieved: |_, _| false }, strategy }`. 320's HTN
//! frame-push gate then catches the Goal shape and walks
//! `MethodRegistry`.
//!
//! # Actor-private
//!
//! Like `HeldIntention` and `HeldGoalStack`, never read across cats.
//! The picker's emissions reflect *this cat's* aspiration set; another
//! cat's L2 author site never inspects them.

use bevy_ecs::prelude::*;

use crate::ai::aspirations::Priority;
use crate::ai::dse::{CommitmentStrategy, GoalKind};

/// One per-aspiration emission row the picker produced this tick.
///
/// `chain` matches `ActiveAspiration.chain_name` for the emitting
/// aspiration — used by the L2 author site to set
/// `IntentionSource::AspirationEmitted { chain }` so 320's
/// frame-push gate knows which aspiration drove the commitment.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmissionRow {
    /// The chain that emitted this row (matches `chain.name` of the
    /// authoring `AspirationChain`).
    pub chain: &'static str,
    /// The milestone index within `chain.milestones` that owned the
    /// emit row. Captured at picker time so trace records and
    /// downstream consumers don't need to re-resolve.
    pub milestone_index: usize,
    /// Goal-label this candidate `Intention::Goal` carries — matches a
    /// `Method.goal_label` in `MethodRegistry`.
    pub label: &'static str,
    /// Strategy the emitted Intention takes. Authored by the `Emit`
    /// row's `strategy` field. `Serialize`-skipped because
    /// `CommitmentStrategy` is a plain enum without a serde derive;
    /// trace surfaces that need the strategy read it from
    /// `HeldIntention.intention.strategy()` post-adoption.
    #[serde(skip)]
    pub strategy: CommitmentStrategy,
    /// Picker priority — Primary < Secondary < Tertiary. The L2 author
    /// selects the lowest-`Priority` row when multiple are present.
    pub priority: Priority,
    /// `true` when this row came from the domain-affinity fallback
    /// (§H step 3) rather than the milestone's authored `emits[]`
    /// table (§H step 2). Surfaced in the L1Aspiration trace record.
    pub fallback_used: bool,
    /// Typed goal-state carried by this emission (ticket 463). When
    /// `Some(kind)`, the L2 author site wraps an `Intention::Goal`
    /// whose `state.kind` is this variant directly — used by
    /// `CraftItemAspiration` to carry the per-cat per-tick winning
    /// recipe identity (`GoalKind::HaveItem(recipe.output)`) through
    /// to L2 and the HTN frame-push without round-tripping through a
    /// `&'static str` label. When `None`, the L2 author falls back to
    /// the legacy `GoalState::predicate(row.label, |_,_| false)` path
    /// so existing static aspirations (Hunting, Kinship, etc.) are
    /// untouched. `GoalKind` is not `Serialize`; `serde(skip)` keeps
    /// the trace payload shape unchanged for label-only consumers.
    #[serde(skip)]
    pub goal_kind: Option<GoalKind>,
}

/// Per-cat ephemeral substrate carrying every emission this picker
/// tick produced for this cat. Empty `rows` means the L2 author falls
/// through to the default `Intention::Activity { Idle, .. }` wrap; the
/// picker removes the Component entirely when no emission applies so
/// the L2 query's `get` returns `None`.
#[derive(Component, Debug, Clone, Default, serde::Serialize)]
pub struct AspirationEmissions {
    pub rows: Vec<EmissionRow>,
}

impl AspirationEmissions {
    /// Empty emission set — equivalent to "no Component" semantically.
    /// Convenience constructor for tests; the picker removes the
    /// Component instead of inserting an empty one.
    pub fn empty() -> Self {
        Self { rows: Vec::new() }
    }

    /// Highest-`Priority` row, used by the L2 author site to pick the
    /// emission that overrides the default Activity wrap. Returns
    /// `None` when `rows` is empty.
    pub fn winner(&self) -> Option<&EmissionRow> {
        // `min_by_key` over `priority as u8` picks Primary (0) before
        // Secondary (1) before Tertiary (2); registration order acts
        // as the tiebreaker within tier because the picker walks each
        // milestone's `emits` in declaration order.
        self.rows.iter().min_by_key(|r| r.priority as u8)
    }
}
