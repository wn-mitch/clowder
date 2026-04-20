use bevy_ecs::prelude::*;

use crate::components::building::StructureType;

/// Colony-wide ledger of "frustrated wants": a cat wanted to perform an
/// action that depends on a specific structure, but no such structure
/// exists (or isn't functional). The coordinator uses this signal to
/// prioritize the missing infrastructure in BuildPressure.
///
/// This models the real-world feedback loop where repeated unmet demand
/// drives organizations to invest in the tool/building that would resolve
/// it. Each frustrated attempt adds a small amount; accumulated demand
/// translates to faster BuildPressure accumulation on the matching
/// channel. The ledger decays slowly so that stale frustration fades
/// once the infrastructure is in place.
#[derive(Resource, Debug, Clone, Default)]
pub struct UnmetDemand {
    /// Times a cat wanted to cook but no functional Kitchen existed (or
    /// no raw food was available to cook).
    pub kitchen: f32,
    /// Reserved for future use — a cat wanting to do magic with no
    /// Workshop, etc. Keep the struct extensible.
    pub workshop: f32,
    pub garden: f32,
}

impl UnmetDemand {
    /// Per-frustrated-attempt increment. Small so single cats don't
    /// dominate the signal, but with enough cats attempting repeatedly,
    /// it accumulates toward the pressure threshold.
    pub const INCREMENT: f32 = 0.05;
    /// Decay applied per assessment cycle. 0.9 means the ledger halves
    /// every ~7 cycles when no new frustration arrives — fast enough
    /// that stale demand fades once the building exists, slow enough
    /// that a spiky pattern of attempts still accumulates.
    pub const DECAY: f32 = 0.9;

    pub fn record(&mut self, kind: StructureType) {
        match kind {
            StructureType::Kitchen => self.kitchen += Self::INCREMENT,
            StructureType::Workshop => self.workshop += Self::INCREMENT,
            StructureType::Garden => self.garden += Self::INCREMENT,
            // Other structures don't have a matching "advanced action"
            // that cats directly want — they're defensive/logistic.
            _ => {}
        }
    }

    pub fn decay(&mut self) {
        self.kitchen *= Self::DECAY;
        self.workshop *= Self::DECAY;
        self.garden *= Self::DECAY;
    }

    pub fn of(&self, kind: StructureType) -> f32 {
        match kind {
            StructureType::Kitchen => self.kitchen,
            StructureType::Workshop => self.workshop,
            StructureType::Garden => self.garden,
            _ => 0.0,
        }
    }
}
