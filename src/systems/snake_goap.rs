//! Snake GOAP systems — evaluate, plan, and resolve snake actions.
//!
//! Ticket 025 Phase 2 mirror of `hawk_goap.rs` for snakes. Adds the
//! thermoregulation tier — `sync_snake_needs` derives `warmth` from
//! terrain under the snake and from `SnakeState`.

use bevy_ecs::prelude::*;

use crate::ai::eval::{DseRegistry, ModifierPipeline};
use crate::ai::planner::core::make_plan;
use crate::ai::scoring::{EvalInputs, MarkerSnapshot};
use crate::ai::snake_planner::actions::actions_for_disposition;
use crate::ai::snake_planner::goals::goal_for_disposition;
use crate::ai::snake_planner::{
    SnakeDomain, SnakeGoapActionKind, SnakePlannerState, SnakeZone,
};
use crate::ai::snake_scoring::{
    score_snake_dispositions, select_snake_disposition_softmax, SnakeNeeds, SnakePersonality,
    SnakeScoringContext,
};
use crate::components::physical::{Dead, DeathCause, Health, Position};
use crate::components::prey::PreyAnimal;
use crate::components::snake_goap_plan::SnakeGoapPlan;
use crate::components::wildlife::{
    SnakeAiPhase, SnakeDied, SnakeState, WildAnimal, WildlifeAiState, WildlifeDeathCause,
};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::rng::SimRng;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeState;
use crate::steps::{snake as snake_steps, StepResult};

// Tuning placeholders — replaced in commit 5 by `SnakeEcologyConstants`.
const SNAKE_SOFTMAX_TEMPERATURE: f32 = 0.15;
const SNAKE_HUNGER_DECAY_PER_TICK: f32 = 0.000_05;
const SNAKE_WARMTH_DECAY_PER_TICK: f32 = 0.000_40;
const SNAKE_STARVATION_DEATH_TICKS: u64 = 60 * 60 * 24 * 5; // 5 days at 60 tps
const SNAKE_STRIKE_RANGE: i32 = 1;
const SNAKE_DETECTION_RANGE: i32 = 4;
const SNAKE_AMBUSH_SETTLE_TICKS: u64 = 40;
const SNAKE_BASK_TICKS: u64 = 180;

/// Terrains that warm a basking snake. Mirrors `WARM_TERRAINS` from
/// ticket 025 §4.
const WARM_TERRAINS: &[Terrain] = &[Terrain::Rock, Terrain::Sand];

fn is_warm(terrain: Terrain) -> bool {
    WARM_TERRAINS.contains(&terrain)
}

// ---------------------------------------------------------------------------
// snake_needs_tick — hunger + warmth decay, cooldown, age
// ---------------------------------------------------------------------------

pub fn snake_needs_tick(mut snakes: Query<(&mut SnakeState, &Position)>, map: Res<TileMap>) {
    for (mut snake, pos) in &mut snakes {
        snake.age_ticks += 1;
        if snake.satiation_ticks > 0 {
            snake.satiation_ticks -= 1;
        } else {
            snake.hunger = (snake.hunger + SNAKE_HUNGER_DECAY_PER_TICK).min(1.0);
        }
        snake.post_action_cooldown = snake.post_action_cooldown.saturating_sub(1);

        // Warmth: decays unless the snake is on warm terrain.
        if map.in_bounds(pos.x, pos.y) {
            let terrain = map.get(pos.x, pos.y).terrain;
            if !is_warm(terrain) {
                snake.warmth = (snake.warmth - SNAKE_WARMTH_DECAY_PER_TICK).max(0.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// sync_snake_needs — bridge SnakeState/Health → SnakeNeeds
// ---------------------------------------------------------------------------

pub fn sync_snake_needs(
    mut snakes: Query<(&SnakeState, &Health, &mut SnakeNeeds), With<WildAnimal>>,
) {
    for (snake_state, health, mut needs) in &mut snakes {
        needs.hunger = (1.0 - snake_state.hunger).clamp(0.0, 1.0);
        needs.health_fraction = (health.current / health.max).clamp(0.0, 1.0);
        needs.warmth = snake_state.warmth.clamp(0.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// snake_evaluate_and_plan
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn snake_evaluate_and_plan(
    mut commands: Commands,
    snakes: Query<
        (
            Entity,
            &SnakeState,
            &Position,
            &SnakeNeeds,
            &SnakePersonality,
        ),
        (With<WildAnimal>, Without<SnakeGoapPlan>, Without<Dead>),
    >,
    map: Res<TileMap>,
    cats: Query<
        &Position,
        (
            Without<WildAnimal>,
            Without<SnakeState>,
            With<Health>,
            Without<Dead>,
        ),
    >,
    prey: Query<&Position, (With<PreyAnimal>, Without<SnakeState>)>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    dse_registry: Res<DseRegistry>,
    modifier_pipeline: Res<ModifierPipeline>,
) {
    let cat_positions: Vec<Position> = cats.iter().copied().collect();
    let prey_positions: Vec<Position> = prey.iter().copied().collect();
    let markers = MarkerSnapshot::new();

    for (snake_entity, snake_state, snake_pos, needs, personality) in &snakes {
        let cats_nearby = cat_positions
            .iter()
            .filter(|p| p.manhattan_distance(snake_pos) <= 4)
            .count();
        let prey_nearby = prey_positions
            .iter()
            .any(|p| p.manhattan_distance(snake_pos) <= SNAKE_DETECTION_RANGE);
        let on_warm_terrain = if map.in_bounds(snake_pos.x, snake_pos.y) {
            is_warm(map.get(snake_pos.x, snake_pos.y).terrain)
        } else {
            false
        };

        let ctx = SnakeScoringContext {
            needs,
            personality,
            prey_nearby,
            cats_nearby,
            on_warm_terrain,
            self_position: *snake_pos,
            jitter_range: 0.05,
        };

        let inputs = EvalInputs {
            cat: snake_entity,
            position: *snake_pos,
            tick: time.tick,
            dse_registry: &dse_registry,
            modifier_pipeline: &modifier_pipeline,
            markers: &markers,
            colony_landmarks: &Default::default(),
            exploration_map: &Default::default(),
            corruption_landmarks: &Default::default(),
            focal_cat: None,
            focal_capture: None,
        };

        let scoring_result = score_snake_dispositions(&ctx, &inputs, &mut rng.rng);
        let Some(chosen) = select_snake_disposition_softmax(
            &scoring_result,
            &mut rng.rng,
            SNAKE_SOFTMAX_TEMPERATURE,
        ) else {
            continue;
        };

        let planner_state = build_planner_state(snake_state, snake_pos, &prey_positions, on_warm_terrain);
        let actions = actions_for_disposition(chosen);
        let goal = goal_for_disposition(chosen);

        let Some(steps) = make_plan::<SnakeDomain>(planner_state, &actions, &goal, 8, 500) else {
            continue;
        };

        let plan = SnakeGoapPlan::new(chosen, time.tick, steps);
        commands.entity(snake_entity).insert(plan);
    }
}

fn build_planner_state(
    snake_state: &SnakeState,
    snake_pos: &Position,
    prey_positions: &[Position],
    on_warm_terrain: bool,
) -> SnakePlannerState {
    let prey_in_range = prey_positions
        .iter()
        .any(|p| p.manhattan_distance(snake_pos) <= SNAKE_STRIKE_RANGE);
    SnakePlannerState {
        zone: SnakeZone::Cover,
        prey_in_range,
        hunger_ok: snake_state.hunger < 0.4,
        warm: on_warm_terrain && snake_state.warmth >= 0.5,
        trips_done: 0,
    }
}

fn resolve_zone_position(
    zone: SnakeZone,
    snake_pos: Position,
    prey_positions: &[Position],
    map: &TileMap,
) -> Option<Position> {
    match zone {
        SnakeZone::Cover => Some(snake_pos),
        SnakeZone::HuntingGround => prey_positions
            .iter()
            .min_by_key(|p| snake_pos.manhattan_distance(p))
            .copied(),
        SnakeZone::BaskingSpot => find_nearest_warm_tile(snake_pos, map),
        SnakeZone::MapEdge => {
            let edge_x = if snake_pos.x < map.width / 2 {
                0
            } else {
                map.width - 1
            };
            let edge_y = if snake_pos.y < map.height / 2 {
                0
            } else {
                map.height - 1
            };
            Some(Position::new(edge_x, edge_y))
        }
    }
}

fn find_nearest_warm_tile(pos: Position, map: &TileMap) -> Option<Position> {
    // Small radial search; snakes don't roam far for thermoregulation.
    let radius: i32 = 8;
    let mut best: Option<(i32, Position)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let nx = pos.x + dx;
            let ny = pos.y + dy;
            if !map.in_bounds(nx, ny) {
                continue;
            }
            if is_warm(map.get(nx, ny).terrain) {
                let d = dx.abs() + dy.abs();
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, Position::new(nx, ny)));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

// ---------------------------------------------------------------------------
// snake_resolve_goap_plans
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn snake_resolve_goap_plans(
    mut commands: Commands,
    mut snakes: Query<
        (
            Entity,
            &mut SnakeGoapPlan,
            &mut SnakeState,
            &mut Position,
            &mut SnakeAiPhase,
            &mut WildlifeAiState,
        ),
        (With<WildAnimal>, Without<Dead>),
    >,
    prey: Query<(Entity, &Position), (With<PreyAnimal>, Without<SnakeState>)>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let prey_positions: Vec<Position> = prey.iter().map(|(_, p)| *p).collect();
    let prey_entities: Vec<(Entity, Position)> = prey.iter().map(|(e, p)| (e, *p)).collect();

    for (snake_entity, mut plan, mut snake_state, mut pos, mut phase, mut ai_state) in &mut snakes {
        if plan.is_exhausted() {
            commands.entity(snake_entity).remove::<SnakeGoapPlan>();
            continue;
        }
        let Some(current_step) = plan.current().cloned() else {
            commands.entity(snake_entity).remove::<SnakeGoapPlan>();
            continue;
        };

        if let Some(step_state) = plan.current_state_mut() {
            if step_state.target_position.is_none() {
                step_state.target_position = match current_step.action {
                    SnakeGoapActionKind::SlideTo(zone) => {
                        resolve_zone_position(zone, *pos, &prey_positions, &map)
                    }
                    _ => None,
                };
            }
        }

        *phase = phase_for_action(current_step.action);
        if matches!(*phase, SnakeAiPhase::Waiting) {
            *ai_state = WildlifeAiState::Waiting;
        }

        let step_state = plan.current_state_mut().unwrap();
        let result = match current_step.action {
            SnakeGoapActionKind::SlideTo(_) => {
                let outcome = snake_steps::resolve_slide_to(&mut pos, step_state, &map);
                outcome.result
            }
            SnakeGoapActionKind::SetAmbush => {
                let outcome =
                    snake_steps::resolve_set_ambush(step_state, SNAKE_AMBUSH_SETTLE_TICKS);
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::SnakeAmbushed);
                outcome.result
            }
            SnakeGoapActionKind::Strike => {
                let outcome = snake_steps::resolve_strike(
                    &mut pos,
                    step_state,
                    &prey_entities,
                    SNAKE_STRIKE_RANGE,
                );
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::SnakeStruckPrey);
                if outcome.witness.is_some() {
                    snake_state.last_strike_tick = time.tick;
                }
                outcome.result
            }
            SnakeGoapActionKind::Bask => {
                let outcome = snake_steps::resolve_bask(step_state, SNAKE_BASK_TICKS);
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::SnakeBasked);
                if outcome.witness {
                    snake_state.warmth = 1.0;
                    snake_state.last_bask_tick = time.tick;
                }
                outcome.result
            }
            SnakeGoapActionKind::Retreat => {
                let outcome = snake_steps::resolve_retreat(&mut pos, step_state, &map);
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::SnakeRetreated);
                outcome.result
            }
        };

        match result {
            StepResult::Advance => plan.advance(),
            StepResult::Continue => {}
            StepResult::Fail(_) => {
                commands.entity(snake_entity).remove::<SnakeGoapPlan>();
            }
        }
    }
}

fn phase_for_action(action: SnakeGoapActionKind) -> SnakeAiPhase {
    match action {
        SnakeGoapActionKind::SlideTo(_) => SnakeAiPhase::Stalking {
            target_x: 0,
            target_y: 0,
        },
        SnakeGoapActionKind::SetAmbush => SnakeAiPhase::Waiting,
        SnakeGoapActionKind::Strike => SnakeAiPhase::Striking { target: None },
        SnakeGoapActionKind::Bask => SnakeAiPhase::Basking { ticks: 0 },
        SnakeGoapActionKind::Retreat => SnakeAiPhase::Fleeing { dx: 0, dy: 0 },
    }
}

// ---------------------------------------------------------------------------
// snake_lifecycle_tick
// ---------------------------------------------------------------------------

pub fn snake_lifecycle_tick(
    mut commands: Commands,
    mut snakes: Query<(Entity, &mut SnakeState), Without<Dead>>,
    mut died_w: MessageWriter<SnakeDied>,
    mut activation: Option<ResMut<SystemActivation>>,
    time: Res<TimeState>,
) {
    for (entity, mut snake_state) in &mut snakes {
        if snake_state.hunger >= 1.0 {
            snake_state.starvation_ticks += 1;
            if snake_state.starvation_ticks >= SNAKE_STARVATION_DEATH_TICKS {
                if let Some(act) = activation.as_deref_mut() {
                    act.record(Feature::SnakeDied);
                }
                died_w.write(SnakeDied {
                    snake: entity,
                    cause: WildlifeDeathCause::Starvation,
                });
                commands.entity(entity).insert(Dead {
                    tick: time.tick,
                    cause: DeathCause::Starvation,
                });
            }
        } else {
            snake_state.starvation_ticks = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_needs_tick_decays_hunger() {
        let mut app = bevy::prelude::App::new();
        app.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        app.add_systems(bevy::prelude::Update, snake_needs_tick);
        let id = app
            .world_mut()
            .spawn((SnakeState::new_adult(), Position::new(5, 5)))
            .id();
        let baseline = app.world().get::<SnakeState>(id).unwrap().hunger;
        app.update();
        let after = app.world().get::<SnakeState>(id).unwrap().hunger;
        assert!(after > baseline);
    }

    #[test]
    fn snake_needs_tick_decays_warmth_off_warm_terrain() {
        let mut app = bevy::prelude::App::new();
        // Grass is not warm; warmth should decay.
        app.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        app.add_systems(bevy::prelude::Update, snake_needs_tick);
        let id = app
            .world_mut()
            .spawn((SnakeState::new_adult(), Position::new(5, 5)))
            .id();
        let baseline = app.world().get::<SnakeState>(id).unwrap().warmth;
        app.update();
        let after = app.world().get::<SnakeState>(id).unwrap().warmth;
        assert!(after < baseline);
    }

    #[test]
    fn snake_needs_tick_holds_warmth_on_warm_terrain() {
        let mut app = bevy::prelude::App::new();
        // Rock is warm; warmth should not decay.
        app.insert_resource(TileMap::new(20, 20, Terrain::Rock));
        app.add_systems(bevy::prelude::Update, snake_needs_tick);
        let id = app
            .world_mut()
            .spawn((SnakeState::new_adult(), Position::new(5, 5)))
            .id();
        let baseline = app.world().get::<SnakeState>(id).unwrap().warmth;
        app.update();
        let after = app.world().get::<SnakeState>(id).unwrap().warmth;
        assert!((after - baseline).abs() < f32::EPSILON);
    }

    #[test]
    fn phase_for_action_maps_each_variant() {
        let _ = phase_for_action(SnakeGoapActionKind::SlideTo(SnakeZone::Cover));
        let _ = phase_for_action(SnakeGoapActionKind::SetAmbush);
        let _ = phase_for_action(SnakeGoapActionKind::Strike);
        let _ = phase_for_action(SnakeGoapActionKind::Bask);
        let _ = phase_for_action(SnakeGoapActionKind::Retreat);
    }
}
