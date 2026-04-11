use bevy_ecs::prelude::*;

/// Stage of gestation — determines physical effect multipliers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GestationStage {
    Early,
    Mid,
    Late,
}

/// Marks a cat as pregnant and tracks gestation state.
///
/// Inserted when a mating chain completes. Removed at birth (after
/// `ticks_per_season` ticks of gestation), at which point kitten entities
/// are spawned.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Pregnant {
    pub conceived_tick: u64,
    #[serde(skip)]
    pub partner: Option<Entity>,
    pub litter_size: u8,
    pub stage: GestationStage,
    /// Running sum of queen's hunger during gestation.
    pub nutrition_sum: f32,
    /// Number of nutrition samples taken.
    pub nutrition_samples: u32,
}

impl Pregnant {
    pub fn new(conceived_tick: u64, partner: Entity, litter_size: u8) -> Self {
        Self {
            conceived_tick,
            partner: Some(partner),
            litter_size,
            stage: GestationStage::Early,
            nutrition_sum: 0.0,
            nutrition_samples: 0,
        }
    }

    /// Average queen nutrition during pregnancy (0.0–1.0).
    pub fn avg_nutrition(&self) -> f32 {
        if self.nutrition_samples == 0 {
            0.5
        } else {
            self.nutrition_sum / self.nutrition_samples as f32
        }
    }
}
