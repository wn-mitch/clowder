//! Hawk GOAP planner — domain types and A* integration.
//!
//! Implements [`GoapDomain`] for hawks via [`HawkDomain`], providing the
//! species-specific state, action, predicate, and effect types that the
//! generic A* planner operates on.
//!
//! Hawks are aerial predators with a 2-level Maslow hierarchy (survival
//! only — no territory/offspring tier). Three dispositions: Hunting,
//! Soaring (patrol), Fleeing.

pub mod actions;
pub mod goals;

use crate::ai::planner::core::GoapDomain;

// ---------------------------------------------------------------------------
// HawkZone — abstract spatial zones from the hawk's perspective
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HawkZone {
    /// Circling over open ground (default airborne state).
    Sky,
    /// Low altitude over prey-rich area, scanning for targets.
    HuntingGround,
    /// Perched on high ground (tree, rock outcrop) for rest.
    Perch,
    /// Map edge (flee destination).
    MapEdge,
}

// ---------------------------------------------------------------------------
// HawkGoapActionKind — identity of each hawk planner action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HawkGoapActionKind {
    /// Fly to an abstract zone.
    SoarTo(HawkZone),
    /// Scan for prey from altitude.
    SpotPrey,
    /// Dive on spotted prey — the kill attempt.
    DiveAttack,
    /// Perch and rest on high ground.
    Rest,
    /// Flee toward map edge.
    FleeSky,
}

// ---------------------------------------------------------------------------
// HawkPlannerState — compact, hashable hawk world state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct HawkPlannerState {
    pub zone: HawkZone,
    pub prey_spotted: bool,
    pub hunger_ok: bool,
    pub trips_done: u32,
}

// ---------------------------------------------------------------------------
// HawkStatePredicate — conditions over HawkPlannerState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HawkStatePredicate {
    ZoneIs(HawkZone),
    ZoneIsNot(HawkZone),
    PreySpotted(bool),
    HungerOk(bool),
    TripsAtLeast(u32),
}

impl HawkStatePredicate {
    pub fn evaluate(&self, state: &HawkPlannerState) -> bool {
        match self {
            Self::ZoneIs(z) => state.zone == *z,
            Self::ZoneIsNot(z) => state.zone != *z,
            Self::PreySpotted(v) => state.prey_spotted == *v,
            Self::HungerOk(v) => state.hunger_ok == *v,
            Self::TripsAtLeast(n) => state.trips_done >= *n,
        }
    }
}

// ---------------------------------------------------------------------------
// HawkStateEffect — mutations applied when an action executes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HawkStateEffect {
    SetZone(HawkZone),
    SetPreySpotted(bool),
    SetHungerOk(bool),
    IncrementTrips,
}

impl HawkStateEffect {
    pub fn apply(&self, state: &mut HawkPlannerState) {
        match self {
            Self::SetZone(z) => state.zone = *z,
            Self::SetPreySpotted(v) => state.prey_spotted = *v,
            Self::SetHungerOk(v) => state.hunger_ok = *v,
            Self::IncrementTrips => state.trips_done += 1,
        }
    }
}

// ---------------------------------------------------------------------------
// HawkDomain — GoapDomain implementation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct HawkDomain;

impl GoapDomain for HawkDomain {
    type State = HawkPlannerState;
    type ActionKind = HawkGoapActionKind;
    type Predicate = HawkStatePredicate;
    type Effect = HawkStateEffect;

    fn evaluate(pred: &HawkStatePredicate, state: &HawkPlannerState) -> bool {
        pred.evaluate(state)
    }

    fn apply(effect: &HawkStateEffect, state: &mut HawkPlannerState) {
        effect.apply(state);
    }
}

// ---------------------------------------------------------------------------
// HawkDispositionKind — high-level behavioral modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HawkDispositionKind {
    /// Scan from altitude, spot prey, dive to kill.
    Hunting,
    /// Soar over territory — default patrol/survey mode.
    Soaring,
    /// Flee from danger toward map edge.
    Fleeing,
    /// Perch and rest to recover energy.
    Resting,
}

impl HawkDispositionKind {
    /// Maslow level this disposition serves.
    /// Hawks have a flat survival-only hierarchy (all level 1).
    pub fn maslow_level(self) -> u8 {
        1 // All hawk dispositions are survival-tier
    }

    /// Target trip completions for this disposition.
    pub fn target_completions(self) -> u32 {
        1
    }
}

impl std::fmt::Display for HawkDispositionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
