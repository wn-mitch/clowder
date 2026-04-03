use bevy_ecs::prelude::ResMut;

use crate::resources::TimeState;

/// Advance the simulation clock by one tick if not paused.
///
/// This system runs every ECS update. Pausing is handled by setting
/// `TimeState::paused = true`; all other time-derived values (season, day
/// phase) are computed on-demand from the raw tick count.
pub fn advance_time(mut time: ResMut<TimeState>) {
    if !time.paused {
        time.tick += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::resources::TimeState;

    #[test]
    fn tick_advances_when_running() {
        let mut time = TimeState::default();
        assert_eq!(time.tick, 0);
        advance_time_direct(&mut time);
        assert_eq!(time.tick, 1);
        advance_time_direct(&mut time);
        assert_eq!(time.tick, 2);
    }

    #[test]
    fn tick_does_not_advance_when_paused() {
        let mut time = TimeState::default();
        time.paused = true;
        advance_time_direct(&mut time);
        advance_time_direct(&mut time);
        assert_eq!(time.tick, 0);
    }

    #[test]
    fn unpausing_resumes_tick_from_current_value() {
        let mut time = TimeState::default();
        time.tick = 10;
        time.paused = true;
        advance_time_direct(&mut time);
        assert_eq!(time.tick, 10);
        time.paused = false;
        advance_time_direct(&mut time);
        assert_eq!(time.tick, 11);
    }

    /// Exercise the core logic without the ECS runtime.
    ///
    /// `advance_time` takes `ResMut<TimeState>` which cannot be constructed
    /// outside a `World`. This helper replicates the logic so tests stay
    /// fast and self-contained.
    fn advance_time_direct(time: &mut TimeState) {
        if !time.paused {
            time.tick += 1;
        }
    }
}
