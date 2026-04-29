//! `Dse` trait + Intention vocabulary — §L2.10 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! A Decision-Scoring-Element (DSE) is the L2 unit of deliberation:
//! a named scoring function that (a) filters eligibility via ECS
//! markers, (b) evaluates 4–8 considerations through a curve each,
//! (c) composes them into a single `[0, 1]` score, and (d) emits an
//! `Intention` describing what the cat would commit to if this DSE
//! wins selection.
//!
//! This module defines the types only — registration, evaluation, and
//! modifier pipeline live in [`super::eval`] (Phase 3a task #8). The
//! first concrete DSE (`Eat`) lands in Phase 3b as the reference port.
//!
//! ## Why the trait is declarative, not opaque
//!
//! The spec has two candidate Dse shapes:
//!
//! - §L2.10.2 opaque: `fn score(&self, cat, ctx) -> f32`.
//! - Refactor-plan Phase 3a declarative: exposes `considerations()` +
//!   `composition()` so the evaluator (not the DSE) walks the axes.
//!
//! We use the declarative shape. Trace emission (§11.3 L2 record) needs
//! per-consideration inputs + scores + the composition step — an opaque
//! `score()` hides all of that and forces each DSE to implement its own
//! trace hook. The declarative shape makes §11.3 free: the evaluator
//! knows the structure and can emit records without per-DSE cooperation.

use bevy_ecs::prelude::*;

use super::composition::Composition;
use super::considerations::{Consideration, MarkerKey};

// ---------------------------------------------------------------------------
// DseId
// ---------------------------------------------------------------------------

/// Stable identifier for a registered DSE. Kept as a `&'static str`
/// per §5.6.9's open-set contract — adding a DSE is writing a string
/// constant, not extending a closed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DseId(pub &'static str);

impl std::fmt::Display for DseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

// ---------------------------------------------------------------------------
// CommitmentStrategy (§7.1 — Rao & Georgeff vocabulary)
// ---------------------------------------------------------------------------

/// Per-Intention commitment strategy. Names how aggressively the
/// persistence layer (§7.4) resists preemption. The strategy tag
/// rides on the Intention, not on the DSE (§L2.10.4) — so a single
/// DSE can emit context-dependent strategies (e.g. `Patrol` emits
/// `Blind` under high-threat, `SingleMinded` under routine).
///
/// Semantics lands in Phase 6 with the drop-trigger gate; Phase 3
/// ships the tag as declarative metadata so DSE authors can
/// commit the design-intent up front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitmentStrategy {
    /// Hold the Intention until it's achieved or the cat dies. Used
    /// rarely — only for lifecycle overrides (fox Dispersing) and
    /// under-threat Guarding. Dropping only on the `achievement_believed`
    /// predicate.
    Blind,
    /// Default for `Goal` Intentions. Drop on any of:
    /// `achievement_believed`, `achievable_believed == false`, or
    /// plan hard-fail (`replan_count ≥ max`).
    SingleMinded,
    /// Default for `Activity` Intentions. Drop additionally on desire
    /// drift (`still_goal == false`) — the activity's drop trigger
    /// *is* the satiation of the desire that produced it.
    OpenMinded,
}

impl CommitmentStrategy {
    /// Stable string slug for JSON trace emission. Used by the §11.3
    /// focal-cat L3Commitment record so tooling doesn't have to
    /// depend on `Debug` format (which could silently change).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blind => "Blind",
            Self::SingleMinded => "SingleMinded",
            Self::OpenMinded => "OpenMinded",
        }
    }
}

// ---------------------------------------------------------------------------
// GoalState
// ---------------------------------------------------------------------------

/// A goal Intention's target state. Carries both a log-able label
/// (for trace emission / narrative binding) and a predicate the
/// commitment layer calls to check `achievement_believed` (§7.2).
///
/// Phase 3a commits the type shape; predicate bodies are authored
/// per-DSE in Phase 3b+.
pub struct GoalState {
    /// Short-form label for logs and narrative emission. Matches the
    /// spec's parenthetical gloss, e.g. `"hunger_below_threshold"`.
    pub label: &'static str,
    /// Returns `true` when the goal is satisfied for `cat` in the
    /// current world state. Called from §7.2's reconsideration gate.
    pub achieved: fn(&World, Entity) -> bool,
}

impl std::fmt::Debug for GoalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalState")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl Clone for GoalState {
    fn clone(&self) -> Self {
        Self {
            label: self.label,
            achieved: self.achieved,
        }
    }
}

// ---------------------------------------------------------------------------
// ActivityKind — enumerated per §L2.10.3 + §L2.10.5
// ---------------------------------------------------------------------------

/// Sustained-activity labels per §L2.10.5. An `Intention::Activity`
/// carries one of these plus a `Termination`. Kept `#[non_exhaustive]`
/// so future registrations (new species, new aspiration domains) can
/// extend without forcing a match-arm churn.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivityKind {
    Patrol,
    Socialize,
    Wander,
    Idle,
    Explore,
    Rest,
    Allogroom,
    Mentor,
    Pairing,
    Coordinate,
    Scry,
    Commune,
    Avoid,
}

// ---------------------------------------------------------------------------
// Termination (§L2.10.4, 3 variants)
// ---------------------------------------------------------------------------

/// When an `Activity` Intention ends. Goal Intentions don't carry a
/// termination — they end when `achievement_believed` becomes true.
#[derive(Clone, Copy)]
pub enum Termination {
    /// Fixed duration in sim ticks.
    Ticks(u32),
    /// End when the predicate returns true. Evaluated by the
    /// commitment layer once per reconsideration cadence.
    UntilCondition(fn(&World, Entity) -> bool),
    /// No explicit termination — preempted whenever something else
    /// scores higher (modulo persistence bonus §7.4). The canonical
    /// `Idle` shape.
    UntilInterrupt,
}

impl std::fmt::Debug for Termination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ticks(n) => write!(f, "Ticks({n})"),
            Self::UntilCondition(_) => f.write_str("UntilCondition(<fn>)"),
            Self::UntilInterrupt => f.write_str("UntilInterrupt"),
        }
    }
}

// ---------------------------------------------------------------------------
// Intention (§L2.10.4, 2 variants + strategy tag)
// ---------------------------------------------------------------------------

/// What a DSE emits when it wins selection. Goal Intentions expand
/// into a GOAP plan; Activity Intentions run the activity runner
/// until their termination fires.
///
/// Every Intention carries its `CommitmentStrategy` per §L2.10.4 — the
/// strategy rides on the Intention, not the DSE, so context-dependent
/// strategy switching is expressible (e.g. Patrol emits `Blind` under
/// threat, `SingleMinded` routine).
#[derive(Debug, Clone)]
pub enum Intention {
    Goal {
        state: GoalState,
        strategy: CommitmentStrategy,
    },
    Activity {
        kind: ActivityKind,
        termination: Termination,
        strategy: CommitmentStrategy,
    },
}

impl Intention {
    pub fn strategy(&self) -> CommitmentStrategy {
        match self {
            Self::Goal { strategy, .. } => *strategy,
            Self::Activity { strategy, .. } => *strategy,
        }
    }

    pub fn is_goal(&self) -> bool {
        matches!(self, Self::Goal { .. })
    }

    pub fn is_activity(&self) -> bool {
        matches!(self, Self::Activity { .. })
    }
}

// ---------------------------------------------------------------------------
// EligibilityFilter (§4 marker filter + §9.3 stance gate)
// ---------------------------------------------------------------------------

/// Pre-scoring gate. Reads §4.3 markers + §9.3 faction stance to
/// decide whether a DSE is even a candidate before scoring runs.
/// Eligible DSEs are scored; ineligible ones are skipped entirely
/// per the §4 "avoiding the cost of computing a score that can't
/// win" principle.
///
/// Marker keys are `&'static str` per the open-set contract; the
/// evaluator resolves each key against a marker-query registry
/// (Phase 3a task #8). Keys that never resolve are a debug-level
/// warning, not a compile error.
#[derive(Debug, Clone, Default)]
pub struct EligibilityFilter {
    pub required: Vec<MarkerKey>,
    pub forbidden: Vec<MarkerKey>,
    /// Optional faction-stance requirement per §9.3. For target-taking
    /// DSEs; populated at registration time from the spec's §9.3 table
    /// (SocializeDse → {Same, Ally}, AttackDse → {Enemy, Prey}, etc.).
    /// `None` means no stance filter.
    pub required_stance: Option<crate::ai::faction::StanceRequirement>,
    /// Ticket 080 — when true, the DSE rejects any candidate target
    /// whose `Reserved` marker names a non-self owner during the
    /// reservation window. Per-cat (cat-side) eligibility evaluators
    /// ignore this flag because it has no candidate to consult; the
    /// flag is meaningful for target-taking DSE candidate prefiltering
    /// (`evaluate_target_taking` consults
    /// `EvalCtx::is_reserved_by_other` per candidate).
    pub require_unreserved: bool,
    /// Ticket 074 — when true, target-taking DSE evaluation scores 0.0
    /// for any candidate failing the `EvalCtx::target_alive` predicate
    /// (Dead / Banished / Incapacitated / despawned). Closes audit gap
    /// #3 — the "stale-target-still-scored" hole that let dispositions
    /// re-pick a dead cat tick after tick. The validity facts already
    /// live in the per-cat snapshot the resolvers read, so this avoids
    /// a parallel marker table.
    pub require_target_alive: bool,
}

impl EligibilityFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn require(mut self, marker: MarkerKey) -> Self {
        self.required.push(marker);
        self
    }

    pub fn forbid(mut self, marker: MarkerKey) -> Self {
        self.forbidden.push(marker);
        self
    }

    pub fn with_stance(mut self, stance: crate::ai::faction::StanceRequirement) -> Self {
        self.required_stance = Some(stance);
        self
    }

    /// Ticket 080 — opt this DSE into the resource-reservation gate.
    /// Target-taking DSE candidate prefilter drops candidates whose
    /// `Reserved` marker names a non-self owner during the reservation
    /// window.
    pub fn require_unreserved(mut self) -> Self {
        self.require_unreserved = true;
        self
    }

    /// Ticket 074 — gate the DSE on target-alive validity. The
    /// evaluator (`evaluate_target_taking`) consults
    /// `EvalCtx::target_alive` for each candidate and replaces the
    /// per-target score with 0.0 when the candidate is invalid.
    pub fn require_alive(mut self) -> Self {
        self.require_target_alive = true;
        self
    }
}

// ---------------------------------------------------------------------------
// EvalCtx (§1.1 ConsiderationCtx — the consideration's input surface)
// ---------------------------------------------------------------------------

/// Per-cat evaluation context handed to DSE consideration scoring.
/// §1.1 calls for three kinds of access:
///
/// - **Scalar state refs** — needs / personality / skills / health /
///   inventory aggregates. Pulled through the ECS queries the
///   evaluator-system owns; the context references borrow from
///   there.
/// - **ECS world access for marker queries** — via the
///   [`MarkerQuery`] closure type; resolves `(marker_key, entity) →
///   bool`.
/// - **Entity position resolution** — for `SpatialConsideration`
///   with `LandmarkSource::Entity(_)`, the evaluator looks up the
///   landmark entity's current `Position` via this closure. Returns
///   `None` for despawned or off-grid entities — the consideration
///   then scores zero rather than sampling a stale tile.
///
/// Phase 3a commits the shape; Phase 3b populates all three fields
/// from the evaluator system. The concrete signature stays flexible
/// (function-pointer closures, not trait objects) because the
/// evaluator captures different query sets per registration method.
pub struct EvalCtx<'ctx> {
    pub cat: Entity,
    pub tick: u64,
    /// Closure for per-cat marker queries.
    pub has_marker: &'ctx dyn Fn(&str, Entity) -> bool,
    /// Closure for landmark entity position lookup (used by
    /// `SpatialConsideration` with `LandmarkSource::Entity`).
    pub entity_position:
        &'ctx dyn Fn(Entity) -> Option<crate::components::physical::Position>,
    /// Closure for cat-relative anchor position lookup (used by
    /// `SpatialConsideration` with `LandmarkSource::Anchor`). The
    /// canonical self-state-DSE resolution path: per-cat-per-tick,
    /// the closure dispatches per-`LandmarkAnchor` against
    /// `ColonyLandmarks` resource positions, per-tick centroid
    /// snapshots, and per-cat queries (own-den, nearest-threat).
    /// Returns `None` when the anchor has no resolvable position
    /// (no kitchen built, no threat in range, empty frontier);
    /// the consideration scores zero in that case.
    pub anchor_position: &'ctx dyn Fn(
        super::considerations::LandmarkAnchor,
    ) -> Option<crate::components::physical::Position>,
    /// Cat's own position. Used as the origin for
    /// `SpatialConsideration` distance calculations.
    pub self_position: crate::components::physical::Position,
    /// Optional target entity (set only when the DSE is target-taking
    /// and the evaluator is scoring against a specific candidate).
    pub target: Option<Entity>,
    /// Optional target position (same condition as `target`).
    pub target_position: Option<crate::components::physical::Position>,
    /// Ticket 074 — per-candidate alive predicate consumed by
    /// `EligibilityFilter::require_target_alive`. Returns `true` iff
    /// the entity is a valid target (alive, not banished, not
    /// incapacitated, not despawned). `None` means callers haven't
    /// supplied a validity surface; in that case the evaluator treats
    /// every candidate as alive (no-op gate). When the DSE sets
    /// `require_target_alive = true` AND a closure is supplied, the
    /// per-target score is forced to 0.0 for any entity returning
    /// `false`.
    pub target_alive: Option<&'ctx dyn Fn(Entity) -> bool>,
}

// ---------------------------------------------------------------------------
// Dse trait
// ---------------------------------------------------------------------------

/// The Phase-3a declarative Dse shape. The evaluator calls
/// `considerations()` + `composition()` to compute a score, then
/// `emit()` to produce an Intention if this DSE wins selection.
pub trait Dse: Send + Sync + 'static {
    fn id(&self) -> DseId;
    fn considerations(&self) -> &[Consideration];
    fn composition(&self) -> &Composition;
    fn eligibility(&self) -> &EligibilityFilter;
    /// Default commitment strategy for Intentions this DSE emits.
    /// `emit()` can override on a per-context basis.
    fn default_strategy(&self) -> CommitmentStrategy;
    /// Build the Intention the DSE would commit to if its score
    /// wins selection. Called post-scoring; receives the winning
    /// score in case the DSE wants to condition strategy on it.
    fn emit(&self, score: f32, ctx: &EvalCtx) -> Intention;
    /// Maslow tier for the §3.4 pre-gate wrapper. 1=physiological,
    /// 5=self-actualization. DSEs outside Maslow (coordinator
    /// election, narrative selection) return `u8::MAX` to opt out
    /// of the gate.
    fn maslow_tier(&self) -> u8;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::markers;

    fn always_false(_: &World, _: Entity) -> bool {
        false
    }

    #[test]
    fn intention_goal_strategy() {
        let goal = GoalState {
            label: "hunger_below_threshold",
            achieved: always_false,
        };
        let i = Intention::Goal {
            state: goal,
            strategy: CommitmentStrategy::SingleMinded,
        };
        assert_eq!(i.strategy(), CommitmentStrategy::SingleMinded);
        assert!(i.is_goal());
        assert!(!i.is_activity());
    }

    #[test]
    fn intention_activity_strategy() {
        let i = Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        };
        assert_eq!(i.strategy(), CommitmentStrategy::OpenMinded);
        assert!(i.is_activity());
    }

    #[test]
    fn eligibility_builder() {
        let filter = EligibilityFilter::new()
            .require(markers::CanHunt::KEY)
            .forbid(markers::Incapacitated::KEY);
        assert_eq!(filter.required, vec![markers::CanHunt::KEY]);
        assert_eq!(filter.forbidden, vec![markers::Incapacitated::KEY]);
        assert!(filter.required_stance.is_none());
    }

    #[test]
    fn termination_ticks_roundtrip() {
        let t = Termination::Ticks(20);
        let dbg = format!("{t:?}");
        assert_eq!(dbg, "Ticks(20)");
    }

    #[test]
    fn dse_id_display() {
        let id = DseId("eat");
        assert_eq!(format!("{id}"), "eat");
    }

    #[test]
    fn commitment_strategy_as_str_covers_all_variants() {
        assert_eq!(CommitmentStrategy::Blind.as_str(), "Blind");
        assert_eq!(CommitmentStrategy::SingleMinded.as_str(), "SingleMinded");
        assert_eq!(CommitmentStrategy::OpenMinded.as_str(), "OpenMinded");
    }
}
