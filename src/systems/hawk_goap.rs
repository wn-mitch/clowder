//! Hawk GOAP systems — evaluate, plan, and resolve hawk actions.
//!
//! Ticket 025 Phase 2 mirror of `fox_goap.rs` for hawks. Each tick:
//! 1. `hawk_needs_tick` decays hunger and ages the hawk.
//! 2. `sync_hawk_needs` copies `HawkState` + `Health` into `HawkNeeds`.
//! 3. `hawk_evaluate_and_plan` scores dispositions, softmaxes a choice,
//!    builds an A* plan, and inserts `HawkGoapPlan`.
//! 4. `hawk_resolve_goap_plans` dispatches the current step to a
//!    resolver under `src/steps/hawk/` and applies the witness Feature.
//! 5. `hawk_lifecycle_tick` writes `HawkDied` on starvation.
//!
//! Hawks lack the fox's territory / breeding tiers, so the scoring
//! context is much simpler. Commit 5 lifts the tuning values into
//! `HawkEcologyConstants`; this commit hardcodes them.

use bevy_ecs::prelude::*;

use crate::ai::eval::{DseRegistry, ModifierPipeline};
use crate::ai::hawk_planner::actions::actions_for_disposition;
use crate::ai::hawk_planner::goals::goal_for_disposition;
use crate::ai::hawk_planner::{HawkDomain, HawkGoapActionKind, HawkPlannerState, HawkZone};
use crate::ai::hawk_scoring::{
    score_hawk_dispositions, select_hawk_disposition_softmax, HawkNeeds, HawkPersonality,
    HawkScoringContext,
};
use crate::ai::planner::core::make_plan;
use crate::ai::scoring::{EvalInputs, MarkerSnapshot};
use crate::components::hawk_goap_plan::HawkGoapPlan;
use crate::components::physical::{Dead, DeathCause, Health, Position};
use crate::components::prey::PreyAnimal;
use crate::components::wildlife::{
    HawkAiPhase, HawkDied, HawkState, WildAnimal, WildlifeAiState, WildlifeDeathCause,
};
use crate::resources::map::TileMap;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{TimeScale, TimeState};
use crate::steps::{hawk as hawk_steps, StepResult};

// ---------------------------------------------------------------------------
// hawk_needs_tick — per-tick hunger decay + age + cooldown
// ---------------------------------------------------------------------------

/// Per-tick hawk-state maintenance: hunger decay, satiation countdown,
/// post-action cooldown, age. Mirrors `fox_needs_tick` (`wildlife.rs`).
pub fn hawk_needs_tick(
    mut hawks: Query<&mut HawkState>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let hc = &constants.hawk_ecology;
    let hunger_per_tick = hc.hunger_decay_rate.per_tick(&time_scale);
    for mut hawk in &mut hawks {
        hawk.age_ticks += 1;
        if hawk.satiation_ticks > 0 {
            hawk.satiation_ticks -= 1;
        } else {
            hawk.hunger = (hawk.hunger + hunger_per_tick).min(1.0);
        }
        hawk.post_action_cooldown = hawk.post_action_cooldown.saturating_sub(1);
    }
}

// ---------------------------------------------------------------------------
// sync_hawk_needs — bridge HawkState/Health → HawkNeeds
// ---------------------------------------------------------------------------

/// Populate [`HawkNeeds`] from the hawk's `HawkState` + `Health`. Runs
/// before scoring so the L2 evaluator reads fresh values.
pub fn sync_hawk_needs(mut hawks: Query<(&HawkState, &Health, &mut HawkNeeds), With<WildAnimal>>) {
    for (hawk_state, health, mut needs) in &mut hawks {
        // `HawkState::hunger` is `0.0 = full, 1.0 = starving`; the L2
        // evaluator reads `HawkNeeds::hunger` with inverse semantics.
        needs.hunger = (1.0 - hawk_state.hunger).clamp(0.0, 1.0);
        needs.health_fraction = (health.current / health.max).clamp(0.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// hawk_evaluate_and_plan — insert HawkGoapPlan for planless hawks
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn hawk_evaluate_and_plan(
    mut commands: Commands,
    hawks: Query<
        (
            Entity,
            &HawkState,
            &Position,
            &HawkNeeds,
            &HawkPersonality,
            Option<&crate::components::beliefs::CatBeliefs>,
        ),
        (With<WildAnimal>, Without<HawkGoapPlan>, Without<Dead>),
    >,
    map: Res<TileMap>,
    cats: Query<
        (Entity, &Position),
        (
            Without<WildAnimal>,
            Without<HawkState>,
            With<Health>,
            Without<Dead>,
        ),
    >,
    prey: Query<(Entity, &Position), (With<PreyAnimal>, Without<HawkState>)>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    dse_registry: Res<DseRegistry>,
    modifier_pipeline: Res<ModifierPipeline>,
    constants: Res<SimConstants>,
    // Ticket 427 Step 3 — per-system A* arena. Local (not Resource) so
    // hawk's planning system doesn't serialize with fox/snake — their
    // parallel-chain scheduling stays intact.
    mut planner_scratch: bevy_ecs::prelude::Local<
        crate::ai::planner::core::PlannerScratch<HawkDomain>,
    >,
) {
    let hc = &constants.hawk_ecology;
    let cat_snapshot: Vec<(Entity, Position)> = cats.iter().map(|(e, p)| (e, *p)).collect();
    let prey_snapshot: Vec<(Entity, Position)> = prey.iter().map(|(e, p)| (e, *p)).collect();
    let prey_positions: Vec<Position> = prey_snapshot.iter().map(|(_, p)| *p).collect();
    let markers = MarkerSnapshot::new();
    // 265: HawkHunting's conditional affordance axis ships dormant at
    // weight 0.0, so the scalar is never read. The live
    // `Res<ActionAffordances>` borrow is deferred to the step-21
    // activation commit (same deferral as 264's caretake pre-check in
    // goap.rs) — taking it here would add an unordered conflict edge
    // against `affordance_writer`'s `ResMut`.
    let affordances_dormant = crate::resources::ActionAffordances::default();

    for (hawk_entity, hawk_state, hawk_pos, needs, personality, cat_beliefs) in &hawks {
        let _ = hawk_state; // reserved for §L2.10.7 anchors when wired
        let cats_nearby = cat_snapshot
            .iter()
            .filter(|(_, p)| p.distance_to(hawk_pos) <= hc.cat_avoidance_range)
            .count();
        // 265: the hawk's own belief about how dangerous the cats in
        // avoidance range are — read by HawkFleeing's conditional
        // `perceived_cat_threat` axis (dormant at 0.0).
        let perceived_cat_threat = crate::components::beliefs::max_perceived_violence(
            cat_beliefs,
            cat_snapshot
                .iter()
                .filter(|(_, p)| p.distance_to(hawk_pos) <= hc.cat_avoidance_range)
                .map(|(e, _)| *e),
        );
        let prey_nearby = prey_positions
            .iter()
            .any(|p| p.distance_to(hawk_pos) <= hc.detection_range);
        // 265: best predation opportunity over prey in detection range —
        // read by HawkHunting's conditional axis (dormant at 0.0).
        let best_prey_predation_affordance = crate::resources::best_affordance_over_targets(
            &affordances_dormant,
            hawk_entity,
            prey_snapshot
                .iter()
                .filter(|(_, p)| p.distance_to(hawk_pos) <= hc.detection_range)
                .map(|(e, _)| *e),
            &[
                crate::resources::ActionKind::Dive,
                crate::resources::ActionKind::Chase,
            ],
        );

        let ctx = HawkScoringContext {
            needs,
            personality,
            prey_nearby,
            cats_nearby,
            best_prey_predation_affordance,
            perceived_cat_threat,
            self_position: *hawk_pos,
            jitter_range: 0.05,
        };

        let inputs = EvalInputs {
            cat: hawk_entity,
            position: *hawk_pos,
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

        let scoring_result = score_hawk_dispositions(&ctx, &inputs, &mut rng.rng);
        let Some(chosen) =
            select_hawk_disposition_softmax(&scoring_result, &mut rng.rng, hc.softmax_temperature)
        else {
            continue;
        };

        let planner_state =
            build_planner_state(hawk_state, hawk_pos, &prey_positions, hc.detection_range);
        let actions = actions_for_disposition(chosen);
        let goal = goal_for_disposition(chosen);

        let Some(steps) =
            make_plan::<HawkDomain>(planner_state, &actions, &goal, 8, 500, &mut planner_scratch)
        else {
            continue;
        };

        let _ = map; // reserved for zone-resolution at plan-build time
        let plan = HawkGoapPlan::new(chosen, time.tick, steps);
        commands.entity(hawk_entity).insert(plan);
    }
}

fn build_planner_state(
    hawk_state: &HawkState,
    hawk_pos: &Position,
    prey_positions: &[Position],
    detection_range: f32,
) -> HawkPlannerState {
    let prey_visible = prey_positions
        .iter()
        .any(|p| p.distance_to(hawk_pos) <= detection_range);
    HawkPlannerState {
        zone: HawkZone::Sky,
        prey_spotted: prey_visible,
        hunger_ok: hawk_state.hunger < 0.4,
        trips_done: 0,
    }
}

fn resolve_zone_position(
    zone: HawkZone,
    hawk_pos: Position,
    prey_positions: &[Position],
    map: &TileMap,
) -> Option<Position> {
    match zone {
        HawkZone::Sky => Some(Position::new(map.width / 2, map.height / 2)),
        HawkZone::HuntingGround => prey_positions
            .iter()
            .min_by_key(|p| hawk_pos.tile_distance_squared(p))
            .copied(),
        HawkZone::Perch => Some(Position::new(
            (hawk_pos.x() + 5).min(map.width - 1),
            (hawk_pos.y() + 5).min(map.height - 1),
        )),
        HawkZone::MapEdge => {
            let edge_x = if hawk_pos.x() < map.width / 2 {
                0
            } else {
                map.width - 1
            };
            let edge_y = if hawk_pos.y() < map.height / 2 {
                0
            } else {
                map.height - 1
            };
            Some(Position::new(edge_x, edge_y))
        }
    }
}

// ---------------------------------------------------------------------------
// hawk_resolve_goap_plans — dispatch current step
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn hawk_resolve_goap_plans(
    mut commands: Commands,
    mut hawks: Query<
        (
            Entity,
            &mut HawkGoapPlan,
            &mut HawkState,
            &Position,
            &mut crate::components::physical::DesiredVelocity,
            &mut HawkAiPhase,
            &mut WildlifeAiState,
        ),
        (With<WildAnimal>, Without<Dead>),
    >,
    prey: Query<(Entity, &Position), (With<PreyAnimal>, Without<HawkState>)>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let hc = &constants.hawk_ecology;
    let prey_positions: Vec<Position> = prey.iter().map(|(_, p)| *p).collect();
    let prey_entities: Vec<(Entity, Position)> = prey.iter().map(|(e, p)| (e, *p)).collect();

    for (hawk_entity, mut plan, mut hawk_state, pos, mut desired, mut phase, mut ai_state) in
        &mut hawks
    {
        if plan.is_exhausted() {
            commands.entity(hawk_entity).remove::<HawkGoapPlan>();
            continue;
        }
        let Some(current_step) = plan.current().cloned() else {
            commands.entity(hawk_entity).remove::<HawkGoapPlan>();
            continue;
        };

        // Lazy target resolution on first tick of each step.
        if let Some(step_state) = plan.current_state_mut() {
            if step_state.target_position.is_none() {
                step_state.target_position = match current_step.action {
                    HawkGoapActionKind::SoarTo(zone) => {
                        resolve_zone_position(zone, *pos, &prey_positions, &map)
                    }
                    _ => None,
                };
            }
        }

        // Set phase + WildlifeAiState mirrors so rendering follows.
        *phase = phase_for_action(current_step.action);
        if let HawkAiPhase::Soaring {
            center_x,
            center_y,
            angle,
        } = *phase
        {
            *ai_state = WildlifeAiState::Circling {
                center_x,
                center_y,
                angle,
            };
        }

        let step_state = plan.current_state_mut().unwrap();
        let result = match current_step.action {
            HawkGoapActionKind::SoarTo(_) => {
                let outcome = hawk_steps::resolve_soar_to(
                    pos,
                    &mut desired,
                    constants.movement.hawk_max_speed,
                    step_state,
                    &map,
                );
                outcome.result
            }
            HawkGoapActionKind::SpotPrey => {
                let outcome = hawk_steps::resolve_spot_prey(
                    pos,
                    &prey_positions,
                    step_state,
                    hc.detection_range,
                );
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::HawkSpottedPrey);
                outcome.result
            }
            HawkGoapActionKind::DiveAttack => {
                let outcome = hawk_steps::resolve_dive_attack(
                    pos,
                    &mut desired,
                    constants.movement.hawk_max_speed,
                    step_state,
                    &prey_entities,
                    hc.dive_range,
                );
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::HawkDiveLanded);
                if outcome.witness.is_some() {
                    // Note: kill-attribution lives in `predator_hunt_prey`.
                    hawk_state.last_dive_tick = time.tick;
                }
                outcome.result
            }
            HawkGoapActionKind::Rest => {
                let outcome = hawk_steps::resolve_rest(step_state, hc.rest_duration_ticks);
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::HawkPerched);
                if outcome.witness {
                    hawk_state.last_perch_tick = time.tick;
                }
                outcome.result
            }
            HawkGoapActionKind::FleeSky => {
                let outcome = hawk_steps::resolve_flee_sky(
                    pos,
                    &mut desired,
                    constants.movement.hawk_max_speed,
                    step_state,
                    &map,
                );
                outcome.record_if_witnessed(activation.as_deref_mut(), Feature::HawkFled);
                outcome.result
            }
        };

        match result {
            StepResult::Advance => plan.advance(),
            StepResult::Continue => {}
            StepResult::Fail(_) => {
                // Drop the plan so the evaluator builds a fresh one next tick.
                commands.entity(hawk_entity).remove::<HawkGoapPlan>();
            }
        }
    }
}

fn phase_for_action(action: HawkGoapActionKind) -> HawkAiPhase {
    match action {
        HawkGoapActionKind::SoarTo(_) => HawkAiPhase::Soaring {
            center_x: 0,
            center_y: 0,
            angle: 0.0,
        },
        HawkGoapActionKind::SpotPrey | HawkGoapActionKind::DiveAttack => {
            HawkAiPhase::HuntingPrey { target: None }
        }
        HawkGoapActionKind::Rest => HawkAiPhase::Perched { ticks: 0 },
        HawkGoapActionKind::FleeSky => HawkAiPhase::Fleeing { dx: 0, dy: 0 },
    }
}

// ---------------------------------------------------------------------------
// hawk_lifecycle_tick — starvation death + HawkDied message
// ---------------------------------------------------------------------------

pub fn hawk_lifecycle_tick(
    mut commands: Commands,
    mut hawks: Query<(Entity, &mut HawkState), Without<Dead>>,
    mut died_w: MessageWriter<HawkDied>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: Option<ResMut<SystemActivation>>,
    time: Res<TimeState>,
) {
    let starvation_death_ticks = constants
        .hawk_ecology
        .starvation_death_duration
        .ticks(&time_scale);
    for (entity, mut hawk_state) in &mut hawks {
        if hawk_state.hunger >= 1.0 {
            hawk_state.starvation_ticks += 1;
            if hawk_state.starvation_ticks >= starvation_death_ticks {
                if let Some(act) = activation.as_deref_mut() {
                    act.record(Feature::HawkDied);
                }
                died_w.write(HawkDied {
                    hawk: entity,
                    cause: WildlifeDeathCause::Starvation,
                });
                commands.entity(entity).insert(Dead {
                    tick: time.tick,
                    cause: DeathCause::Starvation,
                });
            }
        } else {
            hawk_state.starvation_ticks = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::time::SimConfig;

    fn new_test_app() -> bevy::prelude::App {
        let mut app = bevy::prelude::App::new();
        app.insert_resource(SimConstants::default());
        app.insert_resource(SimConfig::default());
        app.insert_resource(TimeScale::from_config(&SimConfig::default(), 16.6667));
        app.add_systems(bevy::prelude::Update, hawk_needs_tick);
        app
    }

    #[test]
    fn hawk_needs_tick_decays_hunger() {
        let mut app = new_test_app();
        let id = app.world_mut().spawn(HawkState::new_adult()).id();
        let baseline = app.world().get::<HawkState>(id).unwrap().hunger;
        app.update();
        let after = app.world().get::<HawkState>(id).unwrap().hunger;
        assert!(after > baseline);
    }

    #[test]
    fn hawk_needs_tick_advances_age() {
        let mut app = new_test_app();
        let id = app.world_mut().spawn(HawkState::new_adult()).id();
        app.update();
        let age = app.world().get::<HawkState>(id).unwrap().age_ticks;
        assert_eq!(age, 1);
    }

    #[test]
    fn phase_for_action_maps_each_variant() {
        let _ = phase_for_action(HawkGoapActionKind::SoarTo(HawkZone::Sky));
        let _ = phase_for_action(HawkGoapActionKind::SpotPrey);
        let _ = phase_for_action(HawkGoapActionKind::DiveAttack);
        let _ = phase_for_action(HawkGoapActionKind::Rest);
        let _ = phase_for_action(HawkGoapActionKind::FleeSky);
    }
}
