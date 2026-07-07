//! Colony knowledge — derivation via mental-model agreement (291).
//!
//! Pre-291 this system scanned per-cat `Memory` buffers every
//! `scan_interval` ticks and promoted `(MemoryType, bucket)` groups
//! held by ≥ `promotion_threshold` carriers — democratic consensus by
//! carrier count. That shape structurally precluded the load-bearing
//! C3 narrative: *the colony wrongly believes X because panic
//! propagated faster than ground truth corrected*. With 258's per-cat
//! `MentalModel`s live, colony knowledge is now DERIVED: a belief
//! promotes when ≥ `agreement_quorum` cats hold a location facet
//! whose values mutually agree (within `agreement_epsilon` of the
//! group median) at evidence strength ≥ `promotion_strength`.
//!
//! **Facet → knowledge mapping.** Only the location-keyed facets with
//! live writers derive entries:
//!
//! - `LocationBeliefs[bucket].recency_of_threat_cue` → `ThreatSeen`
//! - `LocationBeliefs[bucket].prey_yield` → `ResourceFound`
//!
//! These two are exactly what the scoring readers consume
//! (`ColonyKnowledgeLift`'s threat arm reads ThreatSeen ∨ Death;
//! resource arm reads ResourceFound). The other six `MemoryType`s
//! (Death / MagicEvent / Injury / SocialEvent / Triumph / Sleep) were
//! narrative-only and retire with the Memory scan — the
//! mythic-texture continuity canary is the pre-registered gate on
//! that loss (ticket 291 verification).
//!
//! **Truth is not a gate.** The quorum agrees on *beliefs*, not
//! facts: three cats who all witnessed the same misleading cue will
//! promote a false entry (`colony_knowledge_false_belief` scenario
//! pins this as capability, not bug).

use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::beliefs::{LocationBeliefs, LocationKey};
use crate::components::mental::MemoryType;
use crate::components::physical::{Dead, Position};
use crate::resources::colony_knowledge::{knowledge_description, ColonyKnowledge, KnowledgeEntry};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::sim_constants::{KnowledgeConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeState;

/// One cat's belief contribution to the derivation. Collected from the
/// live query by `update_colony_knowledge`; a plain slice input keeps
/// [`derive_colony_knowledge`] pure and unit-testable without an ECS
/// world.
pub struct BeliefRow<'a> {
    pub cat: Entity,
    pub locs: &'a LocationBeliefs,
}

/// Convert a belief-bucket key back to the bucket-center `Position`
/// the legacy entries used (`ColonyKnowledge::bucket_position`
/// produces `(x/5)*5 + 2` — the same center this reconstructs), so
/// the scoring readers' proximity math is unchanged.
fn bucket_key_to_position(key: LocationKey) -> Position {
    Position::new(key.0 * 5 + 2, key.1 * 5 + 2)
}

/// Derive the colony-knowledge set from per-cat location beliefs.
///
/// Per `(bucket, facet)`: collect every cat whose facet strength ≥
/// `promotion_strength`; anchor on the group median value; count the
/// cats within `agreement_epsilon` of that median; promote when the
/// agreeing set reaches `agreement_quorum` AND its mean value clears
/// `min_promotion_value`. The entry carries the agreeing cats as
/// `witnesses` and their mean value as `strength`.
///
/// Returns entries in deterministic order (bucket key, then facet) —
/// HashMap iteration is randomized, so the derivation sorts before
/// returning to keep seed-42 runs reproducible.
pub fn derive_colony_knowledge(
    rows: &[BeliefRow<'_>],
    c: &KnowledgeConstants,
) -> Vec<KnowledgeEntry> {
    // (bucket, MemoryType) → Vec<(cat, value)>
    let mut groups: HashMap<(LocationKey, MemoryType), Vec<(Entity, f32)>> = HashMap::new();
    for row in rows {
        for (key, model) in &row.locs.models {
            for (facet, kind) in [
                (&model.recency_of_threat_cue, MemoryType::ThreatSeen),
                (&model.prey_yield, MemoryType::ResourceFound),
            ] {
                if facet.strength >= c.promotion_strength {
                    groups
                        .entry((*key, kind))
                        .or_default()
                        .push((row.cat, facet.value));
                }
            }
        }
    }

    let quorum = c.agreement_quorum.max(1) as usize;
    let mut entries: Vec<KnowledgeEntry> = Vec::new();
    for ((key, kind), mut members) in groups {
        if members.len() < quorum {
            continue;
        }
        // Median-anchored agreement: sort by value, take the median,
        // keep members within epsilon of it.
        members.sort_by(|a, b| a.1.total_cmp(&b.1));
        let median = members[members.len() / 2].1;
        let agreeing: Vec<(Entity, f32)> = members
            .iter()
            .copied()
            .filter(|(_, v)| (v - median).abs() <= c.agreement_epsilon)
            .collect();
        if agreeing.len() < quorum {
            continue;
        }
        let mean_value = agreeing.iter().map(|(_, v)| v).sum::<f32>() / agreeing.len() as f32;
        if mean_value <= c.min_promotion_value {
            continue;
        }
        entries.push(KnowledgeEntry {
            event_type: kind,
            location: Some(bucket_key_to_position(key)),
            strength: mean_value.clamp(0.0, 1.0),
            carrier_count: agreeing.len() as u32,
            witnesses: agreeing.iter().map(|(e, _)| *e).collect(),
        });
    }

    // Deterministic output order (HashMap iteration is randomized).
    entries.sort_by_key(|e| (e.location.map(|p| (p.x(), p.y())), e.event_type as u32));
    entries
}

// ---------------------------------------------------------------------------
// update_colony_knowledge system
// ---------------------------------------------------------------------------

/// Maintains the colony's collective knowledge (291 — derivation
/// cutover).
///
/// Every `scan_interval` ticks: derive the knowledge set from live
/// per-cat `LocationBeliefs`, diff against the current set, narrate
/// forgetting (with the description cooldown) for dissolved entries,
/// record `Feature::KnowledgePromoted` for new ones, and replace the
/// resource contents. Between scans the set is static — belief decay
/// happens on the cat side (`belief_integrator` Pass B), so a
/// separate per-tick entry decay would double-count it.
///
/// Also accumulates `ColonyKnowledge::divergence_duration_ticks` —
/// the 258 exit-criteria footer signal: per scan, each `(bucket,
/// facet)` group that met the strength quorum but FAILED value
/// agreement contributes `scan_interval` ticks of measured
/// belief-divergence.
pub fn update_colony_knowledge(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    cats: Query<(Entity, &LocationBeliefs), Without<Dead>>,
    mut knowledge: ResMut<ColonyKnowledge>,
    mut log: ResMut<NarrativeLog>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.knowledge;
    if !time.tick.is_multiple_of(c.scan_interval) {
        return;
    }

    let rows: Vec<BeliefRow> = cats
        .iter()
        .map(|(cat, locs)| BeliefRow { cat, locs })
        .collect();
    let derived = derive_colony_knowledge(&rows, c);
    knowledge.divergence_duration_ticks += divergent_group_count(&rows, c) * c.scan_interval;

    // Forgotten: in current but not in derived.
    let mut seen_descriptions = std::collections::HashSet::new();
    let forgotten: Vec<KnowledgeEntry> = knowledge
        .entries
        .iter()
        .filter(|old| {
            !derived
                .iter()
                .any(|new| new.event_type == old.event_type && new.location == old.location)
        })
        .cloned()
        .collect();
    for entry in &forgotten {
        let desc = knowledge_description(entry);
        if !seen_descriptions.insert(desc.clone()) {
            continue;
        }
        let on_cooldown = knowledge
            .recently_forgotten
            .get(&desc)
            .is_some_and(|&last_tick| time.tick.saturating_sub(last_tick) < c.forgotten_cooldown);
        if !on_cooldown {
            activation.record(Feature::KnowledgeForgotten);
            knowledge.recently_forgotten.insert(desc.clone(), time.tick);
            log.push(
                time.tick,
                format!("The colony has forgotten {desc}."),
                NarrativeTier::Significant,
            );
        }
    }

    // Promoted: in derived but not in current.
    for entry in &derived {
        let is_new = !knowledge
            .entries
            .iter()
            .any(|old| old.event_type == entry.event_type && old.location == entry.location);
        if is_new {
            activation.record(Feature::KnowledgePromoted);
        }
    }

    // Prune stale cooldown entries.
    knowledge
        .recently_forgotten
        .retain(|_, tick| time.tick.saturating_sub(*tick) < c.forgotten_cooldown);

    knowledge.entries = derived;
}

/// Count `(bucket, facet)` groups where enough cats hold a
/// strong-enough belief to form a quorum but value agreement fails —
/// the colony is actively divided about that place. Feeds the
/// `belief_divergence_duration_ticks` footer field.
fn divergent_group_count(rows: &[BeliefRow<'_>], c: &KnowledgeConstants) -> u64 {
    let mut groups: HashMap<(LocationKey, MemoryType), Vec<f32>> = HashMap::new();
    for row in rows {
        for (key, model) in &row.locs.models {
            for (facet, kind) in [
                (&model.recency_of_threat_cue, MemoryType::ThreatSeen),
                (&model.prey_yield, MemoryType::ResourceFound),
            ] {
                if facet.strength >= c.promotion_strength {
                    groups.entry((*key, kind)).or_default().push(facet.value);
                }
            }
        }
    }
    let quorum = c.agreement_quorum.max(1) as usize;
    groups
        .values_mut()
        .filter(|values: &&mut Vec<f32>| values.len() >= quorum)
        .filter(|values| {
            let mut values = values.to_vec();
            values.sort_by(|a, b| a.total_cmp(b));
            let median = values[values.len() / 2];
            let agreeing = values
                .iter()
                .filter(|v| (**v - median).abs() <= c.agreement_epsilon)
                .count();
            agreeing < quorum
        })
        .count() as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::beliefs::{bucket_position, Facet, MentalModel};

    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

    /// Build a `LocationBeliefs` holding one bucket's threat facet.
    fn threat_belief(x: i32, y: i32, value: f32, strength: f32) -> LocationBeliefs {
        let mut locs = LocationBeliefs::default();
        let model = locs.models.entry(bucket_position(x, y)).or_default();
        model.recency_of_threat_cue = Facet {
            value,
            strength,
            ..Default::default()
        };
        locs
    }

    fn prey_belief(x: i32, y: i32, value: f32, strength: f32) -> LocationBeliefs {
        let mut locs = LocationBeliefs::default();
        let model = locs.models.entry(bucket_position(x, y)).or_default();
        model.prey_yield = Facet {
            value,
            strength,
            ..Default::default()
        };
        let _ = MentalModel::default();
        locs
    }

    fn rows<'a>(beliefs: &'a [(Entity, LocationBeliefs)]) -> Vec<BeliefRow<'a>> {
        beliefs
            .iter()
            .map(|(cat, locs)| BeliefRow { cat: *cat, locs })
            .collect()
    }

    #[test]
    fn promotes_on_three_cat_agreement() {
        let c = KnowledgeConstants::default();
        let beliefs: Vec<(Entity, LocationBeliefs)> = (1..=3)
            .map(|i| (entity(i), threat_belief(10, 10, 0.8, 0.9)))
            .collect();
        let derived = derive_colony_knowledge(&rows(&beliefs), &c);
        assert_eq!(derived.len(), 1);
        assert_eq!(derived[0].event_type, MemoryType::ThreatSeen);
        assert_eq!(derived[0].carrier_count, 3);
        assert_eq!(derived[0].witnesses.len(), 3);
        // Bucket-center location matches the legacy bucketing.
        assert_eq!(
            derived[0].location,
            Some(ColonyKnowledge::bucket_position(&Position::new(10, 10)))
        );
        assert!((derived[0].strength - 0.8).abs() < 1e-5);
    }

    #[test]
    fn does_not_promote_below_quorum() {
        let c = KnowledgeConstants::default();
        let beliefs: Vec<(Entity, LocationBeliefs)> = (1..=2)
            .map(|i| (entity(i), threat_belief(10, 10, 0.8, 0.9)))
            .collect();
        assert!(derive_colony_knowledge(&rows(&beliefs), &c).is_empty());
    }

    #[test]
    fn divergent_values_block_promotion_and_count_as_divergence() {
        // Three strong beliefs, values 0.1 / 0.5 / 0.9 with epsilon
        // 0.2: only the median agrees with itself ± one neighbor at
        // most — no quorum.
        let c = KnowledgeConstants::default();
        let beliefs = vec![
            (entity(1), threat_belief(10, 10, 0.1, 0.9)),
            (entity(2), threat_belief(10, 10, 0.5, 0.9)),
            (entity(3), threat_belief(10, 10, 0.9, 0.9)),
        ];
        let r = rows(&beliefs);
        assert!(derive_colony_knowledge(&r, &c).is_empty());
        assert_eq!(divergent_group_count(&r, &c), 1);
    }

    #[test]
    fn weak_strength_does_not_count_toward_quorum() {
        let c = KnowledgeConstants::default();
        let beliefs = vec![
            (entity(1), threat_belief(10, 10, 0.8, 0.9)),
            (entity(2), threat_belief(10, 10, 0.8, 0.9)),
            // Barely-formed belief: below promotion_strength (0.3).
            (entity(3), threat_belief(10, 10, 0.8, 0.1)),
        ];
        assert!(derive_colony_knowledge(&rows(&beliefs), &c).is_empty());
    }

    #[test]
    fn near_zero_consensus_is_not_knowledge() {
        // Everyone strongly agrees nothing is here — no entry.
        let c = KnowledgeConstants::default();
        let beliefs: Vec<(Entity, LocationBeliefs)> = (1..=3)
            .map(|i| (entity(i), threat_belief(10, 10, 0.01, 0.9)))
            .collect();
        assert!(derive_colony_knowledge(&rows(&beliefs), &c).is_empty());
    }

    #[test]
    fn prey_yield_derives_resource_found() {
        let c = KnowledgeConstants::default();
        let beliefs: Vec<(Entity, LocationBeliefs)> = (1..=3)
            .map(|i| (entity(i), prey_belief(25, 30, 0.7, 0.8)))
            .collect();
        let derived = derive_colony_knowledge(&rows(&beliefs), &c);
        assert_eq!(derived.len(), 1);
        assert_eq!(derived[0].event_type, MemoryType::ResourceFound);
    }

    #[test]
    fn false_belief_promotes_when_quorum_agrees() {
        // Substrate does not gate truth: three cats who witnessed the
        // same misleading cue promote a "danger" entry about a
        // perfectly safe meadow. This is the 291 capability the
        // carrier-count model precluded.
        let c = KnowledgeConstants::default();
        let beliefs: Vec<(Entity, LocationBeliefs)> = (1..=3)
            .map(|i| (entity(i), threat_belief(40, 40, 0.9, 0.8)))
            .collect();
        let derived = derive_colony_knowledge(&rows(&beliefs), &c);
        assert_eq!(derived.len(), 1);
        let witnesses = &derived[0].witnesses;
        assert_eq!(witnesses.len(), 3, "witness chain must be citable");
        for i in 1..=3 {
            assert!(witnesses.contains(&entity(i)));
        }
    }

    // -----------------------------------------------------------------
    // System-level: diff narration + feature hooks
    // -----------------------------------------------------------------

    fn setup_world() -> (World, bevy_ecs::schedule::Schedule) {
        let mut world = World::new();
        world.insert_resource(ColonyKnowledge::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SystemActivation::default());
        let mut time = TimeState::default();
        time.tick = SimConstants::default().knowledge.scan_interval;
        world.insert_resource(time);
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(update_colony_knowledge);
        (world, schedule)
    }

    #[test]
    fn system_promotes_and_records_feature() {
        let (mut world, mut schedule) = setup_world();
        for _ in 0..3 {
            world.spawn(threat_belief(10, 10, 0.8, 0.9));
        }
        schedule.run(&mut world);
        let knowledge = world.resource::<ColonyKnowledge>();
        assert_eq!(knowledge.entries.len(), 1);
        assert_eq!(knowledge.entries[0].carrier_count, 3);
    }

    #[test]
    fn system_narrates_forgetting_when_agreement_dissolves() {
        let (mut world, mut schedule) = setup_world();
        {
            let mut knowledge = world.resource_mut::<ColonyKnowledge>();
            knowledge.entries.push(KnowledgeEntry {
                event_type: MemoryType::ThreatSeen,
                location: Some(ColonyKnowledge::bucket_position(&Position::new(10, 10))),
                strength: 0.8,
                carrier_count: 3,
                witnesses: vec![],
            });
        }
        // No cats hold the belief anymore → the entry dissolves.
        schedule.run(&mut world);
        let knowledge = world.resource::<ColonyKnowledge>();
        assert!(knowledge.entries.is_empty());
        let log = world.resource::<NarrativeLog>();
        assert!(
            log.entries.iter().any(|e| e.text.contains("forgotten")),
            "should narrate knowledge loss"
        );
    }

    #[test]
    fn divergence_accumulates_footer_ticks() {
        let (mut world, mut schedule) = setup_world();
        world.spawn(threat_belief(10, 10, 0.1, 0.9));
        world.spawn(threat_belief(10, 10, 0.5, 0.9));
        world.spawn(threat_belief(10, 10, 0.9, 0.9));
        schedule.run(&mut world);
        let knowledge = world.resource::<ColonyKnowledge>();
        assert_eq!(
            knowledge.divergence_duration_ticks,
            SimConstants::default().knowledge.scan_interval
        );
    }
}
