use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rand::SeedableRng;

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
use crate::resources::time::{TimeScale, TimeState};

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
/// and sociability. Runs once per in-game day or immediately when a coordinator
/// dies (cadence governed by `CoordinationConstants::evaluate_interval`).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_coordinators(
    mut commands: Commands,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
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
    let should_run =
        coordinator_died.is_some() || c.evaluate_interval.fires_at(time.tick, &time_scale);
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
// WardPlacementSignals — bundles the four spatial inputs the perimeter
// scoring loop reads. Lives here to keep `assess_colony_needs` under
// Bevy's 16-param tuple limit per CLAUDE.md guidance.
// ---------------------------------------------------------------------------

#[derive(SystemParam)]
pub struct WardPlacementSignals<'w> {
    pub tile_map: Res<'w, crate::resources::map::TileMap>,
    pub fox_scent: Res<'w, crate::resources::FoxScentMap>,
    pub cat_presence: Res<'w, crate::resources::CatPresenceMap>,
    pub ward_coverage: Res<'w, crate::resources::WardCoverageMap>,
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
    mut garden_query: Query<
        &mut crate::components::building::CropState,
        With<crate::components::building::Structure>,
    >,
    ward_query: Query<(&crate::components::magic::Ward, &Position)>,
    herb_query: Query<&crate::components::magic::Herb, With<crate::components::magic::Harvestable>>,
    wildlife: Query<(Entity, &Position, &crate::components::wildlife::WildAnimal)>,
    carcass_query: Query<(Entity, &Position, &crate::components::wildlife::Carcass)>,
    placement_signals: WardPlacementSignals,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    constants: Res<SimConstants>,
    colony_center: Res<crate::resources::ColonyCenter>,
    mut activation: ResMut<SystemActivation>,
) {
    let map = &placement_signals.tile_map;
    let fox_scent = &placement_signals.fox_scent;
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
        .filter(|(_, wp, _)| {
            building_positions
                .iter()
                .any(|bp| bp.manhattan_distance(wp) <= cc.threat_proximity_range)
        })
        .map(|(e, p, _)| (e, *p))
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

    // Snapshot ward positions and radii for strength check + placement.
    let ward_data: Vec<(Position, f32)> = ward_query
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius()))
        .collect();

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 =
                ward_query.iter().map(|(w, _)| w.strength).sum::<f32>() / ward_count as f32;
            avg < cc.ward_avg_strength_low_threshold
        }
    };

    let thornbriar_available = herb_query
        .iter()
        .any(|h| h.kind == crate::components::magic::HerbKind::Thornbriar);

    // Corruption sweep: find the hottest corrupted tile in the territory ring
    // and any actionable carcass within colony reach.
    // Cheap scan — sample every few tiles, not every pixel.
    let corruption_hotspot: Option<(Position, f32)> = {
        let cx = colony_center.0.x;
        let cy = colony_center.0.y;
        let search_r: i32 = cc.corruption_search_radius;
        let step: i32 = cc.corruption_search_step.max(1);
        let mut best: Option<(Position, f32)> = None;
        let mut y = -search_r;
        while y <= search_r {
            let mut x = -search_r;
            while x <= search_r {
                let (nx, ny) = (cx + x, cy + y);
                if map.in_bounds(nx, ny) {
                    let c = map.get(nx, ny).corruption;
                    if c > cc.corruption_alarm_threshold
                        && best.as_ref().is_none_or(|(_, bc)| c > *bc)
                    {
                        best = Some((Position::new(nx, ny), c));
                    }
                }
                x += step;
            }
            y += step;
        }
        best
    };

    let uncleansed_carcasses: Vec<(Entity, Position)> = carcass_query
        .iter()
        .filter(|(_, p, c)| {
            !c.cleansed
                && !c.harvested
                && p.manhattan_distance(&colony_center.0) <= cc.corruption_search_radius
        })
        .map(|(e, p, _)| (e, *p))
        .collect();

    let mut event_log = event_log;

    for (coord_entity, name, skills, mut queue) in &mut coordinators {
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

        // Preemptive patrol: fox scent detected near colony without active sightings.
        if nearby_threats.is_empty() {
            if let Some((sx, sy)) = fox_scent.highest_nearby(
                colony_center.0.x,
                colony_center.0.y,
                cc.preemptive_patrol_scent_radius,
            ) {
                let scent_level = fox_scent.get(sx, sy);
                if scent_level > cc.preemptive_patrol_scent_threshold {
                    queue.directives.push(Directive {
                        kind: DirectiveKind::Patrol,
                        priority: cc.preemptive_patrol_priority,
                        target_entity: None,
                        target_position: Some(Position::new(sx, sy)),
                        blueprint: None,
                    });
                }
            }
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

        // Shadow-fox posse formation — when a shadow-fox is detected inside
        // colony territory, queue `posse_size` Fight directives targeting it
        // so the colony musters a counter-offensive instead of only warding.
        // Each directive gets dispatched to a different combat-capable cat
        // by the urgent-dispatch pipeline (which already picks the best
        // uncommitted cat per directive). The high priority keeps these
        // above ward/herbcraft work.
        for (wildlife_entity, wpos, animal) in wildlife.iter() {
            // Only shadow-foxes trigger posse response; regular foxes are
            // handled by ambient Patrol / threat-interrupt logic.
            if animal.species != crate::components::wildlife::WildSpecies::ShadowFox {
                continue;
            }
            if colony_center.0.manhattan_distance(wpos) > cc.posse_alarm_range {
                continue;
            }
            for _ in 0..cc.posse_size {
                queue.directives.push(Directive {
                    kind: DirectiveKind::Fight,
                    priority: cc.posse_priority,
                    target_entity: Some(wildlife_entity),
                    target_position: Some(*wpos),
                    blueprint: None,
                });
            }
        }

        // Ward assessment — only issue if thornbriar exists for cats to gather.
        if ward_strength_low && thornbriar_available {
            // Deterministic per-call jitter seeded by tick + coordinator entity.
            // Coordinator directives run at 20-tick intervals so same-call
            // stacking is rare; the seed varies each call, avoiding the need
            // to thread a shared SimRng (which would push the system past
            // Bevy's 16-param tuple limit).
            let seed = time.tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ coord_entity.to_bits();
            let mut local_rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
            let placement_maps = PlacementMaps {
                fox_scent: &placement_signals.fox_scent,
                cat_presence: &placement_signals.cat_presence,
                ward_coverage: &placement_signals.ward_coverage,
                tile_map: &placement_signals.tile_map,
            };
            let ward_pos = compute_ward_placement(
                &building_positions,
                &ward_data,
                colony_center.0,
                &placement_maps,
                &mut local_rng,
            );
            queue.directives.push(Directive {
                kind: DirectiveKind::SetWard,
                priority: cc.ward_set_priority,
                target_entity: None,
                target_position: Some(ward_pos),
                blueprint: None,
            });
        }

        // Corruption response — issue a Cleanse directive on the hottest
        // corrupted tile, and/or a HarvestCarcass directive on nearby carcasses.
        // Priority scales with corruption severity so a breached ward gets
        // immediate attention.
        if let Some((hotspot_pos, hotspot_c)) = corruption_hotspot {
            let priority = (hotspot_c * cc.corruption_directive_priority_scale
                + skills.magic * cc.corruption_directive_magic_scale)
                .min(1.0);
            queue.directives.push(Directive {
                kind: DirectiveKind::Cleanse,
                priority,
                target_entity: None,
                target_position: Some(hotspot_pos),
                blueprint: None,
            });
        }
        if let Some((carcass_entity, carcass_pos)) = uncleansed_carcasses.first() {
            let priority = (cc.carcass_directive_priority_base
                + skills.herbcraft * cc.carcass_directive_herbcraft_scale)
                .min(1.0);
            queue.directives.push(Directive {
                kind: DirectiveKind::HarvestCarcass,
                priority,
                target_entity: Some(*carcass_entity),
                target_position: Some(*carcass_pos),
                blueprint: None,
            });
        }

        // Garden repurposing: if wards are needed but no thornbriar exists,
        // convert one food-crop garden to thornbriar production.
        if ward_strength_low && !thornbriar_available {
            for mut crop in &mut garden_query {
                if crop.crop_kind == crate::components::building::CropKind::FoodCrops {
                    crop.crop_kind = crate::components::building::CropKind::Thornbriar;
                    crop.growth = 0.0;
                    break; // Only convert one garden at a time.
                }
            }
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
// dispatch_urgent_directives — auto-assign high-priority directives
// ---------------------------------------------------------------------------

/// For each coordinator's queued directives above the emergency threshold,
/// skip the physical walk-to-cat delivery and directly insert [`ActiveDirective`]
/// on the best-skilled uncommitted cat within reach.
///
/// This is the "radio" for emergencies: when corruption breaches the colony
/// or predators siege wards, the coordinator can't afford to wander around
/// handing out orders. Lower-priority directives still route through the
/// normal Coordinating disposition so cats learn of them through social
/// contact (narrative texture preserved for the non-urgent flow).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn dispatch_urgent_directives(
    mut commands: Commands,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut coordinators: Query<
        (
            Entity,
            &Position,
            &crate::components::physical::Needs,
            &mut DirectiveQueue,
        ),
        With<Coordinator>,
    >,
    candidates: Query<
        (
            Entity,
            &Position,
            &Skills,
            &crate::components::physical::Needs,
        ),
        (
            Without<ActiveDirective>,
            Without<Dead>,
            Without<Coordinator>,
        ),
    >,
    mut activation: ResMut<SystemActivation>,
) {
    let cc = &constants.coordination;
    let critical_hunger = constants.disposition.critical_hunger_interrupt_threshold;
    if !time.tick.is_multiple_of(cc.assess_interval) {
        return;
    }

    for (coord_entity, coord_pos, coord_needs, mut queue) in &mut coordinators {
        // Collect uncommitted candidates once per coordinator. Hunger is
        // captured so Fight directives can skip cats below the critical
        // starvation floor — pulling a starving cat into a posse loses us
        // the cat either to death mid-combat or to a desertion interrupt.
        let cands: Vec<(Entity, Position, Skills, f32)> = candidates
            .iter()
            .map(|(e, p, s, n)| (e, *p, s.clone(), n.hunger))
            .collect();

        let mut dispatched_indices: Vec<usize> = Vec::new();
        // Track cats already receiving a directive this cycle so a posse
        // doesn't dispatch the same "best combat cat" for every Fight
        // directive in the queue.
        let mut already_dispatched: Vec<Entity> = Vec::new();
        // One urgent directive per coordinator per cycle in the general
        // case. Anything more and the colony drops everything chasing
        // corruption — ward renewal and hunting collapse. Exception: Fight
        // directives dispatch without this cap so a coordinator can
        // assemble a full posse (typically 3 cats) in a single cycle.
        // Also tracks per-target Fight dispatches so the posse doesn't
        // drag in more cats than posse_size.
        let mut urgent_slots_remaining: u32 = 1;
        let mut fight_dispatches_per_target: std::collections::HashMap<Entity, u32> =
            std::collections::HashMap::new();

        for (idx, directive) in queue.directives.iter().enumerate() {
            let is_fight = matches!(directive.kind, DirectiveKind::Fight);
            if !is_fight && urgent_slots_remaining == 0 {
                break;
            }
            if directive.priority < cc.urgent_directive_priority_threshold {
                continue;
            }
            if is_fight {
                if let Some(target) = directive.target_entity {
                    let count = fight_dispatches_per_target.entry(target).or_insert(0);
                    if *count >= cc.posse_size as u32 {
                        continue;
                    }
                }
            }
            // Pick the best-skilled cat for the directive within range.
            let skill_of = |s: &Skills| -> f32 {
                match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                    DirectiveKind::Cleanse => s.magic,
                    DirectiveKind::HarvestCarcass => s.herbcraft,
                    // Cooking has no dedicated skill — treat as neutral.
                    DirectiveKind::Cook => 0.0,
                }
            };
            // Fight directives respect the critical-hunger floor: a starving
            // cat sent to fight will either starve mid-combat or interrupt
            // to eat, leaving the posse short. Other directive kinds still
            // accept hungry cats because they don't carry immediate mortal
            // risk (and hunting/foraging directives actively help the cat).
            if is_fight {
                for (e, p, _, hunger) in &cands {
                    if coord_pos.manhattan_distance(p) <= cc.urgent_dispatch_range
                        && !already_dispatched.contains(e)
                        && *hunger < critical_hunger
                    {
                        activation.record(Feature::PosseCandidateExcludedStarving);
                    }
                }
            }
            let best = cands
                .iter()
                .filter(|(e, p, _, hunger)| {
                    coord_pos.manhattan_distance(p) <= cc.urgent_dispatch_range
                        && !already_dispatched.contains(e)
                        && !(is_fight && *hunger < critical_hunger)
                })
                .max_by(|(_, pa, sa, _), (_, pb, sb, _)| {
                    let va = skill_of(sa) - coord_pos.manhattan_distance(pa) as f32 * 0.01;
                    let vb = skill_of(sb) - coord_pos.manhattan_distance(pb) as f32 * 0.01;
                    va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some((target_entity, _, _, _)) = best {
                commands.entity(*target_entity).insert(ActiveDirective {
                    kind: directive.kind,
                    priority: directive.priority,
                    coordinator: coord_entity,
                    coordinator_social_weight: coord_needs.respect,
                    delivered_tick: time.tick,
                    target_position: directive.target_position,
                    target_entity: directive.target_entity,
                });
                activation.record(Feature::DirectiveDelivered);
                dispatched_indices.push(idx);
                already_dispatched.push(*target_entity);
                if is_fight {
                    if let Some(tgt) = directive.target_entity {
                        *fight_dispatches_per_target.entry(tgt).or_insert(0) += 1;
                    }
                } else {
                    urgent_slots_remaining -= 1;
                }
            }
        }

        // Remove dispatched directives from the queue so the physical
        // coordinator isn't also trying to deliver them.
        for idx in dispatched_indices.into_iter().rev() {
            queue.directives.remove(idx);
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

/// Whether `pressure.farming` should accumulate this tick.
///
/// Gardens are multiuse — they grow food crops *and* thornbriar (for wards).
/// The gate fires when the colony lacks a garden AND at least one demand axis
/// wants one: low food stockpile *or* weak wards with no thornbriar supply.
/// Once a garden exists, this returns `false` and the post-construction
/// repurposing path (`assess_colony_needs:530`) handles food↔herb specialization.
pub(crate) fn should_accumulate_farming_pressure(
    has_garden: bool,
    food_demand: bool,
    herb_demand: bool,
) -> bool {
    !has_garden && (food_demand || herb_demand)
}

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
    construction_sites: Query<&crate::components::building::ConstructionSite>,
    stored_items_query: Query<(
        &crate::components::building::Structure,
        &crate::components::building::StoredItems,
    )>,
    wildlife: Query<&Position, With<crate::components::wildlife::WildAnimal>>,
    items_query: Query<
        &crate::components::items::Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    wards: Query<&crate::components::magic::Ward>,
    herbs: Query<
        &crate::components::magic::Herb,
        With<crate::components::magic::Harvestable>,
    >,
    cat_inventories: Query<&crate::components::magic::Inventory, Without<Dead>>,
    mut unmet_demand: ResMut<crate::resources::UnmetDemand>,
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
    let has_stores = has_structure(StructureType::Stores);

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
    let has_kitchen = has_structure(StructureType::Kitchen);

    // Garden demand splits into two axes — gardens are multiuse:
    //   • food-side  — colony's stockpile is running low.
    //   • herb-side  — wards are weak AND the colony has no thornbriar
    //                  in any reachable form (no wild patches AND no cat
    //                  carrying any). Stockpile-aware (vs. world-only)
    //                  so the build commitment is not coupled to wild
    //                  patch respawn flicker. Mirrors
    //                  `assess_colony_needs:530` repurposing logic but
    //                  applies a stricter supply check, since building
    //                  is irreversible while repurposing is cheap.
    let ward_strength_low = crate::systems::magic::is_ward_strength_low(
        wards.iter(),
        cc.ward_avg_strength_low_threshold,
    );
    let wild_thornbriar_available =
        crate::systems::magic::is_thornbriar_available(herbs.iter());
    let any_cat_carrying_thornbriar = cat_inventories
        .iter()
        .any(|inv| inv.has_herb(crate::components::magic::HerbKind::Thornbriar));
    let has_hearth = has_structure(StructureType::Hearth);
    // ConstructionSite entities only exist while the build is incomplete —
    // they're despawned on completion. So any non-empty iter means there's
    // work to do somewhere.
    let has_unfinished_site = construction_sites.iter().next().is_some();
    let raw_food_items = stored_items_query
        .iter()
        .filter(|(s, _)| s.kind == StructureType::Stores)
        .map(|(_, si)| {
            si.items
                .iter()
                .copied()
                .filter(|&e| {
                    items_query
                        .get(e)
                        .is_ok_and(|it| it.kind.is_food() && !it.modifiers.cooked)
                })
                .count()
        })
        .sum::<usize>();

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

        // No-store pressure — colony has no Stores building at all.
        if !has_stores {
            pressure.no_store += rate * cc.no_store_pressure_multiplier;
        } else {
            pressure.no_store *= BuildPressure::DECAY;
        }

        // Storage pressure — existing stores are full.
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

        // Cooking pressure. Two regimes, both additive:
        //   1. Foundational: no Kitchen exists at all → push hard
        //      regardless of raw-food or hearth state. The colony can't
        //      enter the Cook loop without one, so it's a phase unlock
        //      (mirrors `no_store_pressure_multiplier` for Stores).
        //   2. Incremental: a Hearth exists and raw food is piling up
        //      → the existing `cooking_pressure_multiplier` path compounds.
        // The unmet-demand ledger amplifies both — frustrated Cook desires
        // from scoring feed directly into the push.
        //
        // TODO(strategist-coordinator): hard-coded "Kitchen is foundational"
        // is a stopgap. A future coordinator should reason over a building
        // tech-tree (Hearth → Kitchen → Workshop → …) and beeline toward
        // phase-unlock structures the way Civilization AI does. See
        // `docs/systems/strategist-coordinator.md`.
        if !has_kitchen {
            let demand_boost = 1.0 + unmet_demand.kitchen * cc.unmet_demand_amplifier;
            pressure.cooking += rate * cc.no_kitchen_pressure_multiplier * demand_boost;
            if has_hearth && raw_food_items >= cc.build_pressure_cooking_min_raw_food {
                pressure.cooking += rate * cc.cooking_pressure_multiplier * demand_boost;
            }
        } else {
            pressure.cooking *= BuildPressure::DECAY;
        }

        // Cook directive — once a Kitchen exists and raw food is available,
        // keep a low-priority Cook directive live on the queue so diligent
        // cats get nudged to prep meals. Lower priority than Hunt/Fight, so
        // survival directives still win when they matter.
        if has_kitchen && raw_food_items > 0 {
            queue.directives.push(Directive {
                kind: DirectiveKind::Cook,
                priority: cc.cook_directive_priority,
                target_entity: None,
                target_position: None,
                blueprint: None,
            });
        }

        // Site-directed Build urgency. The blueprint-carrying Build
        // directive is consumed by `spawn_construction_sites` as soon as
        // a site entity exists — it doesn't propagate to cats. Without
        // a follow-up directive, cats never get an ActiveDirective{Build}
        // and their Build scoring stays at baseline, so sites linger.
        //
        // Push a blueprint-less Build directive above the urgent
        // threshold so `dispatch_urgent_directives` routes it to cats
        // directly. Dedup on `kind == Build && blueprint.is_none()` so
        // the queue doesn't bloat across assess cycles.
        if has_unfinished_site {
            let already_queued = queue
                .directives
                .iter()
                .any(|d| d.kind == DirectiveKind::Build && d.blueprint.is_none());
            if !already_queued {
                queue.directives.push(Directive {
                    kind: DirectiveKind::Build,
                    priority: cc.construct_site_directive_priority,
                    target_entity: None,
                    target_position: None,
                    blueprint: None,
                });
            }
        }

        // Farming pressure — gardens are multiuse (food crops + thornbriar
        // for wards), so accumulate when *either* demand axis fires. See
        // `should_accumulate_farming_pressure` for the truth-table contract.
        let food_demand = food_fraction < cc.build_pressure_farming_food_threshold;
        let herb_demand =
            ward_strength_low && !wild_thornbriar_available && !any_cat_carrying_thornbriar;
        if should_accumulate_farming_pressure(has_garden, food_demand, herb_demand) {
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
            // One build at a time. Starting a second site while the first
            // is unfinished just scatters the colony's labor across
            // competing projects — a Kitchen + Storehouse + Workshop all
            // started in consecutive cycles means none of them finish.
            // Surplus-labor-aware parallelism (allow N sites when idle
            // cats > some threshold) is a future refinement — see
            // docs/systems/strategist-coordinator.md.
            if !already_queued && !has_unfinished_site {
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
                    StructureType::Stores => {
                        pressure.storage = 0.0;
                        pressure.no_store = 0.0;
                    }
                    StructureType::Den => pressure.shelter = 0.0,
                    StructureType::Hearth => pressure.gathering = 0.0,
                    StructureType::Workshop => pressure.workshop = 0.0,
                    StructureType::Kitchen => {
                        pressure.cooking = 0.0;
                        // Clearing the demand once the build is scheduled
                        // prevents stale frustration from re-priming the
                        // pressure after Kitchen is marked for
                        // construction.
                        unmet_demand.kitchen = 0.0;
                    }
                    StructureType::Garden => pressure.farming = 0.0,
                    StructureType::Watchtower => pressure.defense = 0.0,
                    _ => {}
                }
            }
        }
    }
    // Decay unmet-demand once per assessment cycle, regardless of whether
    // any pressure fired. Frustration fades over time when no cat tries.
    unmet_demand.decay();
}

fn structure_display_name(kind: StructureType) -> &'static str {
    match kind {
        StructureType::Den => "den",
        StructureType::Hearth => "hearth",
        StructureType::Kitchen => "kitchen",
        StructureType::Stores => "storehouse",
        StructureType::Workshop => "workshop",
        StructureType::Garden => "garden",
        StructureType::Watchtower => "watchtower",
        StructureType::WardPost => "ward post",
        StructureType::Wall => "wall",
        StructureType::Gate => "gate",
        StructureType::Midden => "midden",
    }
}

// ---------------------------------------------------------------------------
// Construction site spawning
// ---------------------------------------------------------------------------

/// Convert Build directives into physical ConstructionSite entities on the map.
///
/// When a coordinator issues a Build directive with a blueprint, this system
/// finds a valid placement near the colony center and spawns the site. For the
/// founding store, materials are pre-delivered (the colony pooled resources they
/// arrived with).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn spawn_construction_sites(
    mut commands: Commands,
    mut coordinators: Query<(&mut DirectiveQueue, &Name), With<Coordinator>>,
    buildings: Query<(&crate::components::building::Structure, &Position)>,
    construction_sites: Query<&crate::components::building::ConstructionSite>,
    colony_center: Res<crate::resources::ColonyCenter>,
    mut map: ResMut<crate::resources::map::TileMap>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
) {
    // Track blueprints spawned this tick to prevent duplicates from multiple
    // coordinators issuing the same directive (commands are deferred, so
    // construction_sites won't see entities spawned earlier in this loop).
    let mut spawned_this_tick = std::collections::HashSet::new();

    for (mut queue, coordinator_name) in &mut coordinators {
        // Find the first Build directive with a blueprint.
        let directive_idx = queue
            .directives
            .iter()
            .position(|d| d.kind == DirectiveKind::Build && d.blueprint.is_some());
        let Some(idx) = directive_idx else {
            continue;
        };

        let blueprint = queue.directives[idx].blueprint.unwrap();

        // Don't spawn a duplicate if a site for this blueprint already exists.
        let already_exists = construction_sites
            .iter()
            .any(|site| site.blueprint == blueprint)
            || spawned_this_tick.contains(&blueprint);
        // Also skip if the building type already exists as a completed structure.
        let already_built = buildings.iter().any(|(s, _)| s.kind == blueprint);
        if already_exists || already_built {
            queue.directives.remove(idx);
            continue;
        }

        let size = blueprint.default_size();
        let center = colony_center.0;

        // Find a valid placement position via spiral search from colony center.
        let placement = find_building_placement(&map, center, size, &buildings);
        let Some(anchor) = placement else {
            // No valid placement found — leave the directive for next tick.
            continue;
        };

        // Stamp terrain footprint.
        let terrain = blueprint.terrain();
        for dy in 0..size.1 {
            for dx in 0..size.0 {
                let x = anchor.x + dx;
                let y = anchor.y + dy;
                if map.in_bounds(x, y) {
                    map.set(x, y, terrain);
                }
            }
        }

        // Spawn the construction site entity. Founding buildings get pre-funded
        // materials (the colony pools what they brought with them).
        let site = crate::components::building::ConstructionSite::new_prefunded(blueprint);
        commands.spawn((
            Name(format!(
                "Construction: {}",
                structure_display_name(blueprint)
            )),
            anchor,
            crate::components::building::Structure {
                kind: blueprint,
                condition: 0.0,
                cleanliness: 0.0,
                size,
            },
            site,
        ));

        spawned_this_tick.insert(blueprint);

        log.push(
            time.tick,
            format!(
                "{} marks out the site for a new {}.",
                coordinator_name.0,
                structure_display_name(blueprint),
            ),
            NarrativeTier::Significant,
        );

        queue.directives.remove(idx);
    }
}

/// Spiral search outward from `center` to find a position where a building of
/// `size` fits with all tiles passable and at least 1 tile gap from existing
/// buildings.
fn find_building_placement(
    map: &crate::resources::map::TileMap,
    center: Position,
    size: (i32, i32),
    buildings: &Query<(&crate::components::building::Structure, &Position)>,
) -> Option<Position> {
    // Collect existing building footprints for gap checking.
    let occupied: Vec<(Position, (i32, i32))> =
        buildings.iter().map(|(s, p)| (*p, s.size)).collect();

    // Spiral search: try positions at increasing Manhattan distance.
    for radius in 1..=16_i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() + dy.abs() != radius {
                    continue; // Only check the ring at this radius.
                }
                let anchor = Position::new(center.x + dx, center.y + dy);
                if footprint_valid(map, anchor, size, &occupied) {
                    return Some(anchor);
                }
            }
        }
    }
    None
}

/// Check that every tile in the footprint is passable, in-bounds, and has at
/// least a 1-tile gap from any existing building footprint.
fn footprint_valid(
    map: &crate::resources::map::TileMap,
    anchor: Position,
    size: (i32, i32),
    occupied: &[(Position, (i32, i32))],
) -> bool {
    // All tiles in footprint must be passable natural terrain.
    for dy in 0..size.1 {
        for dx in 0..size.0 {
            let x = anchor.x + dx;
            let y = anchor.y + dy;
            if !map.in_bounds(x, y) {
                return false;
            }
            let terrain = map.get(x, y).terrain;
            if !terrain.is_passable() || terrain.is_building() {
                return false;
            }
        }
    }

    // 1-tile gap from existing building footprints.
    for &(bpos, bsize) in occupied {
        if footprints_overlap_with_gap(anchor, size, bpos, bsize, 1) {
            return false;
        }
    }

    true
}

/// Check if two footprints (expanded by `gap` tiles) overlap.
fn footprints_overlap_with_gap(
    a_pos: Position,
    a_size: (i32, i32),
    b_pos: Position,
    b_size: (i32, i32),
    gap: i32,
) -> bool {
    let a_left = a_pos.x - gap;
    let a_right = a_pos.x + a_size.0 + gap;
    let a_top = a_pos.y - gap;
    let a_bottom = a_pos.y + a_size.1 + gap;

    let b_left = b_pos.x;
    let b_right = b_pos.x + b_size.0;
    let b_top = b_pos.y;
    let b_bottom = b_pos.y + b_size.1;

    a_left < b_right && a_right > b_left && a_top < b_bottom && a_bottom > b_top
}

// ---------------------------------------------------------------------------
// Ward placement — sample influence maps to pick the best perimeter tile
// ---------------------------------------------------------------------------

/// Sampler bundle for ward-placement scoring. Borrowed at the call site
/// from the `WardPlacementSignals` SystemParam; kept as a thin struct so
/// the placement algorithm stays a pure function over plain references
/// (testable without spinning up a Bevy World).
pub(crate) struct PlacementMaps<'a> {
    pub fox_scent: &'a crate::resources::FoxScentMap,
    pub cat_presence: &'a crate::resources::CatPresenceMap,
    pub ward_coverage: &'a crate::resources::WardCoverageMap,
    pub tile_map: &'a crate::resources::map::TileMap,
}

impl<'a> PlacementMaps<'a> {
    fn corruption_at(&self, x: i32, y: i32) -> f32 {
        if self.tile_map.in_bounds(x, y) {
            self.tile_map.get(x, y).corruption
        } else {
            0.0
        }
    }
}

/// Pick a position for a new ward by sampling L1 influence maps at
/// candidate tiles across the whole map.
///
/// Per-tile score:
/// - `unaddressed_threat = max(fox_scent, corruption) - ward_coverage`,
///   clamped to `[0, 1]`. High = SFs walked here recently or corruption
///   is creeping, AND existing wards aren't already covering the tile.
/// - `cat_value = cat_presence` — modest bonus for tiles where cats
///   actually live (a ward covering nobody is wasted).
/// - `distance_cost = DIST_PENALTY_PER_TILE × manhattan(anchor, candidate)`
///   — soft travel-cost term so the priestess doesn't walk to the
///   opposite map corner for a marginal score gain. At 0.005/tile a
///   100-tile detour subtracts 0.5 from the score, so a fully-saturated
///   threat tile far away still beats a half-strength threat nearby
///   only by a meaningful margin.
/// - `score = unaddressed_threat + 0.3 × cat_value − distance_cost`
///   plus small jitter for tie-breaking.
///
/// Candidates are a coarse map-wide grid (every 5 tiles, bucket-aligned
/// with the influence maps), with hard exclusion of tiles within
/// Manhattan-3 of any existing ward.
///
/// Falls back to the structure-cluster centroid when (a) no wards yet
/// exist and structures are present (first-ward heuristic, blankets the
/// core) or (b) every candidate is excluded.
pub(crate) fn compute_ward_placement(
    building_positions: &[Position],
    ward_positions: &[(Position, f32)],
    colony_center: Position,
    maps: &PlacementMaps<'_>,
    rng: &mut impl rand::Rng,
) -> Position {
    let anchor = if building_positions.is_empty() {
        colony_center
    } else {
        let (sx, sy) = building_positions.iter().fold((0i64, 0i64), |(ax, ay), p| {
            (ax + p.x as i64, ay + p.y as i64)
        });
        let n = building_positions.len() as i64;
        Position::new((sx / n) as i32, (sy / n) as i32)
    };

    // First ward with structures: blanket the cluster centroid.
    if ward_positions.is_empty() && !building_positions.is_empty() {
        return anchor;
    }

    // Fallback default for empty colonies before any structures exist.
    if ward_positions.is_empty() {
        return anchor;
    }

    // Coarse-grid candidate generation across the whole map (every 5
    // tiles, matching the bucket size of the influence maps). For the
    // default 120×90 map this yields ~430 candidates; cheap to score.
    const CANDIDATE_STEP: i32 = 5;
    const HARD_EXCLUDE_MANHATTAN: i32 = 3;
    /// Travel-cost penalty per Manhattan tile from the anchor. Tuned so
    /// a 100-tile detour costs 0.5 score — a saturated threat far away
    /// still beats a half-saturated threat nearby, but only by a real
    /// margin. Picked dimensionlessly against the [0, 1] threat axis;
    /// no balance constant needed.
    const DIST_PENALTY_PER_TILE: f32 = 0.005;
    let map_w = maps.tile_map.width;
    let map_h = maps.tile_map.height;
    let mut candidates: Vec<Position> = Vec::new();
    for cy in (0..map_h).step_by(CANDIDATE_STEP as usize) {
        for cx in (0..map_w).step_by(CANDIDATE_STEP as usize) {
            let candidate = Position::new(cx, cy);
            if ward_positions
                .iter()
                .any(|(wp, _)| candidate.manhattan_distance(wp) <= HARD_EXCLUDE_MANHATTAN)
            {
                continue;
            }
            candidates.push(candidate);
        }
    }

    // Edge case: every candidate excluded (very crowded colony) — fall
    // back to the anchor so we still emit *something*.
    if candidates.is_empty() {
        return anchor;
    }

    let mut best_pos = candidates[0];
    let mut best_score = f32::NEG_INFINITY;

    for candidate in &candidates {
        let fox_scent = maps.fox_scent.get(candidate.x, candidate.y);
        let corruption = maps.corruption_at(candidate.x, candidate.y);
        let coverage = maps.ward_coverage.get(candidate.x, candidate.y);
        let cat_value = maps.cat_presence.get(candidate.x, candidate.y);

        let threat = fox_scent.max(corruption);
        let unaddressed_threat = (threat - coverage).clamp(0.0, 1.0);

        let dist = anchor.manhattan_distance(candidate) as f32;
        let distance_cost = DIST_PENALTY_PER_TILE * dist;

        // Small jitter ([0, 0.05)) breaks ties deterministically without
        // overwhelming the influence-map signal.
        let jitter = rng.random_range(0.0_f32..0.05);
        let score = unaddressed_threat + 0.3 * cat_value - distance_cost + jitter;

        if score > best_score {
            best_score = score;
            best_pos = *candidate;
        }
    }
    best_pos
}

// ---------------------------------------------------------------------------
// §4 per-cat IsCoordinatorWithDirectives marker author
// ---------------------------------------------------------------------------

/// Author the `IsCoordinatorWithDirectives` ZST on cats that hold the
/// `Coordinator` role AND have a non-empty `DirectiveQueue`.
///
/// **Predicate** — `With<Coordinator> && directive_queue.directives.len() > 0`.
/// Bit-for-bit mirror of the inline `is_coordinator_with_directives`
/// computation in `goap.rs` / `disposition.rs`.
///
/// **Ordering** — Chain 2a, after `update_inventory_markers`. The
/// Coordinator component is stable within a tick (elections run in
/// Chain 2b), so the marker reflects the same state the scoring
/// pipeline would read.
///
/// **Lifecycle** — transition-only; idempotent in steady state. A second
/// query handles cats that lost the `Coordinator` role (or died) —
/// the marker is cleaned up in the same tick.
pub fn update_directive_markers(
    mut commands: Commands,
    coordinators: Query<
        (
            Entity,
            &DirectiveQueue,
            Has<crate::components::markers::IsCoordinatorWithDirectives>,
        ),
        With<Coordinator>,
    >,
    non_coordinators: Query<
        (
            Entity,
            Has<crate::components::markers::IsCoordinatorWithDirectives>,
        ),
        Without<Coordinator>,
    >,
) {
    use crate::components::markers::IsCoordinatorWithDirectives;

    for (entity, queue, has_marker) in coordinators.iter() {
        let has_directives = !queue.directives.is_empty();
        match (has_directives, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(IsCoordinatorWithDirectives);
            }
            (false, true) => {
                commands
                    .entity(entity)
                    .remove::<IsCoordinatorWithDirectives>();
            }
            _ => {}
        }
    }
    // Clean up stale markers on cats that lost coordinator status.
    for (entity, has_marker) in non_coordinators.iter() {
        if has_marker {
            commands
                .entity(entity)
                .remove::<IsCoordinatorWithDirectives>();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::mental::{Memory, MemoryEntry};

    fn empty_placement_maps() -> (
        crate::resources::FoxScentMap,
        crate::resources::CatPresenceMap,
        crate::resources::WardCoverageMap,
        crate::resources::map::TileMap,
    ) {
        (
            crate::resources::FoxScentMap::default(),
            crate::resources::CatPresenceMap::default(),
            crate::resources::WardCoverageMap::default(),
            crate::resources::map::TileMap::new(120, 90, crate::resources::Terrain::Grass),
        )
    }

    #[test]
    fn ward_placement_first_ward_lands_on_cluster_centroid() {
        // Empty wards + structures present → first-ward fallback returns
        // the structure-cluster centroid. Preserves the "blanket the
        // colony core" behavior across the influence-map rewrite.
        let structures = vec![Position::new(10, 10), Position::new(14, 10)];
        let wards: Vec<(Position, f32)> = vec![];
        let (fs, cp, wc, tm) = empty_placement_maps();
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_presence: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(0, 0),
            &maps,
            &mut rng,
        );
        assert_eq!(pos, Position::new(12, 10));
    }

    #[test]
    fn ward_placement_picks_fox_scent_corridor() {
        // One existing ward at the colony center; the new ward should
        // land near a tile saturated with fox-scent rather than back on
        // the cluster. Anchor at (60, 45); fox-scent peak at (67, 45)
        // is 7 tiles away — the soft distance penalty (0.005/tile = 0.035)
        // is dominated by the saturated threat signal (1.0).
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm) = empty_placement_maps();
        fs.deposit(67, 45, 1.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_presence: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &mut rng,
        );
        let dx = (pos.x - 67).abs();
        let dy = (pos.y - 45).abs();
        assert!(
            dx <= 5 && dy <= 5,
            "expected placement near fox-scent peak (67, 45), got {pos:?}"
        );
        // Anti-clustering hard-exclusion (Manhattan-3) keeps the new
        // ward off the existing one.
        assert!(
            pos.manhattan_distance(&Position::new(60, 45)) > 3,
            "placement {pos:?} too close to existing ward",
        );
    }

    #[test]
    fn ward_placement_avoids_already_covered_tiles() {
        // Fox-scent peak coincides with an already-covered region. The
        // anti-clustering term should push placement to a different
        // candidate even if it scores zero on threat — coverage
        // saturation cancels the fox_scent contribution.
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, mut wc, tm) = empty_placement_maps();
        fs.deposit(67, 45, 1.0);
        wc.stamp_ward(60, 45, 1.0, 9.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_presence: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(99);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &mut rng,
        );
        assert!(
            pos.manhattan_distance(&Position::new(60, 45)) > 3,
            "placement {pos:?} violates Manhattan-3 hard-exclusion",
        );
    }

    #[test]
    fn ward_placement_distance_penalty_prefers_nearby_threat() {
        // Two equally-saturated fox-scent peaks: one nearby, one far.
        // Distance penalty should pick the near one — a 60-tile detour
        // costs 0.30 score, exceeding the noise from jitter (max 0.05).
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm) = empty_placement_maps();
        fs.deposit(67, 45, 1.0); // 7 tiles from anchor
        fs.deposit(67, 85, 1.0); // 47 tiles from anchor — much farther
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_presence: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &mut rng,
        );
        let dist_near = pos.manhattan_distance(&Position::new(67, 45));
        let dist_far = pos.manhattan_distance(&Position::new(67, 85));
        assert!(
            dist_near < dist_far,
            "expected placement closer to the nearer peak; got pos={pos:?} \
             dist_near={dist_near} dist_far={dist_far}",
        );
    }

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
        // Tick 1000 = first once-per-day cadence boundary at default
        // TimeScale (1000 ticks/day).
        world.insert_resource(TimeState {
            tick: 1000,
            ..Default::default()
        });
        world.insert_resource(Relationships::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(TimeScale::from_config(
            &crate::resources::time::SimConfig::default(),
            16.6667,
        ));

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
            tick: 1000,
            ..Default::default()
        });
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(TimeScale::from_config(
            &crate::resources::time::SimConfig::default(),
            16.6667,
        ));

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
        world.insert_resource(crate::resources::ColonyCenter(Position::new(20, 20)));
        world.insert_resource(crate::resources::FoxScentMap::default());
        world.insert_resource(crate::resources::CatPresenceMap::default());
        world.insert_resource(crate::resources::WardCoverageMap::default());
        world.insert_resource(crate::resources::map::TileMap::new(
            50,
            50,
            crate::resources::map::Terrain::Grass,
        ));
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
        world.insert_resource(crate::resources::ColonyCenter(Position::new(20, 20)));
        world.insert_resource(crate::resources::FoxScentMap::default());
        world.insert_resource(crate::resources::CatPresenceMap::default());
        world.insert_resource(crate::resources::WardCoverageMap::default());
        world.insert_resource(crate::resources::map::TileMap::new(
            50,
            50,
            crate::resources::map::Terrain::Grass,
        ));
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

    #[test]
    fn farming_gate_truth_table() {
        use super::should_accumulate_farming_pressure as gate;

        // No garden + at least one demand axis → accumulate.
        assert!(gate(false, true, false), "food demand alone fires the gate");
        assert!(gate(false, false, true), "herb demand alone fires the gate");
        assert!(gate(false, true, true), "both demands also fire");

        // No garden + no demand → don't accumulate.
        assert!(!gate(false, false, false), "no demand → no pressure");

        // Garden already exists → never accumulate, regardless of demand.
        // (Repurposing logic at assess_colony_needs:530 handles food↔herb
        // specialization on the existing garden.)
        assert!(!gate(true, true, true), "has_garden short-circuits");
        assert!(!gate(true, true, false), "has_garden short-circuits");
        assert!(!gate(true, false, true), "has_garden short-circuits");
        assert!(!gate(true, false, false), "has_garden short-circuits");
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

    // --- update_directive_markers ---

    use crate::components::markers::IsCoordinatorWithDirectives;

    fn setup_directive_markers() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_directive_markers);
        (world, schedule)
    }

    fn has_coord_dir(world: &World, entity: Entity) -> bool {
        world.get::<IsCoordinatorWithDirectives>(entity).is_some()
    }

    #[test]
    fn coordinator_with_directives_gets_marker() {
        let (mut world, mut schedule) = setup_directive_markers();
        let cat = world
            .spawn((
                Coordinator,
                DirectiveQueue {
                    directives: vec![Directive {
                        kind: DirectiveKind::Build,
                        priority: 1.0,
                        target_entity: None,
                        target_position: None,
                        blueprint: Some(crate::components::building::StructureType::Den),
                    }],
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(has_coord_dir(&world, cat));
    }

    #[test]
    fn coordinator_empty_queue_no_marker() {
        let (mut world, mut schedule) = setup_directive_markers();
        let cat = world.spawn((Coordinator, DirectiveQueue::default())).id();
        schedule.run(&mut world);
        assert!(!has_coord_dir(&world, cat));
    }

    #[test]
    fn non_coordinator_never_gets_marker() {
        let (mut world, mut schedule) = setup_directive_markers();
        // Cat without Coordinator component — should never get the marker
        // even if somehow given a DirectiveQueue.
        let cat = world
            .spawn(DirectiveQueue {
                directives: vec![Directive {
                    kind: DirectiveKind::Build,
                    priority: 1.0,
                    target_entity: None,
                    target_position: None,
                    blueprint: Some(crate::components::building::StructureType::Den),
                }],
            })
            .id();
        schedule.run(&mut world);
        assert!(!has_coord_dir(&world, cat));
    }

    #[test]
    fn losing_coordinator_removes_marker() {
        let (mut world, mut schedule) = setup_directive_markers();
        let cat = world
            .spawn((
                Coordinator,
                DirectiveQueue {
                    directives: vec![Directive {
                        kind: DirectiveKind::Build,
                        priority: 1.0,
                        target_entity: None,
                        target_position: None,
                        blueprint: Some(crate::components::building::StructureType::Den),
                    }],
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(has_coord_dir(&world, cat));

        // Remove Coordinator role.
        world.entity_mut(cat).remove::<Coordinator>();
        schedule.run(&mut world);
        assert!(
            !has_coord_dir(&world, cat),
            "losing Coordinator should remove the directive marker"
        );
    }

    #[test]
    fn completing_directives_removes_marker() {
        let (mut world, mut schedule) = setup_directive_markers();
        let cat = world
            .spawn((
                Coordinator,
                DirectiveQueue {
                    directives: vec![Directive {
                        kind: DirectiveKind::Build,
                        priority: 1.0,
                        target_entity: None,
                        target_position: None,
                        blueprint: Some(crate::components::building::StructureType::Den),
                    }],
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(has_coord_dir(&world, cat));

        // Clear the directive queue.
        world
            .get_mut::<DirectiveQueue>(cat)
            .unwrap()
            .directives
            .clear();
        schedule.run(&mut world);
        assert!(
            !has_coord_dir(&world, cat),
            "empty directive queue should remove marker"
        );
    }
}
