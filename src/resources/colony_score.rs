use bevy_ecs::prelude::*;

use crate::resources::sim_constants::ColonyScoreConstants;

// ---------------------------------------------------------------------------
// ColonyScore
// ---------------------------------------------------------------------------

/// Cumulative ledger of colony-wide achievements and milestones.
///
/// Point-in-time welfare axes are computed fresh each emission by the
/// `emit_colony_score` system — they don't live here. This resource only
/// tracks counters that accumulate over the life of a simulation run.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ColonyScore {
    /// Total season transitions survived (at least 1 cat alive).
    pub seasons_survived: u64,
    /// Total bonds formed (Friends + Partners + Mates).
    pub bonds_formed: u64,
    /// Highest simultaneous living cat count observed.
    pub peak_population: u64,
    /// Deaths by starvation.
    pub deaths_starvation: u64,
    /// Deaths by old age (a life well-lived).
    pub deaths_old_age: u64,
    /// Deaths by injury.
    pub deaths_injury: u64,
    /// Total aspiration chains completed across all cats.
    pub aspirations_completed: u64,
    /// Total buildings constructed.
    pub structures_built: u64,
    /// Number of kittens born in this simulation run.
    pub kittens_born: u64,
    /// Living cats that were born in-sim (not founding members).
    #[serde(default)]
    pub kittens_surviving: u64,
    /// Placeholder — incremented when prey den discovery ships.
    pub prey_dens_discovered: u64,
    /// Shadow-foxes banished by the colony. Each one is a Legend-tier event.
    #[serde(default)]
    pub banishments: u64,
    /// Season number at last season-tick update, to detect transitions.
    pub last_recorded_season: u64,
}

impl ColonyScore {
    /// Compute the achievement portion of the aggregate score from the
    /// cumulative ledger using the provided scoring weights.
    pub fn achievement_points(&self, cs: &ColonyScoreConstants) -> f64 {
        self.bonds_formed as f64 * cs.bonds_weight
            + self.aspirations_completed as f64 * cs.aspirations_weight
            + self.structures_built as f64 * cs.structures_weight
            + self.kittens_born as f64 * cs.kittens_weight
            + self.prey_dens_discovered as f64 * cs.prey_dens_weight
            - self.deaths_starvation as f64 * cs.deaths_starvation_penalty
            - self.deaths_injury as f64 * cs.deaths_injury_penalty
            + self.deaths_old_age as f64 * cs.deaths_old_age_bonus
    }

    /// Compute the full aggregate score given a welfare snapshot and activation score.
    ///
    /// Callers should pass the **positive-only** activation score — mixing in
    /// negative features (deaths, corruption) would cause the aggregate to
    /// reward colony distress. See `SystemActivation::positive_activation_score`.
    pub fn aggregate(
        &self,
        welfare: f32,
        positive_activation_score: f64,
        cs: &ColonyScoreConstants,
    ) -> f64 {
        let time_multiplier = self.seasons_survived.max(1) as f64;
        welfare as f64 * time_multiplier + self.achievement_points(cs) + positive_activation_score
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zeroed() {
        let score = ColonyScore::default();
        assert_eq!(score.seasons_survived, 0);
        assert_eq!(score.bonds_formed, 0);
        assert_eq!(score.peak_population, 0);
        assert_eq!(score.deaths_starvation, 0);
        assert_eq!(score.deaths_old_age, 0);
        assert_eq!(score.deaths_injury, 0);
        assert_eq!(score.aspirations_completed, 0);
        assert_eq!(score.structures_built, 0);
        assert_eq!(score.kittens_born, 0);
        assert_eq!(score.prey_dens_discovered, 0);
    }

    #[test]
    fn achievement_points_rewards_and_penalties() {
        let cs = ColonyScoreConstants::default();
        let mut score = ColonyScore::default();
        score.bonds_formed = 3;
        score.deaths_starvation = 1;
        score.deaths_old_age = 2;

        // 3*10 + 0 + 0 + 0 + 0 - 1*30 - 0 + 2*5 = 30 - 30 + 10 = 10
        assert!((score.achievement_points(&cs) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn aggregate_scales_welfare_by_seasons() {
        let cs = ColonyScoreConstants::default();
        let mut score = ColonyScore::default();
        score.seasons_survived = 4;

        // welfare 0.8, seasons 4, no activation → 0.8 * 4 + 0 + 0 = 3.2
        let agg = score.aggregate(0.8, 0.0, &cs);
        assert!((agg - 3.2).abs() < 1e-6);
    }

    #[test]
    fn aggregate_minimum_one_season_multiplier() {
        let cs = ColonyScoreConstants::default();
        let score = ColonyScore::default();
        // seasons_survived = 0 → clamped to 1, no activation
        let agg = score.aggregate(0.5, 0.0, &cs);
        assert!((agg - 0.5).abs() < 1e-6);
    }

    #[test]
    fn aggregate_includes_activation_score() {
        let cs = ColonyScoreConstants::default();
        let score = ColonyScore::default();
        let agg = score.aggregate(0.5, 100.0, &cs);
        // 0.5 * 1 + 0 + 100.0 = 100.5
        assert!((agg - 100.5).abs() < 1e-6);
    }
}
