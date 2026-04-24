//! Hawk action definitions — data-driven precondition/effect tables per disposition.

use crate::ai::planner::core::ActionDef;

use super::{
    HawkDomain, HawkGoapActionKind, HawkStateEffect as Eff, HawkStatePredicate as Pred, HawkZone,
};

// ---------------------------------------------------------------------------
// Travel action helper
// ---------------------------------------------------------------------------

fn soar_to(zone: HawkZone, cost: u32) -> ActionDef<HawkDomain> {
    ActionDef {
        kind: HawkGoapActionKind::SoarTo(zone),
        cost,
        preconditions: vec![Pred::ZoneIsNot(zone)],
        effects: vec![Eff::SetZone(zone)],
    }
}

// ---------------------------------------------------------------------------
// Hunting: soar to hunting ground, spot prey, dive to kill
// ---------------------------------------------------------------------------

pub fn hunting_actions() -> Vec<ActionDef<HawkDomain>> {
    vec![
        soar_to(HawkZone::HuntingGround, 2),
        ActionDef {
            kind: HawkGoapActionKind::SpotPrey,
            cost: 3,
            preconditions: vec![Pred::ZoneIs(HawkZone::HuntingGround)],
            effects: vec![Eff::SetPreySpotted(true)],
        },
        ActionDef {
            kind: HawkGoapActionKind::DiveAttack,
            cost: 2,
            preconditions: vec![Pred::PreySpotted(true)],
            effects: vec![
                Eff::SetPreySpotted(false),
                Eff::SetHungerOk(true),
                Eff::IncrementTrips,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Soaring: patrol the sky over open terrain
// ---------------------------------------------------------------------------

pub fn soaring_actions() -> Vec<ActionDef<HawkDomain>> {
    vec![soar_to(HawkZone::Sky, 1)]
}

// ---------------------------------------------------------------------------
// Fleeing: fly to map edge
// ---------------------------------------------------------------------------

pub fn fleeing_actions() -> Vec<ActionDef<HawkDomain>> {
    vec![
        soar_to(HawkZone::MapEdge, 1),
        ActionDef {
            kind: HawkGoapActionKind::FleeSky,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(HawkZone::MapEdge)],
            effects: vec![], // Plan complete once at map edge.
        },
    ]
}

// ---------------------------------------------------------------------------
// Resting: fly to perch, rest
// ---------------------------------------------------------------------------

pub fn resting_actions() -> Vec<ActionDef<HawkDomain>> {
    vec![
        soar_to(HawkZone::Perch, 2),
        ActionDef {
            kind: HawkGoapActionKind::Rest,
            cost: 3,
            preconditions: vec![Pred::ZoneIs(HawkZone::Perch)],
            effects: vec![Eff::SetHungerOk(true), Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

use super::HawkDispositionKind;

pub fn actions_for_disposition(kind: HawkDispositionKind) -> Vec<ActionDef<HawkDomain>> {
    match kind {
        HawkDispositionKind::Hunting => hunting_actions(),
        HawkDispositionKind::Soaring => soaring_actions(),
        HawkDispositionKind::Fleeing => fleeing_actions(),
        HawkDispositionKind::Resting => resting_actions(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::hawk_planner::HawkPlannerState;
    use crate::ai::planner::core;

    #[test]
    fn hunting_plan_from_sky() {
        let start = HawkPlannerState {
            zone: HawkZone::Sky,
            prey_spotted: false,
            hunger_ok: false,
            trips_done: 0,
        };
        let goal = super::super::goals::goal_for_disposition(HawkDispositionKind::Hunting);
        let actions = hunting_actions();
        let plan = core::make_plan::<HawkDomain>(start, &actions, &goal, 8, 500)
            .expect("should find hunting plan");

        assert!(!plan.is_empty());
        assert_eq!(plan.last().unwrap().action, HawkGoapActionKind::DiveAttack);
    }

    #[test]
    fn fleeing_plan_from_sky() {
        let start = HawkPlannerState {
            zone: HawkZone::Sky,
            prey_spotted: false,
            hunger_ok: false,
            trips_done: 0,
        };
        let goal = super::super::goals::goal_for_disposition(HawkDispositionKind::Fleeing);
        let actions = fleeing_actions();
        let plan = core::make_plan::<HawkDomain>(start, &actions, &goal, 8, 500)
            .expect("should find fleeing plan");

        assert!(!plan.is_empty());
    }

    #[test]
    fn resting_plan_from_sky() {
        let start = HawkPlannerState {
            zone: HawkZone::Sky,
            prey_spotted: false,
            hunger_ok: false,
            trips_done: 0,
        };
        let goal = super::super::goals::goal_for_disposition(HawkDispositionKind::Resting);
        let actions = resting_actions();
        let plan = core::make_plan::<HawkDomain>(start, &actions, &goal, 8, 500)
            .expect("should find resting plan");

        assert!(!plan.is_empty());
        assert_eq!(plan.last().unwrap().action, HawkGoapActionKind::Rest);
    }
}
