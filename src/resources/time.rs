use bevy_ecs::prelude::Resource;

// ---------------------------------------------------------------------------
// SimConfig
// ---------------------------------------------------------------------------

/// Simulation configuration constants. Stored as a resource; inject this into
/// any system that needs to convert raw ticks into human-readable time.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SimConfig {
    /// Number of ticks per day phase (Dawn / Day / Dusk / Night).
    /// Default 25 → a full day is 100 ticks.
    pub ticks_per_day_phase: u64,
    /// Number of ticks per season (Spring / Summer / Autumn / Winter).
    /// Default 2000 → a full year is 8000 ticks.
    pub ticks_per_season: u64,
    /// RNG seed for reproducible runs.
    pub seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            ticks_per_day_phase: 25,
            ticks_per_season: 2000,
            seed: 42,
        }
    }
}

// ---------------------------------------------------------------------------
// DayPhase
// ---------------------------------------------------------------------------

/// The four phases of the in-game day, cycling in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

// ---------------------------------------------------------------------------
// Season
// ---------------------------------------------------------------------------

/// The four seasons, cycling in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimSpeed {
    Normal,
    Fast,
    VeryFast,
}

impl SimSpeed {
    /// Ticks to advance per update at this speed.
    pub fn ticks_per_update(self) -> u64 {
        match self {
            Self::Normal => 1,
            Self::Fast => 5,
            Self::VeryFast => 20,
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
}

impl Default for SimSpeed {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// TimeState
// ---------------------------------------------------------------------------

/// Global simulation clock. Advance `tick` each update; everything else is
/// derived.
#[derive(Resource, Debug, Clone)]
pub struct TimeState {
    pub tick: u64,
    pub paused: bool,
    pub speed: SimSpeed,
}

impl Default for TimeState {
    fn default() -> Self {
        Self {
            tick: 0,
            paused: false,
            speed: SimSpeed::default(),
        }
    }
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
        assert_eq!(DayPhase::from_tick(24, &config), DayPhase::Dawn);
        assert_eq!(DayPhase::from_tick(25, &config), DayPhase::Day);
        assert_eq!(DayPhase::from_tick(50, &config), DayPhase::Dusk);
        assert_eq!(DayPhase::from_tick(75, &config), DayPhase::Night);
        assert_eq!(DayPhase::from_tick(100, &config), DayPhase::Dawn); // wraps
    }

    #[test]
    fn season_from_tick() {
        let config = SimConfig::default();
        assert_eq!(Season::from_tick(0, &config), Season::Spring);
        assert_eq!(Season::from_tick(1999, &config), Season::Spring);
        assert_eq!(Season::from_tick(2000, &config), Season::Summer);
        assert_eq!(Season::from_tick(4000, &config), Season::Autumn);
        assert_eq!(Season::from_tick(6000, &config), Season::Winter);
        assert_eq!(Season::from_tick(8000, &config), Season::Spring); // wraps
    }

    #[test]
    fn day_number_from_tick() {
        let config = SimConfig::default();
        assert_eq!(TimeState::day_number(0, &config), 1);
        assert_eq!(TimeState::day_number(99, &config), 1);
        assert_eq!(TimeState::day_number(100, &config), 2);
    }

    #[test]
    fn sim_speed_cycle() {
        assert_eq!(SimSpeed::Normal.cycle(), SimSpeed::Fast);
        assert_eq!(SimSpeed::Fast.cycle(), SimSpeed::VeryFast);
        assert_eq!(SimSpeed::VeryFast.cycle(), SimSpeed::Normal);
    }

    #[test]
    fn sim_speed_ticks_per_update() {
        assert_eq!(SimSpeed::Normal.ticks_per_update(), 1);
        assert_eq!(SimSpeed::Fast.ticks_per_update(), 5);
        assert_eq!(SimSpeed::VeryFast.ticks_per_update(), 20);
    }

    #[test]
    fn time_state_derived_accessors() {
        let config = SimConfig::default();
        let mut ts = TimeState::default();
        ts.tick = 75;
        assert_eq!(ts.day_phase(&config), DayPhase::Night);
        assert_eq!(ts.season(&config), Season::Spring);
        assert_eq!(TimeState::day_number(ts.tick, &config), 1);
    }
}
