use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::coordination::ActiveDirective;
use crate::components::mental::{Memory, MemoryType};
use crate::components::physical::{Dead, Position};
use crate::components::skills::Skills;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::food::FoodStores;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::relationships::Relationships;

// ---------------------------------------------------------------------------
// Terrain helpers
// ---------------------------------------------------------------------------

/// Find the nearest tile matching a predicate within a search radius.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
        .filter(|(other, other_pos)| *other != entity && pos.manhattan_distance(other_pos) <= 10)
        .max_by(|(e_a, _), (e_b, _)| {
            let score_a = relationships.get(entity, *e_a).map_or(0.0, |r| {
                r.fondness * fondness_weight + (1.0 - r.familiarity) * novelty_weight
            });
            let score_b = relationships.get(entity, *e_b).map_or(0.0, |r| {
                r.fondness * fondness_weight + (1.0 - r.familiarity) * novelty_weight
            });
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p))
}

/// Check whether a valid mentoring target exists for this cat.
///
/// A target is valid when the mentor has any skill > 0.6 and the candidate has
/// the same skill < 0.3, within 10 tiles.
#[allow(dead_code)]
fn has_mentoring_target(
    entity: Entity,
    pos: &Position,
    skills: &Skills,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
) -> bool {
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
}

/// Pick the best mentoring target: nearby cat with the largest skill gap
/// where the mentor has skill > 0.6 and the apprentice has skill < 0.3.
#[allow(dead_code)]
fn pick_mentoring_target(
    entity: Entity,
    pos: &Position,
    skills: &Skills,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
) -> Option<(Entity, Position)> {
    let mentor_skills = [
        skills.hunting,
        skills.foraging,
        skills.herbcraft,
        skills.building,
        skills.combat,
        skills.magic,
    ];

    cat_positions
        .iter()
        .filter(|(other, other_pos)| *other != entity && pos.manhattan_distance(other_pos) <= 10)
        .filter_map(|(other, other_pos)| {
            let other_skills = skills_query.get(*other).ok()?;
            let other_arr = [
                other_skills.hunting,
                other_skills.foraging,
                other_skills.herbcraft,
                other_skills.building,
                other_skills.combat,
                other_skills.magic,
            ];
            // Find the maximum teachable skill gap.
            let max_gap = mentor_skills
                .iter()
                .zip(other_arr.iter())
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
            gap_a
                .partial_cmp(gap_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p, _)| (e, p))
}

/// Pick the best target cat for a coordination directive.
///
/// Prefers: non-coordinator, not already directed, nearby, high relevant skill.
#[allow(dead_code)]
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
            let skill_a = skills_query
                .get(*e_a)
                .map_or(0.0, |s| match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                });
            let skill_b = skills_query
                .get(*e_b)
                .map_or(0.0, |s| match directive.kind {
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

/// Emit periodic food-level and population snapshots to the event log.
pub fn emit_periodic_events(
    config: Res<crate::resources::snapshot_config::SnapshotConfig>,
    time: Res<crate::resources::time::TimeState>,
    food: Res<FoodStores>,
    prey: Query<&crate::components::prey::PreyConfig, With<crate::components::prey::PreyAnimal>>,
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
        }
    }
}

// Legacy evaluate_actions removed — all action selection now flows through
// evaluate_dispositions in src/systems/disposition.rs.
