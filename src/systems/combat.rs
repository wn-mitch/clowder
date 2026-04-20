use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{
    Dead, Health, Injury, InjuryKind, InjurySource, Needs, Position,
};
use crate::components::skills::Skills;
use crate::components::wildlife::{WildAnimal, WildlifeAiState};
use crate::resources::map::Terrain;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, TemplateRegistry, VariableContext,
};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, TimeState};
use crate::resources::weather::Weather;

// ---------------------------------------------------------------------------
// Combat jitter
// ---------------------------------------------------------------------------

fn combat_jitter(rng: &mut impl Rng, jitter_range: f32) -> f32 {
    rng.random_range(-jitter_range..jitter_range)
}

// ---------------------------------------------------------------------------
// Combat resolution system
// ---------------------------------------------------------------------------

/// Per-tick combat between cats (Action::Fight) and adjacent wildlife.
///
/// For each fighting cat adjacent to its target wildlife:
/// 1. Cat attacks wildlife (damage based on combat_effective * boldness * ally bonus)
/// 2. Wildlife attacks cat (damage based on threat_power)
/// 3. Morale checks determine if either side flees
/// 4. Resolution: wildlife dies, cat dies (handled by death system), or disengage
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_combat(
    mut cats: Query<
        (
            Entity,
            &mut CurrentAction,
            &mut Health,
            &mut Needs,
            &mut Skills,
            &Personality,
            &Position,
            &Name,
            &mut Memory,
            &mut Mood,
        ),
        Without<Dead>,
    >,
    mut wildlife: Query<
        (
            Entity,
            &WildAnimal,
            &mut Health,
            &Position,
            &mut WildlifeAiState,
        ),
        Without<CurrentAction>,
    >,
    time: Res<TimeState>,
    config: Res<crate::resources::time::SimConfig>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut relationships: ResMut<crate::resources::relationships::Relationships>,
    mut pushback_writer: MessageWriter<crate::systems::magic::CorruptionPushback>,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    registry: Option<Res<TemplateRegistry>>,
) {
    let c = &constants.combat;
    // Collect fighting cats and their targets.
    struct FightInfo {
        cat_entity: Entity,
        target_entity: Entity,
    }

    let fights: Vec<FightInfo> = cats
        .iter()
        .filter(|(_, current, _, _, _, _, _, _, _, _)| {
            current.action == Action::Fight && current.target_entity.is_some()
        })
        .map(|(entity, current, _, _, _, _, _, _, _, _)| FightInfo {
            cat_entity: entity,
            target_entity: current.target_entity.unwrap(),
        })
        .collect();

    if fights.is_empty() {
        return;
    }

    activation.record(Feature::CombatResolved);

    // Count allies per target for group bonus.
    let mut ally_counts: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::new();
    for fight in &fights {
        *ally_counts.entry(fight.target_entity).or_insert(0) += 1;
    }

    // Track wildlife to despawn and cats to reset.
    let mut wildlife_to_despawn: Vec<Entity> = Vec::new();
    let mut cats_to_flee: Vec<Entity> = Vec::new();
    let mut victorious_cats: Vec<(Entity, Entity)> = Vec::new(); // (cat, defeated wildlife)
                                                                 // Pending banishments: (target, location, posse_cats). Resolved in a
                                                                 // second pass after the main loop so boon application has clean access
                                                                 // to participant/witness data.
    let mut pending_banishments: Vec<(Entity, Position, Vec<Entity>)> = Vec::new();

    for fight in &fights {
        let ally_count = ally_counts
            .get(&fight.target_entity)
            .copied()
            .unwrap_or(1)
            .saturating_sub(1);

        // Get wildlife data.
        let (threat_power, animal_defense, _wildlife_health_pct, wildlife_pos, wildlife_alive) = {
            if let Ok((_, animal, health, pos, _)) = wildlife.get(fight.target_entity) {
                (
                    animal.threat_power,
                    animal.defense,
                    health.current / health.max.max(0.01),
                    *pos,
                    health.current > 0.0,
                )
            } else {
                // Wildlife already dead or despawned.
                cats_to_flee.push(fight.cat_entity);
                continue;
            }
        };

        if !wildlife_alive {
            cats_to_flee.push(fight.cat_entity);
            continue;
        }

        // Get cat data.
        let (
            cat_pos,
            cat_boldness,
            cat_temper,
            _cat_loyalty,
            _cat_health_pct,
            combat_effective,
            cat_name,
        ) = {
            if let Ok((_, _, health, _, skills, personality, pos, name, _, _)) =
                cats.get(fight.cat_entity)
            {
                let ce = skills.combat + skills.hunting * c.combat_effective_hunting_weight;
                let hp = health.current / health.max.max(0.01);
                (
                    pos.manhattan_distance(&wildlife_pos),
                    personality.boldness,
                    personality.temper,
                    personality.loyalty,
                    hp,
                    ce,
                    name.0.clone(),
                )
            } else {
                continue;
            }
        };

        // Must be adjacent (within 1 tile) to fight.
        if cat_pos > 1 {
            continue;
        }

        // Skip damage if target already banished this tick.
        if wildlife_to_despawn.contains(&fight.target_entity) {
            continue;
        }

        // --- Cat attacks wildlife ---
        // Posse bonus stacks on top of base ally bonus when ≥ min-allies are
        // coordinating. A lone attacker gets standard ally bonus scaling;
        // a 3-cat gank gets the extra multiplier that makes banishing a
        // shadow-fox actually feasible.
        let posse_allies = if ally_count + 1 >= c.combat_posse_min_allies {
            ally_count
        } else {
            0
        };
        let cat_damage = (combat_effective
            * cat_boldness
            * (1.0 + c.ally_damage_bonus_per_ally * ally_count as f32)
            * (1.0 + c.combat_posse_bonus_per_ally * posse_allies as f32)
            * (1.0 + cat_temper * c.temper_damage_bonus)
            - animal_defense
            + combat_jitter(&mut rng.rng, c.jitter_range))
        .max(0.0);

        if let Ok((_, animal, mut wl_health, _, mut ai_state)) =
            wildlife.get_mut(fight.target_entity)
        {
            wl_health.current = (wl_health.current - cat_damage).max(0.0);

            let species_name = animal.species.name();
            let animal_species = animal.species;
            let hp_frac = wl_health.current / wl_health.max.max(0.01);

            // Narrative: cat attacks.
            if rng.rng.random::<f32>() < c.narrative_attack_chance {
                let text = format!("{cat_name} rakes the {species_name} across the muzzle.");
                log.push(time.tick, text, NarrativeTier::Danger);
            }

            // Shadow-fox banishment — two triggers:
            //   (a) the fox is driven below the banish-threshold HP (solo or
            //       posse — rewards any cat brave enough to wear one down), or
            //   (b) a posse has it outnumbered, in which case the fox is
            //       dissolving under spiritual pressure rather than fleeing.
            // The outnumbered branch intercepts the default wildlife morale
            // logic that would otherwise send the fox running to the map edge.
            let is_shadow_fox =
                animal_species == crate::components::wildlife::WildSpecies::ShadowFox;
            let banish_by_hp = hp_frac <= c.shadow_fox_banish_threshold;
            let banish_by_posse =
                posse_allies > 0 && (ally_count + 1) >= c.wildlife_flee_outnumbered_count;
            if is_shadow_fox && (banish_by_hp || banish_by_posse) {
                let posse: Vec<Entity> = fights
                    .iter()
                    .filter(|f| f.target_entity == fight.target_entity)
                    .map(|f| f.cat_entity)
                    .collect();
                pending_banishments.push((fight.target_entity, wildlife_pos, posse));
                wildlife_to_despawn.push(fight.target_entity);
                continue;
            }

            // Check if wildlife is killed.
            if wl_health.current <= 0.0 {
                let text = format!(
                    "The {species_name} crumples. {cat_name} stands over it, breathing hard."
                );
                log.push(time.tick, text, NarrativeTier::Danger);
                wildlife_to_despawn.push(fight.target_entity);
                victorious_cats.push((fight.cat_entity, fight.target_entity));
                continue;
            }

            // Wildlife morale check.
            let wl_health_pct_now = wl_health.current / wl_health.max.max(0.01);
            let outnumbered = (ally_count + 1) >= c.wildlife_flee_outnumbered_count;
            if wl_health_pct_now <= c.wildlife_flee_health_threshold || outnumbered {
                // Wildlife flees.
                let text = format!("The {species_name} breaks away, outnumbered.");
                log.push(time.tick, text, NarrativeTier::Action);
                // Set wildlife to flee toward nearest edge.
                let flee_dx = if wildlife_pos.x < 40 { -1 } else { 1 };
                let flee_dy = if wildlife_pos.y < 30 { -1 } else { 1 };
                *ai_state = WildlifeAiState::Fleeing {
                    dx: flee_dx,
                    dy: flee_dy,
                };
                wildlife_to_despawn.push(fight.target_entity); // will despawn at edge
                victorious_cats.push((fight.cat_entity, fight.target_entity));
                continue;
            }
        }

        // --- Wildlife attacks cat ---
        let wildlife_damage = (threat_power + combat_jitter(&mut rng.rng, c.jitter_range)).max(0.0);

        if let Ok((
            _,
            _current,
            mut cat_health,
            _needs,
            mut skills,
            personality,
            _,
            name,
            mut memory,
            mut mood,
        )) = cats.get_mut(fight.cat_entity)
        {
            cat_health.current = (cat_health.current - wildlife_damage).max(0.0);

            // Apply injury based on damage.
            if let Some(kind) = apply_injury(
                &mut cat_health,
                wildlife_damage,
                time.tick,
                InjurySource::WildlifeCombat,
                c,
            ) {
                // Narrative for injuries.
                if matches!(kind, InjuryKind::Moderate | InjuryKind::Severe) {
                    let text = format!("{} is knocked aside but scrambles back.", name.0);
                    log.push(time.tick, text, NarrativeTier::Danger);
                }

                memory.remember(MemoryEntry {
                    event_type: MemoryType::Injury,
                    location: None,
                    involved: vec![fight.target_entity],
                    tick: time.tick,
                    strength: match kind {
                        InjuryKind::Minor => c.memory_strength_minor,
                        InjuryKind::Moderate => c.memory_strength_moderate,
                        InjuryKind::Severe => c.memory_strength_severe,
                    },
                    firsthand: true,
                });
            }

            // Combat skill growth.
            skills.combat += skills.growth_rate() * c.combat_skill_growth;

            // Cat morale check.
            let cat_hp = cat_health.current / cat_health.max.max(0.01);
            let morale = cat_hp * c.morale_hp_weight
                + personality.boldness * c.morale_boldness_weight
                + personality.temper * c.morale_temper_weight
                + ally_count as f32 * c.morale_ally_weight
                + personality.loyalty * c.morale_loyalty_weight;
            let morale_threshold =
                c.morale_flee_threshold + combat_jitter(&mut rng.rng, c.jitter_range);

            if morale < morale_threshold {
                // Cat flees.
                cats_to_flee.push(fight.cat_entity);

                mood.modifiers.push_back(MoodModifier {
                    amount: c.flee_mood_penalty,
                    ticks_remaining: c.flee_mood_ticks,
                    source: "fled from combat".to_string(),
                });
            }
        }
    }

    // --- Banishment resolution -------------------------------------------
    // Shadow-foxes driven below banish_threshold under posse damage dissolve
    // into mist instead of dying normally. The posse earns a Legend-tier
    // event, permanent combat training, and a half-year of Valor. Witnesses
    // inside legend_witness_range receive a secondhand mood + safety boost
    // and a lasting Triumph memory.
    for (target_entity, target_pos, posse) in &pending_banishments {
        activation.record(Feature::ShadowFoxBanished);
        if let Some(ref mut score) = colony_score {
            score.banishments += 1;
        }
        // Pushback corruption from the banishment site.
        pushback_writer.write(crate::systems::magic::CorruptionPushback {
            position: *target_pos,
            radius: c.banishment_pushback_radius,
            amount: c.banishment_pushback_amount,
        });

        // Identify posse leader (first cat) for narrative. Capture name,
        // personality, and needs now — the `cats` query is iterated mutably
        // below for boon application, so read-only access has to happen
        // before the mutable loops start. If the leader lookup fails (e.g.
        // the cat was despawned between collection and resolution) we fall
        // back to a plain log.push at the end, skipping template selection.
        let leader_read: Option<(String, Personality, Needs)> = posse
            .first()
            .and_then(|e| cats.get(*e).ok())
            .map(|(_, _, _, needs, _, personality, _, name, _, _)| {
                (name.0.clone(), personality.clone(), needs.clone())
            });
        let leader_name = leader_read
            .as_ref()
            .map(|(n, _, _)| n.clone())
            .unwrap_or_else(|| "A cat".to_string());

        // Collect witnesses first (entities within legend_witness_range and
        // not in the posse) so we can iter_mut without aliasing.
        let witnesses: Vec<Entity> = cats
            .iter()
            .filter(|(e, _, _, _, _, _, pos, _, _, _)| {
                !posse.contains(e) && pos.manhattan_distance(target_pos) <= c.legend_witness_range
            })
            .map(|(e, _, _, _, _, _, _, _, _, _)| e)
            .collect();

        // Apply posse boons. Skill gain diminishes per prior banishment a
        // cat has participated in — a cat's first banishment is still a
        // legend-forging event, but the 5th nets half the skill. Prevents
        // a single cat from running away with the combat pool across a
        // long game.
        let valor_ticks = config.ticks_per_season * 2;
        let seasonal_ticks = config.ticks_per_season;
        for cat_entity in posse {
            if let Ok((_, _, _, _, mut skills, _, _, _, mut memory, mut mood)) =
                cats.get_mut(*cat_entity)
            {
                let prior_triumphs = memory
                    .events
                    .iter()
                    .filter(|m| m.event_type == MemoryType::Triumph && m.firsthand)
                    .count() as f32;
                let gain = c.banishment_combat_skill_grow
                    / (1.0 + prior_triumphs * c.banishment_skill_gain_diminish_factor);
                skills.combat = (skills.combat + gain).min(5.0);
                mood.modifiers.push_back(MoodModifier {
                    amount: c.banishment_valor_mood,
                    ticks_remaining: valor_ticks,
                    source: "valor from banishment".to_string(),
                });
                memory.remember(MemoryEntry {
                    event_type: MemoryType::Triumph,
                    location: Some(*target_pos),
                    involved: posse.clone(),
                    tick: time.tick,
                    strength: 1.0,
                    firsthand: true,
                });
            }
        }

        // Apply witness boons.
        for witness in &witnesses {
            if let Ok((_, _, _, mut w_needs, _, _, _, _, mut w_memory, mut w_mood)) =
                cats.get_mut(*witness)
            {
                w_needs.safety = w_needs.safety.max(c.banishment_witness_safety_floor);
                w_mood.modifiers.push_back(MoodModifier {
                    amount: c.banishment_witness_mood,
                    ticks_remaining: seasonal_ticks,
                    source: "witnessed a banishment".to_string(),
                });
                w_memory.remember(MemoryEntry {
                    event_type: MemoryType::Triumph,
                    location: Some(*target_pos),
                    involved: posse.clone(),
                    tick: time.tick,
                    strength: 0.7,
                    firsthand: false,
                });
            }
        }

        // Legend-tier narrative — route through the template registry so
        // banishment.ron's variants rotate across runs. Fallback preserves
        // the hardcoded line for runs without a registry (tests, degraded
        // boot) or when the leader read failed above.
        let fallback = format!(
            "{leader_name} drives the shadow-fox to its knees. It shrieks, dissolves into mist — gone."
        );
        if let Some((_, ref personality, ref needs)) = leader_read {
            let ctx = TemplateContext {
                action: Action::Fight,
                day_phase: DayPhase::from_tick(time.tick, &config),
                season: Season::from_tick(time.tick, &config),
                weather: Weather::Clear,
                mood_bucket: MoodBucket::Neutral,
                life_stage: LifeStage::Adult,
                has_target: true,
                terrain: Terrain::Grass,
                event: Some("banishment".into()),
            };
            let var_ctx = VariableContext {
                name: &leader_name,
                gender: Gender::Nonbinary,
                weather: Weather::Clear,
                day_phase: ctx.day_phase,
                season: ctx.season,
                life_stage: LifeStage::Adult,
                fur_color: "unknown",
                other: None,
                prey: None,
                item: None,
                item_singular: None,
                quality: None,
            };
            emit_event_narrative(
                registry.as_deref(),
                &mut log,
                time.tick,
                fallback,
                NarrativeTier::Legend,
                &ctx,
                &var_ctx,
                personality,
                needs,
                &mut rng.rng,
            );
        } else {
            log.push(time.tick, fallback, NarrativeTier::Legend);
        }

        // Event log entry with full posse roster.
        if let Some(ref mut elog) = event_log {
            let posse_names: Vec<String> = posse
                .iter()
                .filter_map(|e| cats.get(*e).ok())
                .map(|(_, _, _, _, _, _, _, name, _, _)| name.0.clone())
                .collect();
            elog.push(
                time.tick,
                crate::resources::event_log::EventKind::ShadowFoxBanished {
                    posse: posse_names,
                    location: (target_pos.x, target_pos.y),
                },
            );
        }

        // Release posse cats from Fight action so they can re-evaluate.
        for cat_entity in posse {
            if let Ok((_, mut current, _, _, _, _, _, _, _, _)) = cats.get_mut(*cat_entity) {
                current.ticks_remaining = 0;
            }
        }
        let _ = target_entity; // despawn handled by wildlife_to_despawn loop.
    }

    // Apply victory rewards.
    for (cat_entity, _defeated) in &victorious_cats {
        if let Ok((_, mut current, _, mut needs, _, personality, _, _, _memory, mut mood)) =
            cats.get_mut(*cat_entity)
        {
            needs.respect = (needs.respect + c.victory_respect_gain).min(1.0);
            needs.safety = (needs.safety + c.victory_safety_gain).min(1.0);
            current.ticks_remaining = 0; // Allow new action selection.

            let mut victory_mod = MoodModifier {
                amount: c.victory_mood_bonus,
                ticks_remaining: c.victory_mood_ticks,
                source: "won a fight".to_string(),
            };
            crate::systems::mood::patience_extend(
                &mut victory_mod,
                personality.patience,
                &constants.mood,
            );
            mood.modifiers.push_back(victory_mod);
        }
    }

    // Combat bonding: cats who fought the same target gain fondness/familiarity.
    // Group victorious cats by defeated wildlife entity.
    let mut by_target: std::collections::HashMap<Entity, Vec<Entity>> =
        std::collections::HashMap::new();
    for (cat, defeated) in &victorious_cats {
        by_target.entry(*defeated).or_default().push(*cat);
    }
    for allies in by_target.values() {
        if allies.len() < 2 {
            continue;
        }
        for i in 0..allies.len() {
            for j in (i + 1)..allies.len() {
                let a = allies[i];
                let b = allies[j];
                relationships.modify_fondness(a, b, 0.05);
                relationships.modify_familiarity(a, b, 0.03);
                relationships.modify_fondness(b, a, 0.05);
                relationships.modify_familiarity(b, a, 0.03);
            }
        }
    }

    // Make fleeing cats switch to Flee action.
    for cat_entity in &cats_to_flee {
        if let Ok((_, mut current, _, _, _, _, _, _, _, _)) = cats.get_mut(*cat_entity) {
            current.action = Action::Flee;
            current.ticks_remaining = c.flee_action_ticks;
            // Keep target_position — will be recalculated next evaluate_actions.
            current.target_entity = None;
        }
    }

    // Despawn dead/fleeing wildlife and reset any cats targeting them.
    for wl_entity in &wildlife_to_despawn {
        // Reset cats targeting this wildlife.
        for (_, mut current, _, _, _, _, _, _, _, _) in &mut cats {
            if current.target_entity == Some(*wl_entity) {
                current.ticks_remaining = 0;
                current.target_entity = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Injury helpers
// ---------------------------------------------------------------------------

/// Convert raw damage to an Injury, or None for negligible damage.
fn damage_to_injury(
    damage: f32,
    tick: u64,
    source: InjurySource,
    c: &crate::resources::sim_constants::CombatConstants,
) -> Option<Injury> {
    if damage < c.injury_negligible_threshold {
        return None; // Negligible scratch.
    }
    let kind = if damage < c.injury_moderate_threshold {
        InjuryKind::Minor
    } else if damage < c.injury_severe_threshold {
        InjuryKind::Moderate
    } else {
        InjuryKind::Severe
    };
    Some(Injury {
        kind,
        tick_received: tick,
        healed: false,
        source,
    })
}

/// Apply the injury layer on top of raw damage already dealt. If `damage`
/// exceeds the negligible threshold, creates an `Injury` record, subtracts
/// the severity-specific HP penalty, and pushes the injury onto the `Health`
/// component. Returns the injury kind if one was created.
pub(crate) fn apply_injury(
    health: &mut Health,
    damage: f32,
    tick: u64,
    source: InjurySource,
    c: &crate::resources::sim_constants::CombatConstants,
) -> Option<InjuryKind> {
    let inj = damage_to_injury(damage, tick, source, c)?;
    let kind = inj.kind;
    let penalty = match kind {
        InjuryKind::Minor => c.injury_minor_health_penalty,
        InjuryKind::Moderate => c.injury_moderate_health_penalty,
        InjuryKind::Severe => c.injury_severe_health_penalty,
    };
    health.current = (health.current - penalty).max(0.0);
    health.injuries.push(inj);
    Some(kind)
}

/// Duration in ticks for an injury to heal naturally.
pub fn heal_duration(
    kind: InjuryKind,
    c: &crate::resources::sim_constants::CombatConstants,
) -> u64 {
    match kind {
        InjuryKind::Minor => c.heal_duration_minor,
        InjuryKind::Moderate => c.heal_duration_moderate,
        InjuryKind::Severe => c.heal_duration_severe,
    }
}

// ---------------------------------------------------------------------------
// Healing system
// ---------------------------------------------------------------------------

/// Per-tick healing: check each cat's injuries and heal those past their duration.
pub fn heal_injuries(
    mut query: Query<(
        &mut Health,
        Option<&mut crate::components::identity::Appearance>,
    )>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.combat;
    for (mut health, appearance) in &mut query {
        let mut healed_severe = false;
        let mut hp_restored = 0.0_f32;
        for injury in health.injuries.iter_mut() {
            if injury.healed {
                continue;
            }
            let duration = heal_duration(injury.kind, c);
            if time.tick.saturating_sub(injury.tick_received) >= duration {
                injury.healed = true;
                activation.record(Feature::InjuryHealed);

                // Accumulate the injury kind's health penalty for natural recovery.
                hp_restored += match injury.kind {
                    InjuryKind::Minor => c.injury_minor_health_penalty,
                    InjuryKind::Moderate => c.injury_moderate_health_penalty,
                    InjuryKind::Severe => c.injury_severe_health_penalty,
                };

                if injury.kind == InjuryKind::Severe {
                    healed_severe = true;
                }
            }
        }

        // Restore accumulated HP from healed injuries (natural recovery).
        if hp_restored > 0.0 {
            health.current = (health.current + hp_restored).min(health.max);
        }

        // Remove healed injuries.
        health.injuries.retain(|inj| !inj.healed);

        // Add scar for healed severe injuries.
        if healed_severe {
            if let Some(mut app) = appearance {
                app.distinguishing_marks
                    .push("a ragged scar from an old wound".to_string());
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

    #[test]
    fn damage_to_injury_thresholds() {
        let c = &SimConstants::default().combat;
        assert!(
            damage_to_injury(0.02, 0, InjurySource::Unknown, c).is_none(),
            "negligible damage should not create injury"
        );

        let minor = damage_to_injury(0.05, 100, InjurySource::Unknown, c).unwrap();
        assert_eq!(minor.kind, InjuryKind::Minor);
        assert_eq!(minor.tick_received, 100);
        assert!(!minor.healed);

        let moderate = damage_to_injury(0.15, 200, InjurySource::Unknown, c).unwrap();
        assert_eq!(moderate.kind, InjuryKind::Moderate);

        let severe = damage_to_injury(0.30, 300, InjurySource::Unknown, c).unwrap();
        assert_eq!(severe.kind, InjuryKind::Severe);
    }

    #[test]
    fn heal_duration_ordering() {
        let c = &SimConstants::default().combat;
        assert!(heal_duration(InjuryKind::Minor, c) < heal_duration(InjuryKind::Moderate, c));
        assert!(heal_duration(InjuryKind::Moderate, c) < heal_duration(InjuryKind::Severe, c));
    }

    #[test]
    fn heal_injuries_removes_healed() {
        use bevy_ecs::schedule::Schedule;

        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 200,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        world.insert_resource(SimConstants::default());
        world.insert_resource(SystemActivation::default());

        let entity = world
            .spawn(Health {
                current: 0.5,
                max: 1.0,
                injuries: vec![
                    Injury {
                        kind: InjuryKind::Minor,
                        tick_received: 100,
                        healed: false,
                        source: InjurySource::Unknown,
                    },
                    Injury {
                        kind: InjuryKind::Severe,
                        tick_received: 100,
                        healed: false,
                        source: InjurySource::Unknown,
                    },
                ],
            })
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(heal_injuries);
        schedule.run(&mut world);

        let health = world.get::<Health>(entity).unwrap();
        // Minor injury (50 ticks) at tick 100, now tick 200 = 100 ticks elapsed. Should be healed.
        // Severe injury (500 ticks) at tick 100, now tick 200 = 100 ticks elapsed. Should NOT be healed.
        assert_eq!(
            health.injuries.len(),
            1,
            "minor injury should be healed and removed"
        );
        assert_eq!(
            health.injuries[0].kind,
            InjuryKind::Severe,
            "severe injury should remain"
        );

        // HP must increase by the minor injury penalty (natural recovery).
        let expected_hp = 0.5 + SimConstants::default().combat.injury_minor_health_penalty;
        assert!(
            (health.current - expected_hp).abs() < 1e-5,
            "HP should be restored by minor penalty; expected {expected_hp}, got {}",
            health.current,
        );
    }

    #[test]
    fn apply_injury_creates_record_and_penalty() {
        let c = &SimConstants::default().combat;
        let mut health = Health {
            current: 1.0,
            max: 1.0,
            injuries: Vec::new(),
        };

        // Damage above negligible threshold should create a minor injury.
        let kind = apply_injury(&mut health, 0.05, 10, InjurySource::Unknown, c);
        assert_eq!(kind, Some(InjuryKind::Minor));
        assert_eq!(health.injuries.len(), 1);
        assert_eq!(health.injuries[0].tick_received, 10);
        let expected = 1.0 - c.injury_minor_health_penalty;
        assert!(
            (health.current - expected).abs() < 1e-5,
            "expected HP {expected}, got {}",
            health.current,
        );

        // Negligible damage should not create an injury.
        let kind = apply_injury(&mut health, 0.01, 11, InjurySource::Unknown, c);
        assert_eq!(kind, None);
        assert_eq!(health.injuries.len(), 1, "no new injury for negligible hit");
    }

    #[test]
    fn ambush_damage_heals_via_injury_system() {
        // Simulate the predator_stalk_cats damage pattern followed by
        // heal_injuries, verifying partial HP recovery.
        use bevy_ecs::schedule::Schedule;

        let c = SimConstants::default();
        let raw_damage: f32 = 0.15; // above moderate threshold
        let tick_of_injury: u64 = 50;

        let mut health = Health {
            current: 1.0,
            max: 1.0,
            injuries: Vec::new(),
        };

        // Apply raw damage (same as predator_stalk_cats).
        health.current = (health.current - raw_damage).max(0.0);
        let kind = apply_injury(
            &mut health,
            raw_damage,
            tick_of_injury,
            InjurySource::Unknown,
            &c.combat,
        );
        assert_eq!(kind, Some(InjuryKind::Moderate));

        let hp_after_hit = health.current;
        let expected_after_hit = 1.0 - raw_damage - c.combat.injury_moderate_health_penalty;
        assert!(
            (hp_after_hit - expected_after_hit).abs() < 1e-5,
            "HP after hit: expected {expected_after_hit}, got {hp_after_hit}",
        );

        // Advance time past heal_duration_moderate and run heal_injuries.
        let heal_tick = tick_of_injury + c.combat.heal_duration_moderate;

        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: heal_tick,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        world.insert_resource(c);
        world.insert_resource(SystemActivation::default());

        let entity = world.spawn(health).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(heal_injuries);
        schedule.run(&mut world);

        let healed = world.get::<Health>(entity).unwrap();
        assert!(
            healed.injuries.is_empty(),
            "moderate injury should be healed and removed"
        );
        let expected_hp = hp_after_hit
            + SimConstants::default()
                .combat
                .injury_moderate_health_penalty;
        assert!(
            (healed.current - expected_hp).abs() < 1e-5,
            "HP should be partially restored; expected {expected_hp}, got {}",
            healed.current,
        );
        // Raw damage portion remains unrecovered.
        assert!(
            healed.current < 1.0,
            "full HP should not be restored — raw damage is permanent",
        );
    }

    #[test]
    fn combat_effective_formula() {
        let c = &SimConstants::default().combat;
        let skills = Skills {
            combat: 0.05,
            hunting: 0.5,
            ..Skills::default()
        };
        let effective = skills.combat + skills.hunting * c.combat_effective_hunting_weight;
        assert!(
            (effective - 0.2).abs() < 1e-5,
            "combat_effective should be 0.2; got {effective}"
        );
    }

    /// First banishment grants the base boon; subsequent banishments
    /// grant diminishing returns so no single cat runs away with combat
    /// skill across a long game.
    #[test]
    fn banishment_skill_gain_diminishes_per_prior_triumph() {
        let c = &SimConstants::default().combat;
        let gain = |prior: f32| -> f32 {
            c.banishment_combat_skill_grow / (1.0 + prior * c.banishment_skill_gain_diminish_factor)
        };
        // Base = 0.25, factor = 0.25 → progression 0.25, 0.20, 0.167, 0.143, 0.125.
        let expected = [0.25_f32, 0.20, 0.1667, 0.1429, 0.125];
        for (i, want) in expected.iter().enumerate() {
            let got = gain(i as f32);
            assert!(
                (got - want).abs() < 1e-3,
                "prior_triumphs={i}: expected {want}, got {got}"
            );
        }
        // Cumulative total across 7 banishments should stay under 1.1
        // (vs. 1.75 under the old linear formula).
        let total: f32 = (0..7).map(|i| gain(i as f32)).sum();
        assert!(
            total < 1.1,
            "7-banishment total should be under 1.1, got {total}"
        );
    }
}
