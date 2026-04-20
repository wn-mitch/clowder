use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::{find_free_adjacent, step_toward};
use crate::ai::scoring::{
    aggregate_to_dispositions, apply_aspiration_bonuses, apply_cascading_bonuses,
    apply_colony_knowledge_bonuses, apply_directive_bonus, apply_fated_bonuses,
    apply_memory_bonuses, apply_preference_bonuses, apply_priority_bonus, enforce_survival_floor,
    score_actions, select_disposition_softmax, ScoringContext,
};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::coordination::{ActiveDirective, Directive, DirectiveKind, DirectiveQueue};
use crate::components::disposition::{
    ActionHistory, ActionOutcome, ActionRecord, CraftingHint, Disposition, DispositionKind,
};
use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::items::Item;
use crate::components::magic::{Harvestable, Herb, Inventory, Ward};
use crate::components::mental::Memory;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, InjuryKind, Needs, Position};
use crate::components::prey::{
    DenRaided, PreyAnimal, PreyConfig, PreyDen, PreyDensity, PreyKilled, PreyState,
};
use crate::components::skills::{MagicAffinity, Skills};
use crate::components::task_chain::{FailurePolicy, StepKind, TaskChain, TaskStep};
use crate::components::wildlife::WildAnimal;
use crate::resources::colony_hunting_map::ColonyHuntingMap;

/// Bundled system params for prey-related data in `resolve_disposition_chains`.
/// Avoids hitting Bevy's 16-parameter system limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct PreyHuntParams<'w, 's> {
    pub density: Res<'w, PreyDensity>,
    pub kill_writer: MessageWriter<'w, PreyKilled>,
    pub raid_writer: MessageWriter<'w, DenRaided>,
    pub exploration_map: ResMut<'w, crate::resources::ExplorationMap>,
    /// Health lookup for FightThreat bail-out. Lives here to stay under the
    /// 16-system-param limit — conceptually unrelated to prey hunting.
    pub health_query: Query<'w, 's, &'static Health>,
}
/// Bundled system params for narrative emission in `resolve_disposition_chains`.
/// Groups NarrativeLog + optional TemplateRegistry + context resources to stay
/// under Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct NarrativeEmitter<'w> {
    pub log: ResMut<'w, crate::resources::narrative::NarrativeLog>,
    pub registry: Option<Res<'w, crate::resources::narrative_templates::TemplateRegistry>>,
    pub config: Res<'w, crate::resources::time::SimConfig>,
    pub weather: Res<'w, crate::resources::weather::WeatherState>,
    pub activation: Option<ResMut<'w, SystemActivation>>,
}
use crate::resources::food::FoodStores;
use crate::resources::fox_scent_map::FoxScentMap;
use crate::resources::map::{Terrain, TileMap};

/// Bundled read-only resources for `disposition_to_chain`.
/// Groups map, food, relationships, and fox scent map to stay under Bevy's
/// 16-parameter system limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct ChainResources<'w> {
    pub map: Res<'w, TileMap>,
    pub food: Res<'w, FoodStores>,
    pub relationships: Res<'w, Relationships>,
    pub fox_scent_map: Res<'w, FoxScentMap>,
    /// Mutable ledger of frustrated action desires — chain builders record
    /// misses here so the coordinator's BuildPressure can respond.
    pub unmet_demand: ResMut<'w, crate::resources::UnmetDemand>,
}

use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, VariableContext,
};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{DispositionConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, TimeState};

// ===========================================================================
// check_anxiety_interrupts
// ===========================================================================

/// Checks every tick whether a cat's disposition should be interrupted by
/// critical need states or threats. Runs BEFORE disposition evaluation.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_anxiety_interrupts(
    mut query: Query<
        (
            Entity,
            &Needs,
            &Personality,
            &Position,
            &Health,
            &mut CurrentAction,
            Option<&mut ActionHistory>,
        ),
        (With<Disposition>, Without<Dead>),
    >,
    dispositions: Query<&Disposition, Without<Dead>>,
    wildlife: Query<&Position, With<WildAnimal>>,
    time: Res<TimeState>,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
) {
    let d = &constants.disposition;
    for (entity, needs, personality, pos, health, mut current, history) in &mut query {
        let Ok(disposition) = dispositions.get(entity) else {
            continue;
        };

        let interrupt = check_interrupt(
            needs,
            personality,
            pos,
            health,
            disposition,
            &wildlife,
            d,
            &constants.sensory.cat,
        );
        let Some(reason) = interrupt else { continue };

        activation.record(Feature::AnxietyInterrupt);

        // Don't interrupt cats actively hunting (even if Resting disposition).
        if matches!(reason, InterruptReason::ThreatDetected { .. })
            && current.action == Action::Hunt
        {
            continue;
        }

        // Record the interruption in action history.
        if let Some(mut hist) = history {
            hist.record(ActionRecord {
                action: current.action,
                disposition: Some(disposition.kind),
                tick: time.tick,
                outcome: ActionOutcome::Interrupted,
            });
        }

        // Strip disposition and chain.
        commands.entity(entity).remove::<Disposition>();
        commands.entity(entity).remove::<TaskChain>();

        match reason {
            InterruptReason::ThreatDetected { threat_pos } => {
                // Immediate flee — don't wait for re-evaluation.
                let dx = pos.x - threat_pos.x;
                let dy = pos.y - threat_pos.y;
                let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                let mut target = Position::new(
                    pos.x + (dx as f32 / len * d.flee_distance) as i32,
                    pos.y + (dy as f32 / len * d.flee_distance) as i32,
                );
                target.x = target.x.clamp(0, map.width - 1);
                target.y = target.y.clamp(0, map.height - 1);
                current.action = Action::Flee;
                current.ticks_remaining = 0;
                current.target_position = Some(target);
                current.target_entity = None;
            }
            _ => {
                // Let the cat re-evaluate next tick.
                current.ticks_remaining = 0;
            }
        }
    }
}

#[derive(Debug)]
enum InterruptReason {
    Starvation,
    Exhaustion,
    ThreatDetected { threat_pos: Position },
    CriticalSafety,
    CriticalHealth,
}

#[allow(clippy::too_many_arguments)]
fn check_interrupt(
    needs: &Needs,
    personality: &Personality,
    pos: &Position,
    health: &Health,
    disposition: &Disposition,
    wildlife: &Query<&Position, With<WildAnimal>>,
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
) -> Option<InterruptReason> {
    // Critical health check — fires for ALL dispositions, including Guarding.
    // A cat below the health threshold must re-evaluate immediately.
    if health.current / health.max < d.critical_health_threshold {
        return Some(InterruptReason::CriticalHealth);
    }

    // Resting, Hunting, and Foraging are exempt from hunger interrupts.
    // Resting is already handling it; Hunting/Foraging ARE the food solution.
    if !matches!(
        disposition.kind,
        DispositionKind::Resting | DispositionKind::Hunting | DispositionKind::Foraging
    ) {
        if needs.hunger < d.starvation_interrupt_threshold {
            return Some(InterruptReason::Starvation);
        }
        if needs.energy < d.exhaustion_interrupt_threshold {
            return Some(InterruptReason::Exhaustion);
        }
    }

    // Guards are exempt from threat interrupts — they handle threats directly
    // via guard_threat_detection_range.
    if !matches!(disposition.kind, DispositionKind::Guarding) {
        // Check for nearby wildlife threats. Phase 2 migration: the
        // visual-only detection path now flows through the sensory
        // model's sight channel. See `cat_sees_threat_at`.
        let nearest_threat = wildlife
            .iter()
            .filter(|wp| crate::systems::sensing::cat_sees_threat_at(*pos, cat_profile, **wp))
            .min_by_key(|wp| pos.manhattan_distance(wp));

        if let Some(threat_pos) = nearest_threat {
            let dist = pos.manhattan_distance(threat_pos) as f32;
            let threat_urgency = 1.0 - (dist / d.threat_urgency_divisor);
            // Bold cats resist fleeing: threshold scales with boldness.
            let flee_threshold =
                d.flee_threshold_base + personality.boldness * d.flee_threshold_boldness_scale;
            if threat_urgency > flee_threshold {
                return Some(InterruptReason::ThreatDetected {
                    threat_pos: *threat_pos,
                });
            }
        }
    }

    // Critical safety check — guards are no longer exempt. A guard with
    // critically low safety should re-evaluate rather than standing in a
    // fight that's draining them.
    if needs.safety < d.critical_safety_threshold {
        return Some(InterruptReason::CriticalSafety);
    }

    None
}

// ===========================================================================
// evaluate_dispositions
// ===========================================================================

/// Bundle of side-effect params for evaluate_dispositions. Collapses commands,
/// rng, and the mating-eligibility snapshot into one SystemParam slot so the
/// outer function stays under Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct EvalDispositionSideEffects<'w, 's> {
    pub rng: ResMut<'w, SimRng>,
    pub commands: Commands<'w, 's>,
    pub mating: crate::ai::mating::MatingFitnessParams<'w, 's>,
}

/// Read-only queries over stored-item state. Bundled into a SystemParam so
/// the cat scoring systems (evaluate_dispositions, evaluate_and_plan) can
/// derive cooking eligibility without blowing Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct CookingQueries<'w, 's> {
    pub stored_items: Query<'w, 's, &'static StoredItems>,
    pub items: Query<'w, 's, &'static Item>,
}

impl CookingQueries<'_, '_> {
    /// True if any Stores building currently holds at least one uncooked
    /// food item.
    pub fn has_raw_food_in_stores(&self) -> bool {
        self.stored_items.iter().any(|stored| {
            stored.items.iter().copied().any(|e| {
                self.items
                    .get(e)
                    .is_ok_and(|it| it.kind.is_food() && !it.modifiers.cooked)
            })
        })
    }
}

/// For cats without a Disposition whose action has finished: score all actions,
/// aggregate to dispositions, select via softmax, insert Disposition component.
///
/// This replaces evaluate_actions for disposition-driven cats.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn evaluate_dispositions(
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
        (Without<Dead>, Without<Disposition>),
    >,
    all_positions: Query<(Entity, &Position, Option<&PreyAnimal>), Without<Dead>>,
    wildlife: Query<(Entity, &Position), With<WildAnimal>>,
    building_query: Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    herb_query: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    ward_query: Query<(&Ward, &Position)>,
    directive_queue_query: Query<(Entity, &DirectiveQueue)>,
    active_directive_query: Query<&ActiveDirective>,
    skills_query: Query<&Skills, Without<Dead>>,
    map: Res<TileMap>,
    food: Res<FoodStores>,
    relationships: Res<Relationships>,
    colony: super::ColonyContext,
    constants: Res<SimConstants>,
    cooking: CookingQueries,
    mut side_effects: EvalDispositionSideEffects,
) {
    let rng = &mut *side_effects.rng;
    let commands = &mut side_effects.commands;
    let mating_fitness_params = &side_effects.mating;
    let sc = &constants.scoring;
    let d = &constants.disposition;
    let food_available = !food.is_empty();
    let food_fraction = food.fraction();

    // Collect positions once.
    let mut cat_positions: Vec<(Entity, Position)> = Vec::new();
    let mut prey_positions: Vec<Position> = Vec::new();
    for (e, p, prey) in all_positions.iter() {
        cat_positions.push((e, *p));
        if prey.is_some() {
            prey_positions.push(*p);
        }
    }

    let wildlife_positions: Vec<(Entity, Position)> =
        wildlife.iter().map(|(e, p)| (e, *p)).collect();

    let has_construction_site = building_query
        .iter()
        .any(|(_, _, _, site, _)| site.is_some());
    let has_damaged_building = building_query
        .iter()
        .any(|(_, s, _, site, _)| site.is_none() && s.condition < d.damaged_building_threshold);
    let has_garden = building_query
        .iter()
        .any(|(_, s, _, site, _)| s.kind == StructureType::Garden && site.is_none());
    let has_functional_kitchen = building_query.iter().any(|(_, s, _, site, _)| {
        s.kind == StructureType::Kitchen && site.is_none() && s.effectiveness() > 0.0
    });
    let has_raw_food_in_stores = cooking.has_raw_food_in_stores();

    let herb_positions: Vec<(Entity, Position)> =
        herb_query.iter().map(|(e, _, p)| (e, *p)).collect();
    let thornbriar_available = herb_query
        .iter()
        .any(|(_, h, _)| h.kind == crate::components::magic::HerbKind::Thornbriar);

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 =
                ward_query.iter().map(|(w, _)| w.strength).sum::<f32>() / ward_count as f32;
            avg < d.ward_strength_low_threshold
        }
    };

    let colony_injury_count = query
        .iter()
        .filter(|(_, _, _, _, _, _, _, health, _, _, _, _, _, _, _)| health.current < 1.0)
        .count();

    let directive_snapshot: HashMap<Entity, (usize, Option<Directive>)> = directive_queue_query
        .iter()
        .map(|(entity, q)| (entity, (q.directives.len(), q.directives.first().cloned())))
        .collect();

    // Snapshot per-cat fields needed by the mating eligibility gate.
    let mating_fitness = mating_fitness_params.snapshot();
    let current_season = mating_fitness_params.current_season();
    let current_day_phase = mating_fitness_params.current_day_phase();

    // Snapshot current actions for activity cascading.
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
                    &constants.sensory.cat,
                    *other_pos,
                    crate::components::SensorySignature::CAT,
                    d.mentoring_detection_range as f32,
                )
                && skills_query.get(*other).is_ok_and(|other_skills| {
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

    for (
        entity,
        _name,
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

        let can_hunt = has_nearby_tile(pos, &map, d.hunt_terrain_search_radius, |t| {
            matches!(t, Terrain::DenseForest | Terrain::LightForest)
        });
        let can_forage = has_nearby_tile(pos, &map, d.forage_terrain_search_radius, |t| {
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

        let has_herbs_nearby = herb_positions.iter().any(|(_, hp)| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                &constants.sensory.cat,
                *hp,
                crate::components::SensorySignature::PREY,
                d.herb_detection_range as f32,
            )
        });

        let prey_nearby = prey_positions.iter().any(|pp| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                &constants.sensory.cat,
                *pp,
                crate::components::SensorySignature::PREY,
                d.prey_detection_range as f32,
            )
        });

        let (on_corrupted_tile, tile_corruption, on_special_terrain) =
            if map.in_bounds(pos.x, pos.y) {
                let tile = map.get(pos.x, pos.y);
                (
                    tile.corruption > d.corrupted_tile_threshold,
                    tile.corruption,
                    matches!(tile.terrain, Terrain::FairyRing | Terrain::StandingStone),
                )
            } else {
                (false, 0.0, false)
            };

        // Check if an eligible mating partner exists. Uses the Spring-only,
        // sated-and-happy gate from `crate::ai::mating`.
        let has_eligible_mate = crate::ai::mating::has_eligible_mate(
            entity,
            needs.mating,
            current_season,
            sc,
            &mating_fitness,
            &cat_positions,
            &relationships,
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
                .any(|s| matches!(s, crate::components::ItemSlot::Herb(_))),
            has_remedy_herbs: inventory.has_remedy_herb(),
            has_ward_herbs: inventory.has_ward_herb(),
            thornbriar_available,
            colony_injury_count,
            ward_strength_low,
            on_corrupted_tile,
            tile_corruption,
            nearby_corruption_level: 0.0, // legacy disposition path — not wired yet
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
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: current_day_phase,
            has_functional_kitchen,
            has_raw_food_in_stores,
        };

        let result = score_actions(&ctx, &mut rng.rng);
        let mut scores = result.scores;

        // Apply all bonus layers (identical to evaluate_actions).
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
                    &constants.sensory.cat,
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
                    &constants.sensory.cat,
                    *rp,
                    crate::components::SensorySignature::CAT,
                    d.fated_rival_detection_range as f32,
                )
            });
        apply_fated_bonuses(&mut scores, love_visible, rival_nearby, sc);
        if let Ok(directive) = active_directive_query.get(entity) {
            let fondness_factor = relationships
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

        // Determine Groom routing.
        let self_groom_score =
            (1.0 - needs.warmth) * sc.self_groom_warmth_scale * needs.level_suppression(1);
        let other_groom_score = if has_social_target {
            personality.warmth * (1.0 - needs.social) * needs.level_suppression(2)
        } else {
            0.0
        };
        let self_groom_won = self_groom_score >= other_groom_score;

        // Aggregate action scores to disposition scores.
        let mut disposition_scores = aggregate_to_dispositions(&scores, self_groom_won);

        // Independence: penalize group-oriented dispositions.
        for (kind, score) in disposition_scores.iter_mut() {
            if matches!(
                kind,
                DispositionKind::Coordinating | DispositionKind::Socializing
            ) {
                *score = (*score - personality.independence * d.disposition_independence_penalty)
                    .max(0.0);
            }
        }

        // Select disposition via softmax.
        let chosen = select_disposition_softmax(&disposition_scores, &mut rng.rng, sc);

        // Store all gate-open action scores for diagnostics (unchanged from
        // evaluate_actions). Truncation removed 2026-04-20 to match goap.rs.
        {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            current.last_scores = sorted;
        }

        // Insert the Disposition component. Chain creation happens in disposition_to_chain.
        // adopted_tick is 0 here; resolve_disposition_chains will set it from TimeState.
        let crafting_hint = if chosen == DispositionKind::Crafting {
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
            let cook_score = scores
                .iter()
                .find(|(a, _)| *a == Action::Cook)
                .map(|(_, s)| *s)
                .unwrap_or(0.0);
            if cook_score > herbcraft_score && cook_score > magic_score {
                Some(CraftingHint::Cook)
            } else if magic_score > herbcraft_score {
                result.magic_hint.or(Some(CraftingHint::Magic))
            } else {
                result.herbcraft_hint
            }
        } else {
            None
        };
        let mut disp = Disposition::new(chosen, 0, personality);
        disp.crafting_hint = crafting_hint;
        commands.entity(entity).insert(disp);

        // Keep ticks_remaining = 0 so disposition_to_chain picks it up this tick.
    }
}

/// Find nearest tile matching a predicate within radius.
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

fn has_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> bool {
    find_nearest_tile(from, map, radius, predicate).is_some()
}

/// Find a random tile matching `predicate` within `radius`, weighted by inverse
/// distance so closer tiles are more likely but cats don't all converge on the
/// same nearest tile.
#[allow(dead_code)]
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
                        // Weight: inverse distance squared — close tiles strongly
                        // preferred, but not deterministically.
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

// ===========================================================================
// disposition_to_chain
// ===========================================================================

/// For cats with a Disposition but no TaskChain: create the appropriate chain
/// based on disposition kind and current world state.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn disposition_to_chain(
    mut query: Query<
        (
            Entity,
            &Needs,
            &Personality,
            &Position,
            &Memory,
            &Skills,
            &MagicAffinity,
            &Inventory,
            &Health,
            &mut Disposition,
            &mut CurrentAction,
        ),
        (With<Disposition>, Without<Dead>, Without<TaskChain>),
    >,
    cat_positions: Query<(Entity, &Position), Without<Dead>>,
    wildlife: Query<(Entity, &Position), With<WildAnimal>>,
    building_query: Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    herb_query: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    ward_query: Query<(&Ward, &Position)>,
    directive_queue_query: Query<(Entity, &DirectiveQueue)>,
    active_directive_query: Query<&ActiveDirective>,
    skills_query: Query<&Skills, Without<Dead>>,
    injured_cat_query: Query<(Entity, &Health, &Position), Without<Dead>>,
    res: ChainResources,
    constants: Res<SimConstants>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
) {
    let d = &constants.disposition;
    // Pre-collect cat position pairs for social target selection.
    let cat_pos_list: Vec<(Entity, Position)> =
        cat_positions.iter().map(|(e, p)| (e, *p)).collect();

    // Anti-stacking: tiles that already have a cat on them.
    let occupied_tiles: std::collections::HashSet<Position> =
        cat_pos_list.iter().map(|(_, p)| *p).collect();

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 =
                ward_query.iter().map(|(w, _)| w.strength).sum::<f32>() / ward_count as f32;
            avg < d.ward_strength_low_threshold
        }
    };

    // Pre-collect injured cat positions for herbcraft targeting.
    let injured_cat_list: Vec<(Entity, Position)> = injured_cat_query
        .iter()
        .filter(|(_, h, _)| h.current < 1.0)
        .map(|(e, _, p)| (e, *p))
        .collect();

    for (
        entity,
        needs,
        personality,
        pos,
        memory,
        skills,
        magic_aff,
        inventory,
        health,
        disposition,
        mut current,
    ) in &mut query
    {
        // Check completion FIRST: if the disposition is already done, remove it.
        if should_complete_disposition(&disposition, needs, d) {
            commands.entity(entity).remove::<Disposition>();
            current.ticks_remaining = 0;
            continue;
        }

        // Pre-compute nearest Stores building for chains that need a return leg.
        let nearest_store = building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Stores && site.is_none())
            .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
            .map(|(e, _, bp, _, _)| (e, *bp));

        let chain = match disposition.kind {
            DispositionKind::Resting => build_resting_chain(
                needs,
                pos,
                &building_query,
                &res.map,
                nearest_store,
                res.food.is_empty(),
                d,
                &mut rng.rng,
            ),
            DispositionKind::Hunting => {
                build_hunting_chain(pos, memory, &res.map, nearest_store, &mut rng.rng)
            }
            DispositionKind::Foraging => {
                build_foraging_chain(pos, &res.map, nearest_store, &mut rng.rng)
            }
            DispositionKind::Guarding => build_guarding_chain(
                pos,
                health,
                &wildlife,
                &res.map,
                Some(&res.fox_scent_map),
                d,
                &constants.sensory.cat,
                &mut rng.rng,
            ),
            DispositionKind::Socializing => build_socializing_chain(
                entity,
                pos,
                personality,
                skills,
                &cat_pos_list,
                &res.relationships,
                &skills_query,
                d,
            ),
            DispositionKind::Building => {
                build_building_chain(entity, pos, &building_query, d, &mut commands)
            }
            DispositionKind::Farming => build_farming_chain(pos, &building_query),
            DispositionKind::Crafting => build_crafting_chain(
                pos,
                personality,
                needs,
                skills,
                magic_aff,
                inventory,
                &herb_query,
                &building_query,
                &ward_query,
                &cat_pos_list,
                &injured_cat_list,
                &res.map,
                ward_strength_low,
                d,
                &mut rng.rng,
                disposition.crafting_hint,
            ),
            DispositionKind::Coordinating => build_coordinating_chain(
                entity,
                pos,
                &directive_queue_query,
                &active_directive_query,
                &cat_pos_list,
                &skills_query,
                d,
                &mut commands,
            ),
            DispositionKind::Exploring => build_exploring_chain(pos, &res.map, d, &mut rng.rng),
            DispositionKind::Mating => build_mating_chain(
                entity,
                pos,
                personality,
                &cat_pos_list,
                &res.relationships,
                d,
            ),
            DispositionKind::Caretaking => {
                build_caretaking_chain(entity, pos, personality, nearest_store, d)
            }
        };

        if let Some((mut chain, action)) = chain {
            // Anti-stacking: jitter MoveTo/PatrolTo destinations away from
            // tiles that already have a cat on them.
            if d.anti_stack_jitter {
                for step in &mut chain.steps {
                    if matches!(step.kind, StepKind::MoveTo | StepKind::PatrolTo) {
                        if let Some(ref mut target) = step.target_position {
                            if occupied_tiles.contains(target) {
                                if let Some(free) =
                                    find_free_adjacent(*target, *pos, &res.map, &occupied_tiles)
                                {
                                    *target = free;
                                }
                            }
                        }
                    }
                }
            }
            current.action = action;
            current.ticks_remaining = u64::MAX;
            current.target_position = chain.steps.first().and_then(|s| s.target_position);
            current.target_entity = chain.steps.first().and_then(|s| s.target_entity);
            commands.entity(entity).insert(chain);
        } else {
            // No valid chain could be built — remove disposition and idle.
            commands.entity(entity).remove::<Disposition>();
            current.action = Action::Idle;
            current.ticks_remaining = 0;
            current.target_position = None;
            current.target_entity = None;
        }
    }
}

/// Check whether a disposition's goal is met and should be cleared.
fn should_complete_disposition(
    disposition: &Disposition,
    needs: &Needs,
    d: &DispositionConstants,
) -> bool {
    match disposition.kind {
        DispositionKind::Resting => {
            needs.hunger >= d.resting_complete_hunger
                && needs.energy >= d.resting_complete_energy
                && needs.warmth >= d.resting_complete_warmth
        }
        _ => disposition.is_count_complete(),
    }
}

/// Respect earned on successful disposition completion.
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

// ---------------------------------------------------------------------------
// Chain builders — one per disposition
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_resting_chain(
    needs: &Needs,
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    _map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    food_empty: bool,
    d: &DispositionConstants,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // Pick the most urgent physiological need.
    let hunger_deficit = 1.0 - needs.hunger;
    let energy_deficit = 1.0 - needs.energy;
    let warmth_deficit = 1.0 - needs.warmth;

    if hunger_deficit >= energy_deficit && hunger_deficit >= warmth_deficit {
        if food_empty {
            // Stores are empty — hunt for food instead of walking to empty stores.
            let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
            let mut steps = vec![TaskStep::new(StepKind::HuntPrey { patrol_dir })];
            if let Some((store_entity, store_pos)) = nearest_store {
                steps.push(TaskStep::new(StepKind::MoveTo).with_position(store_pos));
                steps.push(
                    TaskStep::new(StepKind::DepositAtStores)
                        .with_position(store_pos)
                        .with_entity(store_entity),
                );
            }
            let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
            return Some((chain, Action::Hunt));
        }

        // Eat: walk to nearest Stores building.
        let nearest_store = building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Stores && site.is_none())
            .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
            .map(|(e, _, bp, _, _)| (e, *bp));

        if let Some((store_entity, store_pos)) = nearest_store {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(store_pos),
                    TaskStep::new(StepKind::EatAtStores)
                        .with_position(store_pos)
                        .with_entity(store_entity),
                ],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::Eat))
        } else {
            // No stores — just idle.
            None
        }
    } else if energy_deficit >= warmth_deficit {
        // Sleep in place.
        let sleep_ticks = ((1.0 - needs.energy) * d.sleep_duration_deficit_multiplier) as u64
            + d.sleep_duration_base;
        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::Sleep { ticks: sleep_ticks }).with_position(*pos)],
            FailurePolicy::AbortChain,
        );
        Some((chain, Action::Sleep))
    } else {
        // Self-groom.
        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::SelfGroom).with_position(*pos)],
            FailurePolicy::AbortChain,
        );
        Some((chain, Action::Groom))
    }
}

fn build_hunting_chain(
    _pos: &Position,
    _memory: &Memory,
    _map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // HuntPrey handles all movement internally (scent search → stalk → pounce).
    // No MoveTo preamble — the cat starts hunting from its current position.
    let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
    let mut steps = vec![TaskStep::new(StepKind::HuntPrey { patrol_dir })];
    if let Some((store_entity, store_pos)) = nearest_store {
        steps.push(TaskStep::new(StepKind::MoveTo).with_position(store_pos));
        steps.push(
            TaskStep::new(StepKind::DepositAtStores)
                .with_position(store_pos)
                .with_entity(store_entity),
        );
    }
    let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
    Some((chain, Action::Hunt))
}

fn build_foraging_chain(
    _pos: &Position,
    _map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // ForageItem handles all movement internally (directional patrol + tile checks).
    let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
    let mut steps = vec![TaskStep::new(StepKind::ForageItem { patrol_dir })];
    if let Some((store_entity, store_pos)) = nearest_store {
        steps.push(TaskStep::new(StepKind::MoveTo).with_position(store_pos));
        steps.push(
            TaskStep::new(StepKind::DepositAtStores)
                .with_position(store_pos)
                .with_entity(store_entity),
        );
    }
    let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
    Some((chain, Action::Forage))
}

#[allow(clippy::too_many_arguments)]
fn build_guarding_chain(
    pos: &Position,
    health: &Health,
    wildlife: &Query<(Entity, &Position), With<WildAnimal>>,
    map: &TileMap,
    fox_scent: Option<&FoxScentMap>,
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // If threat nearby, fight it. Phase 4 migration: visual channel
    // via the unified sensory model.
    let nearest_threat = wildlife
        .iter()
        .filter(|(_, wp)| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                cat_profile,
                **wp,
                crate::components::SensorySignature::WILDLIFE,
                d.guard_threat_detection_range as f32,
            )
        })
        .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

    if let Some((threat_entity, threat_pos)) = nearest_threat {
        // If the threat is in high fox-scent territory (boundary encounter),
        // patrol toward and survey instead of full fight — let the fox's own
        // avoidance/standoff logic handle the interaction.
        let scent_at_threat = fox_scent.map_or(0.0, |fs| fs.get(threat_pos.x, threat_pos.y));
        if scent_at_threat > 0.3 {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::PatrolTo).with_position(*threat_pos),
                    TaskStep::new(StepKind::Survey).with_position(*threat_pos),
                ],
                FailurePolicy::AbortChain,
            );
            return Some((chain, Action::Patrol));
        }

        // Health gate: cats below guard_fight_health_min patrol+survey
        // instead of engaging directly. Prevents wounded guards from
        // walking into fights they can't survive.
        let hp_ratio = health.current / health.max;
        if hp_ratio < d.guard_fight_health_min {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::PatrolTo).with_position(*threat_pos),
                    TaskStep::new(StepKind::Survey).with_position(*threat_pos),
                ],
                FailurePolicy::AbortChain,
            );
            return Some((chain, Action::Patrol));
        }

        let chain = TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(*threat_pos),
                TaskStep::new(StepKind::FightThreat)
                    .with_position(*threat_pos)
                    .with_entity(threat_entity),
            ],
            FailurePolicy::AbortChain,
        );
        return Some((chain, Action::Fight));
    }

    // Patrol toward the nearest fox scent boundary if detectable.
    let patrol_radius = d.guard_patrol_radius;
    let scent_target =
        fox_scent.and_then(|fs| fs.highest_nearby(pos.x, pos.y, patrol_radius as i32));
    let target = if let Some((sx, sy)) = scent_target {
        // Patrol toward the fox scent hotspot.
        Position::new(sx.clamp(0, map.width - 1), sy.clamp(0, map.height - 1))
    } else {
        // Fallback: random perimeter patrol.
        let center_x = map.width / 2;
        let center_y = map.height / 2;
        let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        let mut t = Position::new(
            center_x + (angle.cos() * patrol_radius) as i32,
            center_y + (angle.sin() * patrol_radius) as i32,
        );
        t.x = t.x.clamp(0, map.width - 1);
        t.y = t.y.clamp(0, map.height - 1);
        t
    };

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::PatrolTo).with_position(target),
            TaskStep::new(StepKind::Survey).with_position(target),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Patrol))
}

#[allow(clippy::too_many_arguments)]
fn build_socializing_chain(
    entity: Entity,
    pos: &Position,
    personality: &Personality,
    skills: &Skills,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    skills_query: &Query<&Skills, Without<Dead>>,
    d: &DispositionConstants,
) -> Option<(TaskChain, Action)> {
    // Pick best social target.
    let target = cat_positions
        .iter()
        .filter(|(other, other_pos)| {
            *other != entity && pos.manhattan_distance(other_pos) <= d.social_chain_target_range
        })
        .max_by(|(e_a, _), (e_b, _)| {
            let score_a = relationships.get(entity, *e_a).map_or(0.0, |r| {
                r.fondness * d.fondness_social_weight
                    + (1.0 - r.familiarity) * d.novelty_social_weight
            });
            let score_b = relationships.get(entity, *e_b).map_or(0.0, |r| {
                r.fondness * d.fondness_social_weight
                    + (1.0 - r.familiarity) * d.novelty_social_weight
            });
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p));

    let (target_entity, target_pos) = target?;

    // Decide sub-action: mentor if applicable, groom if warm, otherwise socialize.
    let can_mentor = {
        let mentor_skills = [
            skills.hunting,
            skills.foraging,
            skills.herbcraft,
            skills.building,
            skills.combat,
            skills.magic,
        ];
        mentor_skills
            .iter()
            .any(|&s| s > d.mentor_skill_threshold_high)
            && skills_query.get(target_entity).is_ok_and(|other| {
                let other_arr = [
                    other.hunting,
                    other.foraging,
                    other.herbcraft,
                    other.building,
                    other.combat,
                    other.magic,
                ];
                mentor_skills.iter().zip(other_arr.iter()).any(|(&m, &a)| {
                    m > d.mentor_skill_threshold_high && a < d.mentor_skill_threshold_low
                })
            })
    };

    let (step_kind, action) = if can_mentor && personality.warmth > d.mentor_warmth_threshold {
        (StepKind::MentorCat, Action::Mentor)
    } else if personality.warmth > d.groom_warmth_threshold {
        (StepKind::GroomOther, Action::Groom)
    } else {
        (StepKind::Socialize, Action::Socialize)
    };

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target_pos),
            TaskStep::new(step_kind)
                .with_position(target_pos)
                .with_entity(target_entity),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, action))
}

#[allow(clippy::type_complexity)]
fn build_building_chain(
    _entity: Entity,
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    d: &DispositionConstants,
    _commands: &mut Commands,
) -> Option<(TaskChain, Action)> {
    let target = building_query
        .iter()
        .filter(|(_, _, bpos, site, _)| {
            site.is_some() || pos.manhattan_distance(bpos) <= d.building_search_range
        })
        .min_by_key(|(_, _s, bpos, site, _)| {
            let priority = if site.is_some() { 0 } else { 1 };
            let dist = pos.manhattan_distance(bpos);
            (priority, dist)
        });

    let (target_entity, _structure, bpos, site, _) = target?;

    let chain = if site.is_some() {
        TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(*bpos),
                TaskStep::new(StepKind::Construct)
                    .with_position(*bpos)
                    .with_entity(target_entity),
            ],
            FailurePolicy::AbortChain,
        )
    } else {
        StructureType::repair_chain(*bpos, target_entity)
    };

    Some((chain, Action::Build))
}

#[allow(clippy::type_complexity)]
fn build_farming_chain(
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
) -> Option<(TaskChain, Action)> {
    let garden = building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::Garden && site.is_none())
        .min_by_key(|(_, _, bpos, _, _)| pos.manhattan_distance(bpos));

    let (garden_entity, _, garden_pos, _, _) = garden?;
    let chain = StructureType::farm_chain(*garden_pos, garden_entity);
    Some((chain, Action::Farm))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_crafting_chain(
    pos: &Position,
    _personality: &Personality,
    _needs: &Needs,
    skills: &Skills,
    magic_aff: &MagicAffinity,
    inventory: &Inventory,
    herb_query: &Query<(Entity, &Herb, &Position), With<Harvestable>>,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    ward_query: &Query<(&Ward, &Position)>,
    _cat_positions: &[(Entity, Position)],
    injured_cats: &[(Entity, Position)],
    map: &TileMap,
    ward_strength_low: bool,
    d: &DispositionConstants,
    rng: &mut impl Rng,
    hint: Option<CraftingHint>,
) -> Option<(TaskChain, Action)> {
    // Pre-compute ward placement position for SetWard chains.
    let ward_placement_pos = if ward_strength_low {
        let building_positions: Vec<Position> = building_query
            .iter()
            .filter(|(_, _, _, site, _)| site.is_none())
            .map(|(_, _, bpos, _, _)| *bpos)
            .collect();
        let ward_data: Vec<(Position, f32)> = ward_query
            .iter()
            .filter(|(w, _)| !w.inverted && w.strength > 0.01)
            .map(|(w, p)| (*p, w.repel_radius()))
            .collect();
        let center = Position::new(map.width / 2, map.height / 2);
        Some(crate::systems::coordination::compute_ward_placement(
            &building_positions,
            &ward_data,
            center,
            d.crafting_ward_placement_radius,
            rng,
        ))
    } else {
        None
    };

    // Try hinted mode first.
    if let Some(h) = hint {
        if let Some(chain) = try_crafting_sub_mode(
            h,
            pos,
            skills,
            magic_aff,
            inventory,
            herb_query,
            building_query,
            injured_cats,
            map,
            ward_strength_low,
            ward_placement_pos,
            d,
            rng,
        ) {
            return Some(chain);
        }
    }

    // Fallback: cascade through remaining modes, skipping the hint.
    // (Cook intentionally absent — it's resolved by the GOAP planner, not
    // this task-chain path.)
    for mode in [
        CraftingHint::GatherHerbs,
        CraftingHint::PrepareRemedy,
        CraftingHint::SetWard,
        CraftingHint::DurableWard,
        CraftingHint::Magic,
    ] {
        if Some(mode) == hint {
            continue;
        }
        if let Some(chain) = try_crafting_sub_mode(
            mode,
            pos,
            skills,
            magic_aff,
            inventory,
            herb_query,
            building_query,
            injured_cats,
            map,
            ward_strength_low,
            ward_placement_pos,
            d,
            rng,
        ) {
            return Some(chain);
        }
    }

    None
}

/// Try to build a crafting chain for a specific sub-mode.
/// Returns `None` if the preconditions for that mode aren't met.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn try_crafting_sub_mode(
    mode: CraftingHint,
    pos: &Position,
    skills: &Skills,
    magic_aff: &MagicAffinity,
    inventory: &Inventory,
    herb_query: &Query<(Entity, &Herb, &Position), With<Harvestable>>,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    injured_cats: &[(Entity, Position)],
    map: &TileMap,
    ward_strength_low: bool,
    ward_placement_pos: Option<Position>,
    d: &DispositionConstants,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    use crate::components::magic::{HerbKind, RemedyKind, WardKind};

    match mode {
        CraftingHint::GatherHerbs => {
            let has_herbs_nearby = herb_query
                .iter()
                .any(|(_, _, hp)| pos.manhattan_distance(hp) <= d.crafting_herb_detection_range);

            if !has_herbs_nearby || skills.herbcraft <= d.crafting_herbcraft_skill_threshold {
                return None;
            }

            let nearest_herb = herb_query
                .iter()
                .filter(|(_, _, hp)| pos.manhattan_distance(hp) <= d.crafting_herb_detection_range)
                .min_by_key(|(_, _, hp)| pos.manhattan_distance(hp));

            let (herb_entity, _, herb_pos) = nearest_herb?;
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(*herb_pos),
                    TaskStep::new(StepKind::GatherHerb)
                        .with_position(*herb_pos)
                        .with_entity(herb_entity),
                ],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::Herbcraft))
        }

        CraftingHint::PrepareRemedy => {
            if !inventory.has_remedy_herb() {
                return None;
            }

            let remedy_kind = if inventory.has_herb(HerbKind::HealingMoss) {
                RemedyKind::HealingPoultice
            } else if inventory.has_herb(HerbKind::Moonpetal) {
                RemedyKind::EnergyTonic
            } else {
                RemedyKind::MoodTonic
            };

            let workshop_pos = building_query
                .iter()
                .filter(|(_, s, _, site, _)| s.kind == StructureType::Workshop && site.is_none())
                .map(|(_, _, bpos, _, _)| *bpos)
                .min_by_key(|bpos| pos.manhattan_distance(bpos));

            let mut steps = Vec::new();
            if let Some(wp) = workshop_pos {
                steps.push(TaskStep::new(StepKind::MoveTo).with_position(wp));
            }
            steps.push(TaskStep::new(StepKind::PrepareRemedy {
                remedy: remedy_kind,
            }));

            // After preparing, deliver the remedy to the nearest injured cat.
            if let Some((patient_entity, patient_pos)) = injured_cats
                .iter()
                .min_by_key(|(_, ip)| pos.manhattan_distance(ip))
            {
                steps.push(
                    TaskStep::new(StepKind::ApplyRemedy {
                        remedy: remedy_kind,
                    })
                    .with_position(*patient_pos)
                    .with_entity(*patient_entity),
                );
            }

            let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
            Some((chain, Action::Herbcraft))
        }

        CraftingHint::SetWard | CraftingHint::DurableWard => {
            let is_durable = matches!(mode, CraftingHint::DurableWard);
            // Thornwards need herbs; durable wards skip the herb check.
            if (!is_durable && !inventory.has_ward_herb()) || !ward_strength_low {
                return None;
            }
            if is_durable
                && (magic_aff.0 <= d.crafting_magic_affinity_threshold
                    || skills.magic <= d.crafting_magic_skill_threshold)
            {
                return None;
            }

            let ward_pos = ward_placement_pos.unwrap_or_else(|| {
                // Fallback: random angle if no computed position.
                let center_x = map.width / 2;
                let center_y = map.height / 2;
                let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                let radius = d.crafting_ward_placement_radius;
                let mut p = Position::new(
                    center_x + (angle.cos() * radius) as i32,
                    center_y + (angle.sin() * radius) as i32,
                );
                p.x = p.x.clamp(0, map.width - 1);
                p.y = p.y.clamp(0, map.height - 1);
                p
            });

            let kind = if is_durable {
                WardKind::DurableWard
            } else {
                WardKind::Thornward
            };
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(ward_pos),
                    TaskStep::new(StepKind::SetWard { kind }).with_position(ward_pos),
                ],
                FailurePolicy::AbortChain,
            );
            let action = if is_durable {
                Action::PracticeMagic
            } else {
                Action::Herbcraft
            };
            Some((chain, action))
        }

        CraftingHint::Magic => {
            if magic_aff.0 <= d.crafting_magic_affinity_threshold
                || skills.magic <= d.crafting_magic_skill_threshold
            {
                return None;
            }

            let on_special = if map.in_bounds(pos.x, pos.y) {
                matches!(
                    map.get(pos.x, pos.y).terrain,
                    Terrain::FairyRing | Terrain::StandingStone
                )
            } else {
                false
            };

            if on_special {
                let chain = TaskChain::new(
                    vec![TaskStep::new(StepKind::SpiritCommunion).with_position(*pos)],
                    FailurePolicy::AbortChain,
                );
                return Some((chain, Action::PracticeMagic));
            }

            let chain = TaskChain::new(
                vec![TaskStep::new(StepKind::Scry).with_position(*pos)],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::PracticeMagic))
        }

        CraftingHint::Cleanse => {
            if magic_aff.0 <= d.crafting_magic_affinity_threshold {
                return None;
            }
            let chain = TaskChain::new(
                vec![TaskStep::new(StepKind::CleanseCorruption).with_position(*pos)],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::PracticeMagic))
        }

        CraftingHint::HarvestCarcass => {
            // Legacy TaskChain path can't express HarvestCarcass cleanly;
            // the GOAP executor handles it.
            None
        }

        // Cook is executed entirely through the GOAP planner
        // (`GoapActionKind::RetrieveRawFood` / `Cook` / `DepositCookedFood`
        // in `src/systems/goap.rs`). The unmet-demand signal is emitted
        // from `score_cook` via `ScoringResult::wants_cook_but_no_kitchen`,
        // not from here.
        CraftingHint::Cook => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_coordinating_chain(
    entity: Entity,
    pos: &Position,
    directive_queue_query: &Query<(Entity, &DirectiveQueue)>,
    active_directive_query: &Query<&ActiveDirective>,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
    d: &DispositionConstants,
    _commands: &mut Commands,
) -> Option<(TaskChain, Action)> {
    let (_, queue) = directive_queue_query.get(entity).ok()?;
    let directive = queue.directives.first()?.clone();

    // Find the best target for this directive.
    let target = cat_positions
        .iter()
        .filter(|(e, _)| *e != entity)
        .filter(|(e, _)| active_directive_query.get(*e).is_err())
        .filter(|(_, p)| pos.manhattan_distance(p) <= d.coordinating_target_range)
        .max_by(|(e_a, p_a), (e_b, p_b)| {
            let skill_a = skills_query
                .get(*e_a)
                .map_or(0.0, |s| match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                    DirectiveKind::Cleanse => s.magic,
                    DirectiveKind::HarvestCarcass => s.herbcraft,
                    DirectiveKind::Cook => 0.0,
                });
            let skill_b = skills_query
                .get(*e_b)
                .map_or(0.0, |s| match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                    DirectiveKind::Cleanse => s.magic,
                    DirectiveKind::HarvestCarcass => s.herbcraft,
                    DirectiveKind::Cook => 0.0,
                });
            let rank_a =
                skill_a - pos.manhattan_distance(p_a) as f32 * d.coordinating_distance_penalty;
            let rank_b =
                skill_b - pos.manhattan_distance(p_b) as f32 * d.coordinating_distance_penalty;
            rank_a
                .partial_cmp(&rank_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p));

    let (target_entity, target_pos) = target?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target_pos),
            TaskStep::new(StepKind::DeliverDirective {
                kind: directive.kind,
                priority: directive.priority,
                directive_target: directive.target_position,
            })
            .with_position(target_pos)
            .with_entity(target_entity),
        ],
        FailurePolicy::AbortChain,
    );

    Some((chain, Action::Coordinate))
}

fn build_exploring_chain(
    pos: &Position,
    map: &TileMap,
    d: &DispositionConstants,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    let dx: i32 = rng.random_range(-d.explore_range..=d.explore_range);
    let dy: i32 = rng.random_range(-d.explore_range..=d.explore_range);
    let mut target = Position::new(pos.x + dx, pos.y + dy);
    target.x = target.x.clamp(0, map.width - 1);
    target.y = target.y.clamp(0, map.height - 1);

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target),
            TaskStep::new(StepKind::Survey).with_position(target),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Explore))
}

// ===========================================================================
// build_mating_chain
// ===========================================================================

fn build_mating_chain(
    entity: Entity,
    pos: &Position,
    _personality: &Personality,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    _d: &DispositionConstants,
) -> Option<(TaskChain, Action)> {
    // Find the best eligible partner: Partners+ bond, nearby.
    let mut best: Option<(Entity, Position, f32)> = None;
    for &(other, other_pos) in cat_positions {
        if other == entity {
            continue;
        }
        let Some(rel) = relationships.get(entity, other) else {
            continue;
        };
        let bond = rel
            .bond
            .unwrap_or(crate::resources::relationships::BondType::Friends);
        if !matches!(
            bond,
            crate::resources::relationships::BondType::Partners
                | crate::resources::relationships::BondType::Mates
        ) {
            continue;
        }
        let dist = pos.manhattan_distance(&other_pos) as f32;
        let score = rel.romantic + rel.fondness - dist * 0.05;
        if best.as_ref().is_none_or(|(_, _, s)| score > *s) {
            best = Some((other, other_pos, score));
        }
    }

    let (partner, partner_pos, _) = best?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(partner_pos),
            TaskStep::new(StepKind::Socialize).with_entity(partner),
            TaskStep::new(StepKind::GroomOther).with_entity(partner),
            TaskStep::new(StepKind::MateWith).with_entity(partner),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Mate))
}

// ===========================================================================
// build_caretaking_chain
// ===========================================================================

fn build_caretaking_chain(
    _entity: Entity,
    _pos: &Position,
    _personality: &Personality,
    nearest_store: Option<(Entity, Position)>,
    _d: &DispositionConstants,
) -> Option<(TaskChain, Action)> {
    // For now, build a simple chain: go to stores, pick up food, find kitten.
    // The actual kitten targeting is resolved at step execution time.
    let (store_entity, store_pos) = nearest_store?;
    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(store_pos),
            TaskStep::new(StepKind::FeedKitten).with_entity(store_entity),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Caretake))
}

// ===========================================================================
// Scent detection helper
// ===========================================================================

/// Check if a cat can smell prey based on wind direction and terrain.
///
/// Phase 3 migration: delegates to
/// `crate::systems::sensing::cat_smells_prey_windaware` — the unified
/// scent-channel implementation. The previous local body of this
/// function and the byte-identical copy in `goap.rs` both now route
/// through the same helper.
fn can_smell_prey(
    cat_pos: &Position,
    prey_pos: &Position,
    wind: &crate::resources::wind::WindState,
    map: &TileMap,
    d: &DispositionConstants,
) -> bool {
    crate::systems::sensing::cat_smells_prey_windaware(
        *cat_pos,
        *prey_pos,
        wind,
        map,
        // `disposition.rs` used only the wind-aware path (no scent_min_range
        // close-range bypass). Passing 0.0 preserves that exactly.
        0.0,
        d.scent_base_range,
        d.scent_downwind_dot_threshold,
        d.scent_dense_forest_modifier,
        d.scent_light_forest_modifier,
    )
}

/// Move one tile in direction (dx, dy). If blocked, try perpendicular or
/// reverse. Returns the new position. Guaranteed to attempt movement — never
/// returns the original position without trying alternatives.
fn patrol_move(pos: &Position, dx: i32, dy: i32, map: &TileMap) -> Position {
    // Primary direction.
    let primary = Position::new(pos.x + dx, pos.y + dy);
    if map.in_bounds(primary.x, primary.y) && map.get(primary.x, primary.y).terrain.is_passable() {
        return primary;
    }
    // Try perpendicular (swap dx/dy).
    let perp = Position::new(pos.x + dy, pos.y + dx);
    if map.in_bounds(perp.x, perp.y) && map.get(perp.x, perp.y).terrain.is_passable() {
        return perp;
    }
    // Try reverse.
    let rev = Position::new(pos.x - dx, pos.y - dy);
    if map.in_bounds(rev.x, rev.y) && map.get(rev.x, rev.y).terrain.is_passable() {
        return rev;
    }
    // Stuck — stay put.
    *pos
}

// ===========================================================================
// resolve_disposition_chains
// ===========================================================================

/// Apply a step handler result to the task chain, syncing `CurrentAction`
/// targets when the active step changes.
fn apply_step_result(
    result: crate::steps::StepResult,
    chain: &mut TaskChain,
    current: &mut CurrentAction,
) {
    match result {
        crate::steps::StepResult::Continue => {}
        crate::steps::StepResult::Advance => {
            chain.advance();
            chain.sync_targets(current);
        }
        crate::steps::StepResult::Fail(reason) => {
            chain.fail_current(reason);
            // SkipStep policy advances to the next step — sync those targets.
            chain.sync_targets(current);
        }
    }
}

/// Resolves disposition-specific TaskChain steps (HuntPrey, ForageItem, etc.)
/// and handles disposition completion when chains finish.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_disposition_chains(
    mut cats: Query<
        (
            Entity,
            &mut TaskChain,
            &mut CurrentAction,
            &mut Position,
            &mut Skills,
            &mut Needs,
            &mut Inventory,
            &Personality,
            &Name,
            &Gender,
            Option<&mut Disposition>,
            Option<&mut ActionHistory>,
            &mut HuntingPriors,
            Option<&mut crate::components::grooming::GroomingCondition>,
            &mut crate::components::mental::Mood,
        ),
        (
            Without<Dead>,
            Without<Structure>,
            Without<PreyAnimal>,
            Without<PreyDen>,
        ),
    >,
    mut prey_query: Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    mut stores_query: Query<&mut StoredItems>,
    items_query: Query<&Item>,
    // Disjoint from `cats` (which requires TaskChain) — reads skills of non-chain
    // entities like apprentices being mentored.
    mut unchained_skills: Query<&mut Skills, (Without<TaskChain>, Without<Structure>)>,
    map: Res<TileMap>,
    wind: Res<crate::resources::wind::WindState>,
    mut relationships: ResMut<Relationships>,
    mut narr: NarrativeEmitter<'_>,
    time: Res<TimeState>,
    mut rng: ResMut<SimRng>,
    mut colony_map: ResMut<ColonyHuntingMap>,
    den_query: Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    mut prey_params: PreyHuntParams,
    constants: Res<SimConstants>,
    mut commands: Commands,
) {
    let d = &constants.disposition;
    // Deferred effects applied after the main loop (avoids mutable borrow conflicts).
    struct MentorEffect {
        apprentice: Entity,
        mentor_skills: Skills,
    }
    let mut mentor_effects: Vec<MentorEffect> = Vec::new();
    let mut chains_to_remove: Vec<Entity> = Vec::new();

    // Collect grooming snapshots for target lookups during social interactions.
    let grooming_snapshot: std::collections::HashMap<Entity, f32> = cats
        .iter()
        .map(|(e, _, _, _, _, _, _, _, _, _, _, _, _, g, _)| (e, g.map_or(0.8, |g| g.0)))
        .collect();
    // Deferred grooming restoration for targets of GroomOther steps.
    let mut grooming_restorations: Vec<(Entity, f32)> = Vec::new();

    // Snapshot tile occupancy for anti-stacking jitter on PatrolTo arrival.
    let cat_tile_counts: std::collections::HashMap<Position, u32> = {
        let mut counts = std::collections::HashMap::new();
        for (_, _, _, pos, _, _, _, _, _, _, _, _, _, _, _) in &cats {
            *counts.entry(*pos).or_insert(0) += 1;
        }
        counts
    };

    for (
        cat_entity,
        mut chain,
        mut current,
        mut pos,
        mut skills,
        mut needs,
        mut inventory,
        personality,
        name,
        gender,
        disposition,
        history,
        mut hunting_priors,
        mut grooming,
        mut mood,
    ) in &mut cats
    {
        // Den discovery: peek at current step kind without mutable borrow.
        // If the cat is hunting and near a den, raid it and advance the chain
        // before taking the mutable step borrow below.
        if let Some(step_ref) = chain.steps.get(chain.current_step) {
            if matches!(step_ref.kind, StepKind::HuntPrey { .. }) {
                use crate::components::magic::ItemSlot;
                let mut found_den = false;
                for (den_entity, den, den_pos) in den_query.iter() {
                    if pos.manhattan_distance(den_pos) <= d.den_discovery_range {
                        let discovery_chance = d.den_discovery_base_chance
                            + skills.hunting * d.den_discovery_skill_scale;
                        if rng.rng.random::<f32>() < discovery_chance && den.spawns_remaining > 0 {
                            // Raid: kill ~40% of remaining, capped at raid_drop.
                            let kills = ((den.spawns_remaining as f32 * d.den_raid_kill_fraction)
                                .ceil() as u32)
                                .min(den.raid_drop);
                            let drop_item = den.item_kind;
                            let den_name = den.den_name;
                            let den_pos_copy = *den_pos;

                            // Cat picks up what it can carry.
                            let den_corruption = if map.in_bounds(den_pos_copy.x, den_pos_copy.y) {
                                map.get(den_pos_copy.x, den_pos_copy.y).corruption
                            } else {
                                0.0
                            };
                            let den_mods = crate::components::items::ItemModifiers::with_corruption(
                                den_corruption,
                            );
                            for _ in 0..kills {
                                if !inventory.is_full() {
                                    inventory.slots.push(ItemSlot::Item(drop_item, den_mods));
                                } else {
                                    commands.spawn((
                                        crate::components::items::Item::with_modifiers(
                                            drop_item,
                                            d.den_dropped_item_quality,
                                            crate::components::items::ItemLocation::OnGround,
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

                            // Send raid message — den mutation happens in apply_den_raids.
                            prey_params.raid_writer.write(DenRaided {
                                den_entity,
                                kills,
                                item_kind: drop_item,
                                position: den_pos_copy,
                                den_name,
                            });

                            {
                                let terrain = if map.in_bounds(pos.x, pos.y) {
                                    map.get(pos.x, pos.y).terrain
                                } else {
                                    Terrain::Grass
                                };
                                let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                                let season = Season::from_tick(time.tick, &narr.config);
                                let ctx = TemplateContext {
                                    action: crate::ai::Action::Hunt,
                                    day_phase,
                                    season,
                                    weather: narr.weather.current,
                                    mood_bucket: MoodBucket::Neutral,
                                    life_stage: LifeStage::Adult,
                                    has_target: true,
                                    terrain,
                                    event: Some("raid".into()),
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
                                    prey: Some(den_name),
                                    item: None,
                                    item_singular: None,
                                    quality: None,
                                };
                                emit_event_narrative(
                                    narr.registry.as_deref(),
                                    &mut narr.log,
                                    time.tick,
                                    format!("{} raids a {}!", name.0, den_name),
                                    crate::resources::narrative::NarrativeTier::Action,
                                    &ctx,
                                    &var_ctx,
                                    personality,
                                    &needs,
                                    &mut rng.rng,
                                );
                            }
                            chain.advance();
                            chain.sync_targets(&mut current);
                            found_den = true;
                            break;
                        }
                    }
                }
                if found_den {
                    continue;
                }
            }
        }

        let Some(step) = chain.current_mut() else {
            // Chain exhausted — handle completion or failure.
            chains_to_remove.push(cat_entity);
            if let Some(mut disp) = disposition {
                let outcome = if chain.is_succeeded() {
                    disp.completions += 1;
                    // Successful completions earn respect proportional to contribution.
                    let respect_gain = respect_for_disposition(disp.kind, d);
                    if respect_gain > 0.0 {
                        needs.respect = (needs.respect + respect_gain).min(1.0);
                    }
                    ActionOutcome::Success
                } else {
                    ActionOutcome::Failure
                };
                if let Some(mut hist) = history {
                    hist.record(ActionRecord {
                        action: current.action,
                        disposition: Some(disp.kind),
                        tick: time.tick,
                        outcome,
                    });
                }
            }
            current.ticks_remaining = 0;
            continue;
        };

        // Only handle disposition-specific steps; skip others.
        let is_disposition_step = matches!(
            step.kind,
            StepKind::HuntPrey { .. }
                | StepKind::ForageItem { .. }
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
                | StepKind::DeliverDirective { .. }
                | StepKind::MateWith
                | StepKind::FeedKitten
                | StepKind::RetrieveFromStores { .. }
        );
        if !is_disposition_step {
            continue;
        }

        // Ensure step is in progress.
        if matches!(
            step.status,
            crate::components::task_chain::StepStatus::Pending
        ) {
            step.status =
                crate::components::task_chain::StepStatus::InProgress { ticks_elapsed: 0 };
        }

        let ticks = match &mut step.status {
            crate::components::task_chain::StepStatus::InProgress { ticks_elapsed } => {
                *ticks_elapsed += 1;
                *ticks_elapsed
            }
            _ => continue,
        };

        match &step.kind {
            StepKind::HuntPrey { patrol_dir } => {
                // Multi-phase hunt: Search → Stalk → Pounce.
                // Phase is implicit from step.target_entity:
                //   None = Search (scent-based) or Approach (scent locked)
                //   Some = Stalk/Pounce (prey visible)
                use crate::components::magic::ItemSlot;
                use crate::components::prey::PreyAiState;

                if let Some(target_entity) = step.target_entity {
                    // We have a locked target — check if it still exists.
                    let Ok((_, prey_pos, prey_cfg, prey_state)) = prey_query.get(target_entity)
                    else {
                        step.target_entity = None;
                        continue;
                    };
                    let prey_pos = *prey_pos;
                    let prey_is_fleeing =
                        matches!(prey_state.ai_state, PreyAiState::Fleeing { .. });
                    let prey_awareness = prey_state.ai_state;
                    let catch_mod = prey_cfg.catch_difficulty;
                    let item_kind = prey_cfg.item_kind;
                    let species_name = prey_cfg.name;
                    let flee_strategy = prey_cfg.flee_strategy;
                    let dist = pos.manhattan_distance(&prey_pos);

                    // Bird-specific: if prey teleported away, give up immediately.
                    if prey_is_fleeing
                        && flee_strategy == crate::components::prey::FleeStrategy::Teleport
                    {
                        step.target_entity = None;
                        continue;
                    }

                    // Dynamic stalk-start: slow down before entering detection zone.
                    let stalk_start =
                        (prey_cfg.alert_radius + d.stalk_start_buffer).max(d.stalk_start_minimum);

                    // Determine pounce range from patience.
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
                            PreyAiState::Idle | PreyAiState::Grazing { .. } => {
                                d.pounce_awareness_idle
                            }
                            PreyAiState::Alert { .. } => d.pounce_awareness_alert,
                            PreyAiState::Fleeing { .. } => d.pounce_awareness_fleeing,
                        };
                        let distance_mod = match dist {
                            0..=1 => d.pounce_distance_close_mod,
                            2 => d.pounce_distance_mid_mod,
                            _ => d.pounce_distance_far_mod,
                        };
                        // Density-dependent vulnerability: crowded prey are easier to catch.
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
                                    crate::components::items::ItemModifiers::with_corruption(
                                        catch_corruption,
                                    ),
                                ));
                            }
                            skills.hunting += skills.growth_rate() * d.hunt_catch_skill_growth;

                            // Send kill event for den pressure tracking.
                            prey_params.kill_writer.write(PreyKilled {
                                kind: prey_cfg.kind,
                                position: prey_pos,
                            });

                            {
                                let terrain = if map.in_bounds(pos.x, pos.y) {
                                    map.get(pos.x, pos.y).terrain
                                } else {
                                    Terrain::Grass
                                };
                                let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                                let season = Season::from_tick(time.tick, &narr.config);
                                let catch_desc = if catch_corruption > 0.3 {
                                    format!("corrupted {}", species_name)
                                } else {
                                    species_name.to_string()
                                };
                                let ctx = TemplateContext {
                                    action: crate::ai::Action::Hunt,
                                    day_phase,
                                    season,
                                    weather: narr.weather.current,
                                    mood_bucket: MoodBucket::Neutral,
                                    life_stage: LifeStage::Adult,
                                    has_target: true,
                                    terrain,
                                    event: Some("catch".into()),
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
                                    prey: Some(species_name),
                                    item: None,
                                    item_singular: None,
                                    quality: None,
                                };
                                emit_event_narrative(
                                    narr.registry.as_deref(),
                                    &mut narr.log,
                                    time.tick,
                                    format!("{} catches a {}.", name.0, catch_desc),
                                    crate::resources::narrative::NarrativeTier::Action,
                                    &ctx,
                                    &var_ctx,
                                    personality,
                                    &needs,
                                    &mut rng.rng,
                                );
                            }

                            hunting_priors.record_catch(&prey_pos);

                            // Multi-kill: if inventory has room, hunt for more.
                            if inventory.is_full() {
                                chain.advance();
                                chain.sync_targets(&mut current);
                            } else {
                                step.target_entity = None; // Search for another target.
                            }
                        } else {
                            // Pounce failed — prey bolts.
                            if let Ok((_, _, _, mut prey_st)) = prey_query.get_mut(target_entity) {
                                prey_st.ai_state = PreyAiState::Fleeing {
                                    from: cat_entity,
                                    toward: None,
                                    ticks: 0,
                                };
                            }

                            {
                                let terrain = if map.in_bounds(pos.x, pos.y) {
                                    map.get(pos.x, pos.y).terrain
                                } else {
                                    Terrain::Grass
                                };
                                let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                                let season = Season::from_tick(time.tick, &narr.config);
                                let ctx = TemplateContext {
                                    action: crate::ai::Action::Hunt,
                                    day_phase,
                                    season,
                                    weather: narr.weather.current,
                                    mood_bucket: MoodBucket::Neutral,
                                    life_stage: LifeStage::Adult,
                                    has_target: true,
                                    terrain,
                                    event: Some("miss".into()),
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
                                    prey: Some(species_name),
                                    item: None,
                                    item_singular: None,
                                    quality: None,
                                };
                                emit_event_narrative(
                                    narr.registry.as_deref(),
                                    &mut narr.log,
                                    time.tick,
                                    format!("{}'s quarry bolts.", name.0),
                                    crate::resources::narrative::NarrativeTier::Micro,
                                    &ctx,
                                    &var_ctx,
                                    personality,
                                    &needs,
                                    &mut rng.rng,
                                );
                            }

                            let chase_limit = if personality.boldness > 0.7 {
                                d.chase_limit_bold
                            } else {
                                d.chase_limit_default
                            };
                            if ticks > chase_limit {
                                // Don't fail the whole chain — just drop target and
                                // resume searching. Cats are persistent hunters.
                                step.target_entity = None;
                            }
                        }
                    } else if dist <= stalk_start {
                        let mut moved = false;
                        if prey_is_fleeing {
                            // === CHASE === sprint burst.
                            for _ in 0..d.chase_speed {
                                if let Some(next) = step_toward(&pos, &prey_pos, &map) {
                                    *pos = next;
                                    moved = true;
                                }
                            }
                        } else {
                            // === STALK === Deliberate approach, 1 tile/tick.
                            // Cats are agile ambush predators — they close quickly
                            // while relying on stealth to avoid detection.
                            if let Some(next) = step_toward(&pos, &prey_pos, &map) {
                                *pos = next;
                                moved = true;
                            }
                            // Anxiety check: nervous cat spooks prey.
                            if personality.anxiety > d.anxiety_spook_threshold
                                && rng.rng.random::<f32>() < d.anxiety_spook_chance
                            {
                                if let Ok((_, _, _, mut prey_st)) =
                                    prey_query.get_mut(target_entity)
                                {
                                    prey_st.ai_state = PreyAiState::Fleeing {
                                        from: cat_entity,
                                        toward: None,
                                        ticks: 0,
                                    };
                                }
                                step.target_entity = None; // Resume searching.
                            }
                        }
                        // Can't reach prey (terrain blocked) — give up after a few ticks.
                        if !moved && ticks > 10 {
                            step.target_entity = None;
                        }
                    } else {
                        // === APPROACH === Trot toward scented prey.
                        let mut moved = false;
                        for _ in 0..d.approach_speed {
                            if let Some(next) = step_toward(&pos, &prey_pos, &map) {
                                *pos = next;
                                moved = true;
                            }
                        }
                        if dist > d.approach_give_up_distance || (!moved && ticks > 10) {
                            step.target_entity = None;
                        }
                    }
                } else {
                    // === SEARCH === (no target yet — move toward best-known hunting ground)
                    // Priority: personal belief > colony belief > wind > patrol_dir.
                    let belief_dir = hunting_priors.best_direction(&pos, d.search_belief_radius);
                    let colony_dir = colony_map
                        .beliefs
                        .best_direction(&pos, d.search_belief_radius);
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
                        *patrol_dir
                    };
                    // Jitter — enough to explore but not lose the gradient.
                    if rng.rng.random::<f32>() < d.search_jitter_chance {
                        dx = rng.rng.random_range(-1i32..=1);
                        dy = rng.rng.random_range(-1i32..=1);
                    }
                    // Ensure we actually move (avoid 0,0).
                    if dx == 0 && dy == 0 {
                        dx = 1;
                    }
                    // Trot while searching to cover ground.
                    for _ in 0..d.search_speed {
                        *pos = patrol_move(&pos, dx, dy, &map);
                    }

                    // Visual detection: spot nearby prey within 15 tiles.
                    let visible_prey = prey_query
                        .iter()
                        .filter(|(_, pp, _, _)| {
                            crate::systems::sensing::observer_sees_at(
                                crate::components::SensorySpecies::Cat,
                                *pos,
                                &constants.sensory.cat,
                                **pp,
                                crate::components::SensorySignature::PREY,
                                d.search_visual_detection_range as f32,
                            )
                        })
                        .min_by_key(|(_, pp, _, _)| pos.manhattan_distance(pp));

                    if let Some((prey_entity, _prey_pos_ref, _, _)) = visible_prey {
                        step.target_entity = Some(prey_entity);
                    } else {
                        // Scan for prey scent (wind-dependent, longer range).
                        let scented_prey = prey_query
                            .iter()
                            .filter(|(_, pp, _, _)| can_smell_prey(&pos, pp, &wind, &map, d))
                            .min_by_key(|(_, pp, _, _)| pos.manhattan_distance(pp));

                        if let Some((prey_entity, prey_pos_ref, _, _)) = scented_prey {
                            step.target_entity = Some(prey_entity);
                            hunting_priors.record_scent(prey_pos_ref);
                            {
                                let terrain = if map.in_bounds(pos.x, pos.y) {
                                    map.get(pos.x, pos.y).terrain
                                } else {
                                    Terrain::Grass
                                };
                                let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                                let season = Season::from_tick(time.tick, &narr.config);
                                let ctx = TemplateContext {
                                    action: crate::ai::Action::Hunt,
                                    day_phase,
                                    season,
                                    weather: narr.weather.current,
                                    mood_bucket: MoodBucket::Neutral,
                                    life_stage: LifeStage::Adult,
                                    has_target: false,
                                    terrain,
                                    event: Some("scent".into()),
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
                                    item: None,
                                    item_singular: None,
                                    quality: None,
                                };
                                emit_event_narrative(
                                    narr.registry.as_deref(),
                                    &mut narr.log,
                                    time.tick,
                                    format!("{} catches a scent on the wind.", name.0),
                                    crate::resources::narrative::NarrativeTier::Micro,
                                    &ctx,
                                    &var_ctx,
                                    personality,
                                    &needs,
                                    &mut rng.rng,
                                );
                            }
                        }
                    }

                    if ticks > d.search_timeout_ticks {
                        // Multi-kill: if we already have food, head to stores.
                        if inventory
                            .slots
                            .iter()
                            .any(|s| matches!(s, ItemSlot::Item(k, _) if k.is_food()))
                        {
                            chain.advance();
                            chain.sync_targets(&mut current);
                        } else {
                            hunting_priors.record_failed_search(&pos, ticks);
                            chain.fail_current("no scent found".into());
                        }
                    }
                }
            }

            StepKind::ForageItem { patrol_dir } => {
                // Active foraging: directional patrol, check each tile.
                // Use patrol_dir with jitter and reverse-on-blocked (wildlife pattern).
                let mut dx = patrol_dir.0;
                let mut dy = patrol_dir.1;
                if dx == 0 && dy == 0 {
                    dx = 1;
                } // ensure movement
                  // Forage jitter.
                if rng.rng.random::<f32>() < d.forage_jitter_chance {
                    dx = rng.rng.random_range(-1i32..=1);
                    dy = rng.rng.random_range(-1i32..=1);
                    if dx == 0 && dy == 0 {
                        dx = 1;
                    }
                }
                *pos = patrol_move(&pos, dx, dy, &map);

                // Check current tile for forage yield.
                if map.in_bounds(pos.x, pos.y) {
                    let tile = map.get(pos.x, pos.y);
                    let forage_yield = tile.terrain.foraging_yield();
                    if forage_yield > 0.0
                        && rng.rng.random::<f32>() < forage_yield * d.forage_yield_scale
                    {
                        use crate::components::items::ItemKind;
                        use crate::components::magic::ItemSlot;
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
                                crate::components::items::ItemModifiers::with_corruption(
                                    forage_corruption,
                                ),
                            ));
                        }
                        skills.foraging += skills.growth_rate() * d.forage_skill_growth;
                        {
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
                                action: crate::ai::Action::Forage,
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
                                &needs,
                                &mut rng.rng,
                            );
                        }
                        chain.advance();
                        chain.sync_targets(&mut current);
                    } else if ticks > d.forage_timeout_ticks {
                        chain.fail_current("nothing found while foraging".into());
                    }
                }
            }

            StepKind::DepositAtStores => {
                let target = step.target_entity;
                let deposit = crate::steps::disposition::resolve_deposit_at_stores(
                    target,
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
                apply_step_result(deposit.step, &mut chain, &mut current);
            }

            StepKind::EatAtStores => {
                let target = step.target_entity;
                apply_step_result(
                    crate::steps::disposition::resolve_eat_at_stores(
                        ticks,
                        target,
                        &mut needs,
                        &mut stores_query,
                        &items_query,
                        &mut commands,
                        d,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::Sleep { ticks: duration } => {
                apply_step_result(
                    crate::steps::disposition::resolve_sleep(ticks, *duration, &mut needs, d),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::SelfGroom => {
                apply_step_result(
                    crate::steps::disposition::resolve_self_groom(
                        ticks,
                        &mut needs,
                        grooming.as_deref_mut(),
                        d,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::Socialize => {
                let target = step.target_entity;
                apply_step_result(
                    crate::steps::disposition::resolve_socialize(
                        ticks,
                        cat_entity,
                        target,
                        &mut needs,
                        &mut hunting_priors,
                        &mut relationships,
                        &mut colony_map,
                        &grooming_snapshot,
                        time.tick,
                        &constants.social,
                        d,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::GroomOther => {
                let target = step.target_entity;
                let (result, restoration) = crate::steps::disposition::resolve_groom_other(
                    ticks,
                    cat_entity,
                    target,
                    &mut needs,
                    &mut hunting_priors,
                    &mut relationships,
                    &mut colony_map,
                    &grooming_snapshot,
                    time.tick,
                    &constants.social,
                    d,
                );
                if let Some(r) = restoration {
                    grooming_restorations.push(r);
                }
                apply_step_result(result, &mut chain, &mut current);
            }

            StepKind::MentorCat => {
                let target = step.target_entity;
                let (result, effect) = crate::steps::disposition::resolve_mentor_cat(
                    ticks,
                    cat_entity,
                    target,
                    &mut needs,
                    &skills,
                    &mut relationships,
                    time.tick,
                    d,
                );
                if let Some((apprentice, mentor_skills)) = effect {
                    mentor_effects.push(MentorEffect {
                        apprentice,
                        mentor_skills,
                    });
                }
                apply_step_result(result, &mut chain, &mut current);
            }

            StepKind::PatrolTo => {
                let target = step.target_position;
                let cached = &mut step.cached_path;
                apply_step_result(
                    crate::steps::disposition::resolve_patrol_to(
                        &mut pos,
                        target,
                        cached,
                        &mut needs,
                        &map,
                        d,
                        &cat_tile_counts,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::FightThreat => {
                let health = prey_params
                    .health_query
                    .get(cat_entity)
                    .cloned()
                    .unwrap_or_default();
                apply_step_result(
                    crate::steps::disposition::resolve_fight_threat(
                        ticks,
                        &mut skills,
                        &mut needs,
                        &health,
                        d,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::Survey => {
                apply_step_result(
                    crate::steps::disposition::resolve_survey(
                        ticks,
                        &mut needs,
                        &pos,
                        &mut prey_params.exploration_map,
                        d,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::DeliverDirective {
                kind,
                priority,
                directive_target,
            } => {
                let result =
                    crate::steps::disposition::resolve_deliver_directive(ticks, &mut needs, d);
                if matches!(result, crate::steps::StepResult::Advance) {
                    // Insert ActiveDirective on the target cat.
                    if let Some(target) = step.target_entity {
                        commands.entity(target).insert(ActiveDirective {
                            kind: *kind,
                            priority: *priority,
                            coordinator: cat_entity,
                            coordinator_social_weight: needs.respect,
                            delivered_tick: time.tick,
                            target_position: *directive_target,
                            target_entity: None,
                        });
                        if let Some(ref mut act) = narr.activation {
                            act.record(Feature::DirectiveDelivered);
                        }
                    }
                }
                apply_step_result(result, &mut chain, &mut current);
            }

            StepKind::MateWith => {
                let target = step.target_entity;
                let (result, pregnancy) = crate::steps::disposition::resolve_mate_with(
                    ticks,
                    cat_entity,
                    target,
                    &mut needs,
                    &mut relationships,
                );
                if let Some((partner, litter_size)) = pregnancy {
                    commands.entity(cat_entity).insert(
                        crate::components::pregnancy::Pregnant::new(
                            time.tick,
                            partner,
                            litter_size,
                        ),
                    );
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::MatingOccurred);
                    }
                }
                apply_step_result(result, &mut chain, &mut current);
            }

            StepKind::FeedKitten => {
                let target = step.target_entity;
                apply_step_result(
                    crate::steps::disposition::resolve_feed_kitten(
                        ticks,
                        target,
                        &mut needs,
                        &mut stores_query,
                        &items_query,
                        &mut commands,
                    ),
                    &mut chain,
                    &mut current,
                );
            }

            StepKind::RetrieveFromStores { kind } => {
                let kind = *kind;
                let target = step.target_entity;
                let (result, retrieved) = crate::steps::disposition::resolve_retrieve_from_stores(
                    ticks,
                    kind,
                    target,
                    &mut inventory,
                    &mut stores_query,
                    &items_query,
                    &mut commands,
                );
                if retrieved {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::ItemRetrieved);
                    }
                }
                apply_step_result(result, &mut chain, &mut current);
            }

            // Non-disposition steps are handled elsewhere.
            _ => {}
        }

        if chain.is_complete() {
            chains_to_remove.push(cat_entity);
            if let Some(mut disp) = disposition {
                let succeeded = !chain.is_failed();
                disp.completions += 1;
                if succeeded {
                    let respect_gain = respect_for_disposition(disp.kind, d);
                    if respect_gain > 0.0 {
                        needs.respect = (needs.respect + respect_gain).min(1.0);
                    }
                    // Building completion grants extra mood boost ("built something").
                    if disp.kind == DispositionKind::Building {
                        mood.modifiers
                            .push_back(crate::components::mental::MoodModifier {
                                amount: 0.2,
                                ticks_remaining: 100,
                                source: "built something".to_string(),
                            });
                    }
                }
                if let Some(mut hist) = history {
                    hist.record(ActionRecord {
                        action: current.action,
                        disposition: Some(disp.kind),
                        tick: time.tick,
                        outcome: if succeeded {
                            ActionOutcome::Success
                        } else {
                            ActionOutcome::Failure
                        },
                    });
                }
            }
            current.ticks_remaining = 0;
        }
    }

    for entity in chains_to_remove {
        commands.entity(entity).remove::<TaskChain>();
    }

    // Apply deferred grooming restorations from GroomOther steps.
    for (target, delta) in grooming_restorations {
        if let Ok((_, _, _, _, _, _, _, _, _, _, _, _, _, Some(mut g), _)) = cats.get_mut(target) {
            g.0 = (g.0 + delta).min(1.0);
        }
    }

    // Apply deferred mentor effects: grow apprentice's weakest teachable skill.
    // The apprentice may have a TaskChain (in `cats`) or not (in `unchained_skills`).
    for effect in &mentor_effects {
        // Try the unchained query first (more common — apprentice is usually idle).
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
        } else if let Ok((_, _, _, _, s, _, _, _, _, _, _, _, _, _, _)) =
            cats.get(effect.apprentice)
        {
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
            let _pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            let pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            // Skill with the largest teachable gap.
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
                // Write to whichever query holds this entity.
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
                } else if let Ok((_, _, _, _, mut s, _, _, _, _, _, _, _, _, _, _)) =
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

// ---------------------------------------------------------------------------
// cat_presence_tick
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::SystemState;

    fn make_world_with_no_wildlife() -> World {
        World::new()
    }

    fn _make_world_with_wildlife_at(pos: Position) -> World {
        let mut world = World::new();
        world.spawn((
            pos,
            WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
        ));
        world
    }

    fn default_disposition(kind: DispositionKind) -> Disposition {
        Disposition {
            kind,
            adopted_tick: 0,
            completions: 0,
            target_completions: 3,
            crafting_hint: None,
        }
    }

    fn mid_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    #[test]
    fn critical_health_interrupts_guarding() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health {
            current: 0.3,
            max: 1.0,
            injuries: Vec::new(),
        };
        let disposition = default_disposition(DispositionKind::Guarding);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            matches!(result, Some(InterruptReason::CriticalHealth)),
            "guarding cat at 30% HP should get CriticalHealth interrupt, got {result:?}"
        );
    }

    #[test]
    fn healthy_guarding_cat_not_interrupted() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health::default(); // 1.0
        let disposition = default_disposition(DispositionKind::Guarding);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            result.is_none(),
            "healthy guarding cat should not be interrupted, got {result:?}"
        );
    }

    #[test]
    fn critical_health_interrupts_resting() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health {
            current: 0.2,
            max: 1.0,
            injuries: Vec::new(),
        };
        let disposition = default_disposition(DispositionKind::Resting);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            matches!(result, Some(InterruptReason::CriticalHealth)),
            "CriticalHealth should fire for any disposition, got {result:?}"
        );
    }

    #[test]
    fn health_just_above_threshold_no_interrupt() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health {
            current: 0.5,
            max: 1.0,
            injuries: Vec::new(),
        };
        let disposition = default_disposition(DispositionKind::Guarding);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            result.is_none(),
            "cat at 50% HP (above 0.4 threshold) should not be interrupted, got {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // build_crafting_chain hint test
    // -----------------------------------------------------------------------

    #[test]
    fn chain_respects_prepare_hint() {
        use crate::components::magic::{GrowthStage, Harvestable, Herb, HerbKind, Inventory, Ward};
        use crate::components::skills::{MagicAffinity, Skills};
        use crate::resources::map::{Terrain, TileMap};
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let mut world = World::new();

        // Spawn a harvestable herb at (3, 3) — so GatherHerbs *could* fire.
        world.spawn((
            Herb {
                kind: HerbKind::HealingMoss,
                growth_stage: GrowthStage::Bloom,
                magical: false,
                twisted: false,
            },
            Harvestable,
            Position { x: 3, y: 3 },
        ));

        // Spawn injured cat entity before extracting queries (avoids borrow conflict).
        let injured_entity = world.spawn_empty().id();

        type CraftingQueries<'w, 's> = (
            Query<'w, 's, (Entity, &'static Herb, &'static Position), With<Harvestable>>,
            Query<
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
            Query<'w, 's, (&'static Ward, &'static Position)>,
            Query<'w, 's, (Entity, &'static StoredItems)>,
            Query<'w, 's, &'static Item>,
        );
        let mut state: SystemState<CraftingQueries> = SystemState::new(&mut world);
        let (herb_query, building_query, ward_query, stored_items_query, items_query) =
            state.get(&world);

        let pos = Position { x: 2, y: 2 }; // close to the herb
        let personality = mid_personality();
        let needs = Needs::default();
        let mut skills = Skills::default();
        skills.herbcraft = 0.5; // above threshold
        let magic_aff = MagicAffinity(0.0);
        let mut inventory = Inventory { slots: Vec::new() };
        inventory.add_herb(HerbKind::HealingMoss); // remedy herb

        // One injured cat nearby for the ApplyRemedy target.
        let injured_cats = vec![(injured_entity, Position { x: 4, y: 4 })];

        let map = TileMap::new(10, 10, Terrain::Grass);
        let d = SimConstants::default().disposition;
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let mut unmet_demand = crate::resources::UnmetDemand::default();

        let result = build_crafting_chain(
            &pos,
            &personality,
            &needs,
            &skills,
            &magic_aff,
            &inventory,
            &herb_query,
            &building_query,
            &ward_query,
            &[],
            &injured_cats,
            &map,
            false,
            &d,
            &mut rng,
            Some(CraftingHint::PrepareRemedy),
        );

        let (chain, action) = result.expect("should produce a chain with PrepareRemedy hint");
        assert_eq!(action, Action::Herbcraft);

        // The chain should contain a PrepareRemedy step — NOT GatherHerb.
        let has_prepare = chain
            .steps
            .iter()
            .any(|s| matches!(s.kind, StepKind::PrepareRemedy { .. }));
        let has_gather = chain
            .steps
            .iter()
            .any(|s| matches!(s.kind, StepKind::GatherHerb));
        assert!(
            has_prepare,
            "chain should contain PrepareRemedy step; steps: {:?}",
            chain.steps.iter().map(|s| &s.kind).collect::<Vec<_>>()
        );
        assert!(
            !has_gather,
            "chain should NOT contain GatherHerb when hint is PrepareRemedy; steps: {:?}",
            chain.steps.iter().map(|s| &s.kind).collect::<Vec<_>>()
        );
    }
}

/// Deposit cat territorial presence for patrolling/guarding cats and decay
/// the presence map globally. Runs every tick.
pub fn cat_presence_tick(
    cats: Query<(&Position, &CurrentAction), Without<Dead>>,
    mut presence_map: ResMut<crate::resources::CatPresenceMap>,
    constants: Res<SimConstants>,
) {
    let fc = &constants.fox_ecology;
    // Global decay — same rate as fox scent decay for symmetry.
    presence_map.decay_all(fc.scent_decay_per_tick);

    // Patrolling/guarding cats deposit presence at their position.
    let deposit = fc.scent_deposit; // reuse fox deposit rate for symmetry
    for (pos, action) in &cats {
        if matches!(
            action.action,
            Action::Patrol | Action::Fight | Action::Explore
        ) {
            presence_map.deposit(pos.x, pos.y, deposit);
        }
    }
}
