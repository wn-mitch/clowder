use bevy_ecs::prelude::{Res, ResMut};

use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::time::TransitionTracker;
use crate::resources::weather::WeatherState;
use crate::resources::TimeState;
use rand::Rng;

/// Schedule-level guard: only advance the clock when the sim is not paused.
pub fn not_paused(time: Res<TimeState>) -> bool {
    !time.paused
}

/// Advance the simulation clock by one tick.
///
/// Gate with `run_if(not_paused)` at the schedule level; the system itself
/// is unconditional. All other time-derived values (season, day phase) are
/// computed on-demand from the raw tick count.
pub fn advance_time(mut time: ResMut<TimeState>) {
    time.tick += 1;
}

/// Detect weather transitions and push narratives to the log.
pub fn emit_weather_transitions(
    time: Res<TimeState>,
    weather: Res<WeatherState>,
    mut tracker: ResMut<TransitionTracker>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
    let current = weather.current;

    if let Some(prev) = tracker.last_weather {
        if prev != current {
            let text = weather_narrative(current, &mut rng);
            log.push(time.tick, text, NarrativeTier::Action);
        }
    }

    tracker.last_weather = Some(current);
}

fn weather_narrative(weather: crate::resources::weather::Weather, rng: &mut SimRng) -> String {
    use crate::resources::weather::Weather;

    let variants: &[&str] = match weather {
        Weather::Clear => &[
            "The clouds part. Sunlight fills the clearing.",
            "Blue sky breaks through. The air warms.",
            "The sky clears. Light dapples through the canopy.",
        ],
        Weather::Overcast => &[
            "Clouds gather overhead, muting the light.",
            "A grey blanket spreads across the sky.",
            "The sun disappears behind thickening clouds.",
        ],
        Weather::LightRain => &[
            "A gentle rain begins to fall.",
            "Soft droplets patter against the leaves.",
            "A light drizzle settles over the camp.",
        ],
        Weather::HeavyRain => &[
            "Rain hammers down in heavy sheets.",
            "The sky opens up. Water pools on the ground.",
            "A downpour drenches the clearing.",
        ],
        Weather::Snow => &[
            "Soft flakes begin to fall.",
            "Snow drifts down, blanketing the camp in white.",
            "The first snowflakes appear, swirling in the air.",
        ],
        Weather::Fog => &[
            "A thick fog creeps through the trees.",
            "Mist rolls in, swallowing the edges of the camp.",
            "The world shrinks to a grey haze.",
        ],
        Weather::Wind => &[
            "A biting wind picks up, tugging at fur.",
            "Gusts rattle through the branches.",
            "The wind rises, carrying the scent of distant places.",
        ],
        Weather::Storm => &[
            "Thunder rumbles. A storm rolls in.",
            "Lightning splits the sky. The camp braces.",
            "Dark clouds churn overhead. A storm breaks.",
        ],
    };

    let idx = rng.rng.random_range(0..variants.len());
    variants[idx].to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::resources::TimeState;

    #[test]
    fn tick_advances() {
        let mut time = TimeState::default();
        assert_eq!(time.tick, 0);
        time.tick += 1;
        assert_eq!(time.tick, 1);
        time.tick += 1;
        assert_eq!(time.tick, 2);
    }

    #[test]
    fn not_paused_when_running() {
        let time = TimeState::default();
        assert!(!time.paused);
    }

    #[test]
    fn paused_when_set() {
        let mut time = TimeState::default();
        time.paused = true;
        assert!(time.paused);
    }
}
