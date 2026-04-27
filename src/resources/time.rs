use bevy_ecs::prelude::Resource;

use crate::resources::weather::Weather;

// ---------------------------------------------------------------------------
// TransitionTracker
// ---------------------------------------------------------------------------

/// Tracks previous-tick state so systems can detect transitions and emit
/// narratives. `None` values on the first tick prevent spurious emissions.
#[derive(Resource, Default)]
pub struct TransitionTracker {
    pub last_weather: Option<Weather>,
}

// ---------------------------------------------------------------------------
// SimConfig
// ---------------------------------------------------------------------------

/// Simulation configuration constants. Stored as a resource; inject this into
/// any system that needs to convert raw ticks into human-readable time.
#[derive(Resource, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimConfig {
    /// Number of ticks per day phase (Dawn / Day / Dusk / Night).
    /// Default 250 → a full day is 1000 ticks.
    pub ticks_per_day_phase: u64,
    /// Number of ticks per season (Spring / Summer / Autumn / Winter).
    /// Default 20000 → a full year is 80000 ticks.
    pub ticks_per_season: u64,
    /// RNG seed for reproducible runs.
    pub seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            ticks_per_day_phase: 250,
            ticks_per_season: 20000,
            seed: 42,
        }
    }
}

/// Test-scoped season length. Production uses 20 000 ticks/season; tests
/// use this 10× smaller value so age-window assertions remain readable
/// without paying the cost of a full season's tick count. Single source
/// of truth across `src/components/identity.rs` and
/// `src/world_gen/colony.rs` (ticket 033 Phase 5 — was duplicated).
#[cfg(test)]
pub const TEST_TICKS_PER_SEASON: u64 = 2000;

// ---------------------------------------------------------------------------
// TimeScale
// ---------------------------------------------------------------------------

/// The single anchor connecting in-game time to wall-clock time.
///
/// Holds two facts:
/// 1. The in-game tick scale (`ticks_per_day`, `ticks_per_season`),
///    derived from [`SimConfig`].
/// 2. The user-facing real-time peg `wall_seconds_per_game_day` —
///    "how many wall-clock seconds equal one in-game day."
///
/// The headless build's `Time<Fixed>` Hz and the windowed build's
/// `sync_sim_speed` both derive their tick rate from
/// [`TimeScale::tick_rate_hz`]. Two runs are only behaviorally
/// comparable iff their `TimeScale` matches.
///
/// Inserted as a [`Resource`] by the host (headless `run_headless` from
/// the `--game-day-seconds N` CLI flag; windowed `main` from
/// [`SimSpeed`]). Consumed by the typed-units module
/// [`super::time_units`] to convert per-day rates / day-durations /
/// per-day intervals into raw tick values.
///
/// See `docs/systems/time-anchor.md` for the canonical reference.
#[derive(Resource, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TimeScale {
    ticks_per_day: u64,
    ticks_per_season: u64,
    wall_seconds_per_game_day: f32,
}

impl TimeScale {
    /// Build a [`TimeScale`] from the live [`SimConfig`] and a
    /// host-supplied real-time peg.
    pub fn from_config(config: &SimConfig, wall_seconds_per_game_day: f32) -> Self {
        Self {
            ticks_per_day: config.ticks_per_day_phase * 4,
            ticks_per_season: config.ticks_per_season,
            wall_seconds_per_game_day,
        }
    }

    pub fn ticks_per_day(&self) -> u64 {
        self.ticks_per_day
    }

    pub fn ticks_per_season(&self) -> u64 {
        self.ticks_per_season
    }

    pub fn wall_seconds_per_game_day(&self) -> f32 {
        self.wall_seconds_per_game_day
    }

    /// Mutate the real-time peg without rebuilding the resource.
    /// Used by the windowed build's `sync_sim_speed` when [`SimSpeed`]
    /// changes.
    pub fn set_wall_seconds_per_game_day(&mut self, secs: f32) {
        self.wall_seconds_per_game_day = secs;
    }

    /// Tick rate in Hz: how many ticks the [`Time<Fixed>`] schedule
    /// must advance per wall-clock second to honor the peg.
    pub fn tick_rate_hz(&self) -> f32 {
        // Guard against pathological pegs; minimum 1 Hz so neither
        // headless nor windowed grinds to zero on an inadvertent
        // `--game-day-seconds 0`.
        if self.wall_seconds_per_game_day <= 0.0 {
            return 1.0;
        }
        self.ticks_per_day as f32 / self.wall_seconds_per_game_day
    }
}

// ---------------------------------------------------------------------------
// DayPhase
// ---------------------------------------------------------------------------

/// The four phases of the in-game day, cycling in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl DayPhase {
    /// Derive the current phase from an absolute tick count.
    pub fn from_tick(tick: u64, config: &SimConfig) -> Self {
        let ticks_per_day = config.ticks_per_day_phase * 4;
        let phase_index = (tick % ticks_per_day) / config.ticks_per_day_phase;
        match phase_index {
            0 => Self::Dawn,
            1 => Self::Day,
            2 => Self::Dusk,
            3 => Self::Night,
            _ => unreachable!("phase_index is always 0–3"),
        }
    }

    /// Short human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dawn => "Dawn",
            Self::Day => "Day",
            Self::Dusk => "Dusk",
            Self::Night => "Night",
        }
    }

    // ---- Sensory multipliers (see sensing.rs) ----
    //
    // Phase 1 stubs returning 1.0 (identity). Activation is a Phase 5b
    // semantic change requiring a verisimilitude hypothesis — planned
    // values have Night dim sight (~0.5), Dusk/Dawn mild sight dim
    // (~0.8), and tremor/hearing/scent phase-independent. See plan file.

    pub fn sight_multiplier(self) -> f32 {
        1.0
    }

    pub fn hearing_multiplier(self) -> f32 {
        1.0
    }

    pub fn scent_multiplier(self) -> f32 {
        1.0
    }

    pub fn tremor_multiplier(self) -> f32 {
        1.0
    }
}

// ---------------------------------------------------------------------------
// Season
// ---------------------------------------------------------------------------

/// The four seasons, cycling in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Derive the current season from an absolute tick count.
    pub fn from_tick(tick: u64, config: &SimConfig) -> Self {
        let ticks_per_year = config.ticks_per_season * 4;
        let season_index = (tick % ticks_per_year) / config.ticks_per_season;
        match season_index {
            0 => Self::Spring,
            1 => Self::Summer,
            2 => Self::Autumn,
            3 => Self::Winter,
            _ => unreachable!("season_index is always 0–3"),
        }
    }

    /// Foraging yield multiplier for this season.
    ///
    /// Spring is abundant, summer baseline, autumn declining, winter barren.
    pub fn foraging_multiplier(self) -> f32 {
        match self {
            Self::Spring => 1.2,
            Self::Summer => 1.0,
            Self::Autumn => 0.6,
            Self::Winter => 0.15,
        }
    }

    /// Short human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Spring => "Spring",
            Self::Summer => "Summer",
            Self::Autumn => "Autumn",
            Self::Winter => "Winter",
        }
    }
}

// ---------------------------------------------------------------------------
// SimSpeed
// ---------------------------------------------------------------------------

/// How many simulation ticks to advance per game-loop update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SimSpeed {
    #[default]
    Normal,
    Fast,
    VeryFast,
}

impl SimSpeed {
    /// Target tick rate: how many simulation ticks per real second.
    pub fn ticks_per_second(self) -> f64 {
        match self {
            Self::Normal => 1.0,
            Self::Fast => 5.0,
            Self::VeryFast => 20.0,
        }
    }

    /// Short human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Fast => "Fast",
            Self::VeryFast => "Very Fast",
        }
    }

    /// Cycle through speeds: Normal → Fast → Very Fast → Normal.
    pub fn cycle(self) -> Self {
        match self {
            Self::Normal => Self::Fast,
            Self::Fast => Self::VeryFast,
            Self::VeryFast => Self::Normal,
        }
    }

    /// Map a speed preset to a `wall_seconds_per_game_day` peg given a
    /// fixed tick scale (1000 ticks/day at default). Inverts
    /// [`Self::ticks_per_second`]: Normal = 1 Hz means 1000 wall-secs
    /// per in-game day, Fast = 5 Hz means 200 wall-secs/day, VeryFast
    /// = 20 Hz means 50 wall-secs/day.
    pub fn wall_seconds_per_game_day(self, config: &SimConfig) -> f32 {
        let ticks_per_day = (config.ticks_per_day_phase * 4) as f32;
        ticks_per_day / self.ticks_per_second() as f32
    }
}

// ---------------------------------------------------------------------------
// TimeState
// ---------------------------------------------------------------------------

/// Global simulation clock. Advance `tick` each update; everything else is
/// derived.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TimeState {
    pub tick: u64,
    pub paused: bool,
    pub speed: SimSpeed,
}

impl TimeState {
    /// Current day phase derived from the stored tick.
    pub fn day_phase(&self, config: &SimConfig) -> DayPhase {
        DayPhase::from_tick(self.tick, config)
    }

    /// Current season derived from the stored tick.
    pub fn season(&self, config: &SimConfig) -> Season {
        Season::from_tick(self.tick, config)
    }

    /// 1-indexed day number. Day 1 starts at tick 0.
    pub fn day_number(tick: u64, config: &SimConfig) -> u64 {
        let ticks_per_day = config.ticks_per_day_phase * 4;
        tick / ticks_per_day + 1
    }

    /// Progress through the current day as a fraction in `[0.0, 1.0)`.
    pub fn day_progress(tick: u64, config: &SimConfig) -> f32 {
        let ticks_per_day = config.ticks_per_day_phase * 4;
        (tick % ticks_per_day) as f32 / ticks_per_day as f32
    }

    /// 1-indexed week number. Week 1 starts on day 1.
    pub fn week_number(tick: u64, config: &SimConfig) -> u64 {
        (Self::day_number(tick, config) - 1) / 7 + 1
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_phase_from_tick() {
        let config = SimConfig::default();
        assert_eq!(DayPhase::from_tick(0, &config), DayPhase::Dawn);
        assert_eq!(DayPhase::from_tick(249, &config), DayPhase::Dawn);
        assert_eq!(DayPhase::from_tick(250, &config), DayPhase::Day);
        assert_eq!(DayPhase::from_tick(500, &config), DayPhase::Dusk);
        assert_eq!(DayPhase::from_tick(750, &config), DayPhase::Night);
        assert_eq!(DayPhase::from_tick(1000, &config), DayPhase::Dawn); // wraps
    }

    #[test]
    fn season_from_tick() {
        let config = SimConfig::default();
        assert_eq!(Season::from_tick(0, &config), Season::Spring);
        assert_eq!(Season::from_tick(19999, &config), Season::Spring);
        assert_eq!(Season::from_tick(20000, &config), Season::Summer);
        assert_eq!(Season::from_tick(40000, &config), Season::Autumn);
        assert_eq!(Season::from_tick(60000, &config), Season::Winter);
        assert_eq!(Season::from_tick(80000, &config), Season::Spring); // wraps
    }

    #[test]
    fn day_number_from_tick() {
        let config = SimConfig::default();
        assert_eq!(TimeState::day_number(0, &config), 1);
        assert_eq!(TimeState::day_number(999, &config), 1);
        assert_eq!(TimeState::day_number(1000, &config), 2);
    }

    #[test]
    fn sim_speed_cycle() {
        assert_eq!(SimSpeed::Normal.cycle(), SimSpeed::Fast);
        assert_eq!(SimSpeed::Fast.cycle(), SimSpeed::VeryFast);
        assert_eq!(SimSpeed::VeryFast.cycle(), SimSpeed::Normal);
    }

    #[test]
    fn sim_speed_ticks_per_second() {
        assert_eq!(SimSpeed::Normal.ticks_per_second(), 1.0);
        assert_eq!(SimSpeed::Fast.ticks_per_second(), 5.0);
        assert_eq!(SimSpeed::VeryFast.ticks_per_second(), 20.0);
    }

    #[test]
    fn time_state_derived_accessors() {
        let config = SimConfig::default();
        let mut ts = TimeState::default();
        ts.tick = 750;
        assert_eq!(ts.day_phase(&config), DayPhase::Night);
        assert_eq!(ts.season(&config), Season::Spring);
        assert_eq!(TimeState::day_number(ts.tick, &config), 1);
    }

    #[test]
    fn day_progress_within_day() {
        let config = SimConfig::default(); // 250 ticks/phase, 1000 ticks/day
        assert!((TimeState::day_progress(0, &config) - 0.0).abs() < 1e-6);
        assert!((TimeState::day_progress(500, &config) - 0.5).abs() < 1e-6);
        assert!((TimeState::day_progress(999, &config) - 0.999).abs() < 1e-6);
        // Wraps at day boundary
        assert!((TimeState::day_progress(1000, &config) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn time_scale_derives_ticks_per_day_from_config() {
        let config = SimConfig::default();
        let ts = TimeScale::from_config(&config, 16.6667);
        assert_eq!(ts.ticks_per_day(), 1000);
        assert_eq!(ts.ticks_per_season(), 20_000);
    }

    #[test]
    fn time_scale_tick_rate_hz_default() {
        let config = SimConfig::default();
        // Default headless peg: 16.6667s/day → 60 Hz.
        let ts = TimeScale::from_config(&config, 16.6667);
        assert!((ts.tick_rate_hz() - 60.0).abs() < 0.01);
    }

    #[test]
    fn time_scale_tick_rate_hz_windowed_normal() {
        let config = SimConfig::default();
        // Windowed Normal: 1000s/day → 1 Hz, preserves prior behavior.
        let ts = TimeScale::from_config(&config, 1000.0);
        assert!((ts.tick_rate_hz() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn time_scale_clamps_zero_peg_to_one_hz() {
        let config = SimConfig::default();
        let ts = TimeScale::from_config(&config, 0.0);
        assert_eq!(ts.tick_rate_hz(), 1.0);
    }

    #[test]
    fn sim_speed_wall_seconds_inverts_ticks_per_second() {
        let config = SimConfig::default();
        assert!((SimSpeed::Normal.wall_seconds_per_game_day(&config) - 1000.0).abs() < 1e-6);
        assert!((SimSpeed::Fast.wall_seconds_per_game_day(&config) - 200.0).abs() < 1e-6);
        assert!((SimSpeed::VeryFast.wall_seconds_per_game_day(&config) - 50.0).abs() < 1e-6);
    }

    #[test]
    fn week_number_from_tick() {
        let config = SimConfig::default(); // 1000 ticks/day
        assert_eq!(TimeState::week_number(0, &config), 1); // Day 1 → Week 1
        assert_eq!(TimeState::week_number(6999, &config), 1); // Day 7 → Week 1
        assert_eq!(TimeState::week_number(7000, &config), 2); // Day 8 → Week 2
        assert_eq!(TimeState::week_number(13999, &config), 2); // Day 14 → Week 2
    }
}
