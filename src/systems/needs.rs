use bevy_ecs::prelude::*;

use crate::components::mental::{Mood, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs};
use crate::resources::time::{Season, SimConfig, TimeState};
use crate::resources::weather::{Weather, WeatherState};

// ---------------------------------------------------------------------------
// decay_needs system
// ---------------------------------------------------------------------------

/// Advance need decay for every cat entity each tick.
///
/// - Physiological needs (hunger, energy, warmth) drain continuously.
/// - Safety *recovers* passively toward 1.0; it drops only from events.
/// - Social/acceptance/respect/mastery/purpose decay faster for cats whose
///   personality traits make them more invested in those needs.
/// - Warmth takes additional drain from cold weather and winter seasons.
/// - **Starvation** (hunger == 0): drains health, drops safety, doubles
///   social decay, and applies a persistent mood penalty.
pub fn decay_needs(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    mut query: Query<(&mut Needs, &Personality, &mut Health, &mut Mood), Without<Dead>>,
) {
    let season = time.season(&config);

    // Additional warmth drain from weather.
    let weather_warmth_drain = match weather.current {
        Weather::Snow => 0.004,
        Weather::Storm => 0.003,
        Weather::Wind => 0.002,
        Weather::HeavyRain => 0.002,
        Weather::LightRain => 0.001,
        _ => 0.0,
    };

    // Additional warmth drain from season.
    let season_warmth_drain = match season {
        Season::Winter => 0.003,
        Season::Autumn => 0.001,
        _ => 0.0,
    };

    let warmth_drain = 0.001 + weather_warmth_drain + season_warmth_drain;

    for (mut needs, personality, mut health, mut mood) in &mut query {
        // --- Level 1: Physiological ---
        needs.hunger = (needs.hunger - 0.002).max(0.0);
        needs.energy = (needs.energy - 0.002).max(0.0);
        needs.warmth = (needs.warmth - warmth_drain).max(0.0);

        // --- Starvation cascade ---
        let starving = needs.hunger == 0.0;
        if starving {
            // Health drains when starving.
            health.current = (health.current - 0.005).max(0.0);

            // Safety drops from existential anxiety.
            needs.safety = (needs.safety - 0.005).max(0.0);

            // Persistent mood penalty (refresh each tick while starving).
            if !mood.modifiers.iter().any(|m| m.source == "starvation") {
                mood.modifiers.push_back(MoodModifier {
                    amount: -0.3,
                    ticks_remaining: 5,
                    source: "starvation".to_string(),
                });
            } else {
                // Refresh the existing starvation modifier.
                for m in mood.modifiers.iter_mut() {
                    if m.source == "starvation" {
                        m.ticks_remaining = 5;
                    }
                }
            }
        }

        // --- Level 2: Safety — recovers passively (unless starving) ---
        if !starving {
            needs.safety = (needs.safety + 0.005).min(1.0);
        }

        // --- Level 3: Belonging ---
        let social_multiplier = if starving { 2.0 } else { 1.0 };
        let social_drain = 0.001 * (1.0 + personality.sociability * 0.5) * social_multiplier;
        needs.social = (needs.social - social_drain).max(0.0);

        let acceptance_drain = 0.0005 * (1.0 + personality.warmth * 0.5);
        needs.acceptance = (needs.acceptance - acceptance_drain).max(0.0);

        // --- Level 4: Esteem ---
        let respect_drain = 0.0003 * (1.0 + personality.ambition * 0.5);
        needs.respect = (needs.respect - respect_drain).max(0.0);

        let mastery_drain = 0.0002 * (1.0 + personality.diligence * 0.5);
        needs.mastery = (needs.mastery - mastery_drain).max(0.0);

        // --- Level 5: Self-actualisation ---
        let purpose_drain = 0.0001 * (1.0 + personality.curiosity * 0.5);
        needs.purpose = (needs.purpose - purpose_drain).max(0.0);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState::default());
        world.insert_resource(SimConfig::default());
        world.insert_resource(WeatherState::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_needs);
        (world, schedule)
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

    fn spawn_cat(world: &mut World, needs: Needs, personality: Personality) -> Entity {
        world
            .spawn((needs, personality, Health::default(), Mood::default()))
            .id()
    }

    #[test]
    fn hunger_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality());

        let before = world.get::<Needs>(entity).unwrap().hunger;
        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().hunger;

        assert!(after < before, "hunger should decrease; before={before}, after={after}");
        assert!((before - after - 0.002).abs() < 1e-6, "expected drain of 0.002");
    }

    #[test]
    fn energy_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality());

        let before = world.get::<Needs>(entity).unwrap().energy;
        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().energy;

        assert!(after < before, "energy should decrease; before={before}, after={after}");
        assert!((before - after - 0.002).abs() < 1e-6, "expected drain of 0.002");
    }

    #[test]
    fn social_decays_faster_for_social_cats() {
        let (mut world, mut schedule) = setup_world();

        let mut high_social = default_personality();
        high_social.sociability = 1.0;

        let mut low_social = default_personality();
        low_social.sociability = 0.0;

        let cat_social = spawn_cat(&mut world, Needs::default(), high_social);
        let cat_loner = spawn_cat(&mut world, Needs::default(), low_social);

        schedule.run(&mut world);

        let social_after = world.get::<Needs>(cat_social).unwrap().social;
        let loner_after = world.get::<Needs>(cat_loner).unwrap().social;

        assert!(
            social_after < loner_after,
            "highly social cat should lose more social need; social={social_after}, loner={loner_after}"
        );
    }

    #[test]
    fn safety_recovers_each_tick() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.safety = 0.5;
        let entity = spawn_cat(&mut world, needs, default_personality());

        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().safety;

        assert!(after > 0.5, "safety should recover toward 1.0; got {after}");
        assert!((after - 0.505).abs() < 1e-6, "expected recovery of 0.005");
    }

    #[test]
    fn safety_clamped_at_one() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, Needs::default(), default_personality());

        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().safety;

        assert_eq!(after, 1.0, "safety should not exceed 1.0; got {after}");
    }

    #[test]
    fn needs_do_not_go_below_zero() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0;
        needs.energy = 0.0;
        needs.warmth = 0.0;
        needs.social = 0.0;
        needs.acceptance = 0.0;
        needs.respect = 0.0;
        needs.mastery = 0.0;
        needs.purpose = 0.0;

        let entity = spawn_cat(&mut world, needs, default_personality());
        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, 0.0);
        assert_eq!(n.energy, 0.0);
        assert_eq!(n.warmth, 0.0);
        assert_eq!(n.social, 0.0);
        assert_eq!(n.acceptance, 0.0);
        assert_eq!(n.respect, 0.0);
        assert_eq!(n.mastery, 0.0);
        assert_eq!(n.purpose, 0.0);
    }

    #[test]
    fn warmth_drains_extra_in_snow() {
        let (mut world_clear, mut schedule_clear) = setup_world();
        let cat_clear = spawn_cat(&mut world_clear, Needs::default(), default_personality());
        schedule_clear.run(&mut world_clear);
        let warmth_clear = world_clear.get::<Needs>(cat_clear).unwrap().warmth;

        let (mut world_snow, mut schedule_snow) = setup_world();
        world_snow.insert_resource(WeatherState {
            current: Weather::Snow,
            ticks_until_change: 50,
        });
        let cat_snow = spawn_cat(&mut world_snow, Needs::default(), default_personality());
        schedule_snow.run(&mut world_snow);
        let warmth_snow = world_snow.get::<Needs>(cat_snow).unwrap().warmth;

        assert!(
            warmth_snow < warmth_clear,
            "snow should drain warmth faster; snow={warmth_snow}, clear={warmth_clear}"
        );
    }

    #[test]
    fn starvation_drains_health() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0; // starving

        let entity = spawn_cat(&mut world, needs, default_personality());

        let health_before = world.get::<Health>(entity).unwrap().current;
        schedule.run(&mut world);
        let health_after = world.get::<Health>(entity).unwrap().current;

        assert!(
            health_after < health_before,
            "starvation should drain health; before={health_before}, after={health_after}"
        );
        assert!(
            (health_before - health_after - 0.005).abs() < 1e-6,
            "expected 0.005 health drain per tick"
        );
    }

    #[test]
    fn starvation_drops_safety() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0;
        needs.safety = 0.5;

        let entity = spawn_cat(&mut world, needs, default_personality());

        schedule.run(&mut world);
        let safety = world.get::<Needs>(entity).unwrap().safety;

        // Starvation drains 0.005; no passive recovery when starving.
        assert!(
            safety < 0.5,
            "starvation should drain safety; got {safety}"
        );
    }

    #[test]
    fn starvation_doubles_social_decay() {
        let (mut world_fed, mut schedule_fed) = setup_world();
        let fed = spawn_cat(&mut world_fed, Needs::default(), default_personality());
        schedule_fed.run(&mut world_fed);
        let social_fed = world_fed.get::<Needs>(fed).unwrap().social;

        let (mut world_starving, mut schedule_starving) = setup_world();
        let mut starving_needs = Needs::default();
        starving_needs.hunger = 0.0;
        let starving = spawn_cat(&mut world_starving, starving_needs, default_personality());
        schedule_starving.run(&mut world_starving);
        let social_starving = world_starving.get::<Needs>(starving).unwrap().social;

        assert!(
            social_starving < social_fed,
            "starving cat should lose social faster; starving={social_starving}, fed={social_fed}"
        );
    }

    #[test]
    fn starvation_applies_mood_penalty() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        needs.hunger = 0.0;

        let entity = spawn_cat(&mut world, needs, default_personality());
        schedule.run(&mut world);

        let mood = world.get::<Mood>(entity).unwrap();
        let has_starvation_modifier = mood.modifiers.iter().any(|m| m.source == "starvation");
        assert!(has_starvation_modifier, "starving cat should have starvation mood modifier");
    }
}
