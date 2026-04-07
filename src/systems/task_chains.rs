use bevy_ecs::prelude::*;

use crate::ai::pathfinding::step_toward;
use crate::ai::CurrentAction;
use crate::components::building::{ConstructionSite, CropState, Structure, StructureType};
use crate::components::physical::{Dead, Needs, Position};
use crate::components::skills::Skills;
use crate::components::task_chain::{StepKind, StepStatus, TaskChain};
use crate::resources::food::FoodStores;
use crate::resources::map::TileMap;
use crate::resources::time::{Season, SimConfig, TimeState};

// ---------------------------------------------------------------------------
// resolve_task_chains
// ---------------------------------------------------------------------------

/// Ticks each cat's `TaskChain` one step forward.
///
/// Runs after `evaluate_actions` (which creates chains) and before
/// `resolve_actions` (which should skip cats that have a chain).
#[allow(clippy::type_complexity)]
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
            &mut Structure,
            Option<&mut ConstructionSite>,
            Option<&mut CropState>,
            &Position,
        ),
        Without<TaskChain>,
    >,
    map: Res<TileMap>,
    mut food: ResMut<FoodStores>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut commands: Commands,
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

    // Cache workshop positions for skill growth bonus.
    let workshop_positions: Vec<Position> = buildings
        .iter()
        .filter(|(s, site, _, _)| {
            s.kind == StructureType::Workshop && s.effectiveness() > 0.0 && site.is_none()
        })
        .map(|(_, _, _, pos)| *pos)
        .collect();

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
                *ticks_elapsed += 1;
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

        match &step.kind {
            StepKind::MoveTo => {
                let Some(target) = step.target_position else {
                    chain.fail_current("no target position for MoveTo".into());
                    continue;
                };
                if pos.manhattan_distance(&target) == 0 {
                    chain.advance();
                } else if let Some(next) = step_toward(&pos, &target, &map) {
                    *pos = next;
                } else if ticks > 20 {
                    chain.fail_current("stuck for 20 ticks".into());
                }
            }

            StepKind::Gather { .. } => {
                if ticks >= 5 {
                    skills.building += skills.growth_rate() * 0.005 * workshop_bonus;
                    chain.advance();
                }
            }

            StepKind::Deliver { material, amount } => {
                if let Some(target_entity) = step.target_entity {
                    if let Ok((_, Some(mut site), _, _)) = buildings.get_mut(target_entity) {
                        site.deliver(*material, *amount);
                    }
                }
                chain.advance();
            }

            StepKind::Construct => {
                let Some(target_entity) = step.target_entity else {
                    chain.fail_current("no target for Construct".into());
                    continue;
                };

                let Ok((_, maybe_site, _, building_pos)) = buildings.get_mut(target_entity) else {
                    chain.fail_current("construction site not found".into());
                    continue;
                };

                if pos.manhattan_distance(building_pos) > 1 {
                    if let Some(next) = step_toward(&pos, building_pos, &map) {
                        *pos = next;
                    }
                    continue;
                }

                if let Some(mut site) = maybe_site {
                    if !site.materials_complete() {
                        chain.fail_current("materials not delivered".into());
                        continue;
                    }

                    let other_builders = builders_per_site
                        .get(&target_entity)
                        .copied()
                        .unwrap_or(1)
                        .max(1);
                    let rate =
                        skills.building * 0.02 * (1.0 + 0.3 * (other_builders as f32 - 1.0));
                    site.progress = (site.progress + rate).min(1.0);
                    skills.building += skills.growth_rate() * 0.01 * workshop_bonus;

                    if site.progress >= 1.0 {
                        let blueprint = site.blueprint;
                        commands.entity(target_entity).remove::<ConstructionSite>();
                        commands.entity(target_entity).insert(Structure::new(blueprint));
                        chain.advance();
                    }
                } else {
                    // ConstructionSite already removed — building is done.
                    chain.advance();
                }
            }

            StepKind::Repair => {
                let Some(target_entity) = step.target_entity else {
                    chain.fail_current("no target for Repair".into());
                    continue;
                };

                let Ok((mut structure, _, _, building_pos)) = buildings.get_mut(target_entity)
                else {
                    chain.fail_current("building not found".into());
                    continue;
                };

                if pos.manhattan_distance(building_pos) > 1 {
                    if let Some(next) = step_toward(&pos, building_pos, &map) {
                        *pos = next;
                    }
                    continue;
                }

                structure.condition = (structure.condition + skills.building * 0.01).min(1.0);
                skills.building += skills.growth_rate() * 0.01 * workshop_bonus;

                if structure.condition >= 1.0 {
                    chain.advance();
                }
            }

            StepKind::Tend => {
                let Some(target_entity) = step.target_entity else {
                    chain.fail_current("no target for Tend".into());
                    continue;
                };

                let Ok((_, _, maybe_crop, garden_pos)) = buildings.get_mut(target_entity) else {
                    chain.fail_current("garden not found".into());
                    continue;
                };

                if pos.manhattan_distance(garden_pos) > 1 {
                    if let Some(next) = step_toward(&pos, garden_pos, &map) {
                        *pos = next;
                    }
                    continue;
                }

                if let Some(mut crop) = maybe_crop {
                    crop.growth += skills.foraging * season_mod * 0.01;
                    skills.foraging += skills.growth_rate() * 0.005 * workshop_bonus;

                    if crop.growth >= 1.0 {
                        chain.advance();
                    }
                } else {
                    chain.fail_current("no CropState on garden".into());
                }
            }

            StepKind::Harvest => {
                let Some(target_entity) = step.target_entity else {
                    chain.fail_current("no target for Harvest".into());
                    continue;
                };

                let Ok((_, _, maybe_crop, _)) = buildings.get_mut(target_entity) else {
                    chain.fail_current("garden not found".into());
                    continue;
                };

                if let Some(mut crop) = maybe_crop {
                    food.deposit(3.0);
                    crop.growth = 0.0;
                    chain.advance();
                } else {
                    chain.fail_current("no CropState on garden".into());
                }
            }

            // Magic/herbcraft steps are resolved by the magic task chain system.
            StepKind::GatherHerb
            | StepKind::PrepareRemedy { .. }
            | StepKind::ApplyRemedy { .. }
            | StepKind::SetWard { .. }
            | StepKind::Scry
            | StepKind::CleanseCorruption
            | StepKind::SpiritCommunion => {
                // Handled by systems::magic::resolve_magic_task_chains
            }

            // Disposition-driven steps are resolved by the disposition system.
            StepKind::HuntPrey
            | StepKind::ForageItem
            | StepKind::DepositAtStores
            | StepKind::EatAtStores
            | StepKind::Sleep { .. }
            | StepKind::SelfGroom
            | StepKind::Socialize
            | StepKind::GroomOther
            | StepKind::MentorCat
            | StepKind::PatrolTo
            | StepKind::FightThreat
            | StepKind::Survey
            | StepKind::DeliverDirective => {
                // Handled by systems::disposition::resolve_disposition_chains
            }
        }

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
    use bevy_ecs::schedule::Schedule;
    use crate::ai::Action;
    use crate::components::building::StructureType;
    use crate::components::task_chain::{FailurePolicy, Material, TaskStep};

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
                    size: (2, 2),
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
        let mut world = test_world();

        let garden = world
            .spawn((
                Structure::new(StructureType::Garden),
                CropState { growth: 1.0 },
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

        let before = world.resource::<FoodStores>().current;

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_task_chains);
        schedule.run(&mut world);

        let after = world.resource::<FoodStores>().current;
        assert!(
            after > before,
            "harvest should deposit food (before={before}, after={after})"
        );

        let crop = world.get::<CropState>(garden).unwrap();
        assert_eq!(crop.growth, 0.0, "crop should reset after harvest");
    }
}
