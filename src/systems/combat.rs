use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::identity::Name;
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Injury, InjuryKind, Needs, Position};
use crate::components::skills::Skills;
use crate::components::wildlife::{WildAnimal, WildlifeAiState};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// Combat jitter
// ---------------------------------------------------------------------------

fn combat_jitter(rng: &mut impl Rng) -> f32 {
    rng.random_range(-0.02f32..0.02f32)
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
#[allow(clippy::type_complexity)]
pub fn resolve_combat(
    mut cats: Query<(
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
    ), Without<Dead>>,
    mut wildlife: Query<(Entity, &WildAnimal, &mut Health, &Position, &mut WildlifeAiState), Without<CurrentAction>>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
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

    // Count allies per target for group bonus.
    let mut ally_counts: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for fight in &fights {
        *ally_counts.entry(fight.target_entity).or_insert(0) += 1;
    }

    // Track wildlife to despawn and cats to reset.
    let mut wildlife_to_despawn: Vec<Entity> = Vec::new();
    let mut cats_to_flee: Vec<Entity> = Vec::new();
    let mut victorious_cats: Vec<(Entity, Entity)> = Vec::new(); // (cat, defeated wildlife)

    for fight in &fights {
        let ally_count = ally_counts.get(&fight.target_entity).copied().unwrap_or(1).saturating_sub(1);

        // Get wildlife data.
        let (threat_power, animal_defense, _wildlife_health_pct, wildlife_pos, wildlife_alive) = {
            if let Ok((_, animal, health, pos, _)) = wildlife.get(fight.target_entity) {
                (animal.threat_power, animal.defense, health.current / health.max.max(0.01), *pos, health.current > 0.0)
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
        let (cat_pos, cat_boldness, _cat_loyalty, _cat_health_pct, combat_effective, cat_name) = {
            if let Ok((_, _, health, _, skills, personality, pos, name, _, _)) = cats.get(fight.cat_entity) {
                let ce = skills.combat + skills.hunting * 0.3;
                let hp = health.current / health.max.max(0.01);
                (pos.manhattan_distance(&wildlife_pos), personality.boldness, personality.loyalty, hp, ce, name.0.clone())
            } else {
                continue;
            }
        };

        // Must be adjacent (within 1 tile) to fight.
        if cat_pos > 1 {
            continue;
        }

        // --- Cat attacks wildlife ---
        let cat_damage = (combat_effective * cat_boldness * (1.0 + 0.2 * ally_count as f32)
            - animal_defense
            + combat_jitter(&mut rng.rng))
            .max(0.0);

        if let Ok((_, animal, mut wl_health, _, mut ai_state)) = wildlife.get_mut(fight.target_entity) {
            wl_health.current = (wl_health.current - cat_damage).max(0.0);

            let species_name = animal.species.name();

            // Narrative: cat attacks.
            if rng.rng.random::<f32>() < 0.15 {
                let text = format!("{cat_name} rakes the {species_name} across the muzzle.");
                log.push(time.tick, text, NarrativeTier::Action);
            }

            // Check if wildlife is killed.
            if wl_health.current <= 0.0 {
                let text = format!("The {species_name} crumples. {cat_name} stands over it, breathing hard.");
                log.push(time.tick, text, NarrativeTier::Significant);
                wildlife_to_despawn.push(fight.target_entity);
                victorious_cats.push((fight.cat_entity, fight.target_entity));
                continue;
            }

            // Wildlife morale check.
            let wl_health_pct_now = wl_health.current / wl_health.max.max(0.01);
            let outnumbered = (ally_count + 1) >= 3;
            if wl_health_pct_now <= 0.3 || outnumbered {
                // Wildlife flees.
                let text = format!("The {species_name} breaks away, outnumbered.");
                log.push(time.tick, text, NarrativeTier::Action);
                // Set wildlife to flee toward nearest edge.
                let flee_dx = if wildlife_pos.x < 40 { -1 } else { 1 };
                let flee_dy = if wildlife_pos.y < 30 { -1 } else { 1 };
                *ai_state = WildlifeAiState::Fleeing { dx: flee_dx, dy: flee_dy };
                wildlife_to_despawn.push(fight.target_entity); // will despawn at edge
                victorious_cats.push((fight.cat_entity, fight.target_entity));
                continue;
            }
        }

        // --- Wildlife attacks cat ---
        let wildlife_damage = (threat_power + combat_jitter(&mut rng.rng)).max(0.0);

        if let Ok((_, _current, mut cat_health, _needs, mut skills, personality, _, name, mut memory, mut mood)) =
            cats.get_mut(fight.cat_entity)
        {
            cat_health.current = (cat_health.current - wildlife_damage).max(0.0);

            // Apply injury based on damage.
            let injury = damage_to_injury(wildlife_damage, time.tick);
            if let Some(inj) = injury {
                let health_penalty = match inj.kind {
                    InjuryKind::Minor => 0.03,
                    InjuryKind::Moderate => 0.08,
                    InjuryKind::Severe => 0.15,
                };
                cat_health.current = (cat_health.current - health_penalty).max(0.0);

                // Narrative for injuries.
                if matches!(inj.kind, InjuryKind::Moderate | InjuryKind::Severe) {
                    let text = format!("{} is knocked aside but scrambles back.", name.0);
                    log.push(time.tick, text, NarrativeTier::Action);
                }

                memory.remember(MemoryEntry {
                    event_type: MemoryType::Injury,
                    location: None,
                    involved: vec![fight.target_entity],
                    tick: time.tick,
                    strength: match inj.kind {
                        InjuryKind::Minor => 0.5,
                        InjuryKind::Moderate => 0.8,
                        InjuryKind::Severe => 1.0,
                    },
                    firsthand: true,
                });

                cat_health.injuries.push(inj);
            }

            // Combat skill growth.
            skills.combat += skills.growth_rate() * 0.02;

            // Cat morale check.
            let cat_hp = cat_health.current / cat_health.max.max(0.01);
            let morale = cat_hp * 0.4
                + personality.boldness * 0.3
                + ally_count as f32 * 0.1
                + personality.loyalty * 0.2;
            let morale_threshold = 0.4 + combat_jitter(&mut rng.rng);

            if morale < morale_threshold {
                // Cat flees.
                cats_to_flee.push(fight.cat_entity);

                mood.modifiers.push_back(MoodModifier {
                    amount: -0.3,
                    ticks_remaining: 40,
                    source: "fled from combat".to_string(),
                });
            }
        }
    }

    // Apply victory rewards.
    for (cat_entity, _defeated) in &victorious_cats {
        if let Ok((_, mut current, _, mut needs, _, _, _, _, _memory, mut mood)) = cats.get_mut(*cat_entity) {
            needs.respect = (needs.respect + 0.1).min(1.0);
            needs.safety = (needs.safety + 0.2).min(1.0);
            current.ticks_remaining = 0; // Allow new action selection.

            mood.modifiers.push_back(MoodModifier {
                amount: 0.3,
                ticks_remaining: 50,
                source: "won a fight".to_string(),
            });
        }
    }

    // Make fleeing cats switch to Flee action.
    for cat_entity in &cats_to_flee {
        if let Ok((_, mut current, _, _, _, _, _, _, _, _)) = cats.get_mut(*cat_entity) {
            current.action = Action::Flee;
            current.ticks_remaining = 15;
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
fn damage_to_injury(damage: f32, tick: u64) -> Option<Injury> {
    if damage < 0.03 {
        return None; // Negligible scratch.
    }
    let kind = if damage < 0.1 {
        InjuryKind::Minor
    } else if damage < 0.25 {
        InjuryKind::Moderate
    } else {
        InjuryKind::Severe
    };
    Some(Injury {
        kind,
        tick_received: tick,
        healed: false,
    })
}

/// Duration in ticks for an injury to heal naturally.
pub fn heal_duration(kind: InjuryKind) -> u64 {
    match kind {
        InjuryKind::Minor => 50,
        InjuryKind::Moderate => 200,
        InjuryKind::Severe => 500,
    }
}

// ---------------------------------------------------------------------------
// Healing system
// ---------------------------------------------------------------------------

/// Per-tick healing: check each cat's injuries and heal those past their duration.
pub fn heal_injuries(
    mut query: Query<(&mut Health, Option<&mut crate::components::identity::Appearance>)>,
    time: Res<TimeState>,
) {
    for (mut health, appearance) in &mut query {
        let mut healed_severe = false;
        for injury in health.injuries.iter_mut() {
            if injury.healed {
                continue;
            }
            let duration = heal_duration(injury.kind);
            if time.tick.saturating_sub(injury.tick_received) >= duration {
                injury.healed = true;
                if injury.kind == InjuryKind::Severe {
                    healed_severe = true;
                }
            }
        }

        // Remove healed injuries.
        health.injuries.retain(|inj| !inj.healed);

        // Add scar for healed severe injuries.
        if healed_severe {
            if let Some(mut app) = appearance {
                app.distinguishing_marks.push("a ragged scar from an old wound".to_string());
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
        assert!(damage_to_injury(0.02, 0).is_none(), "negligible damage should not create injury");

        let minor = damage_to_injury(0.05, 100).unwrap();
        assert_eq!(minor.kind, InjuryKind::Minor);
        assert_eq!(minor.tick_received, 100);
        assert!(!minor.healed);

        let moderate = damage_to_injury(0.15, 200).unwrap();
        assert_eq!(moderate.kind, InjuryKind::Moderate);

        let severe = damage_to_injury(0.30, 300).unwrap();
        assert_eq!(severe.kind, InjuryKind::Severe);
    }

    #[test]
    fn heal_duration_ordering() {
        assert!(heal_duration(InjuryKind::Minor) < heal_duration(InjuryKind::Moderate));
        assert!(heal_duration(InjuryKind::Moderate) < heal_duration(InjuryKind::Severe));
    }

    #[test]
    fn heal_injuries_removes_healed() {
        use bevy_ecs::schedule::Schedule;

        let mut world = World::new();
        world.insert_resource(TimeState { tick: 200, paused: false, speed: crate::resources::time::SimSpeed::Normal });

        let entity = world.spawn(Health {
            current: 0.5,
            max: 1.0,
            injuries: vec![
                Injury { kind: InjuryKind::Minor, tick_received: 100, healed: false },
                Injury { kind: InjuryKind::Severe, tick_received: 100, healed: false },
            ],
        }).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(heal_injuries);
        schedule.run(&mut world);

        let health = world.get::<Health>(entity).unwrap();
        // Minor injury (50 ticks) at tick 100, now tick 200 = 100 ticks elapsed. Should be healed.
        // Severe injury (500 ticks) at tick 100, now tick 200 = 100 ticks elapsed. Should NOT be healed.
        assert_eq!(health.injuries.len(), 1, "minor injury should be healed and removed");
        assert_eq!(health.injuries[0].kind, InjuryKind::Severe, "severe injury should remain");
    }

    #[test]
    fn combat_effective_formula() {
        let skills = Skills {
            combat: 0.05,
            hunting: 0.5,
            ..Skills::default()
        };
        let effective = skills.combat + skills.hunting * 0.3;
        assert!((effective - 0.2).abs() < 1e-5, "combat_effective should be 0.2; got {effective}");
    }
}
