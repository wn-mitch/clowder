use bevy_ecs::prelude::*;

use crate::components::building::StructureType;
use crate::components::coordination::{
    ActiveDirective, BuildPressure, Coordinator, CoordinatorDied, Directive, DirectiveKind,
    DirectiveQueue, PendingDelivery,
};
use crate::components::identity::Name;
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Position};
use crate::components::skills::Skills;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// Social weight (pure function, not a system)
// ---------------------------------------------------------------------------

/// Compute social weight for a cat based on relationships and memorable deeds.
///
/// Formula: `sum(positive fondness) + avg(familiarity) * 0.5 + significant_events * 0.1`
///
/// Social weight is derived, not stored — computed when needed for coordinator
/// evaluation, directive compliance bonuses, and narrative.
pub fn social_weight(
    entity: Entity,
    relationships: &Relationships,
    memory: &Memory,
    constants: &crate::resources::sim_constants::CoordinationConstants,
) -> f32 {
    let rels = relationships.all_for(entity);
    let positive_fondness_sum: f32 = rels.iter().map(|(_, r)| r.fondness.max(0.0)).sum();
    let avg_familiarity: f32 = if rels.is_empty() {
        0.0
    } else {
        rels.iter().map(|(_, r)| r.familiarity).sum::<f32>() / rels.len() as f32
    };
    let significant_events = memory
        .events
        .iter()
        .filter(|e| matches!(e.event_type, MemoryType::SocialEvent | MemoryType::Death))
        .count();
    positive_fondness_sum
        + avg_familiarity * constants.social_weight_familiarity_scale
        + significant_events as f32 * constants.social_weight_event_scale
}

// ---------------------------------------------------------------------------
// evaluate_coordinators
// ---------------------------------------------------------------------------

/// Identify the top 1–2 cats as coordinators based on social weight, diligence,
/// and sociability. Runs every 100 ticks or immediately when a coordinator dies.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_coordinators(
    mut commands: Commands,
    time: Res<TimeState>,
    coordinator_died: Option<Res<CoordinatorDied>>,
    query: Query<(Entity, &Personality, &Memory, &Name), Without<Dead>>,
    existing_coordinators: Query<Entity, With<Coordinator>>,
    relationships: Res<Relationships>,
    mut log: ResMut<NarrativeLog>,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.coordination;
    let should_run = coordinator_died.is_some()
        || (time.tick > 0 && time.tick.is_multiple_of(c.evaluate_interval));
    if !should_run {
        return;
    }

    let living_count = query.iter().count();
    let max_coordinators: usize = if living_count < c.small_colony_threshold {
        1
    } else {
        2
    };
    let threshold = c.promotion_threshold;

    // Score all living cats.
    let mut candidates: Vec<(Entity, f32, String)> = query
        .iter()
        .map(|(entity, personality, memory, name)| {
            let sw = social_weight(entity, &relationships, memory, c);
            let score = sw
                * personality.diligence
                * personality.sociability
                * (1.0 + personality.ambition * c.ambition_bonus);
            (entity, score, name.0.clone())
        })
        .filter(|(_, score, _)| *score >= threshold)
        .collect();

    // Sort by score descending, tiebreak by entity index for determinism.
    candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.index().cmp(&b.0.index()))
    });
    candidates.truncate(max_coordinators);

    let new_set: Vec<Entity> = candidates.iter().map(|(e, _, _)| *e).collect();

    // Remove Coordinator from cats who lost the role.
    for entity in &existing_coordinators {
        if !new_set.contains(&entity) {
            commands.entity(entity).remove::<Coordinator>();
            commands.entity(entity).remove::<DirectiveQueue>();
            commands.entity(entity).remove::<BuildPressure>();
            commands.entity(entity).remove::<PendingDelivery>();
        }
    }

    // Add Coordinator + DirectiveQueue to new coordinators.
    let mut event_log = event_log;
    let mut new_coordinator_names: Vec<&str> = Vec::new();
    for (entity, score, name) in &candidates {
        if existing_coordinators.get(*entity).is_err() {
            commands.entity(*entity).insert((
                Coordinator,
                DirectiveQueue::default(),
                BuildPressure::default(),
            ));
            new_coordinator_names.push(name.as_str());
            if let Some(ref mut elog) = event_log {
                elog.push(
                    time.tick,
                    crate::resources::event_log::EventKind::CoordinatorElected {
                        cat: name.clone(),
                        social_weight: *score,
                    },
                );
            }
        }
    }

    // Emit a single combined narrative line for all new coordinators.
    if !new_coordinator_names.is_empty() {
        activation.record(Feature::CoordinatorElected);
        let names = match new_coordinator_names.len() {
            1 => new_coordinator_names[0].to_string(),
            2 => format!(
                "{} and {}",
                new_coordinator_names[0], new_coordinator_names[1]
            ),
            _ => {
                let (last, rest) = new_coordinator_names.split_last().unwrap();
                format!("{}, and {last}", rest.join(", "))
            }
        };
        log.push(
            time.tick,
            format!("The others look to {names} when decisions need making."),
            NarrativeTier::Significant,
        );
    }

    // Clear the flag if it was set.
    if coordinator_died.is_some() {
        commands.remove_resource::<CoordinatorDied>();
    }
}

// ---------------------------------------------------------------------------
// assess_colony_needs
// ---------------------------------------------------------------------------

/// For each coordinator, evaluate colony state and fill their directive queue.
/// Runs every 20 ticks. The coordinator's own skills shift assessment thresholds
/// (domain specialization).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn assess_colony_needs(
    time: Res<TimeState>,
    food: Res<crate::resources::food::FoodStores>,
    mut coordinators: Query<(Entity, &Name, &Skills, &mut DirectiveQueue), With<Coordinator>>,
    injured_cats: Query<(Entity, &crate::components::physical::Health, &Position), Without<Dead>>,
    building_query: Query<(
        Entity,
        &crate::components::building::Structure,
        &Position,
        Option<&crate::components::building::ConstructionSite>,
    )>,
    ward_query: Query<&crate::components::magic::Ward>,
    wildlife: Query<(Entity, &Position), With<crate::components::wildlife::WildAnimal>>,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let cc = &constants.coordination;
    if !time.tick.is_multiple_of(cc.assess_interval) {
        return;
    }

    // Pre-compute colony state once.
    let food_fraction = food.fraction();
    let colony_injury_count = injured_cats
        .iter()
        .filter(|(_, h, _)| h.injuries.iter().any(|i| !i.healed))
        .count();

    // Collect building positions for proximity checks.
    let building_positions: Vec<Position> = building_query
        .iter()
        .filter(|(_, _, _, site)| site.is_none())
        .map(|(_, _, bpos, _)| *bpos)
        .collect();

    // Count threats near colony buildings (not the entire map).
    let nearby_threats: Vec<(Entity, Position)> = wildlife
        .iter()
        .filter(|(_, wp)| {
            building_positions
                .iter()
                .any(|bp| bp.manhattan_distance(wp) <= cc.threat_proximity_range)
        })
        .map(|(e, p)| (e, *p))
        .collect();

    // Breach = wildlife very close to a building.
    let breach_threats: Vec<(Entity, Position)> = nearby_threats
        .iter()
        .filter(|(_, wp)| {
            building_positions
                .iter()
                .any(|bp| bp.manhattan_distance(wp) <= cc.colony_breach_range)
        })
        .cloned()
        .collect();

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 = ward_query.iter().map(|w| w.strength).sum::<f32>() / ward_count as f32;
            avg < cc.ward_avg_strength_low_threshold
        }
    };

    let mut event_log = event_log;

    for (_entity, name, skills, mut queue) in &mut coordinators {
        queue.directives.clear();

        // Domain specialization: coordinator's skills shift thresholds.
        let food_threshold = cc.food_threshold_base
            - skills.hunting * cc.food_threshold_hunting_scale
            - skills.foraging * cc.food_threshold_foraging_scale;
        let building_threshold =
            cc.building_threshold_base - skills.building * cc.building_threshold_building_scale;

        // Food assessment.
        if food_fraction < food_threshold {
            let priority = (1.0 - food_fraction).min(1.0);
            queue.directives.push(Directive {
                kind: DirectiveKind::Hunt,
                priority,
                target_entity: None,
                target_position: None,
                blueprint: None,
            });
            // Also queue forage if food is critically low.
            if food_fraction < food_threshold * 0.5 {
                queue.directives.push(Directive {
                    kind: DirectiveKind::Forage,
                    priority: priority * cc.forage_critical_multiplier,
                    target_entity: None,
                    target_position: None,
                    blueprint: None,
                });
            }
        }

        // Threat assessment — only react to wildlife near colony.
        if !breach_threats.is_empty() {
            // Wildlife has breached colony perimeter — issue Fight.
            queue.directives.push(Directive {
                kind: DirectiveKind::Fight,
                priority: cc.threat_fight_priority,
                target_entity: breach_threats.first().map(|(e, _)| *e),
                target_position: breach_threats.first().map(|(_, p)| *p),
                blueprint: None,
            });
        }
        if !nearby_threats.is_empty() {
            // Wildlife detected near colony — issue targeted Patrol toward it.
            let closest_threat = nearby_threats.iter().min_by_key(|(_, wp)| {
                building_positions
                    .iter()
                    .map(|bp| bp.manhattan_distance(wp))
                    .min()
                    .unwrap_or(i32::MAX)
            });
            queue.directives.push(Directive {
                kind: DirectiveKind::Patrol,
                priority: cc.threat_patrol_targeted_priority,
                target_entity: None,
                target_position: closest_threat.map(|(_, p)| *p),
                blueprint: None,
            });
        }

        // Building assessment.
        let worst_building = building_query
            .iter()
            .filter(|(_, s, _, site)| site.is_none() && s.condition < building_threshold)
            .min_by(|(_, a, _, _), (_, b, _, _)| {
                a.condition
                    .partial_cmp(&b.condition)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some((build_entity, structure, build_pos, _)) = worst_building {
            let priority = (cc.build_repair_priority_base
                + skills.building * cc.build_repair_priority_building_scale)
                .min(1.0);
            queue.directives.push(Directive {
                kind: DirectiveKind::Build,
                priority,
                target_entity: Some(build_entity),
                target_position: Some(*build_pos),
                blueprint: None,
            });
            let _ = structure; // used indirectly via condition filter
        }

        // Injury assessment.
        if colony_injury_count > 0 {
            let priority = (colony_injury_count as f32 * cc.injury_priority_per_cat).min(1.0);
            queue.directives.push(Directive {
                kind: DirectiveKind::Herbcraft,
                priority,
                target_entity: None,
                target_position: None,
                blueprint: None,
            });
        }

        // Ward assessment.
        if ward_strength_low {
            queue.directives.push(Directive {
                kind: DirectiveKind::SetWard,
                priority: cc.ward_set_priority,
                target_entity: None,
                target_position: None,
                blueprint: None,
            });
        }

        // Sort by priority descending.
        queue.directives.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Emit directive events.
        if !queue.directives.is_empty() {
            activation.record(Feature::DirectiveIssued);
        }
        if let Some(ref mut elog) = event_log {
            for d in &queue.directives {
                elog.push(
                    time.tick,
                    crate::resources::event_log::EventKind::DirectiveIssued {
                        coordinator: name.0.clone(),
                        kind: format!("{:?}", d.kind),
                        priority: d.priority,
                    },
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// flag_coordinator_death
// ---------------------------------------------------------------------------

/// If any dead entity has the Coordinator marker, insert the CoordinatorDied
/// flag resource to trigger immediate re-evaluation.
pub fn flag_coordinator_death(
    mut commands: Commands,
    query: Query<(), (With<Dead>, With<Coordinator>)>,
) {
    if !query.is_empty() {
        commands.insert_resource(CoordinatorDied);
    }
}

// ---------------------------------------------------------------------------
// expire_directives
// ---------------------------------------------------------------------------

/// Remove `ActiveDirective` from cats whose coordinator is dead or whose
/// directive is older than 200 ticks. Also remove stale `PendingDelivery`
/// from coordinators who are no longer performing the Coordinate action.
pub fn expire_directives(
    mut commands: Commands,
    time: Res<TimeState>,
    active_query: Query<(Entity, &ActiveDirective)>,
    alive_check: Query<(), Without<Dead>>,
    stale_delivery_query: Query<(Entity, &PendingDelivery, &crate::ai::CurrentAction)>,
    constants: Res<SimConstants>,
) {
    let expiry_ticks = constants.coordination.directive_expiry_ticks;
    for (entity, directive) in &active_query {
        let coordinator_dead = alive_check.get(directive.coordinator).is_err();
        let expired = time.tick.saturating_sub(directive.delivered_tick) > expiry_ticks;
        if coordinator_dead || expired {
            commands.entity(entity).remove::<ActiveDirective>();
        }
    }

    // Clean up PendingDelivery on coordinators who switched away from Coordinate.
    for (entity, _, current) in &stale_delivery_query {
        if current.action != crate::ai::Action::Coordinate {
            commands.entity(entity).remove::<PendingDelivery>();
        }
    }
}

// ---------------------------------------------------------------------------
// accumulate_build_pressure
// ---------------------------------------------------------------------------

/// Evaluate colony infrastructure gaps and accumulate build pressure on each
/// coordinator. When pressure exceeds the coordinator's action threshold
/// (derived from attentiveness), issue a Build directive for new construction.
///
/// Runs on the same 20-tick cadence as `assess_colony_needs`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn accumulate_build_pressure(
    time: Res<TimeState>,
    food: Res<crate::resources::food::FoodStores>,
    mut coordinators: Query<
        (
            Entity,
            &Name,
            &Personality,
            &Skills,
            &mut BuildPressure,
            &mut DirectiveQueue,
        ),
        With<Coordinator>,
    >,
    cats: Query<(&Position, &crate::ai::CurrentAction), Without<Dead>>,
    buildings: Query<(&crate::components::building::Structure, &Position)>,
    stored_items_query: Query<(
        &crate::components::building::Structure,
        &crate::components::building::StoredItems,
    )>,
    wildlife: Query<&Position, With<crate::components::wildlife::WildAnimal>>,
    items_query: Query<&crate::components::items::Item>,
    mut log: ResMut<NarrativeLog>,
    constants: Res<SimConstants>,
) {
    let cc = &constants.coordination;
    if !time.tick.is_multiple_of(cc.assess_interval) {
        return;
    }

    // Pre-compute colony state.
    let has_structure =
        |kind: StructureType| -> bool { buildings.iter().any(|(s, _)| s.kind == kind) };

    let stores_full = stored_items_query.iter().any(|(s, items)| {
        s.kind == StructureType::Stores
            && items.is_effectively_full(StructureType::Stores, &items_query)
    });

    // Cats sleeping without a Den nearby.
    let unsheltered_sleepers = cats
        .iter()
        .filter(|(cat_pos, action)| {
            action.action == crate::ai::Action::Sleep
                && !buildings.iter().any(|(s, bpos)| {
                    s.kind == StructureType::Den && cat_pos.manhattan_distance(&s.center(bpos)) <= 4
                })
        })
        .count();

    let food_fraction = food.fraction();
    let has_garden = has_structure(StructureType::Garden);
    let has_workshop = has_structure(StructureType::Workshop);
    let has_watchtower = has_structure(StructureType::Watchtower);

    let skilled_crafters = cats.iter().count(); // simplified: count living cats as proxy
                                                // Wildlife inside colony area (within ~wildlife_breach_range tiles of any building).
    let wildlife_breach = wildlife.iter().any(|wpos| {
        buildings
            .iter()
            .any(|(s, bpos)| wpos.manhattan_distance(&s.center(bpos)) <= cc.wildlife_breach_range)
    });

    for (_entity, name, personality, skills, mut pressure, mut queue) in &mut coordinators {
        let attentiveness = personality.diligence * cc.attentiveness_diligence_weight
            + personality.ambition * cc.attentiveness_ambition_weight
            + (1.0 - personality.patience) * cc.attentiveness_impatience_weight;
        let rate = BuildPressure::BASE_RATE * attentiveness;
        let threshold = 1.0 - attentiveness * cc.build_pressure_attentiveness_threshold_scale;

        // Storage pressure.
        if stores_full {
            pressure.storage += rate;
        } else {
            pressure.storage *= BuildPressure::DECAY;
        }

        // Shelter pressure.
        if unsheltered_sleepers > 0 {
            pressure.shelter += rate * unsheltered_sleepers as f32;
        } else {
            pressure.shelter *= BuildPressure::DECAY;
        }

        // Gathering pressure — low social despite Hearth existing.
        // Simplified: if food is fine but we don't have social infrastructure.
        // Full implementation would check avg social need of cats near hearth.
        pressure.gathering *= BuildPressure::DECAY;

        // Workshop pressure.
        if !has_workshop && skilled_crafters >= cc.build_pressure_workshop_min_cats {
            pressure.workshop += rate;
        } else {
            pressure.workshop *= BuildPressure::DECAY;
        }

        // Farming pressure.
        if !has_garden && food_fraction < cc.build_pressure_farming_food_threshold {
            pressure.farming += rate;
        } else {
            pressure.farming *= BuildPressure::DECAY;
        }

        // Defense pressure.
        if wildlife_breach && !has_watchtower {
            pressure.defense += rate;
        } else {
            pressure.defense *= BuildPressure::DECAY;
        }

        // Check if any pressure exceeds the action threshold.
        if let Some(blueprint) = pressure.highest_actionable(threshold) {
            // Only issue if there isn't already a Build directive with a blueprint
            // in the queue (avoid spamming).
            let already_queued = queue
                .directives
                .iter()
                .any(|d| d.kind == DirectiveKind::Build && d.blueprint.is_some());
            if !already_queued {
                let priority = (cc.build_directive_priority_base
                    + skills.building * cc.build_directive_priority_building_scale)
                    .min(1.0);
                queue.directives.push(Directive {
                    kind: DirectiveKind::Build,
                    priority,
                    target_entity: None,
                    target_position: None,
                    blueprint: Some(blueprint),
                });

                log.push(
                    time.tick,
                    format!(
                        "{} decides the colony needs a new {}.",
                        name.0,
                        structure_display_name(blueprint),
                    ),
                    NarrativeTier::Significant,
                );

                // Reset the channel that fired so it doesn't re-trigger next eval.
                match blueprint {
                    StructureType::Stores => pressure.storage = 0.0,
                    StructureType::Den => pressure.shelter = 0.0,
                    StructureType::Hearth => pressure.gathering = 0.0,
                    StructureType::Workshop => pressure.workshop = 0.0,
                    StructureType::Garden => pressure.farming = 0.0,
                    StructureType::Watchtower => pressure.defense = 0.0,
                    _ => {}
                }
            }
        }
    }
}

fn structure_display_name(kind: StructureType) -> &'static str {
    match kind {
        StructureType::Den => "den",
        StructureType::Hearth => "hearth",
        StructureType::Stores => "storehouse",
        StructureType::Workshop => "workshop",
        StructureType::Garden => "garden",
        StructureType::Watchtower => "watchtower",
        StructureType::WardPost => "ward post",
        StructureType::Wall => "wall",
        StructureType::Gate => "gate",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::mental::{Memory, MemoryEntry};

    /// A cat with no relationships has zero social weight.
    #[test]
    fn social_weight_no_relationships() {
        let cc = &crate::resources::SimConstants::default().coordination;
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let relationships = Relationships::default();
        let memory = Memory::default();

        let sw = social_weight(entity, &relationships, &memory, cc);
        assert_eq!(sw, 0.0);
    }

    /// Social weight increases with positive fondness.
    #[test]
    fn social_weight_increases_with_fondness() {
        let cc = &crate::resources::SimConstants::default().coordination;
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut rels = Relationships::default();
        rels.get_or_insert(a, b);
        rels.modify_fondness(a, b, 0.5);
        rels.modify_familiarity(a, b, 0.4);

        let memory = Memory::default();

        let sw = social_weight(a, &rels, &memory, cc);
        // positive fondness = 0.5, avg familiarity = 0.4, no events
        // 0.5 + 0.4 * 0.5 + 0 = 0.7
        assert!((sw - 0.7).abs() < 0.001, "expected ~0.7, got {sw}");
    }

    /// Negative fondness does not contribute to social weight.
    #[test]
    fn social_weight_ignores_negative_fondness() {
        let cc = &crate::resources::SimConstants::default().coordination;
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();

        let mut rels = Relationships::default();
        rels.get_or_insert(a, b);
        rels.modify_fondness(a, b, -0.5);
        rels.modify_familiarity(a, b, 0.6);

        let memory = Memory::default();

        let sw = social_weight(a, &rels, &memory, cc);
        // positive fondness clamped to 0, avg familiarity = 0.6
        // 0.0 + 0.6 * 0.5 + 0 = 0.3
        assert!((sw - 0.3).abs() < 0.001, "expected ~0.3, got {sw}");
    }

    /// Significant events contribute to social weight.
    #[test]
    fn social_weight_includes_significant_events() {
        let cc = &crate::resources::SimConstants::default().coordination;
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let relationships = Relationships::default();

        let mut memory = Memory::default();
        memory.remember(MemoryEntry {
            event_type: MemoryType::SocialEvent,
            location: None,
            involved: vec![],
            tick: 0,
            strength: 1.0,
            firsthand: true,
        });
        memory.remember(MemoryEntry {
            event_type: MemoryType::Death,
            location: None,
            involved: vec![],
            tick: 10,
            strength: 0.8,
            firsthand: true,
        });

        let sw = social_weight(entity, &relationships, &memory, cc);
        // 0 fondness + 0 familiarity + 2 events * 0.1 = 0.2
        assert!((sw - 0.2).abs() < 0.001, "expected ~0.2, got {sw}");
    }

    /// evaluate_coordinators selects the highest-scoring cat.
    #[test]
    fn evaluate_coordinators_picks_highest_scorer() {
        use bevy_ecs::schedule::Schedule;

        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 100,
            ..Default::default()
        });
        world.insert_resource(Relationships::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());

        let high_diligence = Personality {
            diligence: 0.9,
            sociability: 0.9,
            ..default_personality()
        };
        let low_diligence = Personality {
            diligence: 0.2,
            sociability: 0.2,
            ..default_personality()
        };

        // Give them relationships so social_weight > 0.
        let a = world
            .spawn((
                high_diligence,
                Memory::default(),
                Name("Bramble".to_string()),
            ))
            .id();
        let b = world
            .spawn((low_diligence, Memory::default(), Name("Reed".to_string())))
            .id();

        let mut rels = Relationships::default();
        rels.get_or_insert(a, b);
        rels.modify_fondness(a, b, 0.6);
        rels.modify_familiarity(a, b, 0.5);
        // Give b some fondness too so both have social weight.
        rels.modify_fondness(b, a, 0.3);
        rels.modify_familiarity(b, a, 0.4);
        world.insert_resource(rels);

        let mut schedule = Schedule::default();
        schedule.add_systems(evaluate_coordinators);
        schedule.run(&mut world);

        // Cat 'a' has higher diligence*sociability, should be coordinator.
        assert!(
            world.get::<Coordinator>(a).is_some(),
            "high-scoring cat should be coordinator"
        );
    }

    /// Small colony (< 6 cats) should have at most 1 coordinator.
    #[test]
    fn small_colony_max_one_coordinator() {
        use bevy_ecs::schedule::Schedule;

        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 100,
            ..Default::default()
        });
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());

        let strong = Personality {
            diligence: 0.9,
            sociability: 0.9,
            ..default_personality()
        };

        // 4 cats — all with good scores.
        let mut entities = Vec::new();
        for i in 0..4 {
            let e = world
                .spawn((strong.clone(), Memory::default(), Name(format!("Cat{i}"))))
                .id();
            entities.push(e);
        }

        // Give everyone relationships.
        let mut rels = Relationships::default();
        for i in 0..entities.len() {
            for j in (i + 1)..entities.len() {
                rels.get_or_insert(entities[i], entities[j]);
                rels.modify_fondness(entities[i], entities[j], 0.5);
                rels.modify_familiarity(entities[i], entities[j], 0.5);
            }
        }
        world.insert_resource(rels);

        let mut schedule = Schedule::default();
        schedule.add_systems(evaluate_coordinators);
        schedule.run(&mut world);

        let coordinator_count = entities
            .iter()
            .filter(|e| world.get::<Coordinator>(**e).is_some())
            .count();
        assert_eq!(
            coordinator_count, 1,
            "small colony should have exactly 1 coordinator, got {coordinator_count}"
        );
    }

    /// assess_colony_needs emits Hunt directive when food is low.
    #[test]
    fn assess_emits_hunt_when_food_low() {
        use crate::components::skills::Skills;
        use bevy_ecs::schedule::Schedule;

        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 20,
            ..Default::default()
        });
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        // Food stores at 10% capacity.
        world.insert_resource(crate::resources::food::FoodStores::new(5.0, 50.0, 0.0));

        let entity = world
            .spawn((
                Coordinator,
                DirectiveQueue::default(),
                Skills::default(),
                Name("Tester".to_string()),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(assess_colony_needs);
        schedule.run(&mut world);

        let queue = world.get::<DirectiveQueue>(entity).unwrap();
        assert!(
            queue
                .directives
                .iter()
                .any(|d| d.kind == DirectiveKind::Hunt),
            "should have Hunt directive when food is low; got: {:?}",
            queue.directives.iter().map(|d| d.kind).collect::<Vec<_>>()
        );
    }

    /// Domain specialization: a skilled hunter coordinator has a lower food threshold.
    #[test]
    fn domain_specialization_lowers_threshold() {
        use crate::components::skills::Skills;
        use bevy_ecs::schedule::Schedule;

        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 20,
            ..Default::default()
        });
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        // Food at 45% — above default 0.5 threshold but below shifted threshold
        // for a non-hunter coordinator.
        world.insert_resource(crate::resources::food::FoodStores::new(22.5, 50.0, 0.0));

        // Skilled hunter: threshold = 0.5 - 0.9*0.1 = 0.41. 45% > 41%, no directive.
        let mut hunter_skills = Skills::default();
        hunter_skills.hunting = 0.9;
        let hunter = world
            .spawn((
                Coordinator,
                DirectiveQueue::default(),
                hunter_skills,
                Name("Hunter".to_string()),
            ))
            .id();

        // Unskilled cat: threshold = 0.5 - 0.0*0.1 = 0.5. 45% < 50%, directive!
        let unskilled = world
            .spawn((
                Coordinator,
                DirectiveQueue::default(),
                Skills::default(),
                Name("Unskilled".to_string()),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(assess_colony_needs);
        schedule.run(&mut world);

        let hunter_queue = world.get::<DirectiveQueue>(hunter).unwrap();
        let unskilled_queue = world.get::<DirectiveQueue>(unskilled).unwrap();

        assert!(
            !hunter_queue
                .directives
                .iter()
                .any(|d| d.kind == DirectiveKind::Hunt),
            "skilled hunter coordinator should NOT emit Hunt at 45% food"
        );
        assert!(
            unskilled_queue
                .directives
                .iter()
                .any(|d| d.kind == DirectiveKind::Hunt),
            "unskilled coordinator should emit Hunt at 45% food"
        );
    }

    // --- BuildPressure ---

    #[test]
    fn build_pressure_accumulates_when_signal_active() {
        let mut pressure = BuildPressure::default();
        let attentiveness = 0.8; // diligent, ambitious, impatient
        let rate = BuildPressure::BASE_RATE * attentiveness;

        // Simulate 50 evaluations with active storage signal.
        for _ in 0..50 {
            pressure.storage += rate;
        }

        assert!(
            pressure.storage > 0.3,
            "storage pressure should accumulate significantly after 50 evals (got {})",
            pressure.storage,
        );
    }

    #[test]
    fn build_pressure_decays_when_signal_inactive() {
        let mut pressure = BuildPressure::default();
        pressure.storage = 0.5;

        // Simulate 20 evaluations with no signal.
        for _ in 0..20 {
            pressure.storage *= BuildPressure::DECAY;
        }

        assert!(
            pressure.storage < 0.2,
            "storage pressure should decay substantially after 20 evals (got {})",
            pressure.storage,
        );
    }

    #[test]
    fn attentive_coordinator_has_lower_action_threshold() {
        let attentive = Personality {
            diligence: 0.9,
            ambition: 0.9,
            patience: 0.1, // impatient → acts sooner
            ..default_personality()
        };
        let inattentive = Personality {
            diligence: 0.2,
            ambition: 0.1,
            patience: 0.9, // patient → deliberates longer
            ..default_personality()
        };

        let attentive_val =
            attentive.diligence * 0.5 + attentive.ambition * 0.3 + (1.0 - attentive.patience) * 0.2;
        let inattentive_val = inattentive.diligence * 0.5
            + inattentive.ambition * 0.3
            + (1.0 - inattentive.patience) * 0.2;

        let attentive_threshold = 1.0 - attentive_val * 0.3;
        let inattentive_threshold = 1.0 - inattentive_val * 0.3;

        assert!(
            attentive_threshold < inattentive_threshold,
            "attentive coordinator threshold ({attentive_threshold}) should be lower than inattentive ({inattentive_threshold})"
        );
    }

    #[test]
    fn highest_actionable_returns_none_below_threshold() {
        let pressure = BuildPressure {
            storage: 0.3,
            shelter: 0.2,
            ..Default::default()
        };
        assert!(
            pressure.highest_actionable(0.5).is_none(),
            "no channel above threshold 0.5"
        );
    }

    #[test]
    fn highest_actionable_returns_highest_channel() {
        let pressure = BuildPressure {
            storage: 0.8,
            shelter: 0.9,
            ..Default::default()
        };
        let result = pressure.highest_actionable(0.5);
        assert_eq!(
            result,
            Some(StructureType::Den),
            "shelter (0.9) is highest above threshold"
        );
    }

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
}
