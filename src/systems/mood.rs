use bevy_ecs::prelude::*;

use crate::components::mental::{Mood, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::resources::relationships::Relationships;

// ---------------------------------------------------------------------------
// update_mood system
// ---------------------------------------------------------------------------

/// Decay mood modifiers and recompute effective valence each tick.
///
/// Modifiers count down via `ticks_remaining`; expired ones are removed.
/// Effective valence = personality baseline + positive modifiers +
/// anxiety-amplified negative modifiers, clamped to [-1.0, 1.0].
pub fn update_mood(
    mut query: Query<(&mut Mood, &Personality, &Needs), Without<Dead>>,
) {
    for (mut mood, personality, needs) in &mut query {
        // Tick down and remove expired modifiers.
        mood.modifiers.retain_mut(|m| {
            m.ticks_remaining = m.ticks_remaining.saturating_sub(1);
            m.ticks_remaining > 0
        });

        // Personality-driven baseline: optimistic cats lean positive.
        let baseline = personality.optimism * 0.4 - 0.2;

        let positive_sum: f32 = mood.modifiers.iter()
            .filter(|m| m.amount > 0.0)
            .map(|m| m.amount)
            .sum();

        let negative_sum: f32 = mood.modifiers.iter()
            .filter(|m| m.amount < 0.0)
            .map(|m| m.amount)
            .sum();

        // Anxious cats feel negative events more intensely.
        // Temper amplifies negatives when physiological needs are unmet.
        let phys = needs.physiological_satisfaction();
        let temper_amp = personality.temper * 0.3 * (1.0 - phys);
        let amplified_negative = negative_sum * (1.0 + personality.anxiety * 0.5 + temper_amp);

        mood.valence = (baseline + positive_sum + amplified_negative).clamp(-1.0, 1.0);

        // Pride: wounded pride generates per-tick negative modifier when
        // respect is critically low.
        if needs.respect < 0.3 && !mood.modifiers.iter().any(|m| m.source == "wounded pride") {
            mood.modifiers.push_back(MoodModifier {
                amount: -(personality.pride * 0.15),
                ticks_remaining: 1,
                source: "wounded pride".to_string(),
            });
        }
    }
}

/// Extend a positive mood modifier's duration based on a cat's patience.
///
/// Called at modifier creation time (not per-tick). At patience=1.0, positive
/// modifiers last 30% longer. Negative modifiers are unaffected.
pub fn patience_extend(modifier: &mut MoodModifier, patience: f32) {
    if modifier.amount > 0.0 {
        let extension = (patience * modifier.ticks_remaining as f32 * 0.3).round() as u64;
        modifier.ticks_remaining += extension;
    }
}

// ---------------------------------------------------------------------------
// mood_contagion system
// ---------------------------------------------------------------------------

/// Spread mood between nearby cats.
///
/// Each tick, cats within 3 Manhattan tiles exert emotional influence on each
/// other. Influence scales with proximity, fondness, and the source's mood
/// intensity. Applied as short-duration modifiers so it fades naturally.
pub fn mood_contagion(
    mut query: Query<(Entity, &Position, &mut Mood, &Personality), Without<Dead>>,
    relationships: Res<Relationships>,
) {
    // Read pass: snapshot all positions and valences.
    let snapshot: Vec<(Entity, Position, f32)> = query.iter()
        .map(|(e, p, m, _)| (e, *p, m.valence))
        .collect();

    // Write pass: apply contagion modifiers.
    for (entity, pos, mut mood, personality) in &mut query {
        for &(other_entity, other_pos, other_valence) in &snapshot {
            if entity == other_entity {
                continue;
            }
            let dist = pos.manhattan_distance(&other_pos);
            if dist == 0 || dist > 3 {
                continue;
            }

            let fondness = relationships
                .get(entity, other_entity)
                .map_or(0.0, |r| r.fondness);
            let fondness_weight = (fondness + 1.0) / 2.0; // map -1..1 to 0..1
            let weight = (1.0 / dist as f32) * fondness_weight * other_valence.abs();
            // Stubborn cats resist mood contagion.
            let influence = other_valence * weight * 0.002
                * (1.0 - personality.stubbornness * 0.2);

            mood.modifiers.push_back(MoodModifier {
                amount: influence,
                ticks_remaining: 5,
                source: "contagion".to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use bevy_ecs::schedule::Schedule;
    use crate::resources::relationships::Relationships;

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

    fn setup_mood_world() -> (World, Schedule) {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_mood);
        (world, schedule)
    }

    fn spawn_cat_mood(
        world: &mut World,
        personality: Personality,
        modifiers: Vec<MoodModifier>,
    ) -> Entity {
        world
            .spawn((
                Mood {
                    valence: 0.2,
                    modifiers: VecDeque::from(modifiers),
                },
                personality,
                Needs::default(),
            ))
            .id()
    }

    #[test]
    fn modifier_expires_after_countdown() {
        let (mut world, mut schedule) = setup_mood_world();
        let entity = spawn_cat_mood(
            &mut world,
            default_personality(),
            vec![MoodModifier {
                amount: 0.5,
                ticks_remaining: 3,
                source: "test".to_string(),
            }],
        );

        schedule.run(&mut world);
        assert_eq!(world.get::<Mood>(entity).unwrap().modifiers.len(), 1);

        schedule.run(&mut world);
        assert_eq!(world.get::<Mood>(entity).unwrap().modifiers.len(), 1);

        schedule.run(&mut world);
        assert_eq!(
            world.get::<Mood>(entity).unwrap().modifiers.len(),
            0,
            "modifier should be removed after 3 ticks"
        );
    }

    #[test]
    fn effective_valence_reflects_modifiers() {
        let (mut world, mut schedule) = setup_mood_world();
        let entity = spawn_cat_mood(
            &mut world,
            default_personality(),
            vec![MoodModifier {
                amount: 0.5,
                ticks_remaining: 10,
                source: "happy".to_string(),
            }],
        );

        schedule.run(&mut world);
        let mood = world.get::<Mood>(entity).unwrap();

        // baseline for optimism=0.5: 0.5*0.4 - 0.2 = 0.0
        // plus 0.5 modifier = 0.5
        assert!(
            (mood.valence - 0.5).abs() < 1e-5,
            "valence should be ~0.5; got {}",
            mood.valence
        );
    }

    #[test]
    fn anxiety_amplifies_negative_modifiers() {
        let (mut world_high, mut schedule_high) = setup_mood_world();
        let mut anxious = default_personality();
        anxious.anxiety = 1.0;
        let cat_anxious = spawn_cat_mood(
            &mut world_high,
            anxious,
            vec![MoodModifier {
                amount: -0.3,
                ticks_remaining: 10,
                source: "bad".to_string(),
            }],
        );

        let (mut world_low, mut schedule_low) = setup_mood_world();
        let mut calm = default_personality();
        calm.anxiety = 0.0;
        let cat_calm = spawn_cat_mood(
            &mut world_low,
            calm,
            vec![MoodModifier {
                amount: -0.3,
                ticks_remaining: 10,
                source: "bad".to_string(),
            }],
        );

        schedule_high.run(&mut world_high);
        schedule_low.run(&mut world_low);

        let valence_anxious = world_high.get::<Mood>(cat_anxious).unwrap().valence;
        let valence_calm = world_low.get::<Mood>(cat_calm).unwrap().valence;

        assert!(
            valence_anxious < valence_calm,
            "anxious cat should feel worse; anxious={valence_anxious}, calm={valence_calm}"
        );
    }

    #[test]
    fn optimistic_baseline_is_positive() {
        let (mut world, mut schedule) = setup_mood_world();
        let mut personality = default_personality();
        personality.optimism = 1.0;
        let entity = spawn_cat_mood(&mut world, personality, vec![]);

        schedule.run(&mut world);
        let valence = world.get::<Mood>(entity).unwrap().valence;

        // baseline: 1.0 * 0.4 - 0.2 = 0.2
        assert!(
            valence > 0.0,
            "optimistic cat should have positive baseline; got {valence}"
        );
    }

    #[test]
    fn pessimistic_baseline_is_negative() {
        let (mut world, mut schedule) = setup_mood_world();
        let mut personality = default_personality();
        personality.optimism = 0.0;
        let entity = spawn_cat_mood(&mut world, personality, vec![]);

        schedule.run(&mut world);
        let valence = world.get::<Mood>(entity).unwrap().valence;

        // baseline: 0.0 * 0.4 - 0.2 = -0.2
        assert!(
            valence < 0.0,
            "pessimistic cat should have negative baseline; got {valence}"
        );
    }

    #[test]
    fn valence_clamped_to_bounds() {
        let (mut world, mut schedule) = setup_mood_world();
        let entity = spawn_cat_mood(
            &mut world,
            default_personality(),
            vec![MoodModifier {
                amount: 5.0,
                ticks_remaining: 10,
                source: "extreme".to_string(),
            }],
        );

        schedule.run(&mut world);
        let valence = world.get::<Mood>(entity).unwrap().valence;
        assert_eq!(valence, 1.0, "valence should clamp at 1.0; got {valence}");

        // Replace with extreme negative
        let mood = world.get_mut::<Mood>(entity).unwrap().into_inner();
        mood.modifiers.clear();
        mood.modifiers.push_back(MoodModifier {
            amount: -5.0,
            ticks_remaining: 10,
            source: "extreme".to_string(),
        });

        schedule.run(&mut world);
        let valence = world.get::<Mood>(entity).unwrap().valence;
        assert_eq!(valence, -1.0, "valence should clamp at -1.0; got {valence}");
    }

    // --- Contagion tests ---

    fn setup_contagion_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(mood_contagion);
        (world, schedule)
    }

    #[test]
    fn contagion_spreads_between_nearby_cats() {
        let (mut world, mut schedule) = setup_contagion_world();

        // Happy cat at (5, 5)
        let source = world.spawn((
            Mood {
                valence: 0.8,
                modifiers: VecDeque::new(),
            },
            Position::new(5, 5),
            default_personality(),
        )).id();

        // Neutral cat at (6, 5) — 1 tile away
        let receiver = world
            .spawn((
                Mood::default(),
                Position::new(6, 5),
                default_personality(),
            ))
            .id();

        // Give them a relationship with positive fondness.
        world.resource_mut::<Relationships>()
            .get_or_insert(source, receiver).fondness = 0.5;

        schedule.run(&mut world);

        let mood = world.get::<Mood>(receiver).unwrap();
        assert!(
            !mood.modifiers.is_empty(),
            "nearby cat should receive contagion modifier"
        );
        assert!(
            mood.modifiers.iter().any(|m| m.amount > 0.0),
            "modifier from happy cat should be positive"
        );
    }

    #[test]
    fn contagion_does_not_spread_beyond_3_tiles() {
        let (mut world, mut schedule) = setup_contagion_world();

        // Happy cat at (0, 0)
        world.spawn((
            Mood {
                valence: 0.8,
                modifiers: VecDeque::new(),
            },
            Position::new(0, 0),
            default_personality(),
        ));

        // Distant cat at (4, 0) — 4 tiles away
        let receiver = world
            .spawn((
                Mood::default(),
                Position::new(4, 0),
                default_personality(),
            ))
            .id();

        schedule.run(&mut world);

        let mood = world.get::<Mood>(receiver).unwrap();
        let contagion_mods: Vec<_> = mood
            .modifiers
            .iter()
            .filter(|m| m.source == "contagion")
            .collect();
        assert!(
            contagion_mods.is_empty(),
            "cat beyond 3 tiles should not receive contagion"
        );
    }

    #[test]
    fn contagion_stronger_with_high_fondness() {
        // Cat pair with high fondness vs low fondness, same distance.
        let (mut world_high, mut schedule_high) = setup_contagion_world();

        let src_h = world_high.spawn((
            Mood { valence: 0.8, modifiers: VecDeque::new() },
            Position::new(5, 5),
            default_personality(),
        )).id();
        let rcv_h = world_high.spawn((
            Mood::default(),
            Position::new(6, 5),
            default_personality(),
        )).id();
        world_high.resource_mut::<Relationships>()
            .get_or_insert(src_h, rcv_h).fondness = 0.9;

        let (mut world_low, mut schedule_low) = setup_contagion_world();

        let src_l = world_low.spawn((
            Mood { valence: 0.8, modifiers: VecDeque::new() },
            Position::new(5, 5),
            default_personality(),
        )).id();
        let rcv_l = world_low.spawn((
            Mood::default(),
            Position::new(6, 5),
            default_personality(),
        )).id();
        world_low.resource_mut::<Relationships>()
            .get_or_insert(src_l, rcv_l).fondness = -0.8;

        schedule_high.run(&mut world_high);
        schedule_low.run(&mut world_low);

        let influence_high: f32 = world_high.get::<Mood>(rcv_h).unwrap()
            .modifiers.iter().map(|m| m.amount).sum();
        let influence_low: f32 = world_low.get::<Mood>(rcv_l).unwrap()
            .modifiers.iter().map(|m| m.amount).sum();

        assert!(
            influence_high > influence_low,
            "high fondness should produce stronger contagion; high={influence_high}, low={influence_low}"
        );
    }
}
