//! Colony-level views derived from per-cat [`LocationBeliefs`].
//!
//! The C3 belief substrate (ticket 258) is per-cat: each witness within
//! `WITNESS_RANGE` of an event updates their own
//! `LocationBeliefs[bucket(pos)]` model. Some consumers — notably ward
//! placement — need a colony-level view that asks "how strongly do *any*
//! cats remember being ambushed near this candidate spot?". This module
//! provides the minimum aggregation primitive needed by 294's
//! `RecentAmbushMap` retirement.
//!
//! Aggregation here is deliberately simple — max-over-cats of facet
//! `value`, gated by a per-cat strength floor — to match the shape of the
//! legacy [`RecentAmbushMap`](crate::resources::RecentAmbushMap) it
//! replaces (deposit saturates at 1.0, so the colony field was already a
//! max-of-witnesses). Ticket 291's full ColonyKnowledge restructure will
//! replace this with mental-model-agreement promotion that admits
//! divergence and false-belief epidemics.

use std::collections::HashMap;

use bevy_ecs::prelude::Query;

use crate::components::beliefs::{
    bucket_position, Facet, FacetSlot, LocationBeliefs, LocationKey, MentalModel,
};
use crate::components::physical::Position;
use crate::resources::sim_constants::BeliefAggregationConstants;

/// Max-over-cats of the chosen facet's `value` at `bucket_position(pos)`,
/// considering only cats whose facet `strength` meets
/// `cfg.min_strength_to_contribute`. Returns `0.0` if no cat has a
/// qualifying belief at the bucket.
///
/// Designed for read-only sampling from ECS systems. Pass
/// `cats_query.iter()` from a `Query<&LocationBeliefs>` SystemParam.
/// Result is clamped to `[0.0, 1.0]` — the facet's documented range for
/// every slot except `affiliation_history`, which is out of scope for
/// this v1 helper (the legacy `RecentAmbushMap` reader only consumed
/// `recency_of_threat_cue`).
pub fn aggregated_location_belief<'a, I>(
    cats: I,
    facet: FacetSlot,
    pos: Position,
    cfg: &BeliefAggregationConstants,
) -> f32
where
    I: IntoIterator<Item = &'a LocationBeliefs>,
{
    let key = bucket_position(pos.x(), pos.y());
    let mut best = 0.0_f32;
    for beliefs in cats {
        if let Some(model) = beliefs.models.get(&key) {
            let f = select_facet(model, facet);
            if f.strength >= cfg.min_strength_to_contribute {
                best = best.max(f.value);
            }
        }
    }
    best.clamp(0.0, 1.0)
}

/// Convenience wrapper that accepts a Bevy `Query<&LocationBeliefs>`
/// directly — the production call shape from ward placement and any
/// other system-side reader. Identical aggregation semantics; exists so
/// callers don't have to write `.iter()` at every call site and so the
/// borrow shape is unambiguous to the compiler.
pub fn aggregated_location_belief_q(
    cats: &Query<&LocationBeliefs>,
    facet: FacetSlot,
    pos: Position,
    cfg: &BeliefAggregationConstants,
) -> f32 {
    aggregated_location_belief(cats.iter(), facet, pos, cfg)
}

/// Build a colony-wide aggregated snapshot of the chosen facet across
/// every populated bucket. Output maps `LocationKey` → max-over-cats of
/// `Facet::value`, gated by `cfg.min_strength_to_contribute`. Buckets
/// that no cat has any qualifying belief about are absent from the map
/// (callers should treat `None` as `0.0`).
///
/// Used by ward placement: build once per call to `compute_ward_placement`
/// and look up per candidate via `map.get(&bucket_position(x, y)).copied()`.
/// Snapshotting is O(cats × buckets-per-cat); per-candidate lookup is O(1).
pub fn aggregate_location_belief_snapshot<'a, I>(
    cats: I,
    facet: FacetSlot,
    cfg: &BeliefAggregationConstants,
) -> HashMap<LocationKey, f32>
where
    I: IntoIterator<Item = &'a LocationBeliefs>,
{
    let mut out: HashMap<LocationKey, f32> = HashMap::new();
    for beliefs in cats {
        for (key, model) in &beliefs.models {
            let f = select_facet(model, facet);
            if f.strength < cfg.min_strength_to_contribute {
                continue;
            }
            let entry = out.entry(*key).or_insert(0.0);
            *entry = entry.max(f.value);
        }
    }
    out
}

fn select_facet(model: &MentalModel, slot: FacetSlot) -> &Facet {
    match slot {
        FacetSlot::PerceivedInjuryLevel => &model.perceived_injury_level,
        FacetSlot::PerceivedIntentClarity => &model.perceived_intent_clarity,
        FacetSlot::RecencyOfThreatCue => &model.recency_of_threat_cue,
        FacetSlot::PerceivedViolenceCapability => &model.perceived_violence_capability,
        FacetSlot::AffiliationHistory => &model.affiliation_history,
        FacetSlot::Predictability => &model.predictability,
        FacetSlot::PerceivedHostility => &model.perceived_hostility,
        FacetSlot::PerceivedReceptivity => &model.perceived_receptivity,
        FacetSlot::PreyYield => &model.prey_yield,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::beliefs::{bucket_position, MentalModel};
    use bevy::math::Vec2;

    fn cat_with_belief(pos: (i32, i32), value: f32, strength: f32) -> LocationBeliefs {
        let mut lb = LocationBeliefs::default();
        let key = bucket_position(pos.0, pos.1);
        let mut model = MentalModel::default();
        model.recency_of_threat_cue.value = value;
        model.recency_of_threat_cue.strength = strength;
        lb.models.insert(key, model);
        lb
    }

    fn p(x: i32, y: i32) -> Position {
        Position(Vec2::new(x as f32, y as f32))
    }

    #[test]
    fn max_across_three_cats_at_same_bucket() {
        let cats = [
            cat_with_belief((10, 10), 0.3, 1.0),
            cat_with_belief((10, 10), 0.7, 1.0),
            cat_with_belief((10, 10), 0.2, 1.0),
        ];
        let cfg = BeliefAggregationConstants::default();
        let v = aggregated_location_belief(&cats, FacetSlot::RecencyOfThreatCue, p(11, 11), &cfg);
        assert!((v - 0.7).abs() < f32::EPSILON, "got {v}");
    }

    #[test]
    fn zero_when_no_cat_has_belief_at_bucket() {
        let cats = [cat_with_belief((50, 50), 0.9, 1.0)];
        let cfg = BeliefAggregationConstants::default();
        let v = aggregated_location_belief(&cats, FacetSlot::RecencyOfThreatCue, p(0, 0), &cfg);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn strength_floor_filters_low_confidence_cats() {
        let cats = [
            cat_with_belief((10, 10), 0.9, 0.05), // below floor
            cat_with_belief((10, 10), 0.4, 0.5),  // above floor
        ];
        let cfg = BeliefAggregationConstants {
            min_strength_to_contribute: 0.1,
        };
        let v = aggregated_location_belief(&cats, FacetSlot::RecencyOfThreatCue, p(10, 10), &cfg);
        // Only the second cat qualifies; result is its value.
        assert!((v - 0.4).abs() < f32::EPSILON, "got {v}");
    }

    #[test]
    fn empty_iterator_yields_zero() {
        let cats: [LocationBeliefs; 0] = [];
        let cfg = BeliefAggregationConstants::default();
        let v = aggregated_location_belief(&cats, FacetSlot::RecencyOfThreatCue, p(7, 7), &cfg);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn snapshot_collects_max_per_bucket_across_cats() {
        let cats = [
            cat_with_belief((10, 10), 0.3, 1.0),
            cat_with_belief((10, 10), 0.7, 1.0),
            cat_with_belief((50, 50), 0.5, 1.0),
        ];
        let cfg = BeliefAggregationConstants::default();
        let snap = aggregate_location_belief_snapshot(&cats, FacetSlot::RecencyOfThreatCue, &cfg);
        // Two buckets present: (2, 2) for (10, 10) and (10, 10) for (50, 50).
        assert_eq!(snap.len(), 2);
        assert!((snap[&bucket_position(10, 10)] - 0.7).abs() < f32::EPSILON);
        assert!((snap[&bucket_position(50, 50)] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn snapshot_omits_buckets_below_strength_floor() {
        let cats = [
            cat_with_belief((10, 10), 0.9, 0.05), // below floor
            cat_with_belief((20, 20), 0.4, 0.8),  // above floor
        ];
        let cfg = BeliefAggregationConstants {
            min_strength_to_contribute: 0.1,
        };
        let snap = aggregate_location_belief_snapshot(&cats, FacetSlot::RecencyOfThreatCue, &cfg);
        // Only the second cat's bucket appears.
        assert_eq!(snap.len(), 1);
        assert!((snap[&bucket_position(20, 20)] - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn prey_yield_facet_surfaces_through_aggregator() {
        let mut lb = LocationBeliefs::default();
        let key = bucket_position(40, 40);
        let mut model = MentalModel::default();
        model.prey_yield.value = 0.8;
        model.prey_yield.strength = 0.6;
        lb.models.insert(key, model);
        let cfg = BeliefAggregationConstants::default();
        let v = aggregated_location_belief([&lb], FacetSlot::PreyYield, p(40, 40), &cfg);
        assert!(
            (v - 0.8).abs() < f32::EPSILON,
            "PreyYield slot dispatches to model.prey_yield; got {v}"
        );
        let snap = aggregate_location_belief_snapshot([&lb], FacetSlot::PreyYield, &cfg);
        assert_eq!(snap.len(), 1);
        assert!((snap[&key] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn bucket_alignment_matches_5_tile_grid() {
        // (10, 10) and (14, 14) bucket together at (2, 2). The cat
        // remembers an ambush at bucket (2, 2); query at (14, 14) finds
        // it; query at (15, 15) — bucket (3, 3) — does not.
        let cats = [cat_with_belief((10, 10), 0.8, 1.0)];
        let cfg = BeliefAggregationConstants::default();
        assert!(
            (aggregated_location_belief(&cats, FacetSlot::RecencyOfThreatCue, p(14, 14), &cfg)
                - 0.8)
                .abs()
                < f32::EPSILON
        );
        assert_eq!(
            aggregated_location_belief(&cats, FacetSlot::RecencyOfThreatCue, p(15, 15), &cfg),
            0.0
        );
    }
}
