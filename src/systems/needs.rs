use bevy_ecs::prelude::*;

use crate::components::physical::Needs;
use crate::components::personality::Personality;
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
pub fn decay_needs(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    mut query: Query<(&mut Needs, &Personality)>,
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

    for (mut needs, personality) in &mut query {
        // --- Level 1: Physiological ---
        needs.hunger = (needs.hunger - 0.003).max(0.0);
        needs.energy = (needs.energy - 0.002).max(0.0);
        needs.warmth = (needs.warmth - warmth_drain).max(0.0);

        // --- Level 2: Safety — recovers passively ---
        needs.safety = (needs.safety + 0.005).min(1.0);

        // --- Level 3: Belonging ---
        let social_drain = 0.001 * (1.0 + personality.sociability * 0.5);
        needs.social = (needs.social - social_drain).max(0.0);

        // `personality.warmth` is the warmth-trait axis (not the warmth need).
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

    #[test]
    fn hunger_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let entity = world.spawn((Needs::default(), default_personality())).id();

        let before = world.get::<Needs>(entity).unwrap().hunger;
        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().hunger;

        assert!(after < before, "hunger should decrease; before={before}, after={after}");
        assert!((before - after - 0.003).abs() < 1e-6, "expected drain of 0.003");
    }

    #[test]
    fn energy_decays_over_time() {
        let (mut world, mut schedule) = setup_world();
        let entity = world.spawn((Needs::default(), default_personality())).id();

        let before = world.get::<Needs>(entity).unwrap().energy;
        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().energy;

        assert!(after < before, "energy should decrease; before={before}, after={after}");
        assert!((before - after - 0.002).abs() < 1e-6, "expected drain of 0.002");
    }

    #[test]
    fn social_decays_faster_for_social_cats() {
        let (mut world, mut schedule) = setup_world();

        let mut high_social_personality = default_personality();
        high_social_personality.sociability = 1.0;

        let mut low_social_personality = default_personality();
        low_social_personality.sociability = 0.0;

        let cat_social = world.spawn((Needs::default(), high_social_personality)).id();
        let cat_loner = world.spawn((Needs::default(), low_social_personality)).id();

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
        let entity = world.spawn((needs, default_personality())).id();

        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().safety;

        assert!(after > 0.5, "safety should recover toward 1.0; got {after}");
        assert!((after - 0.505).abs() < 1e-6, "expected recovery of 0.005");
    }

    #[test]
    fn safety_clamped_at_one() {
        let (mut world, mut schedule) = setup_world();
        // Safety already at max; recovery should not push it above 1.0.
        let entity = world.spawn((Needs::default(), default_personality())).id();

        schedule.run(&mut world);
        let after = world.get::<Needs>(entity).unwrap().safety;

        assert_eq!(after, 1.0, "safety should not exceed 1.0; got {after}");
    }

    #[test]
    fn needs_do_not_go_below_zero() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = Needs::default();
        // Drive all draining needs to exactly zero.
        needs.hunger = 0.0;
        needs.energy = 0.0;
        needs.warmth = 0.0;
        needs.social = 0.0;
        needs.acceptance = 0.0;
        needs.respect = 0.0;
        needs.mastery = 0.0;
        needs.purpose = 0.0;

        let entity = world.spawn((needs, default_personality())).id();
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
        // Run once with Clear weather (default) and once with Snow.
        let (mut world_clear, mut schedule_clear) = setup_world();
        let cat_clear = world_clear.spawn((Needs::default(), default_personality())).id();
        schedule_clear.run(&mut world_clear);
        let warmth_clear = world_clear.get::<Needs>(cat_clear).unwrap().warmth;

        let (mut world_snow, mut schedule_snow) = setup_world();
        world_snow.insert_resource(WeatherState {
            current: Weather::Snow,
            ticks_until_change: 50,
        });
        let cat_snow = world_snow.spawn((Needs::default(), default_personality())).id();
        schedule_snow.run(&mut world_snow);
        let warmth_snow = world_snow.get::<Needs>(cat_snow).unwrap().warmth;

        assert!(
            warmth_snow < warmth_clear,
            "snow should drain warmth faster; snow={warmth_snow}, clear={warmth_clear}"
        );
    }
}
