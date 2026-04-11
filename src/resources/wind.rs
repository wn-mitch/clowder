use bevy_ecs::prelude::*;

/// Global wind direction and strength. Affects scent carry for hunting and
/// adds environmental flavor to narrative.
///
/// Wind rotates slowly over time and couples to weather: storms randomize
/// direction and boost strength; calm weather dampens it.
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindState {
    /// Current angle in radians. Direction is derived as (cos, sin).
    pub angle: f32,
    /// Wind strength from 0.0 (dead calm) to 1.0 (gale).
    pub strength: f32,
}

impl WindState {
    /// Normalized wind direction vector derived from angle.
    pub fn direction(&self) -> (f32, f32) {
        (self.angle.cos(), self.angle.sin())
    }
}

impl Default for WindState {
    fn default() -> Self {
        Self {
            angle: 0.0,
            strength: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_is_unit_length() {
        let wind = WindState {
            angle: 1.2,
            strength: 0.7,
        };
        let (dx, dy) = wind.direction();
        let len = (dx * dx + dy * dy).sqrt();
        assert!(
            (len - 1.0).abs() < 1e-5,
            "direction should be unit length, got {len}"
        );
    }

    #[test]
    fn default_has_moderate_strength() {
        let wind = WindState::default();
        assert!(wind.strength > 0.0 && wind.strength < 1.0);
    }
}
