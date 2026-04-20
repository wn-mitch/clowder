//! Fox action definitions — data-driven precondition/effect tables per disposition.

use crate::ai::planner::core::ActionDef;

use super::{
    FoxDomain, FoxGoapActionKind, FoxStateEffect as Eff, FoxStatePredicate as Pred, FoxZone,
};

// ---------------------------------------------------------------------------
// Travel actions (generated for each zone pair)
// ---------------------------------------------------------------------------

fn travel(to: FoxZone, cost: u32) -> ActionDef<FoxDomain> {
    ActionDef {
        kind: FoxGoapActionKind::TravelTo(to),
        cost,
        preconditions: vec![Pred::ZoneIsNot(to)],
        effects: vec![Eff::SetZone(to)],
    }
}

// ---------------------------------------------------------------------------
// Hunting: find prey, stalk, kill, eat
// ---------------------------------------------------------------------------

pub fn hunting_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::HuntingGround, 3),
        ActionDef {
            kind: FoxGoapActionKind::SearchPrey,
            cost: 3,
            preconditions: vec![
                Pred::ZoneIs(FoxZone::HuntingGround),
                Pred::CarryingFood(false),
            ],
            effects: vec![Eff::SetPreyFound(true)],
        },
        ActionDef {
            kind: FoxGoapActionKind::StalkPrey,
            cost: 2,
            preconditions: vec![Pred::PreyFound(true)],
            effects: vec![Eff::SetZone(FoxZone::PreyLocation)],
        },
        ActionDef {
            kind: FoxGoapActionKind::KillPrey,
            cost: 2,
            preconditions: vec![Pred::ZoneIs(FoxZone::PreyLocation)],
            effects: vec![
                Eff::SetCarryingFood(true),
                Eff::SetPreyFound(false),
                Eff::SetHungerOk(true),
                Eff::IncrementTrips,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Feeding: hunt prey and bring it back to feed cubs
// ---------------------------------------------------------------------------

pub fn feeding_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::HuntingGround, 3),
        // No generic TravelTo(Den) here — ReturnToDen below requires carrying
        // food, which ensures the fox hunts BEFORE returning. The planner would
        // otherwise shortcut via TravelTo(Den) without food.
        ActionDef {
            kind: FoxGoapActionKind::SearchPrey,
            cost: 3,
            preconditions: vec![
                Pred::ZoneIs(FoxZone::HuntingGround),
                Pred::CarryingFood(false),
            ],
            effects: vec![Eff::SetPreyFound(true)],
        },
        ActionDef {
            kind: FoxGoapActionKind::StalkPrey,
            cost: 2,
            preconditions: vec![Pred::PreyFound(true)],
            effects: vec![Eff::SetZone(FoxZone::PreyLocation)],
        },
        ActionDef {
            kind: FoxGoapActionKind::KillPrey,
            cost: 2,
            preconditions: vec![Pred::ZoneIs(FoxZone::PreyLocation)],
            effects: vec![Eff::SetCarryingFood(true), Eff::SetPreyFound(false)],
        },
        ActionDef {
            kind: FoxGoapActionKind::ReturnToDen,
            cost: 3,
            preconditions: vec![Pred::CarryingFood(true), Pred::ZoneIsNot(FoxZone::Den)],
            effects: vec![Eff::SetZone(FoxZone::Den)],
        },
        ActionDef {
            kind: FoxGoapActionKind::FeedCubs,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(FoxZone::Den), Pred::CarryingFood(true)],
            effects: vec![
                Eff::SetCarryingFood(false),
                Eff::SetCubsFed(true),
                Eff::IncrementTrips,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Patrolling: walk territory perimeter and mark with scent
// ---------------------------------------------------------------------------

pub fn patrolling_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::TerritoryEdge, 2),
        ActionDef {
            kind: FoxGoapActionKind::PatrolBoundary,
            cost: 3,
            preconditions: vec![Pred::ZoneIs(FoxZone::TerritoryEdge)],
            effects: vec![], // patrol itself doesn't change planner state; scent is the goal
        },
        ActionDef {
            kind: FoxGoapActionKind::DepositScent,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(FoxZone::TerritoryEdge)],
            effects: vec![Eff::SetTerritoryMarked(true), Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Raiding: steal food from colony stores
// ---------------------------------------------------------------------------

pub fn raiding_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::NearColony, 3),
        ActionDef {
            kind: FoxGoapActionKind::ApproachStore,
            cost: 2,
            preconditions: vec![Pred::ZoneIs(FoxZone::NearColony)],
            effects: vec![], // approach is movement; steal is the payoff
        },
        ActionDef {
            kind: FoxGoapActionKind::StealFood,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(FoxZone::NearColony)],
            effects: vec![Eff::SetHungerOk(true), Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Den defense: confront intruders
// ---------------------------------------------------------------------------

pub fn den_defense_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![ActionDef {
        kind: FoxGoapActionKind::ConfrontTarget,
        cost: 2,
        preconditions: vec![],
        effects: vec![Eff::SetDenSecured(true), Eff::IncrementTrips],
    }]
}

// ---------------------------------------------------------------------------
// Resting: rest at den, groom
// ---------------------------------------------------------------------------

pub fn resting_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::Den, 2),
        ActionDef {
            kind: FoxGoapActionKind::Rest,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(FoxZone::Den)],
            effects: vec![Eff::SetHungerOk(true), Eff::IncrementTrips],
        },
        ActionDef {
            kind: FoxGoapActionKind::GroomSelf,
            cost: 1,
            preconditions: vec![],
            effects: vec![], // grooming is a side effect of resting
        },
    ]
}

// ---------------------------------------------------------------------------
// Dispersing: juvenile territory search
// ---------------------------------------------------------------------------

pub fn dispersing_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::Wilds, 1),
        ActionDef {
            kind: FoxGoapActionKind::ScoutTerritory,
            cost: 3,
            preconditions: vec![Pred::ZoneIs(FoxZone::Wilds)],
            effects: vec![], // scouting is exploration; establishment is the payoff
        },
        ActionDef {
            kind: FoxGoapActionKind::EstablishDen,
            cost: 2,
            preconditions: vec![Pred::ZoneIs(FoxZone::Wilds)],
            effects: vec![Eff::SetDenSecured(true), Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Avoiding: quietly move away from cats without fleeing
// ---------------------------------------------------------------------------

/// Used when a fox sees cats but isn't in immediate danger — a non-combat
/// withdrawal to the territory edge. Distinct from Fleeing, which targets the
/// map edge.
pub fn avoiding_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::TerritoryEdge, 1),
        ActionDef {
            kind: FoxGoapActionKind::PatrolBoundary,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(FoxZone::TerritoryEdge)],
            effects: vec![Eff::SetInteractionDone(true), Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Fleeing: escape toward map edge
// ---------------------------------------------------------------------------

pub fn fleeing_actions() -> Vec<ActionDef<FoxDomain>> {
    vec![
        travel(FoxZone::MapEdge, 1),
        ActionDef {
            kind: FoxGoapActionKind::FleeArea,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(FoxZone::MapEdge)],
            effects: vec![Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Aggregate: actions for a disposition
// ---------------------------------------------------------------------------

use super::FoxDispositionKind;

pub fn actions_for_disposition(kind: FoxDispositionKind) -> Vec<ActionDef<FoxDomain>> {
    match kind {
        FoxDispositionKind::Hunting => hunting_actions(),
        FoxDispositionKind::Feeding => feeding_actions(),
        FoxDispositionKind::Patrolling => patrolling_actions(),
        FoxDispositionKind::Raiding => raiding_actions(),
        FoxDispositionKind::DenDefense => den_defense_actions(),
        FoxDispositionKind::Resting => resting_actions(),
        FoxDispositionKind::Dispersing => dispersing_actions(),
        FoxDispositionKind::Fleeing => fleeing_actions(),
        FoxDispositionKind::Avoiding => avoiding_actions(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::fox_planner::FoxPlannerState;
    use crate::ai::planner::core;

    fn default_fox_state() -> FoxPlannerState {
        FoxPlannerState {
            zone: FoxZone::Wilds,
            carrying_food: false,
            prey_found: false,
            hunger_ok: false,
            cubs_fed: false,
            territory_marked: false,
            den_secured: false,
            interaction_done: false,
            trips_done: 0,
        }
    }

    #[test]
    fn hunting_plan_satisfies_hunger() {
        let start = default_fox_state();
        let actions = hunting_actions();
        let goal = core::Goal::<FoxDomain> {
            predicates: vec![Pred::HungerOk(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find hunting plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&FoxGoapActionKind::KillPrey));
    }

    #[test]
    fn feeding_plan_includes_return_and_feed() {
        let start = default_fox_state();
        let actions = feeding_actions();
        let goal = core::Goal::<FoxDomain> {
            predicates: vec![Pred::CubsFed(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find feeding plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&FoxGoapActionKind::ReturnToDen));
        assert!(kinds.contains(&FoxGoapActionKind::FeedCubs));
    }

    #[test]
    fn patrolling_plan_marks_territory() {
        let mut start = default_fox_state();
        start.hunger_ok = true;
        let actions = patrolling_actions();
        let goal = core::Goal::<FoxDomain> {
            predicates: vec![Pred::TerritoryMarked(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find patrol plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&FoxGoapActionKind::DepositScent));
    }

    #[test]
    fn raiding_plan_steals_food() {
        let start = default_fox_state();
        let actions = raiding_actions();
        let goal = core::Goal::<FoxDomain> {
            predicates: vec![Pred::HungerOk(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find raiding plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&FoxGoapActionKind::StealFood));
    }

    #[test]
    fn dispersal_plan_establishes_den() {
        let start = default_fox_state();
        let actions = dispersing_actions();
        let goal = core::Goal::<FoxDomain> {
            predicates: vec![Pred::DenSecured(true)],
        };

        let plan = core::make_plan::<FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find dispersal plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&FoxGoapActionKind::EstablishDen));
    }
}
