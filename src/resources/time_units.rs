//! Typed time-unit wrappers (ticket 033).
//!
//! Make ticks ↔ in-game time ↔ wall-clock a compile-time invariant.
//!
//! Every temporal constant in `sim_constants.rs` migrates to one of
//! these newtypes. Consumers cannot multiply a [`RatePerDay`] into a
//! stat directly — the type refuses `f32` arithmetic — they must call
//! `.per_tick(time_scale)` to convert into a per-tick rate via the
//! [`TimeScale`] resource. Same idea for [`DurationDays`] (`.ticks(ts)`)
//! and [`IntervalPerDay`] (`.fires_at(tick, ts)` / `.ticks(ts)`).
//!
//! Why: the 2026-04-10 100→1000 ticks/day overhaul missed three
//! `evaluate_interval = 100` consts because nothing forced consumers
//! through a converter. Centralized storage isn't enough — we need the
//! type system to refuse raw arithmetic. Same playbook as
//! `StepOutcome<W>` (silent-advance bug → type error).
//!
//! Design choices:
//! - **No `Default::default()` returning a magic value.** Only
//!   `pub const fn new(value)` constructors. A `Default` of `0.0` would
//!   reintroduce the silent-do-nothing failure shape we're preventing.
//! - **No `From<f32>` or `Into<f32>` impls.** Conversion is named
//!   (`per_tick`, `ticks`, `fires_at`) so reviewers see every unit
//!   transition explicitly. Matches `record_if_witnessed`'s ergonomics
//!   in the GOAP step contract.
//! - **No `raw()` accessor.** Serde derives handle JSON I/O; the type
//!   should never need to "leak" its inner scalar to a consumer. Add
//!   one only when a real call site demands it (and even then, prefer
//!   adding the operation as a method on the wrapper).
//! - **`Ticks` is a `u64` newtype, not a type alias.** Forces `tick.0`
//!   to drop the wrapper, which the gate flags outside `time*.rs`.
//!
//! See `docs/systems/time-anchor.md` for the canonical reference and
//! `docs/open-work/tickets/033-time-unit-typing.md` for phase status.

use serde::{Deserialize, Serialize};

use crate::resources::time::TimeScale;

// ---------------------------------------------------------------------------
// RatePerDay — drains, decays, regens.
// ---------------------------------------------------------------------------

/// A drain / decay / regen rate expressed per **in-game day**.
///
/// Example: `RatePerDay::new(0.1)` means "this stat moves by 0.1 over
/// one in-game day," i.e. 10 in-game days to span a `0.0..=1.0` range.
/// Conversion to per-tick goes through [`TimeScale`].
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RatePerDay(f32);

impl RatePerDay {
    pub const fn new(per_day: f32) -> Self {
        Self(per_day)
    }

    /// Convert to a per-tick rate using the active [`TimeScale`].
    pub fn per_tick(self, ts: &TimeScale) -> f32 {
        self.0 / ts.ticks_per_day() as f32
    }
}

// ---------------------------------------------------------------------------
// DurationDays — durations measured in in-game days.
// ---------------------------------------------------------------------------

/// A duration measured in **in-game days**.
///
/// Example: `DurationDays::new(2.0)` means "two in-game days." Convert
/// to a tick count via [`TimeScale`].
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DurationDays(f32);

impl DurationDays {
    pub const fn new(days: f32) -> Self {
        Self(days)
    }

    /// Tick count corresponding to this duration under the active
    /// [`TimeScale`]. Truncates toward zero.
    pub fn ticks(self, ts: &TimeScale) -> u64 {
        (self.0 * ts.ticks_per_day() as f32) as u64
    }
}

// ---------------------------------------------------------------------------
// DurationSeasons — durations measured in in-game seasons.
// ---------------------------------------------------------------------------

/// A duration measured in **in-game seasons** (use for season-scoped
/// cycles like fertility, growth, life stages).
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DurationSeasons(f32);

impl DurationSeasons {
    pub const fn new(seasons: f32) -> Self {
        Self(seasons)
    }

    /// Tick count corresponding to this duration under the active
    /// [`TimeScale`]. Truncates toward zero.
    pub fn ticks(self, ts: &TimeScale) -> u64 {
        (self.0 * ts.ticks_per_season() as f32) as u64
    }
}

// ---------------------------------------------------------------------------
// IntervalPerDay — cadences, "fires N times per in-game day."
// ---------------------------------------------------------------------------

/// A cadence: "fires N times per **in-game day**."
///
/// Example: `IntervalPerDay::new(1.0)` fires once per in-game day. The
/// old `if tick % 100 == 0 && tick > 0` idiom (at the old 100-tick day)
/// is `IntervalPerDay::new(1.0).fires_at(tick, ts)` at the current
/// scale.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntervalPerDay(f32);

impl IntervalPerDay {
    pub const fn new(per_day: f32) -> Self {
        Self(per_day)
    }

    /// Tick count between firings under the active [`TimeScale`].
    /// Returns at least 1 to avoid divide-by-zero / hot-loop firing for
    /// pathologically large `per_day`.
    pub fn ticks(self, ts: &TimeScale) -> u64 {
        if self.0 <= 0.0 {
            // A non-positive cadence is never-fires; encode as u64::MAX
            // so `fires_at` returns false except on tick 0 (which we
            // also guard against).
            return u64::MAX;
        }
        let raw = (ts.ticks_per_day() as f32 / self.0) as u64;
        raw.max(1)
    }

    /// True iff `tick` is a firing tick. Skips tick 0 (matches the
    /// existing `tick > 0 && tick.is_multiple_of(N)` idiom across the
    /// codebase).
    pub fn fires_at(self, tick: u64, ts: &TimeScale) -> bool {
        if tick == 0 || self.0 <= 0.0 {
            return false;
        }
        tick.is_multiple_of(self.ticks(ts))
    }
}

// ---------------------------------------------------------------------------
// Ticks — wraps a raw tick count.
// ---------------------------------------------------------------------------

/// Wraps a raw tick count. Prefer [`DurationDays`] /
/// [`DurationSeasons`] for storage; `Ticks` is for runtime values
/// (e.g. `TimeState::tick`) and the rare hardcoded-tick case.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ticks(pub u64);

impl Ticks {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::time::SimConfig;

    /// Default scale: 1000 ticks/day, 20000 ticks/season.
    fn default_scale() -> TimeScale {
        TimeScale::from_config(&SimConfig::default(), 16.6667)
    }

    // ---- RatePerDay ----

    #[test]
    fn rate_per_day_converts_at_default_scale() {
        let ts = default_scale();
        // 0.1/day at 1000 ticks/day = 0.0001/tick.
        let r = RatePerDay::new(0.1);
        assert!((r.per_tick(&ts) - 0.0001).abs() < 1e-9);
    }

    #[test]
    fn rate_per_day_round_trip() {
        let ts = default_scale();
        let original = RatePerDay::new(0.25);
        let per_tick = original.per_tick(&ts);
        let recovered = per_tick * ts.ticks_per_day() as f32;
        assert!((recovered - 0.25).abs() < 1e-6);
    }

    #[test]
    fn rate_per_day_scales_with_ticks_per_day() {
        // Same per-day rate ≡ the same per-day behavior, regardless of
        // how many ticks subdivide the day.
        let r = RatePerDay::new(1.0);
        let mut config = SimConfig::default();
        config.ticks_per_day_phase = 25; // Old 100-ticks/day scale.
        let old_scale = TimeScale::from_config(&config, 1.667);
        let new_scale = default_scale(); // 1000 ticks/day.

        let old_per_tick = r.per_tick(&old_scale);
        let new_per_tick = r.per_tick(&new_scale);

        // Old scale drains 10× faster per-tick — but each tick represents
        // 10× the in-game time.
        assert!((old_per_tick / new_per_tick - 10.0).abs() < 1e-4);
    }

    // ---- DurationDays ----

    #[test]
    fn duration_days_converts_at_default_scale() {
        let ts = default_scale();
        assert_eq!(DurationDays::new(1.0).ticks(&ts), 1000);
        assert_eq!(DurationDays::new(2.5).ticks(&ts), 2500);
        assert_eq!(DurationDays::new(0.0).ticks(&ts), 0);
    }

    #[test]
    fn duration_days_truncates_fractional() {
        let ts = default_scale();
        // 0.0005 days × 1000 ticks/day = 0.5 ticks → truncates to 0.
        assert_eq!(DurationDays::new(0.0005).ticks(&ts), 0);
    }

    // ---- DurationSeasons ----

    #[test]
    fn duration_seasons_scales_with_config() {
        let ts = default_scale(); // 20000 ticks/season.
        assert_eq!(DurationSeasons::new(1.0).ticks(&ts), 20_000);
        assert_eq!(DurationSeasons::new(0.5).ticks(&ts), 10_000);
        assert_eq!(DurationSeasons::new(4.0).ticks(&ts), 80_000);

        let mut config = SimConfig::default();
        config.ticks_per_season = 2000; // Test-scale.
        let small = TimeScale::from_config(&config, 16.6667);
        assert_eq!(DurationSeasons::new(0.5).ticks(&small), 1000);
    }

    // ---- IntervalPerDay ----

    #[test]
    fn interval_per_day_ticks() {
        let ts = default_scale(); // 1000 ticks/day.
        assert_eq!(IntervalPerDay::new(1.0).ticks(&ts), 1000);
        assert_eq!(IntervalPerDay::new(10.0).ticks(&ts), 100);
        assert_eq!(IntervalPerDay::new(4.0).ticks(&ts), 250);
    }

    #[test]
    fn interval_per_day_matches_old_modulo_semantics() {
        let ts = default_scale();
        // The old `tick % 100 == 0 && tick > 0` is `IntervalPerDay(10/day)`
        // at the new 1000-ticks/day scale.
        let interval = IntervalPerDay::new(10.0);
        assert!(!interval.fires_at(0, &ts), "tick 0 never fires");
        assert!(!interval.fires_at(50, &ts));
        assert!(interval.fires_at(100, &ts));
        assert!(interval.fires_at(200, &ts));
        assert!(!interval.fires_at(150, &ts));
    }

    #[test]
    fn interval_per_day_clamps_zero_or_negative() {
        let ts = default_scale();
        let never = IntervalPerDay::new(0.0);
        assert!(!never.fires_at(0, &ts));
        assert!(!never.fires_at(1000, &ts));
        assert!(!never.fires_at(u64::MAX - 1, &ts));
        assert_eq!(never.ticks(&ts), u64::MAX);
    }

    #[test]
    fn interval_per_day_clamps_to_minimum_tick() {
        let ts = default_scale(); // 1000 ticks/day.
        // 100000/day → would be 0.01 ticks, clamps to 1.
        let blistering = IntervalPerDay::new(100_000.0);
        assert_eq!(blistering.ticks(&ts), 1);
        assert!(blistering.fires_at(1, &ts));
    }

    // ---- Ticks ----

    #[test]
    fn ticks_newtype_round_trip() {
        let t = Ticks::new(54_000);
        assert_eq!(t.get(), 54_000);
        assert_eq!(t.0, 54_000);
    }

    // ---- Serde ----

    #[test]
    fn rate_per_day_serializes_as_inner_f32() {
        let r = RatePerDay::new(0.125);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "0.125");
        let recovered: RatePerDay = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, r);
    }

    #[test]
    fn duration_days_round_trip() {
        let d = DurationDays::new(2.5);
        let json = serde_json::to_string(&d).unwrap();
        let recovered: DurationDays = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, d);
    }

    #[test]
    fn duration_seasons_serializes_as_inner_f32() {
        let d = DurationSeasons::new(0.5);
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "0.5");
    }

    #[test]
    fn interval_per_day_round_trip() {
        let i = IntervalPerDay::new(4.0);
        let json = serde_json::to_string(&i).unwrap();
        let recovered: IntervalPerDay = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, i);
    }

    #[test]
    fn ticks_serializes_as_inner_u64() {
        let t = Ticks::new(42);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "42");
    }
}
