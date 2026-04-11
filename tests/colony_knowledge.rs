use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::components::mental::{Memory, MemoryEntry, MemoryType};
use clowder::components::physical::Position;
use clowder::resources::colony_knowledge::ColonyKnowledge;
use clowder::resources::narrative::{NarrativeLog, NarrativeTier};
use clowder::resources::time::TimeState;
use clowder::systems::colony_knowledge::update_colony_knowledge;

fn setup_world(tick: u64) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(ColonyKnowledge::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(clowder::resources::SimConstants::default());
    world.insert_resource(clowder::resources::SystemActivation::default());
    world.insert_resource(TimeState {
        tick,
        paused: false,
        speed: clowder::resources::SimSpeed::Normal,
    });
    let mut schedule = Schedule::default();
    schedule.add_systems(update_colony_knowledge);
    (world, schedule)
}

fn make_memory(event_type: MemoryType, x: i32, y: i32, strength: f32) -> MemoryEntry {
    MemoryEntry {
        event_type,
        location: Some(Position::new(x, y)),
        involved: vec![],
        tick: 0,
        strength,
        firsthand: true,
    }
}

fn spawn_cat_with_memories(world: &mut World, entries: Vec<MemoryEntry>) -> Entity {
    let mut memory = Memory::default();
    for e in entries {
        memory.remember(e);
    }
    world.spawn(memory).id()
}

/// When 3+ cats share the same memory (type + approximate location), it
/// promotes to colony knowledge.
#[test]
fn knowledge_promotes_at_threshold() {
    let (mut world, mut schedule) = setup_world(500);

    for _ in 0..3 {
        spawn_cat_with_memories(
            &mut world,
            vec![make_memory(MemoryType::ThreatSeen, 10, 10, 0.8)],
        );
    }

    schedule.run(&mut world);

    let knowledge = world.resource::<ColonyKnowledge>();
    assert_eq!(knowledge.entries.len(), 1);
    assert_eq!(knowledge.entries[0].event_type, MemoryType::ThreatSeen);
    assert_eq!(knowledge.entries[0].carrier_count, 3);
}

/// Colony knowledge is not promoted below the threshold.
#[test]
fn knowledge_does_not_promote_below_threshold() {
    let (mut world, mut schedule) = setup_world(500);

    for _ in 0..2 {
        spawn_cat_with_memories(
            &mut world,
            vec![make_memory(MemoryType::ThreatSeen, 10, 10, 0.8)],
        );
    }

    schedule.run(&mut world);

    let knowledge = world.resource::<ColonyKnowledge>();
    assert!(knowledge.entries.is_empty());
}

/// When all carriers die (carrier_count drops to 0), the entry is removed
/// and a "forgotten" narrative fires.
#[test]
fn knowledge_lost_when_carriers_gone() {
    let (mut world, mut schedule) = setup_world(500);

    // Manually insert a colony knowledge entry.
    {
        let bucketed = ColonyKnowledge::bucket_position(&Position::new(10, 10));
        let mut ck = world.resource_mut::<ColonyKnowledge>();
        ck.entries.push(clowder::resources::colony_knowledge::KnowledgeEntry {
            event_type: MemoryType::ThreatSeen,
            location: Some(bucketed),
            strength: 0.8,
            carrier_count: 3,
        });
    }

    // No cats spawned → carrier count drops to 0 on next scan.
    schedule.run(&mut world);

    // Next tick: entry removed.
    {
        let mut time = world.resource_mut::<TimeState>();
        time.tick = 501;
    }
    schedule.run(&mut world);

    let knowledge = world.resource::<ColonyKnowledge>();
    assert!(knowledge.entries.is_empty(), "entry should be removed with 0 carriers");

    let log = world.resource::<NarrativeLog>();
    assert!(
        log.entries.iter().any(|e| e.text.contains("forgotten")),
        "should narrate knowledge loss"
    );
    assert!(
        log.entries.iter().any(|e| e.tier == NarrativeTier::Significant),
        "knowledge loss should be tier Significant"
    );
}
