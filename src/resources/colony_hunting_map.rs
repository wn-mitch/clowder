use bevy_ecs::prelude::*;

use crate::components::hunting_priors::{HuntingPriors, DEFAULT_PRIOR};

/// Colony-wide hunting belief map — read by `best_direction` and the
/// `HuntingBeliefSnapshot` visualization. Internally a `HuntingPriors`
/// grid (legacy shape; preserves snapshot.rs and best_direction's reader
/// surface untouched), but the **values are now derived** each cadence
/// from the per-cat `LocationBeliefs.prey_yield` facet via
/// [`crate::systems::belief_aggregation::aggregate_location_belief_snapshot`].
///
/// Ticket 293: replaces the legacy social-transmission `absorb` pathway
/// (socialize / groom_other `colony.absorb(cat) + cat.learn_from(colony)`)
/// with substrate-derived aggregation. Cross-cat consensus is now implicit
/// in the aggregator's max-over-cats-with-strength-floor rule. The
/// per-cat `HuntingPriors` component still exists (legacy
/// `best_direction` reader); it retires in the 293 cutover commit.
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

    /// Reset every cell to `DEFAULT_PRIOR` (uninformative). Called by the
    /// rebuild system before overlaying the substrate-derived values.
    pub fn reset_to_prior(&mut self) {
        for cell in self.beliefs.beliefs.iter_mut() {
            *cell = DEFAULT_PRIOR;
        }
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
    fn reset_to_prior_clears_all_cells() {
        let mut colony = ColonyHuntingMap::default();
        colony.beliefs.record_catch(&Position::new(20, 20));
        colony.reset_to_prior();
        assert_eq!(colony.beliefs.get(20, 20), DEFAULT_PRIOR);
    }
}
