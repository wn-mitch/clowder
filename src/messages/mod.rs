//! Bevy Messages emitted by simulation systems and consumed by downstream
//! readers.
//!
//! New convention introduced by ticket 258: substrate-wide messages live in
//! their own module rather than co-located with a domain type. Existing
//! messages (`PreyKilled`, `DenRaided`, `PlanNarrative`, `CorruptionPushback`,
//! `JointInteractionObserved`) remain next to their domain types; later
//! tickets may migrate them here.

pub mod body_part_injury;
pub mod cat_moved;
pub mod fox_lifecycle;
pub mod misfire_effect;
pub mod witnessable_event;
