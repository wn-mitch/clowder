use bevy_ecs::prelude::*;

use crate::components::building::{Structure, StructureType};
use crate::components::identity::Species;
use crate::components::mental::Mood;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::resources::colony_score::ColonyScore;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::SimConstants;
use crate::resources::snapshot_config::SnapshotConfig;
use crate::resources::system_activation::{FeatureCategory, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};

// ---------------------------------------------------------------------------
// Welfare computation helpers
// ---------------------------------------------------------------------------

/// Fraction of living cats within a functional den's shelter radius.
fn compute_shelter(
    cats: &[(Position,)],
    dens: &[(Position, &Structure)],
    den_shelter_radius: i32,
) -> f32 {
    if cats.is_empty() {
        return 0.0;
    }
    let sheltered = cats
        .iter()
        .filter(|(cat_pos,)| {
            dens.iter().any(|(den_anchor, structure)| {
                let center = structure.center(den_anchor);
                let eff = structure.effectiveness();
                eff > 0.0 && cat_pos.manhattan_distance(&center) <= den_shelter_radius
            })
        })
        .count();
    sheltered as f32 / cats.len() as f32
}

/// Average hunger across living cats.
fn compute_nourishment(needs: &[&Needs]) -> f32 {
    if needs.is_empty() {
        return 0.0;
    }
    needs.iter().map(|n| n.hunger).sum::<f32>() / needs.len() as f32
}

/// Average health across living cats.
fn compute_health(healths: &[f32]) -> f32 {
    if healths.is_empty() {
        return 0.0;
    }
    healths.iter().sum::<f32>() / healths.len() as f32
}

/// Average effective mood valence, remapped from [-1, 1] to [0, 1].
fn compute_happiness(moods: &[f32]) -> f32 {
    if moods.is_empty() {
        return 0.0;
    }
    let avg = moods.iter().sum::<f32>() / moods.len() as f32;
    ((avg + 1.0) / 2.0).clamp(0.0, 1.0)
}

/// Average of level 3-5 Maslow needs weighted by suppression.
///
/// For each cat, we take their belonging, esteem, and self-actualisation
/// satisfaction levels, each scaled by the cat's level suppression. This
/// captures whether cats are actually *able to pursue* higher needs, not
/// just whether the raw values are high.
fn compute_fulfillment(needs: &[&Needs]) -> f32 {
    if needs.is_empty() {
        return 0.0;
    }
    let sum: f32 = needs
        .iter()
        .map(|n| {
            let belonging = ((n.social + n.acceptance) / 2.0) * n.level_suppression(3);
            let esteem = ((n.respect + n.mastery) / 2.0) * n.level_suppression(4);
            let purpose = n.purpose * n.level_suppression(5);
            (belonging + esteem + purpose) / 3.0
        })
        .sum();
    sum / needs.len() as f32
}

// ---------------------------------------------------------------------------
// emit_colony_score system
// ---------------------------------------------------------------------------

/// Emit a `ColonyScore` event at the configured interval. Also updates
/// `seasons_survived` and `peak_population` in the ledger.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn emit_colony_score(
    config: Res<SnapshotConfig>,
    time: Res<TimeState>,
    sim_config: Res<SimConfig>,
    constants: Res<SimConstants>,
    activation: Res<SystemActivation>,
    relationships: Res<Relationships>,
    cat_query: Query<(&Position, &Needs, &Health, &Mood), (With<Species>, Without<Dead>)>,
    den_query: Query<(&Position, &Structure), Without<Species>>,
    mut score: ResMut<ColonyScore>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let interval = config.economy_interval;
    if interval == 0 || !time.tick.is_multiple_of(interval) {
        return;
    }

    // --- Update season counter ---
    let tps = sim_config.ticks_per_season;
    if tps > 0 {
        let current_season = time.tick / tps;
        if current_season > score.last_recorded_season {
            score.seasons_survived += current_season - score.last_recorded_season;
            score.last_recorded_season = current_season;
        }
    }

    // --- Gather cat data ---
    let cat_positions: Vec<(Position,)> = cat_query.iter().map(|(p, _, _, _)| (*p,)).collect();
    let needs: Vec<&Needs> = cat_query.iter().map(|(_, n, _, _)| n).collect();
    let healths: Vec<f32> = cat_query.iter().map(|(_, _, h, _)| h.current).collect();
    let effective_moods: Vec<f32> = cat_query
        .iter()
        .map(|(_, _, _, m)| {
            let mod_sum: f32 = m.modifiers.iter().map(|md| md.amount).sum();
            (m.valence + mod_sum).clamp(-1.0, 1.0)
        })
        .collect();

    let living_cats = cat_positions.len() as u64;

    // Update peak population.
    if living_cats > score.peak_population {
        score.peak_population = living_cats;
    }

    // --- Gather den data ---
    let dens: Vec<(Position, &Structure)> = den_query
        .iter()
        .filter(|(_, s)| s.kind == StructureType::Den)
        .map(|(p, s)| (*p, s))
        .collect();

    // --- Compute welfare axes ---
    let cs = &constants.colony_score;
    let shelter = compute_shelter(&cat_positions, &dens, cs.den_shelter_radius);
    let nourishment = compute_nourishment(&needs);
    let health = compute_health(&healths);
    let happiness = compute_happiness(&effective_moods);
    let fulfillment = compute_fulfillment(&needs);

    let welfare = (shelter + nourishment + health + happiness + fulfillment) / 5.0;

    // --- Compute activation score (positive features only) ---
    //
    // Negative features (deaths, corruption, etc.) and neutral features
    // (ecology churn) are tracked separately so the aggregate doesn't reward
    // colony distress.
    let positive_activation_score = activation
        .positive_activation_score(cs.activation_breadth_bonus, cs.activation_depth_bonus);
    let positive_features_active = activation.features_active_in(FeatureCategory::Positive);
    let positive_features_total = SystemActivation::features_total_in(FeatureCategory::Positive);
    let negative_events_total = activation.negative_event_count();
    let neutral_features_active = activation.features_active_in(FeatureCategory::Neutral);
    let neutral_features_total = SystemActivation::features_total_in(FeatureCategory::Neutral);

    let aggregate = score.aggregate(welfare, positive_activation_score, cs);

    // --- Bond tier snapshot ---
    let mut friends_count = 0u64;
    let mut partners_count = 0u64;
    let mut mates_count = 0u64;
    for (_, rel) in relationships.iter() {
        match rel.bond {
            Some(BondType::Friends) => friends_count += 1,
            Some(BondType::Partners) => partners_count += 1,
            Some(BondType::Mates) => mates_count += 1,
            _ => {}
        }
    }

    // --- Emit events ---
    let Some(ref mut log) = event_log else { return };

    // Colony score snapshot.
    log.push(
        time.tick,
        EventKind::ColonyScore {
            shelter,
            nourishment,
            health,
            happiness,
            fulfillment,
            welfare,

            seasons_survived: score.seasons_survived,
            bonds_formed: score.bonds_formed,
            peak_population: score.peak_population,
            deaths_starvation: score.deaths_starvation,
            deaths_old_age: score.deaths_old_age,
            deaths_injury: score.deaths_injury,
            aspirations_completed: score.aspirations_completed,
            structures_built: score.structures_built,
            kittens_born: score.kittens_born,
            prey_dens_discovered: score.prey_dens_discovered,

            friends_count,
            partners_count,
            mates_count,

            aggregate,
            positive_activation_score,
            positive_features_active,
            positive_features_total,
            negative_events_total,
            neutral_features_active,
            neutral_features_total,
            living_cats,
        },
    );

    // System activation snapshot, grouped by feature valence. Every feature
    // in `Feature::ALL` is emitted — including ones that have never fired —
    // so analysis tooling can distinguish "no event yet" from "dead system"
    // without consulting a parallel classification table.
    use crate::resources::system_activation::Feature;
    let mut positive = std::collections::HashMap::new();
    let mut negative = std::collections::HashMap::new();
    let mut neutral = std::collections::HashMap::new();
    for feature in Feature::ALL {
        let count = activation.counts.get(feature).copied().unwrap_or(0);
        let bucket = match feature.category() {
            FeatureCategory::Positive => &mut positive,
            FeatureCategory::Negative => &mut negative,
            FeatureCategory::Neutral => &mut neutral,
        };
        bucket.insert(format!("{feature:?}"), count);
    }
    log.push(
        time.tick,
        EventKind::SystemActivation {
            positive,
            negative,
            neutral,
        },
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::resources::sim_constants::ColonyScoreConstants;

    fn test_shelter_radius() -> i32 {
        ColonyScoreConstants::default().den_shelter_radius
    }

    #[test]
    fn shelter_all_cats_in_range() {
        let cats = vec![(Position::new(7, 7),), (Position::new(8, 6),)];
        let den_structure = Structure::new(StructureType::Den); // condition 1.0
        let dens = vec![(Position::new(5, 5), &den_structure)];
        // Den center = (6, 6). Cat (7,7) → dist 2. Cat (8,6) → dist 2.
        let score = compute_shelter(&cats, &dens, test_shelter_radius());
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn shelter_no_dens() {
        let cats = vec![(Position::new(5, 5),)];
        let dens: Vec<(Position, &Structure)> = vec![];
        assert_eq!(compute_shelter(&cats, &dens, test_shelter_radius()), 0.0);
    }

    #[test]
    fn shelter_cat_out_of_range() {
        let cats = vec![
            (Position::new(7, 7),),   // dist 2 from center (6,6) — in range
            (Position::new(20, 20),), // far away
        ];
        let den_structure = Structure::new(StructureType::Den);
        let dens = vec![(Position::new(5, 5), &den_structure)];
        let score = compute_shelter(&cats, &dens, test_shelter_radius());
        assert!((score - 0.5).abs() < 1e-6);
    }

    #[test]
    fn shelter_non_functional_den_ignored() {
        let cats = vec![(Position::new(7, 7),)];
        let mut den_structure = Structure::new(StructureType::Den);
        den_structure.condition = 0.1; // below 0.2 → effectiveness 0
        let dens = vec![(Position::new(5, 5), &den_structure)];
        assert_eq!(compute_shelter(&cats, &dens, test_shelter_radius()), 0.0);
    }

    #[test]
    fn nourishment_averages_hunger() {
        let n1 = Needs {
            hunger: 0.8,
            ..Needs::default()
        };
        let n2 = Needs {
            hunger: 0.4,
            ..Needs::default()
        };
        let score = compute_nourishment(&[&n1, &n2]);
        assert!((score - 0.6).abs() < 1e-6);
    }

    #[test]
    fn health_averages() {
        assert!((compute_health(&[1.0, 0.5]) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn happiness_maps_range() {
        // Mood -1 → 0, mood 0 → 0.5, mood 1 → 1.0
        assert!((compute_happiness(&[-1.0]) - 0.0).abs() < 1e-6);
        assert!((compute_happiness(&[0.0]) - 0.5).abs() < 1e-6);
        assert!((compute_happiness(&[1.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fulfillment_empty() {
        assert_eq!(compute_fulfillment(&[]), 0.0);
    }

    #[test]
    fn all_welfare_empty_is_zero() {
        assert_eq!(compute_shelter(&[], &[], test_shelter_radius()), 0.0);
        assert_eq!(compute_nourishment(&[]), 0.0);
        assert_eq!(compute_health(&[]), 0.0);
        assert_eq!(compute_happiness(&[]), 0.0);
    }
}
