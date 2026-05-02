use crate::components::mental::{Memory, MemoryEntry, MemoryType};
use crate::components::physical::{Needs, Position};
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Sleep`
///
/// **Real-world effect** — every tick, raises `needs.energy` and
/// `needs.temperature` by the configured per-tick deltas. On the
/// `Advance` branch (chain completion), writes a `MemoryType::Sleep`
/// entry to the cat's `Memory` recording the rest spot for the future
/// `LandmarkAnchor::OwnSafeRestSpot` substrate (ticket 089).
///
/// **Plan-level preconditions** — emitted under `ZoneIs(ResidenceZone)`
/// by `src/ai/planner/actions.rs::sleeping_actions`. Duration is
/// passed in by the caller (varies with energy deficit).
///
/// **Runtime preconditions** — none; the effect runs every tick
/// unconditionally while in progress.
///
/// **Witness** — `StepOutcome<()>`. Sleep's effect is
/// unconditional once the step is running; there's no failure
/// path that returns Advance without having slept.
///
/// **Feature emission** — none. Sleep is not currently tracked as
/// a Positive Feature (it's ubiquitous and not a diagnostic
/// signal on its own).
pub fn resolve_sleep(
    ticks: u64,
    duration: u64,
    needs: &mut Needs,
    memory: &mut Memory,
    self_position: &Position,
    tick: u64,
    d: &DispositionConstants,
) -> StepOutcome<()> {
    needs.energy = (needs.energy + d.sleep_energy_per_tick).min(1.0);
    needs.temperature = (needs.temperature + d.sleep_temperature_per_tick).min(1.0);
    if ticks >= duration {
        memory.remember(MemoryEntry {
            event_type: MemoryType::Sleep,
            location: Some(*self_position),
            involved: Vec::new(),
            tick,
            strength: d.safe_rest_memory_strength_initial,
            firsthand: true,
        });
        StepOutcome::bare(StepResult::Advance)
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::sim_constants::SimConstants;

    fn fresh_needs() -> Needs {
        Needs {
            hunger: 0.5,
            energy: 0.3,
            temperature: 0.5,
            safety: 0.5,
            social: 0.5,
            acceptance: 0.5,
            mating: 0.5,
            respect: 0.5,
            mastery: 0.5,
            purpose: 0.5,
        }
    }

    #[test]
    fn resolve_sleep_advance_writes_safe_rest_memory() {
        let d = &SimConstants::default().disposition;
        let mut needs = fresh_needs();
        let mut memory = Memory::default();
        let pos = Position::new(7, 3);
        let outcome = resolve_sleep(100, 100, &mut needs, &mut memory, &pos, 50, d);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert_eq!(memory.events.len(), 1);
        let entry = memory.events.back().unwrap();
        assert_eq!(entry.event_type, MemoryType::Sleep);
        assert_eq!(entry.location, Some(pos));
        assert_eq!(entry.tick, 50);
        assert!((entry.strength - d.safe_rest_memory_strength_initial).abs() < 1e-6);
        assert!(entry.firsthand);
    }

    #[test]
    fn resolve_sleep_continue_does_not_write_memory() {
        let d = &SimConstants::default().disposition;
        let mut needs = fresh_needs();
        let mut memory = Memory::default();
        let pos = Position::new(0, 0);
        let outcome = resolve_sleep(50, 100, &mut needs, &mut memory, &pos, 50, d);
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(memory.events.is_empty());
    }
}
