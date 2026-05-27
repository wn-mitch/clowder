use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::body_zones::{BodyPart, CatBodyModel};
use crate::components::equipment_effects::{equipment_modifiers_for, EquipmentModifiers};
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::magic::Inventory;
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier, MoodSource};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, InjurySource, Needs, Position};
use crate::components::skills::Skills;
use crate::components::wildlife::{WildAnimal, WildlifeAiState};
use crate::messages::body_part_injury::BodyPartInjury;
use crate::resources::map::Terrain;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, TemplateRegistry, VariableContext,
};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, TimeState};
use crate::resources::trace_log::FocalResolverSink;
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

/// Message writers used by `resolve_combat`, bundled into one
/// `SystemParam` so the system stays within Bevy's 16-param ceiling
/// after ticket 477 added the focal-trace param.
#[derive(bevy_ecs::system::SystemParam)]
pub struct CombatWriters<'w> {
    pub pushback: MessageWriter<'w, crate::systems::magic::CorruptionPushback>,
    pub body_part: MessageWriter<'w, BodyPartInjury>,
}

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
            &mut CatBodyModel,
            &Inventory,
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
    time_scale: Res<crate::resources::time::TimeScale>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut relationships: ResMut<crate::resources::relationships::Relationships>,
    mut writers: CombatWriters,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    registry: Option<Res<TemplateRegistry>>,
    // 477 — focal-cat resolver-trace sink for armor-reduction reads.
    focal_trace: crate::resources::trace_log::FocalTraceParam,
    mut commands: Commands,
) {
    let c = &constants.combat;
    let focal_sink = focal_trace.sink(time.tick);
    // Collect fighting cats and their targets.
    struct FightInfo {
        cat_entity: Entity,
        target_entity: Entity,
    }

    let fights: Vec<FightInfo> = cats
        .iter()
        .filter(|(_, current, _, _, _, _, _, _, _, _, _, _)| {
            current.action == Action::Fight && current.target_entity.is_some()
        })
        .map(
            |(entity, current, _, _, _, _, _, _, _, _, _, _)| FightInfo {
                cat_entity: entity,
                target_entity: current.target_entity.unwrap(),
            },
        )
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
            if let Ok((_, _, health, _, skills, personality, pos, name, _, _, _, _)) =
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
            cat_pos,
            name,
            mut memory,
            mut mood,
            mut cat_body_model,
            inventory,
        )) = cats.get_mut(fight.cat_entity)
        {
            let injury_pos = *cat_pos;
            // 477 — worn-equipment aggregate for armor reduction. The
            // health scalar and the body-model injury must agree on the
            // post-armor damage, so reduce once here and pass the reduced
            // value to both. `damage_to_body_part` still receives the
            // equipment + sink so the reduction surfaces in the trace.
            let em = equipment_modifiers_for(inventory, c);
            let reduced_damage = armor_reduced_damage(
                wildlife_damage,
                InjurySource::WildlifeCombat,
                crate::components::body_zones::WoundKind::Normal,
                &em,
            );
            cat_health.current = (cat_health.current - reduced_damage).max(0.0);

            // 095 Phase 1 — anatomical substrate is canonical. Legacy
            // `Injury` record + injury_*_health_penalty retired.
            damage_to_body_part(
                fight.cat_entity,
                &mut cat_body_model,
                wildlife_damage,
                time.tick,
                InjurySource::WildlifeCombat,
                c,
                &mut rng,
                &mut writers.body_part,
                &mut activation,
                Some(&em),
                focal_sink.as_ref(),
            );

            // Narrative + memory side-effects derived from raw damage
            // magnitude (no stored Injury record).
            if let Some(tier) = classify_damage_for_narrative(wildlife_damage, c) {
                if matches!(tier, DamageTier::Moderate | DamageTier::Severe) {
                    let text = format!("{} is knocked aside but scrambles back.", name.0);
                    log.push(time.tick, text, NarrativeTier::Danger);
                }

                memory.remember(MemoryEntry {
                    event_type: MemoryType::Injury,
                    location: Some(injury_pos),
                    involved: vec![fight.target_entity],
                    tick: time.tick,
                    strength: match tier {
                        DamageTier::Minor => c.memory_strength_minor,
                        DamageTier::Moderate => c.memory_strength_moderate,
                        DamageTier::Severe => c.memory_strength_severe,
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

                mood.modifiers.push_back(
                    MoodModifier::new(
                        c.flee_mood_penalty,
                        c.flee_mood_duration.ticks(&time_scale),
                        "fled from combat",
                    )
                    .with_kind(MoodSource::Fear),
                );
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
        // §9.2 / ticket 049 — cat-on-cat banishment branch. The
        // shadowfox path that populates `pending_banishments` today is
        // the only producer; this branch lights up only when a future
        // system pushes a cat onto the list. Tag the cat with the
        // `Banished` marker (consumed by §9.3 stance overlay) and
        // increment the colony's `banishments` counter. Skip the
        // shadowfox-specific corruption pushback + posse/witness boons
        // — banishing a clanmate is not a Legend-tier triumph.
        if cats.get(*target_entity).is_ok() {
            commands
                .entity(*target_entity)
                .insert(crate::components::markers::Banished);
            if let Some(ref mut score) = colony_score {
                score.banishments += 1;
            }
            continue;
        }
        activation.record(Feature::ShadowFoxBanished);
        if let Some(ref mut score) = colony_score {
            score.banishments += 1;
        }
        // Pushback corruption from the banishment site.
        writers
            .pushback
            .write(crate::systems::magic::CorruptionPushback {
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
            .map(|(_, _, _, needs, _, personality, _, name, _, _, _, _)| {
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
            .filter(|(e, _, _, _, _, _, pos, _, _, _, _, _)| {
                !posse.contains(e) && pos.manhattan_distance(target_pos) <= c.legend_witness_range
            })
            .map(|(e, _, _, _, _, _, _, _, _, _, _, _)| e)
            .collect();

        // Apply posse boons. Skill gain diminishes per prior banishment a
        // cat has participated in — a cat's first banishment is still a
        // legend-forging event, but the 5th nets half the skill. Prevents
        // a single cat from running away with the combat pool across a
        // long game.
        let valor_ticks = config.ticks_per_season * 2;
        let seasonal_ticks = config.ticks_per_season;
        for cat_entity in posse {
            if let Ok((_, _, _, _, mut skills, _, _, _, mut memory, mut mood, _, _)) =
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
                mood.modifiers.push_back(
                    MoodModifier::new(
                        c.banishment_valor_mood,
                        valor_ticks,
                        "valor from banishment",
                    )
                    .with_kind(MoodSource::Triumph),
                );
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
            if let Ok((_, _, _, mut w_needs, _, _, _, _, mut w_memory, mut w_mood, _, _)) =
                cats.get_mut(*witness)
            {
                w_needs.safety = w_needs.safety.max(c.banishment_witness_safety_floor);
                w_mood.modifiers.push_back(
                    MoodModifier::new(
                        c.banishment_witness_mood,
                        seasonal_ticks,
                        "witnessed a banishment",
                    )
                    .with_kind(MoodSource::Triumph),
                );
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
                .map(|(_, _, _, _, _, _, _, name, _, _, _, _)| name.0.clone())
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
            if let Ok((_, mut current, _, _, _, _, _, _, _, _, _, _)) = cats.get_mut(*cat_entity) {
                current.ticks_remaining = 0;
            }
        }
        let _ = target_entity; // despawn handled by wildlife_to_despawn loop.
    }

    // Apply victory rewards.
    for (cat_entity, _defeated) in &victorious_cats {
        if let Ok((_, mut current, _, mut needs, _, personality, _, _, _memory, mut mood, _, _)) =
            cats.get_mut(*cat_entity)
        {
            needs.respect = (needs.respect + c.victory_respect_gain).min(1.0);
            needs.safety = (needs.safety + c.victory_safety_gain).min(1.0);
            current.ticks_remaining = 0; // Allow new action selection.

            let mut victory_mod = MoodModifier::new(
                c.victory_mood_bonus,
                c.victory_mood_duration.ticks(&time_scale),
                "won a fight",
            )
            .with_kind(MoodSource::Pride);
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
    //
    // `ticks_remaining = 0` matches the sibling Flee paths in
    // `disposition.rs` (ThreatDetected interrupt) and `goap.rs`
    // (ThreatNearby urgency preempt). A non-zero value here would block
    // `evaluate_and_plan`'s gate at goap.rs:~975 — and because the
    // combat system re-fires every tick on persistent wildlife threats,
    // it refreshed faster than it could decay, locking cats in Flee
    // until they starved (ticket 043, mirrors ticket 042's pattern for
    // a different urgency path).
    for cat_entity in &cats_to_flee {
        if let Ok((_, mut current, _, _, _, _, _, _, _, _, _, _)) = cats.get_mut(*cat_entity) {
            current.action = Action::Flee;
            current.ticks_remaining = 0;
            // Keep target_position — will be recalculated next evaluate_actions.
            current.target_entity = None;
        }
    }

    // Despawn dead/fleeing wildlife and reset any cats targeting them.
    for wl_entity in &wildlife_to_despawn {
        // Reset cats targeting this wildlife.
        for (_, mut current, _, _, _, _, _, _, _, _, _, _) in &mut cats {
            if current.target_entity == Some(*wl_entity) {
                current.ticks_remaining = 0;
                current.target_entity = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Injury narrative classifier (ticket 095 Phase 1 Stage B)
//
// Replaces the retired `damage_to_injury` / `apply_injury` pair. The
// anatomical substrate (`CatBodyModel`) is now canonical for injury state
// — what's left at each combat site is the narrative + memory side-effect
// keyed off damage magnitude. `classify_damage_for_narrative` returns the
// tier label for those side-effects without creating any stored record.
// ---------------------------------------------------------------------------

/// Damage-magnitude bucket used by narrative + memory writers. Mirrors the
/// retired `InjuryKind` enum so existing narrative templates keep working
/// without storing a per-cat injury list. `None` ⇒ negligible scratch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageTier {
    Minor,
    Moderate,
    Severe,
}

pub fn classify_damage_for_narrative(
    damage: f32,
    c: &crate::resources::sim_constants::CombatConstants,
) -> Option<DamageTier> {
    if damage < c.injury_negligible_threshold {
        return None;
    }
    Some(if damage < c.injury_moderate_threshold {
        DamageTier::Minor
    } else if damage < c.injury_severe_threshold {
        DamageTier::Moderate
    } else {
        DamageTier::Severe
    })
}

// ---------------------------------------------------------------------------
// Body-zone substrate (ticket 095 Phase 1)
// ---------------------------------------------------------------------------

/// Select a target body part for a non-negligible hit, weighted by attacker
/// type. Spec §Combat Targeting Weights (Predators → Cats). Phase 1 collapses
/// `InjurySource` to the rough attacker class — `WildlifeCombat` /
/// `FoxConfrontation` / `ShadowFoxAmbush` all use the Fox table (wildlife
/// damage is fox-dominated today; hawk/snake combat lands with Phase 2).
/// `MagicMisfire` / `Unknown` distribute uniformly.
fn select_body_part_for_attacker(source: InjurySource, rng: &mut SimRng) -> BodyPart {
    use BodyPart::*;
    let table: &[(BodyPart, f32)] = match source {
        InjurySource::WildlifeCombat
        | InjurySource::FoxConfrontation
        | InjurySource::ShadowFoxAmbush => &[
            (Throat, 0.35),
            (Flanks, 0.25),
            (Haunches, 0.15),
            (FrontLeftPaw, 0.0375),
            (FrontRightPaw, 0.0375),
            (RearLeftPaw, 0.0375),
            (RearRightPaw, 0.0375),
            (Ears, 0.10),
        ],
        InjurySource::MagicMisfire | InjurySource::Unknown => &[
            (Whiskers, 1.0 / 13.0),
            (Ears, 1.0 / 13.0),
            (MouthJaw, 1.0 / 13.0),
            (Scruff, 1.0 / 13.0),
            (Throat, 1.0 / 13.0),
            (Flanks, 1.0 / 13.0),
            (Belly, 1.0 / 13.0),
            (FrontLeftPaw, 1.0 / 13.0),
            (FrontRightPaw, 1.0 / 13.0),
            (RearLeftPaw, 1.0 / 13.0),
            (RearRightPaw, 1.0 / 13.0),
            (Haunches, 1.0 / 13.0),
            (Tail, 1.0 / 13.0),
        ],
    };
    let total: f32 = table.iter().map(|(_, w)| *w).sum();
    let mut roll = rng.rng.random::<f32>() * total;
    for (part, w) in table {
        if roll < *w {
            return *part;
        }
        roll -= *w;
    }
    table.last().expect("targeting table is non-empty").0
}

/// Shadow-write the anatomical injury substrate. Called alongside the legacy
/// `damage_to_injury` / `apply_injury` push during Stage A. Selects a target
/// part via `select_body_part_for_attacker`, applies the tissue damage on
/// the `CatBodyModel`, and emits a `BodyPartInjury` message for the L1 trace
/// + Feature canary.
///
/// Returns `Some((part, condition))` when damage was non-negligible, else
/// `None`. The caller passes the cat's entity for the message.
#[allow(clippy::too_many_arguments)]
pub(crate) fn damage_to_body_part(
    entity: Entity,
    body_model: &mut CatBodyModel,
    damage: f32,
    tick: u64,
    source: InjurySource,
    c: &crate::resources::sim_constants::CombatConstants,
    rng: &mut SimRng,
    writer: &mut MessageWriter<BodyPartInjury>,
    activation: &mut SystemActivation,
    equipment: Option<&EquipmentModifiers>,
    focal_sink: Option<&FocalResolverSink>,
) -> Option<(BodyPart, crate::components::body_zones::PartCondition)> {
    damage_to_body_part_with_kind(
        entity,
        body_model,
        damage,
        crate::components::body_zones::WoundKind::Normal,
        tick,
        source,
        c,
        rng,
        writer,
        activation,
        equipment,
        focal_sink,
    )
}

/// Which armor channel an incoming hit is reduced by (ticket 477).
/// `Unarmored` means physical armor offers no protection — the doctrine
/// call is that hide bracers don't blunt magical / festering damage, so
/// `MagicMisfire` and `WoundKind::Festering` route here regardless of
/// what the cat wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmorDamageClass {
    Blunt,
    /// Dormant until a pierce-class attacker exists. `PiercePartial`
    /// armor already aggregates `armor_pierce_reduction`, and the
    /// `armor_reduced_damage` match consumes this arm — but no current
    /// `InjurySource` maps here (claws/bites read as `Blunt`). The sling
    /// follow-up (or a future pierce-class wildlife) flips a source to
    /// this variant; the read site is wired now so that change is a
    /// one-arm edit. Mirrors the dormant `NoiseClass::Loud` read site.
    #[allow(dead_code)]
    Pierce,
    Unarmored,
}

fn armor_damage_class(
    source: InjurySource,
    kind: crate::components::body_zones::WoundKind,
) -> ArmorDamageClass {
    use crate::components::body_zones::WoundKind;
    if matches!(kind, WoundKind::Festering) {
        return ArmorDamageClass::Unarmored;
    }
    match source {
        // Claws + bites read as blunt-class physical contact. No pierce-
        // class incoming attacker exists yet (a future ranged/pierce
        // wildlife or the sling follow-up would route to `Pierce`).
        InjurySource::WildlifeCombat
        | InjurySource::ShadowFoxAmbush
        | InjurySource::FoxConfrontation => ArmorDamageClass::Blunt,
        // Magic + legacy untagged damage bypass physical armor.
        InjurySource::MagicMisfire | InjurySource::Unknown => ArmorDamageClass::Unarmored,
    }
}

/// 477 — armor-reduced damage for a hit of the given source/kind against
/// a cat wearing `em`. Single source of truth shared by the `Health`
/// scalar path and `damage_to_body_part`'s body-model path so they never
/// diverge. Pure; no trace side-effect (the caller that wants the trace
/// row routes through `damage_to_body_part`).
pub(crate) fn armor_reduced_damage(
    damage: f32,
    source: InjurySource,
    kind: crate::components::body_zones::WoundKind,
    em: &EquipmentModifiers,
) -> f32 {
    let reduction = match armor_damage_class(source, kind) {
        ArmorDamageClass::Blunt => em.armor_blunt_reduction,
        ArmorDamageClass::Pierce => em.armor_pierce_reduction,
        ArmorDamageClass::Unarmored => 0.0,
    };
    damage * (1.0 - reduction)
}

/// 472 — variant of `damage_to_body_part` that explicitly carries the
/// `WoundKind` flavor. `apply_misfire`'s `WoundTransfer` arm calls this
/// with `WoundKind::Festering` to author the slow-healing wound that
/// drives the `SeekHealing` HTN method (when wired in 473).
#[allow(clippy::too_many_arguments)]
pub(crate) fn damage_to_body_part_with_kind(
    entity: Entity,
    body_model: &mut CatBodyModel,
    damage: f32,
    kind: crate::components::body_zones::WoundKind,
    tick: u64,
    source: InjurySource,
    c: &crate::resources::sim_constants::CombatConstants,
    rng: &mut SimRng,
    writer: &mut MessageWriter<BodyPartInjury>,
    activation: &mut SystemActivation,
    // 477 — worn-equipment aggregate for armor reduction. `None` from
    // callers where armor doesn't apply (magic misfire path) or isn't
    // plumbed.
    equipment: Option<&EquipmentModifiers>,
    // 477 — focal-cat trace sink. Records the armor-reduction modifier
    // as a named `L4Resolver` row when the struck cat is the focal cat.
    focal_sink: Option<&FocalResolverSink>,
) -> Option<(BodyPart, crate::components::body_zones::PartCondition)> {
    // 477 — armor reduction. Composes the worn-armor aggregate against
    // the hit's damage class, surfacing the applied reduction in the
    // resolver trace (never a hidden post-hoc bonus).
    let damage = if let Some(em) = equipment {
        let reduced = armor_reduced_damage(damage, source, kind, em);
        if reduced < damage {
            if let Some(sink) = focal_sink {
                sink.record(
                    entity,
                    "damage_to_body_part",
                    "armor.reduction",
                    damage,
                    reduced,
                );
            }
        }
        reduced
    } else {
        damage
    };

    if damage < c.injury_negligible_threshold {
        return None;
    }
    let part = select_body_part_for_attacker(source, rng);
    let condition = body_model.apply_damage_with_kind(
        part,
        damage,
        kind,
        &c.body_zone_condition_thresholds,
        &c.body_zone_permanent_at_destroyed,
    );
    activation.record(Feature::BodyPartInjury);
    writer.write(BodyPartInjury {
        entity,
        part,
        tissue_damage_delta: damage,
        condition,
        source,
        kind,
        tick,
    });
    Some((part, condition))
}

// `heal_duration(InjuryKind, ...)` retired with the `InjuryKind` enum
// (Ticket 095 Phase 1 Stage B). Body-zone per-part healing uses
// `BodyZoneHealing` durations indexed by (category, condition).

// ---------------------------------------------------------------------------
// Healing system
// ---------------------------------------------------------------------------

/// Compute per-tick `tissue_damage` decrement for each body part based on
/// its category × current condition. Returns an array indexed by
/// `BodyPart::index()`. Spec §Cat Healing Rates.
fn per_part_heal_decrements(
    body_model: &CatBodyModel,
    healing: &crate::resources::sim_constants::BodyZoneHealing,
    time_scale: &crate::resources::time::TimeScale,
) -> [f32; crate::components::body_zones::CAT_BODY_PART_COUNT] {
    use crate::components::body_zones::{
        BodyPart, PartCategory, PartCondition, WoundKind, CAT_BODY_PART_COUNT,
    };
    let mut out = [0.0_f32; CAT_BODY_PART_COUNT];
    for (i, part) in BodyPart::ALL.iter().enumerate() {
        let condition = body_model.parts[i].condition;
        let kind = body_model.parts[i].kind;
        if condition == PartCondition::Healthy {
            continue;
        }
        let duration = match (part.category(), condition) {
            (PartCategory::SoftTissue, PartCondition::Bruised) => healing.soft_bruised_to_healthy,
            (PartCategory::SoftTissue, PartCondition::Wounded) => healing.soft_wounded_to_bruised,
            (PartCategory::SoftTissue, PartCondition::Mangled)
            | (PartCategory::SoftTissue, PartCondition::Destroyed) => {
                healing.soft_mangled_to_wounded
            }
            (PartCategory::Structural, PartCondition::Bruised) => {
                healing.structural_bruised_to_healthy
            }
            (PartCategory::Structural, PartCondition::Wounded) => {
                healing.structural_wounded_to_bruised
            }
            (PartCategory::Structural, PartCondition::Mangled)
            | (PartCategory::Structural, PartCondition::Destroyed) => {
                healing.structural_mangled_to_wounded
            }
            (PartCategory::Sensory, PartCondition::Bruised) => healing.sensory_bruised_to_healthy,
            (PartCategory::Sensory, PartCondition::Wounded) => healing.sensory_wounded_to_bruised,
            (PartCategory::Sensory, PartCondition::Mangled)
            | (PartCategory::Sensory, PartCondition::Destroyed) => {
                healing.sensory_mangled_to_wounded
            }
            (PartCategory::Throat, PartCondition::Bruised) => healing.throat_bruised_to_healthy,
            (PartCategory::Throat, PartCondition::Wounded) => healing.throat_wounded_to_bruised,
            // Throat Mangled+ is fatal before natural healing per spec — no
            // recovery rate. Leave decrement at 0 so this branch is a no-op.
            (PartCategory::Throat, _) => continue,
            (PartCategory::Tail, PartCondition::Bruised) => healing.tail_bruised_to_healthy,
            (PartCategory::Tail, PartCondition::Wounded) => healing.tail_wounded_to_bruised,
            (PartCategory::Tail, PartCondition::Mangled)
            | (PartCategory::Tail, PartCondition::Destroyed) => healing.tail_mangled_to_wounded,
            (_, PartCondition::Healthy) => continue,
        };
        let ticks = duration.ticks(time_scale).max(1);
        let base_decrement = 1.0 / ticks as f32;
        // 472 — festering wounds heal much more slowly. Exhaustive
        // match on `WoundKind` so a future variant (Frozen, Poisoned)
        // is a compile error here until its multiplier is named.
        out[i] = match kind {
            WoundKind::Normal => base_decrement,
            WoundKind::Festering => base_decrement * healing.festering_heal_rate_multiplier,
        };
    }
    out
}

/// Per-tick healing: check each cat's injuries and heal those past their duration.
pub fn heal_injuries(
    mut query: Query<(
        &mut Health,
        Option<&mut crate::components::identity::Appearance>,
        &mut CatBodyModel,
    )>,
    time: Res<TimeState>,
    time_scale: Res<crate::resources::time::TimeScale>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.combat;
    let _ = time;
    let weights = &c.body_zone_pain_weights;
    let max_pain: f32 = weights.iter().sum();
    for (mut health, appearance, mut body_model) in &mut query {
        // Snapshot pre-heal pain so we can credit the freed pain delta
        // back to Health.current. Replaces the legacy
        // `Health.injuries` heal loop's HP-restore-on-heal path.
        let pre_tick_pain = body_model.total_pain(weights);
        // 095 Phase 1 — anatomical per-part healing on the canonical
        // substrate. Permanent destroyed parts (ears, mouth/jaw,
        // haunches, tail) stay locked; the scar appearance below is
        // authored when a part first crosses to permanently Destroyed.
        let decrements = per_part_heal_decrements(&body_model, &c.body_zone_healing, &time_scale);
        body_model.heal_tick(&decrements, &c.body_zone_condition_thresholds);
        let post_tick_pain = body_model.total_pain(weights);
        let pain_recovered = (pre_tick_pain - post_tick_pain).max(0.0);
        if pain_recovered > 0.0 && max_pain > 0.0 {
            // Tissue healing returns the freed pain-fraction to
            // Health.current. A part healing from Wounded → Bruised
            // restores its pain-weighted share of the HP budget.
            // `InjuryHealed` Feature emits per cat per healing tick that
            // produces any recovery — mirrors the legacy "injury crossed
            // its duration" emission cadence.
            let hp_restored = pain_recovered / max_pain;
            health.current = (health.current + hp_restored).min(health.max);
            activation.record(Feature::InjuryHealed);
        }

        // Permanent-Destroyed parts (ears, mouth/jaw, haunches, tail)
        // are identity-bearing scars. Author the appearance line iff
        // any newly-permanent part exists for this cat without an
        // existing scar marker. Spec §Cat Functional Consequences.
        if let Some(mut app) = appearance {
            let has_permanent = body_model.parts.iter().any(|p| p.permanent);
            let has_scar_text = app
                .distinguishing_marks
                .iter()
                .any(|m| m == "a ragged scar from an old wound");
            if has_permanent && !has_scar_text {
                app.distinguishing_marks
                    .push("a ragged scar from an old wound".to_string());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// update_combat_marker system (§4.2 InCombat)
// ---------------------------------------------------------------------------

/// Author the `InCombat` ZST on living cats whose `CurrentAction` is an
/// active fight step (`Action::Fight` with a resolved target entity);
/// remove it otherwise.
///
/// **Predicate** — `current.action == Action::Fight &&
/// current.target_entity.is_some()`. Mirrors the fight-collection probe
/// in `resolve_combat` (this file, ~line 90), so a cat that
/// `resolve_combat` would treat as "currently fighting" carries the
/// marker. Transition-only writes — idempotent on steady-state ticks.
///
/// **v1 scope** — covers only "actively in a fight step". The §4.2
/// rustdoc on `markers::InCombat` also names "hostile-adjacent" as a
/// trigger, but spotting hostile-adjacency requires the
/// species-attenuated detection range that `HasThreatNearby`
/// (`update_threat_proximity_markers`) is also waiting on. Both are
/// deferred together to a sensing-batch follow-up so the predicate
/// stays a single source of truth.
///
/// **Ordering** — registered in Chain 2a alongside the other §4.2 /
/// §4.3 marker authors, before the GOAP scoring pipeline runs, so the
/// `MarkerSnapshot` population in `evaluate_dispositions` and
/// `evaluate_and_plan` sees the freshly-authored ZST.
///
/// **Lifecycle** — `Dead` cats are filtered out so no marker is
/// authored on corpses during the narrative grace-period window
/// before `cleanup_dead`.
pub fn update_combat_marker(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            &CurrentAction,
            Has<crate::components::markers::InCombat>,
        ),
        Without<Dead>,
    >,
) {
    use crate::components::markers::InCombat;
    for (entity, current, has_marker) in cats.iter() {
        let in_combat = current.action == Action::Fight && current.target_entity.is_some();
        match (in_combat, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(InCombat);
            }
            (false, true) => {
                commands.entity(entity).remove::<InCombat>();
            }
            _ => {}
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
    fn classify_damage_tier_thresholds() {
        let c = &SimConstants::default().combat;
        assert!(classify_damage_for_narrative(0.02, c).is_none());
        assert_eq!(
            classify_damage_for_narrative(0.05, c),
            Some(DamageTier::Minor)
        );
        assert_eq!(
            classify_damage_for_narrative(0.15, c),
            Some(DamageTier::Moderate)
        );
        assert_eq!(
            classify_damage_for_narrative(0.30, c),
            Some(DamageTier::Severe)
        );
    }

    // ----- 477 — equipment armor-reduction read site -----

    #[test]
    fn armor_reduces_blunt_damage_and_records_trace_row() {
        use crate::components::equipment_effects::EquipmentModifiers;
        use crate::resources::trace_log::{FocalResolverSink, FocalScoreCapture, FocalTraceTarget};
        use bevy_ecs::system::SystemState;

        let c = &SimConstants::default().combat;
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(bevy_ecs::message::Messages::<BodyPartInjury>::default());
        let mut writer_state = SystemState::<MessageWriter<BodyPartInjury>>::new(&mut world);

        let mut model = CatBodyModel::default();
        let mut rng = test_rng();
        let mut activation = SystemActivation::default();

        let cat = Entity::PLACEHOLDER;
        let em = EquipmentModifiers {
            weapon: None,
            armor_blunt_reduction: 0.30,
            armor_pierce_reduction: 0.0,
            ranged_enabled: false,
            detection_visual_mask: 0.0,
            noise_level: crate::components::equipment::NoiseClass::Silent,
        };
        let capture = FocalScoreCapture::default();
        let target = FocalTraceTarget {
            name: "focal".into(),
            entity: Some(cat),
        };
        let sink = FocalResolverSink::new(Some(&capture), Some(&target), 42).unwrap();

        let raw_damage = 0.30_f32;
        {
            let mut writer = writer_state.get_mut(&mut world);
            damage_to_body_part(
                cat,
                &mut model,
                raw_damage,
                42,
                InjurySource::WildlifeCombat,
                c,
                &mut rng,
                &mut writer,
                &mut activation,
                Some(&em),
                Some(&sink),
            );
        }
        writer_state.apply(&mut world);

        // The body model + emitted message must reflect the *reduced*
        // damage (0.30 × (1 − 0.30) = 0.21), not the raw 0.30.
        let messages = world.resource::<bevy_ecs::message::Messages<BodyPartInjury>>();
        let mut cursor = messages.get_cursor();
        let msg = cursor
            .read(messages)
            .next()
            .expect("a BodyPartInjury message");
        assert!(
            (msg.tissue_damage_delta - 0.21).abs() < 1e-5,
            "expected reduced damage 0.21, got {}",
            msg.tissue_damage_delta
        );

        // The reduction must surface as a named resolver-trace row.
        let inner = capture.drain();
        let row = inner
            .resolver_modifiers
            .iter()
            .find(|r| r.resolver == "damage_to_body_part" && r.modifier == "armor.reduction")
            .expect("an armor.reduction resolver-trace row");
        assert!((row.pre - 0.30).abs() < 1e-5);
        assert!((row.post - 0.21).abs() < 1e-5);
    }

    #[test]
    fn magic_damage_bypasses_armor() {
        use crate::components::body_zones::WoundKind;
        use crate::components::equipment_effects::EquipmentModifiers;
        // Festering / magic damage routes through `Unarmored` regardless
        // of worn armor (deliberate doctrine call).
        let em = EquipmentModifiers {
            weapon: None,
            armor_blunt_reduction: 0.30,
            armor_pierce_reduction: 0.30,
            ranged_enabled: false,
            detection_visual_mask: 0.0,
            noise_level: crate::components::equipment::NoiseClass::Silent,
        };
        let reduced =
            armor_reduced_damage(0.50, InjurySource::MagicMisfire, WoundKind::Festering, &em);
        assert_eq!(reduced, 0.50, "magic damage must not be blunted by armor");
    }

    // ----- 095 Phase 1 — damage_to_body_part substrate tests -----

    /// Helper: a fresh SimRng with a fixed seed for deterministic
    /// part-selection rolls.
    fn test_rng() -> SimRng {
        use rand::SeedableRng;
        SimRng {
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(7),
        }
    }

    #[test]
    fn damage_to_body_part_below_threshold_skips() {
        use bevy_ecs::system::SystemState;
        let c = &SimConstants::default().combat;
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(bevy_ecs::message::Messages::<BodyPartInjury>::default());
        let mut writer_state = SystemState::<MessageWriter<BodyPartInjury>>::new(&mut world);

        let mut model = CatBodyModel::default();
        let mut rng = test_rng();
        let mut activation = SystemActivation::default();
        let result = {
            let mut writer = writer_state.get_mut(&mut world);
            damage_to_body_part(
                Entity::PLACEHOLDER,
                &mut model,
                0.01,
                10,
                InjurySource::WildlifeCombat,
                c,
                &mut rng,
                &mut writer,
                &mut activation,
                None,
                None,
            )
        };
        writer_state.apply(&mut world);
        assert!(result.is_none(), "below-threshold damage returns None");
        assert_eq!(
            model.parts.iter().filter(|p| p.tissue_damage > 0.0).count(),
            0
        );
        let messages = world.resource::<bevy_ecs::message::Messages<BodyPartInjury>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(cursor.read(messages).count(), 0);
    }

    #[test]
    fn damage_to_body_part_populates_substrate_and_emits_message() {
        use bevy_ecs::system::SystemState;
        let c = &SimConstants::default().combat;
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(bevy_ecs::message::Messages::<BodyPartInjury>::default());

        let mut model = CatBodyModel::default();
        let mut rng = test_rng();
        let mut activation = SystemActivation::default();

        let mut writer_state = SystemState::<MessageWriter<BodyPartInjury>>::new(&mut world);
        let result = {
            let mut writer = writer_state.get_mut(&mut world);
            damage_to_body_part(
                Entity::PLACEHOLDER,
                &mut model,
                0.30, // above injury_severe_threshold (0.25)
                42,
                InjurySource::ShadowFoxAmbush,
                c,
                &mut rng,
                &mut writer,
                &mut activation,
                None,
                None,
            )
        };
        writer_state.apply(&mut world);

        let (part, condition) = result.expect("severe damage should produce a body part injury");

        // Tissue damage went somewhere in the Fox targeting table — Phase 1
        // collapses ShadowFoxAmbush to the Fox table.
        assert!(
            model.parts[part.index()].tissue_damage >= 0.30,
            "selected part should carry the damage delta"
        );

        // Damage of 0.30 is in the Wounded tier per default thresholds
        // (0.26 lower bound), unless drift carries it higher.
        use crate::components::body_zones::PartCondition;
        assert!(
            condition >= PartCondition::Wounded,
            "0.30 damage exceeds Wounded threshold"
        );

        // Reading messages out of the world confirms the L1 emit.
        let messages = world.resource::<bevy_ecs::message::Messages<BodyPartInjury>>();
        let mut cursor = messages.get_cursor();
        let collected: Vec<_> = cursor.read(messages).collect();
        assert_eq!(collected.len(), 1, "exactly one BodyPartInjury message");
        let msg = collected[0];
        assert_eq!(msg.part, part);
        assert!((msg.tissue_damage_delta - 0.30).abs() < 1e-6);
        assert_eq!(msg.condition, condition);
        assert_eq!(msg.tick, 42);

        // Feature canary recorded.
        assert!(
            activation
                .counts
                .get(&Feature::BodyPartInjury)
                .copied()
                .unwrap_or(0)
                >= 1,
            "BodyPartInjury feature should be activated"
        );
    }

    #[test]
    fn damage_to_body_part_permanent_destroyed_persists() {
        use bevy_ecs::system::SystemState;
        let c = &SimConstants::default().combat;
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(bevy_ecs::message::Messages::<BodyPartInjury>::default());

        let mut model = CatBodyModel::default();
        let mut rng = test_rng();
        let mut activation = SystemActivation::default();

        // Force the haunches to be Destroyed by applying directly (the
        // weighted target picker is fox-table-biased, but we don't want
        // to depend on RNG selection here).
        model.apply_damage(
            BodyPart::Haunches,
            0.95,
            &c.body_zone_condition_thresholds,
            &c.body_zone_permanent_at_destroyed,
        );
        assert_eq!(
            model.part(BodyPart::Haunches).condition,
            crate::components::body_zones::PartCondition::Destroyed
        );
        assert!(model.part(BodyPart::Haunches).permanent);

        // Subsequent healing must not undo the permanent destroyed flag.
        let decrements = [1.0_f32; crate::components::body_zones::CAT_BODY_PART_COUNT];
        model.heal_tick(&decrements, &c.body_zone_condition_thresholds);
        assert!(
            model.part(BodyPart::Haunches).permanent,
            "haunches must stay permanently destroyed"
        );

        // No spurious message generation.
        let mut writer_state = SystemState::<MessageWriter<BodyPartInjury>>::new(&mut world);
        {
            let mut writer = writer_state.get_mut(&mut world);
            // Apply a tiny ear hit to verify the message path still works.
            damage_to_body_part(
                Entity::PLACEHOLDER,
                &mut model,
                0.10,
                100,
                InjurySource::FoxConfrontation,
                c,
                &mut rng,
                &mut writer,
                &mut activation,
                None,
                None,
            );
        }
        writer_state.apply(&mut world);
        let messages = world.resource::<bevy_ecs::message::Messages<BodyPartInjury>>();
        let mut cursor = messages.get_cursor();
        let collected: Vec<_> = cursor.read(messages).collect();
        assert_eq!(collected.len(), 1);
    }

    #[test]
    fn heal_injuries_advances_tissue_healing_and_restores_hp() {
        // 095 Phase 1 Stage B — `Health.injuries`-based healing retired.
        // Verify the per-part tissue healing tick restores Health.current
        // proportionally to the pain delta freed by healing.
        use bevy_ecs::schedule::Schedule;

        let c = SimConstants::default();
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 200,
            paused: false,
            speed: crate::resources::time::SimSpeed::Normal,
        });
        world.insert_resource(c.clone());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(crate::resources::time::TimeScale::from_config(
            &crate::resources::time::SimConfig::default(),
            16.6667,
        ));

        // Construct a cat with a Wounded ear (tissue 0.3) so heal_tick
        // decrements meaningfully on the soft-tissue rate.
        let mut model = CatBodyModel::default();
        model.apply_damage(
            BodyPart::Ears,
            0.3,
            &c.combat.body_zone_condition_thresholds,
            &c.combat.body_zone_permanent_at_destroyed,
        );
        let initial_hp = 0.5;
        let entity = world
            .spawn((
                Health {
                    current: initial_hp,
                    max: 1.0,
                    total_starvation_damage: 0.0,
                },
                model,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(heal_injuries);
        schedule.run(&mut world);

        let after = world.get::<CatBodyModel>(entity).unwrap();
        let healed_tissue = after.part(BodyPart::Ears).tissue_damage;
        assert!(
            healed_tissue < 0.3,
            "ear tissue should decrement after a heal tick (was 0.3, now {healed_tissue})"
        );

        let post_health = world.get::<Health>(entity).unwrap();
        assert!(
            post_health.current > initial_hp,
            "Health.current should rise as pain-fraction recovers (was {initial_hp}, now {})",
            post_health.current
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

    // -----------------------------------------------------------------------
    // update_combat_marker tests (§4.2 InCombat)
    // -----------------------------------------------------------------------

    use crate::components::markers::InCombat;
    use crate::components::physical::DeathCause;
    use bevy_ecs::schedule::Schedule;

    fn setup_marker_world() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_combat_marker);
        (world, schedule)
    }

    fn spawn_cat_with_action(world: &mut World, current: CurrentAction) -> Entity {
        world.spawn(current).id()
    }

    fn has_in_combat(world: &World, entity: Entity) -> bool {
        world.get::<InCombat>(entity).is_some()
    }

    #[test]
    fn fight_with_target_inserts_marker() {
        let (mut world, mut schedule) = setup_marker_world();
        let target = world.spawn_empty().id();
        let cat = spawn_cat_with_action(
            &mut world,
            CurrentAction {
                action: Action::Fight,
                target_entity: Some(target),
                ..CurrentAction::default()
            },
        );
        schedule.run(&mut world);
        assert!(
            has_in_combat(&world, cat),
            "cat in active fight step should carry InCombat marker"
        );
    }

    #[test]
    fn fight_without_target_no_marker() {
        let (mut world, mut schedule) = setup_marker_world();
        let cat = spawn_cat_with_action(
            &mut world,
            CurrentAction {
                action: Action::Fight,
                target_entity: None,
                ..CurrentAction::default()
            },
        );
        schedule.run(&mut world);
        assert!(
            !has_in_combat(&world, cat),
            "Action::Fight without a target_entity should not carry InCombat marker"
        );
    }

    #[test]
    fn non_fight_actions_no_marker() {
        let (mut world, mut schedule) = setup_marker_world();
        let target = world.spawn_empty().id();
        let cases = [
            Action::Hunt,
            Action::Forage,
            Action::Idle,
            Action::Sleep,
            Action::Flee,
        ];
        for action in cases {
            let cat = spawn_cat_with_action(
                &mut world,
                CurrentAction {
                    action,
                    target_entity: Some(target),
                    ..CurrentAction::default()
                },
            );
            schedule.run(&mut world);
            assert!(
                !has_in_combat(&world, cat),
                "Action::{action:?} should not carry InCombat marker"
            );
        }
    }

    #[test]
    fn fight_to_idle_transition_removes_marker() {
        let (mut world, mut schedule) = setup_marker_world();
        let target = world.spawn_empty().id();
        let cat = spawn_cat_with_action(
            &mut world,
            CurrentAction {
                action: Action::Fight,
                target_entity: Some(target),
                ..CurrentAction::default()
            },
        );
        schedule.run(&mut world);
        assert!(has_in_combat(&world, cat));

        // Wildlife dies / fight resolves → action flips to Idle.
        let mut current = world.get_mut::<CurrentAction>(cat).unwrap();
        current.action = Action::Idle;
        current.target_entity = None;
        schedule.run(&mut world);
        assert!(
            !has_in_combat(&world, cat),
            "marker should drop on fight resolution"
        );
    }

    #[test]
    fn idempotent_no_flap_on_steady_fight() {
        let (mut world, mut schedule) = setup_marker_world();
        let target = world.spawn_empty().id();
        let cat = spawn_cat_with_action(
            &mut world,
            CurrentAction {
                action: Action::Fight,
                target_entity: Some(target),
                ..CurrentAction::default()
            },
        );
        schedule.run(&mut world);
        assert!(has_in_combat(&world, cat));
        schedule.run(&mut world);
        assert!(
            has_in_combat(&world, cat),
            "steady-state fight should not flap marker"
        );
    }

    #[test]
    fn dead_cats_are_skipped() {
        let (mut world, mut schedule) = setup_marker_world();
        let target = world.spawn_empty().id();
        let cat = world
            .spawn((
                CurrentAction {
                    action: Action::Fight,
                    target_entity: Some(target),
                    ..CurrentAction::default()
                },
                Dead {
                    tick: 0,
                    cause: DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(
            !has_in_combat(&world, cat),
            "dead cats should not receive InCombat marker even mid-Fight"
        );
    }

    #[test]
    fn mixed_population_independent_authoring() {
        let (mut world, mut schedule) = setup_marker_world();
        let target = world.spawn_empty().id();
        let fighter = spawn_cat_with_action(
            &mut world,
            CurrentAction {
                action: Action::Fight,
                target_entity: Some(target),
                ..CurrentAction::default()
            },
        );
        let hunter = spawn_cat_with_action(
            &mut world,
            CurrentAction {
                action: Action::Hunt,
                target_entity: Some(target),
                ..CurrentAction::default()
            },
        );
        let idler = spawn_cat_with_action(&mut world, CurrentAction::default());

        schedule.run(&mut world);

        assert!(has_in_combat(&world, fighter));
        assert!(!has_in_combat(&world, hunter));
        assert!(!has_in_combat(&world, idler));
    }

    // -----------------------------------------------------------------------
    // §9.2 / ticket 049 cat-on-cat banishment branch tests
    // -----------------------------------------------------------------------

    /// The `Banished` marker is a ZST with a stable KEY. Inserting it
    /// onto a cat entity flips `Has<Banished>` true and keeps the
    /// entity alive (vs the shadowfox path which despawns).
    #[test]
    fn banished_marker_persists_on_cat() {
        let mut world = World::new();
        let cat = world.spawn(()).id();
        world
            .commands()
            .entity(cat)
            .insert(crate::components::markers::Banished);
        world.flush();
        assert!(world
            .get::<crate::components::markers::Banished>(cat)
            .is_some());
        // Marker is sticky — running a no-op schedule does not clear it.
        let mut schedule = Schedule::default();
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::Banished>(cat)
            .is_some());
    }

    /// Resolved stance with the `Banished` overlay set demotes
    /// `Same → Enemy` for a cat-on-cat observation. This is the
    /// runtime contract that the §9.3 prefilter exploits to drop a
    /// banished cat from `socialize_target` candidate sets.
    #[test]
    fn banished_overlay_demotes_same_to_enemy() {
        use crate::ai::faction::{resolve_stance, FactionStance, StanceOverlays};
        let resolved = resolve_stance(
            FactionStance::Same,
            true, // observer is a cat
            StanceOverlays {
                banished: true,
                ..Default::default()
            },
        );
        assert_eq!(resolved, FactionStance::Enemy);
    }
}
