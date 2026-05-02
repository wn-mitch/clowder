use crate::components::disposition::DispositionKind;
use crate::components::markers;

use super::{GoalState, PlanContext, StatePredicate};

/// Build the goal state for a single trip of the given disposition.
///
/// Multi-trip dispositions (Hunting, Foraging, etc.) target `trips_done >= current_trips + 1`.
/// The executor handles the trip loop — it checks `trips_done < target_trips` and re-invokes
/// the planner for each subsequent trip.
///
/// `ctx` is consulted by the Resting branch only — see tickets 091/092. The
/// builder reads the colony-scoped `HasStoredFood` marker directly, so the
/// goal-side branching shares the source of truth with the action-side
/// `EatAtStores` precondition (`StatePredicate::HasMarker(...)`).
pub fn goal_for_disposition(
    kind: DispositionKind,
    current_trips: u32,
    ctx: &PlanContext<'_>,
) -> GoalState {
    match kind {
        // Resting completes on need thresholds, not trip count.
        //
        // Tickets 091/092: when the `HasStoredFood` marker is absent,
        // drop `HungerOk` from the Resting goal. `EatAtStores` gates on
        // the same marker, so the full three-need Resting goal is
        // unreachable for a hungry cat with empty stores → `make_plan`
        // returns None → cat can never sleep its way through the food-
        // shortage period. The partial Resting goal lets the cat address
        // Energy/Temperature even when Hunger is unrecoverable here,
        // then re-elects on the next decision tick (ideally Foraging or
        // Hunting once producer paths plan).
        DispositionKind::Resting => {
            let mut predicates = vec![
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ];
            if ctx.markers.has(markers::HasStoredFood::KEY, ctx.entity) {
                predicates.insert(0, StatePredicate::HungerOk(true));
            }
            GoalState { predicates }
        }

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
    use crate::ai::scoring::MarkerSnapshot;
    use bevy::prelude::Entity;

    fn default_state() -> PlannerState {
        PlannerState {
            zone: PlannerZone::Wilds,
            carrying: Carrying::Nothing,
            trips_done: 0,
            hunger_ok: true,
            energy_ok: true,
            temperature_ok: true,
            interaction_done: false,
            construction_done: false,
            prey_found: false,
            farm_tended: false,
            materials_delivered_this_plan: false,
        }
    }

    fn empty_markers() -> MarkerSnapshot {
        MarkerSnapshot::new()
    }

    fn food_stocked_markers() -> MarkerSnapshot {
        let mut m = MarkerSnapshot::new();
        m.set_colony(markers::HasStoredFood::KEY, true);
        m
    }

    fn test_entity() -> Entity {
        Entity::from_raw_u32(1).expect("nonzero raw entity id")
    }

    fn ctx<'a>(markers: &'a MarkerSnapshot) -> PlanContext<'a> {
        PlanContext {
            markers,
            entity: test_entity(),
        }
    }

    #[test]
    fn resting_goal_checks_needs() {
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Resting, 0, &cx);
        let satisfied = PlannerState {
            hunger_ok: true,
            energy_ok: true,
            temperature_ok: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&satisfied, &cx));

        let hungry = PlannerState {
            hunger_ok: false,
            ..satisfied.clone()
        };
        assert!(!goal.is_satisfied(&hungry, &cx));
        assert_eq!(goal.heuristic(&hungry, &cx), 1);
    }

    #[test]
    fn hunting_goal_checks_trips() {
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Hunting, 0, &cx);
        let start = default_state();
        assert!(!goal.is_satisfied(&start, &cx));

        let after_trip = PlannerState {
            trips_done: 1,
            ..start
        };
        assert!(goal.is_satisfied(&after_trip, &cx));
    }

    #[test]
    fn multi_trip_goal_increments() {
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Foraging, 2, &cx);
        let state = PlannerState {
            trips_done: 2,
            ..default_state()
        };
        assert!(!goal.is_satisfied(&state, &cx));

        let state = PlannerState {
            trips_done: 3,
            ..default_state()
        };
        assert!(goal.is_satisfied(&state, &cx));
    }

    #[test]
    fn building_goal_checks_construction() {
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Building, 0, &cx);
        assert!(!goal.is_satisfied(&default_state(), &cx));

        let done = PlannerState {
            construction_done: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&done, &cx));
    }

    #[test]
    fn mating_goal_checks_interaction() {
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Mating, 0, &cx);
        assert!(!goal.is_satisfied(&default_state(), &cx));

        let done = PlannerState {
            interaction_done: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&done, &cx));
    }

    #[test]
    fn resting_goal_drops_hunger_when_stores_empty() {
        // Tickets 091/092: with `HasStoredFood` absent, the Resting goal
        // drops `HungerOk` so a hungry-tired-cold cat with empty stores
        // can still Sleep + SelfGroom and re-elect on the next decision
        // tick. The same marker gates `EatAtStores`'s precondition, so
        // the goal-side and action-side branching share one source of
        // truth (092 unification).
        let stocked = food_stocked_markers();
        let empty = empty_markers();
        let goal_full = goal_for_disposition(DispositionKind::Resting, 0, &ctx(&stocked));
        assert_eq!(goal_full.predicates.len(), 3);
        assert!(goal_full
            .predicates
            .contains(&StatePredicate::HungerOk(true)));

        let goal_partial = goal_for_disposition(DispositionKind::Resting, 0, &ctx(&empty));
        assert_eq!(goal_partial.predicates.len(), 2);
        assert!(!goal_partial
            .predicates
            .contains(&StatePredicate::HungerOk(true)));
        assert!(goal_partial
            .predicates
            .contains(&StatePredicate::EnergyOk(true)));
        assert!(goal_partial
            .predicates
            .contains(&StatePredicate::TemperatureOk(true)));
    }

    #[test]
    fn heuristic_counts_unsatisfied() {
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Resting, 0, &cx);
        let all_bad = PlannerState {
            hunger_ok: false,
            energy_ok: false,
            temperature_ok: false,
            ..default_state()
        };
        assert_eq!(goal.heuristic(&all_bad, &cx), 3);

        let one_bad = PlannerState {
            hunger_ok: false,
            energy_ok: true,
            temperature_ok: true,
            ..default_state()
        };
        assert_eq!(goal.heuristic(&one_bad, &cx), 1);
    }
}
