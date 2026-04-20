use bevy_ecs::prelude::Resource;
use rand::Rng;

use crate::resources::time::Season;

// ---------------------------------------------------------------------------
// Weather enum
// ---------------------------------------------------------------------------

/// Current weather condition in the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weather {
    Clear,
    Overcast,
    LightRain,
    HeavyRain,
    Snow,
    Fog,
    Wind,
    Storm,
}

impl Weather {
    /// Human-readable name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::Overcast => "Overcast",
            Self::LightRain => "Light Rain",
            Self::HeavyRain => "Heavy Rain",
            Self::Snow => "Snow",
            Self::Fog => "Fog",
            Self::Wind => "Wind",
            Self::Storm => "Storm",
        }
    }

    /// Short symbol for TUI display.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Clear => "☀",
            Self::Overcast => "☁",
            Self::LightRain => "🌦",
            Self::HeavyRain => "🌧",
            Self::Snow => "❄",
            Self::Fog => "🌫",
            Self::Wind => "💨",
            Self::Storm => "⛈",
        }
    }

    /// Multiplier applied to movement speed under this weather.
    ///
    /// Values < 1.0 slow movement; 1.0 is unimpeded.
    pub fn movement_multiplier(self) -> f32 {
        match self {
            Self::Clear | Self::Overcast => 1.0,
            Self::LightRain | Self::Fog => 0.9,
            Self::Wind => 0.85,
            Self::HeavyRain => 0.7,
            Self::Snow => 0.6,
            Self::Storm => 0.4,
        }
    }

    /// Additive modifier applied to entity comfort under this weather.
    ///
    /// Negative values reduce comfort.
    pub fn comfort_modifier(self) -> f32 {
        match self {
            Self::Clear | Self::Overcast => 0.0,
            Self::LightRain => -0.05,
            Self::Fog => -0.02,
            Self::Wind => -0.08,
            Self::HeavyRain => -0.15,
            Self::Snow => -0.2,
            Self::Storm => -0.3,
        }
    }

    // ---- Sensory multipliers (sensing.rs) ----
    //
    // Values ship one at a time, each bundled with a verisimilitude
    // hypothesis + concordance sweep per the Balance Methodology rule in
    // CLAUDE.md. The activation log lives inline in the per-field docstrings
    // below.

    /// Multiplier applied to an observer's effective sight range.
    ///
    /// Temporarily reverted to 1.0 to re-capture a baseline under the
    /// updated `start_tick = 60 * ticks_per_season` regime; the prior
    /// baseline was captured at the legacy `start_tick = 100_000` and
    /// cross-sweep comparison is unsound across that boundary.
    pub fn sight_multiplier(self) -> f32 {
        1.0
    }

    /// Multiplier applied to an observer's effective hearing range.
    pub fn hearing_multiplier(self) -> f32 {
        1.0
    }

    /// Multiplier applied to an observer's effective scent range.
    pub fn scent_multiplier(self) -> f32 {
        1.0
    }

    /// Multiplier applied to an observer's effective tremor range.
    pub fn tremor_multiplier(self) -> f32 {
        1.0
    }
}

// ---------------------------------------------------------------------------
// WeatherState resource
// ---------------------------------------------------------------------------

/// ECS resource tracking current weather and when it will next change.
#[derive(Resource, serde::Serialize, serde::Deserialize)]
pub struct WeatherState {
    pub current: Weather,
    /// Ticks remaining before a weather transition is evaluated.
    pub ticks_until_change: u64,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            current: Weather::Clear,
            ticks_until_change: 50,
        }
    }
}

impl WeatherState {
    /// Pick the next weather variant using season-weighted probability.
    ///
    /// The weight tables reflect realistic seasonal tendencies. Variants with
    /// zero weight for a season are excluded entirely.
    pub fn next_weather(&self, season: Season, rng: &mut impl Rng) -> Weather {
        // (variant, weight) pairs per season
        let table: &[(Weather, f32)] = match season {
            Season::Spring => &[
                (Weather::Clear, 3.0),
                (Weather::Overcast, 2.0),
                (Weather::LightRain, 2.0),
                (Weather::HeavyRain, 0.5),
                (Weather::Fog, 1.0),
                (Weather::Wind, 1.0),
            ],
            Season::Summer => &[
                (Weather::Clear, 5.0),
                (Weather::Overcast, 1.5),
                (Weather::LightRain, 1.0),
                (Weather::HeavyRain, 0.3),
                (Weather::Wind, 0.5),
                (Weather::Storm, 0.2),
            ],
            Season::Autumn => &[
                (Weather::Clear, 2.0),
                (Weather::Overcast, 3.0),
                (Weather::LightRain, 2.0),
                (Weather::HeavyRain, 1.5),
                (Weather::Fog, 1.5),
                (Weather::Wind, 2.0),
                (Weather::Storm, 0.5),
            ],
            Season::Winter => &[
                (Weather::Clear, 2.0),
                (Weather::Overcast, 3.0),
                (Weather::Snow, 3.0),
                (Weather::Wind, 2.0),
                (Weather::Fog, 1.0),
                (Weather::Storm, 0.5),
            ],
        };

        let total: f32 = table.iter().map(|(_, w)| w).sum();
        let mut roll: f32 = rng.random_range(0.0..total);

        for (variant, weight) in table {
            if roll < *weight {
                return *variant;
            }
            roll -= weight;
        }

        // Fallback — should be unreachable due to float rounding only
        table.last().expect("table is non-empty").0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn seeded_rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    #[test]
    fn weather_transitions_produce_valid_variants() {
        let state = WeatherState::default();
        let mut rng = seeded_rng(42);

        let valid_all: &[Weather] = &[
            Weather::Clear,
            Weather::Overcast,
            Weather::LightRain,
            Weather::HeavyRain,
            Weather::Snow,
            Weather::Fog,
            Weather::Wind,
            Weather::Storm,
        ];

        for season in [
            Season::Spring,
            Season::Summer,
            Season::Autumn,
            Season::Winter,
        ] {
            for _ in 0..50 {
                let w = state.next_weather(season, &mut rng);
                assert!(
                    valid_all.contains(&w),
                    "unexpected variant {w:?} for {season:?}"
                );
            }
        }
    }

    #[test]
    fn winter_eventually_produces_snow() {
        let state = WeatherState::default();
        let mut rng = seeded_rng(1);

        let saw_snow =
            (0..500).any(|_| state.next_weather(Season::Winter, &mut rng) == Weather::Snow);
        assert!(saw_snow, "snow should appear within 500 winter draws");
    }

    #[test]
    fn winter_never_produces_heavy_rain() {
        let state = WeatherState::default();
        let mut rng = seeded_rng(7);

        for _ in 0..1000 {
            let w = state.next_weather(Season::Winter, &mut rng);
            assert_ne!(w, Weather::HeavyRain, "HeavyRain has no weight in winter");
        }
    }

    #[test]
    fn summer_never_produces_snow() {
        let state = WeatherState::default();
        let mut rng = seeded_rng(13);

        for _ in 0..1000 {
            let w = state.next_weather(Season::Summer, &mut rng);
            assert_ne!(w, Weather::Snow, "Snow has no weight in summer");
        }
    }

    #[test]
    fn movement_multiplier_ordered() {
        // Storm is slowest, Clear/Overcast are fastest
        assert!(Weather::Storm.movement_multiplier() < Weather::Snow.movement_multiplier());
        assert!(Weather::Snow.movement_multiplier() < Weather::HeavyRain.movement_multiplier());
        assert_eq!(Weather::Clear.movement_multiplier(), 1.0);
        assert_eq!(Weather::Overcast.movement_multiplier(), 1.0);
    }

    #[test]
    fn comfort_modifier_negative_for_bad_weather() {
        assert!(Weather::Storm.comfort_modifier() < 0.0);
        assert!(Weather::Snow.comfort_modifier() < 0.0);
        assert_eq!(Weather::Clear.comfort_modifier(), 0.0);
        assert_eq!(Weather::Overcast.comfort_modifier(), 0.0);
    }

    #[test]
    fn default_weather_state() {
        let ws = WeatherState::default();
        assert_eq!(ws.current, Weather::Clear);
        assert_eq!(ws.ticks_until_change, 50);
    }
}
