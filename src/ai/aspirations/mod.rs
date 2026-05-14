//! Ticket 321 — `&'static`-shaped aspiration substrate.
//!
//! Replaces the RON-deserialized `Milestone` / `AspirationChain` shape
//! that lived in [`crate::components::aspirations`] (pre-321) with
//! code-defined const data. The migration is required by the
//! per-milestone `emits: &'static [Emit]` table from
//! [`docs/systems/htn-methods.md`](../../../../docs/systems/htn-methods.md)
//! §H — `Emit::applicable_when: fn(&World, Entity) -> bool` is a
//! function pointer and cannot deserialize from RON.
//!
//! All 14 production chains land in this module across seven domain
//! files; [`ALL_CHAINS`] is the registry walked at app build by
//! [`crate::resources::aspiration_registry::AspirationRegistry::build_static`].
//!
//! # Per-milestone `emits` (the 321 picker contract)
//!
//! Each milestone carries `emits: &'static [Emit]`. The L1→L2 emission
//! picker (`crate::systems::aspiration_picker`) walks the active
//! cat's current milestone's `emits` list per tick and, for each row,
//! checks (a) that the named goal-label resolves to a Live method in
//! `MethodRegistry` and (b) that the per-row `applicable_when`
//! predicate holds. The first matching row produces an
//! `Intention::Goal` candidate that ticket 320's HTN gate catches.
//!
//! At 321's combine-and-test land only `hunting::MASTER_OF_THE_HUNT`'s
//! "First Blood" milestone carries a non-empty `emits` table; every
//! other milestone declares `emits: &[]`. Per-chain wrapper tickets
//! (#325–#331) fill the rest.

use bevy_ecs::prelude::*;

use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;
use crate::components::mental::MemoryType;
use crate::resources::relationships::BondType;

pub mod building;
pub mod combat;
pub mod exploration;
pub mod herbcraft;
pub mod hunting;
pub mod leadership;
pub mod social;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Default `gate` predicate — every cat with the chain active is
/// eligible. Per-milestone life-stage / colony-state gating lands as
/// follow-on work when authoring needs surface.
pub fn always_true(_world: &World, _entity: Entity) -> bool {
    true
}

// ---------------------------------------------------------------------------
// SkillKind — typed skill axis (replaces stringly-typed RON keys)
// ---------------------------------------------------------------------------

/// Typed skill axis for [`ProgressTracker::SkillLevel`]. Mirrors the
/// six numeric fields on [`crate::components::skills::Skills`]; the
/// resolver reads the matching field at progress-check time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillKind {
    Hunting,
    Foraging,
    Herbcraft,
    Building,
    Combat,
    Magic,
}

impl SkillKind {
    /// Read the corresponding numeric value from a `Skills` snapshot.
    pub fn value(self, skills: &crate::components::skills::Skills) -> f32 {
        match self {
            Self::Hunting => skills.hunting,
            Self::Foraging => skills.foraging,
            Self::Herbcraft => skills.herbcraft,
            Self::Building => skills.building,
            Self::Combat => skills.combat,
            Self::Magic => skills.magic,
        }
    }
}

// ---------------------------------------------------------------------------
// ProgressTracker — what increments milestone progress
// ---------------------------------------------------------------------------

/// Per-milestone progress / completion predicate. Replaces the
/// stringly-typed pre-321 `MilestoneCondition`. Each variant maps 1:1
/// to a former RON case; `ActionCount` is now a slice of `Action`
/// variants so a milestone can count any action in a domain bucket
/// (the herbcraft chains' "Herbcraft" RON key fanned across the three
/// `HerbcraftX` Action variants post-155; using a slice captures that
/// without minting an `Action::Herbcraft` alias).
#[derive(Debug, Clone, Copy)]
pub enum ProgressTracker {
    /// Count completions of any listed action. Pre-321's RON-only
    /// `ActionCount(action: "Herbcraft", ...)` referenced a string
    /// that no `Action` variant stringifies to (the 155 fan-out left
    /// the milestone silently broken); the slice form lets a milestone
    /// count any of the post-155 `HerbcraftX` actions toward a single
    /// progress total. Single-action milestones use a one-element slice.
    ActionCount {
        actions: &'static [Action],
        count: u32,
    },
    SkillLevel {
        skill: SkillKind,
        level: f32,
    },
    FormBond {
        bond_type: BondType,
    },
    WitnessEvent {
        event_type: MemoryType,
        count: u32,
    },
    Mentor {
        count: u32,
    },
}

// ---------------------------------------------------------------------------
// Priority + Emit — the picker's emission tuple
// ---------------------------------------------------------------------------

/// Per-`Emit` row tier. The picker walks `emits` in `Priority` order
/// (Primary first), then by registration order within tier. First
/// match wins per htn-methods.md §H.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Priority {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

/// One row in a milestone's `emits` table. Names a candidate
/// `Intention::Goal` label, a per-cat applicability predicate, the
/// `CommitmentStrategy` the emitted Intention carries, and the tier.
#[derive(Debug, Clone, Copy)]
pub struct Emit {
    pub label: &'static str,
    pub applicable_when: fn(&World, Entity) -> bool,
    pub strategy: CommitmentStrategy,
    pub priority: Priority,
}

// ---------------------------------------------------------------------------
// Milestone + AspirationChain — `&'static`-shaped (was RON pre-321)
// ---------------------------------------------------------------------------

/// One milestone in an aspiration chain. Per htn-methods.md §H the
/// shape carries:
///
/// - `name`: short-form label for narrative + trace emission.
/// - `gate`: per-cat eligibility predicate (life-stage / colony-state
///   gating). 321 land: every milestone declares `gate: always_true`;
///   per-milestone gates land as follow-on work.
/// - `progress_tracker`: what increments progress + completion check.
/// - `emits`: the L1→L2 picker's per-row candidate table.
/// - `narrative_on_complete`: narrative-emitter template with the
///   existing `{name}`, `{possessive}`, `{subject}`, `{object}` slots.
#[derive(Debug, Clone, Copy)]
pub struct Milestone {
    pub name: &'static str,
    pub gate: fn(&World, Entity) -> bool,
    pub progress_tracker: ProgressTracker,
    pub emits: &'static [Emit],
    pub narrative_on_complete: &'static str,
}

/// A full aspiration chain — ordered milestones plus a chain-completion
/// narrative. Lives as `&'static` const data per `&'static [Milestone]`
/// + `&'static str` fields; listed in [`ALL_CHAINS`] so the registry
///   walk doesn't need to know individual chain names.
#[derive(Debug, Clone, Copy)]
pub struct AspirationChain {
    pub name: &'static str,
    pub domain: AspirationDomain,
    pub milestones: &'static [Milestone],
    pub completion_narrative: &'static str,
}

// ---------------------------------------------------------------------------
// ALL_CHAINS — the registry walk surface
// ---------------------------------------------------------------------------

/// Every production aspiration chain in registration order. Walked at
/// app build by `AspirationRegistry::build_static`. Adding a chain is
/// one line here plus a `pub const` in the matching domain module.
pub const ALL_CHAINS: &[&AspirationChain] = &[
    &hunting::MASTER_OF_THE_HUNT,
    &hunting::PROVIDER_OF_THE_COLONY,
    &combat::WARRIORS_PATH,
    &combat::SHADOW_FIGHTER,
    &social::HEART_OF_THE_COLONY,
    &social::THE_BELOVED,
    &herbcraft::WHISKERWEAVERS_APPRENTICE,
    &herbcraft::HEALERS_CALLING,
    &exploration::MAPMAKER,
    &exploration::BEYOND_THE_BORDER,
    &building::DEN_SHAPER,
    &building::THE_ARCHITECT,
    &leadership::VOICE_OF_THE_COLONY,
    &leadership::THE_UNIFIER,
];
