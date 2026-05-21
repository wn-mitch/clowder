//! Snake GOAP plan component — per-snake active plan state.
//!
//! Mirrors [`FoxGoapPlan`](super::fox_goap_plan::FoxGoapPlan) shape; see
//! that file's rustdoc for the structural invariants. Lifted into a
//! parallel type so snakes can carry future thermoregulation-specific
//! step state without affecting foxes or hawks.

use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::ai::planner::core::PlannedStep;
use crate::ai::snake_planner::{SnakeDispositionKind, SnakeDomain, SnakeGoapActionKind};
use crate::components::goap_plan::StepPhase;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// SnakeGoapPlan — active plan for a snake
// ---------------------------------------------------------------------------

/// Active GOAP plan for a snake. Inserted by `snake_evaluate_and_plan`
/// and ticked by `snake_resolve_goap_plans` until exhausted or
/// interrupted.
#[derive(Component, Debug, Clone)]
pub struct SnakeGoapPlan {
    pub steps: Vec<PlannedStep<SnakeDomain>>,
    pub current_step: usize,
    pub kind: SnakeDispositionKind,
    pub adopted_tick: u64,
    pub trips_done: u32,
    pub target_trips: u32,
    pub step_state: Vec<SnakeStepState>,
    pub replan_count: u32,
    pub max_replans: u32,
    pub failed_actions: HashSet<SnakeGoapActionKind>,
}

impl SnakeGoapPlan {
    pub const DEFAULT_MAX_REPLANS: u32 = 3;

    pub fn new(
        kind: SnakeDispositionKind,
        tick: u64,
        steps: Vec<PlannedStep<SnakeDomain>>,
    ) -> Self {
        let step_count = steps.len();
        Self {
            steps,
            current_step: 0,
            kind,
            adopted_tick: tick,
            trips_done: 0,
            target_trips: kind.target_completions(),
            step_state: vec![SnakeStepState::default(); step_count],
            replan_count: 0,
            max_replans: Self::DEFAULT_MAX_REPLANS,
            failed_actions: HashSet::new(),
        }
    }

    pub fn current(&self) -> Option<&PlannedStep<SnakeDomain>> {
        self.steps.get(self.current_step)
    }

    pub fn current_state_mut(&mut self) -> Option<&mut SnakeStepState> {
        self.step_state.get_mut(self.current_step)
    }

    pub fn current_state(&self) -> Option<&SnakeStepState> {
        self.step_state.get(self.current_step)
    }

    pub fn advance(&mut self) {
        self.current_step += 1;
    }

    pub fn is_exhausted(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    pub fn replan(&mut self, new_steps: Vec<PlannedStep<SnakeDomain>>) -> bool {
        if self.replan_count >= self.max_replans {
            return false;
        }
        let step_count = new_steps.len();
        self.steps = new_steps;
        self.step_state = vec![SnakeStepState::default(); step_count];
        self.current_step = 0;
        self.replan_count += 1;
        true
    }
}

// ---------------------------------------------------------------------------
// SnakeStepState — per-step runtime data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SnakeStepState {
    pub ticks_elapsed: u64,
    pub target_entity: Option<Entity>,
    pub target_position: Option<Position>,
    pub cached_path: Option<Vec<Position>>,
    pub phase: StepPhase,
    pub patrol_dir: (i32, i32),
    pub no_move_ticks: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_plan_defaults() {
        let plan = SnakeGoapPlan::new(SnakeDispositionKind::Ambushing, 100, vec![]);
        assert_eq!(plan.current_step, 0);
        assert_eq!(plan.trips_done, 0);
        assert_eq!(plan.kind, SnakeDispositionKind::Ambushing);
        assert_eq!(plan.adopted_tick, 100);
        assert!(plan.is_exhausted());
    }

    #[test]
    fn advance_moves_step_index() {
        let steps = vec![
            PlannedStep::<SnakeDomain> {
                action: SnakeGoapActionKind::SetAmbush,
                cost: 2,
            },
            PlannedStep::<SnakeDomain> {
                action: SnakeGoapActionKind::Strike,
                cost: 3,
            },
        ];
        let mut plan = SnakeGoapPlan::new(SnakeDispositionKind::Ambushing, 100, steps);
        assert!(!plan.is_exhausted());
        assert_eq!(
            plan.current().unwrap().action,
            SnakeGoapActionKind::SetAmbush
        );

        plan.advance();
        assert_eq!(plan.current().unwrap().action, SnakeGoapActionKind::Strike);

        plan.advance();
        assert!(plan.is_exhausted());
    }

    #[test]
    fn replan_respects_max_replans() {
        let mut plan = SnakeGoapPlan::new(SnakeDispositionKind::Ambushing, 100, vec![]);
        for _ in 0..SnakeGoapPlan::DEFAULT_MAX_REPLANS {
            assert!(plan.replan(vec![]));
        }
        assert!(!plan.replan(vec![]));
    }
}
