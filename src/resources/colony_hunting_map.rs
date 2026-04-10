use bevy_ecs::prelude::*;

use crate::components::hunting_priors::HuntingPriors;

/// Colony-wide shared hunting belief map. Updated through social transmission
/// when cats interact — individual experience feeds into collective knowledge.
///
/// All cats can read this map as a baseline. Individual `HuntingPriors` on each
/// cat overlay personal experience on top.
#[derive(Resource, Debug, Clone)]
pub struct ColonyHuntingMap {
    pub beliefs: HuntingPriors,
}

impl ColonyHuntingMap {
    pub fn new(map_w: i32, map_h: i32) -> Self {
        Self {
            beliefs: HuntingPriors::new(map_w as usize, map_h as usize, 5),
        }
    }

    /// Incorporate a cat's personal beliefs into the colony map.
    /// Called during social interactions.
    pub fn absorb(&mut self, cat_priors: &HuntingPriors, weight: f32) {
        self.beliefs.learn_from(cat_priors, weight);
    }
}

impl Default for ColonyHuntingMap {
    fn default() -> Self {
        Self::new(120, 90)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Position;

    #[test]
    fn absorb_updates_colony_beliefs() {
        let mut colony = ColonyHuntingMap::default();
        let mut cat = HuntingPriors::default();
        cat.record_catch(&Position::new(20, 20));
        cat.record_catch(&Position::new(20, 20));

        colony.absorb(&cat, 0.3);
        assert!(
            colony.beliefs.get(20, 20) > 0.5,
            "colony should learn from cat's experience"
        );
    }
}
