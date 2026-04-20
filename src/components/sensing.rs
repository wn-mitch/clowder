use bevy_ecs::prelude::*;

use crate::components::prey::PreyKind;
use crate::components::wildlife::WildSpecies;

// ---------------------------------------------------------------------------
// SensorySpecies — taxonomy spanning cats, wildlife predators, and prey
// ---------------------------------------------------------------------------

/// Identifies which species-level sensory profile applies to an entity.
///
/// The three variants cover the three taxonomies in the sim: colony cats
/// (single species), wildlife predators (`WildSpecies`), and prey animals
/// (`PreyKind`). Used as a key into the per-species `SensoryProfile` table
/// in `SimConstants`.
#[derive(
    Component, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum SensorySpecies {
    Cat,
    Wild(WildSpecies),
    Prey(PreyKind),
}

// ---------------------------------------------------------------------------
// SensorySignature — how detectable is this entity
// ---------------------------------------------------------------------------

/// Static detectability profile of an entity across sensory channels.
///
/// Each field is a baseline emission on [0.0, 1.0]. A cat has high visual
/// and moderate auditory/olfactory; prey emit more scent than visual; a
/// carcass emits strong scent but no sound.
///
/// `tremor_baseline` is the static substrate-vibration emission from body
/// mass. The effective tremor signature at detection time is
/// `tremor_baseline * action_multiplier(current_action)`, so stalking
/// emits far less than sprinting.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SensorySignature {
    pub visual: f32,
    pub auditory: f32,
    pub olfactory: f32,
    pub tremor_baseline: f32,
}

impl SensorySignature {
    pub const CAT: Self = Self {
        visual: 1.0,
        auditory: 0.6,
        olfactory: 0.4,
        tremor_baseline: 0.7,
    };

    pub const PREY: Self = Self {
        visual: 0.7,
        auditory: 0.3,
        olfactory: 0.8,
        tremor_baseline: 0.3,
    };

    pub const WILDLIFE: Self = Self {
        visual: 0.8,
        auditory: 0.5,
        olfactory: 0.9,
        tremor_baseline: 0.6,
    };

    pub const CARCASS: Self = Self {
        visual: 0.5,
        auditory: 0.0,
        olfactory: 1.0,
        tremor_baseline: 0.0,
    };

    pub const CORRUPTION: Self = Self {
        visual: 0.3,
        auditory: 0.0,
        olfactory: 0.9,
        tremor_baseline: 0.0,
    };
}

// ---------------------------------------------------------------------------
// SensoryModifier — role-based bonuses that stack onto the species profile
// ---------------------------------------------------------------------------

/// Additive bonuses to an observer's species-level sensory profile.
///
/// Represents role-based sensory differences (a Guard sees further, a
/// Hunter hears sharper) without duplicating the entire profile per role.
/// Multiple modifiers can be combined; see `SensoryModifier::combine`.
#[derive(
    Component, Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct SensoryModifier {
    pub sight_range_bonus: f32,
    pub hearing_range_bonus: f32,
    pub scent_range_bonus: f32,
    pub tremor_range_bonus: f32,
    pub sight_acuity_bonus: f32,
    pub hearing_acuity_bonus: f32,
    pub scent_acuity_bonus: f32,
    pub tremor_acuity_bonus: f32,
}

impl SensoryModifier {
    pub fn combine(self, other: Self) -> Self {
        Self {
            sight_range_bonus: self.sight_range_bonus + other.sight_range_bonus,
            hearing_range_bonus: self.hearing_range_bonus + other.hearing_range_bonus,
            scent_range_bonus: self.scent_range_bonus + other.scent_range_bonus,
            tremor_range_bonus: self.tremor_range_bonus + other.tremor_range_bonus,
            sight_acuity_bonus: self.sight_acuity_bonus + other.sight_acuity_bonus,
            hearing_acuity_bonus: self.hearing_acuity_bonus + other.hearing_acuity_bonus,
            scent_acuity_bonus: self.scent_acuity_bonus + other.scent_acuity_bonus,
            tremor_acuity_bonus: self.tremor_acuity_bonus + other.tremor_acuity_bonus,
        }
    }
}
