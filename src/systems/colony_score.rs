use bevy_ecs::prelude::*;

use crate::components::beliefs::{ShelterBeliefs, ShelterFacet};
use crate::components::identity::Species;
use crate::components::mental::Mood;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::resources::colony_score::{ColonyScore, ColonyScoreSnapshot};
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::{ShelterBeliefConstants, SimConstants};
use crate::resources::snapshot_config::SnapshotConfig;
use crate::resources::system_activation::{FeatureCategory, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};
use crate::systems::shelter_beliefs::shelter_security;

// ---------------------------------------------------------------------------
// Welfare computation helpers
// ---------------------------------------------------------------------------

/// 374: average per-cat housing-security belief across living cats.
/// Replaces the pre-374 spatial-proximity rollup (count of cats within
/// `den_shelter_radius` of a functional Den / total cats) with a
/// belief-side aggregation — each cat carries their own composed
/// security from `ShelterBeliefs.facet`, the colony score averages
/// the population.
///
/// Cats whose belief has not yet been seeded (`home_den == None`,
/// belonging = 0) contribute 0 — same as the pre-374 spatial
/// semantics where a cat far from any Den contributed 0.
fn compute_shelter(facets: &[&ShelterFacet], cfg: &ShelterBeliefConstants) -> f32 {
    if facets.is_empty() {
        return 0.0;
    }
    let sum: f32 = facets.iter().map(|f| shelter_security(f, cfg)).sum();
    sum / facets.len() as f32
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

/// Average of tier 3-5 Maslow needs weighted by suppression.
///
/// For each cat, we take their belonging, esteem, and self-actualisation
/// satisfaction values, each scaled by the cat's tier suppression. This
/// captures whether cats are actually *able to pursue* higher needs, not
/// just whether the raw values are high.
fn compute_fulfillment(needs: &[&Needs]) -> f32 {
    if needs.is_empty() {
        return 0.0;
    }
    let sum: f32 = needs
        .iter()
        .map(|n| {
            let belonging = ((n.social + n.acceptance) / 2.0) * n.tier_suppression(3);
            let esteem = ((n.respect + n.mastery) / 2.0) * n.tier_suppression(4);
            let purpose = n.purpose * n.tier_suppression(5);
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
    cat_query: Query<
        (&Position, &Needs, &Health, &Mood, &ShelterBeliefs),
        (With<Species>, Without<Dead>),
    >,
    mut score: ResMut<ColonyScore>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let interval = config.economy_interval;
    if interval == 0 || !time.tick.is_multiple_of(interval) {
        return;
    }

    // --- Update season counter ---
    let tps = sim_config.ticks_per_season;
    if let Some(current_season) = time.tick.checked_div(tps) {
        if current_season > score.last_recorded_season {
            score.seasons_survived += current_season - score.last_recorded_season;
            score.last_recorded_season = current_season;
        }
    }

    // --- Gather cat data ---
    let needs: Vec<&Needs> = cat_query.iter().map(|(_, n, _, _, _)| n).collect();
    let healths: Vec<f32> = cat_query.iter().map(|(_, _, h, _, _)| h.current).collect();
    let effective_moods: Vec<f32> = cat_query
        .iter()
        .map(|(_, _, _, m, _)| {
            let mod_sum: f32 = m.modifiers.iter().map(|md| md.amount).sum();
            (m.valence + mod_sum).clamp(-1.0, 1.0)
        })
        .collect();
    let shelter_facets: Vec<&ShelterFacet> =
        cat_query.iter().map(|(_, _, _, _, s)| &s.facet).collect();

    let living_cats = needs.len() as u64;

    // Update peak population.
    if living_cats > score.peak_population {
        score.peak_population = living_cats;
    }

    // --- Compute welfare axes ---
    let cs = &constants.colony_score;
    let shelter = compute_shelter(&shelter_facets, &constants.shelter_beliefs);
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

    // Cache the per-tick computation on the resource so the post-loop
    // footer writer (and ticket-125 verdict tooling) can read welfare
    // axes + aggregate without re-tailing the events.jsonl.
    let snapshot = ColonyScoreSnapshot {
        shelter,
        nourishment,
        health,
        happiness,
        fulfillment,
        welfare,
        aggregate,
    };
    score.last_snapshot = Some(snapshot.clone());

    // TPS-invariant checkpoint: freeze the snapshot AND the achievement
    // ledger once, at the first emission at or after
    // `checkpoint_elapsed_ticks` *elapsed* (not absolute) ticks. The
    // ledger must freeze too — it is the other elapsed-time-dependent
    // term in the aggregate, so an end-of-run read rewards faster
    // binaries, not healthier colonies.
    let checkpoint_mark = cs.checkpoint_elapsed_ticks;
    if score.checkpoint.is_none()
        && checkpoint_mark > 0
        && time.tick.saturating_sub(score.run_start_tick) >= checkpoint_mark
    {
        score.checkpoint = Some(crate::resources::colony_score::ColonyScoreCheckpoint {
            captured_at_elapsed_tick: time.tick.saturating_sub(score.run_start_tick),
            snapshot,
            seasons_survived: score.seasons_survived,
            peak_population: score.peak_population,
            kittens_born: score.kittens_born,
            kittens_matured: score.kittens_matured,
            structures_built: score.structures_built,
            bonds_formed: score.bonds_formed,
            deaths_starvation: score.deaths_starvation,
            deaths_old_age: score.deaths_old_age,
            deaths_injury: score.deaths_injury,
        });
    }

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
    // BTreeMap so the SystemActivation event's JSON key order is stable
    // across processes (replay determinism — see EventKind::SystemActivation).
    let mut positive = std::collections::BTreeMap::new();
    let mut negative = std::collections::BTreeMap::new();
    let mut neutral = std::collections::BTreeMap::new();
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

    fn test_cfg() -> ShelterBeliefConstants {
        ShelterBeliefConstants::default()
    }

    fn fully_housed() -> ShelterFacet {
        ShelterFacet {
            belonging: 1.0,
            quality: 1.0,
            continuity: 1.0,
            threat: 0.0,
            last_updated_tick: 0,
        }
    }

    #[test]
    fn shelter_fully_housed_cats_max_score() {
        let f1 = fully_housed();
        let f2 = fully_housed();
        let facets = vec![&f1, &f2];
        let score = compute_shelter(&facets, &test_cfg());
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn shelter_no_home_den_contributes_zero() {
        // Default ShelterFacet has belonging=0 (no claim) → contribution 0.
        let f = ShelterFacet::default();
        let facets = vec![&f];
        assert_eq!(compute_shelter(&facets, &test_cfg()), 0.0);
    }

    #[test]
    fn shelter_averages_population_security() {
        let secure = fully_housed();
        let insecure = ShelterFacet::default();
        let facets = vec![&secure, &insecure];
        let score = compute_shelter(&facets, &test_cfg());
        assert!(
            (score - 0.5).abs() < 1e-6,
            "expected mean(1.0, 0.0) = 0.5; got {score}"
        );
    }

    #[test]
    fn shelter_threat_pulls_security_down() {
        let mut f = fully_housed();
        f.threat = 1.0;
        let facets = vec![&f];
        // belonging*quality*(1-threat) = 1*1*0 = 0; continuity factor doesn't rescue.
        assert!(compute_shelter(&facets, &test_cfg()) < 1e-6);
    }

    #[test]
    fn shelter_damaged_den_pulls_quality_down() {
        let mut f = fully_housed();
        f.quality = 0.4; // belief about damaged den
        let facets = vec![&f];
        let score = compute_shelter(&facets, &test_cfg());
        // 1.0 * 0.4 * 1.0 * (1.0*1.0 + 0.0) = 0.4
        assert!((score - 0.4).abs() < 1e-6, "expected 0.4; got {score}");
    }

    #[test]
    fn shelter_continuity_weight_zero_ignores_continuity() {
        let mut cfg = test_cfg();
        cfg.continuity_weight = 0.0;
        let mut f = fully_housed();
        f.continuity = 0.0; // no felt time at home
        let facets = vec![&f];
        let score = compute_shelter(&facets, &cfg);
        // base 1.0 * factor (0*0 + 1) = 1.0
        assert!((score - 1.0).abs() < 1e-6, "expected 1.0; got {score}");
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
        assert_eq!(compute_shelter(&[], &test_cfg()), 0.0);
        assert_eq!(compute_nourishment(&[]), 0.0);
        assert_eq!(compute_health(&[]), 0.0);
        assert_eq!(compute_happiness(&[]), 0.0);
    }
}
