use bevy_ecs::prelude::*;

use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::food::FoodStores;

// ---------------------------------------------------------------------------
// emit_periodic_events system
// ---------------------------------------------------------------------------

/// Emit periodic food-level, prey population, and wildlife population
/// snapshots to the event log. All three share the `economy_interval`.
pub fn emit_periodic_events(
    config: Res<crate::resources::snapshot_config::SnapshotConfig>,
    time: Res<crate::resources::time::TimeState>,
    food: Res<FoodStores>,
    prey: Query<&crate::components::prey::PreyConfig, With<crate::components::prey::PreyAnimal>>,
    wildlife: Query<&crate::components::wildlife::WildAnimal>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let interval = config.economy_interval;
    if let Some(ref mut log) = event_log {
        if interval > 0 && time.tick.is_multiple_of(interval) {
            log.push(
                time.tick,
                EventKind::FoodLevel {
                    current: food.current,
                    capacity: food.capacity,
                    fraction: food.fraction(),
                },
            );

            use crate::components::prey::PreyKind;
            let (mut mice, mut rats, mut rabbits, mut fish, mut birds) = (0, 0, 0, 0, 0);
            for p in &prey {
                match p.kind {
                    PreyKind::Mouse => mice += 1,
                    PreyKind::Rat => rats += 1,
                    PreyKind::Rabbit => rabbits += 1,
                    PreyKind::Fish => fish += 1,
                    PreyKind::Bird => birds += 1,
                }
            }
            log.push(
                time.tick,
                EventKind::PopulationSnapshot {
                    mice,
                    rats,
                    rabbits,
                    fish,
                    birds,
                },
            );

            use crate::components::wildlife::WildSpecies;
            let (mut foxes, mut hawks, mut snakes, mut shadow_foxes) = (0u32, 0u32, 0u32, 0u32);
            for w in &wildlife {
                match w.species {
                    WildSpecies::Fox => foxes += 1,
                    WildSpecies::Hawk => hawks += 1,
                    WildSpecies::Snake => snakes += 1,
                    WildSpecies::ShadowFox => shadow_foxes += 1,
                }
            }
            log.push(
                time.tick,
                EventKind::WildlifePopulation {
                    foxes,
                    hawks,
                    snakes,
                    shadow_foxes,
                },
            );
        }
    }
}
