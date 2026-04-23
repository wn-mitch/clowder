use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::ai::CurrentAction;
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::physical::{Dead, Needs, Position};
use crate::components::skills::Skills;
use crate::components::task_chain::{StepKind, StepStatus, TaskChain};
use crate::resources::colony_score::ColonyScore;
use crate::resources::map::TileMap;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{Season, SimConfig, TimeState};

// ---------------------------------------------------------------------------
// resolve_task_chains
// ---------------------------------------------------------------------------

/// Ticks each cat's `TaskChain` one step forward.
///
/// Runs after `evaluate_actions` (which creates chains) and before
/// `resolve_actions` (which should skip cats that have a chain).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_task_chains(
    mut cats: Query<
        (
            Entity,
            &mut TaskChain,
            &mut CurrentAction,
            &mut Position,
            &mut Skills,
            &mut Needs,
        ),
        (Without<Dead>, Without<Structure>),
    >,
    mut buildings: Query<
        (
            Entity,
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ),
        Without<TaskChain>,
    >,
    mut stored_items: Query<&mut StoredItems>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut commands: Commands,
    mut colony_score: Option<ResMut<ColonyScore>>,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let season = time.season(&config);
    let season_mod = match season {
        Season::Spring => 0.8,
        Season::Summer => 1.0,
        Season::Autumn => 0.5,
        Season::Winter => 0.0,
    };

    // Count builders per construction site for cooperative bonus.
    let mut builders_per_site: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::new();
    for (_, chain, _, _, _, _) in &cats {
        if let Some(step) = chain.current() {
            if matches!(step.kind, StepKind::Construct) {
                if let Some(target) = step.target_entity {
                    *builders_per_site.entry(target).or_insert(0) += 1;
                }
            }
        }
    }

    // Cache stores for depositing harvested items.
    let stores_list: Vec<(Entity, Position)> = buildings
        .iter()
        .filter(|(_, s, site, _, _)| s.kind == StructureType::Stores && site.is_none())
        .map(|(e, _, _, _, pos)| (e, *pos))
        .collect();

    // Cache workshop positions for skill growth bonus.
    let workshop_positions: Vec<Position> = buildings
        .iter()
        .filter(|(_, s, site, _, _)| {
            s.kind == StructureType::Workshop && s.effectiveness() > 0.0 && site.is_none()
        })
        .map(|(_, _, _, _, pos)| *pos)
        .collect();

    // Snapshot tile occupancy for anti-stacking jitter on arrival.
    let cat_tile_counts: HashMap<Position, u32> = {
        let mut counts = HashMap::new();
        for (_, _, _, pos, _, _) in &cats {
            *counts.entry(*pos).or_insert(0) += 1;
        }
        counts
    };

    let mut chains_to_remove: Vec<Entity> = Vec::new();

    for (cat_entity, mut chain, mut current, mut pos, mut skills, _needs) in &mut cats {
        let Some(step) = chain.current_mut() else {
            chains_to_remove.push(cat_entity);
            current.ticks_remaining = 0;
            continue;
        };

        // Ensure step is in progress.
        if matches!(step.status, StepStatus::Pending) {
            step.status = StepStatus::InProgress { ticks_elapsed: 0 };
        }

        let ticks = match &mut step.status {
            StepStatus::InProgress { ticks_elapsed } => {
                // Steps handled by other systems (disposition, magic) manage
                // their own timers — don't increment here or they tick 2×.
                if !step.kind.is_externally_timed() {
                    *ticks_elapsed += 1;
                }
                *ticks_elapsed
            }
            _ => continue,
        };

        let workshop_bonus = if workshop_positions
            .iter()
            .any(|wp| pos.manhattan_distance(wp) <= 2)
        {
            1.5
        } else {
            1.0
        };

        // Extract step data before the match to avoid borrow conflicts.
        let step_target_entity = step.target_entity;
        let step_target_position = step.target_position;

        use crate::steps::StepResult;
        let apply = |result: StepResult, chain: &mut TaskChain| match result {
            StepResult::Continue => {}
            StepResult::Advance => {
                chain.advance();
            }
            StepResult::Fail(reason) => {
                chain.fail_current(reason);
            }
        };

        match &step.kind {
            StepKind::MoveTo => {
                let cached = &mut step.cached_path;
                apply(
                    crate::steps::building::resolve_move_to(
                        &mut pos,
                        step_target_position,
                        cached,
                        &map,
                        &cat_tile_counts,
                    ),
                    &mut chain,
                );
            }

            StepKind::Gather { .. } => {
                apply(
                    crate::steps::building::resolve_gather(ticks, &mut skills, workshop_bonus),
                    &mut chain,
                );
            }

            StepKind::Deliver { material, amount } => {
                let outcome = crate::steps::building::resolve_deliver(
                    *material,
                    *amount,
                    step_target_entity,
                    &mut buildings,
                );
                outcome.record_if_witnessed(
                    activation.as_deref_mut(),
                    Feature::MaterialsDelivered,
                );
                apply(outcome.result, &mut chain);
            }

            StepKind::Construct => {
                let cached = &mut step.cached_path;
                let result = crate::steps::building::resolve_construct(
                    step_target_entity,
                    &mut pos,
                    cached,
                    &mut skills,
                    workshop_bonus,
                    &builders_per_site,
                    &mut buildings,
                    &map,
                    &mut commands,
                    &mut colony_score,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut act) = activation {
                        act.record(Feature::BuildingConstructed);
                    }
                }
                apply(result, &mut chain);
            }

            StepKind::Repair => {
                let cached = &mut step.cached_path;
                apply(
                    crate::steps::building::resolve_repair(
                        step_target_entity,
                        &mut pos,
                        cached,
                        &mut skills,
                        workshop_bonus,
                        &mut buildings,
                        &map,
                    ),
                    &mut chain,
                );
            }

            StepKind::Tend => {
                let cached = &mut step.cached_path;
                let outcome = crate::steps::building::resolve_tend(
                    step_target_entity,
                    &mut pos,
                    cached,
                    &mut skills,
                    season_mod,
                    workshop_bonus,
                    &mut buildings,
                    &map,
                );
                // Disposition-chain path is currently unscheduled (GOAP
                // replaced it); if it's ever reinstated, wire the
                // `outcome.witness` (bool tended) through to
                // `SystemActivation` via `record_if_witnessed` here.
                apply(outcome.result, &mut chain);
            }

            StepKind::Harvest => {
                let outcome = crate::steps::building::resolve_harvest(
                    step_target_entity,
                    &pos,
                    &stores_list,
                    &mut buildings,
                    &mut stored_items,
                    &mut commands,
                );
                outcome.record_if_witnessed(
                    activation.as_deref_mut(),
                    Feature::CropHarvested,
                );
                apply(outcome.result, &mut chain);
            }

            // Magic/herbcraft and disposition steps are resolved by their own systems.
            _ => {}
        }

        // After the step match, sync CurrentAction targets from whatever step
        // is now active (may have changed via advance/fail inside the match).
        chain.sync_targets(&mut current);

        if chain.is_complete() {
            chains_to_remove.push(cat_entity);
            current.ticks_remaining = 0;
        }
    }

    for entity in chains_to_remove {
        commands.entity(entity).remove::<TaskChain>();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Action;
    use crate::components::building::StructureType;
    use crate::components::task_chain::{FailurePolicy, Material, TaskStep};
    use crate::resources::food::FoodStores;
    use bevy_ecs::schedule::Schedule;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, crate::resources::map::Terrain::Grass));
        world.insert_resource(FoodStores::default());
        world.insert_resource(TimeState {
            tick: 100_000,
            paused: false,
            speed: crate::resources::SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world
    }

    fn build_action() -> CurrentAction {
        CurrentAction {
            action: Action::Build,
            ticks_remaining: u64::MAX,
            target_position: None,
            target_entity: None,
            last_scores: Vec::new(),
        }
    }

    #[test]
    fn move_to_step_moves_cat() {
        let mut world = test_world();

        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::MoveTo).with_position(Position::new(5, 5))],
            FailurePolicy::AbortChain,
        );

        let cat = world
            .spawn((
                chain,
                build_action(),
                Position::new(0, 0),
                Skills::default(),
                Needs::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_task_chains);
        schedule.run(&mut world);

        let pos = *world.get::<Position>(cat).unwrap();
        assert!(
            pos.manhattan_distance(&Position::new(0, 0)) > 0,
            "cat should have moved from origin"
        );
    }

    #[test]
    fn gather_completes_after_5_ticks() {
        let mut world = test_world();

        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::Gather {
                material: Material::Wood,
                amount: 3,
            })],
            FailurePolicy::AbortChain,
        );

        let cat = world
            .spawn((
                chain,
                build_action(),
                Position::new(5, 5),
                Skills::default(),
                Needs::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_task_chains);

        for _ in 0..4 {
            schedule.run(&mut world);
        }
        assert!(
            world.get::<TaskChain>(cat).is_some(),
            "chain should still be active after 4 ticks"
        );

        // 5th tick completes, then one more to apply Commands.
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert!(
            world.get::<TaskChain>(cat).is_none(),
            "chain should be removed after gather completes"
        );
    }

    #[test]
    fn repair_increases_condition() {
        let mut world = test_world();

        let building = world
            .spawn((
                Structure {
                    kind: StructureType::Den,
                    condition: 0.5,
                    cleanliness: 1.0,
                    size: StructureType::Den.default_size(),
                },
                Position::new(5, 5),
            ))
            .id();

        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::Repair).with_entity(building)],
            FailurePolicy::AbortChain,
        );

        world.spawn((
            chain,
            build_action(),
            Position::new(5, 5),
            Skills {
                building: 0.5,
                ..Skills::default()
            },
            Needs::default(),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_task_chains);
        schedule.run(&mut world);

        let s = world.get::<Structure>(building).unwrap();
        assert!(
            s.condition > 0.5,
            "repair should increase condition (got {})",
            s.condition
        );
    }

    #[test]
    fn harvest_deposits_food() {
        use crate::components::building::StoredItems;
        use crate::components::items::Item;

        let mut world = test_world();

        // A Stores building for the harvest to deposit into.
        let store = world
            .spawn((
                Structure::new(StructureType::Stores),
                StoredItems::default(),
                Position::new(6, 5),
            ))
            .id();

        let garden = world
            .spawn((
                Structure::new(StructureType::Garden),
                CropState {
                    growth: 1.0,
                    ..Default::default()
                },
                Position::new(5, 5),
            ))
            .id();

        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::Harvest).with_entity(garden)],
            FailurePolicy::AbortChain,
        );

        world.spawn((
            chain,
            CurrentAction {
                action: Action::Farm,
                ticks_remaining: u64::MAX,
                target_position: None,
                target_entity: None,
                last_scores: Vec::new(),
            },
            Position::new(5, 5),
            Skills::default(),
            Needs::default(),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_task_chains);
        schedule.run(&mut world);

        // Harvest should create real food items in the store.
        let stored = world.get::<StoredItems>(store).unwrap();
        assert!(
            !stored.items.is_empty(),
            "harvest should deposit food items in stores"
        );

        // Verify the deposited items are real food.
        for &item_entity in &stored.items {
            let item = world.get::<Item>(item_entity).unwrap();
            assert!(item.kind.is_food(), "harvested item should be food");
        }

        let crop = world.get::<CropState>(garden).unwrap();
        assert_eq!(crop.growth, 0.0, "crop should reset after harvest");
    }
}
