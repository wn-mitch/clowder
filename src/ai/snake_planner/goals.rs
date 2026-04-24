//! Snake goal definitions — maps each disposition to the predicates that satisfy it.

use crate::ai::planner::core::Goal;

use super::{SnakeDispositionKind, SnakeDomain, SnakeStatePredicate, SnakeZone};

/// Build the goal state for a snake disposition.
pub fn goal_for_disposition(kind: SnakeDispositionKind) -> Goal<SnakeDomain> {
    use SnakeStatePredicate as P;

    match kind {
        SnakeDispositionKind::Ambushing => Goal {
            predicates: vec![P::HungerOk(true)],
        },
        SnakeDispositionKind::Foraging => Goal {
            predicates: vec![P::HungerOk(true)],
        },
        SnakeDispositionKind::Basking => Goal {
            predicates: vec![P::Warm(true)],
        },
        SnakeDispositionKind::Fleeing => Goal {
            predicates: vec![P::ZoneIs(SnakeZone::MapEdge)],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::snake_planner::SnakePlannerState;

    #[test]
    fn ambush_goal_checks_hunger() {
        let goal = goal_for_disposition(SnakeDispositionKind::Ambushing);
        let hungry = SnakePlannerState {
            zone: SnakeZone::Cover,
            prey_in_range: false,
            hunger_ok: false,
            warm: true,
            trips_done: 0,
        };
        assert!(!goal.is_satisfied(&hungry));

        let fed = SnakePlannerState {
            hunger_ok: true,
            ..hungry
        };
        assert!(goal.is_satisfied(&fed));
    }

    #[test]
    fn basking_goal_checks_warmth() {
        let goal = goal_for_disposition(SnakeDispositionKind::Basking);
        let cold = SnakePlannerState {
            zone: SnakeZone::BaskingSpot,
            prey_in_range: false,
            hunger_ok: true,
            warm: false,
            trips_done: 0,
        };
        assert!(!goal.is_satisfied(&cold));

        let warm = SnakePlannerState {
            warm: true,
            ..cold
        };
        assert!(goal.is_satisfied(&warm));
    }

    #[test]
    fn fleeing_goal_checks_zone() {
        let goal = goal_for_disposition(SnakeDispositionKind::Fleeing);
        let at_edge = SnakePlannerState {
            zone: SnakeZone::MapEdge,
            prey_in_range: false,
            hunger_ok: false,
            warm: true,
            trips_done: 0,
        };
        assert!(goal.is_satisfied(&at_edge));
    }
}
