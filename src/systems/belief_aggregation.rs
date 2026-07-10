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

/// Width in tiles of one `LocationKey` bucket — the granularity at which
/// `LocationBeliefs` quantizes positions. Constant across all readers so
/// the bucket-to-tile conversion is uniform.
pub const LOCATION_BUCKET_SIZE: i32 = 5;

/// 293: scan the cat's own [`LocationBeliefs`] within `radius_buckets` of
/// `pos` and return a unit direction step toward the bucket with the
/// highest `prey_yield.value` that exceeds `min_value`. Returns `None`
/// when no qualifying bucket is reachable — caller falls back to patrol
/// drift.
///
/// Replaces the legacy `HuntingPriors::best_direction` reader. The
/// `min_value` threshold sits at the substrate's neutral midpoint (0.5)
/// so this matches the legacy "buckets above `DEFAULT_PRIOR`" semantic;
/// uninformed buckets (`strength == 0`, `value == 0`) automatically fall
/// below the threshold.
pub fn best_prey_direction(
    beliefs: &LocationBeliefs,
    pos: Position,
    radius_buckets: f32,
    min_value: f32,
) -> Option<(i32, i32)> {
    let origin_bx = pos.x() / LOCATION_BUCKET_SIZE;
    let origin_by = pos.y() / LOCATION_BUCKET_SIZE;
    let r = radius_buckets.max(0.0).round() as i32;

    let mut best_value = min_value;
    let mut best_bucket: Option<(i32, i32)> = None;
    for dy in -r..=r {
        for dx in -r..=r {
            let bx = origin_bx + dx;
            let by = origin_by + dy;
            if let Some(model) = beliefs.models.get(&(bx, by)) {
                if model.prey_yield.value > best_value {
                    best_value = model.prey_yield.value;
                    best_bucket = Some((bx, by));
                }
            }
        }
    }
    best_bucket.map(|(bx, by)| {
        let center_x = bx * LOCATION_BUCKET_SIZE + LOCATION_BUCKET_SIZE / 2;
        let center_y = by * LOCATION_BUCKET_SIZE + LOCATION_BUCKET_SIZE / 2;
        ((center_x - pos.x()).signum(), (center_y - pos.y()).signum())
    })
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
        FacetSlot::SurplusFood => &model.surplus_food,
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
    fn best_prey_direction_picks_highest_value_above_threshold() {
        let mut lb = LocationBeliefs::default();
        let pos = p(50, 50); // origin bucket (10, 10)
        let mut nearby_low = MentalModel::default();
        nearby_low.prey_yield.value = 0.55; // barely above threshold
        nearby_low.prey_yield.strength = 0.5;
        lb.models.insert((11, 10), nearby_low);

        let mut nearby_high = MentalModel::default();
        nearby_high.prey_yield.value = 0.9;
        nearby_high.prey_yield.strength = 0.9;
        lb.models.insert((10, 11), nearby_high);

        let dir = best_prey_direction(&lb, pos, 3.0, 0.5).expect("a qualifying bucket exists");
        // Best bucket is (10, 11), center at (52, 57); origin (50, 50);
        // dx = 2.signum() = 1 ... wait — center_x = 10*5 + 2 = 52, dx =
        // 52-50 = 2 → signum 1; center_y = 11*5 + 2 = 57, dy = 7 → 1.
        assert_eq!(dir, (1, 1));
    }

    #[test]
    fn best_prey_direction_returns_none_when_no_bucket_qualifies() {
        let mut lb = LocationBeliefs::default();
        let pos = p(50, 50);
        // Bucket inside radius but below threshold.
        let mut weak = MentalModel::default();
        weak.prey_yield.value = 0.4;
        weak.prey_yield.strength = 0.9;
        lb.models.insert((10, 10), weak);

        assert!(best_prey_direction(&lb, pos, 3.0, 0.5).is_none());
    }

    #[test]
    fn best_prey_direction_skips_buckets_outside_radius() {
        let mut lb = LocationBeliefs::default();
        let pos = p(50, 50); // origin bucket (10, 10)
        let mut far = MentalModel::default();
        far.prey_yield.value = 0.99;
        far.prey_yield.strength = 0.9;
        // Bucket 30 buckets away — far outside radius 3.
        lb.models.insert((40, 10), far);

        assert!(best_prey_direction(&lb, pos, 3.0, 0.5).is_none());
    }

    #[test]
    fn best_prey_direction_empty_beliefs_returns_none() {
        let lb = LocationBeliefs::default();
        assert!(best_prey_direction(&lb, p(50, 50), 5.0, 0.5).is_none());
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
