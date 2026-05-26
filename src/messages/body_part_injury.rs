//! `BodyPartInjury` — emitted whenever `damage_to_body_part` (ticket 095
//! Phase 1) lands non-negligible damage on a `CatBodyModel` part. Read by
//! the L1 focal-cat trace emitter and by Feature-canary recorders.
//!
//! Stage A wires this alongside the legacy `Injury` push, so the new
//! substrate gains a write-witness while old readers stay on
//! `Health.injuries`. Stage B retires the legacy push.

use bevy_ecs::prelude::*;

use crate::components::body_zones::{BodyPart, PartCondition, WoundKind};
use crate::components::physical::InjurySource;

#[derive(Message, Debug, Clone, Copy)]
pub struct BodyPartInjury {
    pub entity: Entity,
    pub part: BodyPart,
    /// Increment applied to `tissue_damage` on this tick. Always positive.
    pub tissue_damage_delta: f32,
    /// Post-application condition. The caller can compare against the prior
    /// condition (read from `CatBodyModel.part(part).condition` before
    /// `apply_damage`) if a condition-change predicate is needed.
    pub condition: PartCondition,
    pub source: InjurySource,
    /// 472 — wound flavor applied this tick. `Normal` for combat /
    /// wildlife / starvation; `Festering` for the magic-misfire
    /// `WoundTransfer` arm (gated by
    /// `MagicConstants::misfire_festering_chance`).
    pub kind: WoundKind,
    pub tick: u64,
}
