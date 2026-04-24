use bevy_ecs::prelude::*;

use crate::components::mental::Memory;
use crate::components::physical::Dead;

// ---------------------------------------------------------------------------
// decay_memories system
// ---------------------------------------------------------------------------

/// Each tick, decay the strength of every memory entry. Firsthand memories
/// fade slowly (0.001/tick), secondhand memories faster (0.002/tick). Entries
/// whose strength drops to zero are evicted.
pub fn decay_memories(mut query: Query<&mut Memory, Without<Dead>>) {
    for mut memory in &mut query {
        for entry in memory.events.iter_mut() {
            let rate = if entry.firsthand { 0.001 } else { 0.002 };
            entry.strength -= rate;
        }
        memory.events.retain(|e| e.strength > 0.0);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::mental::{MemoryEntry, MemoryType};
    use crate::components::physical::Position;
    use bevy_ecs::schedule::Schedule;

    fn setup_world() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_memories);
        (world, schedule)
    }

    fn make_entry(firsthand: bool, strength: f32) -> MemoryEntry {
        MemoryEntry {
            event_type: MemoryType::ResourceFound,
            location: Some(Position::new(5, 5)),
            involved: vec![],
            tick: 0,
            strength,
            firsthand,
        }
    }

    #[test]
    fn firsthand_memory_decays_at_0001_per_tick() {
        let (mut world, mut schedule) = setup_world();
        let mut memory = Memory::default();
        memory.remember(make_entry(true, 1.0));
        let entity = world.spawn(memory).id();

        schedule.run(&mut world);

        let mem = world.get::<Memory>(entity).unwrap();
        let strength = mem.events[0].strength;
        assert!(
            (strength - 0.999).abs() < 1e-5,
            "firsthand memory should decay to ~0.999; got {strength}"
        );
    }

    #[test]
    fn secondhand_memory_decays_at_0002_per_tick() {
        let (mut world, mut schedule) = setup_world();
        let mut memory = Memory::default();
        memory.remember(make_entry(false, 1.0));
        let entity = world.spawn(memory).id();

        schedule.run(&mut world);

        let mem = world.get::<Memory>(entity).unwrap();
        let strength = mem.events[0].strength;
        assert!(
            (strength - 0.998).abs() < 1e-5,
            "secondhand memory should decay to ~0.998; got {strength}"
        );
    }

    #[test]
    fn weak_memories_evicted_when_strength_zero() {
        let (mut world, mut schedule) = setup_world();
        let mut memory = Memory::default();
        memory.remember(make_entry(true, 0.0005)); // will drop below 0 after 1 tick
        memory.remember(make_entry(true, 1.0)); // will survive
        let entity = world.spawn(memory).id();

        schedule.run(&mut world);

        let mem = world.get::<Memory>(entity).unwrap();
        assert_eq!(mem.events.len(), 1, "weak memory should be evicted");
        assert!(
            mem.events[0].strength > 0.9,
            "surviving memory should be the strong one"
        );
    }
}
