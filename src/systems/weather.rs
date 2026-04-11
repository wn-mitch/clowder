use bevy_ecs::prelude::{Res, ResMut};
use rand::Rng;

use crate::resources::{SimConfig, SimRng, TimeState, WeatherState};

/// Update weather state each tick.
///
/// Counts down `ticks_until_change`. When it reaches zero, a new weather
/// variant is drawn from the season-weighted table and a new countdown is
/// set. This produces weather that holds for 30–79 ticks before shifting.
pub fn update_weather(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut weather: ResMut<WeatherState>,
    mut rng: ResMut<SimRng>,
) {
    if weather.ticks_until_change == 0 {
        let season = time.season(&config);
        weather.current = weather.next_weather(season, &mut rng.rng);
        weather.ticks_until_change = rng.rng.random_range(30..80);
    } else {
        weather.ticks_until_change -= 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::weather::Weather;
    use crate::resources::{SimConfig, SimRng, TimeState, WeatherState};

    /// Mirror of the system logic, callable without ECS.
    fn update_weather_direct(
        time: &TimeState,
        config: &SimConfig,
        weather: &mut WeatherState,
        rng: &mut SimRng,
    ) {
        if weather.ticks_until_change == 0 {
            let season = time.season(config);
            weather.current = weather.next_weather(season, &mut rng.rng);
            weather.ticks_until_change = rng.rng.random_range(30..80);
        } else {
            weather.ticks_until_change -= 1;
        }
    }

    #[test]
    fn countdown_decrements_each_tick() {
        let time = TimeState::default();
        let config = SimConfig::default();
        let mut weather = WeatherState::default(); // ticks_until_change = 50
        let mut rng = SimRng::new(42);

        update_weather_direct(&time, &config, &mut weather, &mut rng);
        assert_eq!(weather.ticks_until_change, 49);
        update_weather_direct(&time, &config, &mut weather, &mut rng);
        assert_eq!(weather.ticks_until_change, 48);
        // Current weather unchanged while counting down
        assert_eq!(weather.current, Weather::Clear);
    }

    #[test]
    fn weather_transitions_at_zero() {
        let time = TimeState::default();
        let config = SimConfig::default();
        let mut weather = WeatherState {
            current: Weather::Clear,
            ticks_until_change: 0,
        };
        let mut rng = SimRng::new(99);

        update_weather_direct(&time, &config, &mut weather, &mut rng);

        // After the transition the countdown must be in [30, 79]
        assert!(
            weather.ticks_until_change >= 30 && weather.ticks_until_change < 80,
            "unexpected countdown: {}",
            weather.ticks_until_change
        );
    }

    #[test]
    fn repeated_transitions_stay_valid() {
        let time = TimeState::default();
        let config = SimConfig::default();
        let mut weather = WeatherState::default();
        let mut rng = SimRng::new(7);

        let valid: &[Weather] = &[
            Weather::Clear,
            Weather::Overcast,
            Weather::LightRain,
            Weather::HeavyRain,
            Weather::Snow,
            Weather::Fog,
            Weather::Wind,
            Weather::Storm,
        ];

        // Run enough ticks to force at least several transitions
        for _ in 0..500 {
            update_weather_direct(&time, &config, &mut weather, &mut rng);
            assert!(
                valid.contains(&weather.current),
                "invalid weather variant: {:?}",
                weather.current
            );
        }
    }
}
