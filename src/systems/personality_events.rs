use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::coordination::ActiveDirective;
use crate::components::identity::{Age, Gender, Name};
use crate::components::mental::{Mood, MoodModifier, PrideCooldown};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::events::personality::{DirectiveRefused, PlayInitiated, PrideCrisis, TemperFlared};
use crate::resources::map::TileMap;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, TemplateRegistry, VariableContext,
};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::{DayPhase, Season, SimConfig, TimeState};
use crate::resources::weather::WeatherState;
use crate::systems::mood::patience_extend;

// ---------------------------------------------------------------------------
// emit_personality_events
// ---------------------------------------------------------------------------

/// Check trigger conditions and emit personality events via Commands::trigger.
///
/// Runs each tick after mood/needs updates. Each event type has its own
/// trigger condition involving personality thresholds + situational state.
#[allow(clippy::type_complexity)]
pub fn emit_personality_events(
    mut commands: Commands,
    time: Res<TimeState>,
    mut rng: ResMut<SimRng>,
    cats: Query<
        (
            Entity,
            &Personality,
            &Needs,
            &Mood,
            &CurrentAction,
            &Position,
            Option<&ActiveDirective>,
            Option<&PrideCooldown>,
        ),
        Without<Dead>,
    >,
) {
    for (entity, personality, needs, mood, current, _pos, directive, pride_cd) in &cats {
        let phys = needs.physiological_satisfaction();

        // --- TemperFlared ---
        // High temper + unmet needs + bad mood = chance of outburst.
        if phys < 0.4 && mood.valence < -0.3 {
            let chance = personality.temper * 0.08;
            if rng.rng.random::<f32>() < chance {
                commands.trigger(TemperFlared {
                    cat: entity,
                    target: None, // Handler will find nearest cat.
                });
            }
        }

        // --- DirectiveRefused ---
        // Stubborn cat with active directive may refuse it.
        if let Some(dir) = directive {
            if personality.stubbornness > 0.7 {
                let chance = (personality.stubbornness - 0.5) * 0.6;
                if rng.rng.random::<f32>() < chance {
                    commands.trigger(DirectiveRefused {
                        cat: entity,
                        coordinator: dir.coordinator,
                    });
                }
            }
        }

        // --- PlayInitiated ---
        // Playful cat socializing in a good mood may start a game.
        if current.action == Action::Socialize
            && personality.playfulness > 0.6
            && mood.valence > 0.0
        {
            let chance = personality.playfulness * 0.1;
            if rng.rng.random::<f32>() < chance {
                commands.trigger(PlayInitiated { cat: entity });
            }
        }

        // --- PrideCrisis ---
        // Proud cat with critically low respect, on a 100-tick cooldown.
        if needs.respect < 0.2 && personality.pride > 0.6 {
            let on_cooldown = pride_cd
                .as_ref()
                .and_then(|cd| cd.last_pride_crisis_tick)
                .is_some_and(|last| time.tick.saturating_sub(last) < 100);
            if !on_cooldown {
                commands.trigger(PrideCrisis { cat: entity });
                // Update cooldown (requires mutable access — handled via command).
                commands.entity(entity).insert(PrideCooldown {
                    last_pride_crisis_tick: Some(time.tick),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Observer registration
// ---------------------------------------------------------------------------

/// Register all personality event observers on the app.
///
/// Called from the simulation plugin. Observers handle cascade effects:
/// mood modifiers, fondness changes, and narrative entries.
pub fn register_observers(app: &mut bevy::prelude::App) {
    app.add_observer(on_temper_flared);
    app.add_observer(on_directive_refused);
    app.add_observer(on_play_initiated);
    app.add_observer(on_pride_crisis);
}

// ---------------------------------------------------------------------------
// Observers (cascade handlers)
// ---------------------------------------------------------------------------

/// TemperFlared cascade: fondness penalty to nearest cat, mood hit on target,
/// narrative entry.
fn on_temper_flared(
    trigger: On<TemperFlared>,
    cats: Query<(Entity, &Position, &Name), Without<Dead>>,
    mut moods: Query<&mut Mood>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
) {
    let event = trigger.event();
    let Ok((_, cat_pos, cat_name)) = cats.get(event.cat) else {
        return;
    };
    let cat_pos = *cat_pos;
    let cat_name = cat_name.0.clone();

    // Find nearest other cat within 3 tiles.
    let mut nearest: Option<(Entity, i32, String)> = None;
    for (other, other_pos, other_name) in &cats {
        if other == event.cat {
            continue;
        }
        let dist = cat_pos.manhattan_distance(other_pos);
        if dist > 0 && dist <= 3 && (nearest.is_none() || dist < nearest.as_ref().unwrap().1) {
            nearest = Some((other, dist, other_name.0.clone()));
        }
    }

    if let Some((target, _, target_name)) = nearest {
        // Fondness penalty.
        relationships.modify_fondness(event.cat, target, -0.05);

        // Mood hit on target.
        if let Ok(mut target_mood) = moods.get_mut(target) {
            target_mood.modifiers.push_back(MoodModifier {
                amount: -0.2,
                ticks_remaining: 20,
                source: format!("snapped at by {cat_name}"),
            });
        }

        log.push(
            time.tick,
            format!("{cat_name} hisses at {target_name} for no reason anyone can name."),
            NarrativeTier::Action,
        );
    }
}

/// DirectiveRefused cascade: coordinator mood penalty, loyal bystanders
/// resent the refusal, fondness drops.
fn on_directive_refused(
    trigger: On<DirectiveRefused>,
    cats: Query<(Entity, &Position, &Personality, &Name), Without<Dead>>,
    mut moods: Query<&mut Mood>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    mut commands: Commands,
) {
    let event = trigger.event();
    let Ok((_, cat_pos, _, cat_name)) = cats.get(event.cat) else {
        return;
    };
    let cat_pos = *cat_pos;
    let cat_name = cat_name.0.clone();

    // Coordinator mood penalty.
    if let Ok(mut coord_mood) = moods.get_mut(event.coordinator) {
        coord_mood.modifiers.push_back(MoodModifier {
            amount: -0.15,
            ticks_remaining: 20,
            source: format!("directive ignored by {cat_name}"),
        });
    }
    relationships.modify_fondness(event.coordinator, event.cat, -0.03);

    // Loyal bystanders within 5 tiles who like the coordinator.
    for (other, other_pos, other_pers, _) in &cats {
        if other == event.cat || other == event.coordinator {
            continue;
        }
        if cat_pos.manhattan_distance(other_pos) > 5 {
            continue;
        }
        if other_pers.loyalty > 0.5 {
            let fondness_to_coord = relationships
                .get(other, event.coordinator)
                .map_or(0.0, |r| r.fondness);
            if fondness_to_coord > 0.3 {
                if let Ok(mut bystander_mood) = moods.get_mut(other) {
                    bystander_mood.modifiers.push_back(MoodModifier {
                        amount: -0.08,
                        ticks_remaining: 15,
                        source: format!("saw {cat_name} ignore the coordinator"),
                    });
                }
                relationships.modify_fondness(other, event.cat, -0.01);
            }
        }
    }

    // Remove the active directive (the refusal is the point).
    commands.entity(event.cat).remove::<ActiveDirective>();

    // Narrative.
    let coord_name = cats
        .get(event.coordinator)
        .map(|(_, _, _, n)| n.0.clone())
        .unwrap_or_else(|_| "the coordinator".to_string());
    log.push(
        time.tick,
        format!(
            "{coord_name} calls for {cat_name} to join. {cat_name} flicks an ear and goes back to what they were doing."
        ),
        NarrativeTier::Action,
    );
}

/// PlayInitiated cascade: mood boost to nearby cats, template-driven narrative.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn on_play_initiated(
    trigger: On<PlayInitiated>,
    cats: Query<(Entity, &Position, &Personality, &Name, &Gender, &Age), Without<Dead>>,
    needs_q: Query<&Needs, Without<Dead>>,
    mut moods: Query<&mut Mood>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    map: Res<TileMap>,
    registry: Option<Res<TemplateRegistry>>,
    mut rng: ResMut<SimRng>,
) {
    let event = trigger.event();
    let Ok((_, cat_pos, personality, cat_name, gender, age)) = cats.get(event.cat) else {
        return;
    };
    let cat_pos = *cat_pos;
    let cat_name = cat_name.0.clone();
    let gender = *gender;
    let life_stage = age.stage(time.tick, config.ticks_per_season);

    let mut play_partner: Option<String> = None;
    for (other, other_pos, other_pers, other_name, _, _) in &cats {
        if other == event.cat {
            continue;
        }
        if cat_pos.manhattan_distance(other_pos) > 4 {
            continue;
        }
        // Mood boost to all nearby.
        if let Ok(mut other_mood) = moods.get_mut(other) {
            let mut modifier = MoodModifier {
                amount: 0.1,
                ticks_remaining: 15,
                source: "watched play nearby".to_string(),
            };
            patience_extend(&mut modifier, other_pers.patience, &constants.mood);
            other_mood.modifiers.push_back(modifier);
        }
        if play_partner.is_none() {
            play_partner = Some(other_name.0.clone());
        }
    }

    // Build template context from available state.
    let day_phase = DayPhase::from_tick(time.tick, &config);
    let season = Season::from_tick(time.tick, &config);
    let terrain = if map.in_bounds(cat_pos.x, cat_pos.y) {
        map.get(cat_pos.x, cat_pos.y).terrain
    } else {
        crate::resources::map::Terrain::Grass
    };
    let mood_bucket = moods
        .get(event.cat)
        .map(|m| MoodBucket::from_valence(m.valence))
        .unwrap_or(MoodBucket::Neutral);
    let needs = needs_q
        .get(event.cat)
        .cloned()
        .unwrap_or_else(|_| Needs::default());

    let has_partner = play_partner.is_some();
    let event_tag = if has_partner {
        "play_social"
    } else {
        "play_solo"
    };

    let ctx = TemplateContext {
        action: Action::Socialize,
        day_phase,
        season,
        weather: weather.current,
        mood_bucket,
        life_stage,
        has_target: has_partner,
        terrain,
        event: Some(event_tag.into()),
    };
    let var_ctx = VariableContext {
        name: &cat_name,
        gender,
        weather: weather.current,
        day_phase,
        season,
        life_stage,
        fur_color: "unknown",
        other: play_partner.as_deref(),
        prey: None,
        item: None,
        quality: None,
    };

    let (fallback, fallback_tier) = if let Some(ref partner) = play_partner {
        (
            format!("A game breaks out. {cat_name} bats a pinecone toward {partner}."),
            NarrativeTier::Action,
        )
    } else {
        (
            format!("{cat_name} chases their own tail, briefly entertained."),
            NarrativeTier::Micro,
        )
    };

    emit_event_narrative(
        registry.as_deref(),
        &mut log,
        time.tick,
        fallback,
        fallback_tier,
        &ctx,
        &var_ctx,
        personality,
        &needs,
        &mut rng.rng,
    );
}

/// PrideCrisis cascade: narrative entry. Status-seeking boost is handled via
/// the wounded pride mood modifier in update_mood (already implemented).
fn on_pride_crisis(
    trigger: On<PrideCrisis>,
    cats: Query<&Name, Without<Dead>>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
) {
    let event = trigger.event();
    let Ok(name) = cats.get(event.cat) else {
        return;
    };
    log.push(
        time.tick,
        format!(
            "{}'s tail lashes. Nobody seems to notice what {} has done for this colony.",
            name.0, name.0
        ),
        NarrativeTier::Action,
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patience_extend_positive_modifier() {
        let mc = &crate::resources::SimConstants::default().mood;
        let mut m = MoodModifier {
            amount: 0.3,
            ticks_remaining: 50,
            source: "test".to_string(),
        };
        patience_extend(&mut m, 1.0, mc);
        // 50 + (1.0 * 50 * 0.3).round() = 50 + 15 = 65
        assert_eq!(m.ticks_remaining, 65);
    }

    #[test]
    fn patience_extend_does_not_affect_negative() {
        let mc = &crate::resources::SimConstants::default().mood;
        let mut m = MoodModifier {
            amount: -0.3,
            ticks_remaining: 50,
            source: "test".to_string(),
        };
        patience_extend(&mut m, 1.0, mc);
        assert_eq!(m.ticks_remaining, 50);
    }

    #[test]
    fn patience_extend_zero_patience() {
        let mc = &crate::resources::SimConstants::default().mood;
        let mut m = MoodModifier {
            amount: 0.3,
            ticks_remaining: 50,
            source: "test".to_string(),
        };
        patience_extend(&mut m, 0.0, mc);
        assert_eq!(m.ticks_remaining, 50);
    }
}
