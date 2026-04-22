use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::{find_free_adjacent, find_path, step_toward};
use crate::ai::planner::actions::actions_for_disposition;
use crate::ai::planner::goals::goal_for_disposition;
use crate::ai::planner::{
    make_plan, Carrying, GoapActionKind, PlannerState, PlannerZone, ZoneDistances,
};
use crate::ai::scoring::{
    apply_aspiration_bonuses, apply_cascading_bonuses, apply_colony_knowledge_bonuses,
    apply_directive_bonus, apply_fated_bonuses, apply_memory_bonuses, apply_preference_bonuses,
    apply_priority_bonus, enforce_survival_floor, score_actions,
    select_disposition_via_intention_softmax, ScoringContext,
};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::coordination::{ActiveDirective, Directive, DirectiveKind, DirectiveQueue};
use crate::components::disposition::{
    ActionHistory, ActionOutcome, ActionRecord, CraftingHint, DispositionKind,
};
use crate::components::goap_plan::{
    GoapPlan, PendingUrgencies, PlanEvent, PlanNarrative, StepExecutionState, UrgencyKind,
    UrgentNeed,
};
use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::items::{Item, ItemLocation};
use crate::components::magic::{Harvestable, Herb, HerbKind, Inventory, Ward};
use crate::components::mental::Memory;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, InjuryKind, Needs, Position};
use crate::components::prey::{
    DenRaided, PreyAnimal, PreyConfig, PreyDen, PreyDensity, PreyKilled, PreyState,
};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::components::wildlife::WildAnimal;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::food::FoodStores;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, VariableContext,
};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{DispositionConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, TimeState};

// ===========================================================================
// SystemParam bundles — keep system param counts under Bevy's 16-param limit
// ===========================================================================

#[derive(bevy_ecs::system::SystemParam)]
pub struct PreyHuntParams<'w, 's> {
    pub density: Res<'w, PreyDensity>,
    pub kill_writer: MessageWriter<'w, PreyKilled>,
    pub raid_writer: MessageWriter<'w, DenRaided>,
    pub exploration_map: ResMut<'w, crate::resources::ExplorationMap>,
    pub health_query: Query<'w, 's, &'static Health, With<PreyAnimal>>,
    /// Phase 2B — scent-detection grid. Cats sample
    /// `highest_nearby(pos, scent_search_radius)` to find prey-scent
    /// source tiles rather than running point-to-point
    /// `cat_smells_prey_windaware` against each prey entity.
    pub prey_scent_map: Res<'w, crate::resources::PreyScentMap>,
}

#[derive(bevy_ecs::system::SystemParam)]
pub struct NarrativeEmitter<'w> {
    pub log: ResMut<'w, crate::resources::narrative::NarrativeLog>,
    pub registry: Option<Res<'w, crate::resources::narrative_templates::TemplateRegistry>>,
    pub config: Res<'w, crate::resources::time::SimConfig>,
    pub weather: Res<'w, crate::resources::weather::WeatherState>,
    pub activation: Option<ResMut<'w, SystemActivation>>,
}

/// Bundles world-state queries for evaluate_and_plan to stay under 16 params.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct WorldStateQueries<'w, 's> {
    pub all_positions:
        Query<'w, 's, (Entity, &'static Position, Option<&'static PreyAnimal>), Without<Dead>>,
    pub wildlife: Query<'w, 's, (Entity, &'static Position), With<WildAnimal>>,
    pub building_query: Query<
        'w,
        's,
        (
            Entity,
            &'static Structure,
            &'static Position,
            Option<&'static ConstructionSite>,
            Option<&'static CropState>,
        ),
    >,
    pub herb_query: Query<'w, 's, (Entity, &'static Herb, &'static Position), With<Harvestable>>,
    pub ward_query: Query<'w, 's, (&'static Ward, &'static Position)>,
    pub directive_queue_query: Query<'w, 's, (Entity, &'static DirectiveQueue)>,
    pub active_directive_query: Query<'w, 's, &'static ActiveDirective>,
    pub skills_query: Query<'w, 's, &'static Skills, Without<Dead>>,
    pub carcass_query: Query<
        'w,
        's,
        (
            &'static crate::components::wildlife::Carcass,
            &'static Position,
        ),
    >,
    pub wildlife_ai_query:
        Query<'w, 's, &'static crate::components::wildlife::WildlifeAiState, With<WildAnimal>>,
    pub stored_items_query: Query<'w, 's, &'static crate::components::building::StoredItems>,
    pub items_query: Query<'w, 's, &'static crate::components::items::Item>,
}

/// Bundles resources for evaluate_and_plan.
#[derive(bevy_ecs::system::SystemParam)]
pub struct PlanResources<'w> {
    pub map: Res<'w, TileMap>,
    pub food: Res<'w, FoodStores>,
    pub relationships: Res<'w, Relationships>,
    pub constants: Res<'w, SimConstants>,
    pub time: Res<'w, TimeState>,
    pub colony_center: Res<'w, crate::resources::ColonyCenter>,
    pub dse_registry: Res<'w, crate::ai::eval::DseRegistry>,
    pub modifier_pipeline: Res<'w, crate::ai::eval::ModifierPipeline>,
}

/// Bundles magic resolver dependencies to keep resolve_goap_plans under 16 params.
/// The herb_query reads `&Position` (immutable), which would conflict with the
/// cats query's `&mut Position`. Disjointness is ensured by `Without<Herb>` on
/// the cats filter (herbs are never cats).
#[derive(bevy_ecs::system::SystemParam)]
pub struct MagicResolverParams<'w, 's> {
    pub herb_query: Query<
        'w,
        's,
        (
            Entity,
            &'static Herb,
            &'static crate::components::physical::Position,
        ),
        With<Harvestable>,
    >,
    pub pushback_writer: MessageWriter<'w, crate::systems::magic::CorruptionPushback>,
    pub carcass_query: Query<
        'w,
        's,
        (
            Entity,
            &'static mut crate::components::wildlife::Carcass,
            &'static crate::components::physical::Position,
        ),
    >,
    /// Lookup of ActiveDirective by entity — used by Cleanse/HarvestCarcass
    /// resolvers to route the cat to the coordinator-specified target tile.
    pub active_directive_query: Query<'w, 's, &'static ActiveDirective>,
}

/// Bundles building queries for resolve_goap_plans.
/// Disjoint with the cats query because cats have `Without<Structure>` and
/// this query accesses `&mut Structure` — Bevy proves disjointness on Structure.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct BuildingResolverParams<'w, 's> {
    pub buildings: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Structure,
            Option<&'static mut ConstructionSite>,
            Option<&'static mut CropState>,
            &'static Position,
        ),
        Without<crate::components::task_chain::TaskChain>,
    >,
    pub colony_score: Option<ResMut<'w, crate::resources::colony_score::ColonyScore>>,
}

/// Bundles resources for resolve_goap_plans.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct ExecutorContext<'w, 's> {
    pub map: ResMut<'w, TileMap>,
    pub wind: Res<'w, crate::resources::wind::WindState>,
    pub time: Res<'w, TimeState>,
    pub constants: Res<'w, SimConstants>,
    pub event_log: Option<ResMut<'w, EventLog>>,
    /// Wildlife entities with positions, for `EngageThreat` target resolution.
    /// Excludes prey animals so cats don't try to "fight" rabbits as threats.
    pub wildlife: bevy_ecs::prelude::Query<
        'w,
        's,
        (Entity, &'static Position),
        (With<WildAnimal>, Without<Dead>, Without<PreyAnimal>),
    >,
}

// ===========================================================================
// check_anxiety_interrupts — hard interrupt for CriticalHealth only;
// all other critical needs accumulate as pending urgencies evaluated at
// step boundaries in resolve_goap_plans.
// ===========================================================================

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_anxiety_interrupts(
    mut query: Query<
        (
            Entity,
            &Name,
            &Needs,
            &Personality,
            &Position,
            &Health,
            &mut CurrentAction,
            &mut PendingUrgencies,
            Option<&mut ActionHistory>,
        ),
        (With<GoapPlan>, Without<Dead>),
    >,
    plans: Query<&GoapPlan, Without<Dead>>,
    wildlife: Query<(Entity, &Position), (With<WildAnimal>, Without<Dead>, Without<PreyAnimal>)>,
    ward_query: Query<(&Ward, &Position)>,
    all_cats: Query<(Entity, &Position), (Without<Dead>, Without<WildAnimal>)>,
    building_query: Query<&Position, (With<Structure>, Without<ConstructionSite>)>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    colony_center: Res<crate::resources::ColonyCenter>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
    mut plan_writer: MessageWriter<PlanNarrative>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let d = &constants.disposition;

    // Pre-collect data to avoid query conflicts in the loop.
    let wildlife_positions: Vec<(Position, Entity)> =
        wildlife.iter().map(|(e, p)| (*p, e)).collect();
    let ward_data: Vec<(Position, f32)> = ward_query
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius()))
        .collect();
    let cat_positions: Vec<(Entity, Position)> = all_cats.iter().map(|(e, p)| (e, *p)).collect();
    let building_positions: Vec<Position> = building_query.iter().copied().collect();

    for (entity, name, needs, personality, pos, health, mut current, mut urgencies, history) in
        &mut query
    {
        let Ok(plan) = plans.get(entity) else {
            continue;
        };

        // --- Hard interrupt: CriticalHealth only ---
        // A critically injured cat that chose Resting is already recovering;
        // interrupting it creates the same oscillation we're fixing.
        if plan.kind != DispositionKind::Resting
            && health.current / health.max < d.critical_health_threshold
        {
            activation.record(Feature::AnxietyInterrupt);

            if let Some(ref mut log) = event_log {
                let current_step = plan
                    .current()
                    .map(|s| format!("{:?}", s.action))
                    .unwrap_or_else(|| "none".into());
                log.push(
                    time.tick,
                    EventKind::PlanInterrupted {
                        cat: name.0.clone(),
                        disposition: format!("{:?}", plan.kind),
                        reason: "CriticalHealth".into(),
                        current_step,
                        hunger: needs.hunger,
                        energy: needs.energy,
                        temperature: needs.temperature,
                    },
                );
            }

            if let Some(mut hist) = history {
                hist.record(ActionRecord {
                    action: current.action,
                    disposition: Some(plan.kind),
                    tick: time.tick,
                    outcome: ActionOutcome::Interrupted,
                });
            }

            plan_writer.write(PlanNarrative {
                entity,
                kind: plan.kind,
                event: PlanEvent::Abandoned,
                completions: plan.trips_done,
            });

            commands.entity(entity).remove::<GoapPlan>();
            current.ticks_remaining = 0;
            continue;
        }

        // --- Accumulate soft urgencies for step-boundary evaluation ---
        accumulate_urgencies(
            needs,
            personality,
            pos,
            plan.kind,
            &wildlife_positions,
            &ward_data,
            &cat_positions,
            &colony_center.0,
            &building_positions,
            d,
            &constants.sensory.cat,
            entity,
            &mut urgencies,
        );
    }
}

// ---------------------------------------------------------------------------
// Urgency accumulation — runs every tick, writes to PendingUrgencies
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn accumulate_urgencies(
    needs: &Needs,
    personality: &Personality,
    pos: &Position,
    kind: DispositionKind,
    wildlife_positions: &[(Position, Entity)],
    ward_data: &[(Position, f32)],
    cat_positions: &[(Entity, Position)],
    colony_center: &Position,
    building_positions: &[Position],
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    entity: Entity,
    urgencies: &mut PendingUrgencies,
) {
    urgencies.needs.clear();

    // --- Starvation (maslow 1) ---
    if !matches!(
        kind,
        DispositionKind::Resting | DispositionKind::Hunting | DispositionKind::Foraging
    ) && needs.hunger < d.starvation_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Starvation,
            maslow_level: 1,
            intensity: 1.0 - (needs.hunger / d.starvation_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }
    // Critical starvation override for Hunting/Foraging.
    if matches!(kind, DispositionKind::Hunting | DispositionKind::Foraging)
        && needs.hunger < d.critical_hunger_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Starvation,
            maslow_level: 1,
            intensity: 1.0 - (needs.hunger / d.critical_hunger_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- Exhaustion (maslow 1) ---
    if !matches!(
        kind,
        DispositionKind::Resting | DispositionKind::Hunting | DispositionKind::Foraging
    ) && needs.energy < d.exhaustion_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Exhaustion,
            maslow_level: 1,
            intensity: 1.0 - (needs.energy / d.exhaustion_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- CriticalSafety (maslow 2) ---
    if needs.safety < d.critical_safety_threshold {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::CriticalSafety,
            maslow_level: 2,
            intensity: 1.0 - (needs.safety / d.critical_safety_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- ThreatNearby (maslow 2, contextual) ---
    if !matches!(kind, DispositionKind::Guarding) {
        if let Some(threat) = evaluate_threat_context(
            pos,
            personality,
            wildlife_positions,
            ward_data,
            cat_positions,
            colony_center,
            building_positions,
            d,
            cat_profile,
            entity,
        ) {
            urgencies.needs.push(threat);
        }
    }
}

// ---------------------------------------------------------------------------
// Contextual threat evaluation — the "zoo vs bush" formula
// ---------------------------------------------------------------------------

/// Evaluates whether a nearby threat warrants an urgency, considering the cat's
/// full environmental context. A cat at the stores with wards and allies barely
/// reacts. A cat alone in the wilderness drops everything.
#[allow(clippy::too_many_arguments)]
fn evaluate_threat_context(
    pos: &Position,
    personality: &Personality,
    wildlife_positions: &[(Position, Entity)],
    ward_data: &[(Position, f32)],
    cat_positions: &[(Entity, Position)],
    colony_center: &Position,
    building_positions: &[Position],
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    entity: Entity,
) -> Option<UrgentNeed> {
    // Phase 2 migration: the visual-only detection path now flows
    // through the sensory model's sight channel. See `cat_sees_threat_at`.
    let nearest = wildlife_positions
        .iter()
        .filter(|(wp, _)| crate::systems::sensing::cat_sees_threat_at(*pos, cat_profile, *wp))
        .min_by_key(|(wp, _)| pos.manhattan_distance(wp));

    let (threat_pos, _) = nearest?;
    let dist = pos.manhattan_distance(threat_pos) as f32;

    // Base urgency: inverse distance.
    let base_urgency = (1.0 - dist / d.threat_urgency_divisor).max(0.0);
    if base_urgency <= 0.0 {
        return None;
    }

    // Ward protection: inside a ward's repel radius dampens threat.
    let within_ward = ward_data
        .iter()
        .any(|(wp, radius)| (pos.manhattan_distance(wp) as f32) < *radius);
    let ward_factor = if within_ward {
        d.threat_ward_dampening
    } else {
        1.0
    };

    // Colony proximity: near buildings or colony center dampens threat.
    let near_buildings = building_positions
        .iter()
        .any(|bp| pos.manhattan_distance(bp) <= d.threat_building_safety_range);
    let colony_factor = if near_buildings {
        d.threat_colony_building_dampening
    } else {
        let colony_dist = pos.manhattan_distance(colony_center) as f32;
        let normalized = (colony_dist / d.threat_colony_radius).min(1.0);
        d.threat_colony_center_dampening + (1.0 - d.threat_colony_center_dampening) * normalized
    };

    // Allies: each nearby cat reduces perceived threat (diminishing returns).
    let ally_count = cat_positions
        .iter()
        .filter(|(e, cp)| *e != entity && pos.manhattan_distance(cp) <= d.threat_ally_range)
        .count()
        .min(d.allies_fighting_cap);
    let ally_factor = 1.0 / (1.0 + ally_count as f32 * d.threat_ally_dampening_per_cat);

    // Boldness: bold cats feel less threatened.
    let boldness_factor = 1.0 - personality.boldness * d.flee_threshold_boldness_scale;

    let intensity = base_urgency * ward_factor * colony_factor * ally_factor * boldness_factor;

    if intensity > d.flee_threshold_base {
        Some(UrgentNeed {
            kind: UrgencyKind::ThreatNearby,
            maslow_level: 2,
            intensity,
            threat_pos: Some(*threat_pos),
        })
    } else {
        None
    }
}

// ===========================================================================
// evaluate_and_plan — scores dispositions, invokes planner, inserts GoapPlan
// ===========================================================================

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn evaluate_and_plan(
    mut query: Query<
        (
            Entity,
            &Name,
            &Needs,
            &Personality,
            &Position,
            &Memory,
            &Skills,
            &Health,
            &MagicAffinity,
            &Inventory,
            &mut CurrentAction,
            Option<&crate::components::aspirations::Aspirations>,
            Option<&crate::components::aspirations::Preferences>,
            Option<&crate::components::fate::FatedLove>,
            Option<&crate::components::fate::FatedRival>,
        ),
        (Without<Dead>, Without<GoapPlan>),
    >,
    world_state: WorldStateQueries,
    res: PlanResources,
    mating_fitness_params: crate::ai::mating::MatingFitnessParams,
    colony: super::ColonyContext<'_>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
    mut plan_writer: MessageWriter<PlanNarrative>,
    mut event_log: Option<ResMut<EventLog>>,
    mut unmet_demand: ResMut<crate::resources::UnmetDemand>,
) {
    let sc = &res.constants.scoring;
    let d = &res.constants.disposition;
    let food_available = !res.food.is_empty();
    let food_fraction = res.food.fraction();

    // §4 marker snapshot (Phase 4b.2). Populated once at system start
    // from Resources and Queries, then passed by reference through
    // `EvalInputs` so `EligibilityFilter::require(marker)` rows resolve
    // without each DSE carrying its own query bundle. Phase 4b.2 MVP
    // ships the colony-scoped `HasStoredFood` reference row; per-cat
    // markers extend this as authoring systems land per §4.6.
    let mut markers = crate::ai::scoring::MarkerSnapshot::new();
    markers.set_colony("HasStoredFood", food_available);

    let mut cat_positions: Vec<(Entity, Position)> = Vec::new();
    let mut prey_positions: Vec<Position> = Vec::new();
    for (e, p, prey) in world_state.all_positions.iter() {
        cat_positions.push((e, *p));
        if prey.is_some() {
            prey_positions.push(*p);
        }
    }

    let wildlife_positions: Vec<(Entity, Position)> =
        world_state.wildlife.iter().map(|(e, p)| (e, *p)).collect();

    let has_construction_site = world_state
        .building_query
        .iter()
        .any(|(_, _, _, site, _)| site.is_some());
    let has_damaged_building = world_state
        .building_query
        .iter()
        .any(|(_, s, _, site, _)| site.is_none() && s.condition < d.damaged_building_threshold);
    let has_garden = world_state
        .building_query
        .iter()
        .any(|(_, s, _, site, _)| s.kind == StructureType::Garden && site.is_none());
    let has_functional_kitchen = world_state.building_query.iter().any(|(_, s, _, site, _)| {
        s.kind == StructureType::Kitchen && site.is_none() && s.effectiveness() > 0.0
    });
    let has_raw_food_in_stores = world_state.stored_items_query.iter().any(|stored| {
        stored.items.iter().copied().any(|e| {
            world_state
                .items_query
                .get(e)
                .is_ok_and(|it| it.kind.is_food() && !it.modifiers.cooked)
        })
    });

    let herb_positions: Vec<(Entity, Position, HerbKind)> = world_state
        .herb_query
        .iter()
        .map(|(e, herb, p)| (e, *p, herb.kind))
        .collect();

    let ward_strength_low = {
        let ward_count = world_state.ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 = world_state
                .ward_query
                .iter()
                .map(|(w, _)| w.strength)
                .sum::<f32>()
                / ward_count as f32;
            avg < d.ward_strength_low_threshold
        }
    };

    // Snapshot actionable carcasses for scoring.
    let carcass_positions: Vec<Position> = world_state
        .carcass_query
        .iter()
        .filter(|(c, _)| !c.cleansed || !c.harvested)
        .map(|(_, p)| *p)
        .collect();

    // Territory corruption — max corruption in the ring around colony center.
    let territory_max_corruption = {
        let mc = &res.constants.magic;
        let inner = mc.territory_corruption_inner_radius;
        let outer = mc.territory_corruption_outer_radius;
        let cx = res.colony_center.0.x;
        let cy = res.colony_center.0.y;
        let mut max_c = 0.0f32;
        for y in (cy - outer)..=(cy + outer) {
            for x in (cx - outer)..=(cx + outer) {
                if !res.map.in_bounds(x, y) {
                    continue;
                }
                let dist = (x - cx).abs() + (y - cy).abs();
                if dist >= inner && dist <= outer {
                    max_c = max_c.max(res.map.get(x, y).corruption);
                }
            }
        }
        max_c
    };

    // Detect if any shadow fox is actively sieging a ward.
    let wards_under_siege = world_state.wildlife_ai_query.iter().any(|s| {
        matches!(
            s,
            crate::components::wildlife::WildlifeAiState::EncirclingWard { .. }
        )
    });

    let colony_injury_count = query
        .iter()
        .filter(|(_, _, _, _, _, _, _, health, _, _, _, _, _, _, _)| health.current < 1.0)
        .count();

    let directive_snapshot: HashMap<Entity, (usize, Option<Directive>)> = world_state
        .directive_queue_query
        .iter()
        .map(|(entity, q)| (entity, (q.directives.len(), q.directives.first().cloned())))
        .collect();

    let action_snapshot: Vec<(Entity, Position, Action)> = query
        .iter()
        .map(
            |(entity, _, _, _, pos, _, _, _, _, _, current, _, _, _, _)| {
                (entity, *pos, current.action)
            },
        )
        .collect();

    let has_mentoring_target_fn = |entity: Entity, pos: &Position, skills: &Skills| -> bool {
        let mentor_skills = [
            skills.hunting,
            skills.foraging,
            skills.herbcraft,
            skills.building,
            skills.combat,
            skills.magic,
        ];
        if !mentor_skills
            .iter()
            .any(|&s| s > d.mentor_skill_threshold_high)
        {
            return false;
        }
        cat_positions.iter().any(|(other, other_pos)| {
            *other != entity
                && crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    *other_pos,
                    crate::components::SensorySignature::CAT,
                    d.mentoring_detection_range as f32,
                )
                && world_state
                    .skills_query
                    .get(*other)
                    .is_ok_and(|other_skills| {
                        let other_arr = [
                            other_skills.hunting,
                            other_skills.foraging,
                            other_skills.herbcraft,
                            other_skills.building,
                            other_skills.combat,
                            other_skills.magic,
                        ];
                        mentor_skills.iter().zip(other_arr.iter()).any(|(&m, &a)| {
                            m > d.mentor_skill_threshold_high && a < d.mentor_skill_threshold_low
                        })
                    })
        })
    };

    // Pre-compute stores positions for zone distance calculations.
    let stores_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, _, _)| s.kind == StructureType::Stores)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    // Pre-compute kitchen positions (completed only) for zone distance.
    let kitchen_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::Kitchen && site.is_none())
        .map(|(_, _, p, _, _)| *p)
        .collect();

    // Snapshot per-cat fields needed by the mating eligibility gate.
    let mating_fitness = mating_fitness_params.snapshot();
    let current_season = mating_fitness_params.current_season();
    let current_day_phase = mating_fitness_params.current_day_phase();

    for (
        entity,
        name,
        needs,
        personality,
        pos,
        memory,
        skills,
        health,
        magic_aff,
        inventory,
        mut current,
        aspirations,
        preferences,
        fated_love,
        fated_rival,
    ) in &mut query
    {
        if current.ticks_remaining != 0 {
            continue;
        }

        let can_hunt = has_nearby_tile(pos, &res.map, d.hunt_terrain_search_radius, |t| {
            matches!(t, Terrain::DenseForest | Terrain::LightForest)
        });
        let can_forage = has_nearby_tile(pos, &res.map, d.forage_terrain_search_radius, |t| {
            t.foraging_yield() > 0.0
        });

        let has_social_target = cat_positions.iter().any(|(other, other_pos)| {
            *other != entity && pos.manhattan_distance(other_pos) <= d.social_target_range
        });

        let nearest_threat = wildlife_positions
            .iter()
            .filter(|(_, wp)| pos.manhattan_distance(wp) <= d.wildlife_threat_range)
            .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

        let has_threat_nearby = nearest_threat.is_some();
        let allies_fighting_threat = if let Some(&(_, threat_pos)) = nearest_threat {
            action_snapshot
                .iter()
                .filter(|(e, ally_pos, action)| {
                    *e != entity
                        && *action == Action::Fight
                        && ally_pos.manhattan_distance(&threat_pos) <= d.allies_fighting_range
                })
                .count()
                .min(d.allies_fighting_cap)
        } else {
            0
        };

        let combat_effective =
            skills.combat + skills.hunting * d.combat_effective_hunting_cross_train;
        let is_incapacitated = health
            .injuries
            .iter()
            .any(|inj| inj.kind == InjuryKind::Severe && !inj.healed);

        let has_herbs_nearby = herb_positions.iter().any(|(_, hp, _)| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                &res.constants.sensory.cat,
                *hp,
                crate::components::SensorySignature::PREY,
                d.herb_detection_range as f32,
            )
        });

        let prey_nearby = prey_positions.iter().any(|pp| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                &res.constants.sensory.cat,
                *pp,
                crate::components::SensorySignature::PREY,
                d.prey_detection_range as f32,
            )
        });

        let nearby_carcass_count = carcass_positions
            .iter()
            .filter(|cp| {
                crate::systems::sensing::observer_smells_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    **cp,
                    crate::components::SensorySignature::CARCASS,
                    sc.carcass_detection_range as f32,
                )
            })
            .count();

        let (on_corrupted_tile, tile_corruption, on_special_terrain) =
            if res.map.in_bounds(pos.x, pos.y) {
                let tile = res.map.get(pos.x, pos.y);
                (
                    tile.corruption > d.corrupted_tile_threshold,
                    tile.corruption,
                    matches!(tile.terrain, Terrain::FairyRing | Terrain::StandingStone),
                )
            } else {
                (false, 0.0, false)
            };

        // "Smell the rot": sample the map within corruption_smell_range tiles
        // and take the max. This lets cats proactively react to corruption
        // before they're standing on it.
        let nearby_corruption_level = {
            let r = sc.corruption_smell_range;
            let mut max_c: f32 = 0.0;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() > r {
                        continue; // Manhattan radius
                    }
                    let nx = pos.x + dx;
                    let ny = pos.y + dy;
                    if res.map.in_bounds(nx, ny) {
                        let c = res.map.get(nx, ny).corruption;
                        if c > max_c {
                            max_c = c;
                        }
                    }
                }
            }
            max_c
        };

        let has_eligible_mate = crate::ai::mating::has_eligible_mate(
            entity,
            needs.mating,
            current_season,
            sc,
            &mating_fitness,
            &cat_positions,
            &res.relationships,
        );

        let ctx = ScoringContext {
            scoring: sc,
            needs,
            personality,
            food_available,
            can_hunt,
            can_forage,
            has_social_target,
            has_threat_nearby,
            allies_fighting_threat,
            combat_effective,
            health: health.current,
            is_incapacitated,
            has_construction_site,
            has_damaged_building,
            has_garden,
            food_fraction,
            magic_affinity: magic_aff.0,
            magic_skill: skills.magic,
            herbcraft_skill: skills.herbcraft,
            has_herbs_nearby,
            has_herbs_in_inventory: inventory
                .slots
                .iter()
                .any(|s| matches!(s, crate::components::magic::ItemSlot::Herb(_))),
            has_remedy_herbs: inventory.has_remedy_herb(),
            has_ward_herbs: inventory.has_ward_herb(),
            thornbriar_available: herb_positions
                .iter()
                .any(|(_, _, kind)| *kind == HerbKind::Thornbriar),
            colony_injury_count,
            ward_strength_low,
            on_corrupted_tile,
            tile_corruption,
            nearby_corruption_level,
            on_special_terrain,
            is_coordinator_with_directives: directive_snapshot
                .get(&entity)
                .is_some_and(|(len, _)| *len > 0),
            pending_directive_count: directive_snapshot.get(&entity).map_or(0, |(len, _)| *len),
            has_mentoring_target: has_mentoring_target_fn(entity, pos, skills),
            prey_nearby,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: colony.exploration_map.unexplored_fraction_nearby(
                pos.x,
                pos.y,
                d.explore_range,
                0.5,
            ),
            fox_scent_level: colony.fox_scent_map.get(pos.x, pos.y),
            carcass_nearby: nearby_carcass_count > 0,
            nearby_carcass_count,
            territory_max_corruption,
            wards_under_siege,
            day_phase: current_day_phase,
            has_functional_kitchen,
            has_raw_food_in_stores,
        };

        let eval_inputs = crate::ai::scoring::EvalInputs {
            cat: entity,
            position: *pos,
            tick: res.time.tick,
            dse_registry: &res.dse_registry,
            modifier_pipeline: &res.modifier_pipeline,
            markers: &markers,
        };
        let result = score_actions(&ctx, &eval_inputs, &mut rng.rng);
        // Record latent Cook desire so the coordinator's BuildPressure
        // channel for Kitchen rises when enough cats want to cook but
        // no Kitchen exists.
        if result.wants_cook_but_no_kitchen {
            unmet_demand
                .record(crate::components::building::StructureType::Kitchen);
        }
        let mut scores = result.scores;

        // Apply all bonus layers.
        apply_memory_bonuses(&mut scores, memory, pos, sc);
        if let Some(ref ck) = colony.knowledge {
            apply_colony_knowledge_bonuses(&mut scores, ck, pos, sc);
        }
        if let Some(ref cp) = colony.priority {
            apply_priority_bonus(&mut scores, cp.active, sc);
        }
        let mut nearby_actions = HashMap::new();
        for &(other_entity, other_pos, other_action) in &action_snapshot {
            if other_entity != entity
                && pos.manhattan_distance(&other_pos) <= d.cascading_bonus_range
            {
                *nearby_actions.entry(other_action).or_insert(0usize) += 1;
            }
        }
        apply_cascading_bonuses(&mut scores, &nearby_actions, sc);
        if let Some(asp) = aspirations {
            apply_aspiration_bonuses(&mut scores, asp, sc);
        }
        if let Some(pref) = preferences {
            apply_preference_bonuses(&mut scores, pref, sc);
        }
        let love_visible = fated_love
            .filter(|l| l.awakened)
            .and_then(|l| cat_positions.iter().find(|(e, _)| *e == l.partner))
            .is_some_and(|(_, pp)| {
                crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    *pp,
                    crate::components::SensorySignature::CAT,
                    d.fated_love_detection_range as f32,
                )
            });
        let rival_nearby = fated_rival
            .filter(|r| r.awakened)
            .and_then(|r| cat_positions.iter().find(|(e, _)| *e == r.rival))
            .is_some_and(|(_, rp)| {
                crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    *rp,
                    crate::components::SensorySignature::CAT,
                    d.fated_rival_detection_range as f32,
                )
            });
        apply_fated_bonuses(&mut scores, love_visible, rival_nearby, sc);
        if let Ok(directive) = world_state.active_directive_query.get(entity) {
            let fondness_factor = res
                .relationships
                .get(entity, directive.coordinator)
                .map_or(d.fondness_default, |r| (r.fondness + 1.0) / 2.0);
            let bonus = directive.priority
                * directive.coordinator_social_weight
                * d.directive_bonus_base_weight
                * personality.diligence
                * fondness_factor
                * (1.0 - personality.independence * d.directive_independence_penalty)
                * (1.0 - personality.stubbornness * d.directive_stubbornness_penalty);
            apply_directive_bonus(&mut scores, directive.kind.to_action(), bonus);
        }
        enforce_survival_floor(&mut scores, needs, sc);

        // Groom routing.
        let self_groom_score =
            (1.0 - needs.temperature) * sc.self_groom_temperature_scale * needs.level_suppression(1);
        let other_groom_score = if has_social_target {
            personality.warmth * (1.0 - needs.social) * needs.level_suppression(2)
        } else {
            0.0
        };
        let self_groom_won = self_groom_score >= other_groom_score;

        // §L2.10.6 softmax-over-Intentions: softmax the flat action pool
        // directly, then map the winning Intention to its disposition. The
        // helper preserves the legacy disposition-level independence penalty
        // by applying it as an action-level transform on Coordinate /
        // Socialize / Mentor (and Groom when socializing) before softmax.
        let chosen = select_disposition_via_intention_softmax(
            &scores,
            self_groom_won,
            personality.independence,
            d.disposition_independence_penalty,
            sc,
            &mut rng.rng,
        );

        // Store all gate-open action scores, sorted descending, for
        // diagnostics. Truncation removed 2026-04-20 so scoring-competition
        // analysis can see ranks beyond the top few (e.g., Mate vs Socialize
        // on shared ticks).
        {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            current.last_scores = sorted;
        }

        let crafting_hint = if chosen == DispositionKind::Crafting {
            // If a corruption-response directive is active, route the plan
            // directly to the matching narrow action.
            let directive_hint = world_state
                .active_directive_query
                .get(entity)
                .ok()
                .and_then(|d| match d.kind {
                    DirectiveKind::Cleanse => Some(CraftingHint::Cleanse),
                    DirectiveKind::HarvestCarcass => Some(CraftingHint::HarvestCarcass),
                    _ => None,
                });

            if let Some(h) = directive_hint {
                Some(h)
            } else {
                let herbcraft_score = scores
                    .iter()
                    .find(|(a, _)| *a == Action::Herbcraft)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                let magic_score = scores
                    .iter()
                    .find(|(a, _)| *a == Action::PracticeMagic)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                if magic_score > herbcraft_score {
                    result.magic_hint.or(Some(CraftingHint::Magic))
                } else {
                    result.herbcraft_hint
                }
            }
        } else {
            None
        };

        // Build planner state and zone distances.
        let construction_pos: Vec<(Entity, Position)> = world_state
            .building_query
            .iter()
            .filter(|(_, _, _, site, _)| site.is_some())
            .map(|(e, _, p, _, _)| (e, *p))
            .collect();
        let farm_pos: Vec<Position> = world_state
            .building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Garden && site.is_none())
            .map(|(_, _, p, _, _)| *p)
            .collect();
        let planner_state = build_planner_state(
            pos,
            needs,
            inventory,
            0,
            &res.map,
            &stores_positions,
            &construction_pos,
            &farm_pos,
            &herb_positions,
            d,
        );
        let zone_distances = build_zone_distances(
            pos,
            &res.map,
            &stores_positions,
            &construction_pos,
            &farm_pos,
            &herb_positions,
            &kitchen_positions,
            &cat_positions,
            entity,
            d,
        );
        let mut actions = actions_for_disposition(chosen, crafting_hint, &zone_distances);
        // Posse override: when a Fight directive is active on the cat and
        // they've landed in Guarding disposition, replace the generic
        // action list (which A* solves with cheapest = Survey) with a
        // single EngageThreat step. The posse mechanic depends on cats
        // converging on and engaging the target shadow-fox rather than
        // wandering their patrol zone.
        let fight_directive_target = if chosen == DispositionKind::Guarding {
            if let Ok(directive) = world_state.active_directive_query.get(entity) {
                if directive.kind == DirectiveKind::Fight {
                    actions = vec![crate::ai::planner::GoapActionDef {
                        kind: GoapActionKind::EngageThreat,
                        cost: 1,
                        preconditions: vec![],
                        effects: vec![crate::ai::planner::StateEffect::IncrementTrips],
                    }];
                    directive
                        .target_position
                        .map(|tp| (tp, directive.target_entity))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        let goal = goal_for_disposition(chosen, 0);

        if let Some(steps) = make_plan(planner_state, &actions, &goal, 12, 1000) {
            let mut plan = GoapPlan::new(chosen, res.time.tick, personality, steps, crafting_hint);
            if chosen == DispositionKind::Resting {
                plan.max_replans = d.resting_max_replans;
            }
            // Flow ward placement position from coordinator directive.
            if crafting_hint == Some(CraftingHint::SetWard) {
                if let Ok(directive) = world_state.active_directive_query.get(entity) {
                    if directive.kind == DirectiveKind::SetWard {
                        plan.ward_placement_pos = directive.target_position;
                    }
                }
            }
            // Flow posse target (Fight directive) into the first step's
            // target_entity so EngageThreat doesn't re-pick by nearest.
            if let Some((_target_pos, Some(target_entity))) = fight_directive_target {
                if let Some(slot) = plan.step_state.first_mut() {
                    slot.target_entity = Some(target_entity);
                }
            }

            if let Some(ref mut log) = event_log {
                log.push(
                    res.time.tick,
                    EventKind::PlanCreated {
                        cat: name.0.clone(),
                        disposition: format!("{:?}", chosen),
                        steps: plan
                            .steps
                            .iter()
                            .map(|s| format!("{:?}", s.action))
                            .collect(),
                        hunger: needs.hunger,
                        energy: needs.energy,
                        temperature: needs.temperature,
                        food_available,
                    },
                );
            }

            plan_writer.write(PlanNarrative {
                entity,
                kind: chosen,
                event: PlanEvent::Adopted,
                completions: 0,
            });

            current.ticks_remaining = u64::MAX;
            commands.entity(entity).insert(plan);
        }
        // If no plan found, cat stays idle (ticks_remaining = 0).
    }
}

// ===========================================================================
// resolve_goap_plans — executor dispatching to step resolver helpers
// ===========================================================================

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_goap_plans(
    mut cats: Query<
        (
            (
                Entity,
                &mut GoapPlan,
                &mut CurrentAction,
                &mut Position,
                &mut Skills,
                &mut Needs,
                &mut Inventory,
                &Personality,
                &Name,
            ),
            (
                &Gender,
                Option<&mut ActionHistory>,
                &mut HuntingPriors,
                Option<&mut crate::components::grooming::GroomingCondition>,
                &mut crate::components::mental::Mood,
                &mut Health,
                &MagicAffinity,
                &mut Corruption,
                &mut Memory,
                &mut PendingUrgencies,
            ),
        ),
        (
            Without<Dead>,
            Without<Structure>,
            Without<PreyAnimal>,
            Without<PreyDen>,
            Without<Herb>,
            Without<crate::components::wildlife::Carcass>,
            Without<WildAnimal>,
        ),
    >,
    mut prey_query: Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    mut stores_query: Query<&mut StoredItems>,
    items_query: Query<&Item>,
    mut unchained_skills: Query<&mut Skills, (Without<GoapPlan>, Without<Structure>)>,
    mut relationships: ResMut<Relationships>,
    mut narr: NarrativeEmitter<'_>,
    mut rng: ResMut<SimRng>,
    mut colony_map: ResMut<ColonyHuntingMap>,
    den_query: Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    mut prey_params: PreyHuntParams,
    mut commands: Commands,
    mut ec: ExecutorContext,
    mut building_params: BuildingResolverParams,
    mut magic_params: MagicResolverParams,
    mut plan_writer: MessageWriter<PlanNarrative>,
) {
    let d = &ec.constants.disposition;

    struct MentorEffect {
        apprentice: Entity,
        mentor_skills: Skills,
    }
    let mut mentor_effects: Vec<MentorEffect> = Vec::new();
    let mut plans_to_remove: Vec<Entity> = Vec::new();

    let grooming_snapshot: HashMap<Entity, f32> = cats
        .iter()
        .map(
            |((e, _, _, _, _, _, _, _, _), (_, _, _, g, _, _, _, _, _, _))| {
                (e, g.as_ref().map_or(0.8, |g| g.0))
            },
        )
        .collect();
    // Gender snapshot for §7.M.7.4's `resolve_mate_with` partner lookup —
    // lets the MateWith step pick the gestation-capable partner without
    // double-borrowing the mutable `cats` query.
    let gender_snapshot: HashMap<Entity, Gender> = cats
        .iter()
        .map(|((e, _, _, _, _, _, _, _, _), (g, _, _, _, _, _, _, _, _, _))| (e, *g))
        .collect();
    let mut grooming_restorations: Vec<(Entity, f32)> = Vec::new();

    let cat_tile_counts: HashMap<Position, u32> = {
        let mut counts = HashMap::new();
        for ((_, _, _, pos, _, _, _, _, _), _) in &cats {
            *counts.entry(*pos).or_insert(0) += 1;
        }
        counts
    };

    // Pre-collect building and herb data to avoid query conflicts with cats.
    let building_snapshot: Vec<(Entity, StructureType, Position, bool, bool)> = building_params
        .buildings
        .iter()
        .map(|(e, s, site, crop, p)| (e, s.kind, *p, site.is_some(), crop.is_some()))
        .collect();

    let stores_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, _, _)| *kind == StructureType::Stores)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    let stores_entities: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, kind, _, _, _)| *kind == StructureType::Stores)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();

    // Only completed kitchens count — a construction site can't be cooked at.
    let kitchen_entities: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Kitchen && !*is_site)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();
    let kitchen_positions: Vec<Position> =
        kitchen_entities.iter().map(|(_, p)| *p).collect();

    let construction_positions: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, _, _, is_site, _)| *is_site)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();

    let farm_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Garden && !*is_site)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    let herb_positions: Vec<(Entity, Position, HerbKind)> = magic_params
        .herb_query
        .iter()
        .map(|(e, herb, p)| (e, *p, herb.kind))
        .collect();

    let workshop_bonus: f32 = if building_snapshot
        .iter()
        .any(|(_, kind, _, _, _)| *kind == StructureType::Workshop)
    {
        1.3
    } else {
        1.0
    };

    // Seasonal modifier for farming — simplified to 1.0 pending SimConfig
    // access in ExecutorContext. Tunable later.
    let season_mod: f32 = 1.0;

    // Count cats adjacent to each construction site (for multi-builder bonuses).
    let builders_per_site: HashMap<Entity, usize> = {
        let cat_pos_list: Vec<Position> = cats
            .iter()
            .map(|((_, _, _, pos, _, _, _, _, _), _)| *pos)
            .collect();
        let mut counts = HashMap::new();
        for (site_e, _, site_pos, is_site, _) in &building_snapshot {
            if *is_site {
                let n = cat_pos_list
                    .iter()
                    .filter(|cp| cp.manhattan_distance(site_pos) <= 1)
                    .count();
                if n > 0 {
                    counts.insert(*site_e, n);
                }
            }
        }
        counts
    };

    let cat_positions: Vec<(Entity, Position)> = cats
        .iter()
        .map(|((e, _, _, pos, _, _, _, _, _), _)| (e, *pos))
        .collect();

    let injured_cat_positions: Vec<(Entity, Position)> = cats
        .iter()
        .filter(|(_, (_, _, _, _, _, health, _, _, _, _))| health.current < health.max)
        .map(|((e, _, _, pos, _, _, _, _, _), _)| (e, *pos))
        .collect();

    for (
        (
            cat_entity,
            mut plan,
            mut current,
            mut pos,
            mut skills,
            mut needs,
            mut inventory,
            personality,
            name,
        ),
        (
            gender,
            history,
            mut hunting_priors,
            mut grooming,
            mut mood,
            mut health,
            magic_aff,
            mut corruption,
            mut memory,
            mut urgencies,
        ),
    ) in &mut cats
    {
        // ---- Plan exhausted: handle trip completion / replanning ----
        if plan.is_exhausted() {
            plan.trips_done += 1;
            let respect_gain = respect_for_disposition(plan.kind, d);
            if respect_gain > 0.0 {
                needs.respect = (needs.respect + respect_gain).min(1.0);
            }

            // Building completion mood boost.
            if plan.kind == DispositionKind::Building {
                mood.modifiers
                    .push_back(crate::components::mental::MoodModifier {
                        amount: 0.2,
                        ticks_remaining: 100,
                        source: "built something".to_string(),
                    });
            }

            // Check if disposition goal is fully met.
            let disposition_complete = match plan.kind {
                DispositionKind::Resting => {
                    needs.hunger >= d.resting_complete_hunger
                        && needs.energy >= d.resting_complete_energy
                        && needs.temperature >= d.resting_complete_temperature
                }
                _ => plan.trips_done >= plan.target_trips,
            };

            if disposition_complete {
                if let Some(mut hist) = history {
                    hist.record(ActionRecord {
                        action: current.action,
                        disposition: Some(plan.kind),
                        tick: ec.time.tick,
                        outcome: ActionOutcome::Success,
                    });
                }
                plan_writer.write(PlanNarrative {
                    entity: cat_entity,
                    kind: plan.kind,
                    event: PlanEvent::Completed,
                    completions: plan.trips_done,
                });
                current.ticks_remaining = 0;
                plans_to_remove.push(cat_entity);
            } else {
                // Need more trips — replan from current state.
                let planner_state = build_planner_state(
                    &pos,
                    &needs,
                    &inventory,
                    plan.trips_done,
                    &ec.map,
                    &stores_positions,
                    &construction_positions,
                    &farm_positions,
                    &herb_positions,
                    d,
                );
                let zone_distances = build_zone_distances(
                    &pos,
                    &ec.map,
                    &stores_positions,
                    &construction_positions,
                    &farm_positions,
                    &herb_positions,
                    &kitchen_positions,
                    &cat_positions,
                    cat_entity,
                    d,
                );
                let actions =
                    actions_for_disposition(plan.kind, plan.crafting_hint, &zone_distances);
                let goal = goal_for_disposition(plan.kind, plan.trips_done);

                if let Some(new_steps) = make_plan(planner_state, &actions, &goal, 12, 1000) {
                    plan.replan(new_steps);
                } else {
                    // Can't plan next trip — complete anyway.
                    current.ticks_remaining = 0;
                    plans_to_remove.push(cat_entity);
                }
            }
            continue;
        }

        // ---- Get current step and tick ----
        let step_idx = plan.current_step;
        let step = &plan.steps[step_idx];
        let action_kind = step.action;

        // Initialize step state on first tick.
        if plan.step_state[step_idx].ticks_elapsed == 0 {
            current.action = action_kind.to_action(plan.kind);
            current.target_position = plan.step_state[step_idx].target_position;
            current.target_entity = plan.step_state[step_idx].target_entity;
        }

        plan.step_state[step_idx].ticks_elapsed += 1;
        let ticks = plan.step_state[step_idx].ticks_elapsed;

        // ---- Dispatch on action kind ----
        let step_result = match action_kind {
            GoapActionKind::TravelTo(zone) => resolve_travel_to(
                zone,
                &mut plan.step_state[step_idx],
                &mut pos,
                &ec.map,
                &cat_tile_counts,
                &stores_positions,
                &construction_positions,
                &farm_positions,
                &herb_positions,
                &kitchen_positions,
                &cat_positions,
                cat_entity,
                d,
            ),

            GoapActionKind::SearchPrey => resolve_search_prey(
                &mut plan.step_state[step_idx],
                ticks,
                &mut pos,
                &mut hunting_priors,
                &mut colony_map,
                &prey_query,
                &den_query,
                &mut inventory,
                &mut skills,
                &mut prey_params,
                &ec.map,
                &ec.wind,
                &mut narr,
                &ec.time,
                &mut rng,
                &mut commands,
                cat_entity,
                personality,
                name,
                gender,
                &needs,
                d,
                &ec.constants.sensory.cat,
            ),

            GoapActionKind::EngagePrey => {
                // Get prey target from previous SearchPrey step's state, or from
                // our own state (set during replan).
                if plan.step_state[step_idx].target_entity.is_none() && step_idx > 0 {
                    plan.step_state[step_idx].target_entity =
                        plan.step_state[step_idx - 1].target_entity;
                }
                resolve_engage_prey(
                    &mut plan.step_state[step_idx],
                    ticks,
                    &mut pos,
                    &mut inventory,
                    &mut skills,
                    &mut hunting_priors,
                    &mut prey_query,
                    &mut prey_params,
                    &ec.map,
                    &mut narr,
                    &ec.time,
                    &mut rng,
                    &mut commands,
                    cat_entity,
                    personality,
                    name,
                    gender,
                    &needs,
                    d,
                    ec.event_log.as_deref_mut(),
                )
            }

            GoapActionKind::DepositPrey
            | GoapActionKind::DepositFood
            | GoapActionKind::DepositCookedFood => {
                // Resolve nearest store as target.
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = stores_entities
                        .iter()
                        .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
                        .map(|(e, _)| *e);
                }
                let deposit = crate::steps::disposition::resolve_deposit_at_stores(
                    plan.step_state[step_idx].target_entity,
                    &mut inventory,
                    &skills,
                    &pos,
                    &mut stores_query,
                    &items_query,
                    &mut commands,
                    d,
                );
                if deposit.storage_upgraded {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::StorageUpgraded);
                    }
                }
                if deposit.rejected {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::DepositRejected);
                    }
                }
                if deposit.no_store {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::DepositFailedNoStore);
                    }
                }
                deposit.step
            }

            GoapActionKind::ForageItem => resolve_forage_item(
                &mut plan.step_state[step_idx],
                ticks,
                &mut pos,
                &mut inventory,
                &mut skills,
                &ec.map,
                &mut narr,
                &ec.time,
                &mut rng,
                personality,
                name,
                gender,
                &needs,
                d,
            ),

            GoapActionKind::EatAtStores => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = stores_entities
                        .iter()
                        .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
                        .map(|(e, _)| *e);
                }
                crate::steps::disposition::resolve_eat_at_stores(
                    ticks,
                    plan.step_state[step_idx].target_entity,
                    &mut needs,
                    &mut stores_query,
                    &items_query,
                    &mut commands,
                    d,
                )
            }

            GoapActionKind::Sleep => {
                let duration = d.sleep_duration_base
                    + ((1.0 - needs.energy) * d.sleep_duration_deficit_multiplier) as u64;
                // Corruption degrades rest quality.
                let tile_corruption = if ec.map.in_bounds(pos.x, pos.y) {
                    ec.map.get(pos.x, pos.y).corruption
                } else {
                    0.0
                };
                let result =
                    crate::steps::disposition::resolve_sleep(ticks, duration, &mut needs, d);
                if tile_corruption > 0.0 {
                    let penalty =
                        tile_corruption * (1.0 - ec.constants.magic.corruption_rest_penalty);
                    needs.energy = (needs.energy - d.sleep_energy_per_tick * penalty).max(0.0);
                }
                result
            }

            GoapActionKind::SelfGroom => {
                let result = crate::steps::disposition::resolve_self_groom(
                    ticks,
                    &mut needs,
                    grooming.as_deref_mut(),
                    d,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut log) = ec.event_log {
                        log.push(
                            ec.time.tick,
                            EventKind::GroomingFired {
                                cat: name.0.clone(),
                                target: None,
                            },
                        );
                    }
                }
                result
            }

            GoapActionKind::SocializeWith => {
                // Resolve social target on first tick.
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity =
                        find_social_target(cat_entity, &pos, &cat_positions, &relationships, d);
                }
                let result = crate::steps::disposition::resolve_socialize(
                    ticks,
                    cat_entity,
                    plan.step_state[step_idx].target_entity,
                    &mut needs,
                    &mut hunting_priors,
                    &mut relationships,
                    &mut colony_map,
                    &grooming_snapshot,
                    ec.time.tick,
                    &ec.constants.social,
                    d,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    magic_params
                        .pushback_writer
                        .write(crate::systems::magic::CorruptionPushback {
                            position: *pos,
                            radius: 2,
                            amount: 0.01,
                        });
                }
                result
            }

            GoapActionKind::GroomOther => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity =
                        find_social_target(cat_entity, &pos, &cat_positions, &relationships, d);
                }
                let (result, restoration) = crate::steps::disposition::resolve_groom_other(
                    ticks,
                    cat_entity,
                    plan.step_state[step_idx].target_entity,
                    &mut needs,
                    &mut hunting_priors,
                    &mut relationships,
                    &mut colony_map,
                    &grooming_snapshot,
                    ec.time.tick,
                    &ec.constants.social,
                    d,
                );
                if let Some(r) = restoration {
                    grooming_restorations.push(r);
                }
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut log) = ec.event_log {
                        log.push(
                            ec.time.tick,
                            EventKind::GroomingFired {
                                cat: name.0.clone(),
                                target: plan.step_state[step_idx]
                                    .target_entity
                                    .map(|e| format!("entity:{}", e.index())),
                            },
                        );
                    }
                }
                result
            }

            GoapActionKind::MentorCat => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity =
                        find_social_target(cat_entity, &pos, &cat_positions, &relationships, d);
                }
                let (result, effect) = crate::steps::disposition::resolve_mentor_cat(
                    ticks,
                    cat_entity,
                    plan.step_state[step_idx].target_entity,
                    &mut needs,
                    &skills,
                    &mut relationships,
                    ec.time.tick,
                    d,
                );
                if let Some((apprentice, mentor_skills)) = effect {
                    mentor_effects.push(MentorEffect {
                        apprentice,
                        mentor_skills,
                    });
                }
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut log) = ec.event_log {
                        log.push(
                            ec.time.tick,
                            EventKind::MentoringFired {
                                mentor: name.0.clone(),
                                apprentice: plan.step_state[step_idx]
                                    .target_entity
                                    .map(|e| format!("entity:{}", e.index()))
                                    .unwrap_or_else(|| "unknown".into()),
                            },
                        );
                    }
                }
                result
            }

            GoapActionKind::PatrolArea => {
                if plan.step_state[step_idx].target_position.is_none() {
                    plan.step_state[step_idx].target_position = find_random_nearby_tile(
                        &pos,
                        &ec.map,
                        d.guard_patrol_radius as i32,
                        |t| t.is_passable(),
                        &mut rng.rng,
                    );
                }
                crate::steps::disposition::resolve_patrol_to(
                    &mut pos,
                    plan.step_state[step_idx].target_position,
                    &mut plan.step_state[step_idx].cached_path,
                    &mut needs,
                    &ec.map,
                    d,
                    &cat_tile_counts,
                )
            }

            GoapActionKind::EngageThreat => {
                // Resolve nearest wildlife as the combat target on the first tick.
                // step_state.target_entity is copied into CurrentAction.target_entity
                // only at ticks_elapsed == 0 (before dispatch), so we must also write
                // current.target_entity directly here for resolve_combat to pick it up.
                if plan.step_state[step_idx].target_entity.is_none() {
                    let nearest = ec
                        .wildlife
                        .iter()
                        .min_by_key(|(_, wp)| pos.manhattan_distance(wp))
                        .map(|(e, _)| e);
                    plan.step_state[step_idx].target_entity = nearest;
                    current.target_entity = nearest;
                }
                // Move toward the target until adjacent. Without this step,
                // posse-directed cats would set Action::Fight where they
                // stood and wait for the fox to walk to them — which never
                // happens because shadow-foxes avoid wards and cats. Posse
                // formation requires cats to actually converge on the fox.
                let target_pos_opt: Option<Position> = plan.step_state[step_idx]
                    .target_entity
                    .and_then(|t| ec.wildlife.get(t).ok().map(|(_, p)| *p));
                if let Some(target_pos) = target_pos_opt {
                    let dist = pos.manhattan_distance(&target_pos);
                    if dist > 1 {
                        if plan.step_state[step_idx].cached_path.is_none()
                            || plan.step_state[step_idx]
                                .cached_path
                                .as_ref()
                                .and_then(|p| p.last())
                                .is_some_and(|last| *last != target_pos)
                        {
                            plan.step_state[step_idx].cached_path =
                                crate::ai::pathfinding::find_path(*pos, target_pos, &ec.map);
                        }
                        if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                            if let Some(next) = path.first().copied() {
                                path.remove(0);
                                *pos = next;
                            }
                        }
                        crate::steps::StepResult::Continue
                    } else {
                        crate::steps::disposition::resolve_fight_threat(
                            ticks,
                            &mut skills,
                            &mut needs,
                            &health,
                            d,
                        )
                    }
                } else {
                    crate::steps::disposition::resolve_fight_threat(
                        ticks,
                        &mut skills,
                        &mut needs,
                        &health,
                        d,
                    )
                }
            }

            GoapActionKind::Survey => crate::steps::disposition::resolve_survey(
                ticks,
                &mut needs,
                &pos,
                &mut prey_params.exploration_map,
                d,
            ),

            GoapActionKind::DeliverDirective => {
                let result =
                    crate::steps::disposition::resolve_deliver_directive(ticks, &mut needs, d);
                if matches!(result, crate::steps::StepResult::Advance) {
                    // TODO: resolve directive kind and target from the coordination system.
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::DirectiveDelivered);
                    }
                }
                result
            }

            GoapActionKind::MateWith => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity =
                        find_social_target(cat_entity, &pos, &cat_positions, &relationships, d);
                }
                let target = plan.step_state[step_idx].target_entity;
                let target_gender = target.and_then(|t| gender_snapshot.get(&t).copied());
                let (result, pregnancy) = crate::steps::disposition::resolve_mate_with(
                    ticks,
                    cat_entity,
                    *gender,
                    target,
                    target_gender,
                    &mut needs,
                    &mut relationships,
                );
                if let Some((gestator, litter_size)) = pregnancy {
                    // §7.M.7.4: Pregnant lands on the gestation-capable
                    // partner. `partner` on the Pregnant struct is the
                    // other mate — so if the initiator is the gestator,
                    // partner = target; otherwise partner = initiator.
                    let partner = if gestator == cat_entity {
                        target.unwrap_or(cat_entity)
                    } else {
                        cat_entity
                    };
                    commands.entity(gestator).insert(
                        crate::components::pregnancy::Pregnant::new(
                            ec.time.tick,
                            partner,
                            litter_size,
                        ),
                    );
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::MatingOccurred);
                    }
                    if let Some(ref mut elog) = ec.event_log {
                        elog.push(
                            ec.time.tick,
                            EventKind::MatingOccurred {
                                partner_a: name.0.clone(),
                                partner_b: format!("{partner:?}"),
                                location: (pos.x, pos.y),
                            },
                        );
                    }
                    magic_params
                        .pushback_writer
                        .write(crate::systems::magic::CorruptionPushback {
                            position: *pos,
                            radius: 2,
                            amount: 0.03,
                        });
                }
                result
            }

            GoapActionKind::FeedKitten => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = stores_entities
                        .iter()
                        .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
                        .map(|(e, _)| *e);
                }
                crate::steps::disposition::resolve_feed_kitten(
                    ticks,
                    plan.step_state[step_idx].target_entity,
                    &mut needs,
                    &mut stores_query,
                    &items_query,
                    &mut commands,
                )
            }

            GoapActionKind::GatherHerb => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    // When the plan includes SetWard, target Thornbriar specifically.
                    // Otherwise SetWard fails at runtime ("no thornbriar for ward")
                    // because the cat gathered the wrong herb type.
                    let wants_thornbriar = plan
                        .steps
                        .iter()
                        .any(|s| matches!(s.action, GoapActionKind::SetWard));
                    plan.step_state[step_idx].target_entity = herb_positions
                        .iter()
                        .filter(|(_, _, kind)| !wants_thornbriar || *kind == HerbKind::Thornbriar)
                        .min_by_key(|(_, hp, _)| pos.manhattan_distance(hp))
                        .map(|(e, _, _)| *e);
                }
                let result = crate::steps::magic::resolve_gather_herb(
                    ticks,
                    plan.step_state[step_idx].target_entity,
                    &mut inventory,
                    &mut skills,
                    &magic_params.herb_query,
                    &mut commands,
                    &ec.constants.magic,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::GatherHerbCompleted);
                    }
                }
                result
            }

            GoapActionKind::SetWard => {
                // Walk to ward placement target if one was set by the coordinator.
                if let Some(ward_target) = plan.ward_placement_pos {
                    if pos.manhattan_distance(&ward_target) > 1 {
                        if plan.step_state[step_idx].cached_path.is_none() {
                            plan.step_state[step_idx].cached_path =
                                crate::ai::pathfinding::find_path(*pos, ward_target, &ec.map);
                        }
                        if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                            if let Some(next) = path.first().copied() {
                                path.remove(0);
                                *pos = next;
                            }
                        }
                        crate::steps::StepResult::Continue
                    } else {
                        let ward_kind = match plan.crafting_hint {
                            Some(crate::components::disposition::CraftingHint::DurableWard) => {
                                crate::components::magic::WardKind::DurableWard
                            }
                            _ => crate::components::magic::WardKind::Thornward,
                        };
                        let result = crate::steps::magic::resolve_set_ward(
                            ticks,
                            ward_kind,
                            &name.0,
                            &mut inventory,
                            magic_aff,
                            &mut skills,
                            &mut mood,
                            &mut corruption,
                            &mut health,
                            &ward_target,
                            &mut rng.rng,
                            &mut commands,
                            &mut narr.log,
                            ec.event_log.as_deref_mut(),
                            ec.time.tick,
                            &ec.constants.magic,
                            &ec.constants.combat,
                        );
                        if matches!(result, crate::steps::StepResult::Advance) {
                            if let Some(ref mut act) = narr.activation {
                                act.record(Feature::WardPlaced);
                            }
                        }
                        result
                    }
                } else {
                    let ward_kind = match plan.crafting_hint {
                        Some(crate::components::disposition::CraftingHint::DurableWard) => {
                            crate::components::magic::WardKind::DurableWard
                        }
                        _ => crate::components::magic::WardKind::Thornward,
                    };
                    let result = crate::steps::magic::resolve_set_ward(
                        ticks,
                        ward_kind,
                        &name.0,
                        &mut inventory,
                        magic_aff,
                        &mut skills,
                        &mut mood,
                        &mut corruption,
                        &mut health,
                        &pos,
                        &mut rng.rng,
                        &mut commands,
                        &mut narr.log,
                        ec.event_log.as_deref_mut(),
                        ec.time.tick,
                        &ec.constants.magic,
                        &ec.constants.combat,
                    );
                    if matches!(result, crate::steps::StepResult::Advance) {
                        if let Some(ref mut act) = narr.activation {
                            act.record(Feature::WardPlaced);
                        }
                    }
                    result
                }
            }

            GoapActionKind::PrepareRemedy => {
                let remedy = inventory
                    .first_remedy_kind()
                    .unwrap_or(crate::components::magic::RemedyKind::HealingPoultice);
                let at_workshop = building_snapshot.iter().any(|(_, kind, p, _, _)| {
                    *kind == StructureType::Stores && pos.manhattan_distance(p) <= 1
                });
                crate::steps::magic::resolve_prepare_remedy(
                    ticks,
                    remedy,
                    at_workshop,
                    &mut inventory,
                    &mut skills,
                    &ec.constants.magic,
                )
            }

            GoapActionKind::ApplyRemedy => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    if let Some((patient_e, patient_pos)) = injured_cat_positions
                        .iter()
                        .filter(|(e, _)| *e != cat_entity)
                        .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
                    {
                        plan.step_state[step_idx].target_entity = Some(*patient_e);
                        plan.step_state[step_idx].target_position = Some(*patient_pos);
                    }
                }
                let remedy = inventory
                    .first_remedy_kind()
                    .unwrap_or(crate::components::magic::RemedyKind::HealingPoultice);
                let patient_alive = plan.step_state[step_idx]
                    .target_entity
                    .map(|e| cat_positions.iter().any(|(ce, _)| *ce == e))
                    .unwrap_or(false);
                let (result, gratitude) = crate::steps::magic::resolve_apply_remedy(
                    remedy,
                    cat_entity,
                    plan.step_state[step_idx].target_position,
                    plan.step_state[step_idx].target_entity,
                    patient_alive,
                    &mut plan.step_state[step_idx].cached_path,
                    &mut pos,
                    &mut skills,
                    &ec.map,
                    &mut commands,
                    &mut narr.log,
                    ec.time.tick,
                    &ec.constants.magic,
                );
                if let Some((patient, healer, gain)) = gratitude {
                    relationships.modify_fondness(patient, healer, gain);
                }
                result
            }

            GoapActionKind::Scry => {
                let result = crate::steps::magic::resolve_scry(
                    ticks,
                    &name.0,
                    magic_aff,
                    &mut skills,
                    &mut memory,
                    &mut mood,
                    &mut corruption,
                    &mut health,
                    &pos,
                    &ec.map,
                    &mut rng.rng,
                    &mut commands,
                    &mut narr.log,
                    ec.time.tick,
                    &ec.constants.magic,
                    &ec.constants.combat,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::ScryCompleted);
                    }
                }
                result
            }

            GoapActionKind::SpiritCommunion => {
                let act = &mut narr.activation;
                let result = crate::steps::magic::resolve_spirit_communion(
                    ticks,
                    &name.0,
                    magic_aff,
                    &mut skills,
                    &mut mood,
                    &mut corruption,
                    &mut health,
                    &pos,
                    &mut rng.rng,
                    &mut commands,
                    &mut narr.log,
                    ec.time.tick,
                    act.as_deref_mut().unwrap(),
                    &ec.constants.magic,
                    &ec.constants.combat,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    magic_params
                        .pushback_writer
                        .write(crate::systems::magic::CorruptionPushback {
                            position: *pos,
                            radius: 4,
                            amount: 0.08,
                        });
                }
                result
            }

            GoapActionKind::CleanseCorruption => {
                // On the first tick, resolve the target corrupted tile from
                // the active directive OR the nearest corruption the cat can
                // see. This is the fix that makes directed cleanse actually
                // walk to the hotspot instead of scrubbing an already-clean
                // patch of grass at the cat's feet.
                if plan.step_state[step_idx].target_position.is_none() {
                    let directive_target = magic_params
                        .active_directive_query
                        .get(cat_entity)
                        .ok()
                        .and_then(|d| d.target_position);
                    plan.step_state[step_idx].target_position =
                        directive_target.or_else(|| nearest_corrupted_tile(&pos, &ec.map, 8));
                }

                // Walk toward the target if we have one and we're not adjacent.
                if let Some(target) = plan.step_state[step_idx].target_position {
                    if pos.manhattan_distance(&target) > 0 {
                        if plan.step_state[step_idx].cached_path.is_none() {
                            plan.step_state[step_idx].cached_path =
                                crate::ai::pathfinding::find_path(*pos, target, &ec.map);
                        }
                        if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                            if !path.is_empty() {
                                *pos = path.remove(0);
                            }
                        }
                        crate::steps::StepResult::Continue
                    } else {
                        // Arrived: perform the cleanse.
                        let result = crate::steps::magic::resolve_cleanse_corruption(
                            ticks,
                            &name.0,
                            magic_aff,
                            &mut skills,
                            &mut corruption,
                            &mut mood,
                            &mut health,
                            &pos,
                            &mut ec.map,
                            &mut rng.rng,
                            &mut commands,
                            &mut narr.log,
                            ec.time.tick,
                            &ec.constants.magic,
                            &ec.constants.combat,
                        );
                        if matches!(result, crate::steps::StepResult::Advance) {
                            if let Some(ref mut act) = narr.activation {
                                act.record(Feature::CleanseCompleted);
                            }
                            // Check carcasses within 1 tile — corruption
                            // spreads from a carcass to adjacent tiles, so a
                            // cat cleansing a hotspot may be standing next to
                            // (not on) the actual source.
                            for (_, mut carcass, cp) in &mut magic_params.carcass_query {
                                if !carcass.cleansed && pos.manhattan_distance(cp) <= 1 {
                                    carcass.cleansed = true;
                                    if let Some(ref mut act) = narr.activation {
                                        act.record(Feature::CarcassCleansed);
                                    }
                                }
                            }
                        }
                        result
                    }
                } else {
                    // No corruption found within reach — the crisis has eased
                    // since the directive was issued. Advance without effect.
                    crate::steps::StepResult::Advance
                }
            }

            GoapActionKind::HarvestCarcass => {
                // Resolve target: directive-targeted carcass entity preferred,
                // otherwise nearest unharvested carcass.
                if plan.step_state[step_idx].target_entity.is_none() {
                    let directive_target = magic_params
                        .active_directive_query
                        .get(cat_entity)
                        .ok()
                        .and_then(|d| d.target_position);
                    if let Some(target_pos) = directive_target {
                        plan.step_state[step_idx].target_entity = magic_params
                            .carcass_query
                            .iter()
                            .filter(|(_, c, _)| !c.harvested)
                            .min_by_key(|(_, _, cp)| cp.manhattan_distance(&target_pos))
                            .map(|(e, _, _)| e);
                    } else {
                        plan.step_state[step_idx].target_entity = magic_params
                            .carcass_query
                            .iter()
                            .filter(|(_, c, _)| !c.harvested)
                            .min_by_key(|(_, _, cp)| pos.manhattan_distance(cp))
                            .map(|(e, _, _)| e);
                    }
                    // Cache the carcass position for pathfinding.
                    if let Some(carcass_entity) = plan.step_state[step_idx].target_entity {
                        if let Ok((_, _, cp)) = magic_params.carcass_query.get(carcass_entity) {
                            plan.step_state[step_idx].target_position = Some(*cp);
                        }
                    }
                }

                if let Some(carcass_entity) = plan.step_state[step_idx].target_entity {
                    // Walk to the carcass if we aren't on it yet.
                    let walking = plan.step_state[step_idx]
                        .target_position
                        .is_some_and(|target| pos.manhattan_distance(&target) > 0);

                    if walking {
                        let target = plan.step_state[step_idx].target_position.unwrap();
                        if plan.step_state[step_idx].cached_path.is_none() {
                            plan.step_state[step_idx].cached_path =
                                crate::ai::pathfinding::find_path(*pos, target, &ec.map);
                        }
                        if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                            if !path.is_empty() {
                                *pos = path.remove(0);
                            }
                        }
                        crate::steps::StepResult::Continue
                    } else if ticks >= ec.constants.magic.harvest_carcass_ticks {
                        if let Ok((_, mut carcass, _)) =
                            magic_params.carcass_query.get_mut(carcass_entity)
                        {
                            carcass.harvested = true;
                            let harvest_corruption = if ec.map.in_bounds(pos.x, pos.y) {
                                ec.map.get(pos.x, pos.y).corruption
                            } else {
                                0.0
                            };
                            inventory.add_item_with_modifiers(
                                crate::components::items::ItemKind::ShadowBone,
                                crate::components::items::ItemModifiers::with_corruption(
                                    harvest_corruption,
                                ),
                            );
                            corruption.0 = (corruption.0
                                + ec.constants.magic.harvest_corruption_gain)
                                .min(1.0);
                            skills.herbcraft += skills.growth_rate()
                                * ec.constants.magic.herbcraft_gather_skill_growth;
                            if let Some(ref mut act) = narr.activation {
                                act.record(Feature::CarcassHarvested);
                            }
                        }
                        crate::steps::StepResult::Advance
                    } else {
                        crate::steps::StepResult::Continue
                    }
                } else {
                    crate::steps::StepResult::Fail("no carcass nearby".into())
                }
            }

            GoapActionKind::Construct => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = construction_positions
                        .iter()
                        .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
                        .map(|(e, _)| *e);
                }
                let result = crate::steps::building::resolve_construct(
                    plan.step_state[step_idx].target_entity,
                    &mut pos,
                    &mut plan.step_state[step_idx].cached_path,
                    &mut skills,
                    workshop_bonus,
                    &builders_per_site,
                    &mut building_params.buildings,
                    &ec.map,
                    &mut commands,
                    &mut building_params.colony_score,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::BuildingConstructed);
                    }
                    if let Some(ref mut elog) = ec.event_log {
                        elog.push(
                            ec.time.tick,
                            EventKind::BuildingConstructed {
                                kind: "structure".into(),
                                location: (pos.x, pos.y),
                            },
                        );
                    }
                }
                result
            }

            GoapActionKind::TendCrops => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = building_snapshot
                        .iter()
                        .filter(|(_, kind, _, _, has_crop)| {
                            *kind == StructureType::Garden && *has_crop
                        })
                        .min_by_key(|(_, _, gp, _, _)| pos.manhattan_distance(gp))
                        .map(|(e, _, _, _, _)| *e);
                }
                crate::steps::building::resolve_tend(
                    plan.step_state[step_idx].target_entity,
                    &mut pos,
                    &mut plan.step_state[step_idx].cached_path,
                    &mut skills,
                    season_mod,
                    workshop_bonus,
                    &mut building_params.buildings,
                    &ec.map,
                )
            }

            GoapActionKind::HarvestCrops => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = building_snapshot
                        .iter()
                        .filter(|(_, kind, _, _, has_crop)| {
                            *kind == StructureType::Garden && *has_crop
                        })
                        .min_by_key(|(_, _, gp, _, _)| pos.manhattan_distance(gp))
                        .map(|(e, _, _, _, _)| *e);
                }
                crate::steps::building::resolve_harvest(
                    plan.step_state[step_idx].target_entity,
                    &pos,
                    &stores_entities,
                    &mut building_params.buildings,
                    &mut stores_query,
                    &mut commands,
                )
            }

            GoapActionKind::GatherMaterials => {
                // Not produced by the planner (Construct is a single action).
                // Skill growth fallback for enum exhaustiveness.
                crate::steps::building::resolve_gather(ticks, &mut skills, workshop_bonus)
            }

            GoapActionKind::DeliverMaterials => {
                // Not produced by the planner (Construct handles delivery internally).
                // Fallback for enum exhaustiveness.
                if ticks >= 20 {
                    crate::steps::StepResult::Advance
                } else {
                    crate::steps::StepResult::Continue
                }
            }

            GoapActionKind::RetrieveRawFood => {
                if plan.step_state[step_idx].target_entity.is_none() {
                    plan.step_state[step_idx].target_entity = stores_entities
                        .iter()
                        .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
                        .map(|(e, _)| *e);
                }
                let (result, _retrieved) =
                    crate::steps::disposition::resolve_retrieve_raw_food_from_stores(
                        ticks,
                        plan.step_state[step_idx].target_entity,
                        &mut inventory,
                        &mut stores_query,
                        &items_query,
                        &mut commands,
                    );
                result
            }

            GoapActionKind::Cook => {
                let (result, cooked) = crate::steps::disposition::resolve_cook(
                    ticks,
                    &mut inventory,
                    d,
                );
                if cooked {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::FoodCooked);
                    }
                }
                result
            }

            GoapActionKind::ExploreSurvey => {
                // Survey at a distant tile.
                crate::steps::disposition::resolve_survey(
                    ticks,
                    &mut needs,
                    &pos,
                    &mut prey_params.exploration_map,
                    d,
                )
            }
        };

        // Global safety net: no single step should run indefinitely.
        let step_result = if matches!(step_result, crate::steps::StepResult::Continue)
            && ticks > d.global_step_timeout_ticks
        {
            crate::steps::StepResult::Fail("global step timeout".into())
        } else {
            step_result
        };

        // Apply step result.
        match step_result {
            crate::steps::StepResult::Continue => {}
            crate::steps::StepResult::Advance => {
                // --- Step boundary: evaluate pending urgencies ---
                let mut preempted = false;
                if let Some(urgent) = urgencies.highest() {
                    let current_maslow = plan.kind.maslow_level();
                    // An urgency preempts only if its maslow level is strictly
                    // lower (more fundamental) than the current plan's.
                    if urgent.maslow_level < current_maslow {
                        // Preserve Hunt/Herbcraft guard for threats.
                        let suppressed = urgent.kind == UrgencyKind::ThreatNearby
                            && matches!(current.action, Action::Hunt | Action::Herbcraft);

                        if !suppressed {
                            if let Some(ref mut log) = ec.event_log {
                                let current_step = plan
                                    .current()
                                    .map(|s| format!("{:?}", s.action))
                                    .unwrap_or_else(|| "none".into());
                                log.push(
                                    ec.time.tick,
                                    EventKind::PlanInterrupted {
                                        cat: name.0.clone(),
                                        disposition: format!("{:?}", plan.kind),
                                        reason: format!(
                                            "urgency {:?} (level {}) preempted level {} plan",
                                            urgent.kind, urgent.maslow_level, current_maslow
                                        ),
                                        current_step,
                                        hunger: needs.hunger,
                                        energy: needs.energy,
                                        temperature: needs.temperature,
                                    },
                                );
                            }

                            // ThreatNearby: set flee action with vector away from threat.
                            if urgent.kind == UrgencyKind::ThreatNearby {
                                if let Some(threat_pos) = urgent.threat_pos {
                                    let dx = pos.x - threat_pos.x;
                                    let dy = pos.y - threat_pos.y;
                                    let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                                    let fd = d.flee_distance;
                                    let mut target = Position::new(
                                        pos.x + (dx as f32 / len * fd) as i32,
                                        pos.y + (dy as f32 / len * fd) as i32,
                                    );
                                    target.x = target.x.clamp(0, ec.map.width - 1);
                                    target.y = target.y.clamp(0, ec.map.height - 1);
                                    current.action = Action::Flee;
                                    current.ticks_remaining = 0;
                                    current.target_position = Some(target);
                                    current.target_entity = None;
                                }
                            }

                            // Mark plan exhausted so it flows through the normal
                            // completion path for cleanup.
                            plan.current_step = plan.steps.len();

                            plan_writer.write(PlanNarrative {
                                entity: cat_entity,
                                kind: plan.kind,
                                event: PlanEvent::Abandoned,
                                completions: plan.trips_done,
                            });

                            preempted = true;
                        }
                    }
                }
                urgencies.needs.clear();

                if preempted {
                    continue;
                }

                plan.advance();
                // Sync CurrentAction targets for the new step.
                if let Some(state) = plan.current_state() {
                    current.target_position = state.target_position;
                    current.target_entity = state.target_entity;
                }
                if let Some(step) = plan.current() {
                    current.action = step.action.to_action(plan.kind);
                }
            }
            crate::steps::StepResult::Fail(ref fail_reason) => {
                if let Some(ref mut log) = ec.event_log {
                    let step_name = plan
                        .current()
                        .map(|s| format!("{:?}", s.action))
                        .unwrap_or_else(|| "none".into());
                    log.push(
                        ec.time.tick,
                        EventKind::PlanStepFailed {
                            cat: name.0.clone(),
                            disposition: format!("{:?}", plan.kind),
                            step: step_name,
                            step_index: plan.current_step,
                            reason: fail_reason.clone(),
                            hunger: needs.hunger,
                            energy: needs.energy,
                            temperature: needs.temperature,
                        },
                    );
                }

                // Record the failed action so replanning can exclude it.
                let failed_action = plan.current().map(|s| s.action);
                if let Some(action) = failed_action {
                    plan.failed_actions.insert(action);
                }

                // Attempt replanning.
                let planner_state = build_planner_state(
                    &pos,
                    &needs,
                    &inventory,
                    plan.trips_done,
                    &ec.map,
                    &stores_positions,
                    &construction_positions,
                    &farm_positions,
                    &herb_positions,
                    d,
                );
                let zone_distances = build_zone_distances(
                    &pos,
                    &ec.map,
                    &stores_positions,
                    &construction_positions,
                    &farm_positions,
                    &herb_positions,
                    &kitchen_positions,
                    &cat_positions,
                    cat_entity,
                    d,
                );
                let mut actions =
                    actions_for_disposition(plan.kind, plan.crafting_hint, &zone_distances);
                actions.retain(|a| !plan.failed_actions.contains(&a.kind));
                let goal = goal_for_disposition(plan.kind, plan.trips_done);

                if let Some(new_steps) = make_plan(planner_state, &actions, &goal, 12, 1000) {
                    if plan.replan(new_steps) {
                        if let Some(ref mut log) = ec.event_log {
                            log.push(
                                ec.time.tick,
                                EventKind::PlanReplanned {
                                    cat: name.0.clone(),
                                    disposition: format!("{:?}", plan.kind),
                                    replan_count: plan.replan_count,
                                    new_steps: plan
                                        .steps
                                        .iter()
                                        .map(|s| format!("{:?}", s.action))
                                        .collect(),
                                    hunger: needs.hunger,
                                    energy: needs.energy,
                                    temperature: needs.temperature,
                                },
                            );
                        }
                        plan_writer.write(PlanNarrative {
                            entity: cat_entity,
                            kind: plan.kind,
                            event: PlanEvent::Replanned,
                            completions: plan.trips_done,
                        });
                    } else {
                        // Max replans exceeded.
                        plan_writer.write(PlanNarrative {
                            entity: cat_entity,
                            kind: plan.kind,
                            event: PlanEvent::Abandoned,
                            completions: plan.trips_done,
                        });
                        if let Some(mut hist) = history {
                            hist.record(ActionRecord {
                                action: current.action,
                                disposition: Some(plan.kind),
                                tick: ec.time.tick,
                                outcome: ActionOutcome::Failure,
                            });
                        }
                        current.ticks_remaining = 0;
                        plans_to_remove.push(cat_entity);
                    }
                } else {
                    // No plan possible — abandon.
                    plan_writer.write(PlanNarrative {
                        entity: cat_entity,
                        kind: plan.kind,
                        event: PlanEvent::Abandoned,
                        completions: plan.trips_done,
                    });
                    if let Some(mut hist) = history {
                        hist.record(ActionRecord {
                            action: current.action,
                            disposition: Some(plan.kind),
                            tick: ec.time.tick,
                            outcome: ActionOutcome::Failure,
                        });
                    }
                    current.ticks_remaining = 0;
                    plans_to_remove.push(cat_entity);
                }
            }
        }
    }

    // Remove completed/abandoned plans.
    for entity in plans_to_remove {
        commands.entity(entity).remove::<GoapPlan>();
    }

    // Deferred grooming restorations.
    for (target, delta) in grooming_restorations {
        if let Ok((_, (_, _, _, Some(mut g), _, _, _, _, _, _))) = cats.get_mut(target) {
            g.0 = (g.0 + delta).min(1.0);
        }
    }

    // Deferred mentor effects.
    for effect in &mentor_effects {
        let app_skills_result = if let Ok(s) = unchained_skills.get(effect.apprentice) {
            Some((
                s.hunting,
                s.foraging,
                s.herbcraft,
                s.building,
                s.combat,
                s.magic,
                s.growth_rate(),
            ))
        } else if let Ok(((_, _, _, _, s, _, _, _, _), _)) = cats.get(effect.apprentice) {
            Some((
                s.hunting,
                s.foraging,
                s.herbcraft,
                s.building,
                s.combat,
                s.magic,
                s.growth_rate(),
            ))
        } else {
            None
        };
        if let Some((hunt, forage, herb, build, combat, magic, growth_rate)) = app_skills_result {
            let pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            if let Some((idx, _)) = pairs
                .iter()
                .enumerate()
                .filter(|(_, (m, a))| {
                    *m > d.mentor_skill_threshold_high && *a < d.mentor_skill_threshold_low
                })
                .max_by(|(_, (am, aa)), (_, (bm, ba))| {
                    (am - aa)
                        .partial_cmp(&(bm - ba))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                let growth = growth_rate * d.apprentice_skill_growth_multiplier;
                if let Ok(mut s) = unchained_skills.get_mut(effect.apprentice) {
                    match idx {
                        0 => s.hunting += growth,
                        1 => s.foraging += growth,
                        2 => s.herbcraft += growth,
                        3 => s.building += growth,
                        4 => s.combat += growth,
                        5 => s.magic += growth,
                        _ => {}
                    }
                } else if let Ok(((_, _, _, _, mut s, _, _, _, _), _)) =
                    cats.get_mut(effect.apprentice)
                {
                    match idx {
                        0 => s.hunting += growth,
                        1 => s.foraging += growth,
                        2 => s.herbcraft += growth,
                        3 => s.building += growth,
                        4 => s.combat += growth,
                        5 => s.magic += growth,
                        _ => {}
                    }
                }
            }
        }
    }
}

// ===========================================================================
// emit_plan_narrative
// ===========================================================================

#[allow(clippy::too_many_arguments)]
pub fn emit_plan_narrative(
    mut messages: MessageReader<PlanNarrative>,
    names: Query<(&Name, &Gender, &Personality, &Needs, &Position)>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    config: Res<crate::resources::time::SimConfig>,
    weather: Res<crate::resources::weather::WeatherState>,
    registry: Option<Res<crate::resources::narrative_templates::TemplateRegistry>>,
    mut log: ResMut<crate::resources::narrative::NarrativeLog>,
    mut rng: ResMut<SimRng>,
    mut history_query: Query<&mut ActionHistory>,
) {
    for msg in messages.read() {
        // Dedup: don't narrate repeated Adopted events for the same disposition.
        if msg.event == PlanEvent::Adopted {
            if let Ok(mut hist) = history_query.get_mut(msg.entity) {
                if hist.last_narrated_disposition == Some(msg.kind) {
                    continue;
                }
                hist.last_narrated_disposition = Some(msg.kind);
                hist.replans_narrated = 0;
            }
        }

        // Throttle Completed events: suppress repeated completions for the
        // same disposition within 500 ticks (e.g., rest/rested cycles).
        if msg.event == PlanEvent::Completed {
            if let Ok(mut hist) = history_query.get_mut(msg.entity) {
                if let Some((kind, tick)) = hist.last_completed_tick {
                    if kind == msg.kind && time.tick.saturating_sub(tick) < 500 {
                        continue;
                    }
                }
                hist.last_completed_tick = Some((msg.kind, time.tick));
            }
        }

        // Throttle Replanned events: max 1 replan narrative per plan lifecycle.
        if msg.event == PlanEvent::Replanned {
            if let Ok(mut hist) = history_query.get_mut(msg.entity) {
                if hist.replans_narrated >= 1 {
                    continue;
                }
                hist.replans_narrated += 1;
            }
        }

        let Ok((name, gender, personality, needs, pos)) = names.get(msg.entity) else {
            continue;
        };

        let action = msg.kind.constituent_actions()[0];
        let event_tag = match msg.event {
            PlanEvent::Adopted => "plan_adopted",
            PlanEvent::Completed => "plan_complete",
            PlanEvent::Replanned => "plan_replanned",
            PlanEvent::Abandoned => "plan_abandoned",
        };

        let terrain = if map.in_bounds(pos.x, pos.y) {
            map.get(pos.x, pos.y).terrain
        } else {
            Terrain::Grass
        };
        let day_phase = DayPhase::from_tick(time.tick, &config);
        let season = Season::from_tick(time.tick, &config);

        let ctx = TemplateContext {
            action,
            day_phase,
            season,
            weather: weather.current,
            mood_bucket: MoodBucket::Neutral,
            life_stage: LifeStage::Adult,
            has_target: false,
            terrain,
            event: Some(event_tag.into()),
        };
        let var_ctx = VariableContext {
            name: &name.0,
            gender: *gender,
            weather: weather.current,
            day_phase,
            season,
            life_stage: LifeStage::Adult,
            fur_color: "unknown",
            other: None,
            prey: None,
            item: None,
            item_singular: None,
            quality: None,
        };

        let fallback = match msg.event {
            PlanEvent::Adopted => format!("{} sets out to {}.", name.0, msg.kind.verb_infinitive()),
            PlanEvent::Completed => {
                format!("{} finishes {}.", name.0, msg.kind.label().to_lowercase())
            }
            PlanEvent::Replanned => format!("{} adjusts course.", name.0),
            PlanEvent::Abandoned => format!("{} gives up.", name.0),
        };

        emit_event_narrative(
            registry.as_deref(),
            &mut log,
            time.tick,
            fallback,
            crate::resources::narrative::NarrativeTier::Action,
            &ctx,
            &var_ctx,
            personality,
            needs,
            &mut rng.rng,
        );
    }
}

// ===========================================================================
// Helper: resolve TravelTo
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_travel_to(
    zone: PlannerZone,
    state: &mut StepExecutionState,
    pos: &mut Position,
    map: &TileMap,
    cat_tile_counts: &HashMap<Position, u32>,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    cat_entity: Entity,
    d: &DispositionConstants,
) -> crate::steps::StepResult {
    if state.target_position.is_none() {
        state.target_position = resolve_zone_position(
            zone,
            pos,
            map,
            stores_positions,
            construction_positions,
            farm_positions,
            herb_positions,
            kitchen_positions,
            cat_positions,
            cat_entity,
            d,
        );
    }
    let Some(target) = state.target_position else {
        return crate::steps::StepResult::Fail("no reachable zone target".into());
    };

    // Use cached A* path.
    if state.cached_path.is_none() {
        state.cached_path = find_path(*pos, target, map);
    }

    if let Some(ref mut path) = state.cached_path {
        if let Some(next) = path.first().copied() {
            path.remove(0);
            *pos = next;
        }
        if pos.manhattan_distance(&target) <= 1 {
            // Anti-stacking jitter.
            if cat_tile_counts.get(&target).copied().unwrap_or(0) > 1 {
                let occupied: std::collections::HashSet<Position> = cat_tile_counts
                    .keys()
                    .filter(|p| cat_tile_counts[p] > 1)
                    .copied()
                    .collect();
                if let Some(adj) = find_free_adjacent(target, *pos, map, &occupied) {
                    *pos = adj;
                }
            } else {
                *pos = target;
            }
            return crate::steps::StepResult::Advance;
        }
    } else {
        // No path found — step toward target directly.
        let before = *pos;
        if let Some(next) = step_toward(pos, &target, map) {
            *pos = next;
        }
        if pos.manhattan_distance(&target) <= 1 {
            return crate::steps::StepResult::Advance;
        }
        // Early exit: A* found no path and greedy movement made no progress.
        if *pos == before {
            state.no_move_ticks += 1;
        } else {
            state.no_move_ticks = 0;
        }
        if state.no_move_ticks > d.travel_no_path_stuck_ticks {
            return crate::steps::StepResult::Fail("no path and stuck".into());
        }
    }

    // Timeout: if stuck for too long, fail.
    if state.ticks_elapsed > d.travel_timeout_ticks {
        return crate::steps::StepResult::Fail("travel timeout".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve SearchPrey (transplanted from HuntPrey search phase)
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_search_prey(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    hunting_priors: &mut HuntingPriors,
    colony_map: &mut ColonyHuntingMap,
    prey_query: &Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    den_query: &Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    inventory: &mut Inventory,
    skills: &mut Skills,
    prey_params: &mut PreyHuntParams,
    map: &TileMap,
    wind: &crate::resources::wind::WindState,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    commands: &mut Commands,
    _cat_entity: Entity,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    needs: &Needs,
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
) -> crate::steps::StepResult {
    use crate::components::magic::ItemSlot;

    // Den discovery check.
    for (den_entity, den, den_pos) in den_query.iter() {
        if pos.manhattan_distance(den_pos) <= d.den_discovery_range {
            let discovery_chance =
                d.den_discovery_base_chance + skills.hunting * d.den_discovery_skill_scale;
            if rng.rng.random::<f32>() < discovery_chance && den.spawns_remaining > 0 {
                let kills = ((den.spawns_remaining as f32 * d.den_raid_kill_fraction).ceil()
                    as u32)
                    .min(den.raid_drop);
                let drop_item = den.item_kind;
                let den_name = den.den_name;
                let den_pos_copy = *den_pos;

                let den_corruption = if map.in_bounds(den_pos_copy.x, den_pos_copy.y) {
                    map.get(den_pos_copy.x, den_pos_copy.y).corruption
                } else {
                    0.0
                };
                let den_mods =
                    crate::components::items::ItemModifiers::with_corruption(den_corruption);
                for _ in 0..kills {
                    if !inventory.is_full() {
                        inventory.slots.push(ItemSlot::Item(drop_item, den_mods));
                    } else {
                        commands.spawn((
                            crate::components::items::Item::with_modifiers(
                                drop_item,
                                d.den_dropped_item_quality,
                                ItemLocation::OnGround,
                                den_mods,
                            ),
                            Position::new(
                                den_pos_copy.x + rng.rng.random_range(-1..=1i32),
                                den_pos_copy.y + rng.rng.random_range(-1..=1i32),
                            ),
                        ));
                    }
                }

                hunting_priors.record_catch(&den_pos_copy);
                colony_map.beliefs.record_catch(&den_pos_copy);

                prey_params.raid_writer.write(DenRaided {
                    den_entity,
                    kills,
                    item_kind: drop_item,
                    position: den_pos_copy,
                    den_name,
                });

                emit_hunt_narrative(
                    narr,
                    time,
                    rng,
                    map,
                    pos,
                    name,
                    gender,
                    personality,
                    needs,
                    "raid",
                    &format!("{} raids a {}!", name.0, den_name),
                    Some(den_name),
                    None,
                );

                // Den raid counts as finding prey — advance.
                return crate::steps::StepResult::Advance;
            }
        }
    }

    // Search movement: belief > colony belief > wind > patrol_dir.
    let belief_dir = hunting_priors.best_direction(pos, d.search_belief_radius);
    let colony_dir = colony_map
        .beliefs
        .best_direction(pos, d.search_belief_radius);
    let (wx, wy) = wind.direction();
    let (mut dx, mut dy) = if let Some((bx, by)) = belief_dir {
        (bx, by)
    } else if let Some((cx, cy)) = colony_dir {
        (cx, cy)
    } else if wx.abs() > d.search_wind_direction_threshold
        || wy.abs() > d.search_wind_direction_threshold
    {
        (-(wx.signum() as i32), -(wy.signum() as i32))
    } else {
        state.patrol_dir
    };

    if rng.rng.random::<f32>() < d.search_jitter_chance {
        dx = rng.rng.random_range(-1i32..=1);
        dy = rng.rng.random_range(-1i32..=1);
    }
    if dx == 0 && dy == 0 {
        dx = 1;
    }
    let before = *pos;
    for _ in 0..d.search_speed {
        *pos = patrol_move(pos, dx, dy, map);
    }
    // If stuck at terrain edge, randomize direction to escape.
    if *pos == before {
        state.patrol_dir = (
            rng.rng.random_range(-1i32..=1),
            rng.rng.random_range(-1i32..=1),
        );
        let (ndx, ndy) = state.patrol_dir;
        *pos = patrol_move(pos, ndx, ndy, map);
    }

    // Visual detection.
    let visible_prey = prey_query
        .iter()
        .filter(|(_, pp, _, _)| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                cat_profile,
                **pp,
                crate::components::SensorySignature::PREY,
                d.search_visual_detection_range as f32,
            )
        })
        .min_by_key(|(_, pp, _, _)| pos.manhattan_distance(pp));

    if let Some((prey_entity, _, _, _)) = visible_prey {
        state.target_entity = Some(prey_entity);
        return crate::steps::StepResult::Advance;
    }

    // Scent detection via PreyScentMap (Phase 2B — grid-sampled
    // influence map). Finds the strongest-scent bucket within
    // `scent_search_radius`; `min_by_key` resolves to the prey
    // entity closest to that source tile.
    let scent_source = prey_params
        .prey_scent_map
        .highest_nearby(pos.x, pos.y, d.scent_search_radius);
    let scent_above_threshold = scent_source
        .map(|(sx, sy)| prey_params.prey_scent_map.get(sx, sy) >= d.scent_detect_threshold)
        .unwrap_or(false);
    let scented_prey = if scent_above_threshold {
        let (sx, sy) = scent_source.unwrap();
        let source = Position::new(sx, sy);
        prey_query
            .iter()
            .min_by_key(|(_, pp, _, _)| source.manhattan_distance(pp))
    } else {
        None
    };

    if let Some((prey_entity, prey_pos_ref, _, _)) = scented_prey {
        state.target_entity = Some(prey_entity);
        hunting_priors.record_scent(prey_pos_ref);
        emit_hunt_narrative(
            narr,
            time,
            rng,
            map,
            pos,
            name,
            gender,
            personality,
            needs,
            "scent",
            &format!("{} catches a scent on the wind.", name.0),
            None,
            None,
        );
        return crate::steps::StepResult::Advance;
    }

    // Timeout.
    if ticks > d.search_timeout_ticks {
        if inventory
            .slots
            .iter()
            .any(|s| matches!(s, ItemSlot::Item(k, _) if k.is_food()))
        {
            // Have food from earlier — advance to deposit.
            return crate::steps::StepResult::Advance;
        }
        hunting_priors.record_failed_search(pos, ticks);
        return crate::steps::StepResult::Fail("no scent found".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve EngagePrey (transplanted from HuntPrey stalk/chase/pounce)
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_engage_prey(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    inventory: &mut Inventory,
    skills: &mut Skills,
    hunting_priors: &mut HuntingPriors,
    prey_query: &mut Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    prey_params: &mut PreyHuntParams,
    map: &TileMap,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    commands: &mut Commands,
    cat_entity: Entity,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    needs: &Needs,
    d: &DispositionConstants,
    mut event_log: Option<&mut EventLog>,
) -> crate::steps::StepResult {
    use crate::components::magic::ItemSlot;
    use crate::components::prey::PreyAiState;

    let Some(target_entity) = state.target_entity else {
        return crate::steps::StepResult::Fail("no prey target for engage".into());
    };

    let Ok((_, prey_pos, prey_cfg, prey_state)) = prey_query.get(target_entity) else {
        // Prey despawned.
        return crate::steps::StepResult::Fail("prey despawned".into());
    };

    let prey_pos = *prey_pos;
    let prey_is_fleeing = matches!(prey_state.ai_state, PreyAiState::Fleeing { .. });
    let prey_awareness = prey_state.ai_state;
    let catch_mod = prey_cfg.catch_difficulty;
    let item_kind = prey_cfg.item_kind;
    let species_name = prey_cfg.name;
    let flee_strategy = prey_cfg.flee_strategy;
    let dist = pos.manhattan_distance(&prey_pos);

    // Bird teleport — give up immediately.
    if prey_is_fleeing && flee_strategy == crate::components::prey::FleeStrategy::Teleport {
        return crate::steps::StepResult::Fail("prey teleported".into());
    }

    let stalk_start = (prey_cfg.alert_radius + d.stalk_start_buffer).max(d.stalk_start_minimum);
    let pounce_range: i32 = if personality.patience > 0.7 {
        d.pounce_range_patient
    } else if personality.patience < 0.3 {
        d.pounce_range_impatient
    } else {
        d.pounce_range_default
    };

    if dist <= pounce_range {
        // === POUNCE ===
        let awareness_base = match prey_awareness {
            PreyAiState::Idle | PreyAiState::Grazing { .. } => d.pounce_awareness_idle,
            PreyAiState::Alert { .. } => d.pounce_awareness_alert,
            PreyAiState::Fleeing { .. } => d.pounce_awareness_fleeing,
        };
        let distance_mod = match dist {
            0..=1 => d.pounce_distance_close_mod,
            2 => d.pounce_distance_mid_mod,
            _ => d.pounce_distance_far_mod,
        };
        let density = prey_params
            .density
            .0
            .get(&prey_cfg.kind)
            .copied()
            .unwrap_or(d.pounce_density_threshold);
        let density_bonus = if density > d.pounce_density_threshold {
            1.0 + (density - d.pounce_density_threshold)
        } else {
            1.0
        };
        let success_chance = awareness_base
            * (d.pounce_skill_base + skills.hunting * d.pounce_skill_scale)
            * distance_mod
            * catch_mod
            * density_bonus;

        if rng.rng.random::<f32>() < success_chance {
            // Catch!
            commands.entity(target_entity).despawn();
            let catch_corruption = if map.in_bounds(prey_pos.x, prey_pos.y) {
                map.get(prey_pos.x, prey_pos.y).corruption
            } else {
                0.0
            };
            if !inventory.is_full() {
                inventory.slots.push(ItemSlot::Item(
                    item_kind,
                    crate::components::items::ItemModifiers::with_corruption(catch_corruption),
                ));
            }
            skills.hunting += skills.growth_rate() * d.hunt_catch_skill_growth;

            prey_params.kill_writer.write(PreyKilled {
                kind: prey_cfg.kind,
                position: prey_pos,
            });
            if let Some(ref mut elog) = event_log {
                elog.push(
                    time.tick,
                    EventKind::PreyKilled {
                        cat: name.0.clone(),
                        species: species_name.to_string(),
                        location: (prey_pos.x, prey_pos.y),
                    },
                );
            }

            let catch_desc = if catch_corruption > 0.3 {
                format!("{} catches a corrupted {}.", name.0, species_name)
            } else {
                format!("{} catches a {}.", name.0, species_name)
            };
            emit_hunt_narrative(
                narr,
                time,
                rng,
                map,
                pos,
                name,
                gender,
                personality,
                needs,
                "catch",
                &catch_desc,
                Some(species_name),
                None,
            );

            hunting_priors.record_catch(&prey_pos);

            if inventory.is_full() {
                return crate::steps::StepResult::Advance;
            } else {
                // Multi-kill: reset target, keep searching.
                state.target_entity = None;
                return crate::steps::StepResult::Fail("seeking another target".into());
            }
        } else {
            // Miss — prey bolts.
            if let Ok((_, _, _, mut prey_st)) = prey_query.get_mut(target_entity) {
                prey_st.ai_state = PreyAiState::Fleeing {
                    from: cat_entity,
                    toward: None,
                    ticks: 0,
                };
            }

            emit_hunt_narrative(
                narr,
                time,
                rng,
                map,
                pos,
                name,
                gender,
                personality,
                needs,
                "miss",
                &format!("{}'s quarry bolts.", name.0),
                Some(species_name),
                None,
            );

            let chase_limit = if personality.boldness > 0.7 {
                d.chase_limit_bold
            } else {
                d.chase_limit_default
            };
            if ticks > chase_limit {
                return crate::steps::StepResult::Fail("chase timeout".into());
            }
        }
    } else if dist <= stalk_start {
        if prey_is_fleeing {
            // === CHASE ===
            let mut moved = false;
            for _ in 0..d.chase_speed {
                if let Some(next) = step_toward(pos, &prey_pos, map) {
                    *pos = next;
                    moved = true;
                }
            }
            if moved {
                state.no_move_ticks = 0;
            } else {
                state.no_move_ticks += 1;
            }
            if state.no_move_ticks > d.chase_stuck_ticks {
                return crate::steps::StepResult::Fail("stuck while chasing".into());
            }
            let chase_limit = if personality.boldness > 0.7 {
                d.chase_limit_bold
            } else {
                d.chase_limit_default
            };
            if ticks > chase_limit {
                return crate::steps::StepResult::Fail("chase timeout".into());
            }
        } else {
            // === STALK ===
            let mut moved = false;
            if let Some(next) = step_toward(pos, &prey_pos, map) {
                *pos = next;
                moved = true;
            }
            if personality.anxiety > d.anxiety_spook_threshold
                && rng.rng.random::<f32>() < d.anxiety_spook_chance
            {
                if let Ok((_, _, _, mut prey_st)) = prey_query.get_mut(target_entity) {
                    prey_st.ai_state = PreyAiState::Fleeing {
                        from: cat_entity,
                        toward: None,
                        ticks: 0,
                    };
                }
                return crate::steps::StepResult::Fail("anxiety spooked prey".into());
            }
            if moved {
                state.no_move_ticks = 0;
            } else {
                state.no_move_ticks += 1;
            }
            if state.no_move_ticks > d.chase_stuck_ticks {
                return crate::steps::StepResult::Fail("stuck while stalking".into());
            }
        }
    } else {
        // === APPROACH ===
        let mut moved = false;
        for _ in 0..d.approach_speed {
            if let Some(next) = step_toward(pos, &prey_pos, map) {
                *pos = next;
                moved = true;
            }
        }
        if moved {
            state.no_move_ticks = 0;
        } else {
            state.no_move_ticks += 1;
        }
        if dist > d.approach_give_up_distance || state.no_move_ticks > d.chase_stuck_ticks {
            return crate::steps::StepResult::Fail("lost prey during approach".into());
        }
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve ForageItem (transplanted from ForageItem step)
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_forage_item(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    inventory: &mut Inventory,
    skills: &mut Skills,
    map: &TileMap,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    needs: &Needs,
    d: &DispositionConstants,
) -> crate::steps::StepResult {
    use crate::components::items::ItemKind;
    use crate::components::magic::ItemSlot;

    let (mut dx, mut dy) = state.patrol_dir;
    if dx == 0 && dy == 0 {
        dx = 1;
    }
    if rng.rng.random::<f32>() < d.forage_jitter_chance {
        dx = rng.rng.random_range(-1i32..=1);
        dy = rng.rng.random_range(-1i32..=1);
        if dx == 0 && dy == 0 {
            dx = 1;
        }
    }
    *pos = patrol_move(pos, dx, dy, map);

    if map.in_bounds(pos.x, pos.y) {
        let tile = map.get(pos.x, pos.y);
        let forage_yield = tile.terrain.foraging_yield() * (1.0 - tile.corruption).max(0.0);
        if forage_yield > 0.0 && rng.rng.random::<f32>() < forage_yield * d.forage_yield_scale {
            let item_kind = match tile.terrain {
                Terrain::DenseForest => {
                    if rng.rng.random::<bool>() {
                        ItemKind::Mushroom
                    } else {
                        ItemKind::Nuts
                    }
                }
                Terrain::LightForest => {
                    if rng.rng.random::<bool>() {
                        ItemKind::Nuts
                    } else {
                        ItemKind::Berries
                    }
                }
                _ => {
                    if rng.rng.random::<bool>() {
                        ItemKind::Berries
                    } else {
                        ItemKind::Roots
                    }
                }
            };
            let forage_corruption = if map.in_bounds(pos.x, pos.y) {
                map.get(pos.x, pos.y).corruption
            } else {
                0.0
            };
            if !inventory.is_full() {
                inventory.slots.push(ItemSlot::Item(
                    item_kind,
                    crate::components::items::ItemModifiers::with_corruption(forage_corruption),
                ));
            }
            skills.foraging += skills.growth_rate() * d.forage_skill_growth;

            let item_name = if forage_corruption > 0.3 {
                format!("corrupted {}", item_kind.name())
            } else {
                item_kind.name().to_string()
            };
            let terrain = if map.in_bounds(pos.x, pos.y) {
                map.get(pos.x, pos.y).terrain
            } else {
                Terrain::Grass
            };
            let day_phase = DayPhase::from_tick(time.tick, &narr.config);
            let season = Season::from_tick(time.tick, &narr.config);
            let ctx = TemplateContext {
                action: Action::Forage,
                day_phase,
                season,
                weather: narr.weather.current,
                mood_bucket: MoodBucket::Neutral,
                life_stage: LifeStage::Adult,
                has_target: false,
                terrain,
                event: Some("find".into()),
            };
            let var_ctx = VariableContext {
                name: &name.0,
                gender: *gender,
                weather: narr.weather.current,
                day_phase,
                season,
                life_stage: LifeStage::Adult,
                fur_color: "unknown",
                other: None,
                prey: None,
                item: Some(&item_name),
                item_singular: Some(item_kind.singular_name()),
                quality: None,
            };
            emit_event_narrative(
                narr.registry.as_deref(),
                &mut narr.log,
                time.tick,
                format!("{} finds {}.", name.0, item_name),
                crate::resources::narrative::NarrativeTier::Action,
                &ctx,
                &var_ctx,
                personality,
                needs,
                &mut rng.rng,
            );
            return crate::steps::StepResult::Advance;
        }
    }

    if ticks > d.forage_timeout_ticks {
        return crate::steps::StepResult::Fail("nothing found while foraging".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: narrative emission
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn emit_hunt_narrative(
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    map: &TileMap,
    pos: &Position,
    name: &Name,
    gender: &Gender,
    personality: &Personality,
    needs: &Needs,
    event: &str,
    fallback: &str,
    prey: Option<&str>,
    item: Option<&str>,
) {
    let terrain = if map.in_bounds(pos.x, pos.y) {
        map.get(pos.x, pos.y).terrain
    } else {
        Terrain::Grass
    };
    let day_phase = DayPhase::from_tick(time.tick, &narr.config);
    let season = Season::from_tick(time.tick, &narr.config);
    let ctx = TemplateContext {
        action: Action::Hunt,
        day_phase,
        season,
        weather: narr.weather.current,
        mood_bucket: MoodBucket::Neutral,
        life_stage: LifeStage::Adult,
        has_target: prey.is_some(),
        terrain,
        event: Some(event.into()),
    };
    let var_ctx = VariableContext {
        name: &name.0,
        gender: *gender,
        weather: narr.weather.current,
        day_phase,
        season,
        life_stage: LifeStage::Adult,
        fur_color: "unknown",
        other: None,
        prey,
        item,
        item_singular: None,
        quality: None,
    };
    let tier = if event == "catch" || event == "raid" {
        crate::resources::narrative::NarrativeTier::Action
    } else {
        crate::resources::narrative::NarrativeTier::Micro
    };
    emit_event_narrative(
        narr.registry.as_deref(),
        &mut narr.log,
        time.tick,
        fallback.to_string(),
        tier,
        &ctx,
        &var_ctx,
        personality,
        needs,
        &mut rng.rng,
    );
}

// ===========================================================================
// Spatial helpers (transplanted from disposition.rs)
// ===========================================================================

fn patrol_move(pos: &Position, dx: i32, dy: i32, map: &TileMap) -> Position {
    let primary = Position::new(pos.x + dx, pos.y + dy);
    if map.in_bounds(primary.x, primary.y) && map.get(primary.x, primary.y).terrain.is_passable() {
        return primary;
    }
    let perp = Position::new(pos.x + dy, pos.y + dx);
    if map.in_bounds(perp.x, perp.y) && map.get(perp.x, perp.y).terrain.is_passable() {
        return perp;
    }
    let rev = Position::new(pos.x - dx, pos.y - dy);
    if map.in_bounds(rev.x, rev.y) && map.get(rev.x, rev.y).terrain.is_passable() {
        return rev;
    }
    *pos
}

fn has_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> bool {
    find_nearest_tile(from, map, radius, predicate).is_some()
}

fn find_nearest_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> Option<Position> {
    let mut best: Option<(Position, i32)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let p = Position::new(from.x + dx, from.y + dy);
            if map.in_bounds(p.x, p.y) {
                let tile = map.get(p.x, p.y);
                if predicate(tile.terrain) {
                    let dist = from.manhattan_distance(&p);
                    if dist > 0 && best.is_none_or(|(_, d)| dist < d) {
                        best = Some((p, dist));
                    }
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

fn find_random_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
    rng: &mut impl Rng,
) -> Option<Position> {
    let mut candidates: Vec<(Position, f32)> = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let p = Position::new(from.x + dx, from.y + dy);
            if map.in_bounds(p.x, p.y) {
                let tile = map.get(p.x, p.y);
                if predicate(tile.terrain) {
                    let dist = from.manhattan_distance(&p);
                    if dist > 0 {
                        candidates.push((p, 1.0 / (dist as f32 * dist as f32)));
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let total: f32 = candidates.iter().map(|(_, w)| w).sum();
    let mut roll: f32 = rng.random::<f32>() * total;
    for (pos, weight) in &candidates {
        roll -= weight;
        if roll <= 0.0 {
            return Some(*pos);
        }
    }
    Some(candidates.last().unwrap().0)
}

fn respect_for_disposition(kind: DispositionKind, d: &DispositionConstants) -> f32 {
    match kind {
        DispositionKind::Hunting => d.respect_gain_hunting,
        DispositionKind::Foraging => d.respect_gain_foraging,
        DispositionKind::Guarding => d.respect_gain_guarding,
        DispositionKind::Building => d.respect_gain_building,
        DispositionKind::Coordinating => d.respect_gain_coordinating,
        DispositionKind::Socializing => d.respect_gain_socializing,
        _ => 0.0,
    }
}

// ===========================================================================
// Zone resolution and planner state construction
// ===========================================================================

#[allow(dead_code, clippy::type_complexity)]
fn find_nearest_store(
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
) -> Option<Entity> {
    building_query
        .iter()
        .filter(|(_, s, _, _, _)| s.kind == StructureType::Stores)
        .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
        .map(|(e, _, _, _, _)| e)
}

fn find_social_target(
    cat_entity: Entity,
    pos: &Position,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    d: &DispositionConstants,
) -> Option<Entity> {
    cat_positions
        .iter()
        .filter(|(other, other_pos)| {
            *other != cat_entity && pos.manhattan_distance(other_pos) <= d.social_target_range
        })
        .max_by(|(a, _), (b, _)| {
            let fa = relationships
                .get(cat_entity, *a)
                .map_or(0.0, |r| r.fondness);
            let fb = relationships
                .get(cat_entity, *b)
                .map_or(0.0, |r| r.fondness);
            fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _)| *e)
}

#[allow(clippy::too_many_arguments)]
fn resolve_zone_position(
    zone: PlannerZone,
    pos: &Position,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    cat_entity: Entity,
    d: &DispositionConstants,
) -> Option<Position> {
    match zone {
        PlannerZone::Stores => stores_positions
            .iter()
            .min_by_key(|sp| pos.manhattan_distance(sp))
            .copied(),
        PlannerZone::HuntingGround => {
            find_nearest_tile(pos, map, d.hunt_terrain_search_radius, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            })
        }
        PlannerZone::ForagingGround => {
            find_nearest_tile(pos, map, d.forage_terrain_search_radius, |t| {
                t.foraging_yield() > 0.0
            })
        }
        PlannerZone::Farm => farm_positions
            .iter()
            .min_by_key(|fp| pos.manhattan_distance(fp))
            .copied(),
        PlannerZone::ConstructionSite => construction_positions
            .iter()
            .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
            .map(|(_, p)| *p),
        PlannerZone::HerbPatch => herb_positions
            .iter()
            .min_by_key(|(_, hp, _)| pos.manhattan_distance(hp))
            .map(|(_, p, _)| *p),
        PlannerZone::Kitchen => kitchen_positions
            .iter()
            .min_by_key(|kp| pos.manhattan_distance(kp))
            .copied(),
        PlannerZone::RestingSpot => stores_positions
            .iter()
            .min_by_key(|sp| pos.manhattan_distance(sp))
            .map(|sp| Position::new(sp.x + 1, sp.y))
            .or(Some(*pos)),
        PlannerZone::SocialTarget => cat_positions
            .iter()
            .filter(|(other, _)| *other != cat_entity)
            .min_by_key(|(_, op)| pos.manhattan_distance(op))
            .map(|(_, p)| *p),
        PlannerZone::Wilds => find_nearest_tile(pos, map, 20, |t| t.is_passable()).or(Some(*pos)),
        PlannerZone::PatrolZone => stores_positions
            .iter()
            .min_by_key(|sp| pos.manhattan_distance(sp))
            .map(|sp| Position::new(sp.x + d.guard_patrol_radius as i32, sp.y))
            .or(Some(*pos)),
    }
}

/// Find the most corrupted tile within `radius` tiles of `origin`.
/// Returns `None` if no tile has corruption above 0.05.
fn nearest_corrupted_tile(origin: &Position, map: &TileMap, radius: i32) -> Option<Position> {
    let mut best: Option<(Position, f32)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs() + dy.abs() > radius {
                continue;
            }
            let p = Position::new(origin.x + dx, origin.y + dy);
            if !map.in_bounds(p.x, p.y) {
                continue;
            }
            let c = map.get(p.x, p.y).corruption;
            if c > 0.05 && best.as_ref().is_none_or(|(_, bc)| c > *bc) {
                best = Some((p, c));
            }
        }
    }
    best.map(|(p, _)| p)
}

#[allow(clippy::too_many_arguments)]
fn build_planner_state(
    pos: &Position,
    needs: &Needs,
    inventory: &Inventory,
    trips_done: u32,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    d: &DispositionConstants,
) -> PlannerState {
    let zone = classify_zone(
        pos,
        map,
        stores_positions,
        construction_positions,
        farm_positions,
        herb_positions,
    );
    let carrying = if inventory
        .slots
        .iter()
        .any(|s| matches!(s, crate::components::magic::ItemSlot::Item(k, _) if k.is_food()))
    {
        if inventory.slots.iter().any(|s| {
            matches!(
                s,
                crate::components::magic::ItemSlot::Item(
                    crate::components::items::ItemKind::RawMouse
                        | crate::components::items::ItemKind::RawRat
                        | crate::components::items::ItemKind::RawBird
                        | crate::components::items::ItemKind::RawFish
                        | crate::components::items::ItemKind::RawRabbit,
                    _
                )
            )
        }) {
            Carrying::Prey
        } else {
            Carrying::ForagedFood
        }
    } else if inventory
        .slots
        .iter()
        .any(|s| matches!(s, crate::components::magic::ItemSlot::Herb(_)))
    {
        Carrying::Herbs
    } else {
        Carrying::Nothing
    };

    PlannerState {
        zone,
        carrying,
        trips_done,
        hunger_ok: needs.hunger >= d.planner_hunger_ok_threshold,
        energy_ok: needs.energy >= d.planner_energy_ok_threshold,
        temperature_ok: needs.temperature >= d.planner_temperature_ok_threshold,
        interaction_done: false,
        construction_done: false,
        prey_found: false,
        farm_tended: false,
        thornbriar_available: herb_positions
            .iter()
            .any(|(_, _, kind)| *kind == HerbKind::Thornbriar),
    }
}

fn classify_zone(
    pos: &Position,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
) -> PlannerZone {
    if stores_positions
        .iter()
        .any(|sp| pos.manhattan_distance(sp) <= 2)
    {
        return PlannerZone::Stores;
    }
    if construction_positions
        .iter()
        .any(|(_, cp)| pos.manhattan_distance(cp) <= 2)
    {
        return PlannerZone::ConstructionSite;
    }
    if farm_positions
        .iter()
        .any(|fp| pos.manhattan_distance(fp) <= 2)
    {
        return PlannerZone::Farm;
    }
    if herb_positions
        .iter()
        .any(|(_, hp, _)| pos.manhattan_distance(hp) <= 3)
    {
        return PlannerZone::HerbPatch;
    }
    if map.in_bounds(pos.x, pos.y) {
        let terrain = map.get(pos.x, pos.y).terrain;
        if matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
            return PlannerZone::HuntingGround;
        }
        if terrain.foraging_yield() > 0.0 {
            return PlannerZone::ForagingGround;
        }
    }
    PlannerZone::Wilds
}

#[allow(clippy::too_many_arguments)]
fn build_zone_distances(
    pos: &Position,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    cat_entity: Entity,
    d: &DispositionConstants,
) -> ZoneDistances {
    let mut distances = ZoneDistances::default();

    let zone_positions: Vec<(PlannerZone, Option<Position>)> = vec![
        (
            PlannerZone::Stores,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.manhattan_distance(sp))
                .copied(),
        ),
        (
            PlannerZone::HuntingGround,
            find_nearest_tile(pos, map, d.hunt_terrain_search_radius, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            }),
        ),
        (
            PlannerZone::ForagingGround,
            find_nearest_tile(pos, map, d.forage_terrain_search_radius, |t| {
                t.foraging_yield() > 0.0
            }),
        ),
        (
            PlannerZone::Farm,
            farm_positions
                .iter()
                .min_by_key(|fp| pos.manhattan_distance(fp))
                .copied(),
        ),
        (
            PlannerZone::ConstructionSite,
            construction_positions
                .iter()
                .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
                .map(|(_, p)| *p),
        ),
        (
            PlannerZone::HerbPatch,
            herb_positions
                .iter()
                .min_by_key(|(_, hp, _)| pos.manhattan_distance(hp))
                .map(|(_, p, _)| *p),
        ),
        (
            PlannerZone::Kitchen,
            kitchen_positions
                .iter()
                .min_by_key(|kp| pos.manhattan_distance(kp))
                .copied(),
        ),
        (
            PlannerZone::RestingSpot,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.manhattan_distance(sp))
                .map(|sp| Position::new(sp.x + 1, sp.y)),
        ),
        (
            PlannerZone::SocialTarget,
            cat_positions
                .iter()
                .filter(|(other, _)| *other != cat_entity)
                .min_by_key(|(_, op)| pos.manhattan_distance(op))
                .map(|(_, p)| *p),
        ),
        (PlannerZone::Wilds, Some(*pos)),
        (
            PlannerZone::PatrolZone,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.manhattan_distance(sp))
                .map(|sp| Position::new(sp.x + d.guard_patrol_radius as i32, sp.y)),
        ),
    ];

    // Build pairwise distances between reachable zones.
    for &(from_zone, from_pos) in &zone_positions {
        let Some(fp) = from_pos else { continue };
        for &(to_zone, to_pos) in &zone_positions {
            if from_zone == to_zone {
                continue;
            }
            let Some(tp) = to_pos else { continue };
            let dist = fp.manhattan_distance(&tp) as u32;
            let cost = (dist / 3).max(1); // Scale down: 3 tiles ≈ 1 planning cost.
            distances.set(from_zone, to_zone, cost);
        }
    }

    distances
}
