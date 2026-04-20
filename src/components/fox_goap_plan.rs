//! Fox GOAP plan component — per-fox active plan state.

use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::ai::fox_planner::{FoxDispositionKind, FoxDomain, FoxGoapActionKind};
use crate::ai::planner::core::PlannedStep;
use crate::components::goap_plan::StepPhase;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// FoxGoapPlan — active plan for a fox
// ---------------------------------------------------------------------------

/// Active GOAP plan for a fox. Inserted by `fox_evaluate_and_plan` and
/// ticked by `fox_resolve_goap_plans` until exhausted or interrupted.
#[derive(Component, Debug, Clone)]
pub struct FoxGoapPlan {
    /// Ordered action steps from the planner.
    pub steps: Vec<PlannedStep<FoxDomain>>,
    /// Index of the currently executing step.
    pub current_step: usize,
    /// Which disposition this plan serves.
    pub kind: FoxDispositionKind,
    /// Tick when this plan's disposition was first adopted.
    pub adopted_tick: u64,
    /// Completed trip/interaction cycles.
    pub trips_done: u32,
    /// Target trip count before the plan is considered exhausted.
    pub target_trips: u32,
    /// Per-step execution state. Parallel to `steps`.
    pub step_state: Vec<FoxStepState>,
    /// Number of times the plan has been regenerated after failure.
    pub replan_count: u32,
    /// Maximum replans before the plan is abandoned.
    pub max_replans: u32,
    /// Action kinds that failed during this plan's lifetime. Filtered out
    /// during replanning to avoid regenerating identical impossible plans.
    pub failed_actions: HashSet<FoxGoapActionKind>,
}

impl FoxGoapPlan {
    pub const DEFAULT_MAX_REPLANS: u32 = 3;

    pub fn new(kind: FoxDispositionKind, tick: u64, steps: Vec<PlannedStep<FoxDomain>>) -> Self {
        let step_count = steps.len();
        Self {
            steps,
            current_step: 0,
            kind,
            adopted_tick: tick,
            trips_done: 0,
            target_trips: kind.target_completions(),
            step_state: vec![FoxStepState::default(); step_count],
            replan_count: 0,
            max_replans: Self::DEFAULT_MAX_REPLANS,
            failed_actions: HashSet::new(),
        }
    }

    /// The currently active step, if any.
    pub fn current(&self) -> Option<&PlannedStep<FoxDomain>> {
        self.steps.get(self.current_step)
    }

    /// Mutable access to the current step's execution state.
    pub fn current_state_mut(&mut self) -> Option<&mut FoxStepState> {
        self.step_state.get_mut(self.current_step)
    }

    /// Read access to the current step's execution state.
    pub fn current_state(&self) -> Option<&FoxStepState> {
        self.step_state.get(self.current_step)
    }

    /// Advance to the next step after the current one completes.
    pub fn advance(&mut self) {
        self.current_step += 1;
    }

    /// Whether all steps have been executed.
    pub fn is_exhausted(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    /// Replace plan steps with a new sequence. Increments replan_count.
    pub fn replan(&mut self, new_steps: Vec<PlannedStep<FoxDomain>>) -> bool {
        if self.replan_count >= self.max_replans {
            return false;
        }
        let step_count = new_steps.len();
        self.steps = new_steps;
        self.step_state = vec![FoxStepState::default(); step_count];
        self.current_step = 0;
        self.replan_count += 1;
        true
    }
}

// ---------------------------------------------------------------------------
// FoxStepState — per-step runtime data
// ---------------------------------------------------------------------------

/// Runtime state for a single executing fox step. Mirrors the cat
/// [`StepExecutionState`] but kept separate so fox-specific phases or fields
/// can be added without affecting cats.
#[derive(Debug, Clone, Default)]
pub struct FoxStepState {
    pub ticks_elapsed: u64,
    pub target_entity: Option<Entity>,
    pub target_position: Option<Position>,
    pub cached_path: Option<Vec<Position>>,
    pub phase: StepPhase,
    /// Patrol direction for movement-based steps.
    pub patrol_dir: (i32, i32),
    /// Consecutive ticks without position change. Triggers failure if too high.
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
        let plan = FoxGoapPlan::new(FoxDispositionKind::Hunting, 100, vec![]);
        assert_eq!(plan.current_step, 0);
        assert_eq!(plan.trips_done, 0);
        assert_eq!(plan.kind, FoxDispositionKind::Hunting);
        assert_eq!(plan.adopted_tick, 100);
        assert!(plan.is_exhausted()); // empty steps means already exhausted
    }

    #[test]
    fn advance_moves_step_index() {
        let steps = vec![
            PlannedStep::<FoxDomain> {
                action: FoxGoapActionKind::SearchPrey,
                cost: 3,
            },
            PlannedStep::<FoxDomain> {
                action: FoxGoapActionKind::StalkPrey,
                cost: 2,
            },
        ];
        let mut plan = FoxGoapPlan::new(FoxDispositionKind::Hunting, 100, steps);
        assert!(!plan.is_exhausted());
        assert_eq!(
            plan.current().unwrap().action,
            FoxGoapActionKind::SearchPrey
        );

        plan.advance();
        assert_eq!(plan.current().unwrap().action, FoxGoapActionKind::StalkPrey);

        plan.advance();
        assert!(plan.is_exhausted());
    }

    #[test]
    fn replan_respects_max_replans() {
        let mut plan = FoxGoapPlan::new(FoxDispositionKind::Hunting, 100, vec![]);
        for _ in 0..FoxGoapPlan::DEFAULT_MAX_REPLANS {
            assert!(plan.replan(vec![]));
        }
        // Exceeded max replans.
        assert!(!plan.replan(vec![]));
    }
}
