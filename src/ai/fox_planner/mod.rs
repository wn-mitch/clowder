//! Fox GOAP planner — domain types and A* integration.
//!
//! Implements [`GoapDomain`] for foxes via [`FoxDomain`], providing the
//! species-specific state, action, predicate, and effect types that the
//! generic A* planner operates on.

pub mod actions;
pub mod goals;

use crate::ai::planner::core::GoapDomain;

// ---------------------------------------------------------------------------
// FoxZone — abstract spatial zones from the fox's perspective
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FoxZone {
    /// At or adjacent to the fox's home den.
    Den,
    /// On the perimeter of claimed territory.
    TerritoryEdge,
    /// In a prey-rich area away from the colony.
    HuntingGround,
    /// Near the cat colony (stores, buildings).
    NearColony,
    /// At the map edge (flee destination).
    MapEdge,
    /// Adjacent to detected prey.
    PreyLocation,
    /// Generic wilderness (default).
    Wilds,
}

// ---------------------------------------------------------------------------
// FoxGoapActionKind — identity of each fox planner action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FoxGoapActionKind {
    TravelTo(FoxZone),
    // Hunting
    SearchPrey,
    StalkPrey,
    KillPrey,
    // Den / cubs
    ReturnToDen,
    FeedCubs,
    // Territory
    PatrolBoundary,
    DepositScent,
    // Colony interaction
    ApproachStore,
    StealFood,
    // Combat / confrontation
    ConfrontTarget,
    // Survival
    FleeArea,
    Rest,
    GroomSelf,
    // Juvenile lifecycle
    ScoutTerritory,
    EstablishDen,
}

// ---------------------------------------------------------------------------
// FoxPlannerState — compact, hashable fox world state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FoxPlannerState {
    pub zone: FoxZone,
    pub carrying_food: bool,
    pub prey_found: bool,
    pub hunger_ok: bool,
    pub cubs_fed: bool,
    pub territory_marked: bool,
    pub den_secured: bool,
    pub interaction_done: bool,
    pub trips_done: u32,
}

// ---------------------------------------------------------------------------
// FoxStatePredicate — conditions over FoxPlannerState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoxStatePredicate {
    ZoneIs(FoxZone),
    ZoneIsNot(FoxZone),
    CarryingFood(bool),
    PreyFound(bool),
    HungerOk(bool),
    CubsFed(bool),
    TerritoryMarked(bool),
    DenSecured(bool),
    InteractionDone(bool),
    TripsAtLeast(u32),
}

impl FoxStatePredicate {
    pub fn evaluate(&self, state: &FoxPlannerState) -> bool {
        match self {
            Self::ZoneIs(z) => state.zone == *z,
            Self::ZoneIsNot(z) => state.zone != *z,
            Self::CarryingFood(v) => state.carrying_food == *v,
            Self::PreyFound(v) => state.prey_found == *v,
            Self::HungerOk(v) => state.hunger_ok == *v,
            Self::CubsFed(v) => state.cubs_fed == *v,
            Self::TerritoryMarked(v) => state.territory_marked == *v,
            Self::DenSecured(v) => state.den_secured == *v,
            Self::InteractionDone(v) => state.interaction_done == *v,
            Self::TripsAtLeast(n) => state.trips_done >= *n,
        }
    }
}

// ---------------------------------------------------------------------------
// FoxStateEffect — mutations applied when an action executes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoxStateEffect {
    SetZone(FoxZone),
    SetCarryingFood(bool),
    SetPreyFound(bool),
    SetHungerOk(bool),
    SetCubsFed(bool),
    SetTerritoryMarked(bool),
    SetDenSecured(bool),
    SetInteractionDone(bool),
    IncrementTrips,
}

impl FoxStateEffect {
    pub fn apply(&self, state: &mut FoxPlannerState) {
        match self {
            Self::SetZone(z) => state.zone = *z,
            Self::SetCarryingFood(v) => state.carrying_food = *v,
            Self::SetPreyFound(v) => state.prey_found = *v,
            Self::SetHungerOk(v) => state.hunger_ok = *v,
            Self::SetCubsFed(v) => state.cubs_fed = *v,
            Self::SetTerritoryMarked(v) => state.territory_marked = *v,
            Self::SetDenSecured(v) => state.den_secured = *v,
            Self::SetInteractionDone(v) => state.interaction_done = *v,
            Self::IncrementTrips => state.trips_done += 1,
        }
    }
}

// ---------------------------------------------------------------------------
// FoxDomain — GoapDomain implementation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct FoxDomain;

impl GoapDomain for FoxDomain {
    type State = FoxPlannerState;
    type ActionKind = FoxGoapActionKind;
    type Predicate = FoxStatePredicate;
    type Effect = FoxStateEffect;

    fn evaluate(pred: &FoxStatePredicate, state: &FoxPlannerState) -> bool {
        pred.evaluate(state)
    }

    fn apply(effect: &FoxStateEffect, state: &mut FoxPlannerState) {
        effect.apply(state);
    }
}

// ---------------------------------------------------------------------------
// FoxDispositionKind — high-level behavioral modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FoxDispositionKind {
    /// Find and kill prey to eat.
    Hunting,
    /// Hunt prey and bring it back to feed cubs.
    Feeding,
    /// Patrol territory perimeter and deposit scent.
    Patrolling,
    /// Steal food from colony stores.
    Raiding,
    /// Confront intruders near den / protect cubs.
    DenDefense,
    /// Rest at den.
    Resting,
    /// Juvenile searching for territory to claim.
    Dispersing,
    /// Flee from danger toward map edge.
    Fleeing,
    /// Quietly move away from nearby cats (non-hostile withdrawal).
    Avoiding,
}

impl FoxDispositionKind {
    /// Maslow level this disposition primarily serves.
    pub fn maslow_level(self) -> u8 {
        match self {
            Self::Hunting | Self::Raiding | Self::Resting | Self::Fleeing | Self::Avoiding => 1, // Survival
            Self::Patrolling | Self::Dispersing => 2, // Territory
            Self::Feeding | Self::DenDefense => 3,    // Offspring
        }
    }

    /// Target trip completions for this disposition.
    pub fn target_completions(self) -> u32 {
        match self {
            Self::Hunting | Self::Raiding => 1,
            Self::Feeding => 1,
            Self::Patrolling => 1,
            Self::DenDefense => 1,
            Self::Resting => 1,
            Self::Dispersing => 1,
            Self::Fleeing => 1,
            Self::Avoiding => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::planner::core;

    #[test]
    fn fox_predicate_evaluate() {
        let state = FoxPlannerState {
            zone: FoxZone::Den,
            carrying_food: true,
            prey_found: false,
            hunger_ok: true,
            cubs_fed: false,
            territory_marked: false,
            den_secured: true,
            interaction_done: false,
            trips_done: 0,
        };

        assert!(FoxStatePredicate::ZoneIs(FoxZone::Den).evaluate(&state));
        assert!(FoxStatePredicate::ZoneIsNot(FoxZone::Wilds).evaluate(&state));
        assert!(FoxStatePredicate::CarryingFood(true).evaluate(&state));
        assert!(FoxStatePredicate::HungerOk(true).evaluate(&state));
        assert!(FoxStatePredicate::CubsFed(false).evaluate(&state));
    }

    #[test]
    fn fox_effect_apply() {
        let mut state = FoxPlannerState {
            zone: FoxZone::Wilds,
            carrying_food: false,
            prey_found: false,
            hunger_ok: false,
            cubs_fed: false,
            territory_marked: false,
            den_secured: false,
            interaction_done: false,
            trips_done: 0,
        };

        FoxStateEffect::SetZone(FoxZone::Den).apply(&mut state);
        assert_eq!(state.zone, FoxZone::Den);

        FoxStateEffect::SetCarryingFood(true).apply(&mut state);
        assert!(state.carrying_food);

        FoxStateEffect::IncrementTrips.apply(&mut state);
        assert_eq!(state.trips_done, 1);
    }

    #[test]
    fn fox_domain_works_with_generic_planner() {
        let start = FoxPlannerState {
            zone: FoxZone::Wilds,
            carrying_food: false,
            prey_found: false,
            hunger_ok: false,
            cubs_fed: false,
            territory_marked: false,
            den_secured: false,
            interaction_done: false,
            trips_done: 0,
        };

        let actions = vec![
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::TravelTo(FoxZone::HuntingGround),
                cost: 2,
                preconditions: vec![FoxStatePredicate::ZoneIsNot(FoxZone::HuntingGround)],
                effects: vec![FoxStateEffect::SetZone(FoxZone::HuntingGround)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::SearchPrey,
                cost: 3,
                preconditions: vec![
                    FoxStatePredicate::ZoneIs(FoxZone::HuntingGround),
                    FoxStatePredicate::CarryingFood(false),
                ],
                effects: vec![FoxStateEffect::SetPreyFound(true)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::StalkPrey,
                cost: 2,
                preconditions: vec![FoxStatePredicate::PreyFound(true)],
                effects: vec![FoxStateEffect::SetZone(FoxZone::PreyLocation)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::KillPrey,
                cost: 2,
                preconditions: vec![FoxStatePredicate::ZoneIs(FoxZone::PreyLocation)],
                effects: vec![
                    FoxStateEffect::SetCarryingFood(true),
                    FoxStateEffect::SetPreyFound(false),
                    FoxStateEffect::SetHungerOk(true),
                ],
            },
        ];

        let goal = core::Goal::<FoxDomain> {
            predicates: vec![FoxStatePredicate::HungerOk(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find fox hunting plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                FoxGoapActionKind::TravelTo(FoxZone::HuntingGround),
                FoxGoapActionKind::SearchPrey,
                FoxGoapActionKind::StalkPrey,
                FoxGoapActionKind::KillPrey,
            ]
        );
    }

    #[test]
    fn fox_feeding_plan_returns_to_den() {
        let start = FoxPlannerState {
            zone: FoxZone::Wilds,
            carrying_food: false,
            prey_found: false,
            hunger_ok: true,
            cubs_fed: false,
            territory_marked: false,
            den_secured: false,
            interaction_done: false,
            trips_done: 0,
        };

        let actions = vec![
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::TravelTo(FoxZone::HuntingGround),
                cost: 2,
                preconditions: vec![FoxStatePredicate::ZoneIsNot(FoxZone::HuntingGround)],
                effects: vec![FoxStateEffect::SetZone(FoxZone::HuntingGround)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::SearchPrey,
                cost: 3,
                preconditions: vec![
                    FoxStatePredicate::ZoneIs(FoxZone::HuntingGround),
                    FoxStatePredicate::CarryingFood(false),
                ],
                effects: vec![FoxStateEffect::SetPreyFound(true)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::StalkPrey,
                cost: 2,
                preconditions: vec![FoxStatePredicate::PreyFound(true)],
                effects: vec![FoxStateEffect::SetZone(FoxZone::PreyLocation)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::KillPrey,
                cost: 2,
                preconditions: vec![FoxStatePredicate::ZoneIs(FoxZone::PreyLocation)],
                effects: vec![
                    FoxStateEffect::SetCarryingFood(true),
                    FoxStateEffect::SetPreyFound(false),
                ],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::ReturnToDen,
                cost: 3,
                preconditions: vec![FoxStatePredicate::ZoneIsNot(FoxZone::Den)],
                effects: vec![FoxStateEffect::SetZone(FoxZone::Den)],
            },
            core::ActionDef::<FoxDomain> {
                kind: FoxGoapActionKind::FeedCubs,
                cost: 1,
                preconditions: vec![
                    FoxStatePredicate::ZoneIs(FoxZone::Den),
                    FoxStatePredicate::CarryingFood(true),
                ],
                effects: vec![
                    FoxStateEffect::SetCarryingFood(false),
                    FoxStateEffect::SetCubsFed(true),
                ],
            },
        ];

        let goal = core::Goal::<FoxDomain> {
            predicates: vec![FoxStatePredicate::CubsFed(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find feeding plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                FoxGoapActionKind::TravelTo(FoxZone::HuntingGround),
                FoxGoapActionKind::SearchPrey,
                FoxGoapActionKind::StalkPrey,
                FoxGoapActionKind::KillPrey,
                FoxGoapActionKind::ReturnToDen,
                FoxGoapActionKind::FeedCubs,
            ]
        );
    }
}
