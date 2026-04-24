//! Hawk goal definitions — maps each disposition to the predicates that satisfy it.

use crate::ai::planner::core::Goal;

use super::{HawkDispositionKind, HawkDomain, HawkStatePredicate, HawkZone};

/// Build the goal state for a hawk disposition.
pub fn goal_for_disposition(kind: HawkDispositionKind) -> Goal<HawkDomain> {
    use HawkStatePredicate as P;

    match kind {
        HawkDispositionKind::Hunting => Goal {
            predicates: vec![P::HungerOk(true)],
        },
        HawkDispositionKind::Soaring => Goal {
            predicates: vec![P::ZoneIs(HawkZone::Sky)],
        },
        HawkDispositionKind::Fleeing => Goal {
            predicates: vec![P::ZoneIs(HawkZone::MapEdge)],
        },
        HawkDispositionKind::Resting => Goal {
            predicates: vec![P::HungerOk(true)],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::hawk_planner::HawkPlannerState;

    #[test]
    fn hunting_goal_checks_hunger() {
        let goal = goal_for_disposition(HawkDispositionKind::Hunting);
        let hungry = HawkPlannerState {
            zone: HawkZone::Sky,
            prey_spotted: false,
            hunger_ok: false,
            trips_done: 0,
        };
        assert!(!goal.is_satisfied(&hungry));

        let fed = HawkPlannerState {
            hunger_ok: true,
            ..hungry
        };
        assert!(goal.is_satisfied(&fed));
    }

    #[test]
    fn fleeing_goal_checks_zone() {
        let goal = goal_for_disposition(HawkDispositionKind::Fleeing);
        let at_edge = HawkPlannerState {
            zone: HawkZone::MapEdge,
            prey_spotted: false,
            hunger_ok: false,
            trips_done: 0,
        };
        assert!(goal.is_satisfied(&at_edge));

        let in_sky = HawkPlannerState {
            zone: HawkZone::Sky,
            ..at_edge
        };
        assert!(!goal.is_satisfied(&in_sky));
    }
}
