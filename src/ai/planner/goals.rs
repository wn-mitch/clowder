use crate::components::disposition::DispositionKind;
#[cfg(test)]
use crate::components::markers;

use super::{GoalState, PlanContext, StatePredicate};

/// Build the goal state for a single trip of the given disposition.
///
/// Multi-trip dispositions (Hunting, Foraging, etc.) target `trips_done >= current_trips + 1`.
/// The executor handles the trip loop — it checks `trips_done < target_trips` and re-invokes
/// the planner for each subsequent trip.
///
/// 150 R5a: `ctx` is no longer consulted — the Resting partial-goal
/// branch (091/092) was retired when Eating took ownership of hunger.
/// The parameter stays in the signature for shape compatibility with
/// the rest of the planner surface; future per-disposition goal
/// branches that need marker context will re-engage it.
pub fn goal_for_disposition(
    kind: DispositionKind,
    current_trips: u32,
    _ctx: &PlanContext<'_>,
) -> GoalState {
    match kind {
        // 150 R5a: Resting now covers Sleep + SelfGroom only. Goal
        // gates on energy + temperature; hunger is handled by the
        // separate `Eating` disposition. The 091/092 partial-goal
        // dance (drop HungerOk when stores are empty) is no longer
        // needed here — the marker-gated branch lives on `Eating`'s
        // eligibility instead.
        DispositionKind::Resting => GoalState {
            predicates: vec![
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ],
        },

        // 150 R5a: Eating's goal is hunger-only. Single-trip plan:
        // `[TravelTo(Stores), EatAtStores]`. The action's effect is
        // `SetHungerOk(true)`, so a successful chain reaches the goal
        // in one trip. Marker eligibility (HasStoredFood) gates the
        // EatAtStores precondition; if the marker flips false mid-plan
        // the cat re-plans.
        DispositionKind::Eating => GoalState {
            predicates: vec![StatePredicate::HungerOk(true)],
        },

        // Building completes when construction is done.
        DispositionKind::Building => GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        },

        // Mating, Coordinating, Mentoring, and Grooming complete on
        // interaction. 154 added Mentoring to Pattern B; 158 added
        // Grooming for the same reason — equivalent-effect sibling
        // pre-pruning under Socializing's count-based goal hid
        // GroomOther entirely.
        DispositionKind::Mating
        | DispositionKind::Coordinating
        | DispositionKind::Mentoring
        | DispositionKind::Grooming => GoalState {
            predicates: vec![StatePredicate::InteractionDone(true)],
        },

        // All other dispositions: one trip increment.
        // 155: `Crafting` retired in favor of three new dispositions
        // (Herbalism / Witchcraft / Cooking). Each inherits the
        // single-trip completion proxy. The chain shape (which step
        // terminates with `IncrementTrips`) is carried by the
        // per-Disposition plan template, not the goal predicate.
        DispositionKind::Hunting
        | DispositionKind::Foraging
        | DispositionKind::Guarding
        | DispositionKind::Socializing
        | DispositionKind::Farming
        | DispositionKind::Herbalism
        | DispositionKind::Witchcraft
        | DispositionKind::Cooking
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
    fn resting_goal_checks_energy_and_temperature() {
        // 150 R5a: Resting goal gates on energy + temperature only.
        // Hunger is owned by the new `Eating` disposition (see
        // `eating_goal_checks_hunger` below).
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Resting, 0, &cx);
        let satisfied = PlannerState {
            energy_ok: true,
            temperature_ok: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&satisfied, &cx));

        let tired = PlannerState {
            energy_ok: false,
            ..satisfied.clone()
        };
        assert!(!goal.is_satisfied(&tired, &cx));
        assert_eq!(goal.heuristic(&tired, &cx), 1);

        // Hunger no longer gates Resting — a hungry-but-rested cat is
        // "resting-satisfied."
        let hungry_only = PlannerState {
            hunger_ok: false,
            energy_ok: true,
            temperature_ok: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&hungry_only, &cx));
    }

    #[test]
    fn eating_goal_checks_hunger() {
        // 150 R5a sibling test: Eating's goal is a single HungerOk
        // predicate. Reaching it is the planner's job; the chain
        // template lives in `actions::eating_actions`.
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Eating, 0, &cx);
        let satisfied = PlannerState {
            hunger_ok: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&satisfied, &cx));

        let hungry = PlannerState {
            hunger_ok: false,
            ..satisfied
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
    fn mentoring_goal_checks_interaction() {
        // 154: Mentoring's completion proxy is `InteractionDone(true)`
        // (Pattern B, mirrors Mating). Sharing the proxy ensures the
        // L3 Mentor pick survives the disposition collapse intact.
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Mentoring, 0, &cx);
        assert!(!goal.is_satisfied(&default_state(), &cx));

        let done = PlannerState {
            interaction_done: true,
            ..default_state()
        };
        assert!(goal.is_satisfied(&done, &cx));
    }

    #[test]
    fn resting_goal_unaffected_by_stores_marker() {
        // 150 R5a: the 091/092 partial-goal branching (drop HungerOk
        // when stores are empty) was retired when Eating took over
        // hunger. Resting now has the same two-predicate goal
        // regardless of marker state — `HasStoredFood` only affects
        // Eating's eligibility.
        let stocked = food_stocked_markers();
        let empty = empty_markers();
        let goal_stocked = goal_for_disposition(DispositionKind::Resting, 0, &ctx(&stocked));
        let goal_empty = goal_for_disposition(DispositionKind::Resting, 0, &ctx(&empty));
        assert_eq!(goal_stocked.predicates.len(), 2);
        assert_eq!(goal_empty.predicates.len(), 2);
        for predicates in [&goal_stocked.predicates, &goal_empty.predicates] {
            assert!(!predicates.contains(&StatePredicate::HungerOk(true)));
            assert!(predicates.contains(&StatePredicate::EnergyOk(true)));
            assert!(predicates.contains(&StatePredicate::TemperatureOk(true)));
        }
    }

    #[test]
    fn resting_heuristic_counts_unsatisfied() {
        // 150 R5a: heuristic over Resting's 2-predicate goal (energy +
        // temperature). Hunger no longer participates.
        let m = food_stocked_markers();
        let cx = ctx(&m);
        let goal = goal_for_disposition(DispositionKind::Resting, 0, &cx);
        let both_bad = PlannerState {
            energy_ok: false,
            temperature_ok: false,
            ..default_state()
        };
        assert_eq!(goal.heuristic(&both_bad, &cx), 2);

        let one_bad = PlannerState {
            energy_ok: false,
            temperature_ok: true,
            ..default_state()
        };
        assert_eq!(goal.heuristic(&one_bad, &cx), 1);
    }
}
