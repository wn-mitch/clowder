use bevy_ecs::prelude::*;

/// Physical grooming condition. 0.0 = matted/filthy, 1.0 = pristine.
///
/// A physical property — not a Maslow need. Decays passively and is restored
/// by grooming actions. Other systems read it to modulate social and esteem
/// outcomes.
#[derive(Component, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GroomingCondition(pub f32);

impl Default for GroomingCondition {
    fn default() -> Self {
        Self(0.8)
    }
}
