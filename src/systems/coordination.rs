use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rand::SeedableRng;

use crate::components::building::StructureType;
use crate::components::coordination::{
    ActiveDirective, BuildPressure, ColonyAlignmentScore, Coordinator, CoordinatorDied, Directive,
    DirectiveKind, DirectiveQueue, PendingDelivery,
};
use crate::components::identity::Name;
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Position};
use crate::components::skills::Skills;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::{
    SimConstants, WardPlacementCatValueComposition, WardPlacementSemantics,
};
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
    // Ticket 427 Step 4 — single-pass aggregate over `iter_for` instead
    // of materializing the full `all_for` Vec.
    let mut positive_fondness_sum = 0.0f32;
    let mut familiarity_sum = 0.0f32;
    let mut count = 0usize;
    for (_, r) in relationships.iter_for(entity) {
        positive_fondness_sum += r.fondness.max(0.0);
        familiarity_sum += r.familiarity;
        count += 1;
    }
    let avg_familiarity: f32 = if count == 0 {
        0.0
    } else {
        familiarity_sum / count as f32
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
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn evaluate_coordinators(
    mut commands: Commands,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    coordinator_died: Option<Res<CoordinatorDied>>,
    // 487 — exclude Incapacitated from the candidate pool. A downed
    // cat can't fulfill the Coordinator role (every coordination-
    // affordance DSE forbids `Incapacitated`); electing them would
    // immediately recreate the phantom-leader bug
    // `flag_coordinator_incapacitated` exists to fix.
    query: Query<
        (Entity, &Personality, &Memory, &Name),
        (
            Without<Dead>,
            Without<crate::components::markers::Incapacitated>,
        ),
    >,
    existing_coordinators: Query<Entity, With<Coordinator>>,
    relationships: Res<Relationships>,
    mut log: ResMut<NarrativeLog>,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    // 487 — emergent-leader feedback. Cats whose recent action history
    // is dominated by colony-aligned work (Forage / Build / Cook /
    // Hunt / etc.) accumulate `ColonyAlignmentScore` via
    // `update_colony_alignment_scores`; that score multiplies the
    // election score below so the colony recognises a coordinator
    // from observed behaviour rather than imposing one on personality
    // alone. Disjoint from the main query: read-only `&` access on a
    // distinct Component, no aliasing conflict.
    alignment_q: Query<&ColonyAlignmentScore>,
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
            // 487 — multiplicative alignment factor. Wraps the existing
            // social-weight × personality score so alignment compounds
            // with the legacy pillars (cats with both strong personality
            // AND aligned action history win cleanly); a cat with no
            // alignment history (default `recent_aligned_actions = 0.0`)
            // hits factor 1.0 and rides the legacy score unchanged.
            let alignment_factor = alignment_q.get(entity).map_or(1.0, |s| {
                1.0 + s.recent_aligned_actions * c.alignment_skill_weight
            });
            let score = sw
                * personality.diligence
                * personality.sociability
                * (1.0 + personality.ambition * c.ambition_bonus)
                * alignment_factor;
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
    pub cat_scent: Res<'w, crate::resources::CatScentMap>,
    pub ward_coverage: Res<'w, crate::resources::WardCoverageMap>,
    /// 220: recent-ambush event memory (from ticket 219). Read at
    /// each candidate tile to bias placement toward empirical hot zones
    /// rather than the geometric perimeter. Weight gated by
    /// `ScoringConstants::ward_ambush_anchor_weight` — ships at 0.0,
    /// so the read is performed but has no scoring effect at land.
    pub recent_ambush: Res<'w, crate::resources::RecentAmbushMap>,
    /// 220: kill-site scent (Phase 2C substrate). Same dormant-at-land
    /// posture as `recent_ambush`; weight gated by
    /// `ScoringConstants::ward_recency_anchor_weight`.
    pub carcass_scent: Res<'w, crate::resources::CarcassScentMap>,
    /// 312: fox-approach corridor traffic. Populated by
    /// `update_fox_approach_corridor_map` reading patrolling-fox
    /// positions each tick; sampled by `compute_ward_placement` as
    /// the multiplicative-outside topological-criticality lift.
    /// Dormant at land (`ward_fox_approach_corridor_weight = 0.0`).
    pub fox_approach_corridor: Res<'w, crate::resources::FoxApproachCorridorMap>,
    /// 301: coordinator-stamped ward-placement intent. Populated by
    /// `compute_ward_placement` when
    /// `ward_placement_semantics == DescendingResidual`. At default
    /// `SingleShotArgmax` the populator short-circuits — the map is
    /// never written and the resource decays at no rate (factor 1.0,
    /// no f32 change), preserving seed-42 byte-identity. Bundled into
    /// this SystemParam to stay under Bevy's 16-param tuple limit on
    /// `assess_colony_needs`.
    pub intent: ResMut<'w, crate::resources::WardIntentMap>,
}

// ---------------------------------------------------------------------------
// assess_colony_needs
// ---------------------------------------------------------------------------

/// For each coordinator, evaluate colony state and fill their directive queue.
/// Runs every 20 ticks. The coordinator's own skills shift assessment thresholds
/// (domain specialization).
///
/// 487 — when no coordinator-tagged cat exists (the day-1 founder phase), a
/// subset of directives (Forage / Build / Herbcraft — the headline day-1
/// drivers) is emitted into the `ColonySelfDirectiveQueue` resource instead.
/// `dispatch_urgent_directives` drains both sources, so colony-self
/// directives still reach cats; the directive-bonus formula in
/// `goap.rs::evaluate_and_plan` substitutes
/// `colony_self_directive_weight` when `coordinator.is_none()`, so they
/// apply a softer pull than a charismatic coordinator's orders would.
/// More directive kinds (Cleanse / SetWard / Posse) stay coordinator-only
/// — those address mid/late-game ecology that day-1 founders haven't yet
/// surfaced.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn assess_colony_needs(
    time: Res<TimeState>,
    food: Res<crate::resources::food::FoodStores>,
    mut coordinators: Query<(Entity, &Name, &Skills, &mut DirectiveQueue), With<Coordinator>>,
    injured_cats: Query<(Entity, &crate::components::CatBodyModel, &Position), Without<Dead>>,
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
    mut placement_signals: WardPlacementSignals,
    event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    constants: Res<SimConstants>,
    colony_center: Res<crate::resources::ColonyCenter>,
    mut activation: ResMut<SystemActivation>,
    mut self_queue: ResMut<crate::components::coordination::ColonySelfDirectiveQueue>,
) {
    let map = &placement_signals.tile_map;
    let fox_scent = &placement_signals.fox_scent;
    let cc = &constants.coordination;
    if !time.tick.is_multiple_of(cc.assess_interval) {
        return;
    }

    // Pre-compute colony state once.
    let food_fraction = food.fraction();
    // 095 Phase 1 Stage B — count cats with any body part at Wounded
    // or worse. Replaces the legacy `Health.injuries`-based count.
    let colony_injury_count = injured_cats
        .iter()
        .filter(|(_, body, _)| body.any_wounded_or_worse())
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
                .any(|bp| bp.distance_to(wp) <= cc.threat_proximity_range)
        })
        .map(|(e, p, _)| (e, *p))
        .collect();

    // Breach = wildlife very close to a building.
    let breach_threats: Vec<(Entity, Position)> = nearby_threats
        .iter()
        .filter(|(_, wp)| {
            building_positions
                .iter()
                .any(|bp| bp.distance_to(wp) <= cc.colony_breach_range)
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
        let cx = colony_center.0.x();
        let cy = colony_center.0.y();
        let search_r: i32 = cc.corruption_search_radius.round() as i32;
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
                && p.distance_to(&colony_center.0) <= cc.corruption_search_radius
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
                placement_failure_count: 0,
            });
            // Also queue forage if food is critically low.
            if food_fraction < food_threshold * 0.5 {
                queue.directives.push(Directive {
                    kind: DirectiveKind::Forage,
                    priority: priority * cc.forage_critical_multiplier,
                    target_entity: None,
                    target_position: None,
                    blueprint: None,
                    placement_failure_count: 0,
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
                placement_failure_count: 0,
            });
        }
        if !nearby_threats.is_empty() {
            // Wildlife detected near colony — issue targeted Patrol toward it.
            let closest_threat = nearby_threats.iter().min_by_key(|(_, wp)| {
                building_positions
                    .iter()
                    .map(|bp| bp.tile_distance_squared(wp))
                    .min()
                    .unwrap_or(i32::MAX)
            });
            queue.directives.push(Directive {
                kind: DirectiveKind::Patrol,
                priority: cc.threat_patrol_targeted_priority,
                target_entity: None,
                target_position: closest_threat.map(|(_, p)| *p),
                blueprint: None,
                placement_failure_count: 0,
            });
        }

        // Preemptive patrol: fox scent detected near colony without active sightings.
        if nearby_threats.is_empty() {
            if let Some((sx, sy)) = fox_scent.highest_nearby(
                colony_center.0.x(),
                colony_center.0.y(),
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
                        placement_failure_count: 0,
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
                placement_failure_count: 0,
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
                placement_failure_count: 0,
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
            if colony_center.0.distance_to(wpos) > cc.posse_alarm_range {
                continue;
            }
            for _ in 0..cc.posse_size {
                queue.directives.push(Directive {
                    kind: DirectiveKind::Fight,
                    priority: cc.posse_priority,
                    target_entity: Some(wildlife_entity),
                    target_position: Some(*wpos),
                    blueprint: None,
                    placement_failure_count: 0,
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
                cat_scent: &placement_signals.cat_scent,
                ward_coverage: &placement_signals.ward_coverage,
                tile_map: &placement_signals.tile_map,
                recent_ambush: &placement_signals.recent_ambush,
                carcass_scent: &placement_signals.carcass_scent,
                fox_approach_corridor: &placement_signals.fox_approach_corridor,
            };
            let ward_pos = compute_ward_placement(
                &building_positions,
                &ward_data,
                colony_center.0,
                &placement_maps,
                &constants,
                &mut local_rng,
                Some(&mut placement_signals.intent),
            );
            queue.directives.push(Directive {
                kind: DirectiveKind::SetWard,
                priority: cc.ward_set_priority,
                target_entity: None,
                target_position: Some(ward_pos),
                blueprint: None,
                placement_failure_count: 0,
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
                placement_failure_count: 0,
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
                placement_failure_count: 0,
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

    // 487 — colony-self directives. Day-1 founder phase has no
    // coordinator-tagged cat yet, so the per-coordinator loop above
    // emits nothing and founder behavior collapses to whatever wins
    // L2 scoring with no colony-level signal — historically the
    // "cuddle puddle" of GroomOther chain-grooming. This block fills
    // the gap: when no coordinator exists AND the colony has work to
    // do, emit a small set of directives directly into the
    // `ColonySelfDirectiveQueue`. `dispatch_urgent_directives` drains
    // that queue alongside per-coordinator queues; the receiving cat
    // gets `ActiveDirective { coordinator: None, ... }` and scoring
    // substitutes `colony_self_directive_weight` for the missing
    // `social_weight`. Only Forage / Build / Herbcraft fire here —
    // the headline day-1 drivers. Coordinator-only directives
    // (Cleanse / SetWard / Posse / Cook) gate on mid/late-game
    // ecology a founder cohort hasn't yet surfaced.
    self_queue.directives.clear();
    if coordinators.iter().next().is_none() {
        // No coordinator-tagged cat exists — emit colony-self
        // directives. Skill-based threshold tuning is skipped (use
        // base thresholds and base priorities).
        let food_threshold = cc.food_threshold_base;
        if food_fraction < food_threshold {
            let priority = (1.0 - food_fraction).min(1.0);
            self_queue.directives.push(Directive {
                kind: DirectiveKind::Forage,
                priority,
                target_entity: None,
                target_position: None,
                blueprint: None,
                placement_failure_count: 0,
            });
        }
        // Repair the worst-damaged finished building if any are
        // below the base threshold. Mirrors the per-coordinator
        // "worst_building" pick at the base threshold (no
        // skill-tuned scaling).
        let building_threshold = cc.building_threshold_base;
        let worst_building = building_query
            .iter()
            .filter(|(_, s, _, site)| site.is_none() && s.condition < building_threshold)
            .min_by(|(_, a, _, _), (_, b, _, _)| {
                a.condition
                    .partial_cmp(&b.condition)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some((build_entity, _, build_pos, _)) = worst_building {
            self_queue.directives.push(Directive {
                kind: DirectiveKind::Build,
                priority: cc.build_repair_priority_base.min(1.0),
                target_entity: Some(build_entity),
                target_position: Some(*build_pos),
                blueprint: None,
                placement_failure_count: 0,
            });
        }
        if colony_injury_count > 0 {
            let priority = (colony_injury_count as f32 * cc.injury_priority_per_cat).min(1.0);
            self_queue.directives.push(Directive {
                kind: DirectiveKind::Herbcraft,
                priority,
                target_entity: None,
                target_position: None,
                blueprint: None,
                placement_failure_count: 0,
            });
        }
        self_queue.directives.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if !self_queue.directives.is_empty() {
            activation.record(Feature::DirectiveIssued);
        }
        // Note: we DO NOT thread event_log here because the
        // `event_log` Option was consumed above by the per-
        // coordinator loop. Colony-self directive emission is
        // observable via `DirectiveDelivered` (recorded when
        // `dispatch_urgent_directives` actually places an
        // `ActiveDirective` on a cat); the dispatch path is the
        // ground-truth observable for this code path.
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
    mut self_queue: ResMut<crate::components::coordination::ColonySelfDirectiveQueue>,
    colony_center: Res<crate::resources::ColonyCenter>,
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
                    if coord_pos.distance_to(p) <= cc.urgent_dispatch_range
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
                    coord_pos.distance_to(p) <= cc.urgent_dispatch_range
                        && !already_dispatched.contains(e)
                        && !(is_fight && *hunger < critical_hunger)
                })
                .max_by(|(_, pa, sa, _), (_, pb, sb, _)| {
                    let va = skill_of(sa) - coord_pos.distance_to(pa) * 0.01;
                    let vb = skill_of(sb) - coord_pos.distance_to(pb) * 0.01;
                    va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some((target_entity, _, _, _)) = best {
                commands.entity(*target_entity).insert(ActiveDirective {
                    kind: directive.kind,
                    priority: directive.priority,
                    coordinator: Some(coord_entity),
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

    // 487 — colony-self directive dispatch. Same shape as the per-
    // coordinator loop, with three substitutions: anchor position is
    // `colony_center` (no issuer cat to anchor on), the receiving cat
    // gets `coordinator: None`, and `coordinator_social_weight` reads
    // from `cc.colony_self_directive_weight` (softer than a real
    // coordinator's `Needs.respect`). No urgent-priority gate — every
    // colony-self directive is eligible for dispatch since the
    // founder phase needs the nudge even at moderate priority. Posse-
    // and-Fight slot-tracking is skipped because the colony-self
    // emission path doesn't produce Fight directives.
    if !self_queue.directives.is_empty() {
        let center_pos = colony_center.0;
        let cands: Vec<(Entity, Position, Skills, f32)> = candidates
            .iter()
            .map(|(e, p, s, n)| (e, *p, s.clone(), n.hunger))
            .collect();
        let mut dispatched_indices: Vec<usize> = Vec::new();
        let mut already_dispatched: Vec<Entity> = Vec::new();
        let mut urgent_slots_remaining: u32 = 1;
        for (idx, directive) in self_queue.directives.iter().enumerate() {
            if urgent_slots_remaining == 0 {
                break;
            }
            let skill_of = |s: &Skills| -> f32 {
                match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                    DirectiveKind::Cleanse => s.magic,
                    DirectiveKind::HarvestCarcass => s.herbcraft,
                    DirectiveKind::Cook => 0.0,
                }
            };
            let best = cands
                .iter()
                .filter(|(e, p, _, _)| {
                    center_pos.distance_to(p) <= cc.urgent_dispatch_range
                        && !already_dispatched.contains(e)
                })
                .max_by(|(_, pa, sa, _), (_, pb, sb, _)| {
                    let va = skill_of(sa) - center_pos.distance_to(pa) * 0.01;
                    let vb = skill_of(sb) - center_pos.distance_to(pb) * 0.01;
                    va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some((target_entity, _, _, hunger)) = best {
                // Day-1 critical-hunger gate: skip cats below the
                // starvation floor on a Forage directive — they
                // need to Eat from existing reserves first, not
                // chase outside food and starve mid-trip. Other
                // kinds (Build / Herbcraft) don't carry that
                // mortality risk.
                if matches!(directive.kind, DirectiveKind::Forage) && *hunger < critical_hunger {
                    continue;
                }
                commands.entity(*target_entity).insert(ActiveDirective {
                    kind: directive.kind,
                    priority: directive.priority,
                    coordinator: None,
                    coordinator_social_weight: cc.colony_self_directive_weight,
                    delivered_tick: time.tick,
                    target_position: directive.target_position,
                    target_entity: directive.target_entity,
                });
                activation.record(Feature::DirectiveDelivered);
                dispatched_indices.push(idx);
                already_dispatched.push(*target_entity);
                urgent_slots_remaining -= 1;
            }
        }
        for idx in dispatched_indices.into_iter().rev() {
            self_queue.directives.remove(idx);
        }
    }
}

// ---------------------------------------------------------------------------
// 487 — colony-alignment scoring
// ---------------------------------------------------------------------------

/// Whether an action counts as colony-aligned work for the
/// `ColonyAlignmentScore` EWMA. Read by `update_colony_alignment_scores`.
///
/// The set is deliberately the "visible work the colony needs done":
/// food economy (Forage / Hunt / Cook + the preservation chain),
/// infrastructure (Build / Craft / Farm), defence (Patrol / Fight),
/// herb chains (HerbcraftGather / HerbcraftRemedy / HerbcraftSetWard),
/// magic (MagicCleanse / MagicColonyCleanse / MagicHarvest /
/// MagicDurableWard / MagicScry), care (Mentor / Caretake / Bury), and
/// item management (PickUp / Drop / Trash / Handoff). Coordinate
/// itself counts so an in-flight coordinator's leadership self-
/// reinforces.
///
/// NOT aligned: self-maintenance (Eat / Sleep / GroomSelf / Idle /
/// Wander / Explore), interpersonal-but-not-work (Socialize /
/// GroomOther / Mate), threat-response (Flee / Hide), Stalk / Pounce
/// (sub-modes of Hunt; the parent Hunt counts), and the dormant
/// stubs (WearItem / PetitionCoordinator / Vigil / GriefSit /
/// ReleaseGrief / Wean / Teach / Release).
///
/// GroomOther excluded by design — that's the cuddle-pile action 487
/// fixes; cats who burn time on the puddle should NOT accumulate
/// election credit.
pub(crate) fn is_colony_aligned(action: crate::ai::Action) -> bool {
    use crate::ai::Action;
    matches!(
        action,
        Action::Forage
            | Action::Hunt
            | Action::Build
            | Action::Cook
            | Action::Farm
            | Action::Craft
            | Action::Coordinate
            | Action::Patrol
            | Action::Fight
            | Action::HerbcraftGather
            | Action::HerbcraftRemedy
            | Action::HerbcraftSetWard
            | Action::MagicScry
            | Action::MagicDurableWard
            | Action::MagicCleanse
            | Action::MagicColonyCleanse
            | Action::MagicHarvest
            | Action::MagicCommune
            | Action::Mentor
            | Action::Caretake
            | Action::Bury
            | Action::DryFood
            | Action::SmokeMeat
            | Action::TendSmokingRack
            | Action::PickUp
            | Action::Drop
            | Action::Trash
            | Action::Handoff
    )
}

/// 487 — per-tick decay + accumulate update for `ColonyAlignmentScore`.
///
/// Every tick, every cat's score decays by
/// `alignment_decay_per_tick`. If the cat's `CurrentAction` is a
/// colony-aligned action this tick, the score gains
/// `alignment_match_increment` after decay. The fixpoint for a cat
/// who spends every tick on aligned work is exactly 1.0 at the
/// default tuning; a cat who splits time 30/70 between aligned and
/// non-aligned settles at ≈0.3.
///
/// Cats without the component yet (newly spawned / save-loaded pre-
/// 487) are lazily inserted with an initial score reflecting this
/// tick's alignment — same cheap O(N_cats_missing) commands-buffer
/// pattern as `RecentTargetFailures` (ticket 073).
#[allow(clippy::type_complexity)]
pub fn update_colony_alignment_scores(
    mut commands: Commands,
    constants: Res<SimConstants>,
    mut cats_with: Query<(&crate::ai::CurrentAction, &mut ColonyAlignmentScore), Without<Dead>>,
    cats_without: Query<
        (Entity, &crate::ai::CurrentAction),
        (
            Without<ColonyAlignmentScore>,
            Without<Dead>,
            With<crate::components::identity::Species>,
        ),
    >,
) {
    let cc = &constants.coordination;
    for (current, mut score) in &mut cats_with {
        score.recent_aligned_actions *= cc.alignment_decay_per_tick;
        if is_colony_aligned(current.action) {
            score.recent_aligned_actions += cc.alignment_match_increment;
        }
    }
    for (entity, current) in &cats_without {
        let initial = if is_colony_aligned(current.action) {
            cc.alignment_match_increment
        } else {
            0.0
        };
        commands.entity(entity).insert(ColonyAlignmentScore {
            recent_aligned_actions: initial,
        });
    }
}

// ---------------------------------------------------------------------------
// flag_coordinator_death + flag_coordinator_incapacitated
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

/// 487 — if a Coordinator becomes Incapacitated, strip the role and
/// trigger immediate re-evaluation. An incapacitated cat literally
/// cannot perform `Action::Coordinate` (the `EligibilityFilter` on
/// every coordination-affordance DSE forbids `Incapacitated`), so a
/// Coordinator who's downed by a shadowfox ambush becomes a phantom
/// leader: their `DirectiveQueue` accumulates but never gets walked
/// to cats. Holding the role also blocks the colony-self path in
/// `assess_colony_needs` (which fires only when no Coordinator
/// exists), so the colony loses both the elected and the fallback
/// signal at the moment it most needs one. Dropping the marker
/// re-opens the election (`CoordinatorDied`-driven re-eval picks the
/// best able cat) and re-enables colony-self emissions until a real
/// successor takes over.
///
/// The existing `flag_coordinator_death` is the death equivalent;
/// shared `CoordinatorDied` resource because evaluate_coordinators
/// only needs the "vacate + re-elect" signal, not the cause.
/// Re-using avoids a parallel resource for the same response.
pub fn flag_coordinator_incapacitated(
    mut commands: Commands,
    query: Query<
        Entity,
        (
            With<crate::components::markers::Incapacitated>,
            With<Coordinator>,
        ),
    >,
) {
    let mut any = false;
    for entity in &query {
        commands.entity(entity).remove::<Coordinator>();
        commands.entity(entity).remove::<DirectiveQueue>();
        commands.entity(entity).remove::<BuildPressure>();
        commands.entity(entity).remove::<PendingDelivery>();
        any = true;
    }
    if any {
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
        // 487 — colony-self directives carry `coordinator: None`; they
        // expire purely on `delivered_tick + expiry_ticks` since there
        // is no issuer to check for liveness. Coordinator-issued
        // directives still drop when their issuer dies, preserving
        // the existing fast-path expiry for orphaned orders.
        let coordinator_dead = directive
            .coordinator
            .is_some_and(|c| alive_check.get(c).is_err());
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
    cats: Query<
        (
            &Position,
            &crate::ai::CurrentAction,
            &crate::components::magic::Inventory,
        ),
        Without<Dead>,
    >,
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
    // 084 Commit 3: read `ColonyThornbriarChronicallyLow` off the
    // `ColonyState` singleton. Replaces the per-tick `wards` + `herbs`
    // + `cat_inventories` queries that sourced the inline
    // `ward_strength_low && !wild_thornbriar_available &&
    // !any_cat_carrying_thornbriar` composition — those measured
    // transients (cats hold thornbriar for one weaving cycle, wild
    // patches respawn / get harvested) and carried no strategic signal
    // for an irreversible build decision. The chronicity marker
    // latches at window boundaries (`chronicity_window_ticks`) against
    // the stash level, which is the only steady-state-meaningful
    // surface for this gate.
    colony_state_q: Query<
        Has<crate::components::markers::ColonyThornbriarChronicallyLow>,
        With<crate::components::markers::ColonyState>,
    >,
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
        .filter(|(cat_pos, action, _inv)| {
            action.action == crate::ai::Action::Sleep
                && !buildings.iter().any(|(s, bpos)| {
                    s.kind == StructureType::Den && cat_pos.distance_to(&s.center(bpos)) <= 4.0
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
    // 084 Commit 3: chronic-low signal on the colony stash. Replaces
    // the prior `ward_strength_low && !wild_thornbriar_available &&
    // !any_cat_carrying_thornbriar` inline composition. The marker
    // already subsumes the strategic question ("is the colony out of
    // ward herbs over a sustained window?") via the chronicity tracker
    // — see `update_colony_building_markers` for the latch.
    let thornbriar_chronically_low = colony_state_q.iter().any(|has| has);
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

    // 369: hide items in Stores — the build-pressure signal for the
    // Tanning Frame channel. Same shape as `raw_food_items` (count
    // by item kind across all Stores aggregates). Hide accumulates
    // from prey kills via the 375 byproducts pipeline; without a
    // TanningFrame the hide piles up unused.
    //
    // 461: also count hides cats are carrying in inventory. Pre-463
    // (item-aspiration substrate), cats pick up Hide from prey-kill
    // deposit sites and try to craft warrior's-kit items at the
    // Workshop, failing with `CraftAtWorkshop: no workshop recipe
    // fully satisfied by inventory` (a generic-marker eligibility
    // pass + lex-pick resolver chooses recipes the cat can't satisfy)
    // and end up hoarding the Hide rather than depositing it to Stores.
    // The colony then never sees enough hide-in-Stores to fire the
    // TanningFrame BuildPressure channel even though the supply is
    // physically present. Counting across Stores ∪ Inventory is the
    // bridge signal; once 463 lands `CraftItemAspiration`, cats will
    // either craft hides immediately or release them to Stores, and
    // the inventory term should approach zero — at that point the
    // inventory-aware shape can revert to Stores-only if the welfare
    // metric prefers it.
    let hide_items_in_stores = stored_items_query
        .iter()
        .filter(|(s, _)| s.kind == StructureType::Stores)
        .map(|(_, si)| {
            si.items
                .iter()
                .copied()
                .filter(|&e| {
                    items_query
                        .get(e)
                        .is_ok_and(|it| it.kind == crate::components::items::ItemKind::Hide)
                })
                .count()
        })
        .sum::<usize>();
    let hide_items_in_inventories = cats
        .iter()
        .map(|(_, _, inv)| {
            inv.pouch
                .iter()
                .filter(|s| s.kind == crate::components::items::ItemKind::Hide)
                .count()
        })
        .sum::<usize>();
    let hide_items_anywhere = hide_items_in_stores + hide_items_in_inventories;

    let skilled_crafters = cats.iter().count(); // simplified: count living cats as proxy
                                                // Wildlife inside colony area (within ~wildlife_breach_range tiles of any building).
    let wildlife_breach = wildlife.iter().any(|wpos| {
        buildings
            .iter()
            .any(|(s, bpos)| wpos.distance_to(&s.center(bpos)) <= cc.wildlife_breach_range)
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
                placement_failure_count: 0,
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
                    placement_failure_count: 0,
                });
            }
        }

        // Farming pressure — gardens are multiuse (food crops + thornbriar
        // for wards), so accumulate when *either* demand axis fires. See
        // `should_accumulate_farming_pressure` for the truth-table contract.
        // 084 Commit 3: `herb_demand` is now the chronic-low marker
        // directly. The prior `ward_strength_low && !wild_thornbriar
        // && !carrying` composition retired with this commit — those
        // inputs measured single-tick transients and weren't
        // strategically meaningful for an irreversible build decision.
        let food_demand = food_fraction < cc.build_pressure_farming_food_threshold;
        let herb_demand = thornbriar_chronically_low;
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

        // 367 Commit 8 — preservation pressure (DryingRack + SmokingRack).
        //
        // Drying rack: signal is "the colony has raw food sitting in
        // stores while no Drying Rack exists." `raw_food_items`
        // includes fish + mammal/bird carcasses + organs. Drying
        // preserves fish + organs; the channel decays once a rack
        // exists. Matches the workshop pattern: gate on a structural
        // absence + a non-trivial economic signal.
        //
        // Smoking rack: independent channel — smoking needs fuel
        // (Wood) and tend cycles, so the colony only invests when raw
        // meat is accumulating. We gate the same `raw_food_items`
        // signal (smoking is a meat pipeline; the cooking-cutoff has
        // already established raw-food-in-stores as the economic
        // signal). A future iteration can split this into "raw meat
        // items" specifically if drying-vs-smoking elections need
        // diverging thresholds.
        let has_drying_rack = has_structure(StructureType::DryingRack);
        let has_smoking_rack = has_structure(StructureType::SmokingRack);
        if !has_drying_rack && raw_food_items >= cc.build_pressure_preservation_min_raw_food {
            pressure.drying_rack += rate * cc.preservation_pressure_multiplier;
        } else {
            pressure.drying_rack *= BuildPressure::DECAY;
        }
        if !has_smoking_rack && raw_food_items >= cc.build_pressure_preservation_min_raw_food {
            pressure.smoking_rack += rate * cc.preservation_pressure_multiplier;
        } else {
            pressure.smoking_rack *= BuildPressure::DECAY;
        }

        // 369 — Tanning Frame pressure. Signal: hide items in
        // Stores ≥ threshold AND no TanningFrame exists yet. Same
        // shape as preservation arms; tunable via
        // `SimConstants.crafting.build_pressure_tanning_min_hides`
        // and `.tanning_pressure_multiplier`.
        let has_tanning_frame = has_structure(StructureType::TanningFrame);
        if !has_tanning_frame && hide_items_anywhere >= cc.build_pressure_tanning_min_hides {
            pressure.tanning_frame += rate * cc.tanning_pressure_multiplier;
        } else {
            pressure.tanning_frame *= BuildPressure::DECAY;
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
                    placement_failure_count: 0,
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
                    // 367 Commit 8 — preservation channels reset on
                    // election to match cooking/workshop pattern.
                    StructureType::DryingRack => pressure.drying_rack = 0.0,
                    StructureType::SmokingRack => pressure.smoking_rack = 0.0,
                    // 369 — tanning frame channel resets on election.
                    StructureType::TanningFrame => pressure.tanning_frame = 0.0,
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
        StructureType::DryingRack => "drying rack",
        StructureType::SmokingRack => "smoking rack",
        StructureType::TanningFrame => "tanning frame",
    }
}

// ---------------------------------------------------------------------------
// Construction site spawning
// ---------------------------------------------------------------------------

/// Convert Build directives into physical ConstructionSite entities on the map.
///
/// **Real-world effect** — when a coordinator issues a Build directive
/// with a blueprint, this system finds a valid placement via
/// `compute_building_placement` (382) and spawns a `ConstructionSite`
/// entity. For founding buildings, materials are pre-delivered.
///
/// **Placement failure** — if `compute_building_placement` returns
/// `None`, the directive's `placement_failure_count` increments. At
/// `placement_stuck_narrate_threshold_ticks` the system emits a
/// "looks for a spot for the new …" narration and
/// `Feature::DirectiveStuckOnPlacement`, then resets the counter so a
/// chronic-stuck directive re-emits each window.
///
/// **Witness** — emits `Feature::BuildingConstructed`-adjacent
/// `Feature::DirectiveDelivered` (already wired upstream by
/// `process_directives`) on successful spawn; the construction site is
/// the persistent witness.
///
/// **Feature emission** — `Feature::DirectiveStuckOnPlacement`
/// (regression canary, `expected_to_fire_per_soak() => false`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn spawn_construction_sites(
    mut commands: Commands,
    mut coordinators: Query<(Entity, &mut DirectiveQueue, &Name), With<Coordinator>>,
    buildings: Query<(&crate::components::building::Structure, &Position)>,
    construction_sites: Query<&crate::components::building::ConstructionSite>,
    colony_center: Res<crate::resources::ColonyCenter>,
    mut map: ResMut<crate::resources::map::TileMap>,
    mut cover_map: ResMut<crate::resources::CoverAvailabilityMap>,
    district: Res<crate::resources::ColonyDistrictMap>,
    fox_corridor: Res<crate::resources::FoxApproachCorridorMap>,
    food_location: Res<crate::resources::FoodLocationMap>,
    garden_location: Res<crate::resources::GardenLocationMap>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
) {
    use crate::resources::sim_constants::BuildingPlacementSemantics;

    // Track blueprints spawned this tick to prevent duplicates from multiple
    // coordinators issuing the same directive (commands are deferred, so
    // construction_sites won't see entities spawned earlier in this loop).
    let mut spawned_this_tick = std::collections::HashSet::new();

    // Snapshot building positions once per system call — every coordinator
    // sees the same colony state and per-kind nearest-neighbor query.
    let building_positions: Vec<(Position, (i32, i32), StructureType)> = buildings
        .iter()
        .map(|(s, p)| (*p, s.size, s.kind))
        .collect();
    let occupied: Vec<(Position, (i32, i32))> = building_positions
        .iter()
        .map(|(p, sz, _)| (*p, *sz))
        .collect();
    let stuck_threshold = constants
        .scoring
        .placement_stuck_narrate_threshold_ticks
        .max(1);

    for (coord_entity, mut queue, coordinator_name) in &mut coordinators {
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
        let already_built = building_positions.iter().any(|(_, _, k)| *k == blueprint);
        if already_exists || already_built {
            queue.directives.remove(idx);
            continue;
        }

        let size = blueprint.default_size();
        let center = colony_center.0;

        // 382: collect same-kind building positions for the
        // proximity-clustering term in `compute_building_placement`.
        let buildings_of_kind: Vec<Position> = building_positions
            .iter()
            .filter(|(_, _, k)| *k == blueprint)
            .map(|(p, _, _)| *p)
            .collect();

        let placement = match constants.scoring.building_placement_semantics {
            BuildingPlacementSemantics::Spiral => {
                find_building_placement_spiral(&map, center, size, &occupied)
            }
            BuildingPlacementSemantics::InfluenceMap => {
                // 382: deterministic per-call RNG seeded by tick +
                // coordinator entity. Same pattern as
                // `assess_colony_needs` (`coordination.rs:509`) — avoids
                // threading a shared SimRng through Bevy's 16-param tuple.
                let seed = time.tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ coord_entity.to_bits();
                let mut local_rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
                compute_building_placement(
                    blueprint,
                    size,
                    center,
                    &occupied,
                    &buildings_of_kind,
                    &district,
                    &fox_corridor,
                    &food_location,
                    &garden_location,
                    &map,
                    &constants,
                    &mut local_rng,
                )
            }
        };
        let Some(anchor) = placement else {
            // 382 Phase C: stuck-directive observability. Increment
            // failure counter; at threshold, narrate + emit feature
            // once, then reset so a still-stuck directive re-fires next
            // window rather than spamming every tick.
            let count = &mut queue.directives[idx].placement_failure_count;
            *count = count.saturating_add(1);
            if *count >= stuck_threshold {
                log.push(
                    time.tick,
                    format!(
                        "{} looks for a spot for the new {} but the colony has grown too crowded — the plan sits in the back of her mind.",
                        coordinator_name.0,
                        structure_display_name(blueprint),
                    ),
                    NarrativeTier::Action,
                );
                activation.record(Feature::DirectiveStuckOnPlacement);
                *count = 0;
            }
            continue;
        };

        // Stamp terrain footprint.
        let terrain = blueprint.terrain();
        for dy in 0..size.1 {
            for dx in 0..size.0 {
                let x = anchor.x() + dx;
                let y = anchor.y() + dy;
                if map.in_bounds(x, y) {
                    map.set(x, y, terrain);
                }
            }
        }
        // Ticket 423: invalidate the cover-availability map so the next
        // `update_cover_availability_map` tick re-stamps the new building's
        // low-cover footprint (Den / Hearth / Stores / Workshop /
        // Watchtower are all `is_low_cover()`).
        cover_map.mark_dirty();

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
        // 382: positive observability — pairs with
        // `DirectiveStuckOnPlacement`. Healthy seed-42 soaks issue ~6
        // Build directives over 15 min, so this fires at least once.
        activation.record(Feature::ConstructionSiteSpawned);

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

/// Pre-382 spiral search outward from `center` to find a position
/// where a building of `size` fits with all tiles passable and at
/// least 1 tile gap from existing buildings. Retained behind
/// `BuildingPlacementSemantics::Spiral` as an emergency-revert
/// fixture; the default semantics (`InfluenceMap`) routes through
/// `compute_building_placement` instead.
fn find_building_placement_spiral(
    map: &crate::resources::map::TileMap,
    center: Position,
    size: (i32, i32),
    occupied: &[(Position, (i32, i32))],
) -> Option<Position> {
    for radius in 1..=16_i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() + dy.abs() != radius {
                    continue;
                }
                let anchor = Position::new(center.x() + dx, center.y() + dy);
                if footprint_valid(map, anchor, size, occupied) {
                    return Some(anchor);
                }
            }
        }
    }
    None
}

/// 382: per-kind affinity weights derived from the universal
/// `ScoringConstants` knobs. Computed once per placement call to keep
/// the inner candidate loop tight.
#[derive(Debug, Clone, Copy)]
struct KindAffinity {
    /// Multiplier on the `threat` term. Negative for non-defensive
    /// kinds (threat suppresses); positive for `Watchtower` / `WardPost`
    /// (threat attracts).
    threat_sign: f32,
    /// Multiplier on the `frontier` term. Negative for `Midden` (refuse
    /// pile wants the periphery).
    frontier_sign: f32,
    food_proximity_weight: f32,
    garden_terrain_weight: f32,
    defensive_corridor_weight: f32,
    same_kind_proximity_weight: f32,
}

fn kind_affinity(kind: StructureType, c: &SimConstants) -> KindAffinity {
    use StructureType::*;
    let s = &c.scoring;
    let (threat_sign, frontier_sign, food, garden, defensive, same_kind) = match kind {
        // 367: Drying Rack + Smoking Rack are preservation stations —
        // food-craft adjacents that share the placement profile of the
        // Kitchen / Workshop / Stores cluster (want proximity to food
        // stockpile, want same-kind clustering so a colony can run
        // multiple racks side-by-side). 369: Tanning Frame inherits
        // the same craft-cluster placement profile (the cat needs to
        // shuttle hides between Stores and the frame).
        Stores | Kitchen | Workshop | DryingRack | SmokingRack | TanningFrame => (
            -1.0,
            1.0,
            s.building_placement_food_proximity_weight,
            0.0,
            0.0,
            s.building_placement_same_kind_proximity_weight,
        ),
        Garden => (
            -1.0,
            1.0,
            0.0,
            s.building_placement_garden_terrain_weight,
            0.0,
            0.0,
        ),
        Watchtower | WardPost => (
            1.0,
            1.0,
            0.0,
            0.0,
            s.building_placement_defensive_corridor_weight,
            0.0,
        ),
        Midden => (
            -1.0,
            -s.building_placement_midden_periphery_weight,
            0.0,
            0.0,
            0.0,
            0.0,
        ),
        Den => (
            -1.0,
            1.0,
            0.0,
            0.0,
            0.0,
            -s.building_placement_same_kind_proximity_weight,
        ),
        Hearth | Wall | Gate => (-1.0, 1.0, 0.0, 0.0, 0.0, 0.0),
    };
    KindAffinity {
        threat_sign,
        frontier_sign,
        food_proximity_weight: food,
        garden_terrain_weight: garden,
        defensive_corridor_weight: defensive,
        same_kind_proximity_weight: same_kind,
    }
}

/// Score the same-kind-proximity term for one candidate. Returns the
/// lift in `[0, 1]` derived from the nearest building of matching
/// kind — `1.0` when a same-kind building sits right next to the
/// candidate, falling linearly to `0.0` at
/// `same_kind_proximity_range`.
fn same_kind_proximity_lift(
    candidate: Position,
    kind: StructureType,
    buildings_of_kind: &[Position],
    range: f32,
) -> f32 {
    if buildings_of_kind.is_empty() || range <= 0.0 {
        return 0.0;
    }
    let mut best_dist = f32::MAX;
    for p in buildings_of_kind {
        let d = candidate.distance_to(p);
        if d < best_dist {
            best_dist = d;
        }
    }
    let _ = kind; // kind is captured by the caller's affinity table
    if best_dist >= range {
        0.0
    } else {
        1.0 - (best_dist / range)
    }
}

/// 382: pick an anchor position for a new building of `kind` via an
/// argmax over `ColonyDistrictMap` plus per-kind affinity lifts.
/// Replaces the radius-16 spiral search in
/// `find_building_placement_spiral`. Returns `None` when no candidate
/// scores above `building_placement_score_floor` — the caller defers
/// the directive and increments its stuck-counter.
///
/// Candidate generation: coarse grid across the whole map at step
/// `building_placement_candidate_step` (default 5, matching the
/// 5-tile influence-map bucket size). Every candidate is gated by
/// `footprint_valid` (preserving the 1-tile-gap rule and passability
/// check).
///
/// Determinism: per-call RNG seeded by tick + coordinator entity,
/// jitter `[0, building_placement_jitter_range)` per candidate for
/// deterministic tiebreak.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_building_placement(
    kind: StructureType,
    size: (i32, i32),
    anchor: Position,
    occupied: &[(Position, (i32, i32))],
    buildings_of_kind: &[Position],
    district: &crate::resources::ColonyDistrictMap,
    fox_corridor: &crate::resources::FoxApproachCorridorMap,
    food_location: &crate::resources::FoodLocationMap,
    garden_location: &crate::resources::GardenLocationMap,
    tile_map: &crate::resources::map::TileMap,
    constants: &SimConstants,
    rng: &mut impl rand::Rng,
) -> Option<Position> {
    let s = &constants.scoring;
    let aff = kind_affinity(kind, constants);
    let step = s.building_placement_candidate_step.max(1) as usize;
    let map_w = tile_map.width;
    let map_h = tile_map.height;
    let dist_cost = s.building_placement_distance_cost_per_tile;
    let jitter_range = s.building_placement_jitter_range.max(0.0);

    let mut best: Option<(Position, f32)> = None;

    for cy in (0..map_h).step_by(step) {
        for cx in (0..map_w).step_by(step) {
            let candidate = Position::new(cx, cy);
            if !footprint_valid(tile_map, candidate, size, occupied) {
                continue;
            }
            let frontier = district.frontier_at(cx, cy);
            let crowding = district.crowding_at(cx, cy);
            let threat = district.threat_at(cx, cy);

            let district_score =
                s.building_placement_frontier_weight * aff.frontier_sign * frontier
                    - s.building_placement_crowding_weight * crowding
                    + s.building_placement_threat_weight * aff.threat_sign * threat;

            let food_lift = if aff.food_proximity_weight > 0.0 {
                aff.food_proximity_weight * food_location.get(cx, cy)
            } else {
                0.0
            };

            let garden_lift = if aff.garden_terrain_weight > 0.0 {
                let terrain_bonus = if tile_map.in_bounds(cx, cy) {
                    use crate::resources::map::Terrain;
                    let t = tile_map.get(cx, cy).terrain;
                    // Fertile classes: Grass and Garden footprints
                    // (re-stamped over Grass during construction); other
                    // classes get no lift. LightForest/DenseForest are
                    // edible-leaf rich but cats want open ground for
                    // tilling, so they're explicitly excluded.
                    match t {
                        Terrain::Grass | Terrain::Garden => 1.0,
                        _ => 0.0,
                    }
                } else {
                    0.0
                };
                aff.garden_terrain_weight
                    * (garden_location.get(cx, cy) * 0.5 + terrain_bonus * 0.5)
            } else {
                0.0
            };

            let defensive_lift = if aff.defensive_corridor_weight > 0.0 {
                aff.defensive_corridor_weight * fox_corridor.get(cx, cy)
            } else {
                0.0
            };

            let same_kind_lift = if aff.same_kind_proximity_weight != 0.0 {
                aff.same_kind_proximity_weight
                    * same_kind_proximity_lift(
                        candidate,
                        kind,
                        buildings_of_kind,
                        s.building_placement_same_kind_proximity_range,
                    )
            } else {
                0.0
            };

            let dist = anchor.distance_to(&candidate);
            let distance_cost = dist_cost * dist;

            let jitter = if jitter_range > 0.0 {
                rng.random_range(0.0..jitter_range)
            } else {
                0.0
            };

            let score = district_score + food_lift + garden_lift + defensive_lift + same_kind_lift
                - distance_cost
                + jitter;

            match best {
                Some((_, b)) if score <= b => {}
                _ => {
                    best = Some((candidate, score));
                }
            }
        }
    }

    match best {
        Some((pos, score)) if score >= s.building_placement_score_floor => Some(pos),
        _ => None,
    }
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
            let x = anchor.x() + dx;
            let y = anchor.y() + dy;
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
    let a_left = a_pos.x() - gap;
    let a_right = a_pos.x() + a_size.0 + gap;
    let a_top = a_pos.y() - gap;
    let a_bottom = a_pos.y() + a_size.1 + gap;

    let b_left = b_pos.x();
    let b_right = b_pos.x() + b_size.0;
    let b_top = b_pos.y();
    let b_bottom = b_pos.y() + b_size.1;

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
    pub cat_scent: &'a crate::resources::CatScentMap,
    pub ward_coverage: &'a crate::resources::WardCoverageMap,
    pub tile_map: &'a crate::resources::map::TileMap,
    /// 220: ambush-event memory (substrate from ticket 219). Sampled at
    /// each candidate tile in `compute_ward_placement` and lifted into
    /// the threat term, gated by `ward_ambush_anchor_weight`.
    pub recent_ambush: &'a crate::resources::RecentAmbushMap,
    /// 220: kill-site scent (Phase 2C substrate). Same dormant-at-land
    /// posture as `recent_ambush`; gated by `ward_recency_anchor_weight`.
    pub carcass_scent: &'a crate::resources::CarcassScentMap,
    /// 312: fox-approach corridor traffic. Populated by
    /// `update_fox_approach_corridor_map` each tick from
    /// `FoxAiPhase::PatrolTerritory` deposits. Sampled in
    /// `compute_ward_placement` to produce a multiplicative-outside
    /// lift that escapes the 297 iter-2 saturation ceiling. Gated by
    /// `ward_fox_approach_corridor_weight` (default 0.0, dormant at
    /// land).
    pub fox_approach_corridor: &'a crate::resources::FoxApproachCorridorMap,
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
/// - `unaddressed_threat = (max(fox_scent, corruption) + ambush_lift +
///   carcass_lift - ward_coverage).clamp(0, 1)`. High = SFs walked
///   here recently OR corruption is creeping OR (when weights are
///   tuned up) ambush events / kill-site scent are concentrated here,
///   AND existing wards aren't already covering the tile.
/// - `ambush_lift = w_ambush × logistic_8_05(recent_ambush)` — sigmoid
///   lift gated by `ward_ambush_anchor_weight` (ticket 220, default
///   0.0). At 0.0 the lift is exactly zero and the formula is
///   byte-identical to the pre-220 baseline.
/// - `carcass_lift = w_carcass × logistic_8_05(carcass_scent)` — same
///   shape over the kill-site map, gated by `ward_recency_anchor_weight`
///   (restores 209 §Scope line 74, also dormant by default).
/// - `fox_intercept_lift = w_fox_intercept × logistic_k_m(fox_spawn_vicinity)`
///   — 297's third anchor (predictive, not echo): the
///   the helper scans `TileMap.corruption` in a Manhattan-radius
///   neighborhood and lights up tiles near every high-corruption
///   spawn-eligible tile (a halo around fox-spawn sources), so this
///   lift biases placement toward the regions fox patrols traverse on
///   their way to the colony. Curve params are shared with
///   ambush/carcass (post-296 these are
///   `SimConstants::scoring::ward_placement_logistic_*`). Gated by
///   `ward_fox_intercept_anchor_weight` (default 0.0 — dormant at land).
///   Inline computation (not a populated Resource) avoids a
///   schedule-edge perturbation of seed-42 (ticket 061 precedent at
///   `simulation.rs:314-326`).
/// - `cat_value = cat_scent` — modest bonus for tiles where cats
///   actually live (a ward covering nobody is wasted).
/// - `distance_cost = DIST_PENALTY_PER_TILE × manhattan(anchor, candidate)`
///   — soft travel-cost term so the priestess doesn't walk to the
///   opposite map corner for a marginal score gain. At 0.005/tile a
///   100-tile detour subtracts 0.5 from the score, so a fully-saturated
///   threat tile far away still beats a half-strength threat nearby
///   only by a meaningful margin.
/// - `score = unaddressed_threat + W_cat × cat_value − distance_cost`
///   plus small jitter for tie-breaking. `W_cat` is gated by
///   `ward_placement_cat_value_weight` (ticket 298, default 0.3 —
///   preserves byte-identical pre-298 behavior).
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
    constants: &SimConstants,
    rng: &mut impl rand::Rng,
    // 301: when present *and* semantics is `DescendingResidual`, the
    // function stamps each of the K round picks into this map (with
    // a per-wake decay applied first). `None` is the test-harness and
    // pre-301 path — no stamping occurs. Under `SingleShotArgmax`
    // (the default) this is a no-op regardless of `Some`/`None`, so
    // dormant runs are byte-identical with or without an intent map.
    intent_map: Option<&mut crate::resources::WardIntentMap>,
) -> Position {
    let anchor = if building_positions.is_empty() {
        colony_center
    } else {
        let (sx, sy) = building_positions.iter().fold((0i64, 0i64), |(ax, ay), p| {
            (ax + p.x() as i64, ay + p.y() as i64)
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

    // Coarse-grid candidate generation across the whole map. The stride
    // is `constants.scoring.ward_placement_candidate_step` (default 5,
    // promoted from a hardcoded constant in 300). At default the search
    // grid aligns with the influence-map bucket size; finer strides
    // resolve sub-bucket variation from the distance-to-anchor penalty
    // only, since the influence maps return per-bucket values without
    // interpolation. For the default 120×90 map and step=5 this yields
    // ~430 candidates; cheap to score.
    let candidate_step = constants.scoring.ward_placement_candidate_step.max(1) as usize;
    const HARD_EXCLUDE_DISTANCE: f32 = 3.0;
    /// Travel-cost penalty per Manhattan tile from the anchor. Tuned so
    /// a 100-tile detour costs 0.5 score — a saturated threat far away
    /// still beats a half-saturated threat nearby, but only by a real
    /// margin. Picked dimensionlessly against the [0, 1] threat axis;
    /// no balance constant needed.
    const DIST_PENALTY_PER_TILE: f32 = 0.005;
    let map_w = maps.tile_map.width;
    let map_h = maps.tile_map.height;
    let mut candidates: Vec<Position> = Vec::new();
    for cy in (0..map_h).step_by(candidate_step) {
        for cx in (0..map_w).step_by(candidate_step) {
            let candidate = Position::new(cx, cy);
            if ward_positions
                .iter()
                .any(|(wp, _)| candidate.distance_to(wp) <= HARD_EXCLUDE_DISTANCE)
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

    // 220: dormancy invariant — both weights default to 0.0, so the
    // lifts evaluate to exactly 0.0 and the formula reduces to the
    // pre-220 baseline. Clamp defensively in case a balance experiment
    // overshoots [0, 1]. 284 lifted ambush/carcass off dormancy to
    // (0.5, 0.3); 297 adds the fox-intercept weight (default 0.0,
    // dormant at land).
    let w_ambush = constants.scoring.ward_ambush_anchor_weight.clamp(0.0, 1.0);
    let w_carcass = constants.scoring.ward_recency_anchor_weight.clamp(0.0, 1.0);
    let w_fox_intercept = constants
        .scoring
        .ward_fox_intercept_anchor_weight
        .clamp(0.0, 1.0);
    // 312: corridor weight composes **multiplicatively outside** the
    // saturating threat sum (see CandidateScore::score). Not clamped
    // to [0, 1] — the multiplier `(1 + w * L)` is intentionally
    // allowed to climb above 2.0 when both `w` and the corridor
    // sample saturate, which is the escape-the-ceiling property that
    // distinguishes this lift from the inside-the-sum siblings.
    // (Negative values would invert the threat axis; guard at zero.)
    let w_corridor = constants.scoring.ward_fox_approach_corridor_weight.max(0.0);
    // 298: cat_value tiebreak coefficient, promoted from hardcoded 0.3.
    // Default preserves byte-identical pre-298 behavior. Not clamped —
    // it's a scalar coefficient on a [0, 1] presence value, not a
    // weight whose saturation needs disciplining.
    //
    // 313: bundled with the composition flag + gate floor into
    // `CatValueParams`. Under `Additive` (default) `weight` is the
    // additive coefficient and `gate_floor` is unused. Under `Gate`
    // the weight is unused and `gate_floor` is the saturating-ramp
    // knee point.
    let cat = CatValueParams {
        weight: constants.scoring.ward_placement_cat_value_weight,
        composition: constants.scoring.ward_placement_cat_value_composition,
        gate_floor: constants.scoring.ward_placement_cat_value_gate_floor,
    };
    // 296: Logistic curve params, promoted from hardcoded (8.0, 0.5).
    // Defaults preserve byte-identical pre-296 behavior.
    let curve_k = constants.scoring.ward_placement_logistic_steepness;
    let curve_m = constants.scoring.ward_placement_logistic_midpoint;
    // 297: kernel constants for the inline fox-spawn-vicinity computation.
    // Reads `TileMap.corruption` directly per candidate, not via a
    // populated Resource — adding a per-tick populator to the wildlife
    // chain perturbs seed-42 (ticket 061 precedent: `simulation.rs:314-326`).
    let fox_intercept_radius_tiles = constants.scoring.fox_intercept_kernel_radius_tiles as i32;
    let corruption_threshold = constants.magic.shadow_fox_corruption_threshold;

    // 301: materialize per-candidate scoring data into a Vec instead of
    // tracking the argmax inline. The scoring loop's RNG consumption
    // (one `random_range` draw per candidate, in candidate order) and
    // the score arithmetic are unchanged — `SingleShotArgmax` walks
    // `scored` once and picks the max-score candidate, reproducing
    // pre-301 behavior bit-for-bit. `DescendingResidual` re-uses the
    // cached scoring terms across K rounds, stamping virtual coverage
    // around each round's pick so successive rounds re-score against a
    // partially-eaten threat surface.
    let mut scored: Vec<CandidateScore> = Vec::with_capacity(candidates.len());

    for candidate in &candidates {
        let fox_scent = maps.fox_scent.get(candidate.x(), candidate.y());
        let corruption = maps.corruption_at(candidate.x(), candidate.y());
        let coverage = maps.ward_coverage.get(candidate.x(), candidate.y());
        let cat_value = maps.cat_scent.get(candidate.x(), candidate.y());

        // 220 lift terms. Skip the sigmoid evaluation entirely when the
        // weight is zero so dormant runs incur no extra arithmetic.
        let ambush_lift = if w_ambush > 0.0 {
            w_ambush
                * logistic_threat_lift(
                    maps.recent_ambush.get(candidate.x(), candidate.y()),
                    curve_k,
                    curve_m,
                )
        } else {
            0.0
        };
        let carcass_lift = if w_carcass > 0.0 {
            w_carcass
                * logistic_threat_lift(
                    maps.carcass_scent.get(candidate.x(), candidate.y()),
                    curve_k,
                    curve_m,
                )
        } else {
            0.0
        };
        // 297: fox-spawn-vicinity lift. Predictive (not echo) substrate:
        // the halo lights up tiles NEAR corruption sources (uncorrupted
        // neighbors), not corruption tiles themselves — exactly the
        // regions where `fox_scent.max(corruption)` is LOW but fox
        // patrols traverse. Composes with `+ w_cat_value * cat_value` to peak
        // placement at the cats↔corruption boundary. Computed inline
        // from `TileMap.corruption` to avoid the populator-system
        // schedule-edge perturbation (ticket 061 precedent).
        let fox_intercept_lift = if w_fox_intercept > 0.0 {
            let vicinity = compute_fox_spawn_vicinity(
                *candidate,
                maps.tile_map,
                fox_intercept_radius_tiles,
                corruption_threshold,
            );
            w_fox_intercept * logistic_threat_lift(vicinity, curve_k, curve_m)
        } else {
            0.0
        };

        let threat =
            (fox_scent.max(corruption) + ambush_lift + carcass_lift + fox_intercept_lift).min(1.0);

        // 312: corridor traffic, sampled per candidate and stored as a
        // pre-computed multiplier on `unaddressed_threat`. Short-circuit
        // the sigmoid when the weight is zero so dormant runs do exactly
        // zero extra arithmetic (matches the 297 inline-lift pattern).
        let corridor_lift = if w_corridor > 0.0 {
            w_corridor
                * logistic_threat_lift(
                    maps.fox_approach_corridor.get(candidate.x(), candidate.y()),
                    curve_k,
                    curve_m,
                )
        } else {
            0.0
        };

        let dist = anchor.distance_to(candidate);
        let distance_cost = DIST_PENALTY_PER_TILE * dist;

        // Small jitter ([0, 0.05)) breaks ties deterministically without
        // overwhelming the influence-map signal.
        let jitter = rng.random_range(0.0_f32..0.05);

        scored.push(CandidateScore {
            pos: *candidate,
            threat,
            real_coverage: coverage,
            cat_value,
            distance_cost,
            jitter,
            corridor_lift,
        });
    }

    match constants.scoring.ward_placement_semantics {
        WardPlacementSemantics::SingleShotArgmax => {
            // Default flag — never touches `intent_map` so the
            // resource remains all-zeros and byte-identity vs pre-301
            // holds whether or not the caller passed `Some(&mut …)`.
            select_argmax(&scored, cat, candidates[0])
        }
        WardPlacementSemantics::DescendingResidual => {
            let k = constants.scoring.ward_placement_residual_rounds.max(1) as usize;
            let (pick, round_picks) = select_descending_residual(&scored, cat, k, candidates[0]);
            if let Some(intent) = intent_map {
                // Decay first so stale intent fades before the new
                // round picks land their fresh stamps.
                intent.decay_all(constants.scoring.ward_intent_decay_per_wake);
                /// Radius used when stamping intent under
                /// `DescendingResidual`. Wider than
                /// `THORNWARD_VIRTUAL_RADIUS` (the in-function virtual
                /// coverage radius) because intent should bias Path B
                /// cats who *walk past* the picked tile, not just cats
                /// already standing on it. Picked dimensionlessly
                /// against the influence-map bucket size (5 tiles) —
                /// two buckets of influence in every direction.
                const INTENT_STAMP_RADIUS: f32 = 10.0;
                const INTENT_STAMP_STRENGTH: f32 = 1.0;
                for round_pick in &round_picks {
                    intent.stamp_intent(
                        round_pick.x(),
                        round_pick.y(),
                        INTENT_STAMP_STRENGTH,
                        INTENT_STAMP_RADIUS,
                    );
                }
            }
            pick
        }
    }
}

/// 301: per-candidate scoring terms cached after the single scoring
/// loop in [`compute_ward_placement`]. Storing the terms (rather than
/// the final score) lets [`select_descending_residual`] re-score the
/// same candidate against an updated `virtual_coverage` across K rounds
/// without re-drawing the jitter or re-sampling influence maps. The
/// `jitter` is drawn once per candidate so each round's selection is
/// deterministic given the same start state — the only round-to-round
/// input that changes is the virtual coverage stamp.
#[derive(Debug, Clone, Copy)]
struct CandidateScore {
    pos: Position,
    /// `(fox_scent.max(corruption) + lifts).min(1.0)` — pre-coverage.
    threat: f32,
    /// `maps.ward_coverage.get(pos)` — real coverage from already-
    /// placed wards. Stamped virtual coverage (descending-residual)
    /// adds on top of this at score time.
    real_coverage: f32,
    /// `maps.cat_scent.get(pos)`.
    cat_value: f32,
    /// `DIST_PENALTY_PER_TILE * manhattan(anchor, pos)`.
    distance_cost: f32,
    /// `rng.random_range(0.0..0.05)` — drawn once per candidate.
    jitter: f32,
    /// 312: `w_corridor * L(corridor_sample)` — the
    /// multiplicative-outside lift applied as
    /// `unaddressed_threat * (1 + corridor_lift)` in `score()`.
    /// Zero at default (`ward_fox_approach_corridor_weight = 0.0`)
    /// AND when the candidate sits on an unvisited tile, in either
    /// case preserving byte-identical pre-312 behavior.
    corridor_lift: f32,
}

/// 313: bundled cat_value composition parameters threaded through
/// the score-formula callsites. `weight` is the pre-313 additive
/// coefficient (`ward_placement_cat_value_weight`); `composition`
/// selects between the additive reward and the saturating-ramp
/// gate; `gate_floor` is the gate's knee point. At
/// `composition == Additive` (the default) `gate_floor` is unused
/// and the formula is bit-identical to pre-313.
#[derive(Debug, Clone, Copy)]
struct CatValueParams {
    weight: f32,
    composition: WardPlacementCatValueComposition,
    gate_floor: f32,
}

impl CandidateScore {
    /// Score formula identical to pre-301's inline expression, with
    /// `virtual_coverage` summed into `real_coverage` before the
    /// `(threat - coverage).clamp(0.0, 1.0)` step. At
    /// `virtual_coverage == 0.0` AND `corridor_lift == 0.0` AND
    /// `cat.composition == Additive` this is bit-identical to
    /// pre-313 (and to pre-301 when the corridor lift is also zero).
    ///
    /// 312: the `(1.0 + corridor_lift)` factor scales
    /// `unaddressed_threat` multiplicatively OUTSIDE the saturating
    /// `(threat - coverage).clamp(0, 1)` step. This is the
    /// architectural escape from the 297 iter-2 rank-preservation
    /// ceiling: once any inside-the-sum threat input saturates,
    /// additional inside-the-sum lifts are rank-preserving for the
    /// argmax. Multiplying outside lets a high-corridor tile score
    /// *above* 1.0 on the threat axis, breaking the ceiling on the
    /// specific tiles that earn the topological-criticality lift.
    /// See `docs/balance/297-fox-patrol-topology-axis.md` iter-2.
    ///
    /// 313: when `cat.composition == Gate`, the additive
    /// `+ weight * cat_value` reward is replaced with a
    /// multiplicative saturating-ramp gate on the threat-merit
    /// term: `gate(cat_value) = (cat_value / gate_floor).clamp(0, 1)`.
    /// A dead tile (`cat_value = 0`) yields gate 0 and zeroes the
    /// merit, suppressing placement. Any `cat_value >= gate_floor`
    /// yields gate 1 and full merit — there's no reward for
    /// density peaks beyond reachability. `distance_cost` and
    /// `jitter` remain additive so distance still penalizes
    /// regardless of cat density and jitter still tiebreaks on
    /// dead tiles. See `docs/balance/301-ward-placement-decision-semantics.md`
    /// iter-3.
    fn score(&self, virtual_coverage: f32, cat: CatValueParams) -> f32 {
        let effective_coverage = self.real_coverage + virtual_coverage;
        let unaddressed_threat = (self.threat - effective_coverage).clamp(0.0, 1.0);
        let threat_merit = unaddressed_threat * (1.0 + self.corridor_lift);
        match cat.composition {
            WardPlacementCatValueComposition::Additive => {
                threat_merit + cat.weight * self.cat_value - self.distance_cost + self.jitter
            }
            WardPlacementCatValueComposition::Gate => {
                // Saturating ramp with knee at `gate_floor`. A
                // non-positive floor would divide by zero / invert
                // the ramp; guard at a small epsilon so the gate
                // collapses to a step at cat_value > 0 in the
                // degenerate case.
                let floor = cat.gate_floor.max(f32::EPSILON);
                let gate = (self.cat_value / floor).clamp(0.0, 1.0);
                threat_merit * gate - self.distance_cost + self.jitter
            }
        }
    }
}

/// 301: argmax over `scored` at zero virtual coverage. Reproduces
/// pre-301 single-shot selection bit-for-bit: the `f32` score
/// recomputed from cached terms equals the inline score that the
/// original loop tracked, and the iteration order matches the original
/// loop, so the `if score > best_score` comparison produces the same
/// updates.
fn select_argmax(scored: &[CandidateScore], cat: CatValueParams, fallback: Position) -> Position {
    let mut best_pos = fallback;
    let mut best_score = f32::NEG_INFINITY;
    for cs in scored {
        let score = cs.score(0.0, cat);
        if score > best_score {
            best_score = score;
            best_pos = cs.pos;
        }
    }
    best_pos
}

/// 301 SPLIT option: K rounds of submodular greedy. Round 0 picks the
/// argmax winner (identical to [`select_argmax`]); the winner stamps
/// virtual coverage around itself with thornward-radius falloff, then
/// round 1 re-scores all candidates against the updated virtual
/// coverage. Repeats for K rounds.
///
/// Returns `(final_pick, all_round_picks)` — the round-(K-1) pick
/// (the most-diversified one, returned as the coordinator's directive
/// target) and every round's pick in order, so the caller can stamp
/// intent for every round into `WardIntentMap`.
///
/// `THORNWARD_VIRTUAL_RADIUS` matches `Ward::thornward().repel_radius()`
/// — the coordinator's `SetWard` directive plants a thornward at the
/// chosen tile, so the virtual stamp approximates what its real
/// coverage will look like once the cat materializes the placement.
fn select_descending_residual(
    scored: &[CandidateScore],
    cat: CatValueParams,
    k: usize,
    fallback: Position,
) -> (Position, Vec<Position>) {
    /// Matches `Ward::thornward().repel_radius()` —
    /// `WardKind::Thornward` base `6.0` × strength `1.0`. The
    /// coordinator's `SetWard` directive plants thornwards by default
    /// (`resolve_set_ward` in `src/steps/magic/set_ward.rs`), so the
    /// virtual stamp approximates the real coverage radius the next
    /// directive's ward will project.
    const THORNWARD_VIRTUAL_RADIUS: f32 = 6.0;
    const THORNWARD_VIRTUAL_STRENGTH: f32 = 1.0;

    let mut virtual_cov: Vec<f32> = vec![0.0; scored.len()];
    let mut round_pick = fallback;
    let mut all_picks: Vec<Position> = Vec::with_capacity(k);

    for round in 0..k {
        let mut best_idx: usize = 0;
        let mut best_score = f32::NEG_INFINITY;
        for (i, cs) in scored.iter().enumerate() {
            let score = cs.score(virtual_cov[i], cat);
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }
        round_pick = scored[best_idx].pos;
        all_picks.push(round_pick);

        // Stamp virtual coverage around `round_pick` into every other
        // candidate's `virtual_cov` slot, mirroring
        // `WardCoverageMap::stamp_ward`'s radial-falloff shape. Skip on
        // the last round — the stamp is only meaningful as input to a
        // subsequent round.
        if round + 1 < k {
            let pick = scored[best_idx].pos;
            for (i, cs) in scored.iter().enumerate() {
                let dx = (cs.pos.x() - pick.x()) as f32;
                let dy = (cs.pos.y() - pick.y()) as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > THORNWARD_VIRTUAL_RADIUS {
                    continue;
                }
                let falloff = (1.0 - dist / THORNWARD_VIRTUAL_RADIUS).max(0.0);
                let contribution = THORNWARD_VIRTUAL_STRENGTH * falloff;
                virtual_cov[i] = (virtual_cov[i] + contribution).min(1.0);
            }
        }
    }
    (round_pick, all_picks)
}

/// 220: shared sigmoid for the ward-placement threat lifts. Matches the
/// `Composite{Logistic(k, m)}` curve named in ticket 220 §Scope so the
/// placement scorer and any future DSE consumers share one shape.
/// Input is expected in [0, 1]; output is in (~0, ~1) with the
/// inflection at `m`.
///
/// 296: curve parameters `k` (steepness) and `m` (midpoint) promoted
/// from the previously-hardcoded `(8.0, 0.5)` to
/// `SimConstants::scoring::ward_placement_logistic_{steepness,midpoint}`.
/// Default values preserve pre-296 behavior; 296's hypothesize sweep
/// tunes from there. See `docs/balance/284-ward-anchor-tuning.md`
/// iter-2 for the saturation finding that motivated the promotion.
fn logistic_threat_lift(x: f32, k: f32, m: f32) -> f32 {
    1.0 / (1.0 + (-k * (x - m)).exp())
}

/// 297: compute the fox-spawn-vicinity halo value at a candidate tile.
///
/// Scans the Manhattan neighborhood of `candidate` within
/// `radius_tiles`, and for each tile where `corruption >= threshold`
/// (a ShadowFox spawn-eligible tile per
/// `spawn_shadow_fox_from_corruption`) accumulates a linear falloff
/// `max(0, 1 - manhattan / radius_tiles)`. Returns the maximum
/// contribution across nearby spawn-eligible tiles, clamped to 1.0.
///
/// Max-not-sum because the halo expresses "this tile is near A
/// spawn source" rather than "near many spawn sources" — multiple
/// nearby corruption tiles shouldn't multiplicatively over-anchor
/// the candidate. The Logistic-lift downstream is monotonic, so
/// max-aggregation is the natural fit.
///
/// Cost: O(radius²) per candidate. At radius=20 tiles, that's ~800
/// lookups per candidate × ~430 candidates ≈ 350k lookups per
/// `compute_ward_placement` call. The scorer runs every ~20 ticks
/// per coordinator, gated by `ward_strength_low && thornbriar_available`,
/// so the soak-wide cost is negligible.
///
/// Computed inline (not via a populated `Res<FoxSpawnVicinityMap>`)
/// because the populator system would have to be scheduled, and any
/// schedule-edge near the wildlife chain perturbs seed-42 (ticket
/// 061 precedent at `simulation.rs:314-326`). Inline computation
/// avoids the schedule edge entirely.
fn compute_fox_spawn_vicinity(
    candidate: Position,
    tile_map: &crate::resources::map::TileMap,
    radius_tiles: i32,
    corruption_threshold: f32,
) -> f32 {
    if radius_tiles <= 0 {
        return 0.0;
    }
    let radius_f = radius_tiles as f32;
    let mut best: f32 = 0.0;
    for dy in -radius_tiles..=radius_tiles {
        let y = candidate.y() + dy;
        for dx in -radius_tiles..=radius_tiles {
            let manhattan = dx.unsigned_abs() + dy.unsigned_abs();
            if manhattan as i32 > radius_tiles {
                continue;
            }
            let x = candidate.x() + dx;
            if !tile_map.in_bounds(x, y) {
                continue;
            }
            if tile_map.get(x, y).corruption >= corruption_threshold {
                let falloff = (1.0 - manhattan as f32 / radius_f).max(0.0);
                if falloff > best {
                    best = falloff;
                }
            }
        }
    }
    best.min(1.0)
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
// 382 — sliding ColonyCenter
// ---------------------------------------------------------------------------

/// Periodic re-anchor of `ColonyCenter` from the centroid of live cat
/// positions. Pre-382 the resource was set once at world-gen and never
/// changed; 382's user-chosen approach promotes it to a sliding
/// anchor so as the colony grows, every consumer that uses
/// `colony_center` (patrol perimeter, coordinator perch, corruption
/// search, build placement) orients on the inhabited core rather than
/// the founding tile.
///
/// Cadence: recomputed every
/// `colony_center_update_cadence_ticks` ticks (default 1000 ≈ one
/// in-game season segment). Snap-to-centroid with no clamp; cat
/// populations move slowly enough that jitter isn't an issue at this
/// cadence. Falls back to the existing center when the colony has
/// no live cats (founding edge / total extinction).
#[allow(clippy::type_complexity)]
pub fn update_colony_center(
    cats: Query<
        &Position,
        (
            With<crate::components::physical::Needs>,
            Without<crate::components::physical::Dead>,
            Without<crate::components::wildlife::WildAnimal>,
        ),
    >,
    mut center: ResMut<crate::resources::ColonyCenter>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
) {
    let cadence = constants.scoring.colony_center_update_cadence_ticks.max(1);
    if !time.tick.is_multiple_of(cadence) {
        return;
    }
    let mut count: i64 = 0;
    let mut sx: i64 = 0;
    let mut sy: i64 = 0;
    for p in &cats {
        sx += p.x() as i64;
        sy += p.y() as i64;
        count += 1;
    }
    if count == 0 {
        return;
    }
    let cx = (sx / count) as i32;
    let cy = (sy / count) as i32;
    center.0 = Position::new(cx, cy);
}

// ---------------------------------------------------------------------------
// 382 — ColonyDistrictMap populator
// ---------------------------------------------------------------------------

/// Rebuild `ColonyDistrictMap` each tick from live colony state.
///
/// Three axes, each in `[0.0, 1.0]`:
/// - **frontier**: structure halos (radius
///   `colony_district_structure_halo_radius`) + `CatScentMap` per-bucket
///   contributions scaled by `colony_district_cat_scent_scale`.
/// - **crowding**: per-structure short-radius disc
///   (`colony_district_crowding_radius`) so candidate tiles inside a
///   building footprint or its immediate apron score crowded.
/// - **threat**: per-bucket max of `FoxScentMap`, `FoxApproachCorridorMap`,
///   and `TileMap` corruption sampled at each bucket center.
///
/// Scheduled as a sibling of `update_ward_coverage_map` to share the
/// `magic` chain slot rather than introduce a new top-level edge in
/// `SimulationPlugin::build()` (`learning_bevy_schedule_edge_perturbation`).
///
/// Read by `compute_building_placement` (382) to retire the radius-16
/// spiral search in `find_building_placement`.
pub fn update_colony_district_map(
    structures: Query<
        (&crate::components::building::Structure, &Position),
        Without<crate::components::building::ConstructionSite>,
    >,
    cat_scent: Res<crate::resources::CatScentMap>,
    fox_scent: Res<crate::resources::FoxScentMap>,
    fox_corridor: Res<crate::resources::FoxApproachCorridorMap>,
    tile_map: Res<crate::resources::map::TileMap>,
    mut district: ResMut<crate::resources::ColonyDistrictMap>,
    constants: Res<SimConstants>,
) {
    use crate::resources::DistrictAxis;

    district.clear();

    let halo_radius = constants.scoring.colony_district_structure_halo_radius;
    let crowding_radius = constants.scoring.colony_district_crowding_radius;
    let cat_scent_scale = constants.scoring.colony_district_cat_scent_scale;

    // Frontier: cat-scent contribution per bucket. Iterates the
    // CatScent grid directly rather than per-entity, mirroring the
    // bucket-aligned shape both maps share.
    let bs = district.bucket_size;
    for by in 0..district.grid_h {
        for bx in 0..district.grid_w {
            let cx = bx as i32 * bs + bs / 2;
            let cy = by as i32 * bs + bs / 2;
            let scent = cat_scent.get(cx, cy);
            if scent > 0.0 {
                district.stamp(
                    DistrictAxis::Frontier,
                    cx,
                    cy,
                    (scent * cat_scent_scale).min(1.0),
                    (bs as f32).max(1.0),
                );
            }
        }
    }

    // Frontier halo + crowding disc per existing structure. Iterates
    // once.
    for (structure, anchor) in &structures {
        let center = structure.center(anchor);
        district.stamp(
            DistrictAxis::Frontier,
            center.x(),
            center.y(),
            0.6,
            halo_radius,
        );
        district.stamp(
            DistrictAxis::Crowding,
            center.x(),
            center.y(),
            1.0,
            crowding_radius,
        );
    }

    // Threat: per-bucket max over three signal sources.
    for by in 0..district.grid_h {
        for bx in 0..district.grid_w {
            let cx = bx as i32 * bs + bs / 2;
            let cy = by as i32 * bs + bs / 2;
            let fs = fox_scent.get(cx, cy);
            let fc = fox_corridor.get(cx, cy);
            let cr = if tile_map.in_bounds(cx, cy) {
                tile_map.get(cx, cy).corruption
            } else {
                0.0
            };
            let threat = fs.max(fc).max(cr).clamp(0.0, 1.0);
            if threat > 0.0 {
                let idx = by * district.grid_w + bx;
                district.threat[idx] = threat;
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

    use crate::components::mental::{Memory, MemoryEntry};

    fn empty_placement_maps() -> (
        crate::resources::FoxScentMap,
        crate::resources::CatScentMap,
        crate::resources::WardCoverageMap,
        crate::resources::map::TileMap,
        crate::resources::RecentAmbushMap,
        crate::resources::CarcassScentMap,
        crate::resources::FoxApproachCorridorMap,
    ) {
        (
            crate::resources::FoxScentMap::default(),
            crate::resources::CatScentMap::default(),
            crate::resources::WardCoverageMap::default(),
            crate::resources::map::TileMap::new(120, 90, crate::resources::Terrain::Grass),
            crate::resources::RecentAmbushMap::default(),
            crate::resources::CarcassScentMap::default(),
            crate::resources::FoxApproachCorridorMap::default(),
        )
    }

    /// 296 regression guard: at the post-extraction default params
    /// `(k=8.0, m=0.5)`, the promoted helper must reproduce the
    /// hardcoded pre-296 curve to within `f32::EPSILON`. Catches
    /// accidental value drift during the constants promotion and
    /// during any future refactor of the helper body.
    #[test]
    fn logistic_threat_lift_at_defaults_matches_pre_296_curve() {
        // Hardcoded reference matching coordination.rs at commit 4bcae2de
        // (pre-296): `let k = 8.0; let m = 0.5; 1.0 / (1.0 + (-k * (x - m)).exp())`.
        fn pre_296_reference(x: f32) -> f32 {
            let k: f32 = 8.0;
            let m: f32 = 0.5;
            1.0 / (1.0 + (-k * (x - m)).exp())
        }
        for n in 0..=100 {
            let x = n as f32 / 100.0;
            let promoted = logistic_threat_lift(x, 8.0, 0.5);
            let reference = pre_296_reference(x);
            assert!(
                (promoted - reference).abs() <= f32::EPSILON,
                "logistic_threat_lift({x}, 8.0, 0.5) = {promoted}, \
                 pre-296 reference = {reference}, drift = {}",
                (promoted - reference).abs()
            );
        }
    }

    /// 301 regression guard: `DescendingResidual` with `K=1` is
    /// identical to `SingleShotArgmax`. The round-0 pick is the
    /// argmax winner; with K=1 no virtual coverage is stamped and the
    /// function returns the same `Position` from the same RNG state.
    /// If this drifts, the byte-identity invariant for the default
    /// flag (`SingleShotArgmax`) is at risk because the cached-score
    /// arithmetic in `CandidateScore::score` would no longer match the
    /// inline pre-301 formula.
    #[test]
    fn descending_residual_k1_matches_single_shot_argmax() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm, ra, cs, fac) = empty_placement_maps();
        // Saturate one hot tile so the threat term clamps and the
        // argmax decision lives in the cat_value/distance/jitter
        // domain — the regime 297 iter-2 named as load-bearing.
        fs.deposit(67, 45, 1.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let mut argmax_constants = crate::resources::SimConstants::default();
        argmax_constants.scoring.ward_placement_semantics =
            WardPlacementSemantics::SingleShotArgmax;
        let mut residual_k1_constants = crate::resources::SimConstants::default();
        residual_k1_constants.scoring.ward_placement_semantics =
            WardPlacementSemantics::DescendingResidual;
        residual_k1_constants.scoring.ward_placement_residual_rounds = 1;

        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let argmax_pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &argmax_constants,
            &mut rng_a,
            None,
        );
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let residual_pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &residual_k1_constants,
            &mut rng_b,
            None,
        );
        assert_eq!(
            argmax_pos, residual_pos,
            "K=1 descending-residual must produce the same Position \
             as single-shot argmax for the same RNG seed — byte-identity \
             gate for the default flag"
        );
    }

    /// 301 spread invariant: under `DescendingResidual` with K=2, the
    /// round-1 pick must be geographically distant from the round-0
    /// pick on a map with two disjoint saturated threat clusters.
    /// Round-0 picks the dominant cluster; the virtual coverage stamp
    /// around it suppresses every nearby candidate, so round-1's
    /// argmax must reach the second cluster.
    ///
    /// Pairwise Manhattan ≥ `THORNWARD_VIRTUAL_RADIUS` (6) — the virtual
    /// stamp's repel radius — is the load-bearing assertion: if it
    /// failed, the descending-residual algorithm would not in fact be
    /// spreading picks across the threat surface.
    #[test]
    fn descending_residual_spreads_picks_across_disjoint_clusters() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm, ra, cs, fac) = empty_placement_maps();
        // Two disjoint saturated clusters, both far enough from the
        // existing ward at (60, 45) to escape the Manhattan-3 hard
        // exclusion. Picks should split across them.
        fs.deposit(20, 20, 1.0);
        fs.deposit(100, 70, 1.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let mut k1_constants = crate::resources::SimConstants::default();
        k1_constants.scoring.ward_placement_semantics = WardPlacementSemantics::DescendingResidual;
        k1_constants.scoring.ward_placement_residual_rounds = 1;
        let mut k2_constants = k1_constants.clone();
        k2_constants.scoring.ward_placement_residual_rounds = 2;

        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let round_0_pick = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &k1_constants,
            &mut rng_a,
            None,
        );
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let round_1_pick = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &k2_constants,
            &mut rng_b,
            None,
        );
        // K=2 returns round-1's pick; K=1 returns round-0's pick.
        // The two must differ — the spread invariant. Manhattan ≥ 6
        // (THORNWARD_VIRTUAL_RADIUS) confirms the virtual coverage
        // stamp actually moved the argmax off round-0's cluster.
        assert_ne!(
            round_0_pick, round_1_pick,
            "round-1 pick must differ from round-0 pick on a map with \
             two disjoint clusters; got {round_0_pick:?} both rounds"
        );
        let spread = round_0_pick.distance_to(&round_1_pick);
        assert!(
            spread >= 6.0,
            "round-1 pick {round_1_pick:?} must be ≥ 6 Manhattan from \
             round-0 pick {round_0_pick:?} (THORNWARD_VIRTUAL_RADIUS); \
             got spread = {spread}"
        );
    }

    #[test]
    fn ward_placement_first_ward_lands_on_cluster_centroid() {
        // Empty wards + structures present → first-ward fallback returns
        // the structure-cluster centroid. Preserves the "blanket the
        // colony core" behavior across the influence-map rewrite.
        let structures = vec![Position::new(10, 10), Position::new(14, 10)];
        let wards: Vec<(Position, f32)> = vec![];
        let (fs, cp, wc, tm, ra, cs, fac) = empty_placement_maps();
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let constants = crate::resources::SimConstants::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(0, 0),
            &maps,
            &constants,
            &mut rng,
            None,
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
        let (mut fs, cp, wc, tm, ra, cs, fac) = empty_placement_maps();
        fs.deposit(67, 45, 1.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let constants = crate::resources::SimConstants::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        let dx = (pos.x() - 67).abs();
        let dy = (pos.y() - 45).abs();
        assert!(
            dx <= 5 && dy <= 5,
            "expected placement near fox-scent peak (67, 45), got {pos:?}"
        );
        // Anti-clustering hard-exclusion (Manhattan-3) keeps the new
        // ward off the existing one.
        assert!(
            pos.distance_to(&Position::new(60, 45)) > 3.0,
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
        let (mut fs, cp, mut wc, tm, ra, cs, fac) = empty_placement_maps();
        fs.deposit(67, 45, 1.0);
        wc.stamp_ward(60, 45, 1.0, 9.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let constants = crate::resources::SimConstants::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(99);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        assert!(
            pos.distance_to(&Position::new(60, 45)) > 3.0,
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
        let (mut fs, cp, wc, tm, ra, cs, fac) = empty_placement_maps();
        fs.deposit(67, 45, 1.0); // 7 tiles from anchor
        fs.deposit(67, 85, 1.0); // 47 tiles from anchor — much farther
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let constants = crate::resources::SimConstants::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        let dist_near = pos.distance_to(&Position::new(67, 45));
        let dist_far = pos.distance_to(&Position::new(67, 85));
        assert!(
            dist_near < dist_far,
            "expected placement closer to the nearer peak; got pos={pos:?} \
             dist_near={dist_near} dist_far={dist_far}",
        );
    }

    /// 220 dormancy invariant: with both anchor weights forced to 0.0,
    /// depositing the new substrate signals has zero effect on the
    /// chosen placement — the formula must be byte-identical to the
    /// pre-220 baseline. (Originally an at-default test; 284 activated
    /// the substrate at 0.5 / 0.3, so the regression guard now forces
    /// the weights explicitly rather than relying on the default.)
    #[test]
    fn ward_placement_dormant_when_weights_forced_to_zero() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];

        // Baseline: no ambush / no carcass deposits.
        let (mut fs_a, cp_a, wc_a, tm_a, ra_a, cs_a, fac_a) = empty_placement_maps();
        fs_a.deposit(67, 45, 1.0);
        let maps_a = PlacementMaps {
            fox_scent: &fs_a,
            cat_scent: &cp_a,
            ward_coverage: &wc_a,
            tile_map: &tm_a,
            recent_ambush: &ra_a,
            carcass_scent: &cs_a,
            fox_approach_corridor: &fac_a,
        };

        // Treatment: identical fox-scent peak but with strong ambush +
        // carcass-scent deposits at a *different* hot zone. If dormancy
        // holds, the placement should not move toward the new hot zone.
        let (mut fs_b, cp_b, wc_b, tm_b, mut ra_b, mut cs_b, fac_b) = empty_placement_maps();
        fs_b.deposit(67, 45, 1.0);
        ra_b.deposit(40, 70, 1.0);
        cs_b.deposit(40, 70, 1.0);
        let maps_b = PlacementMaps {
            fox_scent: &fs_b,
            cat_scent: &cp_b,
            ward_coverage: &wc_b,
            tile_map: &tm_b,
            recent_ambush: &ra_b,
            carcass_scent: &cs_b,
            fox_approach_corridor: &fac_b,
        };

        // Force weights to 0.0 explicitly — the default is 0.5 / 0.3 post-284.
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_ambush_anchor_weight = 0.0;
        constants.scoring.ward_recency_anchor_weight = 0.0;

        // Identical RNG seeds → identical jitter → byte-identical scores
        // → byte-identical placement when the lift terms are zero.
        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(220);
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(220);
        let pos_a = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_a,
            &constants,
            &mut rng_a,
            None,
        );
        let pos_b = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_b,
            &constants,
            &mut rng_b,
            None,
        );
        assert_eq!(
            pos_a, pos_b,
            "dormancy invariant violated: depositing ambush/carcass signals \
             changed the chosen placement with default (zero) weights",
        );
    }

    /// 220 lift behavior: when `ward_ambush_anchor_weight` is tuned up,
    /// a recent-ambush hot zone outscores a same-magnitude fox-scent
    /// signal at an equidistant tile, pulling placement toward the
    /// empirical ambush cluster.
    #[test]
    fn ward_placement_shifts_to_ambush_hotspot_when_tuned() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm, mut ra, cs, fac) = empty_placement_maps();
        // Equidistant rival signals from the anchor at (60, 45):
        // fox-scent at (60, 38), ambush at (60, 52) — both 7 tiles away.
        fs.deposit(60, 38, 1.0);
        ra.deposit(60, 52, 1.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        // Tuned-on weight (test-local). The clamp at 1.0 in the threat
        // term means the lift's contribution is bounded; we want a value
        // large enough to dominate the equal-distance jitter band.
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_ambush_anchor_weight = 1.0;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(220);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        // The ambush hotspot is +y from the anchor; the fox-scent peak
        // is -y. The +0.5 baseline of the logistic at zero input means
        // even untouched tiles get *some* lift, so the test asserts a
        // directional preference rather than an exact tile match.
        assert!(
            pos.y() >= 45,
            "expected placement biased toward ambush hotspot at (60, 52); \
             got {pos:?}",
        );
    }

    /// 220 lift behavior: same shape as the ambush test, but for the
    /// kill-site signal — restores the 209 §Scope line 74 consumer.
    /// Note: `CarcassScentMap` uses `bucket_size=3` (Phase 2C, matches
    /// `PreyScentMap`) while the candidate grid steps by 5. The
    /// deposit position is chosen so a candidate tile lands inside the
    /// deposit's 3-tile bucket.
    #[test]
    fn ward_placement_shifts_to_carcass_hotspot_when_tuned() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm, ra, mut cs, fac) = empty_placement_maps();
        // Fox-scent peak at (60, 38) — 7 tiles from anchor.
        fs.deposit(60, 38, 1.0);
        // Carcass at (60, 50) — 5 tiles from anchor; bucket(20, 16) at
        // bucket_size=3 covers world y in [48, 50], and the candidate
        // grid hits y=50 exactly so the read is non-zero.
        cs.deposit(60, 50, 1.0);
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_recency_anchor_weight = 1.0;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(220);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        assert!(
            pos.y() >= 45,
            "expected placement biased toward carcass hotspot at (60, 52); \
             got {pos:?}",
        );
    }

    /// 297 lift behavior: with NO competing fox-scent / ambush / carcass
    /// signals, a corruption tile lifts placement into its halo. The
    /// halo extends `fox_spawn_vicinity_kernel_radius_buckets * 5`
    /// world tiles from each corruption source, falling off linearly
    /// with Manhattan distance. Placement should land within the halo,
    /// not at the corruption tile itself (which has high corruption =
    /// already-high base threat) and not far outside the halo.
    #[test]
    fn ward_placement_shifts_to_fox_intercept_hotspot_when_tuned() {
        let structures = vec![Position::new(30, 20)];
        let wards = vec![(Position::new(30, 20), 6.0)];
        let (fs, cp, wc, mut tm, ra, cs, fac) = empty_placement_maps();
        // Single corruption source at (60, 60). No other threat
        // signals — fox_scent, ambush, carcass all empty. The
        // anchor at (30, 20) is far from corruption (manhattan 70),
        // so the fox-intercept lift is the only positive threat signal.
        tm.get_mut(60, 60).corruption = 1.0;
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_fox_intercept_anchor_weight = 1.0;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(297);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(30, 20),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        // Placement should land in the fox-intercept halo around the
        // corruption tile. The halo radius is 20 world tiles (default
        // kernel_radius_buckets=4 × 5 tiles/bucket). The anchor is at
        // (30, 20), corruption at (60, 60), distance ~70. Placement
        // should be much closer to (60, 60) than to the anchor — the
        // saturated lift (0.96+) inside the halo dominates the
        // distance_cost penalty of 0.005/tile.
        let manhattan_to_corruption = pos.distance_to(&Position::new(60, 60));
        assert!(
            manhattan_to_corruption <= 25.0,
            "expected placement inside fox-intercept halo around (60, 60), \
             distance ≤ ~25 tiles; got {pos:?} at distance {manhattan_to_corruption}",
        );
    }

    /// 297 dormancy invariant: with `ward_fox_intercept_anchor_weight`
    /// forced to 0.0, a corruption tile in the vicinity has no effect
    /// on placement — byte-identical to pre-297 behavior. Combined
    /// with the existing zero-weight dormancy test, this guards the
    /// "lift short-circuits at weight=0" contract.
    #[test]
    fn ward_placement_dormant_when_fox_intercept_weight_zero() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];

        // Baseline: no corruption tile.
        let (mut fs_a, cp_a, wc_a, tm_a, ra_a, cs_a, fac_a) = empty_placement_maps();
        fs_a.deposit(67, 45, 1.0);
        let maps_a = PlacementMaps {
            fox_scent: &fs_a,
            cat_scent: &cp_a,
            ward_coverage: &wc_a,
            tile_map: &tm_a,
            recent_ambush: &ra_a,
            carcass_scent: &cs_a,
            fox_approach_corridor: &fac_a,
        };

        // Treatment: same fox-scent peak + a corruption tile at a
        // different location. If the weight is zero, placement must
        // not move.
        let (mut fs_b, cp_b, wc_b, mut tm_b, ra_b, cs_b, fac_b) = empty_placement_maps();
        fs_b.deposit(67, 45, 1.0);
        tm_b.get_mut(40, 70).corruption = 1.0;
        let maps_b = PlacementMaps {
            fox_scent: &fs_b,
            cat_scent: &cp_b,
            ward_coverage: &wc_b,
            tile_map: &tm_b,
            recent_ambush: &ra_b,
            carcass_scent: &cs_b,
            fox_approach_corridor: &fac_b,
        };

        // Force ALL anchor weights to zero so this test isolates the
        // new axis.
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_ambush_anchor_weight = 0.0;
        constants.scoring.ward_recency_anchor_weight = 0.0;
        constants.scoring.ward_fox_intercept_anchor_weight = 0.0;

        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(297);
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(297);
        let pos_a = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_a,
            &constants,
            &mut rng_a,
            None,
        );
        let pos_b = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_b,
            &constants,
            &mut rng_b,
            None,
        );
        assert_eq!(
            pos_a, pos_b,
            "297 dormancy invariant violated: depositing corruption \
             changed placement with weight forced to 0.0",
        );
    }

    /// 312 dormancy invariant: with `ward_fox_approach_corridor_weight`
    /// forced to 0.0, depositing fox-traffic into the corridor map at a
    /// different hot zone than the fox-scent peak must NOT shift
    /// placement — byte-identical to pre-312 behavior.
    #[test]
    fn corridor_axis_dormant_when_weight_is_zero() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];

        // Baseline: empty corridor map.
        let (mut fs_a, cp_a, wc_a, tm_a, ra_a, cs_a, fac_a) = empty_placement_maps();
        fs_a.deposit(67, 45, 1.0);
        let maps_a = PlacementMaps {
            fox_scent: &fs_a,
            cat_scent: &cp_a,
            ward_coverage: &wc_a,
            tile_map: &tm_a,
            recent_ambush: &ra_a,
            carcass_scent: &cs_a,
            fox_approach_corridor: &fac_a,
        };

        // Treatment: same fox-scent peak + heavy corridor deposit at a
        // different location. With zero weight, placement must not move.
        let (mut fs_b, cp_b, wc_b, tm_b, ra_b, cs_b, mut fac_b) = empty_placement_maps();
        fs_b.deposit(67, 45, 1.0);
        fac_b.deposit(40, 70, 1.0);
        let maps_b = PlacementMaps {
            fox_scent: &fs_b,
            cat_scent: &cp_b,
            ward_coverage: &wc_b,
            tile_map: &tm_b,
            recent_ambush: &ra_b,
            carcass_scent: &cs_b,
            fox_approach_corridor: &fac_b,
        };

        // Force ALL anchor weights to zero so this test isolates the
        // corridor axis.
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_ambush_anchor_weight = 0.0;
        constants.scoring.ward_recency_anchor_weight = 0.0;
        constants.scoring.ward_fox_intercept_anchor_weight = 0.0;
        constants.scoring.ward_fox_approach_corridor_weight = 0.0;

        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(312);
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(312);
        let pos_a = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_a,
            &constants,
            &mut rng_a,
            None,
        );
        let pos_b = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_b,
            &constants,
            &mut rng_b,
            None,
        );
        assert_eq!(
            pos_a, pos_b,
            "312 dormancy invariant violated: corridor deposits shifted \
             placement with weight forced to 0.0",
        );
    }

    /// 312 lift behavior: with the corridor weight tuned ON, a candidate
    /// tile that sits on a high-traffic corridor outscores an equivalent
    /// no-corridor tile at the same threat level and same distance from
    /// the anchor. This is the architectural escape from the 297 iter-2
    /// rank-preservation ceiling: even with both candidates' threat
    /// terms saturated at 1.0, the multiplicative-outside lift expands
    /// the corridor tile's effective score *above* the ceiling.
    #[test]
    fn corridor_axis_lifts_score_on_high_traffic_tile() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];
        let (mut fs, cp, wc, tm, ra, cs, mut fac) = empty_placement_maps();
        // Two equidistant rival signals from the anchor at (60, 45) —
        // each gets the same `fox_scent` saturation, so without the
        // corridor axis the argmax decision lives in the
        // cat_value/distance/jitter domain (the 297 iter-2 regime).
        fs.deposit(60, 38, 1.0); // y -7 from anchor
        fs.deposit(60, 52, 1.0); // y +7 from anchor
                                 // Add corridor traffic ONLY at the +y rival. `FoxApproachCorridorMap`
                                 // is per-tile (bucket_size = 1) — deposit across the +y
                                 // candidate's neighborhood so candidate (60, 50) reads the
                                 // saturated signal. Without this neighborhood the single-tile
                                 // deposit at (60, 52) wouldn't align with any candidate
                                 // (placement candidates step by 5).
        for y in 50..=54 {
            for x in 58..=62 {
                fac.deposit(x, y, 1.0);
            }
        }
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };

        // Tune corridor weight ON, hold every other anchor at default
        // dormancy so the corridor axis is the only post-saturation
        // tiebreaker (besides cat_value/distance/jitter).
        let mut constants = crate::resources::SimConstants::default();
        constants.scoring.ward_ambush_anchor_weight = 0.0;
        constants.scoring.ward_recency_anchor_weight = 0.0;
        constants.scoring.ward_fox_intercept_anchor_weight = 0.0;
        constants.scoring.ward_fox_approach_corridor_weight = 1.0;

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(312);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        // The corridor hotspot is +y. Placement must bias that way to
        // demonstrate the multiplicative-outside lift escaped the
        // ceiling — without it, the symmetric fox-scent peaks would
        // decide on jitter alone.
        assert!(
            pos.y() >= 45,
            "312: expected placement biased toward corridor hotspot at +y; \
             got {pos:?}",
        );
    }

    /// 313 dormancy invariant: with
    /// `ward_placement_cat_value_composition == Additive` (the
    /// default), depositing into `CatScentMap` and into the
    /// corridor map must produce the SAME placement under the new
    /// `CatValueParams` plumbing as under pre-313's `w_cat_value`
    /// scalar. Cross-check against the 312 dormancy test: shared
    /// inputs, shared expected pick, but here we vary `cat_value`
    /// across two runs and assert the picks track each other byte
    /// for byte.
    #[test]
    fn cat_value_gate_dormant_at_additive_default() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];

        // Two runs with identical fox-scent and corridor inputs but
        // mirrored cat-scent. Under `Additive`, the `+ w * cat_value`
        // term biases the argmax toward whichever side has the
        // peak — so swapping cat-scent sides must swap the picks.
        // Under `Gate`, both sides saturate the ramp and decide on
        // jitter alone (tested elsewhere); this test holds the
        // composition at `Additive` to guard byte-identity vs the
        // pre-313 formula.
        let (mut fs_a, mut cp_a, wc_a, tm_a, ra_a, cs_a, fac_a) = empty_placement_maps();
        fs_a.deposit(53, 45, 1.0);
        fs_a.deposit(67, 45, 1.0);
        cp_a.deposit(53, 45, 0.8); // peak on -x side
        let maps_a = PlacementMaps {
            fox_scent: &fs_a,
            cat_scent: &cp_a,
            ward_coverage: &wc_a,
            tile_map: &tm_a,
            recent_ambush: &ra_a,
            carcass_scent: &cs_a,
            fox_approach_corridor: &fac_a,
        };
        let (mut fs_b, mut cp_b, wc_b, tm_b, ra_b, cs_b, fac_b) = empty_placement_maps();
        fs_b.deposit(53, 45, 1.0);
        fs_b.deposit(67, 45, 1.0);
        cp_b.deposit(67, 45, 0.8); // peak on +x side (mirror of A)
        let maps_b = PlacementMaps {
            fox_scent: &fs_b,
            cat_scent: &cp_b,
            ward_coverage: &wc_b,
            tile_map: &tm_b,
            recent_ambush: &ra_b,
            carcass_scent: &cs_b,
            fox_approach_corridor: &fac_b,
        };

        let constants = crate::resources::SimConstants::default();
        assert_eq!(
            constants.scoring.ward_placement_cat_value_composition,
            WardPlacementCatValueComposition::Additive,
            "313: global default composition must remain Additive",
        );

        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(313);
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(313);
        let pos_a = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_a,
            &constants,
            &mut rng_a,
            None,
        );
        let pos_b = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps_b,
            &constants,
            &mut rng_b,
            None,
        );
        // Mirrored cat-scent must mirror the pick when the
        // additive reward is live. If picks were identical the
        // additive bias would be silently inert — a regression.
        assert_ne!(
            pos_a.x(),
            pos_b.x(),
            "313 dormancy: Additive composition must still respond to \
             cat_value; mirrored cat-scent should mirror the pick, \
             got a={pos_a:?}, b={pos_b:?}",
        );
    }

    /// 313 gate behavior — suppression: with
    /// `composition == Gate` and a dead candidate (cat_value = 0)
    /// vs a warm candidate (cat_value >= gate_floor) at equal
    /// threat and equal distance from the anchor, the warm tile
    /// must outscore the dead tile. Without the gate (the
    /// `Additive` baseline checked here for contrast), jitter and
    /// the small `+ w * 0.0` term let the dead side win
    /// roughly half the time.
    #[test]
    fn cat_value_gate_suppresses_dead_tile() {
        let structures = vec![Position::new(60, 45)];
        let wards = vec![(Position::new(60, 45), 6.0)];

        // Two equidistant rival fox-scent peaks. Cat-scent is
        // saturated on the -x side and zero on the +x side. With
        // Gate active, +x has gate=0 and scores ~0 (jitter only);
        // -x has gate=1 and scores ~1.0 (full threat merit).
        let (mut fs, mut cp, wc, tm, ra, cs, fac) = empty_placement_maps();
        fs.deposit(53, 45, 1.0);
        fs.deposit(67, 45, 1.0);
        cp.deposit(53, 45, 1.0); // peak on -x
                                 // +x stays at cat_value = 0 (dead tile).
        let maps = PlacementMaps {
            fox_scent: &fs,
            cat_scent: &cp,
            ward_coverage: &wc,
            tile_map: &tm,
            recent_ambush: &ra,
            carcass_scent: &cs,
            fox_approach_corridor: &fac,
        };

        let mut constants = crate::resources::SimConstants::default();
        // Hold every threat-side weight at dormancy so the test
        // isolates the cat_value gate vs jitter as the only
        // tiebreaker among saturated tiles.
        constants.scoring.ward_ambush_anchor_weight = 0.0;
        constants.scoring.ward_recency_anchor_weight = 0.0;
        constants.scoring.ward_fox_intercept_anchor_weight = 0.0;
        constants.scoring.ward_fox_approach_corridor_weight = 0.0;
        constants.scoring.ward_placement_cat_value_composition =
            WardPlacementCatValueComposition::Gate;

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(313);
        let pos = compute_ward_placement(
            &structures,
            &wards,
            Position::new(60, 45),
            &maps,
            &constants,
            &mut rng,
            None,
        );
        // Gate must zero the +x dead tile's merit; placement
        // therefore lands on -x or near the warm peak. Asserting
        // x <= 60 admits any tile in the warm half-plane.
        assert!(
            pos.x() <= 60,
            "313 gate: expected placement in the cat-warm half-plane \
             (x <= 60); got {pos:?}",
        );
    }

    /// 313 gate behavior — no density-peak reward: with
    /// `composition == Gate`, two candidates at equal threat and
    /// equal distance, both above the gate floor (one warm at
    /// cat_value=floor, one at the peak cat_value=1.0), score
    /// equal merit modulo jitter. Under `Additive`, the
    /// peak tile beats the warm tile by `w_cat_value * (1.0 - floor)`,
    /// which dominates the [0, 0.05) jitter range. This test
    /// inspects the score formula directly (instead of the picked
    /// position) so the assertion is independent of jitter draw
    /// order — the score gap is a deterministic function of the
    /// inputs.
    #[test]
    fn cat_value_gate_does_not_reward_density_peak() {
        // Two candidates with the same threat (1.0 saturated),
        // same coverage (0), same distance_cost, same zero
        // corridor_lift — differ only in cat_value (one at the
        // gate floor, one at the density peak). The jitter is
        // zeroed so the score gap is purely deterministic.
        let warm = CandidateScore {
            pos: Position::new(55, 45),
            threat: 1.0,
            real_coverage: 0.0,
            cat_value: 0.2, // exactly at gate floor
            distance_cost: 0.025,
            jitter: 0.0,
            corridor_lift: 0.0,
        };
        let peak = CandidateScore {
            pos: Position::new(65, 45),
            threat: 1.0,
            real_coverage: 0.0,
            cat_value: 1.0, // density peak
            distance_cost: 0.025,
            jitter: 0.0,
            corridor_lift: 0.0,
        };

        let additive = CatValueParams {
            weight: 0.3,
            composition: WardPlacementCatValueComposition::Additive,
            gate_floor: 0.2,
        };
        let gate = CatValueParams {
            weight: 0.3,
            composition: WardPlacementCatValueComposition::Gate,
            gate_floor: 0.2,
        };

        let warm_additive = warm.score(0.0, additive);
        let peak_additive = peak.score(0.0, additive);
        let warm_gate = warm.score(0.0, gate);
        let peak_gate = peak.score(0.0, gate);

        // Under Additive, the density reward (0.3 * (1.0 - 0.2) =
        // 0.24) makes peak strictly outscore warm; the gap is far
        // wider than the [0, 0.05) jitter range can close.
        let additive_gap = peak_additive - warm_additive;
        assert!(
            additive_gap > 0.20,
            "313 contrast: Additive composition must reward density \
             peaks (gap > 0.20 across the full jitter range); got \
             peak_additive={peak_additive}, warm_additive={warm_additive}, \
             gap={additive_gap}",
        );

        // Under Gate, both candidates clear the floor → gate=1 for
        // both → identical merit. Scores tie exactly when jitter
        // is held at zero.
        assert_eq!(
            warm_gate, peak_gate,
            "313 gate: warm and peak both above gate_floor must score \
             equal under Gate composition; got warm_gate={warm_gate}, \
             peak_gate={peak_gate}",
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
        world.insert_resource(crate::resources::CatScentMap::default());
        world.insert_resource(crate::resources::WardCoverageMap::default());
        world.insert_resource(crate::resources::ColonyDistrictMap::default());
        world.insert_resource(crate::resources::WardIntentMap::default());
        world.insert_resource(crate::resources::RecentAmbushMap::default());
        world.insert_resource(crate::resources::CarcassScentMap::default());
        world.insert_resource(crate::resources::FoxApproachCorridorMap::default());
        world.insert_resource(crate::resources::map::TileMap::new(
            50,
            50,
            crate::resources::map::Terrain::Grass,
        ));
        // Food stores at 10% capacity.
        world.insert_resource(crate::resources::food::FoodStores::new(5.0, 50.0, 0.0));
        // 487 — colony-self queue is initialized in production by
        // SimulationPlugin; tests need to insert it manually.
        world.insert_resource(crate::components::coordination::ColonySelfDirectiveQueue::default());

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
        world.insert_resource(crate::resources::CatScentMap::default());
        world.insert_resource(crate::resources::WardCoverageMap::default());
        world.insert_resource(crate::resources::ColonyDistrictMap::default());
        world.insert_resource(crate::resources::WardIntentMap::default());
        world.insert_resource(crate::resources::RecentAmbushMap::default());
        world.insert_resource(crate::resources::CarcassScentMap::default());
        world.insert_resource(crate::resources::FoxApproachCorridorMap::default());
        world.insert_resource(crate::resources::map::TileMap::new(
            50,
            50,
            crate::resources::map::Terrain::Grass,
        ));
        // Food at 45% — above default 0.5 threshold but below shifted threshold
        // for a non-hunter coordinator.
        world.insert_resource(crate::resources::food::FoodStores::new(22.5, 50.0, 0.0));
        // 487 — colony-self queue is initialized in production by
        // SimulationPlugin; tests need to insert it manually.
        world.insert_resource(crate::components::coordination::ColonySelfDirectiveQueue::default());

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
                        placement_failure_count: 0,
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
                    placement_failure_count: 0,
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
                        placement_failure_count: 0,
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
                        placement_failure_count: 0,
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

    // -----------------------------------------------------------------
    // 382 — compute_building_placement tests
    // -----------------------------------------------------------------

    fn empty_building_placement_maps() -> (
        crate::resources::ColonyDistrictMap,
        crate::resources::FoxApproachCorridorMap,
        crate::resources::FoodLocationMap,
        crate::resources::GardenLocationMap,
        crate::resources::map::TileMap,
    ) {
        (
            crate::resources::ColonyDistrictMap::default(),
            crate::resources::FoxApproachCorridorMap::default(),
            crate::resources::FoodLocationMap::default(),
            crate::resources::GardenLocationMap::default(),
            crate::resources::map::TileMap::new(120, 90, crate::resources::Terrain::Grass),
        )
    }

    #[test]
    fn compute_building_placement_returns_some_in_open_colony() {
        // No existing buildings, fully passable grass map → any
        // candidate is valid. With non-zero frontier (provided by a
        // small CatScentMap deposit, here mocked via a direct stamp on
        // the district map's frontier axis), the argmax should
        // strictly prefer the warm tile.
        let (mut district, fox_corridor, food, garden, tile_map) = empty_building_placement_maps();
        district.stamp(crate::resources::DistrictAxis::Frontier, 60, 45, 1.0, 10.0);
        let constants = SimConstants::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let placement = compute_building_placement(
            StructureType::Stores,
            (3, 3),
            Position::new(60, 45),
            &[],
            &[],
            &district,
            &fox_corridor,
            &food,
            &garden,
            &tile_map,
            &constants,
            &mut rng,
        );
        let pos = placement.expect("placement must succeed on an open map");
        // Argmax should land within the frontier-lifted disc, allowing
        // for the coarse 5-tile candidate step.
        assert!(
            (pos.x() - 60).abs() <= 10 && (pos.y() - 45).abs() <= 10,
            "placement {pos:?} should sit near the lifted frontier center"
        );
    }

    #[test]
    fn compute_building_placement_prefers_food_proximity_for_stores() {
        // Two frontier lifts of equal strength on opposite sides of
        // colony_center. One side also has a saturated FoodLocationMap
        // bucket. Stores carries `food_proximity_weight > 0` per
        // `kind_affinity`, so the argmax should land on the food side.
        let (mut district, fox_corridor, mut food, garden, tile_map) =
            empty_building_placement_maps();
        district.stamp(crate::resources::DistrictAxis::Frontier, 40, 45, 1.0, 15.0);
        district.stamp(crate::resources::DistrictAxis::Frontier, 80, 45, 1.0, 15.0);
        food.stamp(80, 45, 1.0, 15.0);
        let constants = SimConstants::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let pos = compute_building_placement(
            StructureType::Stores,
            (3, 3),
            Position::new(60, 45),
            &[],
            &[],
            &district,
            &fox_corridor,
            &food,
            &garden,
            &tile_map,
            &constants,
            &mut rng,
        )
        .expect("placement must succeed");
        assert!(
            pos.x() > 60,
            "Stores should pick the food-rich side; got {pos:?}"
        );
    }

    #[test]
    fn compute_building_placement_returns_none_below_score_floor() {
        // Lift the score floor above any composite the empty maps can
        // produce. With zero frontier, zero same-kind proximity, and a
        // distance cost that strictly penalizes every candidate, the
        // argmax never clears the floor.
        let (district, fox_corridor, food, garden, tile_map) = empty_building_placement_maps();
        let mut constants = SimConstants::default();
        constants.scoring.building_placement_score_floor = 5.0;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        let placement = compute_building_placement(
            StructureType::Stores,
            (3, 3),
            Position::new(60, 45),
            &[],
            &[],
            &district,
            &fox_corridor,
            &food,
            &garden,
            &tile_map,
            &constants,
            &mut rng,
        );
        assert!(
            placement.is_none(),
            "score floor 5.0 should suppress every candidate on empty maps"
        );
    }
}
