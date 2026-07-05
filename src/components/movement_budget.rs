//! `MovementBudget` — per-entity step accumulator (ticket 138, Phase 1 of
//! the 135 continuous-position-migration epic).
//!
//! Each entity that occupies a tile gains a `MovementBudget` carrying an
//! `accumulator: f32` and a species-specific `per_tick: f32` rate. Each
//! tick `accumulate_movement_budget` adds `per_tick` to `accumulator`;
//! a movement consumer calls `try_spend_step` before writing a new
//! `Position`. The call succeeds iff `accumulator >= 1.0`, and decrements
//! the accumulator by `1.0`. Sub-unit `per_tick` (e.g. `0.5` for snakes)
//! produces the right gameplay shape on the existing integer grid — a
//! snake at `per_tick = 0.5` steps every other tick.
//!
//! This is Phase 1 of the migration: the sim stays on the integer grid;
//! the budget gates step opportunity. Phase 2 (#139) will migrate
//! `Position` itself to `Vec2<f32>`.
//!
//! ## Lifecycle
//!
//! - **Cat spawn**: inserted by `cat_bundle` (`src/plugins/setup.rs`) with
//!   `MovementBudget::cat()` — `per_tick = 1.0`, `accumulator = 1.0` so
//!   freshly-spawned cats can step on their first tick.
//! - **Wildlife spawn**: inserted by the `on_wild_animal_added` observer
//!   (`src/systems/movement_budget.rs`) — reads `WildAnimal::species` and
//!   picks the species default via `WildSpecies::default_movement_budget`.
//!   Zero call-site churn across the ~20 `WildAnimal::new` sites.
//! - **Save-loaded entities (pre-138 saves)**: lazy-insert path in
//!   `accumulate_movement_budget` for entities missing the component
//!   (mirrors the `PrevSafetyDeficit` precedent from ticket 108).
//!
//! ## The "blocked step retains accumulated budget" semantics
//!
//! `try_spend_step` only decrements when the caller will actually write
//! the new position. A snake that *could* step but is blocked by terrain
//! retains its accumulated budget for the next tick. The budget models
//! physical capacity to translate, not intent.

use bevy_ecs::prelude::*;

use crate::components::wildlife::WildSpecies;

/// Per-entity step-opportunity accumulator. Ticked by
/// `accumulate_movement_budget`; spent by movement consumers.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MovementBudget {
    /// Accumulated fractional steps. A step costs `1.0`. Held over
    /// across ticks when no step occurs.
    pub accumulator: f32,
    /// Per-tick rate (`1.0` = every tick, `0.5` = every other tick,
    /// `1.5` = three steps per two ticks).
    pub per_tick: f32,
}

impl MovementBudget {
    /// Cat default: full-cadence (1.0 per tick), with a primed
    /// accumulator so freshly-spawned cats can step on their first tick.
    /// Per-cat-cadence (sprightly elders, lumbering hunters) is out of
    /// scope for this phase per ticket 138.
    pub fn cat() -> Self {
        Self {
            accumulator: 1.0,
            per_tick: 1.0,
        }
    }

    /// Species default for wildlife. Reads
    /// [`crate::resources::sim_constants::MovementConstants::max_speed`]
    /// (ticket 140 retired the hardcoded `default_movement_budget`
    /// match — speeds are tuning knobs now). Accumulator is primed
    /// at `per_tick` (mirrors the cat case — freshly-spawned wildlife
    /// can step on their first tick if their cadence allows).
    pub fn for_species(
        species: WildSpecies,
        movement: &crate::resources::sim_constants::MovementConstants,
    ) -> Self {
        let per_tick = movement.max_speed(species);
        Self {
            accumulator: per_tick,
            per_tick,
        }
    }

    /// Try to spend `1.0` for a step. Returns `true` and decrements the
    /// accumulator iff `accumulator >= 1.0`. Returns `false` without
    /// mutating otherwise. Callers must NOT decrement manually — the
    /// "blocked step retains budget" semantics depend on this gate being
    /// the only spend path.
    pub fn try_spend_step(&mut self) -> bool {
        if self.accumulator >= 1.0 {
            self.accumulator -= 1.0;
            true
        } else {
            false
        }
    }

    /// Per-tick accumulation. Capped at `2.0 * per_tick` so a stationary
    /// entity can't bank arbitrarily many steps for a future burst.
    /// (Future burst-ability tickets will introduce their own
    /// accumulator semantics.)
    pub fn accumulate(&mut self) {
        self.accumulator = (self.accumulator + self.per_tick).min(2.0 * self.per_tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cat_default_is_full_cadence() {
        let b = MovementBudget::cat();
        assert!((b.per_tick - 1.0).abs() < f32::EPSILON);
        assert!(b.accumulator >= 1.0);
    }

    #[test]
    fn snake_default_is_half_cadence() {
        let b = MovementBudget::for_species(
            WildSpecies::Snake,
            &crate::resources::sim_constants::MovementConstants::default(),
        );
        assert!((b.per_tick - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn fox_hawk_shadowfox_default_to_full_cadence() {
        for s in [WildSpecies::Fox, WildSpecies::Hawk, WildSpecies::ShadowFox] {
            assert!(
                (MovementBudget::for_species(
                    s,
                    &crate::resources::sim_constants::MovementConstants::default()
                )
                .per_tick
                    - 1.0)
                    .abs()
                    < f32::EPSILON,
                "{} should default to 1.0 per tick",
                s.name()
            );
        }
    }

    #[test]
    fn try_spend_succeeds_at_unit_budget() {
        let mut b = MovementBudget {
            accumulator: 1.0,
            per_tick: 1.0,
        };
        assert!(b.try_spend_step());
        assert!((b.accumulator - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn try_spend_fails_under_unit() {
        let mut b = MovementBudget {
            accumulator: 0.7,
            per_tick: 0.5,
        };
        assert!(!b.try_spend_step());
        assert!(
            (b.accumulator - 0.7).abs() < f32::EPSILON,
            "blocked step retains budget"
        );
    }

    #[test]
    fn half_cadence_steps_every_other_tick() {
        let mut b = MovementBudget::for_species(
            WildSpecies::Snake,
            &crate::resources::sim_constants::MovementConstants::default(),
        );
        // Tick 0: primed at 0.5 — cannot step.
        assert!(!b.try_spend_step());
        b.accumulate();
        // Tick 1: 0.5 + 0.5 = 1.0 — can step.
        assert!(b.try_spend_step());
        b.accumulate();
        // Tick 2: 0.0 + 0.5 = 0.5 — cannot step.
        assert!(!b.try_spend_step());
        b.accumulate();
        // Tick 3: 0.5 + 0.5 = 1.0 — can step.
        assert!(b.try_spend_step());
    }

    #[test]
    fn accumulator_caps_to_prevent_burst_banking() {
        let mut b = MovementBudget {
            accumulator: 5.0,
            per_tick: 1.0,
        };
        b.accumulate();
        assert!(b.accumulator <= 2.0, "should cap at 2 * per_tick");
    }
}
