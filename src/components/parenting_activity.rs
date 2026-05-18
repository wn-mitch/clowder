//! L2 ParentingActivity — ticket 400 lifelong relational stance substrate.
//!
//! `ParentingActivity` is the **lifelong relational stance** layer. The
//! existing `Has<Parent>` marker (`src/components/markers.rs`) tracks active
//! parenthood — "has at least one living dependent kitten" — and turns off
//! when the last kitten matures or dies. `ParentingActivity` carries a
//! `Vec<RelationshipTo>` whose entries persist for the cat's life: kitten
//! maturity drops the engagement asymptote to a residual (~0.15× full),
//! kitten death preserves the entry with frustrated target-taking (the
//! foundation for the §7.7.b grief cascade), partner death clears the
//! `partner` field but the entry stays. The Component is dropped only on
//! the owner cat's own death (Bevy's entity-despawn cleanup handles this
//! automatically).
//!
//! # Adoption pathways (`ParentalKind`)
//!
//! Four kinds; only Biological + InLaw are wired in 400. Substrate is
//! ready from day one for the other two (gated to follow-on tickets
//! 403/404).
//!
//! - **Biological** — `update_parenting_activity_biological` (in
//!   `src/systems/parenting_activity.rs`) mirrors `update_parent_markers`'s
//!   pattern: each tick, for each living `KittenDependency`, ensure both
//!   the mother and father carry a `Biological`-kind `RelationshipTo`
//!   targeting that kitten.
//! - **InLaw** — `src/ai/joint_intention.rs` adds entries on
//!   `PracticeStage::CourtshipBonded` stage advance: each partner's
//!   biological parents gain an `InLaw`-kind entry targeting the other
//!   partner (mirrored both directions).
//! - **BondFormed** — follow-on 403; accumulated bond-witness threshold.
//! - **Adopted** — follow-on 404; explicit colony event (orphan
//!   integration / formal adoption).
//!
//! BondFormed and Adopted are designated mythic-texture canary
//! contributors (≥1 named event per sim year, alongside Fate-awakening /
//! ShadowFox-banishment) when their adoption rules ship.
//!
//! # Engagement gradient
//!
//! `parental_engagement` ramps via EMA toward an asymptote derived from
//! the owner's `Personality` and the dependent's lifecycle phase. Asymptote
//! computation lives in `src/systems/parenting_activity.rs` (the
//! `parental_engagement_asymptote` + `scale_*` helpers); the modifier
//! pipeline reads the per-cat sums via `ctx_scalars` populated by
//! `populate_parenting_scalars`.
//!
//! # Discipline
//!
//! Mirror `JointIntention`'s discipline — fields are *observable practice
//! state* (bond strength, engagement gradient, lifecycle tick markers).
//! Internal heart-state (private wishes, the actor's commitment strength)
//! stays in `HeldIntention`. The Component is mutated by author/sync
//! systems, not by A* search-state (§4.7).

use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// ParentalKind — adoption pathway taxonomy
// ---------------------------------------------------------------------------

/// How a `RelationshipTo` entry came to exist on a cat's
/// `ParentingActivity`. All four variants are first-class substrate;
/// 400 wires `Biological` + `InLaw` adoption rules. `BondFormed` +
/// `Adopted` are declared so the Vec architecture is ready from day one;
/// adoption-rule wiring belongs to follow-on tickets 403 / 404.
///
/// `#[non_exhaustive]` so future kinds (e.g., FosterTransitive,
/// CommunityWard) can extend the enum without breaking archived trace
/// deserialization.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[non_exhaustive]
pub enum ParentalKind {
    /// Biological parenthood — adopted on first tick a `KittenDependency`
    /// exists naming this cat as `mother` or `father`. Strongest initial
    /// bond (`bond_strength = 1.0`).
    Biological,
    /// In-law parenthood via `PracticeStage::CourtshipBonded` transition.
    /// On Bonded entry, each partner's biological parents gain an `InLaw`
    /// entry targeting the other partner. Lower initial bond strength
    /// (`0.3`).
    InLaw,
    /// **Follow-on 403.** Bond-witness threshold accumulation —
    /// auntie/uncle/close-friend who become parental over time.
    BondFormed,
    /// **Follow-on 404.** Explicit colony event — orphan integration or
    /// formal adoption.
    Adopted,
}

impl ParentalKind {
    /// Stable slug for trace serialization + diagnostic readout.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Biological => "biological",
            Self::InLaw => "in_law",
            Self::BondFormed => "bond_formed",
            Self::Adopted => "adopted",
        }
    }

    /// Initial `bond_strength` for a freshly-inserted entry of this kind.
    /// Biological is unity; InLaw is 0.3 per 399 design (a respected but
    /// secondary relation). BondFormed / Adopted defaults are placeholder
    /// until follow-on tickets specify their adoption rules.
    pub const fn initial_bond_strength(self) -> f32 {
        match self {
            Self::Biological => 1.0,
            Self::InLaw => 0.3,
            Self::BondFormed => 0.5,
            Self::Adopted => 0.7,
        }
    }
}

// ---------------------------------------------------------------------------
// RelationshipTo — one parental relationship to one target cat
// ---------------------------------------------------------------------------

/// One parental relationship entry on a cat's `ParentingActivity`.
///
/// Created by an adoption rule (Biological sync system OR InLaw stage-
/// transition hook OR future BondFormed/Adopted rules). Never removed by
/// the sync system — persistence is the design contract:
///
/// - Kitten matures → entry stays; the engagement asymptote drops to
///   `matured_residual_factor × asymptote` (still your mother).
/// - Kitten dies → entry stays; target-taking finds nothing (the
///   desire-target gap IS the grief mechanic, per §7.7.b foundation).
/// - Partner dies → `partner` field clears to `None`; engagement
///   re-asymptotes from the owner's own Personality.
/// - Owner dies → Bevy's entity despawn drops the whole Component.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RelationshipTo {
    /// The other cat (offspring, in-law, etc.). `Entity` has no `Default`
    /// so `#[serde(skip)]` — the trace pipeline renders the entry by
    /// position within `relationships`, not by entity id.
    #[serde(skip)]
    pub target: Entity,
    /// Adoption pathway. See [`ParentalKind`].
    pub kind: ParentalKind,
    /// Accumulated bond strength in `[0.0, 1.0]`. Initialized per
    /// [`ParentalKind::initial_bond_strength`]; future bond-witness
    /// substrate may increment this.
    pub bond_strength: f32,
    /// Dynamic engagement gradient in `[0.0, 1.0]`. Ramps via EMA toward
    /// a personality-derived asymptote when the owner is in proximity to
    /// `target` OR performing a parental-class action. Decays via EMA
    /// when neither holds. The `ParentingActivityModifier` reads
    /// (`bond_strength × parental_engagement`) summed per DSE × per-scale
    /// formula to produce its per-DSE lift.
    pub parental_engagement: f32,
    /// Co-parent for this target, when one exists. Used by the
    /// JointIntention-aware suppression on Caretake bias (yield to
    /// partner if they already hold Caretake for our dependent). Often
    /// `None` for non-Biological adoption kinds.
    #[serde(skip)]
    pub partner: Option<Entity>,
    /// Tick this entry was first inserted on the owner's
    /// `ParentingActivity`. Diagnostic only.
    pub entered_tick: u64,
    /// Most-recent tick the owner was in proximity to `target` OR
    /// performed a parental-class action toward `target`. Drives the EMA
    /// build/decay decision. Equal to `entered_tick` at insertion.
    pub last_interaction_tick: u64,
}

impl RelationshipTo {
    /// Convenience constructor for a freshly-inserted entry. Sets
    /// `bond_strength` from `kind.initial_bond_strength()`,
    /// `parental_engagement = 0.0` (builds dynamically),
    /// `last_interaction_tick = entered_tick = tick`.
    pub fn new(target: Entity, kind: ParentalKind, partner: Option<Entity>, tick: u64) -> Self {
        Self {
            target,
            kind,
            bond_strength: kind.initial_bond_strength(),
            parental_engagement: 0.0,
            partner,
            entered_tick: tick,
            last_interaction_tick: tick,
        }
    }
}

// ---------------------------------------------------------------------------
// ParentingActivity Component
// ---------------------------------------------------------------------------

/// L2 `ParentingActivity` Component persisted on the cat. Ticket 400.
///
/// Inserted by `update_parenting_activity_biological`
/// (`src/systems/parenting_activity.rs`) on a cat's first tick of
/// biological parenthood; subsequently appended to (never shrunk) by
/// adoption-rule systems. Bevy drops the Component on entity despawn, so
/// the "DROP only on self-death" contract is automatic.
///
/// Only `Serialize` is derived (not `Deserialize`) because
/// `RelationshipTo.target: Entity` has no `Default` and the Component is
/// pure runtime state — no save/load path round-trips it. The trace
/// pipeline reads it via `Serialize` only, mirroring `JointIntention`'s
/// pattern.
///
/// The `relationships` Vec may be empty (a freshly-inserted Component
/// before any entry is appended), but typical adult-cat lifetimes carry
/// 1-5 entries (1-3 biological children + InLaw entries from partner's
/// parents).
#[derive(Component, Debug, Clone, serde::Serialize, Default)]
pub struct ParentingActivity {
    /// All parental relationships this cat carries, ordered by insertion
    /// time. Never shrunk by the sync system — the design contract is
    /// "lifecycle endings preserve substrate state, not destroy it"
    /// (memory: `project_lifecycle_endings_preserve_state`).
    pub relationships: Vec<RelationshipTo>,
}

impl ParentingActivity {
    /// Find an existing entry by target (linear scan; `relationships` is
    /// typically very short).
    pub fn find(&self, target: Entity) -> Option<&RelationshipTo> {
        self.relationships.iter().find(|r| r.target == target)
    }

    /// Find an existing entry by target (mutable).
    pub fn find_mut(&mut self, target: Entity) -> Option<&mut RelationshipTo> {
        self.relationships.iter_mut().find(|r| r.target == target)
    }

    /// True iff an entry exists for `target` with `kind`. Used by sync
    /// systems to avoid duplicate insertion.
    pub fn has_kind(&self, target: Entity, kind: ParentalKind) -> bool {
        self.relationships
            .iter()
            .any(|r| r.target == target && r.kind == kind)
    }
}
