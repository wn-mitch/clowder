//! Snake GOAP planner — domain types and A* integration.
//!
//! Implements [`GoapDomain`] for snakes via [`SnakeDomain`], providing the
//! species-specific state, action, predicate, and effect types that the
//! generic A* planner operates on.
//!
//! Snakes are ambush predators with a 2-level Maslow hierarchy:
//! Level 1 (survival — hunger, safety) and Level 2 (thermoregulation).
//! Four dispositions: Ambushing, Foraging, Basking, Fleeing.

pub mod actions;
pub mod goals;

use crate::ai::planner::core::GoapDomain;

// ---------------------------------------------------------------------------
// SnakeZone — abstract spatial zones from the snake's perspective
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SnakeZone {
    /// In cover (tall grass, undergrowth, rock crevice). Ambush position.
    Cover,
    /// Open ground where prey passes. Active foraging area.
    HuntingGround,
    /// Exposed rock or warm terrain for basking.
    BaskingSpot,
    /// Map edge (flee destination).
    MapEdge,
}

// ---------------------------------------------------------------------------
// SnakeGoapActionKind — identity of each snake planner action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SnakeGoapActionKind {
    /// Slither to an abstract zone.
    SlideTo(SnakeZone),
    /// Coil and wait in ambush position.
    SetAmbush,
    /// Strike at nearby prey.
    Strike,
    /// Bask on warm terrain to thermoregulate.
    Bask,
    /// Flee toward cover or map edge.
    Retreat,
}

// ---------------------------------------------------------------------------
// SnakePlannerState — compact, hashable snake world state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SnakePlannerState {
    pub zone: SnakeZone,
    pub prey_in_range: bool,
    pub hunger_ok: bool,
    pub warm: bool,
    pub trips_done: u32,
}

// ---------------------------------------------------------------------------
// SnakeStatePredicate — conditions over SnakePlannerState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnakeStatePredicate {
    ZoneIs(SnakeZone),
    ZoneIsNot(SnakeZone),
    PreyInRange(bool),
    HungerOk(bool),
    Warm(bool),
    TripsAtLeast(u32),
}

impl SnakeStatePredicate {
    pub fn evaluate(&self, state: &SnakePlannerState) -> bool {
        match self {
            Self::ZoneIs(z) => state.zone == *z,
            Self::ZoneIsNot(z) => state.zone != *z,
            Self::PreyInRange(v) => state.prey_in_range == *v,
            Self::HungerOk(v) => state.hunger_ok == *v,
            Self::Warm(v) => state.warm == *v,
            Self::TripsAtLeast(n) => state.trips_done >= *n,
        }
    }
}

// ---------------------------------------------------------------------------
// SnakeStateEffect — mutations applied when an action executes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnakeStateEffect {
    SetZone(SnakeZone),
    SetPreyInRange(bool),
    SetHungerOk(bool),
    SetWarm(bool),
    IncrementTrips,
}

impl SnakeStateEffect {
    pub fn apply(&self, state: &mut SnakePlannerState) {
        match self {
            Self::SetZone(z) => state.zone = *z,
            Self::SetPreyInRange(v) => state.prey_in_range = *v,
            Self::SetHungerOk(v) => state.hunger_ok = *v,
            Self::SetWarm(v) => state.warm = *v,
            Self::IncrementTrips => state.trips_done += 1,
        }
    }
}

// ---------------------------------------------------------------------------
// SnakeDomain — GoapDomain implementation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct SnakeDomain;

impl GoapDomain for SnakeDomain {
    type State = SnakePlannerState;
    type ActionKind = SnakeGoapActionKind;
    type Predicate = SnakeStatePredicate;
    type Effect = SnakeStateEffect;

    fn evaluate(pred: &SnakeStatePredicate, state: &SnakePlannerState) -> bool {
        pred.evaluate(state)
    }

    fn apply(effect: &SnakeStateEffect, state: &mut SnakePlannerState) {
        effect.apply(state);
    }
}

// ---------------------------------------------------------------------------
// SnakeDispositionKind — high-level behavioral modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SnakeDispositionKind {
    /// Find cover, coil, wait for prey to come within strike range.
    Ambushing,
    /// Actively slither toward prey-rich areas when very hungry.
    Foraging,
    /// Thermoregulate on warm terrain (rock, sun-exposed ground).
    Basking,
    /// Flee from danger toward cover or map edge.
    Fleeing,
}

impl SnakeDispositionKind {
    /// Maslow level this disposition serves.
    /// Level 1: survival (hunger, safety). Level 2: thermoregulation.
    pub fn maslow_level(self) -> u8 {
        match self {
            Self::Ambushing | Self::Foraging | Self::Fleeing => 1,
            Self::Basking => 2,
        }
    }

    /// Target trip completions for this disposition.
    pub fn target_completions(self) -> u32 {
        1
    }
}

impl std::fmt::Display for SnakeDispositionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
