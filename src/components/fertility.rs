//! Fertility component — §7.M.7 of `docs/systems/ai-substrate-refactor.md`.
//!
//! Data-bearing per-cat state driving the L3 `MateWithGoal` hard gate
//! (§7.M.7.6). Present on Queens and Nonbinaries from Adult entry
//! through Elder exit (§7.M.7.1); absent on Toms by construction
//! (§7.M.7.4 — Toms are implicit "always-on in non-winter" via the
//! §7.M.7.5 fallback). Mutually exclusive with `Pregnant` — conception
//! removes the component; birth re-inserts it in `Postpartum`.
//!
//! The phase is a **pure function** of (cycle_tick, season,
//! post_partum) per §7.M.7.2, evaluated by
//! `src/systems/fertility.rs::update_fertility_phase` at
//! `FertilityConstants::update_interval` cadence (once per in-game
//! day).

use bevy_ecs::prelude::*;

/// Five-variant phase enum (§7.M.7.3 expansion). `Anestrus` and
/// `Postpartum` share the `0.0` receptivity mapping (§7.M.7.5) but
/// are distinguishable narratively and in `events.jsonl` filtering —
/// environmental vs. biological suppression are different phenomena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FertilityPhase {
    /// Rising receptivity — first ~15% of cycle.
    Proestrus,
    /// Peak receptivity — next ~20% of cycle.
    Estrus,
    /// Refractory — remaining ~65% of cycle.
    Diestrus,
    /// Environmental suppression — winter only.
    Anestrus,
    /// Biological suppression — nursing interval post-birth.
    Postpartum,
}

impl FertilityPhase {
    /// §7.M.7.5 per-target receptivity scalar (pre-Logistic) before
    /// the §6.5.2 fertility-window consideration maps it through the
    /// curve. The consumer multiplies by the environmental
    /// `mating_fertility_{season}` factor before entering the Logistic.
    pub fn receptivity(self) -> f32 {
        match self {
            Self::Estrus => 1.0,
            Self::Proestrus => 0.5,
            Self::Diestrus => 0.1,
            Self::Anestrus | Self::Postpartum => 0.0,
        }
    }

    /// §7.M.7.6 hard-gate predicate. The §L2.10 `MateWithGoal`
    /// firing-condition hard gate opens only when at least one
    /// partner has a viable phase — this predicate returns `true`
    /// exactly for the three phases the gate accepts.
    pub fn is_viable_for_conception(self) -> bool {
        matches!(self, Self::Proestrus | Self::Estrus | Self::Diestrus)
    }
}

/// Per-cat fertility state. Carried only by Queens + Nonbinaries in
/// the Adult / Elder life stages (outside of pregnancy).
///
/// `cycle_offset` is spawn-immutable (per-cat constant derived at
/// insertion from the entity index) so that no two colony cats peak
/// simultaneously — desynchronization is load-bearing for the spec's
/// "cats visibly come into season" narrative goal (§7.M.7.8).
///
/// `post_partum_remaining_ticks` counts down from
/// `post_partum_recovery_ticks` on birth re-insert; when > 0 the
/// transition function pins `phase = Postpartum` (rule 2 of §7.M.7.2).
#[derive(Component, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Fertility {
    pub phase: FertilityPhase,
    pub cycle_offset: u64,
    pub post_partum_remaining_ticks: u32,
}

impl Fertility {
    /// Build fresh Fertility on Adult entry. `cycle_offset` is derived
    /// deterministically from the entity bits via the §7.M.7.3
    /// golden-ratio constant, producing per-cat desynchronization
    /// without an RNG read.
    pub fn on_adult_entry(entity_bits: u64, initial_phase: FertilityPhase) -> Self {
        const GOLDEN_RATIO_MIX: u64 = 0x9E37_79B9_7F4A_7C15;
        let cycle_offset = entity_bits.wrapping_mul(GOLDEN_RATIO_MIX);
        Self {
            phase: initial_phase,
            cycle_offset,
            post_partum_remaining_ticks: 0,
        }
    }

    /// Build Postpartum Fertility on birth re-insert. Carries forward
    /// the pre-pregnancy `cycle_offset` so the cat's desynchronization
    /// identity is stable across pregnancy boundaries.
    pub fn on_post_partum(cycle_offset: u64, recovery_ticks: u32) -> Self {
        Self {
            phase: FertilityPhase::Postpartum,
            cycle_offset,
            post_partum_remaining_ticks: recovery_ticks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receptivity_matches_spec_7_m_7_5() {
        // §7.M.7.5 table: Estrus 1.0, Proestrus 0.5, Diestrus 0.1,
        // Anestrus 0.0, Postpartum 0.0.
        assert_eq!(FertilityPhase::Estrus.receptivity(), 1.0);
        assert_eq!(FertilityPhase::Proestrus.receptivity(), 0.5);
        assert_eq!(FertilityPhase::Diestrus.receptivity(), 0.1);
        assert_eq!(FertilityPhase::Anestrus.receptivity(), 0.0);
        assert_eq!(FertilityPhase::Postpartum.receptivity(), 0.0);
    }

    #[test]
    fn viability_excludes_anestrus_and_postpartum() {
        // §7.M.7.6 hard-gate: exactly {Proestrus, Estrus, Diestrus}
        // are viable; {Anestrus, Postpartum} are not.
        assert!(FertilityPhase::Proestrus.is_viable_for_conception());
        assert!(FertilityPhase::Estrus.is_viable_for_conception());
        assert!(FertilityPhase::Diestrus.is_viable_for_conception());
        assert!(!FertilityPhase::Anestrus.is_viable_for_conception());
        assert!(!FertilityPhase::Postpartum.is_viable_for_conception());
    }

    #[test]
    fn cycle_offset_differs_across_entity_indices() {
        // Desynchronization: two cats with adjacent indices should
        // not end up on the same cycle phase at the same tick.
        let a = Fertility::on_adult_entry(42, FertilityPhase::Proestrus);
        let b = Fertility::on_adult_entry(43, FertilityPhase::Proestrus);
        assert_ne!(a.cycle_offset, b.cycle_offset);
    }

    #[test]
    fn post_partum_constructor_preserves_cycle_offset() {
        // `on_post_partum` carries forward the pre-pregnancy offset
        // so a cat's identity is stable across birth boundaries.
        let offset = 0x1234_5678_u64;
        let f = Fertility::on_post_partum(offset, 5000);
        assert_eq!(f.cycle_offset, offset);
        assert_eq!(f.phase, FertilityPhase::Postpartum);
        assert_eq!(f.post_partum_remaining_ticks, 5000);
    }
}
