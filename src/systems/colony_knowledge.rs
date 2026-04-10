use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::mental::{Memory, MemoryType};
use crate::components::physical::{Dead, Position};
use crate::resources::colony_knowledge::{
    ColonyKnowledge, KnowledgeEntry, knowledge_description,
};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::time::TimeState;

/// Decay rate per tick for colony knowledge entries.
const DECAY_PER_TICK: f32 = 0.0001;

/// Minimum number of carriers required to promote a memory to colony knowledge.
const PROMOTION_THRESHOLD: u32 = 3;

/// How often (in ticks) to scan for promotable memories.
const SCAN_INTERVAL: u64 = 500;

// ---------------------------------------------------------------------------
// update_colony_knowledge system
// ---------------------------------------------------------------------------

/// Maintains the colony's collective knowledge.
///
/// Every tick: decay entry strength, remove expired or carrierless entries.
/// Every 50 ticks: scan living cats' memories, count carriers per
/// (event_type, bucketed_location), promote if threshold met.
pub fn update_colony_knowledge(
    time: Res<TimeState>,
    cats: Query<&Memory, Without<Dead>>,
    mut knowledge: ResMut<ColonyKnowledge>,
    mut log: ResMut<NarrativeLog>,
) {
    // --- Decay and cleanup (every tick) ------------------------------------
    let mut removed = Vec::new();
    knowledge.entries.retain(|entry| {
        if entry.strength <= 0.0 || entry.carrier_count == 0 {
            removed.push(entry.clone());
            false
        } else {
            true
        }
    });

    // Deduplicate by description before emitting narrative so that multiple
    // entries with the same knowledge_description produce only one line.
    // Also enforce a cooldown: don't re-narrate forgetting the same knowledge
    // within 1000 ticks to prevent spam from promote/decay cycles.
    const FORGOTTEN_COOLDOWN: u64 = 1000;
    let mut seen_descriptions = std::collections::HashSet::new();
    for entry in &removed {
        let desc = knowledge_description(entry);
        if !seen_descriptions.insert(desc.clone()) {
            continue;
        }
        let on_cooldown = knowledge
            .recently_forgotten
            .get(&desc)
            .is_some_and(|&last_tick| time.tick.saturating_sub(last_tick) < FORGOTTEN_COOLDOWN);
        if !on_cooldown {
            knowledge.recently_forgotten.insert(desc.clone(), time.tick);
            log.push(
                time.tick,
                format!("The colony has forgotten {desc}."),
                NarrativeTier::Significant,
            );
        }
    }

    // Prune stale cooldown entries.
    knowledge
        .recently_forgotten
        .retain(|_, tick| time.tick.saturating_sub(*tick) < FORGOTTEN_COOLDOWN);

    // Apply decay after cleanup (so newly-promoted entries don't decay on
    // their first tick).
    for entry in &mut knowledge.entries {
        entry.strength = (entry.strength - DECAY_PER_TICK).max(0.0);
    }

    // --- Promotion scan (every SCAN_INTERVAL ticks) ------------------------
    if !time.tick.is_multiple_of(SCAN_INTERVAL) {
        return;
    }

    // Group memories by (event_type, bucketed_location).
    let mut memory_groups: HashMap<(MemoryType, Option<Position>), (u32, f32)> = HashMap::new();

    for memory in &cats {
        // Track which groups this cat contributes to (avoid double-counting
        // if a cat has two memories of the same type at the same location).
        let mut seen_groups: Vec<(MemoryType, Option<Position>)> = Vec::new();

        for entry in &memory.events {
            let bucketed = entry.location.map(|p| ColonyKnowledge::bucket_position(&p));
            let key = (entry.event_type, bucketed);

            if seen_groups.contains(&key) {
                continue;
            }
            seen_groups.push(key);

            let group = memory_groups.entry(key).or_insert((0, 0.0));
            group.0 += 1;          // carrier count
            group.1 += entry.strength; // sum of strengths (for averaging)
        }
    }

    // Promote qualifying groups and update carrier counts for existing entries.
    for ((event_type, location), (count, strength_sum)) in &memory_groups {
        if let Some(idx) = knowledge.find_entry(*event_type, location) {
            // Update carrier count for existing entry.
            knowledge.entries[idx].carrier_count = *count;
        } else if *count >= PROMOTION_THRESHOLD {
            // New promotion.
            let avg_strength = strength_sum / *count as f32;
            knowledge.entries.push(KnowledgeEntry {
                event_type: *event_type,
                location: *location,
                strength: avg_strength,
                carrier_count: *count,
            });
        }
    }

    // Zero out carrier counts for entries that no longer have matching memories.
    for entry in &mut knowledge.entries {
        let key = (entry.event_type, entry.location);
        if !memory_groups.contains_key(&key) {
            entry.carrier_count = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use crate::components::mental::MemoryEntry;
    use crate::resources::narrative::NarrativeLog;
    use crate::resources::time::TimeState;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(ColonyKnowledge::default());
        world.insert_resource(NarrativeLog::default());
        let mut time = TimeState::default();
        time.tick = 500; // divisible by SCAN_INTERVAL
        world.insert_resource(time);

        let mut schedule = Schedule::default();
        schedule.add_systems(update_colony_knowledge);
        (world, schedule)
    }

    fn make_memory(event_type: MemoryType, location: Position, strength: f32) -> MemoryEntry {
        MemoryEntry {
            event_type,
            location: Some(location),
            involved: vec![],
            tick: 0,
            strength,
            firsthand: true,
        }
    }

    fn spawn_cat_with_memory(world: &mut World, entries: Vec<MemoryEntry>) -> Entity {
        let mut memory = Memory::default();
        for entry in entries {
            memory.remember(entry);
        }
        world.spawn(memory).id()
    }

    #[test]
    fn promotes_when_three_cats_share_memory() {
        let (mut world, mut schedule) = setup_world();

        let pos = Position::new(10, 10);
        for _ in 0..3 {
            spawn_cat_with_memory(
                &mut world,
                vec![make_memory(MemoryType::ThreatSeen, pos, 0.8)],
            );
        }

        schedule.run(&mut world);

        let knowledge = world.resource::<ColonyKnowledge>();
        assert_eq!(knowledge.entries.len(), 1, "should have promoted one entry");
        assert_eq!(knowledge.entries[0].event_type, MemoryType::ThreatSeen);
        assert_eq!(knowledge.entries[0].carrier_count, 3);
    }

    #[test]
    fn does_not_promote_below_threshold() {
        let (mut world, mut schedule) = setup_world();

        let pos = Position::new(10, 10);
        for _ in 0..2 {
            spawn_cat_with_memory(
                &mut world,
                vec![make_memory(MemoryType::ThreatSeen, pos, 0.8)],
            );
        }

        schedule.run(&mut world);

        let knowledge = world.resource::<ColonyKnowledge>();
        assert!(knowledge.entries.is_empty(), "should not promote with only 2 carriers");
    }

    #[test]
    fn decay_removes_weak_entries() {
        let (mut world, mut schedule) = setup_world();

        // Insert a colony knowledge entry with very low strength.
        {
            let mut knowledge = world.resource_mut::<ColonyKnowledge>();
            knowledge.entries.push(KnowledgeEntry {
                event_type: MemoryType::ResourceFound,
                location: Some(Position::new(7, 7)),
                strength: 0.0005, // below decay per tick
                carrier_count: 3,
            });
        }

        // Run once — decay will push strength to ~0.0 or below, but the entry
        // won't be removed until the NEXT tick (retain checks before decay).
        // Actually, retain checks strength <= 0 first. 0.0005 > 0 so it survives
        // retain, then decays to ~-0.0005. On the next tick, retain catches it.
        schedule.run(&mut world);
        schedule.run(&mut world);

        let knowledge = world.resource::<ColonyKnowledge>();
        assert!(
            knowledge.entries.is_empty(),
            "weak entry should be removed after decay"
        );

        let log = world.resource::<NarrativeLog>();
        assert!(
            log.entries.iter().any(|e| e.text.contains("forgotten")),
            "should narrate knowledge loss"
        );
    }

    #[test]
    fn carrier_count_zero_removes_entry() {
        let (mut world, mut schedule) = setup_world();

        // Insert an entry, but no cats hold a matching memory.
        {
            let mut knowledge = world.resource_mut::<ColonyKnowledge>();
            knowledge.entries.push(KnowledgeEntry {
                event_type: MemoryType::Death,
                location: Some(Position::new(12, 12)),
                strength: 0.8,
                carrier_count: 3,
            });
        }

        // No cats spawned → carrier count will be set to 0 on scan.
        // But we need tick % 50 == 0 for the scan.
        schedule.run(&mut world);

        let knowledge = world.resource::<ColonyKnowledge>();
        let count = knowledge.entries.iter()
            .find(|e| e.event_type == MemoryType::Death)
            .map(|e| e.carrier_count);
        assert_eq!(count, Some(0), "carrier count should be 0 with no matching memories");

        // Next tick: entry should be removed (carrier_count == 0 caught by retain).
        {
            let mut time = world.resource_mut::<TimeState>();
            time.tick = 51; // not a scan tick, but cleanup still runs
        }
        schedule.run(&mut world);

        let knowledge = world.resource::<ColonyKnowledge>();
        assert!(
            knowledge.entries.is_empty(),
            "entry with 0 carriers should be removed"
        );
    }

    #[test]
    fn does_not_duplicate_existing_entry() {
        let (mut world, mut schedule) = setup_world();

        let pos = Position::new(10, 10);
        let bucketed = ColonyKnowledge::bucket_position(&pos);

        // Pre-insert an existing colony knowledge entry.
        {
            let mut knowledge = world.resource_mut::<ColonyKnowledge>();
            knowledge.entries.push(KnowledgeEntry {
                event_type: MemoryType::ThreatSeen,
                location: Some(bucketed),
                strength: 0.5,
                carrier_count: 3,
            });
        }

        // Spawn 4 cats with matching memories.
        for _ in 0..4 {
            spawn_cat_with_memory(
                &mut world,
                vec![make_memory(MemoryType::ThreatSeen, pos, 0.9)],
            );
        }

        schedule.run(&mut world);

        let knowledge = world.resource::<ColonyKnowledge>();
        let matching: Vec<_> = knowledge.entries.iter()
            .filter(|e| e.event_type == MemoryType::ThreatSeen)
            .collect();
        assert_eq!(matching.len(), 1, "should not create duplicate entry");
        assert_eq!(matching[0].carrier_count, 4, "should update carrier count");
    }
}
