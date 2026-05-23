//! HTN method registry (ticket 319 — child #1 of the 128 epic).
//!
//! Single source of truth for which HTN methods exist, are live, and
//! are dormant pending substrate. The registry is exhibitable to
//! ECS systems via [`MethodRegistry`]; populated at startup by
//! `populate_method_registry` in `src/plugins/simulation.rs` (parallel
//! to `populate_dse_registry` and `populate_influence_map_registry`).
//!
//! At landing of 319 the registry is empty by design — types and
//! infrastructure first, methods authored in tickets 320 onward. The
//! `scripts/check_method_registry.sh` lint passes vacuously on the
//! empty registry; the first `PendingSubstrate` method to land
//! exercises the bidirectional check (method → open ticket exists;
//! ticket frontmatter `wires-method:` references method back).
//!
//! Spec: [`docs/systems/htn-methods.md`](../../../docs/systems/htn-methods.md)
//! §Architecture (types) + §Dormant-method discipline (`ApplicableWhen`).
//!
//! ## Method-declaration convention (read before authoring methods in 320+)
//!
//! Every `Method` literal that uses `ApplicableWhen::PendingSubstrate`
//! MUST sit in a single multi-line struct-literal block inside a file
//! under `src/ai/methods/`, with `id: MethodId("<slug>")` on its own
//! line and `blocker: "<ticket-id>"` on its own line within the same
//! block. The bash lint walks `Method {` → `}` boundaries to extract
//! the `(method-id, blocker)` pair.

use bevy_ecs::prelude::*;

use crate::ai::dse::GoalState;
use crate::ai::Action;

// ---------------------------------------------------------------------------
// MethodId — stable slug, parallel to `DseId`
// ---------------------------------------------------------------------------

/// Stable identifier for a registered HTN method. Kept as a
/// `&'static str` newtype, parallel to [`crate::ai::dse::DseId`].
/// Adding a method is writing a string constant, not extending a
/// closed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct MethodId(pub &'static str);

impl std::fmt::Display for MethodId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

// ---------------------------------------------------------------------------
// TargetHint — per §6.3 target-taking DSE binding
// ---------------------------------------------------------------------------

/// Hint passed to a primitive sub-goal's target-taking DSE. Only the
/// spec-named variant (`Partner`, per `docs/systems/htn-methods.md:773`)
/// ships at 319; method authors in 320+ extend this enum as real
/// authoring needs surface. Don't pre-populate speculative variants —
/// the §6.3 target-taking DSEs are the authoritative consumers, and
/// adding a variant without a consuming DSE is dead substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetHint {
    Partner,
    /// 321 — primitive sub-goal binds to the cat's prey-target picker
    /// (Hunt-DSE's existing target resolution). Used by `hunt_method`
    /// as the combine-and-test slice's primitive target hint.
    Prey,
    /// 327 — primitive sub-goal binds to the cat's threat-target picker
    /// (Fight-DSE's existing target resolution: nearest hostile creature
    /// within engagement range, mediated by `HasThreatNearby` marker).
    /// Used by `fight_method` as WARRIORS_PATH's primary emit target.
    Threat,
    /// 327 — primitive sub-goal binds to the cat's safe-ground picker
    /// (Flee-DSE's existing target resolution: nearest tile satisfying
    /// the safety axis). Used by `flee_method` as WARRIORS_PATH's
    /// survival-fallback emit target.
    SafeGround,
    /// 332 — primitive sub-goal binds to the cat's grave-target picker
    /// (`pick_grave_for_mourner` — the cat's mourned `Grave` entity,
    /// looked up by `Mourning.deceased_name == Grave.deceased_name`).
    /// Used by `mourn_at_grave`'s three primitive sub-goals
    /// (vigil_at_grave / grieve_in_den / release_grief).
    Grave,
    /// 333 — primitive sub-goal binds to the cat's dependent-kitten
    /// picker (`pick_dependent_kitten_for_mother` — any kitten Entity
    /// whose `KittenDependency.mother == Some(self)`, scored by
    /// maturity for the wean/teach/release stage gate). Used by
    /// `rear_kitten`'s three primitive sub-goals.
    DependentKitten,
    /// 326 — primitive sub-goal binds to the cat's social-partner
    /// picker (Socialize-DSE's existing target resolution in
    /// `src/ai/dses/socialize_target.rs`: a proximate cat scored by
    /// bond / familiarity / affiliation). Used by `socialize_method`
    /// as HEART_OF_THE_COLONY's primary emit target.
    SocialPartner,
    /// 326 — primitive sub-goal binds to the cat's grooming-target
    /// picker (GroomOther-DSE's existing target resolution in
    /// `src/ai/dses/groom_other_target.rs`: a proximate partner / kin /
    /// affiliated cat with grooming need). Used by `groom_other_method`
    /// as THE_BELOVED's primary emit target.
    GroomingTarget,
}

// ---------------------------------------------------------------------------
// MethodFailure — backtrack strategy per htn-methods.md §Architecture
// ---------------------------------------------------------------------------

/// What happens when a sub-goal of this method abandons. Read by the
/// L2 evaluator's `HeldGoalStack` walker (ticket 320) when a leaf
/// primitive fails or a sub-goal recursion drops out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodFailure {
    /// Try the next applicable method for this goal_label. If no
    /// remaining method is applicable, propagate failure to the
    /// parent frame.
    Backtrack,
    /// Abandon the goal entirely; propagate failure to the parent
    /// frame without trying sibling methods.
    Abandon,
    /// Reset `sub_goal_index = 0` and retry this method up to
    /// `max_attempts` times before falling through to `Backtrack`.
    Retry { max_attempts: u8 },
}

// ---------------------------------------------------------------------------
// SubGoal — recursive (compound) or primitive (leaf) decomposition
// ---------------------------------------------------------------------------

/// One step in a method's `sub_goals` list. Compound steps recurse
/// into another method's `goal_label`; primitive steps name a leaf
/// `Action` and a target-taking hint that the L2 evaluator resolves
/// per §6.3.
pub enum SubGoal {
    /// Recursive compound task. Looked up against `MethodRegistry`
    /// at evaluator time; depth-cap enforced by `HeldGoalStack`
    /// (ticket 320).
    Goal(GoalState),
    /// Primitive leaf — a single DSE invocation.
    Primitive {
        /// Short-form label for traces / narrative emission.
        label: &'static str,
        /// The leaf `Action` the resolver chains to.
        action: Action,
        /// Hint for the target-taking DSE that resolves a candidate.
        target_hint: TargetHint,
    },
}

impl std::fmt::Debug for SubGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Goal(g) => f.debug_tuple("Goal").field(g).finish(),
            Self::Primitive {
                label,
                action,
                target_hint,
            } => f
                .debug_struct("Primitive")
                .field("label", label)
                .field("action", action)
                .field("target_hint", target_hint)
                .finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// ApplicableWhen — typed-dormancy enum (§Dormant-method discipline)
// ---------------------------------------------------------------------------

/// When is this method available for selection? `Live` methods are
/// scored normally; `PendingSubstrate` methods are filtered out by
/// [`MethodRegistry::lookup`] until their wiring ticket lands.
///
/// `eventual` on a `PendingSubstrate` variant compiles but is never
/// called by the L2 evaluator — it documents the predicate the method
/// *will* use once its blocker ticket lands and substrate is wired.
/// At that point the variant flips to `Live(eventual)` and the
/// `scripts/check_method_registry.sh` cross-check stops anchoring
/// on this method.
///
/// Discipline (enforced by `scripts/check_method_registry.sh`): every
/// `PendingSubstrate { blocker }` MUST name an open ticket in
/// `docs/open-work/tickets/`, AND that ticket's frontmatter MUST carry
/// `wires-method: [<this-method-id>...]` referencing back.
pub enum ApplicableWhen {
    Live(fn(&World, Entity) -> bool),
    PendingSubstrate {
        /// Ticket id (e.g. `"324"`) of the open glue ticket whose
        /// landing wires this method live. Verified open by the lint.
        blocker: &'static str,
        /// Predicate this method will use once the blocker lands.
        /// Compiles for type-checking; never called by the evaluator
        /// while the variant is `PendingSubstrate`.
        eventual: fn(&World, Entity) -> bool,
    },
}

impl std::fmt::Debug for ApplicableWhen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Live(_) => f.write_str("Live(<fn>)"),
            Self::PendingSubstrate { blocker, .. } => f
                .debug_struct("PendingSubstrate")
                .field("blocker", blocker)
                .finish_non_exhaustive(),
        }
    }
}

// ---------------------------------------------------------------------------
// Method — the registry entry
// ---------------------------------------------------------------------------

/// A single HTN method: "to satisfy goal `goal_label`, decompose into
/// `sub_goals` provided `applicable_when` holds for the candidate
/// `(world, cat)`."
///
/// Methods register at app build via `populate_method_registry` in
/// `src/plugins/simulation.rs`, parallel to `populate_dse_registry`.
/// Const-table authoring (F.E.A.R.-style HTN, per
/// `docs/systems/htn-methods.md` §Literature alignment) is the
/// expected idiom; multiple methods may share a `goal_label` to
/// support `MethodFailure::Backtrack`.
pub struct Method {
    /// Canonical slug. Unique across the registry. Authors pick a
    /// snake-case identifier; the lint regex-extracts the literal
    /// `MethodId("<slug>")` from source.
    pub id: MethodId,
    /// Matches `GoalState.label` for the compound task this method
    /// decomposes. Multiple methods may share a `goal_label`; the
    /// registry returns the first whose `applicable_when` holds.
    pub goal_label: &'static str,
    /// Selection gate. `Live` methods are scored normally;
    /// `PendingSubstrate` methods are filtered out by `lookup`.
    pub applicable_when: ApplicableWhen,
    /// Ordered decomposition. Compound entries recurse via the
    /// registry; primitive entries chain to a leaf `Action`.
    pub sub_goals: &'static [SubGoal],
    /// What happens when a sub-goal abandons.
    pub failure_strategy: MethodFailure,
    /// Ticket 321 — optional aspiration-domain tag. Read by the L1→L2
    /// picker's domain-affinity fallback (§H step 3): when a cat's
    /// active aspiration has no `Emit` row whose label matches a Live
    /// method, the picker iterates `MethodRegistry` for any Live
    /// method whose `domain == Some(aspiration.domain)` and uses the
    /// first match's `goal_label` as the fallback emission.
    /// `None` means "this method has no aspiration affinity"; it
    /// can still be emitted via an authored `Emit` row but never
    /// catches the fallback path. PendingSubstrate methods authored
    /// pre-321 carry `None` by default.
    pub domain: Option<crate::components::aspirations::AspirationDomain>,
}

impl std::fmt::Debug for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Method")
            .field("id", &self.id)
            .field("goal_label", &self.goal_label)
            .field("applicable_when", &self.applicable_when)
            .field("sub_goals_len", &self.sub_goals.len())
            .field("failure_strategy", &self.failure_strategy)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// MethodRegistry — Resource (parallel to DseRegistry / InfluenceMapRegistry)
// ---------------------------------------------------------------------------

/// Catalog of all HTN methods, populated at `Startup` by
/// `populate_method_registry`. Single source of truth for which
/// methods exist and which are dormant pending substrate.
///
/// `lookup` returns the first applicable `Live` method for a given
/// `goal_label`. `PendingSubstrate` methods are filtered out — they
/// compile and register, but `lookup` skips them so the L2 evaluator
/// falls through to the existing 126 adoption path
/// (`src/systems/goap.rs:2597-2604`) for any goal whose only methods
/// are dormant.
///
/// Backtracking (`MethodFailure::Backtrack`) requires walking *all*
/// applicable methods; 320 will add the iterator entry point when it
/// wires the registry into the L2 evaluator. At 319 the single-
/// `lookup` shape suffices because no caller exists yet.
#[derive(Resource, Default)]
pub struct MethodRegistry {
    methods: Vec<Method>,
}

impl MethodRegistry {
    /// Push a method into the registry. Called from
    /// `populate_method_registry` only — no runtime registration path.
    pub fn push(&mut self, method: Method) {
        self.methods.push(method);
    }

    /// Return the first applicable `Live` method for `goal_label`.
    /// `PendingSubstrate` methods are filtered out unconditionally
    /// (their `eventual` predicate is never invoked while dormant).
    ///
    /// Returns `None` when no method matches or when every matching
    /// method is dormant — the L2 evaluator's no-method fallback (the
    /// 126 adoption path) handles both cases identically.
    pub fn lookup(&self, goal_label: &str, world: &World, entity: Entity) -> Option<&Method> {
        self.methods.iter().find(|m| {
            m.goal_label == goal_label
                && match &m.applicable_when {
                    ApplicableWhen::Live(check) => check(world, entity),
                    ApplicableWhen::PendingSubstrate { .. } => false,
                }
        })
    }

    /// Total method count (live + dormant). Used by the
    /// `just methods` audit surface and tests.
    pub fn len(&self) -> usize {
        self.methods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.methods.is_empty()
    }

    /// All methods, ordered as registered. Used by `just methods`
    /// for the audit surface and by the L2 evaluator (320) when
    /// walking backtrack candidates.
    pub fn iter(&self) -> impl Iterator<Item = &Method> {
        self.methods.iter()
    }

    /// Ticket 320 caller: return a [`MethodPushSpec`] for the first
    /// matching method whose `applicable_when` is not
    /// `PendingSubstrate`. Skips the `Live(check)` predicate
    /// invocation — `check` requires `&World`, and 320's L2 author
    /// site is a non-exclusive system that cannot accept `&World`.
    /// At 320's land the registry contains zero `Live` methods, so
    /// this function is unreachable in production; it exists so the
    /// wiring compiles end-to-end and the gate becomes hot at 321's
    /// picker land (which authors the first `Intention::Goal` with
    /// a label) and 323's `courtship_method` land (the first `Live`
    /// method). 323 (or a follow-on) revisits this site to invoke
    /// `check` properly — either by promoting the L2 author to an
    /// exclusive system or by routing the check through a sibling
    /// world-aware system.
    pub fn lookup_spec_dormant_filtered(&self, goal_label: &str) -> Option<MethodPushSpec> {
        self.methods
            .iter()
            .find(|m| {
                m.goal_label == goal_label && matches!(m.applicable_when, ApplicableWhen::Live(_))
            })
            .map(MethodPushSpec::from_method)
    }

    /// Ticket 364 caller: look up a method by its [`MethodId`]. Used by
    /// the L2 frame-pin (adopt hook) to consult the held frame's method
    /// and read its `sub_goals[sub_goal_index]` payload. Linear scan;
    /// the registry is small (≤20 methods today) so this is O(n) but
    /// the constant is tiny.
    pub fn lookup_by_id(&self, id: MethodId) -> Option<&Method> {
        self.methods.iter().find(|m| m.id == id)
    }

    /// Ticket 364 caller: iterate every method matching `goal_label`
    /// whose `applicable_when` evaluates true for `entity`. Used by the
    /// HTN backtrack hook (resolve_goap_plans) to walk sibling Live
    /// methods after one fails per its `MethodFailure::Backtrack`
    /// strategy. `PendingSubstrate` rows are filtered out (their
    /// `eventual` predicate is never invoked while dormant). Walks the
    /// registry in insertion order; callers may filter further (e.g.
    /// skip the current method id).
    pub fn iter_applicable_for<'a>(
        &'a self,
        goal_label: &'a str,
        world: &'a World,
        entity: Entity,
    ) -> impl Iterator<Item = &'a Method> + 'a {
        self.methods.iter().filter(move |m| {
            m.goal_label == goal_label
                && match &m.applicable_when {
                    ApplicableWhen::Live(check) => check(world, entity),
                    ApplicableWhen::PendingSubstrate { .. } => false,
                }
        })
    }
}

/// Plain-old-data slice of a [`Method`] sufficient for the 320 L2
/// author site to push a [`crate::components::GoalFrame`] without
/// holding a borrow on the `MethodRegistry` resource. The fields are
/// copies of the `'static` references on `Method`, so the spec is
/// `'static`-lifetime and cheap to copy across the recursion frames
/// when the L2 gate walks compound sub-goals.
#[derive(Debug, Clone, Copy)]
pub struct MethodPushSpec {
    pub id: MethodId,
    pub goal_label: &'static str,
    pub sub_goals: &'static [SubGoal],
    pub failure_strategy: MethodFailure,
}

impl MethodPushSpec {
    fn from_method(m: &Method) -> Self {
        Self {
            id: m.id,
            goal_label: m.goal_label,
            sub_goals: m.sub_goals,
            failure_strategy: m.failure_strategy,
        }
    }
}

// 322: dormant HTN-method modules. Each file declares one or more
// `ApplicableWhen::PendingSubstrate` methods whose `blocker` field
// points at the open ticket that flips the method to Live. The
// `scripts/check_method_registry.sh` lint walks every `Method { … }`
// literal in these files and enforces the bidirectional dormancy
// gate (blocker → open ticket → ticket's `wires-method:` references
// the method id).
pub mod acquire_stealth;
// 321: Live HTN method module — the combine-and-test slice that
// exercises the picker→L2-wrap→320-gate path end-to-end at 321 land.
// `hunt_method` carries `ApplicableWhen::Live`, distinguishing it
// from the dormant Tier-2 modules below.
pub mod hunt;
// 327: Live HTN method modules — combine-and-test slice for the
// Combat chain. `fight_method` catches `engage_threat`; `flee_method`
// catches `flee_to_safety`. Both are Tier-1 Live primitives, mirroring
// `hunt_method`'s 321 shape. WARRIORS_PATH emits both labels;
// SHADOW_FIGHTER (Patrol-based) lands in a follow-on ticket.
pub mod fight;
pub mod flee;
pub mod mourn_at_grave;
pub mod rear_kitten;
// 398: Live HTN method module — single-primitive `caretake_kitten`
// catches the `caretake_kitten` label that `RAISE_OFFSPRING_ASPIRATION`'s
// dormant emit row will fire once Phase 1c/1d's unified softmax +
// persistence-bonus land. Registers Live at 398 Phase 1a so the
// picker's `MethodRegistry::lookup` check resolves cleanly when the
// row activates.
pub mod caretake_kitten;
// 323: Tier-1 Live HTN method — first method that mirrors a 127
// `JointIntention` practice end-to-end. `courtship_method` catches the
// `courtship_completed` label on any cat carrying `JointIntention {
// practice: Courtship, .. }` and decomposes the four `PracticeStage`
// values into four primitive sub-goals. See module doc for the
// stage-sync rationale and the #340 upgrade path.
pub mod courtship;
// 340: Tier-1 Live HTN method — worked-example landing of the 128
// epic. `mate_with_goal` retargets the legacy `build_mating_chain` (a
// hand-coded `[Socialize, GroomOther, Mate]` template that lived in
// the unscheduled `disposition_to_chain`) onto the registry as three
// primitive sub-goals. Composes with `courtship_method` via the
// `SubGoal::Goal(GoalState { label: "mating_event_completed" })`
// recursion seam — see module doc.
pub mod mating;
// 326: Live HTN method modules — combine-and-test slice for the Social
// chain. `socialize_method` catches `socialize` (Primary emit on every
// HEART_OF_THE_COLONY milestone); `groom_other_method` catches
// `groom_other` (Primary emit on every THE_BELOVED milestone). Tier-1
// Live primitives mirroring `build_method`'s 330 shape.
pub mod groom_other;
pub mod socialize;

// Tests live in `tests.rs` so the bash lint can exclude that path
// while scanning `src/ai/methods/` for production `Method` literals.
#[cfg(test)]
mod tests;
