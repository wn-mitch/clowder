//! Fox GOAP systems — evaluate, plan, and resolve fox actions.
//!
//! This module replaces `fox_ai_decision` (the 440-line priority tree) with
//! GOAP-based decision making. Each tick:
//! 1. Foxes without a plan evaluate their needs and adopt a disposition.
//! 2. The planner generates a step sequence to satisfy the disposition.
//! 3. The resolver dispatches the current step to its handler.
//!
//! Foxes with plans only replan when:
//! - Current plan completes (all steps exhausted).
//! - Current step fails (replanning with failed action filtered out).
//! - An interrupt fires (health critical, outnumbered, den threatened).

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::eval::{DseRegistry, ModifierPipeline};
use crate::ai::fox_planner::actions::actions_for_disposition;
use crate::ai::fox_planner::goals::goal_for_disposition;
use crate::ai::fox_planner::{
    FoxDispositionKind, FoxDomain, FoxGoapActionKind, FoxPlannerState, FoxZone,
};
use crate::ai::fox_scoring::{
    score_fox_dispositions, select_fox_disposition_softmax, FoxScoringContext,
};
use crate::ai::planner::core::make_plan;
use crate::ai::scoring::EvalInputs;
use crate::components::fox_goap_plan::FoxGoapPlan;
use crate::components::fox_personality::{FoxNeeds, FoxPersonality};
use crate::components::fox_spatial::FoxHuntingBeliefs;
use crate::components::physical::{Dead, Health, Position};
use crate::components::wildlife::{
    ActiveConfrontation, ConfrontationReason, ConfrontationRole, FoxAiPhase, FoxDen, FoxLifeStage,
    FoxState, WildAnimal, WildlifeAiState,
};
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::map::TileMap;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{ScoringConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, SimConfig, TimeState};

// ---------------------------------------------------------------------------
// sync_fox_needs — derive FoxNeeds from FoxState + Health + den state
// ---------------------------------------------------------------------------

/// Populate [`FoxNeeds`] from the fox's current FoxState, Health, and den.
///
/// Runs every tick before planning/resolution so the scoring context is fresh.
/// This is the bridge between the existing fox data (hunger in FoxState, health
/// in Health, scent/cubs in FoxDen) and the Maslow-structured FoxNeeds used
/// for GOAP scoring.
pub fn sync_fox_needs(
    mut foxes: Query<(&FoxState, &Health, &mut FoxNeeds), With<WildAnimal>>,
    dens: Query<&FoxDen>,
) {
    for (fox_state, health, mut needs) in &mut foxes {
        // Level 1: Survival.
        // FoxState::hunger is "0.0 = full, 1.0 = starving". FoxNeeds::hunger uses
        // inverted semantics (1.0 = satisfied). Map accordingly.
        needs.hunger = (1.0 - fox_state.hunger).clamp(0.0, 1.0);
        needs.health_fraction = (health.current / health.max).clamp(0.0, 1.0);

        // Level 2: Territory.
        if let Some(den_entity) = fox_state.home_den {
            if let Ok(den) = dens.get(den_entity) {
                needs.territory_scent = den.scent_strength.clamp(0.0, 1.0);
            } else {
                needs.territory_scent = 0.0;
            }
        } else {
            needs.territory_scent = 0.0;
        }
        // Den security defaults to 1.0 (safe). The fox_check_interrupts system
        // will drop this when threats are detected.
        if needs.den_security == 0.0 {
            needs.den_security = 1.0; // reset each tick; interrupts override
        }

        // Level 3: Offspring.
        // Foxes without cubs have these satisfied by default (nothing to worry about).
        if let Some(den_entity) = fox_state.home_den {
            if let Ok(den) = dens.get(den_entity) {
                if den.cubs_present == 0 {
                    needs.cub_satiation = 1.0;
                    needs.cub_safety = 1.0;
                }
                // When cubs present, cub_satiation is updated by FeedCubs resolver
                // and decays in fox_lifecycle_tick. cub_safety is updated by interrupts.
            }
        } else {
            needs.cub_satiation = 1.0;
            needs.cub_safety = 1.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Context builders
// ---------------------------------------------------------------------------

/// Build a [`FoxScoringContext`] by observing world state around a fox.
///
/// Kept intentionally simple for the first pass — spatial belief integration
/// (FoxHuntingBeliefs, FoxThreatMemory) is stubbed to reasonable defaults so
/// the system works end-to-end; those integrations slot in later.
#[allow(clippy::too_many_arguments)]
fn build_scoring_context<'a>(
    needs: &'a FoxNeeds,
    personality: &'a FoxPersonality,
    scoring: &'a ScoringConstants,
    fox_state: &FoxState,
    fox_pos: Position,
    den_pos: Option<Position>,
    cubs_present_count: u32,
    cat_positions: &[Position],
    store_positions: &[Position],
    prey_positions: &[Position],
    hunting_beliefs: Option<&FoxHuntingBeliefs>,
    now: u64,
    day_phase: DayPhase,
) -> FoxScoringContext<'a> {
    let cats_nearby = cat_positions
        .iter()
        .filter(|p| p.manhattan_distance(&fox_pos) <= 6)
        .count();
    let prey_nearby = prey_positions
        .iter()
        .any(|p| p.manhattan_distance(&fox_pos) <= 9);
    let store_visible = store_positions
        .iter()
        .any(|p| p.manhattan_distance(&fox_pos) <= 12);
    let store_guarded = store_positions.iter().any(|sp| {
        cat_positions
            .iter()
            .any(|cp| cp.manhattan_distance(sp) <= 5)
    });

    // Cat threatening the den if any cat is within 5 tiles AND cubs are present.
    let cat_threatening_den = cubs_present_count > 0
        && den_pos.is_some_and(|dp| {
            cat_positions
                .iter()
                .any(|cp| cp.manhattan_distance(&dp) <= 5)
        });

    let has_cubs = cubs_present_count > 0;
    let is_dispersing_juvenile =
        fox_state.life_stage == FoxLifeStage::Juvenile && fox_state.home_den.is_none();

    let local_prey_belief = hunting_beliefs
        .map(|hb| hb.get(fox_pos.x, fox_pos.y))
        .unwrap_or(0.5);

    let ticks_since_patrol = now.saturating_sub(fox_state.last_patrol_tick);

    FoxScoringContext {
        needs,
        personality,
        prey_nearby,
        local_prey_belief,
        store_visible,
        store_guarded,
        cats_nearby,
        cat_threatening_den,
        ward_nearby: false,
        local_threat_level: 0.0,
        local_exploration_coverage: 0.0,
        has_cubs,
        cubs_hungry: has_cubs && needs.cub_satiation < 0.4,
        is_dispersing_juvenile,
        has_den: fox_state.home_den.is_some(),
        ticks_since_patrol,
        day_phase,
        self_position: fox_pos,
        scoring,
        jitter_range: 0.05,
    }
}

/// Build a [`FoxPlannerState`] snapshot for A* search.
fn build_planner_state(
    fox_state: &FoxState,
    fox_pos: Position,
    den_pos: Option<Position>,
) -> FoxPlannerState {
    let zone = if let Some(dp) = den_pos {
        if fox_pos.manhattan_distance(&dp) <= 2 {
            FoxZone::Den
        } else if fox_pos.manhattan_distance(&dp) <= 18 {
            FoxZone::TerritoryEdge
        } else {
            FoxZone::Wilds
        }
    } else {
        FoxZone::Wilds
    };

    FoxPlannerState {
        zone,
        carrying_food: false,
        prey_found: false,
        hunger_ok: fox_state.hunger < 0.4,
        cubs_fed: false,
        territory_marked: false,
        den_secured: fox_state.home_den.is_some(),
        interaction_done: false,
        trips_done: 0,
    }
}

/// Resolve an abstract [`FoxZone`] to a concrete world position.
fn resolve_zone_position(
    zone: FoxZone,
    fox_pos: Position,
    den_pos: Option<Position>,
    prey_positions: &[Position],
    store_positions: &[Position],
    map: &TileMap,
) -> Option<Position> {
    match zone {
        FoxZone::Den => den_pos,
        FoxZone::HuntingGround => prey_positions
            .iter()
            .min_by_key(|p| fox_pos.manhattan_distance(p))
            .copied(),
        FoxZone::NearColony => store_positions
            .iter()
            .min_by_key(|p| fox_pos.manhattan_distance(p))
            .copied(),
        FoxZone::TerritoryEdge => den_pos.map(|d| Position::new(d.x + 10, d.y + 10)),
        FoxZone::MapEdge => {
            // Closest map edge.
            let edge_x = if fox_pos.x < map.width / 2 {
                0
            } else {
                map.width - 1
            };
            let edge_y = if fox_pos.y < map.height / 2 {
                0
            } else {
                map.height - 1
            };
            Some(Position::new(edge_x, edge_y))
        }
        FoxZone::PreyLocation | FoxZone::Wilds => None,
    }
}

// ---------------------------------------------------------------------------
// fox_evaluate_and_plan — insert FoxGoapPlan for planless foxes
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn fox_evaluate_and_plan(
    mut commands: Commands,
    foxes: Query<
        (
            Entity,
            &FoxState,
            &Position,
            &FoxNeeds,
            &FoxPersonality,
            Option<&FoxHuntingBeliefs>,
        ),
        (With<WildAnimal>, Without<FoxGoapPlan>, Without<Dead>),
    >,
    dens: Query<(Entity, &FoxDen, &Position), Without<FoxState>>,
    cats: Query<
        &Position,
        (
            Without<WildAnimal>,
            Without<FoxState>,
            With<Health>,
            Without<Dead>,
        ),
    >,
    prey: Query<&Position, (With<crate::components::prey::PreyAnimal>, Without<FoxState>)>,
    stores: Query<
        &Position,
        (
            With<crate::components::building::Structure>,
            Without<WildAnimal>,
            Without<FoxState>,
        ),
    >,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    config: Res<SimConfig>,
    dse_registry: Res<DseRegistry>,
    modifier_pipeline: Res<ModifierPipeline>,
    mut event_log: Option<ResMut<EventLog>>,
    // Ticket 014 §4 fox spatial batch — markers authored by
    // `fox_spatial::update_*_markers`. Bundled here so `EvalCtx::has_marker`
    // sees the truthful values once the snapshot is populated below.
    fox_marker_q: Query<(
        bevy::prelude::Has<crate::components::markers::StoreVisible>,
        bevy::prelude::Has<crate::components::markers::StoreGuarded>,
        bevy::prelude::Has<crate::components::markers::CatThreateningDen>,
        bevy::prelude::Has<crate::components::markers::WardNearbyFox>,
    )>,
) {
    let cat_positions: Vec<Position> = cats.iter().copied().collect();
    let store_positions: Vec<Position> = stores.iter().copied().collect();
    let prey_positions: Vec<Position> = prey.iter().copied().collect();
    let day_phase = DayPhase::from_tick(time.tick, &config);
    let sc = &constants.scoring;
    // Ticket 014 §4 fox spatial batch — populate per-fox snapshot from
    // the authored ZSTs (StoreVisible / StoreGuarded / CatThreateningDen
    // / WardNearbyFox). The snapshot was empty before; it's wired up now
    // so `EvalCtx::has_marker` resolves truthfully when fox DSEs migrate
    // to `.require()` filters.
    let mut fox_markers = crate::ai::scoring::MarkerSnapshot::new();
    for (fox_entity, _, _, _, _, _) in &foxes {
        if let Ok((store_visible, store_guarded, cat_threatening_den, ward_nearby)) =
            fox_marker_q.get(fox_entity)
        {
            fox_markers.set_entity(
                crate::components::markers::StoreVisible::KEY,
                fox_entity,
                store_visible,
            );
            fox_markers.set_entity(
                crate::components::markers::StoreGuarded::KEY,
                fox_entity,
                store_guarded,
            );
            fox_markers.set_entity(
                crate::components::markers::CatThreateningDen::KEY,
                fox_entity,
                cat_threatening_den,
            );
            fox_markers.set_entity(
                crate::components::markers::WardNearbyFox::KEY,
                fox_entity,
                ward_nearby,
            );
        }
    }

    for (fox_entity, fox_state, fox_pos, needs, personality, hunting_beliefs) in &foxes {
        let den_info = fox_state
            .home_den
            .and_then(|e| dens.get(e).ok())
            .map(|(_, d, p)| (*p, d.cubs_present));
        let den_pos = den_info.map(|(p, _)| p);
        let cubs_present_count = den_info.map(|(_, c)| c).unwrap_or(0);

        let ctx = build_scoring_context(
            needs,
            personality,
            sc,
            fox_state,
            *fox_pos,
            den_pos,
            cubs_present_count,
            &cat_positions,
            &store_positions,
            &prey_positions,
            hunting_beliefs,
            time.tick,
            day_phase,
        );

        let inputs = EvalInputs {
            cat: fox_entity,
            position: *fox_pos,
            tick: time.tick,
            dse_registry: &dse_registry,
            modifier_pipeline: &modifier_pipeline,
            markers: &fox_markers,
            // §11 focal-cat tracing keys off cat name; foxes never
            // match, so unconditionally `None` here keeps fox-scoring
            // on the zero-cost path.
            focal_cat: None,
            focal_capture: None,
        };

        let scoring_result = score_fox_dispositions(&ctx, &inputs, &mut rng.rng);
        let Some(chosen) = select_fox_disposition_softmax(&scoring_result, &mut rng.rng, sc) else {
            continue;
        };

        let planner_state = build_planner_state(fox_state, *fox_pos, den_pos);
        let actions = actions_for_disposition(chosen);
        let goal = goal_for_disposition(chosen);

        let Some(steps) = make_plan::<FoxDomain>(planner_state, &actions, &goal, 12, 1000) else {
            continue; // no plan — try again next tick
        };

        if let Some(ref mut log) = event_log {
            log.push(
                time.tick,
                EventKind::FoxPlanCreated {
                    fox_id: fox_entity.to_bits(),
                    disposition: format!("{:?}", chosen),
                    steps: steps.iter().map(|s| format!("{:?}", s.action)).collect(),
                    hunger: needs.hunger,
                    territory_scent: needs.territory_scent,
                    cub_satiation: needs.cub_satiation,
                    position: (fox_pos.x, fox_pos.y),
                    day_phase: day_phase.label().to_string(),
                },
            );
        }

        let plan = FoxGoapPlan::new(chosen, time.tick, steps);
        commands.entity(fox_entity).insert(plan);
    }
}

// ---------------------------------------------------------------------------
// fox_resolve_goap_plans — execute current step of each fox's plan
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn fox_resolve_goap_plans(
    mut commands: Commands,
    mut foxes: Query<
        (
            Entity,
            &mut FoxGoapPlan,
            &mut FoxState,
            &mut Position,
            &mut FoxAiPhase,
            &mut WildlifeAiState,
            Option<&mut FoxHuntingBeliefs>,
        ),
        (With<WildAnimal>, Without<Dead>),
    >,
    mut dens: Query<(Entity, &mut FoxDen, &Position), Without<FoxState>>,
    prey: Query<&Position, (With<crate::components::prey::PreyAnimal>, Without<FoxState>)>,
    stores: Query<
        &Position,
        (
            With<crate::components::building::Structure>,
            Without<WildAnimal>,
            Without<FoxState>,
        ),
    >,
    cats: Query<(Entity, &Position), (Without<WildAnimal>, Without<FoxState>, Without<Dead>)>,
    confrontations: Query<Entity, With<ActiveConfrontation>>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let store_positions: Vec<Position> = stores.iter().copied().collect();
    let prey_positions: Vec<Position> = prey.iter().copied().collect();
    let cat_entities: Vec<(Entity, Position)> = cats.iter().map(|(e, p)| (e, *p)).collect();
    let active_confrontations: std::collections::HashSet<Entity> = confrontations.iter().collect();

    for (fox_entity, mut plan, mut fox_state, mut pos, mut phase, mut ai_state, mut beliefs) in
        &mut foxes
    {
        if plan.is_exhausted() {
            // Plan complete — remove so evaluator builds a fresh one.
            commands.entity(fox_entity).remove::<FoxGoapPlan>();
            continue;
        }

        let den_pos = fox_state
            .home_den
            .and_then(|e| dens.get(e).ok().map(|(_, _, p)| *p));
        let den_entity = fox_state.home_den;
        let Some(current_step) = plan.current().cloned() else {
            commands.entity(fox_entity).remove::<FoxGoapPlan>();
            continue;
        };

        // Lazily resolve target position for this step on first tick.
        {
            let step_state = plan
                .current_state_mut()
                .expect("current_state_mut must exist when step exists");
            if step_state.target_position.is_none() {
                step_state.target_position = target_for_action(
                    current_step.action,
                    *pos,
                    den_pos,
                    &prey_positions,
                    &store_positions,
                    &map,
                );
            }
        }

        use crate::steps::fox as fox_steps;
        use crate::steps::StepResult;

        let result = match current_step.action {
            FoxGoapActionKind::TravelTo(_)
            | FoxGoapActionKind::ReturnToDen
            | FoxGoapActionKind::StalkPrey
            | FoxGoapActionKind::ApproachStore
            | FoxGoapActionKind::FleeArea
            | FoxGoapActionKind::PatrolBoundary
            | FoxGoapActionKind::ScoutTerritory => {
                // Record the Avoiding feature once per plan (at the first
                // PatrolBoundary tick of an Avoiding plan).
                if matches!(current_step.action, FoxGoapActionKind::PatrolBoundary)
                    && plan.kind == FoxDispositionKind::Avoiding
                    && plan.current_state().is_some_and(|s| s.ticks_elapsed == 0)
                {
                    if let Some(ref mut act) = activation {
                        act.record(Feature::FoxAvoidedCat);
                    }
                }
                // Set the matching visible phase for rendering/narrative consistency.
                *phase = phase_for_action(current_step.action);
                if let Some(target) = plan.current_state().and_then(|s| s.target_position) {
                    *ai_state = WildlifeAiState::Stalking {
                        target_x: target.x,
                        target_y: target.y,
                    };
                }
                let step_state = plan.current_state_mut().unwrap();
                fox_steps::resolve_travel_to(&mut pos, step_state, &map)
            }

            FoxGoapActionKind::SearchPrey => {
                // Simple completion: if prey is within detection range, advance.
                if prey_positions
                    .iter()
                    .any(|p| p.manhattan_distance(&pos) <= 9)
                {
                    StepResult::Advance
                } else {
                    let step_state = plan.current_state_mut().unwrap();
                    step_state.ticks_elapsed += 1;
                    if step_state.ticks_elapsed > 100 {
                        // Fruitless search — decay belief at this location.
                        if let Some(ref mut b) = beliefs {
                            b.decay(*pos, 0.05);
                        }
                        StepResult::Fail("no prey found".into())
                    } else {
                        StepResult::Continue
                    }
                }
            }

            FoxGoapActionKind::KillPrey => {
                // Killing is handled by the predator_hunt_prey system when the
                // fox is near prey with FoxAiPhase::HuntingPrey. We advance once
                // hunger has been satisfied (indicating a kill happened).
                *phase = FoxAiPhase::HuntingPrey { target: None };
                if fox_state.hunger > 0.6 {
                    // Successful kill — reinforce belief at this location.
                    if let Some(ref mut b) = beliefs {
                        b.reinforce(*pos, 0.1);
                    }
                    StepResult::Advance
                } else {
                    let step_state = plan.current_state_mut().unwrap();
                    step_state.ticks_elapsed += 1;
                    if step_state.ticks_elapsed > 100 {
                        StepResult::Fail("kill timeout".into())
                    } else {
                        StepResult::Continue
                    }
                }
            }

            FoxGoapActionKind::FeedCubs => {
                // Stamp the den so `feed_cubs_at_dens` can refresh cub hunger.
                if let (Some(dp), Some(de)) = (den_pos, den_entity) {
                    if pos.manhattan_distance(&dp) <= 2 {
                        if let Ok((_, mut den, _)) = dens.get_mut(de) {
                            den.last_fed_tick = time.tick;
                        }
                        plan.trips_done += 1;
                        StepResult::Advance
                    } else {
                        StepResult::Fail("not at den for FeedCubs".into())
                    }
                } else {
                    StepResult::Fail("no den for FeedCubs".into())
                }
            }

            FoxGoapActionKind::DepositScent => {
                *phase = FoxAiPhase::ScentMarking;
                fox_state.last_patrol_tick = time.tick;
                if let Some(ref mut act) = activation {
                    act.record(Feature::FoxScentMarked);
                }
                let step_state = plan.current_state_mut().unwrap();
                fox_steps::resolve_deposit_scent(step_state)
            }

            FoxGoapActionKind::StealFood => {
                // Hand off to existing store raid logic by setting the Raiding phase.
                *phase = FoxAiPhase::Raiding {
                    target_x: pos.x,
                    target_y: pos.y,
                };
                if fox_state.hunger > 0.6 {
                    StepResult::Advance
                } else {
                    let step_state = plan.current_state_mut().unwrap();
                    step_state.ticks_elapsed += 1;
                    if step_state.ticks_elapsed > 60 {
                        StepResult::Fail("steal timeout".into())
                    } else {
                        StepResult::Continue
                    }
                }
            }

            FoxGoapActionKind::ConfrontTarget => {
                let fox_in_confrontation = active_confrontations.contains(&fox_entity);
                let step_ticks = plan.current_state().map(|s| s.ticks_elapsed).unwrap_or(0);

                if fox_in_confrontation {
                    // Hold until resolve_paired_confrontations ends it.
                    let step_state = plan.current_state_mut().unwrap();
                    step_state.ticks_elapsed += 1;
                    StepResult::Continue
                } else if step_ticks > 0 {
                    // Confrontation ended (component was removed). Advance.
                    StepResult::Advance
                } else if let Some(dp) = den_pos {
                    // Initiate: find nearest cat within den-defense range.
                    let target = cat_entities
                        .iter()
                        .filter(|(_, cp)| cp.manhattan_distance(&dp) <= 5)
                        .min_by_key(|(_, cp)| cp.manhattan_distance(&pos))
                        .copied();
                    if let Some((cat_e, _)) = target {
                        commands.entity(fox_entity).insert(ActiveConfrontation {
                            partner: cat_e,
                            role: ConfrontationRole::Attacker,
                            reason: ConfrontationReason::DenDefense,
                            ticks_remaining: 15,
                            min_commitment: 5,
                            started_tick: time.tick,
                        });
                        commands.entity(cat_e).insert(ActiveConfrontation {
                            partner: fox_entity,
                            role: ConfrontationRole::Defender,
                            reason: ConfrontationReason::DenDefense,
                            ticks_remaining: 15,
                            min_commitment: 5,
                            started_tick: time.tick,
                        });
                        *phase = FoxAiPhase::Confronting {
                            target_id: cat_e.to_bits(),
                            ticks_remaining: 15,
                        };
                        if let Some(ref mut act) = activation {
                            act.record(Feature::FoxStandoff);
                        }
                        let step_state = plan.current_state_mut().unwrap();
                        step_state.ticks_elapsed = 1;
                        StepResult::Continue
                    } else {
                        StepResult::Fail("no confrontation target".into())
                    }
                } else {
                    StepResult::Fail("no den for ConfrontTarget".into())
                }
            }

            FoxGoapActionKind::Rest => {
                *phase = FoxAiPhase::Resting { ticks: 0 };
                let step_state = plan.current_state_mut().unwrap();
                let r = fox_steps::resolve_rest(step_state, 60);
                if matches!(r, StepResult::Advance) {
                    // Resting sates the fox somewhat.
                    fox_state.satiation_ticks = fox_state.satiation_ticks.max(200);
                }
                r
            }

            FoxGoapActionKind::GroomSelf => {
                let step_state = plan.current_state_mut().unwrap();
                fox_steps::resolve_groom_self(step_state, 20)
            }

            FoxGoapActionKind::EstablishDen => {
                // Stub — handled by fox_lifecycle_tick's existing logic until
                // full GOAP integration lands.
                StepResult::Advance
            }
        };

        // Apply the step result.
        match result {
            StepResult::Continue => {}
            StepResult::Advance => {
                plan.advance();
                if plan.is_exhausted() {
                    commands.entity(fox_entity).remove::<FoxGoapPlan>();
                }
            }
            StepResult::Fail(_reason) => {
                let failed = current_step.action;
                plan.failed_actions.insert(failed);
                commands.entity(fox_entity).remove::<FoxGoapPlan>();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// feed_cubs_at_dens — refresh cub hunger when parents have fed them recently
// ---------------------------------------------------------------------------

/// When a fox completes the `FeedCubs` step, it stamps `last_fed_tick` on
/// the den. This system propagates that to every cub at the den, resetting
/// hunger and topping up satiation.
///
/// Runs every tick. Cubs whose den was fed within the last 5 ticks get their
/// satiation bumped — the feeding window. This is the structural fix for the
/// "cubs starve before maturing" bug.
pub fn feed_cubs_at_dens(
    mut cubs: Query<&mut FoxState, With<WildAnimal>>,
    dens: Query<(Entity, &FoxDen)>,
    time: Res<TimeState>,
) {
    // Collect dens fed in the last 5 ticks.
    let feeding_dens: std::collections::HashSet<Entity> = dens
        .iter()
        .filter(|(_, d)| time.tick.saturating_sub(d.last_fed_tick) <= 5)
        .map(|(e, _)| e)
        .collect();

    if feeding_dens.is_empty() {
        return;
    }

    for mut cub in &mut cubs {
        if cub.life_stage != FoxLifeStage::Cub {
            continue;
        }
        if let Some(den_e) = cub.home_den {
            if feeding_dens.contains(&den_e) {
                // Fully satiate and extend satiation window.
                cub.hunger = 0.0;
                cub.satiation_ticks = cub.satiation_ticks.max(3000);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// resolve_paired_confrontations — shared tick-down and damage resolution
// ---------------------------------------------------------------------------

/// Iterates entities with [`ActiveConfrontation`] and resolves the standoff.
///
/// Each tick:
/// 1. If `started_tick + min_commitment > now`, do nothing (locked in).
/// 2. Roll for escalation based on `reason`. If triggered: both parties
///    take damage, record `FoxStandoffEscalated`, clear components.
/// 3. Decrement `ticks_remaining`. On reaching 0: 70% chance the fox flees
///    (FoxRetreated), 30% holds ground. Clear components either way.
///
/// Only processes one side of each pair — the `Attacker` role drives resolution
/// and propagates the outcome to the `Defender` by removing their component too.
#[allow(clippy::too_many_arguments)]
pub fn resolve_paired_confrontations(
    mut commands: Commands,
    confrontations: Query<(Entity, &ActiveConfrontation)>,
    mut healths: Query<&mut Health>,
    fox_states: Query<Entity, With<FoxState>>,
    time: Res<TimeState>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let fc = &constants.fox_ecology;

    let pairs: Vec<(Entity, ActiveConfrontation)> = confrontations
        .iter()
        .filter(|(_, c)| c.role == ConfrontationRole::Attacker)
        .map(|(e, c)| (e, c.clone()))
        .collect();

    for (attacker, conf) in pairs {
        let defender = conf.partner;

        // Still in commitment window? do nothing this tick.
        if time.tick < conf.started_tick + conf.min_commitment {
            continue;
        }

        let escalation_chance = match conf.reason {
            ConfrontationReason::DenDefense => fc.den_defense_escalation_chance,
            ConfrontationReason::DesperateAttack => fc.standoff_escalation_chance,
            ConfrontationReason::TerritoryDispute => fc.standoff_escalation_chance,
        };

        let escalate = rng.rng.random::<f32>() < escalation_chance;
        let expired = conf.ticks_remaining == 0;

        if escalate {
            // Both take damage.
            let dmg = fc.standoff_damage_on_escalation;
            if let Ok(mut h) = healths.get_mut(attacker) {
                h.current = (h.current - dmg).max(0.0);
            }
            if let Ok(mut h) = healths.get_mut(defender) {
                h.current = (h.current - dmg).max(0.0);
            }
            log.push(
                time.tick,
                "Claws flash — both fox and cat draw blood.".to_string(),
                NarrativeTier::Danger,
            );
            if let Some(ref mut act) = activation {
                act.record(Feature::FoxStandoffEscalated);
                act.record(Feature::CombatResolved);
            }
            commands.entity(attacker).remove::<ActiveConfrontation>();
            commands.entity(defender).remove::<ActiveConfrontation>();
        } else if expired {
            // Natural expiration — fox retreats most of the time.
            let retreats = rng.rng.random::<f32>() < fc.standoff_fox_retreat_chance;
            if retreats {
                // Which entity is the fox? Always the attacker in current model.
                if fox_states.get(attacker).is_ok() {
                    log.push(
                        time.tick,
                        "The fox thinks better of it and slinks away.".to_string(),
                        NarrativeTier::Action,
                    );
                    if let Some(ref mut act) = activation {
                        act.record(Feature::FoxRetreated);
                    }
                }
            } else {
                log.push(
                    time.tick,
                    "The fox stands its ground, hackles raised.".to_string(),
                    NarrativeTier::Danger,
                );
            }
            commands.entity(attacker).remove::<ActiveConfrontation>();
            commands.entity(defender).remove::<ActiveConfrontation>();
        } else {
            // Ongoing — tick down.
            commands.entity(attacker).insert(ActiveConfrontation {
                ticks_remaining: conf.ticks_remaining - 1,
                ..conf.clone()
            });
            // Defender side mirrors (lookup current defender's confrontation).
            if let Ok((_, def_conf)) = confrontations.get(defender) {
                commands.entity(defender).insert(ActiveConfrontation {
                    ticks_remaining: def_conf.ticks_remaining.saturating_sub(1),
                    ..def_conf.clone()
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers: action → phase and action → target resolution
// ---------------------------------------------------------------------------

fn phase_for_action(action: FoxGoapActionKind) -> FoxAiPhase {
    match action {
        FoxGoapActionKind::TravelTo(FoxZone::Den) | FoxGoapActionKind::ReturnToDen => {
            FoxAiPhase::Returning { x: 0, y: 0 }
        }
        FoxGoapActionKind::TravelTo(FoxZone::HuntingGround) | FoxGoapActionKind::StalkPrey => {
            FoxAiPhase::HuntingPrey { target: None }
        }
        FoxGoapActionKind::TravelTo(FoxZone::NearColony) | FoxGoapActionKind::ApproachStore => {
            FoxAiPhase::Raiding {
                target_x: 0,
                target_y: 0,
            }
        }
        FoxGoapActionKind::FleeArea | FoxGoapActionKind::TravelTo(FoxZone::MapEdge) => {
            FoxAiPhase::Fleeing { dx: 0, dy: 0 }
        }
        FoxGoapActionKind::PatrolBoundary | FoxGoapActionKind::TravelTo(FoxZone::TerritoryEdge) => {
            FoxAiPhase::PatrolTerritory { dx: 1, dy: 0 }
        }
        FoxGoapActionKind::ScoutTerritory | FoxGoapActionKind::TravelTo(FoxZone::Wilds) => {
            FoxAiPhase::Dispersing { dx: 1, dy: 0 }
        }
        _ => FoxAiPhase::PatrolTerritory { dx: 0, dy: 0 },
    }
}

fn target_for_action(
    action: FoxGoapActionKind,
    fox_pos: Position,
    den_pos: Option<Position>,
    prey_positions: &[Position],
    store_positions: &[Position],
    map: &TileMap,
) -> Option<Position> {
    match action {
        FoxGoapActionKind::TravelTo(zone) => {
            resolve_zone_position(zone, fox_pos, den_pos, prey_positions, store_positions, map)
        }
        FoxGoapActionKind::ReturnToDen => den_pos,
        FoxGoapActionKind::StalkPrey => prey_positions
            .iter()
            .min_by_key(|p| fox_pos.manhattan_distance(p))
            .copied(),
        FoxGoapActionKind::ApproachStore => store_positions
            .iter()
            .min_by_key(|p| fox_pos.manhattan_distance(p))
            .copied(),
        FoxGoapActionKind::FleeArea => {
            let edge_x = if fox_pos.x < map.width / 2 {
                0
            } else {
                map.width - 1
            };
            let edge_y = if fox_pos.y < map.height / 2 {
                0
            } else {
                map.height - 1
            };
            Some(Position::new(edge_x, edge_y))
        }
        _ => None,
    }
}
