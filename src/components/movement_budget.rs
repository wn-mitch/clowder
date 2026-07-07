//! `MovementBudget` — per-entity speed cap (ticket 138 Phase 1,
//! reinterpreted by the 140 fluid-movement migration, slimmed by
//! 140 step 13).
//!
//! History: Phase 1 (#138) carried an `accumulator: f32` stepped by a
//! per-tick system and spent through `try_spend_step` — fractional
//! step-opportunity gating on the integer grid (a snake at
//! `per_tick = 0.5` stepped every other tick). The 140 migration
//! (steps 6–12) moved every mover to `DesiredVelocity` + the
//! `integrate_velocities` integrator, which reinterprets `per_tick`
//! as the entity's **maximum speed in tiles/tick** (the Euclidean
//! clamp on integrated velocity). Step 13 deleted the accumulator,
//! `try_spend_step`, and the per-tick accumulation pass — the only
//! surviving field is the speed cap.
//!
//! ## Readers
//!
//! - `integrate_velocities` (`src/systems/movement.rs`) — the speed
//!   clamp: `per_tick × sprint_speed_mult × terrain_mult` is the hard
//!   ceiling on per-tick displacement.
//! - `escape_viability` (`src/systems/interoception.rs`) — reads
//!   `per_tick` as the cat's top speed when scoring flee viability.
//!
//! ## Authors (one per lifecycle path)
//!
//! - **Cat spawn**: `cat_bundle` (`src/plugins/setup.rs`) —
//!   `MovementBudget::cat()`.
//! - **Wildlife spawn**: `on_wild_animal_added` observer — species
//!   speed from `MovementConstants::max_speed`.
//! - **Prey spawn**: `on_prey_animal_added` observer — ground prey at
//!   `prey_ground_max_speed × flee_speed`, birds at
//!   `bird_burst_speed`.
//! - **Save-loaded entities**: lazy-insert in
//!   `insert_missing_movement_components`
//!   (`src/systems/movement_budget.rs`).

use bevy_ecs::prelude::*;

use crate::components::wildlife::WildSpecies;

/// Per-entity maximum speed in tiles/tick. Read by the integrator as
/// the displacement clamp and by interoception as the flee top-speed.
///
/// Serde note: pre-140-step-13 saves carry an extra `accumulator`
/// field — serde's default unknown-field tolerance drops it on load.
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MovementBudget {
    /// Maximum speed (tiles/tick). `1.0` = one tile per tick at base
    /// gait; the integrator's gait/terrain multipliers scale from it.
    pub per_tick: f32,
}

impl MovementBudget {
    /// Cat default: 1.0 tiles/tick base speed. Per-cat cadence
    /// (sprightly elders, lumbering hunters) remains out of scope per
    /// ticket 138.
    pub fn cat() -> Self {
        Self { per_tick: 1.0 }
    }

    /// Species default for wildlife. Reads
    /// [`crate::resources::sim_constants::MovementConstants::max_speed`]
    /// (ticket 140 retired the hardcoded `default_movement_budget`
    /// match — speeds are tuning knobs now).
    pub fn for_species(
        species: WildSpecies,
        movement: &crate::resources::sim_constants::MovementConstants,
    ) -> Self {
        Self {
            per_tick: movement.max_speed(species),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cat_default_is_unit_speed() {
        let b = MovementBudget::cat();
        assert!((b.per_tick - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn snake_default_is_half_speed() {
        let b = MovementBudget::for_species(
            WildSpecies::Snake,
            &crate::resources::sim_constants::MovementConstants::default(),
        );
        assert!((b.per_tick - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn fox_hawk_shadowfox_default_to_unit_speed() {
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
                "{} should default to 1.0 tiles/tick",
                s.name()
            );
        }
    }

    /// Pre-step-13 saves serialize `{ accumulator, per_tick }`; the
    /// slimmed struct must still deserialize them (serde drops the
    /// unknown field by default). A `deny_unknown_fields` regression
    /// here would break every pre-140 save on load.
    #[test]
    fn deserializes_pre_step13_save_shape() {
        let b: MovementBudget =
            serde_json::from_str(r#"{"accumulator": 1.7, "per_tick": 0.5}"#).unwrap();
        assert!((b.per_tick - 0.5).abs() < f32::EPSILON);
    }
}
