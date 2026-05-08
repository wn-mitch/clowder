//! Ticket 035 — `Grave` component for buried colony-mates.
//!
//! Spawned by `goap.rs::resolve_goap_plans`'s post-loop drain when a
//! `BuryOutcome` is consumed: the deceased entity is despawned, and a
//! fresh entity carrying `Grave` + `Position` is spawned at the corpse
//! tile. Grave entities persist indefinitely; downstream tickets
//! (gravesite selection / monument landmarks / kitten-rest-at-grave)
//! compose against this anchor without churning the foundation.
//!
//! Foundation behavior in this ticket:
//! - Grave entities feed `GraveAuraMap` (a `WardCoverageMap`-shaped
//!   `InfluenceMap`) so the spatial influence layer can model a small
//!   anti-corruption aura around graves. The map is registered in
//!   `populate_influence_map_registry` and recomputed each tick.
//! - The `deceased_name` field links a Grave back to the
//!   `Relationships` table by name — survivors with bonds to the
//!   deceased can identify their grave, supporting the future
//!   `LostBonds` / rest-at-grave chain.

use bevy_ecs::prelude::*;

use crate::components::physical::DeathCause;

/// A buried colony-member's resting place. Spawned at the corpse
/// tile when `resolve_bury` completes; despawned never (foundation
/// scope). Carries the deceased's identity + cause for downstream
/// rituals + memorialization.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Grave {
    /// Display name of the buried cat. Matches `Name.0` at time of
    /// death; persists even after the original entity is despawned so
    /// `Relationships` lookups by name remain meaningful for
    /// surviving kin.
    pub deceased_name: String,
    /// Tick the burial completed. `tick - tick_buried` measures
    /// grave-age for any future weathering / decay logic.
    pub tick_buried: u64,
    /// How the deceased died. Reserved for ritual-tier and
    /// narrative differentiation in follow-on tickets.
    pub cause: DeathCause,
}

impl Grave {
    pub const KEY: &str = "Grave";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grave_construction() {
        let g = Grave {
            deceased_name: "Hazel".into(),
            tick_buried: 1_200_500,
            cause: DeathCause::OldAge,
        };
        assert_eq!(g.deceased_name, "Hazel");
        assert_eq!(g.tick_buried, 1_200_500);
        assert_eq!(g.cause, DeathCause::OldAge);
    }

    #[test]
    fn grave_serializes_round_trip() {
        let g = Grave {
            deceased_name: "Bigwig".into(),
            tick_buried: 42,
            cause: DeathCause::Injury,
        };
        let json = serde_json::to_string(&g).expect("serialize");
        let g2: Grave = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(g.deceased_name, g2.deceased_name);
        assert_eq!(g.tick_buried, g2.tick_buried);
        assert_eq!(g.cause, g2.cause);
    }
}
