use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::scoring::{
    apply_aspiration_bonuses, apply_cascading_bonuses, apply_colony_knowledge_bonuses,
    apply_directive_bonus, apply_fated_bonuses, apply_memory_bonuses, apply_preference_bonuses,
    apply_priority_bonus, enforce_survival_floor, score_actions, select_action_softmax,
    ScoringContext,
};
use crate::ai::{Action, CurrentAction};
use crate::components::coordination::{
    ActiveDirective, DirectiveQueue, PendingDelivery,
};
use crate::components::magic::{Harvestable, Herb, Inventory, Ward};
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::skills::{MagicAffinity, Skills};
use crate::components::identity::Name;
use crate::components::prey::PreyAnimal;
use crate::components::wildlife::WildAnimal;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::food::FoodStores;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;

// ---------------------------------------------------------------------------
// Terrain helpers
// ---------------------------------------------------------------------------

/// Find the nearest tile matching a predicate within a search radius.
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

/// Check whether any tile matching a predicate exists within radius.
fn has_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> bool {
    find_nearest_tile(from, map, radius, predicate).is_some()
}

/// Pick a hunt target: prefer a remembered successful hunt location, fall back
/// to the nearest forest tile.
fn pick_hunt_target(
    pos: &Position,
    map: &TileMap,
    memory: &Memory,
    rng: &mut impl Rng,
) -> Option<Position> {
    // Check memory for ResourceFound entries (successful past hunts).
    let remembered: Vec<&Position> = memory
        .events
        .iter()
        .filter(|e| e.event_type == MemoryType::ResourceFound && e.location.is_some())
        .filter_map(|e| e.location.as_ref())
        .collect();

    if !remembered.is_empty() {
        let idx = rng.random_range(0..remembered.len());
        return Some(*remembered[idx]);
    }

    // Fall back to nearest forest tile.
    find_nearest_tile(pos, map, 15, |t| {
        matches!(t, Terrain::DenseForest | Terrain::LightForest)
    })
}

/// Pick the best social target: among visible cats within range, prefer high
/// fondness with a novelty bonus for low familiarity.
fn pick_social_target(
    entity: Entity,
    pos: &Position,
    cat_positions: &[(Entity, Position)],
    relationships: &Relationships,
    fondness_weight: f32,
    novelty_weight: f32,
) -> Option<(Entity, Position)> {
    cat_positions
        .iter()
        .filter(|(other, other_pos)| {
            *other != entity && pos.manhattan_distance(other_pos) <= 10
        })
        .max_by(|(e_a, _), (e_b, _)| {
            let score_a = relationships
                .get(entity, *e_a)
                .map_or(0.0, |r| r.fondness * fondness_weight + (1.0 - r.familiarity) * novelty_weight);
            let score_b = relationships
                .get(entity, *e_b)
                .map_or(0.0, |r| r.fondness * fondness_weight + (1.0 - r.familiarity) * novelty_weight);
            score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p))
}

/// Check whether a valid mentoring target exists for this cat.
///
/// A target is valid when the mentor has any skill > 0.6 and the candidate has
/// the same skill < 0.3, within 10 tiles.
fn has_mentoring_target(
    entity: Entity,
    pos: &Position,
    skills: &Skills,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
) -> bool {
    let mentor_skills = [
        skills.hunting, skills.foraging, skills.herbcraft,
        skills.building, skills.combat, skills.magic,
    ];
    if !mentor_skills.iter().any(|&s| s > 0.6) {
        return false;
    }
    cat_positions.iter().any(|(other, other_pos)| {
        *other != entity
            && pos.manhattan_distance(other_pos) <= 10
            && skills_query.get(*other).is_ok_and(|other_skills| {
                let other_arr = [
                    other_skills.hunting, other_skills.foraging, other_skills.herbcraft,
                    other_skills.building, other_skills.combat, other_skills.magic,
                ];
                mentor_skills.iter().zip(other_arr.iter()).any(|(&m, &a)| m > 0.6 && a < 0.3)
            })
    })
}

/// Pick the best mentoring target: nearby cat with the largest skill gap
/// where the mentor has skill > 0.6 and the apprentice has skill < 0.3.
fn pick_mentoring_target(
    entity: Entity,
    pos: &Position,
    skills: &Skills,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
) -> Option<(Entity, Position)> {
    let mentor_skills = [
        skills.hunting, skills.foraging, skills.herbcraft,
        skills.building, skills.combat, skills.magic,
    ];

    cat_positions
        .iter()
        .filter(|(other, other_pos)| {
            *other != entity && pos.manhattan_distance(other_pos) <= 10
        })
        .filter_map(|(other, other_pos)| {
            let other_skills = skills_query.get(*other).ok()?;
            let other_arr = [
                other_skills.hunting, other_skills.foraging, other_skills.herbcraft,
                other_skills.building, other_skills.combat, other_skills.magic,
            ];
            // Find the maximum teachable skill gap.
            let max_gap = mentor_skills.iter().zip(other_arr.iter())
                .filter(|(&m, &a)| m > 0.6 && a < 0.3)
                .map(|(&m, &a)| m - a)
                .fold(0.0f32, f32::max);
            if max_gap > 0.0 {
                Some((*other, *other_pos, max_gap))
            } else {
                None
            }
        })
        .max_by(|(_, _, gap_a), (_, _, gap_b)| {
            gap_a.partial_cmp(gap_b).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p, _)| (e, p))
}

/// Pick the best target cat for a coordination directive.
///
/// Prefers: non-coordinator, not already directed, nearby, high relevant skill.
fn pick_directive_target(
    coordinator: Entity,
    coordinator_pos: &Position,
    directive: &crate::components::coordination::Directive,
    cat_positions: &[(Entity, Position)],
    coordinator_entities: &std::collections::HashSet<Entity>,
    active_directive_query: &Query<&ActiveDirective>,
    skills_query: &Query<&Skills, Without<Dead>>,
) -> Option<(Entity, Position)> {
    use crate::components::coordination::DirectiveKind;

    cat_positions
        .iter()
        .filter(|(e, _)| *e != coordinator)
        // Prefer non-coordinators (coordinators can still be targeted, but ranked lower).
        // Exclude cats already directed.
        .filter(|(e, _)| active_directive_query.get(*e).is_err())
        .filter(|(_, p)| coordinator_pos.manhattan_distance(p) <= 30)
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
            // Rank by: skill descending, then distance ascending (prefer nearby).
            let is_coord_a = coordinator_entities.contains(e_a);
            let is_coord_b = coordinator_entities.contains(e_b);
            let rank_a = skill_a + if is_coord_a { -0.5 } else { 0.0 }
                - coordinator_pos.manhattan_distance(p_a) as f32 * 0.01;
            let rank_b = skill_b + if is_coord_b { -0.5 } else { 0.0 }
                - coordinator_pos.manhattan_distance(p_b) as f32 * 0.01;
            rank_a
                .partial_cmp(&rank_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p))
}

// ---------------------------------------------------------------------------
// emit_periodic_events system
// ---------------------------------------------------------------------------

/// Emit periodic food-level snapshots to the event log.
pub fn emit_periodic_events(
    time: Res<crate::resources::time::TimeState>,
    food: Res<FoodStores>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    if let Some(ref mut log) = event_log {
        if time.tick.is_multiple_of(100) {
            log.push(time.tick, EventKind::FoodLevel {
                current: food.current,
                capacity: food.capacity,
                fraction: food.fraction(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// evaluate_actions system
// ---------------------------------------------------------------------------

/// Score available actions for every cat whose current action has finished
/// (`ticks_remaining == 0`) and assign the best-scoring next action.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn evaluate_actions(
    mut query: Query<(
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
    ), (Without<Dead>, Without<crate::components::disposition::Disposition>)>,
    all_positions: Query<(Entity, &Position, Option<&PreyAnimal>), Without<Dead>>,
    wildlife: Query<(Entity, &Position), With<WildAnimal>>,
    building_query: Query<(
        Entity,
        &crate::components::building::Structure,
        &Position,
        Option<&crate::components::building::ConstructionSite>,
        Option<&crate::components::building::CropState>,
    )>,
    herb_query: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    ward_query: Query<(&Ward, &Position)>,
    mut directive_queue_query: Query<(Entity, &mut DirectiveQueue)>,
    active_directive_query: Query<&ActiveDirective>,
    skills_query: Query<&Skills, Without<Dead>>,
    map: Res<TileMap>,
    food: Res<FoodStores>,
    relationships: Res<Relationships>,
    colony_knowledge: Option<Res<crate::resources::colony_knowledge::ColonyKnowledge>>,
    colony_priority: Option<Res<crate::resources::colony_priority::ColonyPriority>>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
) {
    let food_available = !food.is_empty();
    let food_fraction = food.fraction();

    // Collect all living cat positions once for social target selection,
    // and prey positions for hunt proximity checks.
    let mut cat_positions: Vec<(Entity, Position)> = Vec::new();
    let mut prey_positions: Vec<Position> = Vec::new();
    for (e, p, prey) in all_positions.iter() {
        cat_positions.push((e, *p));
        if prey.is_some() {
            prey_positions.push(*p);
        }
    }

    // Snapshot wildlife positions.
    let wildlife_positions: Vec<(Entity, Position)> = wildlife
        .iter()
        .map(|(e, p)| (e, *p))
        .collect();

    // Scan buildings for scoring context.
    let has_construction_site = building_query
        .iter()
        .any(|(_, _, _, site, _)| site.is_some());
    let has_damaged_building = building_query
        .iter()
        .any(|(_, s, _, site, _)| site.is_none() && s.condition < 0.4);
    let has_garden = building_query.iter().any(|(_, s, _, site, _)| {
        s.kind == crate::components::building::StructureType::Garden && site.is_none()
    });

    // Snapshot herb positions for proximity checks.
    let herb_positions: Vec<(Entity, Position)> = herb_query
        .iter()
        .map(|(e, _, p)| (e, *p))
        .collect();

    // Colony-wide ward assessment.
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

    // Pre-collect injured cat entities and count.
    let injured_cat_set: std::collections::HashSet<Entity> = query
        .iter()
        .filter(|(_, _, _, _, _, _, _, health, _, _, _, _, _, _, _)| {
            health.injuries.iter().any(|i| !i.healed)
        })
        .map(|(e, _, _, _, _, _, _, _, _, _, _, _, _, _, _)| e)
        .collect();
    let colony_injury_count = injured_cat_set.len();

    // Snapshot directive queue data before the main loop. We need mutable access
    // to the queue after the loop (to pop consumed directives), so we snapshot
    // the read-only data here to avoid borrow conflicts.
    let directive_snapshot: HashMap<Entity, (usize, Option<crate::components::coordination::Directive>)> =
        directive_queue_query
            .iter()
            .map(|(entity, q)| {
                (entity, (q.directives.len(), q.directives.first().cloned()))
            })
            .collect();

    // Pre-collect coordinator entities for directive targeting.
    let coordinator_entities: std::collections::HashSet<Entity> =
        directive_snapshot.keys().copied().collect();

    // Entities whose top directive should be popped after the main loop.
    let mut directives_to_pop: Vec<Entity> = Vec::new();

    // Snapshot current actions for activity cascading (including fight targets).
    let action_snapshot: Vec<(Entity, Position, Action, Option<Entity>)> = query
        .iter()
        .map(|(entity, _, _, _, pos, _, _, _, _, _, current, _, _, _, _)| (entity, *pos, current.action, current.target_entity))
        .collect();

    for (entity, _name, needs, personality, pos, memory, skills, health, magic_aff, inventory, mut current, aspirations, preferences, fated_love, fated_rival) in &mut query {
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

        // Find nearest threat within detection range (5 tiles).
        let nearest_threat = wildlife_positions
            .iter()
            .filter(|(_, wp)| pos.manhattan_distance(wp) <= 5)
            .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

        let has_threat_nearby = nearest_threat.is_some();

        // Count allies already fighting the same threat.
        let allies_fighting_threat = if let Some(&(threat_entity, _)) = nearest_threat {
            action_snapshot
                .iter()
                .filter(|(e, _, action, target)| {
                    *e != entity && *action == Action::Fight && *target == Some(threat_entity)
                })
                .count()
        } else {
            0
        };

        let combat_effective = skills.combat + skills.hunting * 0.3;
        let is_incapacitated = health.injuries.iter().any(|inj| {
            inj.kind == crate::components::physical::InjuryKind::Severe && !inj.healed
        });

        // Magic/herbcraft context.
        let has_herbs_nearby = herb_positions
            .iter()
            .any(|(_, hp)| pos.manhattan_distance(hp) <= 10);

        // Prey proximity check — is any prey within 10 tiles?
        let prey_nearby = prey_positions
            .iter()
            .any(|pp| pos.manhattan_distance(pp) <= 10);

        let (on_corrupted_tile, tile_corruption, on_special_terrain) = if map.in_bounds(pos.x, pos.y) {
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
            has_herbs_in_inventory: inventory.slots.iter().any(|s| matches!(s, crate::components::ItemSlot::Herb(_))),
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
            has_mentoring_target: has_mentoring_target(
                entity, pos, skills, &cat_positions, &skills_query,
            ),
            prey_nearby,
        };
        let mut scores = score_actions(&ctx, &mut rng.rng);

        // Memory-based adjustments.
        apply_memory_bonuses(&mut scores, memory, pos);

        // Colony knowledge: broader awareness from shared memories.
        if let Some(ref ck) = colony_knowledge {
            apply_colony_knowledge_bonuses(&mut scores, ck, pos);
        }

        // Player-set colony priority.
        if let Some(ref cp) = colony_priority {
            apply_priority_bonus(&mut scores, cp.active);
        }

        // Activity cascading: nearby cats doing the same action boost its score.
        let mut nearby_actions = HashMap::new();
        for &(other_entity, other_pos, other_action, _) in &action_snapshot {
            if other_entity != entity && pos.manhattan_distance(&other_pos) <= 5 {
                *nearby_actions.entry(other_action).or_insert(0usize) += 1;
            }
        }
        apply_cascading_bonuses(&mut scores, &nearby_actions);

        // Aspiration and preference bonuses.
        if let Some(asp) = aspirations {
            apply_aspiration_bonuses(&mut scores, asp);
        }
        if let Some(pref) = preferences {
            apply_preference_bonuses(&mut scores, pref);
        }

        // Fated connection desire bonuses.
        let love_visible = fated_love
            .filter(|l| l.awakened)
            .and_then(|l| cat_positions.iter().find(|(e, _)| *e == l.partner))
            .is_some_and(|(_, pp)| pos.manhattan_distance(pp) <= 15);
        let rival_nearby = fated_rival
            .filter(|r| r.awakened)
            .and_then(|r| cat_positions.iter().find(|(e, _)| *e == r.rival))
            .is_some_and(|(_, rp)| pos.manhattan_distance(rp) <= 15);
        apply_fated_bonuses(&mut scores, love_visible, rival_nearby);

        // Directive compliance: if this cat has an active directive from a
        // coordinator, boost the directed action's score.
        if let Ok(directive) = active_directive_query.get(entity) {
            let fondness_factor = relationships
                .get(entity, directive.coordinator)
                .map_or(0.5, |r| (r.fondness + 1.0) / 2.0);
            let bonus = directive.priority
                * directive.coordinator_social_weight
                * 0.5
                * personality.diligence
                * fondness_factor
                * (1.0 - personality.independence * 0.3);
            apply_directive_bonus(&mut scores, directive.kind.to_action(), bonus);
        }

        // Enforce Maslow survival floor: compress bonus-inflated non-survival
        // scores when physiological needs are critical.
        enforce_survival_floor(&mut scores, needs);

        let chosen = select_action_softmax(&scores, &mut rng.rng);

        // Store top-3 scores for diagnostic snapshot.
        {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            sorted.truncate(3);
            current.last_scores = sorted;
        }

        match chosen {
            Action::Eat => {
                // Walk to nearest Stores building and eat there.
                let nearest_store = building_query
                    .iter()
                    .filter(|(_, s, _, _, _)| {
                        s.kind == crate::components::building::StructureType::Stores
                    })
                    .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
                    .map(|(e, _, bp, _, _)| (e, *bp));

                if let Some((store_entity, store_pos)) = nearest_store {
                    current.action = Action::Eat;
                    current.ticks_remaining = 10; // enough to walk + eat
                    current.target_position = Some(store_pos);
                    current.target_entity = Some(store_entity);
                } else {
                    // No Stores — fall back to idle.
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Sleep => {
                current.action = Action::Sleep;
                current.ticks_remaining = 20;
                current.target_position = None;
                current.target_entity = None;
            }
            Action::Hunt => {
                let target = pick_hunt_target(pos, &map, memory, &mut rng.rng);
                current.action = Action::Hunt;
                current.ticks_remaining = 10;
                current.target_position = target;
                current.target_entity = None;
            }
            Action::Forage => {
                let target = find_nearest_tile(pos, &map, 10, |t| t.foraging_yield() > 0.0);
                current.action = Action::Forage;
                current.ticks_remaining = 10;
                current.target_position = target;
                current.target_entity = None;
            }
            Action::Socialize => {
                let target = pick_social_target(
                    entity, pos, &cat_positions, &relationships, 0.6, 0.4,
                );
                if let Some((target_entity, target_pos)) = target {
                    current.action = Action::Socialize;
                    current.ticks_remaining = 10;
                    current.target_position = Some(target_pos);
                    current.target_entity = Some(target_entity);
                } else {
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Groom => {
                // Determine whether self-groom or groom-other won.
                let self_score = (1.0 - needs.warmth) * 0.8 * needs.level_suppression(1);
                let other_score = if has_social_target {
                    personality.warmth * (1.0 - needs.social) * needs.level_suppression(3)
                } else {
                    0.0
                };

                if other_score > self_score {
                    // Groom-other: weighted more by fondness.
                    let target = pick_social_target(
                        entity, pos, &cat_positions, &relationships, 0.8, 0.2,
                    );
                    if let Some((target_entity, target_pos)) = target {
                        current.action = Action::Groom;
                        current.ticks_remaining = 8;
                        current.target_position = Some(target_pos);
                        current.target_entity = Some(target_entity);
                    } else {
                        // Fallback to self-groom.
                        current.action = Action::Groom;
                        current.ticks_remaining = 8;
                        current.target_position = None;
                        current.target_entity = None;
                    }
                } else {
                    // Self-groom.
                    current.action = Action::Groom;
                    current.ticks_remaining = 8;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Explore => {
                // Target a distant random position. Colony center approximated
                // from current position — the ColonyCenter resource is added in
                // Step 5. For now, wander far from current position.
                let dx: i32 = rng.rng.random_range(-20i32..=20);
                let dy: i32 = rng.rng.random_range(-20i32..=20);
                let mut target = Position::new(pos.x + dx, pos.y + dy);
                // Clamp to map bounds.
                target.x = target.x.clamp(0, map.width - 1);
                target.y = target.y.clamp(0, map.height - 1);
                current.action = Action::Explore;
                current.ticks_remaining = 20;
                current.target_position = Some(target);
                current.target_entity = None;
            }
            Action::Wander => {
                let dx: i32 = rng.rng.random_range(-5i32..=5);
                let dy: i32 = rng.rng.random_range(-5i32..=5);
                let mut target = Position::new(pos.x + dx, pos.y + dy);
                // Clamp to map bounds (same as Explore).
                target.x = target.x.clamp(0, map.width - 1);
                target.y = target.y.clamp(0, map.height - 1);
                current.action = Action::Wander;
                current.ticks_remaining = 10;
                current.target_position = Some(target);
                current.target_entity = None;
            }
            Action::Idle => {
                current.action = Action::Idle;
                current.ticks_remaining = 5;
                current.target_position = None;
                current.target_entity = None;
            }
            Action::Flee => {
                // Flee away from nearest threat.
                let flee_target = if let Some(&(_, threat_pos)) = nearest_threat {
                    // Move in the opposite direction from the threat.
                    let dx = pos.x - threat_pos.x;
                    let dy = pos.y - threat_pos.y;
                    // Normalize to ~8 tiles away.
                    let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                    let mut target = Position::new(
                        pos.x + (dx as f32 / len * 8.0) as i32,
                        pos.y + (dy as f32 / len * 8.0) as i32,
                    );
                    target.x = target.x.clamp(0, map.width - 1);
                    target.y = target.y.clamp(0, map.height - 1);
                    target
                } else {
                    // No specific threat — flee toward map center (safety).
                    Position::new(map.width / 2, map.height / 2)
                };
                current.action = Action::Flee;
                current.ticks_remaining = 15;
                current.target_position = Some(flee_target);
                current.target_entity = None;
            }
            Action::Fight => {
                // Fight: target the nearest wildlife threat.
                if let Some(&(threat_entity, threat_pos)) = nearest_threat {
                    current.action = Action::Fight;
                    current.ticks_remaining = 30;
                    current.target_position = Some(threat_pos);
                    current.target_entity = Some(threat_entity);
                } else {
                    // No threat found — fall back to idle.
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Patrol => {
                // Patrol colony perimeter (~10 tiles from map center).
                let center_x = map.width / 2;
                let center_y = map.height / 2;
                let angle: f32 = rng.rng.random_range(0.0..std::f32::consts::TAU);
                let radius = 10.0_f32;
                let mut target = Position::new(
                    center_x + (angle.cos() * radius) as i32,
                    center_y + (angle.sin() * radius) as i32,
                );
                target.x = target.x.clamp(0, map.width - 1);
                target.y = target.y.clamp(0, map.height - 1);
                current.action = Action::Patrol;
                current.ticks_remaining = 20;
                current.target_position = Some(target);
                current.target_entity = None;
            }
            Action::Build => {
                use crate::components::building::StructureType;
                use crate::components::task_chain::{TaskChain, TaskStep, StepKind, FailurePolicy};

                // Find a build target: prefer construction sites, then damaged buildings.
                let target = building_query
                    .iter()
                    .filter(|(_, _, bpos, site, _)| {
                        site.is_some() || pos.manhattan_distance(bpos) <= 30
                    })
                    .min_by_key(|(_, _s, bpos, site, _)| {
                        // Prefer construction sites, then lowest condition.
                        let priority = if site.is_some() { 0 } else { 1 };
                        let dist = pos.manhattan_distance(bpos);
                        (priority, dist)
                    });

                if let Some((target_entity, _structure, bpos, site, _)) = target {
                    let chain = if site.is_some() {
                        // Construction: build a simple construct chain (materials
                        // already expected to be delivered by other cats or as
                        // a simplification for now).
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
                        // Repair damaged building.
                        StructureType::repair_chain(*bpos, target_entity)
                    };

                    current.action = Action::Build;
                    current.ticks_remaining = u64::MAX;
                    current.target_position = Some(*bpos);
                    current.target_entity = Some(target_entity);
                    commands.entity(entity).insert(chain);
                } else {
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Farm => {
                use crate::components::building::StructureType;

                let garden = building_query
                    .iter()
                    .filter(|(_, s, _, site, _)| {
                        s.kind == StructureType::Garden && site.is_none()
                    })
                    .min_by_key(|(_, _, bpos, _, _)| pos.manhattan_distance(bpos));

                if let Some((garden_entity, _, garden_pos, _, _)) = garden {
                    let chain = StructureType::farm_chain(*garden_pos, garden_entity);
                    current.action = Action::Farm;
                    current.ticks_remaining = u64::MAX;
                    current.target_position = Some(*garden_pos);
                    current.target_entity = Some(garden_entity);
                    commands.entity(entity).insert(chain);
                } else {
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Herbcraft => {
                use crate::components::magic::{RemedyKind, WardKind};
                use crate::components::task_chain::{TaskChain, TaskStep, StepKind, FailurePolicy};

                // Re-derive which sub-mode won scoring.
                let gather_score = if has_herbs_nearby {
                    personality.spirituality * 0.5 * (0.1 + skills.herbcraft)
                        * needs.level_suppression(3)
                } else {
                    0.0
                };
                let prepare_score = if inventory.has_remedy_herb() && colony_injury_count > 0 {
                    personality.compassion * (0.1 + skills.herbcraft)
                        * (colony_injury_count as f32 * 0.3).min(1.5)
                        * needs.level_suppression(3)
                } else {
                    0.0
                };
                let ward_score = if inventory.has_ward_herb() && ward_strength_low {
                    personality.spirituality * (0.1 + skills.herbcraft) * 0.6
                        * needs.level_suppression(4)
                } else {
                    0.0
                };

                if gather_score >= prepare_score && gather_score >= ward_score {
                    // Gather: find nearest harvestable herb.
                    if let Some(&(herb_entity, herb_pos)) = herb_positions
                        .iter()
                        .filter(|(_, hp)| pos.manhattan_distance(hp) <= 15)
                        .min_by_key(|(_, hp)| pos.manhattan_distance(hp))
                    {
                        let chain = TaskChain::new(
                            vec![
                                TaskStep::new(StepKind::MoveTo).with_position(herb_pos),
                                TaskStep::new(StepKind::GatherHerb)
                                    .with_position(herb_pos)
                                    .with_entity(herb_entity),
                            ],
                            FailurePolicy::AbortChain,
                        );
                        current.action = Action::Herbcraft;
                        current.ticks_remaining = u64::MAX;
                        current.target_position = Some(herb_pos);
                        current.target_entity = Some(herb_entity);
                        commands.entity(entity).insert(chain);
                    } else {
                        current.action = Action::Idle;
                        current.ticks_remaining = 5;
                        current.target_position = None;
                        current.target_entity = None;
                    }
                } else if prepare_score >= ward_score {
                    // Prepare remedy: find injured cat, optionally route via workshop.
                    let remedy_kind = if inventory.has_herb(crate::components::magic::HerbKind::HealingMoss) {
                        RemedyKind::HealingPoultice
                    } else if inventory.has_herb(crate::components::magic::HerbKind::Moonpetal) {
                        RemedyKind::EnergyTonic
                    } else {
                        RemedyKind::MoodTonic
                    };

                    // Find nearest injured cat.
                    let injured_target = cat_positions
                        .iter()
                        .filter(|(e, _)| *e != entity)
                        .filter(|(e, _)| injured_cat_set.contains(e))
                        .min_by_key(|(_, p)| pos.manhattan_distance(p))
                        .map(|(e, p)| (*e, *p));

                    if let Some((patient_entity, patient_pos)) = injured_target {
                        // Find workshop for bonus speed.
                        let workshop_pos = building_query
                            .iter()
                            .filter(|(_, s, _, site, _)| {
                                s.kind == crate::components::building::StructureType::Workshop && site.is_none()
                            })
                            .map(|(_, _, bpos, _, _)| *bpos)
                            .min_by_key(|bpos| pos.manhattan_distance(bpos));

                        let mut steps = Vec::new();
                        if let Some(wp) = workshop_pos {
                            steps.push(TaskStep::new(StepKind::MoveTo).with_position(wp));
                        }
                        steps.push(TaskStep::new(StepKind::PrepareRemedy { remedy: remedy_kind }));
                        steps.push(TaskStep::new(StepKind::MoveTo).with_position(patient_pos));
                        steps.push(
                            TaskStep::new(StepKind::ApplyRemedy { remedy: remedy_kind })
                                .with_position(patient_pos)
                                .with_entity(patient_entity),
                        );

                        let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
                        current.action = Action::Herbcraft;
                        current.ticks_remaining = u64::MAX;
                        current.target_position = Some(patient_pos);
                        current.target_entity = Some(patient_entity);
                        commands.entity(entity).insert(chain);
                    } else {
                        current.action = Action::Idle;
                        current.ticks_remaining = 5;
                        current.target_position = None;
                        current.target_entity = None;
                    }
                } else {
                    // Set ward: place at colony perimeter.
                    let center_x = map.width / 2;
                    let center_y = map.height / 2;
                    let angle: f32 = rng.rng.random_range(0.0..std::f32::consts::TAU);
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
                            TaskStep::new(StepKind::SetWard { kind: WardKind::Thornward })
                                .with_position(ward_pos),
                        ],
                        FailurePolicy::AbortChain,
                    );
                    current.action = Action::Herbcraft;
                    current.ticks_remaining = u64::MAX;
                    current.target_position = Some(ward_pos);
                    current.target_entity = None;
                    commands.entity(entity).insert(chain);
                }
            }
            Action::PracticeMagic => {
                use crate::components::magic::WardKind;
                use crate::components::task_chain::{TaskChain, TaskStep, StepKind, FailurePolicy};

                // Re-derive which sub-mode won scoring.
                let scry_score = personality.curiosity * personality.spirituality
                    * skills.magic * needs.level_suppression(5);
                let durable_ward_score = if ward_strength_low && skills.magic > 0.5 {
                    personality.spirituality * skills.magic * 0.8
                        * needs.level_suppression(4)
                } else {
                    0.0
                };
                let cleanse_score = if on_corrupted_tile && tile_corruption > 0.1 {
                    personality.spirituality * skills.magic * tile_corruption
                        * needs.level_suppression(4)
                } else {
                    0.0
                };
                let commune_score = if on_special_terrain {
                    personality.spirituality * skills.magic * 0.7
                        * needs.level_suppression(5)
                } else {
                    0.0
                };

                let max_score = scry_score.max(durable_ward_score).max(cleanse_score).max(commune_score);

                if max_score == scry_score && scry_score > 0.0 {
                    // Scry at current position.
                    let chain = TaskChain::new(
                        vec![TaskStep::new(StepKind::Scry).with_position(*pos)],
                        FailurePolicy::AbortChain,
                    );
                    current.action = Action::PracticeMagic;
                    current.ticks_remaining = u64::MAX;
                    current.target_position = Some(*pos);
                    current.target_entity = None;
                    commands.entity(entity).insert(chain);
                } else if max_score == durable_ward_score {
                    // Place durable ward at perimeter.
                    let center_x = map.width / 2;
                    let center_y = map.height / 2;
                    let angle: f32 = rng.rng.random_range(0.0..std::f32::consts::TAU);
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
                            TaskStep::new(StepKind::SetWard { kind: WardKind::DurableWard })
                                .with_position(ward_pos),
                        ],
                        FailurePolicy::AbortChain,
                    );
                    current.action = Action::PracticeMagic;
                    current.ticks_remaining = u64::MAX;
                    current.target_position = Some(ward_pos);
                    current.target_entity = None;
                    commands.entity(entity).insert(chain);
                } else if max_score == cleanse_score {
                    // Cleanse corruption at current tile.
                    let chain = TaskChain::new(
                        vec![TaskStep::new(StepKind::CleanseCorruption).with_position(*pos)],
                        FailurePolicy::AbortChain,
                    );
                    current.action = Action::PracticeMagic;
                    current.ticks_remaining = u64::MAX;
                    current.target_position = Some(*pos);
                    current.target_entity = None;
                    commands.entity(entity).insert(chain);
                } else if commune_score > 0.0 {
                    // Spirit communion: move to nearest special terrain.
                    let special_pos = find_nearest_tile(pos, &map, 20, |t| {
                        matches!(t, Terrain::FairyRing | Terrain::StandingStone)
                    });
                    if let Some(sp) = special_pos {
                        let chain = TaskChain::new(
                            vec![
                                TaskStep::new(StepKind::MoveTo).with_position(sp),
                                TaskStep::new(StepKind::SpiritCommunion).with_position(sp),
                            ],
                            FailurePolicy::AbortChain,
                        );
                        current.action = Action::PracticeMagic;
                        current.ticks_remaining = u64::MAX;
                        current.target_position = Some(sp);
                        current.target_entity = None;
                        commands.entity(entity).insert(chain);
                    } else {
                        current.action = Action::Idle;
                        current.ticks_remaining = 5;
                        current.target_position = None;
                        current.target_entity = None;
                    }
                } else {
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Coordinate => {
                // Use snapshotted directive data; the actual pop happens after the loop.
                let directive = directive_snapshot
                    .get(&entity)
                    .and_then(|(_, d)| d.clone());
                if let Some(directive) = directive {
                    let target = pick_directive_target(
                        entity,
                        pos,
                        &directive,
                        &cat_positions,
                        &coordinator_entities,
                        &active_directive_query,
                        &skills_query,
                    );
                    if let Some((target_entity, target_pos)) = target {
                        current.action = Action::Coordinate;
                        let dist = pos.manhattan_distance(&target_pos) as u64;
                        current.ticks_remaining = dist + 2;
                        current.target_position = Some(target_pos);
                        current.target_entity = Some(target_entity);
                        commands.entity(entity).insert(PendingDelivery(directive));
                        directives_to_pop.push(entity);
                    } else {
                        current.action = Action::Idle;
                        current.ticks_remaining = 5;
                        current.target_position = None;
                        current.target_entity = None;
                    }
                } else {
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
            Action::Mentor => {
                if let Some((apprentice_entity, apprentice_pos)) = pick_mentoring_target(
                    entity, pos, skills, &cat_positions, &skills_query,
                ) {
                    current.action = Action::Mentor;
                    current.ticks_remaining = 30;
                    current.target_position = Some(apprentice_pos);
                    current.target_entity = Some(apprentice_entity);
                } else {
                    current.action = Action::Idle;
                    current.ticks_remaining = 5;
                    current.target_position = None;
                    current.target_entity = None;
                }
            }
        }
    }

    // Pop consumed directives from coordinator queues.
    for entity in directives_to_pop {
        if let Ok((_, mut queue)) = directive_queue_query.get_mut(entity) {
            if !queue.directives.is_empty() {
                queue.directives.remove(0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    use crate::components::mental::Memory;
    use crate::components::personality::Personality;
    use crate::components::physical::Health;
    use crate::components::skills::Skills;
    use crate::resources::map::{Terrain, TileMap};

    fn default_personality() -> Personality {
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

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        world.insert_resource(FoodStores::new(10.0, 30.0, 0.002));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(Relationships::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(evaluate_actions);
        (world, schedule)
    }

    fn spawn_cat(world: &mut World, needs: Needs, personality: Personality, pos: Position) -> Entity {
        world
            .spawn((
                crate::components::identity::Name("TestCat".to_string()),
                needs,
                personality,
                pos,
                Memory::default(),
                CurrentAction::default(),
                Skills::default(),
                Health::default(),
                MagicAffinity(0.0),
                Inventory::default(),
            ))
            .id()
    }

    /// A cat with ticks_remaining == 0 should get a new action assigned.
    #[test]
    fn assigns_action_when_idle() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality(), Position::new(10, 10));

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert!(
            ca.ticks_remaining > 0,
            "should have assigned a new action with ticks > 0"
        );
    }

    /// A cat mid-action (ticks_remaining > 0) should not have its action replaced.
    #[test]
    fn does_not_replace_active_action() {
        let (mut world, mut schedule) = setup_world();

        let entity = world
            .spawn((
                crate::components::identity::Name("TestCat".to_string()),
                Needs::default(),
                default_personality(),
                Position::new(10, 10),
                Memory::default(),
                CurrentAction {
                    action: Action::Sleep,
                    ticks_remaining: 15,
                    target_position: None,
                    target_entity: None,
                last_scores: Vec::new(),
                },
                Skills::default(),
                Health::default(),
                MagicAffinity(0.0),
                Inventory::default(),
            ))
            .id();

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert_eq!(ca.action, Action::Sleep, "active Sleep should not be replaced");
        assert_eq!(ca.ticks_remaining, 15, "ticks_remaining should be unchanged");
    }

    /// A starving cat (hunger=0.05) with food in stores should be assigned Eat.
    #[test]
    fn starving_cat_chooses_eat() {
        let (mut world, mut schedule) = setup_world();

        // Spawn a Stores building so evaluate_actions can target it.
        world.spawn((
            crate::components::building::Structure::new(
                crate::components::building::StructureType::Stores,
            ),
            crate::components::building::StoredItems::default(),
            Position::new(5, 5),
        ));

        let mut needs = Needs::default();
        needs.hunger = 0.05;
        needs.energy = 0.9;

        let entity = spawn_cat(&mut world, needs, default_personality(), Position::new(5, 5));

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert_eq!(ca.action, Action::Eat, "starving cat should choose Eat");
        assert_eq!(ca.ticks_remaining, 10);
    }

    /// When food stores are empty, cat should not choose Eat.
    #[test]
    fn empty_stores_prevents_eat() {
        let (mut world, mut schedule) = setup_world();
        world.insert_resource(FoodStores::new(0.0, 50.0, 0.002));

        let mut needs = Needs::default();
        needs.hunger = 0.05;
        needs.energy = 0.9;

        let entity = spawn_cat(&mut world, needs, default_personality(), Position::new(5, 5));

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert_ne!(ca.action, Action::Eat, "no food in stores, should not Eat");
    }
}
