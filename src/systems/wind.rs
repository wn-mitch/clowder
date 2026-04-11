use bevy_ecs::prelude::*;
use rand::Rng;

use crate::resources::rng::SimRng;
use crate::resources::weather::{Weather, WeatherState};
use crate::resources::wind::WindState;

/// Drift wind direction and strength each tick.
///
/// Wind rotates slowly by default. Weather influences it: storms randomize
/// direction and boost strength; calm weather lets strength decay toward a
/// moderate baseline.
pub fn update_wind(
    mut wind: ResMut<WindState>,
    weather: Res<WeatherState>,
    mut rng: ResMut<SimRng>,
) {
    // Base drift: slow rotation.
    wind.angle += 0.008;

    // Weather coupling.
    match weather.current {
        Weather::Storm => {
            // Storms: big random angle shifts, high strength.
            wind.angle += rng.rng.random_range(-0.3f32..0.3);
            wind.strength = (wind.strength + 0.005).min(1.0);
        }
        Weather::Wind => {
            // Windy: moderate jitter, boost strength.
            wind.angle += rng.rng.random_range(-0.1f32..0.1);
            wind.strength = (wind.strength + 0.002).min(0.9);
        }
        Weather::Clear | Weather::Overcast => {
            // Calm: strength decays toward 0.4.
            wind.strength += (0.4 - wind.strength) * 0.001;
        }
        _ => {
            // Rain/snow/fog: strength decays toward 0.5.
            wind.strength += (0.5 - wind.strength) * 0.001;
        }
    }

    // Keep angle in [0, TAU).
    wind.angle = wind.angle.rem_euclid(std::f32::consts::TAU);
    wind.strength = wind.strength.clamp(0.1, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(WindState::default());
        world.insert_resource(WeatherState::default());
        world.insert_resource(SimRng::new(42));
        let mut schedule = Schedule::default();
        schedule.add_systems(update_wind);
        (world, schedule)
    }

    #[test]
    fn wind_drifts_over_time() {
        let (mut world, mut schedule) = setup();
        let before = world.resource::<WindState>().angle;
        for _ in 0..100 {
            schedule.run(&mut world);
        }
        let after = world.resource::<WindState>().angle;
        assert!(
            (after - before).abs() > 0.5,
            "wind should drift noticeably over 100 ticks; before={before}, after={after}"
        );
    }

    #[test]
    fn storm_boosts_strength() {
        let (mut world, mut schedule) = setup();
        world.insert_resource(WeatherState {
            current: Weather::Storm,
            ticks_until_change: 100,
        });
        world.resource_mut::<WindState>().strength = 0.3;

        for _ in 0..50 {
            schedule.run(&mut world);
        }

        let strength = world.resource::<WindState>().strength;
        assert!(
            strength > 0.5,
            "storm should boost wind strength; got {strength}"
        );
    }
}
