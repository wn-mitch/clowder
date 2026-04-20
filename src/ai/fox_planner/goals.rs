//! Fox goal definitions — maps each disposition to the predicates that satisfy it.

use crate::ai::planner::core::Goal;

use super::{FoxDispositionKind, FoxDomain, FoxStatePredicate, FoxZone};

/// Build the goal state for a fox disposition.
pub fn goal_for_disposition(kind: FoxDispositionKind) -> Goal<FoxDomain> {
    use FoxStatePredicate as P;

    match kind {
        FoxDispositionKind::Hunting => Goal {
            predicates: vec![P::HungerOk(true)],
        },
        FoxDispositionKind::Feeding => Goal {
            predicates: vec![P::CubsFed(true)],
        },
        FoxDispositionKind::Patrolling => Goal {
            predicates: vec![P::TerritoryMarked(true)],
        },
        FoxDispositionKind::Raiding => Goal {
            predicates: vec![P::HungerOk(true)],
        },
        FoxDispositionKind::DenDefense => Goal {
            predicates: vec![P::DenSecured(true)],
        },
        FoxDispositionKind::Resting => Goal {
            predicates: vec![P::HungerOk(true)],
        },
        FoxDispositionKind::Dispersing => Goal {
            predicates: vec![P::DenSecured(true)],
        },
        FoxDispositionKind::Fleeing => Goal {
            predicates: vec![P::ZoneIs(FoxZone::MapEdge)],
        },
        FoxDispositionKind::Avoiding => Goal {
            predicates: vec![P::InteractionDone(true)],
        },
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

    #[test]
    fn hunting_goal_checks_hunger() {
        let goal = goal_for_disposition(FoxDispositionKind::Hunting);

        let hungry = FoxPlannerState {
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
        assert!(!goal.is_satisfied(&hungry));

        let fed = FoxPlannerState {
            hunger_ok: true,
            ..hungry
        };
        assert!(goal.is_satisfied(&fed));
    }

    #[test]
    fn feeding_goal_checks_cubs_fed() {
        let goal = goal_for_disposition(FoxDispositionKind::Feeding);
        let state = FoxPlannerState {
            zone: FoxZone::Den,
            carrying_food: false,
            prey_found: false,
            hunger_ok: true,
            cubs_fed: false,
            territory_marked: false,
            den_secured: false,
            interaction_done: false,
            trips_done: 0,
        };
        assert!(!goal.is_satisfied(&state));

        let fed = FoxPlannerState {
            cubs_fed: true,
            ..state
        };
        assert!(goal.is_satisfied(&fed));
    }

    #[test]
    fn fleeing_goal_checks_zone() {
        let goal = goal_for_disposition(FoxDispositionKind::Fleeing);
        let at_edge = FoxPlannerState {
            zone: FoxZone::MapEdge,
            carrying_food: false,
            prey_found: false,
            hunger_ok: false,
            cubs_fed: false,
            territory_marked: false,
            den_secured: false,
            interaction_done: false,
            trips_done: 0,
        };
        assert!(goal.is_satisfied(&at_edge));

        let in_wilds = FoxPlannerState {
            zone: FoxZone::Wilds,
            ..at_edge
        };
        assert!(!goal.is_satisfied(&in_wilds));
    }

    #[test]
    fn heuristic_counts_unsatisfied() {
        let goal = goal_for_disposition(FoxDispositionKind::Hunting);
        let hungry = FoxPlannerState {
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
        assert_eq!(goal.heuristic(&hungry), 1);

        let fed = FoxPlannerState {
            hunger_ok: true,
            ..hungry
        };
        assert_eq!(goal.heuristic(&fed), 0);
    }

    #[test]
    fn full_feeding_plan_from_goal() {
        use crate::ai::fox_planner::actions::feeding_actions;

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

        let goal = goal_for_disposition(FoxDispositionKind::Feeding);
        let actions = feeding_actions();
        let plan = core::make_plan::<super::super::FoxDomain>(start, &actions, &goal, 12, 1000)
            .expect("should find feeding plan");

        assert!(!plan.is_empty());
        // Last action should be FeedCubs.
        assert_eq!(
            plan.last().unwrap().action,
            super::super::FoxGoapActionKind::FeedCubs
        );
    }
}
