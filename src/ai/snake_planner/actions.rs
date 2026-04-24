//! Snake action definitions — data-driven precondition/effect tables per disposition.

use crate::ai::planner::core::ActionDef;

use super::{
    SnakeDomain, SnakeGoapActionKind, SnakeStateEffect as Eff, SnakeStatePredicate as Pred,
    SnakeZone,
};

// ---------------------------------------------------------------------------
// Travel action helper
// ---------------------------------------------------------------------------

fn slide_to(zone: SnakeZone, cost: u32) -> ActionDef<SnakeDomain> {
    ActionDef {
        kind: SnakeGoapActionKind::SlideTo(zone),
        cost,
        preconditions: vec![Pred::ZoneIsNot(zone)],
        effects: vec![Eff::SetZone(zone)],
    }
}

// ---------------------------------------------------------------------------
// Ambushing: slither to cover, coil, wait for prey, strike
// ---------------------------------------------------------------------------

pub fn ambushing_actions() -> Vec<ActionDef<SnakeDomain>> {
    vec![
        slide_to(SnakeZone::Cover, 2),
        ActionDef {
            kind: SnakeGoapActionKind::SetAmbush,
            cost: 2,
            preconditions: vec![Pred::ZoneIs(SnakeZone::Cover)],
            effects: vec![Eff::SetPreyInRange(true)],
        },
        ActionDef {
            kind: SnakeGoapActionKind::Strike,
            cost: 1,
            preconditions: vec![Pred::PreyInRange(true)],
            effects: vec![
                Eff::SetPreyInRange(false),
                Eff::SetHungerOk(true),
                Eff::IncrementTrips,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Foraging: active search in prey-rich areas
// ---------------------------------------------------------------------------

pub fn foraging_actions() -> Vec<ActionDef<SnakeDomain>> {
    vec![
        slide_to(SnakeZone::HuntingGround, 3),
        ActionDef {
            kind: SnakeGoapActionKind::Strike,
            cost: 2,
            preconditions: vec![Pred::ZoneIs(SnakeZone::HuntingGround)],
            effects: vec![
                Eff::SetHungerOk(true),
                Eff::IncrementTrips,
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Basking: thermoregulate on warm terrain
// ---------------------------------------------------------------------------

pub fn basking_actions() -> Vec<ActionDef<SnakeDomain>> {
    vec![
        slide_to(SnakeZone::BaskingSpot, 2),
        ActionDef {
            kind: SnakeGoapActionKind::Bask,
            cost: 3,
            preconditions: vec![Pred::ZoneIs(SnakeZone::BaskingSpot)],
            effects: vec![Eff::SetWarm(true), Eff::IncrementTrips],
        },
    ]
}

// ---------------------------------------------------------------------------
// Fleeing: retreat to cover or map edge
// ---------------------------------------------------------------------------

pub fn fleeing_actions() -> Vec<ActionDef<SnakeDomain>> {
    vec![
        slide_to(SnakeZone::MapEdge, 1),
        ActionDef {
            kind: SnakeGoapActionKind::Retreat,
            cost: 1,
            preconditions: vec![Pred::ZoneIs(SnakeZone::MapEdge)],
            effects: vec![],
        },
    ]
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

use super::SnakeDispositionKind;

pub fn actions_for_disposition(kind: SnakeDispositionKind) -> Vec<ActionDef<SnakeDomain>> {
    match kind {
        SnakeDispositionKind::Ambushing => ambushing_actions(),
        SnakeDispositionKind::Foraging => foraging_actions(),
        SnakeDispositionKind::Basking => basking_actions(),
        SnakeDispositionKind::Fleeing => fleeing_actions(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::planner::core;
    use crate::ai::snake_planner::SnakePlannerState;

    #[test]
    fn ambush_plan_from_open_ground() {
        let start = SnakePlannerState {
            zone: SnakeZone::HuntingGround,
            prey_in_range: false,
            hunger_ok: false,
            warm: true,
            trips_done: 0,
        };
        let goal = super::super::goals::goal_for_disposition(SnakeDispositionKind::Ambushing);
        let actions = ambushing_actions();
        let plan = core::make_plan::<SnakeDomain>(start, &actions, &goal, 8, 500)
            .expect("should find ambush plan");

        assert!(!plan.is_empty());
        assert_eq!(plan.last().unwrap().action, SnakeGoapActionKind::Strike);
    }

    #[test]
    fn basking_plan_from_cover() {
        let start = SnakePlannerState {
            zone: SnakeZone::Cover,
            prey_in_range: false,
            hunger_ok: true,
            warm: false,
            trips_done: 0,
        };
        let goal = super::super::goals::goal_for_disposition(SnakeDispositionKind::Basking);
        let actions = basking_actions();
        let plan = core::make_plan::<SnakeDomain>(start, &actions, &goal, 8, 500)
            .expect("should find basking plan");

        assert!(!plan.is_empty());
        assert_eq!(plan.last().unwrap().action, SnakeGoapActionKind::Bask);
    }

    #[test]
    fn fleeing_plan_from_cover() {
        let start = SnakePlannerState {
            zone: SnakeZone::Cover,
            prey_in_range: false,
            hunger_ok: false,
            warm: true,
            trips_done: 0,
        };
        let goal = super::super::goals::goal_for_disposition(SnakeDispositionKind::Fleeing);
        let actions = fleeing_actions();
        let plan = core::make_plan::<SnakeDomain>(start, &actions, &goal, 8, 500)
            .expect("should find fleeing plan");

        assert!(!plan.is_empty());
    }
}
