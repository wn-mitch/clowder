//! Faction model — §9 of `docs/systems/ai-substrate-refactor.md`.
//!
//! Phase 3a scaffolding: the `FactionStance` enum + `StanceRequirement`
//! shape land here so the [`super::dse::EligibilityFilter`] can name a
//! stance-based gate. The full §9.1 10×10 biological base matrix, the
//! §9.2 ECS-marker overlay (Visitor / HostileVisitor / Banished /
//! BefriendedAlly), and the most-negative-wins stance resolver ship in
//! task #7 (a separate commit).

// ---------------------------------------------------------------------------
// FactionStance
// ---------------------------------------------------------------------------

/// Base stance between an observer species and a target species. The
/// full 100-cell directed matrix lives in `FactionRelations` (Phase 3a
/// task #7); this enum is the value-shape every cell holds.
///
/// Ordered so `as_negativity_rank` can implement §9.2's
/// "most-negative-wins" overlay resolution — `Enemy` ≻ `Predator` ≻
/// `Prey` ≻ `Neutral` ≻ `Ally` ≻ `Same`. The variant order here is
/// *not* the resolution order; consult [`FactionStance::negativity`]
/// for the rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactionStance {
    /// Same species, same colony. Default for cat-on-cat pre-overlay;
    /// intra-species peers for wildlife.
    Same,
    /// Different species, aligned (e.g. a befriended fox via the
    /// `BefriendedAlly` overlay).
    Ally,
    Neutral,
    /// Hunting target.
    Prey,
    /// Flee target.
    Predator,
    /// Combat target (banished cats, hostile visitors, shadowfoxes).
    Enemy,
}

impl FactionStance {
    /// Ordinal used by the §9.2 most-negative-wins resolver. Lower is
    /// friendlier; higher is more hostile. Matches the resolution
    /// chain committed in §9.2's prose: `Banished` (Enemy) ≻
    /// `HostileVisitor` (Enemy) ≻ `Visitor` (Neutral) ≻ base ≻
    /// `BefriendedAlly` (Ally).
    pub fn negativity(self) -> u8 {
        match self {
            Self::Same => 0,
            Self::Ally => 1,
            Self::Neutral => 2,
            Self::Prey => 3,
            Self::Predator => 4,
            Self::Enemy => 5,
        }
    }
}

// ---------------------------------------------------------------------------
// StanceRequirement
// ---------------------------------------------------------------------------

/// "Target must be one of" stance set — the §9.3 DSE filter binding
/// shape. Matches the spec's pipe-separated notation: `Same | Ally`
/// becomes `StanceRequirement::any_of(&[Same, Ally])`.
#[derive(Debug, Clone)]
pub struct StanceRequirement {
    pub any_of: Vec<FactionStance>,
}

impl StanceRequirement {
    pub fn any_of(stances: &[FactionStance]) -> Self {
        Self {
            any_of: stances.to_vec(),
        }
    }

    pub fn accepts(&self, stance: FactionStance) -> bool {
        self.any_of.contains(&stance)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negativity_rank_most_negative_wins() {
        assert!(FactionStance::Enemy.negativity() > FactionStance::Predator.negativity());
        assert!(FactionStance::Predator.negativity() > FactionStance::Neutral.negativity());
        assert!(FactionStance::Neutral.negativity() > FactionStance::Ally.negativity());
        assert!(FactionStance::Ally.negativity() > FactionStance::Same.negativity());
    }

    #[test]
    fn stance_requirement_accepts_any_of() {
        let req = StanceRequirement::any_of(&[FactionStance::Same, FactionStance::Ally]);
        assert!(req.accepts(FactionStance::Same));
        assert!(req.accepts(FactionStance::Ally));
        assert!(!req.accepts(FactionStance::Enemy));
    }
}
