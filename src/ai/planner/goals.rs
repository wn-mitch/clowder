use crate::components::disposition::DispositionKind;

use super::{GoalState, StatePredicate};

/// Build the goal state for a single trip of the given disposition.
///
/// Multi-trip dispositions (Hunting, Foraging, etc.) target `trips_done >= current_trips + 1`.
/// The executor handles the trip loop — it checks `trips_done < target_trips` and re-invokes
/// the planner for each subsequent trip.
pub fn goal_for_disposition(kind: DispositionKind, current_trips: u32) -> GoalState {
    match kind {
        // Resting completes on need thresholds, not trip count.
        DispositionKind::Resting => GoalState {
            predicates: vec![
                StatePredicate::HungerOk(true),
                StatePredicate::EnergyOk(true),
                StatePredicate::WarmthOk(true),
            ],
        },

        // Building completes when construction is done.
        DispositionKind::Building => GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        },

        // Mating and Coordinating complete on interaction.
        DispositionKind::Mating | DispositionKind::Coordinating => GoalState {
            predicates: vec![StatePredicate::InteractionDone(true)],
        },

        // All other dispositions: one trip increment.
        DispositionKind::Hunting
        | DispositionKind::Foraging
        | DispositionKind::Guarding
        | DispositionKind::Socializing
        | DispositionKind::Farming
        | DispositionKind::Crafting
        | DispositionKind::Exploring
        | DispositionKind::Caretaking => GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(current_trips + 1)],
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::planner::{Carrying, PlannerState, PlannerZone};

    fn default_state() -> PlannerState {
        PlannerState {
            zone: PlannerZone::Wilds,
            carrying: Carrying::Nothing,
            trips_done: 0,
            hunger_ok: true,
            energy_ok: true,
            warmth_ok: true,
            interaction_done: false,
            construction_done: false,
            prey_found: false,
            farm_tended: false,
            thornbriar_available: false,
        }
    }

    #[test]
    fn resting_goal_checks_needs() {
        let goal = goal_for_disposition(DispositionKind::Resting, 0);
        let satisfied = PlannerState {
            hunger_ok: true,
            energy_ok: true,
            warmth_ok: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&satisfied));

        let hungry = PlannerState {
            hunger_ok: false,
            ..satisfied.clone()
        };
        assert!(!goal.is_satisfied(&hungry));
        assert_eq!(goal.heuristic(&hungry), 1);
    }

    #[test]
    fn hunting_goal_checks_trips() {
        let goal = goal_for_disposition(DispositionKind::Hunting, 0);
        let start = default_state();
        assert!(!goal.is_satisfied(&start));

        let after_trip = PlannerState {
            trips_done: 1,
            ..start
        };
        assert!(goal.is_satisfied(&after_trip));
    }

    #[test]
    fn multi_trip_goal_increments() {
        let goal = goal_for_disposition(DispositionKind::Foraging, 2);
        let state = PlannerState {
            trips_done: 2,
            ..default_state()
        };
        assert!(!goal.is_satisfied(&state));

        let state = PlannerState {
            trips_done: 3,
            ..default_state()
        };
        assert!(goal.is_satisfied(&state));
    }

    #[test]
    fn building_goal_checks_construction() {
        let goal = goal_for_disposition(DispositionKind::Building, 0);
        assert!(!goal.is_satisfied(&default_state()));

        let done = PlannerState {
            construction_done: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&done));
    }

    #[test]
    fn mating_goal_checks_interaction() {
        let goal = goal_for_disposition(DispositionKind::Mating, 0);
        assert!(!goal.is_satisfied(&default_state()));

        let done = PlannerState {
            interaction_done: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&done));
    }

    #[test]
    fn heuristic_counts_unsatisfied() {
        let goal = goal_for_disposition(DispositionKind::Resting, 0);
        let all_bad = PlannerState {
            hunger_ok: false,
            energy_ok: false,
            warmth_ok: false,
            ..default_state()
        };
        assert_eq!(goal.heuristic(&all_bad), 3);

        let one_bad = PlannerState {
            hunger_ok: false,
            energy_ok: true,
            warmth_ok: true,
            ..default_state()
        };
        assert_eq!(goal.heuristic(&one_bad), 1);
    }
}
