use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::step_toward;
use crate::ai::scoring::{
    aggregate_to_dispositions, apply_aspiration_bonuses, apply_cascading_bonuses,
    apply_colony_knowledge_bonuses, apply_directive_bonus, apply_fated_bonuses,
    apply_memory_bonuses, apply_preference_bonuses, apply_priority_bonus,
    enforce_survival_floor, score_actions, select_disposition_softmax, ScoringContext,
};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{ConstructionSite, CropState, StoredItems, Structure, StructureType};
use crate::components::items::{Item, ItemLocation};
use crate::components::coordination::{
    ActiveDirective, Directive, DirectiveKind, DirectiveQueue, PendingDelivery,
};
use crate::components::disposition::{
    ActionHistory, ActionOutcome, ActionRecord, Disposition, DispositionKind,
};
use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::Name;
use crate::components::magic::{Harvestable, Herb, Inventory, Ward};
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, InjuryKind, Needs, Position};
use crate::components::prey::PreyAnimal;
use crate::components::skills::{MagicAffinity, Skills};
use crate::components::task_chain::{FailurePolicy, StepKind, TaskChain, TaskStep};
use crate::components::wildlife::WildAnimal;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::food::FoodStores;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

// ===========================================================================
// check_anxiety_interrupts
// ===========================================================================

/// Checks every tick whether a cat's disposition should be interrupted by
/// critical need states or threats. Runs BEFORE disposition evaluation.
#[allow(clippy::type_complexity)]
pub fn check_anxiety_interrupts(
    mut query: Query<
        (
            Entity,
            &Needs,
            &Personality,
            &Position,
            &mut CurrentAction,
            Option<&mut ActionHistory>,
        ),
        (With<Disposition>, Without<Dead>),
    >,
    dispositions: Query<&Disposition, Without<Dead>>,
    wildlife: Query<&Position, With<WildAnimal>>,
    time: Res<TimeState>,
    map: Res<TileMap>,
    mut commands: Commands,
) {
    for (entity, needs, personality, pos, mut current, history) in &mut query {
        let Ok(disposition) = dispositions.get(entity) else {
            continue;
        };

        let interrupt = check_interrupt(needs, personality, pos, disposition, &wildlife);
        let Some(reason) = interrupt else { continue };

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
                    pos.x + (dx as f32 / len * 8.0) as i32,
                    pos.y + (dy as f32 / len * 8.0) as i32,
                );
                target.x = target.x.clamp(0, map.width - 1);
                target.y = target.y.clamp(0, map.height - 1);
                current.action = Action::Flee;
                current.ticks_remaining = 15;
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

enum InterruptReason {
    Starvation,
    Exhaustion,
    ThreatDetected { threat_pos: Position },
    CriticalSafety,
}

fn check_interrupt(
    needs: &Needs,
    personality: &Personality,
    pos: &Position,
    disposition: &Disposition,
    wildlife: &Query<&Position, With<WildAnimal>>,
) -> Option<InterruptReason> {
    // Resting, Hunting, and Foraging are exempt from hunger interrupts.
    // Resting is already handling it; Hunting/Foraging ARE the food solution.
    if !matches!(
        disposition.kind,
        DispositionKind::Resting | DispositionKind::Hunting | DispositionKind::Foraging
    ) {
        if needs.hunger < 0.15 {
            return Some(InterruptReason::Starvation);
        }
        if needs.energy < 0.10 {
            return Some(InterruptReason::Exhaustion);
        }
    }

    // Guarding, Hunting, and Foraging are exempt from threat interrupts.
    // Guards handle threats directly; hunters/foragers are focused on food.
    if !matches!(
        disposition.kind,
        DispositionKind::Guarding | DispositionKind::Hunting | DispositionKind::Foraging
    ) {
        // Check for nearby wildlife threats (3-tile awareness range).
        let nearest_threat = wildlife
            .iter()
            .filter(|wp| pos.manhattan_distance(wp) <= 3)
            .min_by_key(|wp| pos.manhattan_distance(wp));

        if let Some(threat_pos) = nearest_threat {
            let dist = pos.manhattan_distance(threat_pos) as f32;
            let threat_urgency = 1.0 - (dist / 3.0);
            // Bold cats resist fleeing: threshold is 0.3 (bold) to 0.7 (timid).
            let flee_threshold = 0.3 + personality.boldness * 0.4;
            if threat_urgency > flee_threshold {
                return Some(InterruptReason::ThreatDetected {
                    threat_pos: *threat_pos,
                });
            }
        }
    }

    // Critical safety check (exempt for Guarding).
    if disposition.kind != DispositionKind::Guarding && needs.safety < 0.2 {
        return Some(InterruptReason::CriticalSafety);
    }

    None
}

// ===========================================================================
// evaluate_dispositions
// ===========================================================================

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
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
) {
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
        .any(|(_, s, _, site, _)| site.is_none() && s.condition < 0.4);
    let has_garden = building_query.iter().any(|(_, s, _, site, _)| {
        s.kind == StructureType::Garden && site.is_none()
    });

    let herb_positions: Vec<(Entity, Position)> =
        herb_query.iter().map(|(e, _, p)| (e, *p)).collect();

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 =
                ward_query.iter().map(|(w, _)| w.strength).sum::<f32>() / ward_count as f32;
            avg < 0.3
        }
    };

    let injured_cat_set: std::collections::HashSet<Entity> = query
        .iter()
        .filter(|(_, _, _, _, _, _, _, health, _, _, _, _, _, _, _)| {
            health.injuries.iter().any(|i| !i.healed)
        })
        .map(|(e, _, _, _, _, _, _, _, _, _, _, _, _, _, _)| e)
        .collect();
    let colony_injury_count = injured_cat_set.len();

    let directive_snapshot: HashMap<Entity, (usize, Option<Directive>)> = directive_queue_query
        .iter()
        .map(|(entity, q)| (entity, (q.directives.len(), q.directives.first().cloned())))
        .collect();

    // Snapshot current actions for activity cascading.
    let action_snapshot: Vec<(Entity, Position, Action)> = query
        .iter()
        .map(|(entity, _, _, _, pos, _, _, _, _, _, current, _, _, _, _)| {
            (entity, *pos, current.action)
        })
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
        if !mentor_skills.iter().any(|&s| s > 0.6) {
            return false;
        }
        cat_positions.iter().any(|(other, other_pos)| {
            *other != entity
                && pos.manhattan_distance(other_pos) <= 10
                && skills_query.get(*other).is_ok_and(|other_skills| {
                    let other_arr = [
                        other_skills.hunting,
                        other_skills.foraging,
                        other_skills.herbcraft,
                        other_skills.building,
                        other_skills.combat,
                        other_skills.magic,
                    ];
                    mentor_skills
                        .iter()
                        .zip(other_arr.iter())
                        .any(|(&m, &a)| m > 0.6 && a < 0.3)
                })
        })
    };

    for (entity, _name, needs, personality, pos, memory, skills, health, magic_aff, inventory, mut current, aspirations, preferences, fated_love, fated_rival) in &mut query
    {
        if current.ticks_remaining != 0 {
            continue;
        }

        let can_hunt = has_nearby_tile(pos, &map, 15, |t| {
            matches!(t, Terrain::DenseForest | Terrain::LightForest)
        });
        let can_forage = has_nearby_tile(pos, &map, 10, |t| t.foraging_yield() > 0.0);

        let has_social_target = cat_positions
            .iter()
            .any(|(other, other_pos)| *other != entity && pos.manhattan_distance(other_pos) <= 10);

        let nearest_threat = wildlife_positions
            .iter()
            .filter(|(_, wp)| pos.manhattan_distance(wp) <= 5)
            .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

        let has_threat_nearby = nearest_threat.is_some();
        let allies_fighting_threat = if let Some(&(_, _)) = nearest_threat {
            action_snapshot
                .iter()
                .filter(|(e, _, action)| *e != entity && *action == Action::Fight)
                .count()
                .min(5) // cap for scoring sanity
        } else {
            0
        };

        let combat_effective = skills.combat + skills.hunting * 0.3;
        let is_incapacitated = health
            .injuries
            .iter()
            .any(|inj| inj.kind == InjuryKind::Severe && !inj.healed);

        let has_herbs_nearby = herb_positions
            .iter()
            .any(|(_, hp)| pos.manhattan_distance(hp) <= 10);

        let prey_nearby = prey_positions
            .iter()
            .any(|pp| pos.manhattan_distance(pp) <= 10);

        let (on_corrupted_tile, tile_corruption, on_special_terrain) =
            if map.in_bounds(pos.x, pos.y) {
                let tile = map.get(pos.x, pos.y);
                (
                    tile.corruption > 0.1,
                    tile.corruption,
                    matches!(tile.terrain, Terrain::FairyRing | Terrain::StandingStone),
                )
            } else {
                (false, 0.0, false)
            };

        let ctx = ScoringContext {
            needs,
            personality,
            food_available,
            can_hunt,
            can_forage,
            has_social_target,
            has_threat_nearby,
            allies_fighting_threat,
            combat_effective,
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
            colony_injury_count,
            ward_strength_low,
            on_corrupted_tile,
            tile_corruption,
            on_special_terrain,
            is_coordinator_with_directives: directive_snapshot
                .get(&entity)
                .is_some_and(|(len, _)| *len > 0),
            pending_directive_count: directive_snapshot
                .get(&entity)
                .map_or(0, |(len, _)| *len),
            has_mentoring_target: has_mentoring_target_fn(entity, pos, skills),
            prey_nearby,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false, // evaluate_dispositions runs for cats WITHOUT disposition
            active_disposition: None,
            tradition_location_bonus: 0.0, // TODO: wire up with LocationPreferences
        };

        let mut scores = score_actions(&ctx, &mut rng.rng);

        // Apply all bonus layers (identical to evaluate_actions).
        apply_memory_bonuses(&mut scores, memory, pos);
        if let Some(ref ck) = colony.knowledge {
            apply_colony_knowledge_bonuses(&mut scores, ck, pos);
        }
        if let Some(ref cp) = colony.priority {
            apply_priority_bonus(&mut scores, cp.active);
        }
        let mut nearby_actions = HashMap::new();
        for &(other_entity, other_pos, other_action) in &action_snapshot {
            if other_entity != entity && pos.manhattan_distance(&other_pos) <= 5 {
                *nearby_actions.entry(other_action).or_insert(0usize) += 1;
            }
        }
        apply_cascading_bonuses(&mut scores, &nearby_actions);
        if let Some(asp) = aspirations {
            apply_aspiration_bonuses(&mut scores, asp);
        }
        if let Some(pref) = preferences {
            apply_preference_bonuses(&mut scores, pref);
        }
        let love_visible = fated_love
            .filter(|l| l.awakened)
            .and_then(|l| cat_positions.iter().find(|(e, _)| *e == l.partner))
            .is_some_and(|(_, pp)| pos.manhattan_distance(pp) <= 15);
        let rival_nearby = fated_rival
            .filter(|r| r.awakened)
            .and_then(|r| cat_positions.iter().find(|(e, _)| *e == r.rival))
            .is_some_and(|(_, rp)| pos.manhattan_distance(rp) <= 15);
        apply_fated_bonuses(&mut scores, love_visible, rival_nearby);
        if let Ok(directive) = active_directive_query.get(entity) {
            let fondness_factor = relationships
                .get(entity, directive.coordinator)
                .map_or(0.5, |r| (r.fondness + 1.0) / 2.0);
            let bonus = directive.priority
                * directive.coordinator_social_weight
                * 0.5
                * personality.diligence
                * fondness_factor
                * (1.0 - personality.independence * 0.3)
                * (1.0 - personality.stubbornness * 0.4);
            apply_directive_bonus(&mut scores, directive.kind.to_action(), bonus);
        }
        enforce_survival_floor(&mut scores, needs);

        // Determine Groom routing.
        let self_groom_score = (1.0 - needs.warmth) * 0.8 * needs.level_suppression(1);
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
            if matches!(kind, DispositionKind::Coordinating | DispositionKind::Socializing) {
                *score = (*score - personality.independence * 0.2).max(0.0);
            }
        }

        // Select disposition via softmax.
        let chosen = select_disposition_softmax(&disposition_scores, &mut rng.rng);

        // Store top-3 action scores for diagnostics (unchanged from evaluate_actions).
        {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            sorted.truncate(3);
            current.last_scores = sorted;
        }

        // Insert the Disposition component. Chain creation happens in disposition_to_chain.
        // adopted_tick is 0 here; resolve_disposition_chains will set it from TimeState.
        commands.entity(entity).insert(Disposition::new(
            chosen,
            0,
            personality,
        ));

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
    relationships: Res<Relationships>,
    map: Res<TileMap>,
    food: Res<FoodStores>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
) {
    // Pre-collect cat position pairs for social target selection.
    let cat_pos_list: Vec<(Entity, Position)> = cat_positions.iter().map(|(e, p)| (e, *p)).collect();

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 =
                ward_query.iter().map(|(w, _)| w.strength).sum::<f32>() / ward_count as f32;
            avg < 0.3
        }
    };

    for (entity, needs, personality, pos, memory, skills, magic_aff, inventory, disposition, mut current) in &mut query {
        // Check completion FIRST: if the disposition is already done, remove it.
        if should_complete_disposition(&disposition, needs) {
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
            DispositionKind::Resting => {
                build_resting_chain(needs, pos, &building_query, &map, nearest_store, food.is_empty(), &mut rng.rng)
            }
            DispositionKind::Hunting => {
                build_hunting_chain(pos, &memory, &map, nearest_store, &mut rng.rng)
            }
            DispositionKind::Foraging => {
                build_foraging_chain(pos, &map, nearest_store, &mut rng.rng)
            }
            DispositionKind::Guarding => {
                build_guarding_chain(pos, &wildlife, &map, &mut rng.rng)
            }
            DispositionKind::Socializing => {
                build_socializing_chain(
                    entity, pos, personality, skills,
                    &cat_pos_list, &relationships, &skills_query,
                )
            }
            DispositionKind::Building => {
                build_building_chain(entity, pos, &building_query, &mut commands)
            }
            DispositionKind::Farming => {
                build_farming_chain(pos, &building_query)
            }
            DispositionKind::Crafting => {
                build_crafting_chain(
                    pos, personality, needs, skills, magic_aff, inventory,
                    &herb_query, &building_query, &ward_query,
                    &cat_pos_list, &map, ward_strength_low,
                    &mut rng.rng,
                )
            }
            DispositionKind::Coordinating => {
                build_coordinating_chain(
                    entity, pos, &directive_queue_query, &active_directive_query,
                    &cat_pos_list, &skills_query, &mut commands,
                )
            }
            DispositionKind::Exploring => {
                build_exploring_chain(pos, &map, &mut rng.rng)
            }
        };

        if let Some((chain, action)) = chain {
            current.action = action;
            current.ticks_remaining = u64::MAX;
            current.target_position = chain.steps.first().and_then(|s| s.target_position);
            current.target_entity = chain.steps.first().and_then(|s| s.target_entity);
            commands.entity(entity).insert(chain);
        } else {
            // No valid chain could be built — remove disposition and idle.
            commands.entity(entity).remove::<Disposition>();
            current.action = Action::Idle;
            current.ticks_remaining = 5;
            current.target_position = None;
            current.target_entity = None;
        }
    }
}

/// Check whether a disposition's goal is met and should be cleared.
fn should_complete_disposition(disposition: &Disposition, needs: &Needs) -> bool {
    match disposition.kind {
        DispositionKind::Resting => {
            needs.hunger >= 0.5 && needs.energy >= 0.5 && needs.warmth >= 0.50
        }
        _ => disposition.is_count_complete(),
    }
}

// ---------------------------------------------------------------------------
// Chain builders — one per disposition
// ---------------------------------------------------------------------------

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
            let mut steps = vec![
                TaskStep::new(StepKind::HuntPrey { patrol_dir }),
            ];
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
        let sleep_ticks = ((1.0 - needs.energy) * 20.0) as u64 + 5;
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
    pos: &Position,
    memory: &Memory,
    map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // HuntPrey handles all movement internally (scent search → stalk → pounce).
    // No MoveTo preamble — the cat starts hunting from its current position.
    let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
    let mut steps = vec![
        TaskStep::new(StepKind::HuntPrey { patrol_dir }),
    ];
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
    let mut steps = vec![
        TaskStep::new(StepKind::ForageItem { patrol_dir }),
    ];
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

fn build_guarding_chain(
    pos: &Position,
    wildlife: &Query<(Entity, &Position), With<WildAnimal>>,
    map: &TileMap,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // If threat nearby, fight it.
    let nearest_threat = wildlife
        .iter()
        .filter(|(_, wp)| pos.manhattan_distance(wp) <= 10)
        .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

    if let Some((threat_entity, threat_pos)) = nearest_threat {
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

    // Otherwise patrol the colony perimeter.
    let center_x = map.width / 2;
    let center_y = map.height / 2;
    let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
    let radius = 10.0_f32;
    let mut target = Position::new(
        center_x + (angle.cos() * radius) as i32,
        center_y + (angle.sin() * radius) as i32,
    );
    target.x = target.x.clamp(0, map.width - 1);
    target.y = target.y.clamp(0, map.height - 1);

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
) -> Option<(TaskChain, Action)> {
    // Pick best social target.
    let target = cat_positions
        .iter()
        .filter(|(other, other_pos)| {
            *other != entity && pos.manhattan_distance(other_pos) <= 15
        })
        .max_by(|(e_a, _), (e_b, _)| {
            let score_a = relationships
                .get(entity, *e_a)
                .map_or(0.0, |r| r.fondness * 0.6 + (1.0 - r.familiarity) * 0.4);
            let score_b = relationships
                .get(entity, *e_b)
                .map_or(0.0, |r| r.fondness * 0.6 + (1.0 - r.familiarity) * 0.4);
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p));

    let (target_entity, target_pos) = target?;

    // Decide sub-action: mentor if applicable, groom if warm, otherwise socialize.
    let can_mentor = {
        let mentor_skills = [
            skills.hunting, skills.foraging, skills.herbcraft,
            skills.building, skills.combat, skills.magic,
        ];
        mentor_skills.iter().any(|&s| s > 0.6)
            && skills_query.get(target_entity).is_ok_and(|other| {
                let other_arr = [
                    other.hunting, other.foraging, other.herbcraft,
                    other.building, other.combat, other.magic,
                ];
                mentor_skills.iter().zip(other_arr.iter())
                    .any(|(&m, &a)| m > 0.6 && a < 0.3)
            })
    };

    let (step_kind, action) = if can_mentor && personality.warmth > 0.5 {
        (StepKind::MentorCat, Action::Mentor)
    } else if personality.warmth > 0.7 {
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
    _commands: &mut Commands,
) -> Option<(TaskChain, Action)> {
    let target = building_query
        .iter()
        .filter(|(_, _, bpos, site, _)| site.is_some() || pos.manhattan_distance(bpos) <= 30)
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

#[allow(clippy::too_many_arguments)]
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
    _ward_query: &Query<(&Ward, &Position)>,
    _cat_positions: &[(Entity, Position)],
    map: &TileMap,
    ward_strength_low: bool,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    use crate::components::magic::{HerbKind, RemedyKind, WardKind};

    // Determine sub-mode: herbcraft gather, prepare, ward, or magic variants.
    // Simplified: prefer herbcraft if herbs nearby, else magic if qualified.

    let has_herbs_nearby = herb_query
        .iter()
        .any(|(_, _, hp)| pos.manhattan_distance(hp) <= 15);

    if has_herbs_nearby && skills.herbcraft > 0.1 {
        // Gather herbs.
        let nearest_herb = herb_query
            .iter()
            .filter(|(_, _, hp)| pos.manhattan_distance(hp) <= 15)
            .min_by_key(|(_, _, hp)| pos.manhattan_distance(hp));

        if let Some((herb_entity, _, herb_pos)) = nearest_herb {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(*herb_pos),
                    TaskStep::new(StepKind::GatherHerb)
                        .with_position(*herb_pos)
                        .with_entity(herb_entity),
                ],
                FailurePolicy::AbortChain,
            );
            return Some((chain, Action::Herbcraft));
        }
    }

    if inventory.has_remedy_herb() {
        // Prepare remedy — find injured cat.
        let remedy_kind = if inventory.has_herb(HerbKind::HealingMoss) {
            RemedyKind::HealingPoultice
        } else if inventory.has_herb(HerbKind::Moonpetal) {
            RemedyKind::EnergyTonic
        } else {
            RemedyKind::MoodTonic
        };

        // Find workshop for bonus.
        let workshop_pos = building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Workshop && site.is_none())
            .map(|(_, _, bpos, _, _)| *bpos)
            .min_by_key(|bpos| pos.manhattan_distance(bpos));

        let mut steps = Vec::new();
        if let Some(wp) = workshop_pos {
            steps.push(TaskStep::new(StepKind::MoveTo).with_position(wp));
        }
        steps.push(TaskStep::new(StepKind::PrepareRemedy { remedy: remedy_kind }));

        let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
        return Some((chain, Action::Herbcraft));
    }

    if inventory.has_ward_herb() && ward_strength_low {
        // Set ward.
        let center_x = map.width / 2;
        let center_y = map.height / 2;
        let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        let radius = 10.0_f32;
        let mut ward_pos = Position::new(
            center_x + (angle.cos() * radius) as i32,
            center_y + (angle.sin() * radius) as i32,
        );
        ward_pos.x = ward_pos.x.clamp(0, map.width - 1);
        ward_pos.y = ward_pos.y.clamp(0, map.height - 1);

        let chain = TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(ward_pos),
                TaskStep::new(StepKind::SetWard {
                    kind: WardKind::Thornward,
                })
                .with_position(ward_pos),
            ],
            FailurePolicy::AbortChain,
        );
        return Some((chain, Action::Herbcraft));
    }

    // Magic: scry, cleanse, commune.
    if magic_aff.0 > 0.3 && skills.magic > 0.2 {
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

        // Default: scry.
        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::Scry).with_position(*pos)],
            FailurePolicy::AbortChain,
        );
        return Some((chain, Action::PracticeMagic));
    }

    None
}

fn build_coordinating_chain(
    entity: Entity,
    pos: &Position,
    directive_queue_query: &Query<(Entity, &DirectiveQueue)>,
    active_directive_query: &Query<&ActiveDirective>,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
    commands: &mut Commands,
) -> Option<(TaskChain, Action)> {
    let (_, queue) = directive_queue_query.get(entity).ok()?;
    let directive = queue.directives.first()?.clone();

    // Find the best target for this directive.
    let target = cat_positions
        .iter()
        .filter(|(e, _)| *e != entity)
        .filter(|(e, _)| active_directive_query.get(*e).is_err())
        .filter(|(_, p)| pos.manhattan_distance(p) <= 30)
        .max_by(|(e_a, p_a), (e_b, p_b)| {
            let skill_a = skills_query.get(*e_a).map_or(0.0, |s| match directive.kind {
                DirectiveKind::Hunt => s.hunting,
                DirectiveKind::Forage => s.foraging,
                DirectiveKind::Build => s.building,
                DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
            });
            let skill_b = skills_query.get(*e_b).map_or(0.0, |s| match directive.kind {
                DirectiveKind::Hunt => s.hunting,
                DirectiveKind::Forage => s.foraging,
                DirectiveKind::Build => s.building,
                DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
            });
            let rank_a = skill_a - pos.manhattan_distance(p_a) as f32 * 0.01;
            let rank_b = skill_b - pos.manhattan_distance(p_b) as f32 * 0.01;
            rank_a
                .partial_cmp(&rank_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p));

    let (target_entity, target_pos) = target?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target_pos),
            TaskStep::new(StepKind::DeliverDirective)
                .with_position(target_pos)
                .with_entity(target_entity),
        ],
        FailurePolicy::AbortChain,
    );

    commands.entity(entity).insert(PendingDelivery(directive));

    Some((chain, Action::Coordinate))
}

fn build_exploring_chain(
    pos: &Position,
    map: &TileMap,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    let dx: i32 = rng.random_range(-20i32..=20);
    let dy: i32 = rng.random_range(-20i32..=20);
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
// Scent detection helper
// ===========================================================================

/// Check if a cat can smell prey based on wind direction and terrain.
///
/// The cat must be roughly downwind of the prey (scent blows from prey toward
/// cat). Forest terrain reduces scent carry. Calm wind reduces range.
fn can_smell_prey(
    cat_pos: &Position,
    prey_pos: &Position,
    wind: &crate::resources::wind::WindState,
    map: &TileMap,
) -> bool {
    let dx = (prey_pos.x - cat_pos.x) as f32;
    let dy = (prey_pos.y - cat_pos.y) as f32;
    let dist = cat_pos.manhattan_distance(prey_pos) as f32;
    if dist == 0.0 {
        return true;
    }
    // Normalize prey→cat vector (direction scent travels).
    let (nx, ny) = (dx / dist, dy / dist);
    let (wx, wy) = wind.direction();
    // Positive dot = cat is downwind of prey.
    let dot = wx * nx + wy * ny;
    if dot < 0.3 {
        return false;
    }
    // Terrain at prey position affects scent dispersal.
    let terrain_mod = if map.in_bounds(prey_pos.x, prey_pos.y) {
        match map.get(prey_pos.x, prey_pos.y).terrain {
            Terrain::DenseForest => 0.25,
            Terrain::LightForest => 0.5,
            _ => 1.0,
        }
    } else {
        1.0
    };
    let scent_range = 20.0 * wind.strength * terrain_mod;
    dist <= scent_range
}

/// Move one tile in direction (dx, dy). If blocked, try perpendicular or
/// reverse. Returns the new position. Guaranteed to attempt movement — never
/// returns the original position without trying alternatives.
fn patrol_move(
    pos: &Position,
    dx: i32,
    dy: i32,
    map: &TileMap,
) -> Position {
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
            Option<&mut Disposition>,
            Option<&mut ActionHistory>,
            &mut HuntingPriors,
        ),
        (Without<Dead>, Without<Structure>, Without<PreyAnimal>),
    >,
    mut prey_query: Query<(Entity, &Position, &mut PreyAnimal)>,
    mut stores_query: Query<&mut StoredItems>,
    items_query: Query<&Item>,
    // Disjoint from `cats` (which requires TaskChain) — reads skills of non-chain
    // entities like apprentices being mentored.
    mut unchained_skills: Query<&mut Skills, (Without<TaskChain>, Without<Structure>)>,
    map: Res<TileMap>,
    wind: Res<crate::resources::wind::WindState>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<crate::resources::narrative::NarrativeLog>,
    time: Res<TimeState>,
    mut rng: ResMut<SimRng>,
    mut colony_map: ResMut<ColonyHuntingMap>,
    mut commands: Commands,
) {
    // Deferred effects applied after the main loop (avoids mutable borrow conflicts).
    struct MentorEffect {
        apprentice: Entity,
        mentor_skills: Skills,
    }
    let mut mentor_effects: Vec<MentorEffect> = Vec::new();
    let mut chains_to_remove: Vec<Entity> = Vec::new();

    for (cat_entity, mut chain, mut current, mut pos, mut skills, mut needs, mut inventory, personality, disposition, history, mut hunting_priors) in &mut cats {
        let Some(step) = chain.current_mut() else {
            // Chain exhausted — handle completion or failure.
            chains_to_remove.push(cat_entity);
            if let Some(mut disp) = disposition {
                let outcome = if chain.is_succeeded() {
                    disp.completions += 1;
                    // Successful completions earn respect proportional to contribution.
                    let respect_gain = match disp.kind {
                        DispositionKind::Hunting => 0.03,
                        DispositionKind::Foraging => 0.01,
                        DispositionKind::Guarding => 0.02,
                        DispositionKind::Building => 0.04,
                        DispositionKind::Coordinating => 0.05,
                        DispositionKind::Socializing => 0.02,
                        _ => 0.0,
                    };
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
                | StepKind::DeliverDirective
        );
        if !is_disposition_step {
            continue;
        }

        // Ensure step is in progress.
        if matches!(step.status, crate::components::task_chain::StepStatus::Pending) {
            step.status = crate::components::task_chain::StepStatus::InProgress { ticks_elapsed: 0 };
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
                    let Ok((_, prey_pos, _)) = prey_query.get(target_entity) else {
                        // Prey despawned (caught by another cat or died).
                        step.target_entity = None;
                        continue; // Re-evaluate next tick as Search.
                    };
                    let prey_pos = *prey_pos;
                    let dist = pos.manhattan_distance(&prey_pos);

                    // Determine pounce range from patience.
                    let pounce_range: i32 = if personality.patience > 0.7 {
                        1
                    } else if personality.patience < 0.3 {
                        2
                    } else {
                        1
                    };

                    if dist <= pounce_range {
                        // === POUNCE ===
                        let distance_mod = if dist <= 1 { 1.0 } else { 0.6 };
                        let success_chance = 0.6
                            * (0.7 + skills.hunting * 0.3)
                            * distance_mod;

                        if rng.rng.random::<f32>() < success_chance {
                            // Catch!
                            let prey_data = prey_query.get(target_entity).unwrap().2;
                            let item_kind = prey_data.species.item_kind();
                            let species_name = prey_data.species.name();
                            commands.entity(target_entity).despawn();

                            if !inventory.is_full() {
                                inventory.slots.push(ItemSlot::Item(item_kind));
                            }
                            skills.hunting += skills.growth_rate() * 0.01;

                            log.push(
                                time.tick,
                                format!("A cat catches a {species_name}."),
                                crate::resources::narrative::NarrativeTier::Action,
                            );
                            chain.advance();
                            hunting_priors.record_catch(&prey_pos);
                        } else {
                            // Pounce failed — prey bolts.
                            if let Ok((_, _, mut prey_animal)) = prey_query.get_mut(target_entity) {
                                prey_animal.ai_state = PreyAiState::Fleeing {
                                    from: cat_entity,
                                    ticks: 0,
                                };
                            }

                            log.push(
                                time.tick,
                                format!("The prey bolts."),
                                crate::resources::narrative::NarrativeTier::Micro,
                            );

                            if personality.boldness > 0.7 {
                                // Bold cat: brief chase (keep target, fail after 5 more ticks).
                                if ticks > 5 {
                                    chain.fail_current("prey escaped after chase".into());
                                }
                                // Otherwise keep pursuing — step_toward will run next tick.
                            } else {
                                // Cat watches prey escape, then gives up.
                                chain.fail_current("pounce missed".into());
                            }
                        }
                    } else if dist <= 5 {
                        // === STALK === (slow, deliberate)
                        // Move every 2 ticks (half speed).
                        if ticks % 2 == 0 {
                            if let Some(next) = step_toward(&pos, &prey_pos, &map) {
                                *pos = next;
                            }
                        }
                        // Anxiety check: nervous cat spooks prey.
                        if personality.anxiety > 0.7 && rng.rng.random::<f32>() < 0.15 {
                            if let Ok((_, _, mut prey_animal)) = prey_query.get_mut(target_entity) {
                                prey_animal.ai_state = PreyAiState::Fleeing {
                                    from: cat_entity,
                                    ticks: 0,
                                };
                            }
                            chain.fail_current("spooked the prey".into());
                        }
                    } else {
                        // === APPROACH === (closing distance at full speed)
                        if let Some(next) = step_toward(&pos, &prey_pos, &map) {
                            *pos = next;
                        }
                        // Give up only if prey has fled far beyond detection range.
                        // Cats are persistent hunters — they don't lose interest just
                        // because the wind shifted mid-chase.
                        if dist > 25 {
                            step.target_entity = None;
                        }
                    }
                } else {
                    // === SEARCH === (no target yet — move toward best-known hunting ground)
                    // Priority: personal belief > colony belief > wind > patrol_dir.
                    let belief_dir = hunting_priors.best_direction(&pos, 25);
                    let colony_dir = colony_map.beliefs.best_direction(&pos, 25);
                    let (wx, wy) = wind.direction();
                    let (mut dx, mut dy) = if let Some((bx, by)) = belief_dir {
                        (bx, by)
                    } else if let Some((cx, cy)) = colony_dir {
                        (cx, cy)
                    } else if wx.abs() > 0.3 || wy.abs() > 0.3 {
                        (-(wx.signum() as i32), -(wy.signum() as i32))
                    } else {
                        *patrol_dir
                    };
                    // 20% jitter — enough to explore but not lose the gradient.
                    if rng.rng.random::<f32>() < 0.20 {
                        dx = rng.rng.random_range(-1i32..=1);
                        dy = rng.rng.random_range(-1i32..=1);
                    }
                    // Ensure we actually move (avoid 0,0).
                    if dx == 0 && dy == 0 { dx = 1; }
                    *pos = patrol_move(&pos, dx, dy, &map);

                    // Visual detection: spot nearby prey within 3 tiles.
                    let visible_prey = prey_query
                        .iter()
                        .filter(|(_, pp, _)| pos.manhattan_distance(pp) <= 3)
                        .min_by_key(|(_, pp, _)| pos.manhattan_distance(pp));

                    if let Some((prey_entity, _prey_pos_ref, _)) = visible_prey {
                        step.target_entity = Some(prey_entity);
                    } else {
                        // Scan for prey scent (wind-dependent, longer range).
                        let scented_prey = prey_query
                            .iter()
                            .filter(|(_, pp, _)| can_smell_prey(&pos, pp, &wind, &map))
                            .min_by_key(|(_, pp, _)| pos.manhattan_distance(pp));

                        if let Some((prey_entity, prey_pos_ref, _)) = scented_prey {
                            step.target_entity = Some(prey_entity);
                            hunting_priors.record_scent(prey_pos_ref);
                            log.push(
                                time.tick,
                                "A cat catches a scent on the wind.".to_string(),
                                crate::resources::narrative::NarrativeTier::Micro,
                            );
                        }
                    }

                    if ticks > 100 {
                        hunting_priors.record_failed_search(&pos, ticks);
                        chain.fail_current("no scent found".into());
                    }
                }
            }

            StepKind::ForageItem { patrol_dir } => {
                // Active foraging: directional patrol, check each tile.
                // Use patrol_dir with jitter and reverse-on-blocked (wildlife pattern).
                let mut dx = patrol_dir.0;
                let mut dy = patrol_dir.1;
                if dx == 0 && dy == 0 { dx = 1; } // ensure movement
                // 10% jitter.
                if rng.rng.random::<f32>() < 0.10 {
                    dx = rng.rng.random_range(-1i32..=1);
                    dy = rng.rng.random_range(-1i32..=1);
                    if dx == 0 && dy == 0 { dx = 1; }
                }
                *pos = patrol_move(&pos, dx, dy, &map);

                // Check current tile for forage yield.
                if map.in_bounds(pos.x, pos.y) {
                    let tile = map.get(pos.x, pos.y);
                    let forage_yield = tile.terrain.foraging_yield();
                    if forage_yield > 0.0 && rng.rng.random::<f32>() < forage_yield * 0.25 {
                        use crate::components::items::ItemKind;
                        use crate::components::magic::ItemSlot;
                        let item_kind = match tile.terrain {
                            Terrain::DenseForest => {
                                if rng.rng.random::<bool>() { ItemKind::Mushroom } else { ItemKind::Nuts }
                            }
                            Terrain::LightForest => {
                                if rng.rng.random::<bool>() { ItemKind::Nuts } else { ItemKind::Berries }
                            }
                            _ => {
                                if rng.rng.random::<bool>() { ItemKind::Berries } else { ItemKind::Roots }
                            }
                        };
                        if !inventory.is_full() {
                            inventory.slots.push(ItemSlot::Item(item_kind));
                        }
                        skills.foraging += skills.growth_rate() * 0.008;
                        chain.advance();
                    } else if ticks > 40 {
                        chain.fail_current("nothing found while foraging".into());
                    }
                }
            }

            StepKind::DepositAtStores => {
                // Transfer carried items from inventory into the store.
                if let Some(store_entity) = step.target_entity {
                    use crate::components::magic::ItemSlot;
                    let food_items: Vec<crate::components::items::ItemKind> = inventory.slots
                        .iter()
                        .filter_map(|slot| match slot {
                            ItemSlot::Item(kind) if kind.is_food() => Some(*kind),
                            _ => None,
                        })
                        .collect();
                    // Remove deposited items from inventory.
                    inventory.slots.retain(|slot| !matches!(slot, ItemSlot::Item(k) if k.is_food()));
                    // Spawn real item entities in the store.
                    if let Ok(mut stored) = stores_query.get_mut(store_entity) {
                        let quality = (0.3 + skills.hunting * 0.4).clamp(0.0, 1.0);
                        for kind in food_items {
                            let item_entity = commands
                                .spawn(Item::new(kind, quality, ItemLocation::StoredIn(store_entity)))
                                .id();
                            stored.add(item_entity, StructureType::Stores);
                        }
                    }
                }
                chain.advance();
            }

            StepKind::EatAtStores => {
                if ticks >= 5 {
                    if let Some(store_entity) = step.target_entity {
                        if let Ok(mut stored) = stores_query.get_mut(store_entity) {
                            let food_item = stored.items.iter()
                                .copied()
                                .find(|&item_e| {
                                    items_query.get(item_e)
                                        .is_ok_and(|item| item.kind.is_food())
                                });
                            if let Some(item_entity) = food_item {
                                if let Ok(item) = items_query.get(item_entity) {
                                    needs.hunger = (needs.hunger + item.kind.food_value()).min(1.0);
                                }
                                stored.remove(item_entity);
                                commands.entity(item_entity).despawn();
                            }
                        }
                    }
                    chain.advance();
                }
            }

            StepKind::Sleep { ticks: duration } => {
                // Restore energy and warmth each tick (matches legacy pacing).
                needs.energy = (needs.energy + 0.02).min(1.0);
                needs.warmth = (needs.warmth + 0.01).min(1.0);
                if ticks >= *duration {
                    chain.advance();
                }
            }

            StepKind::SelfGroom => {
                if ticks >= 8 {
                    needs.warmth = (needs.warmth + 0.15).min(1.0);
                    chain.advance();
                }
            }

            StepKind::Socialize => {
                if let Some(target_entity) = step.target_entity {
                    // Per-tick social restoration while adjacent.
                    needs.social = (needs.social + 0.03).min(1.0);
                    relationships.modify_fondness(cat_entity, target_entity, 0.005);
                    relationships.modify_familiarity(cat_entity, target_entity, 0.003);
                    relationships.get_or_insert(cat_entity, target_entity).last_interaction = time.tick;
                    // Share hunting knowledge during social interaction.
                    colony_map.absorb(&hunting_priors, 0.05);
                    hunting_priors.learn_from(&colony_map.beliefs, 0.1);
                }
                if ticks >= 10 {
                    chain.advance();
                }
            }

            StepKind::GroomOther => {
                if let Some(target_entity) = step.target_entity {
                    // Per-tick social + warmth while grooming.
                    needs.social = (needs.social + 0.02).min(1.0);
                    relationships.modify_fondness(cat_entity, target_entity, 0.008);
                    relationships.modify_familiarity(cat_entity, target_entity, 0.003);
                    relationships.get_or_insert(cat_entity, target_entity).last_interaction = time.tick;
                    // Share hunting knowledge during grooming (more intimate interaction).
                    colony_map.absorb(&hunting_priors, 0.08);
                    hunting_priors.learn_from(&colony_map.beliefs, 0.12);
                }
                if ticks >= 8 {
                    needs.warmth = (needs.warmth + 0.05).min(1.0);
                    chain.advance();
                }
            }

            StepKind::MentorCat => {
                if let Some(target_entity) = step.target_entity {
                    // Per-tick teaching effects.
                    needs.mastery = (needs.mastery + 0.02).min(1.0);
                    needs.social = (needs.social + 0.01).min(1.0);
                    needs.respect = (needs.respect + 0.002).min(1.0);
                    relationships.modify_fondness(cat_entity, target_entity, 0.005);
                    relationships.modify_familiarity(cat_entity, target_entity, 0.003);
                    relationships.get_or_insert(cat_entity, target_entity).last_interaction = time.tick;
                }
                if ticks >= 12 {
                    // Defer apprentice skill growth (applied after the loop).
                    if let Some(target_entity) = step.target_entity {
                        mentor_effects.push(MentorEffect {
                            apprentice: target_entity,
                            mentor_skills: skills.clone(),
                        });
                    }
                    chain.advance();
                }
            }

            StepKind::PatrolTo => {
                // Walk to target, scanning for threats.
                let Some(target) = step.target_position else {
                    chain.fail_current("no patrol target".into());
                    continue;
                };
                if pos.manhattan_distance(&target) == 0 {
                    needs.safety = (needs.safety + 0.05).min(1.0);
                    chain.advance();
                } else if let Some(next) = step_toward(&pos, &target, &map) {
                    *pos = next;
                    // Small safety boost per tile patrolled.
                    needs.safety = (needs.safety + 0.005).min(1.0);
                } else if ticks > 30 {
                    chain.fail_current("stuck patrolling".into());
                }
            }

            StepKind::FightThreat => {
                if ticks >= 30 {
                    skills.combat += skills.growth_rate() * 0.015;
                    needs.safety = (needs.safety + 0.2).min(1.0);
                    chain.advance();
                }
            }

            StepKind::Survey => {
                if ticks >= 5 {
                    // Small exploration satisfaction.
                    needs.purpose = (needs.purpose + 0.03).min(1.0);
                    chain.advance();
                }
            }

            StepKind::DeliverDirective => {
                if ticks >= 5 {
                    needs.respect = (needs.respect + 0.05).min(1.0);
                    needs.social = (needs.social + 0.05).min(1.0);
                    chain.advance();
                }
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
                    let respect_gain = match disp.kind {
                        DispositionKind::Hunting => 0.03,
                        DispositionKind::Foraging => 0.01,
                        DispositionKind::Guarding => 0.02,
                        DispositionKind::Building => 0.04,
                        DispositionKind::Coordinating => 0.05,
                        DispositionKind::Socializing => 0.02,
                        _ => 0.0,
                    };
                    if respect_gain > 0.0 {
                        needs.respect = (needs.respect + respect_gain).min(1.0);
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

    // Apply deferred mentor effects: grow apprentice's weakest teachable skill.
    // The apprentice may have a TaskChain (in `cats`) or not (in `unchained_skills`).
    for effect in &mentor_effects {
        // Try the unchained query first (more common — apprentice is usually idle).
        let app_skills_result = if let Ok(s) = unchained_skills.get(effect.apprentice) {
            Some((s.hunting, s.foraging, s.herbcraft, s.building, s.combat, s.magic, s.growth_rate()))
        } else if let Ok((_, _, _, _, s, _, _, _, _, _, _)) = cats.get(effect.apprentice) {
            Some((s.hunting, s.foraging, s.herbcraft, s.building, s.combat, s.magic, s.growth_rate()))
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
            let pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            // Skill with the largest teachable gap (mentor > 0.6, apprentice < 0.3).
            if let Some((idx, _)) = pairs
                .iter()
                .enumerate()
                .filter(|(_, (m, a))| *m > 0.6 && *a < 0.3)
                .max_by(|(_, (am, aa)), (_, (bm, ba))| {
                    (am - aa)
                        .partial_cmp(&(bm - ba))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                let growth = growth_rate * 0.04; // 2× normal
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
                } else if let Ok((_, _, _, _, mut s, _, _, _, _, _, _)) =
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
