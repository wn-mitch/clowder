//! Ticket 332 — `Mourning` actor-private substrate.
//!
//! Per-cat marker that a colony-mate's death has produced active
//! grief. Read by the `mourn_at_grave` HTN method's `applicable_when`
//! gate so the method only applies to cats currently mourning a
//! specific deceased member; written by whichever future system
//! assigns grief on death (out of scope per #332's §Out of scope —
//! tracked alongside the §7.7.b grief-event-emission debt under
//! 060's Phase 6b row).
//!
//! # Substrate placement (§4.7.2)
//!
//! `Mourning` is **substrate**: no `StateEffect::Set*` mutates it
//! during A* expansion (the planner runs over `PlannerState`; this
//! Component is invisible to the planner); external authorship by a
//! to-be-authored grief-trigger system (the §7.7.b debt). Mirrors
//! [`Pregnant`](super::pregnancy::Pregnant)'s shape — a long-lived
//! marker carrying the parameters of the multi-tick state.
//!
//! # Actor-private
//!
//! Like [`HeldIntention`](super::held_intention) and
//! [`HeldGoalStack`](super::held_goal_stack), never read across cats.
//! A grieving cat's `Mourning` is its own commitment to the arc;
//! observers learn about the death from the deceased's `Grave` entity
//! (mutually-public substrate authored at burial), not from inspecting
//! another cat's `Mourning`.
//!
//! # Why `deceased_name: String` (not `Entity`)
//!
//! The deceased entity is despawned at burial (per `Grave`'s comment
//! in [`grave.rs`](super::grave)) — a stale `Entity` handle would
//! break `Grave`-target lookup. `Grave.deceased_name` and
//! `Mourning.deceased_name` are the durable identity link, matching
//! the convention `Grave` already uses for `Relationships` reverse-
//! lookup.

use bevy_ecs::prelude::*;

/// A cat actively mourning a specific deceased colony-mate. Insertion
/// path is the §7.7.b grief-event-emission follow-on (out of scope
/// for #332). Removal path is the terminal `release_grief` sub-goal
/// of the `mourn_at_grave` HTN method, executed via
/// [`crate::steps::disposition::resolve_release_grief`] once the
/// HTN-driven action dispatch wiring lands.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mourning {
    /// Display name of the deceased — matches `Grave.deceased_name`
    /// for the cat being mourned. The name is the durable link;
    /// `Entity` handles for the deceased are invalid post-burial.
    pub deceased_name: String,
    /// Tick the mourning state was authored. Drives `ticks_in_arc`
    /// for the future grief-cascade severity tuning thread.
    pub started_tick: u64,
}

impl Mourning {
    pub const KEY: &str = "Mourning";
}
