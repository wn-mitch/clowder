//! Hawk GOAP plan component — per-hawk active plan state.
//!
//! Mirrors [`FoxGoapPlan`](super::fox_goap_plan::FoxGoapPlan) shape; see
//! that file's rustdoc for the structural invariants. Lifted into a
//! parallel type so hawks can add aerial-specific phases or fields
//! without affecting foxes.

use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::ai::hawk_planner::{HawkDispositionKind, HawkDomain, HawkGoapActionKind};
use crate::ai::planner::core::PlannedStep;
use crate::components::goap_plan::StepPhase;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// HawkGoapPlan — active plan for a hawk
// ---------------------------------------------------------------------------

/// Active GOAP plan for a hawk. Inserted by `hawk_evaluate_and_plan` and
/// ticked by `hawk_resolve_goap_plans` until exhausted or interrupted.
#[derive(Component, Debug, Clone)]
pub struct HawkGoapPlan {
    pub steps: Vec<PlannedStep<HawkDomain>>,
    pub current_step: usize,
    pub kind: HawkDispositionKind,
    pub adopted_tick: u64,
    pub trips_done: u32,
    pub target_trips: u32,
    pub step_state: Vec<HawkStepState>,
    pub replan_count: u32,
    pub max_replans: u32,
    /// Action kinds that failed during this plan's lifetime. Filtered out
    /// during replanning to avoid regenerating identical impossible plans.
    pub failed_actions: HashSet<HawkGoapActionKind>,
}

impl HawkGoapPlan {
    pub const DEFAULT_MAX_REPLANS: u32 = 3;

    pub fn new(kind: HawkDispositionKind, tick: u64, steps: Vec<PlannedStep<HawkDomain>>) -> Self {
        let step_count = steps.len();
        Self {
            steps,
            current_step: 0,
            kind,
            adopted_tick: tick,
            trips_done: 0,
            target_trips: kind.target_completions(),
            step_state: vec![HawkStepState::default(); step_count],
            replan_count: 0,
            max_replans: Self::DEFAULT_MAX_REPLANS,
            failed_actions: HashSet::new(),
        }
    }

    pub fn current(&self) -> Option<&PlannedStep<HawkDomain>> {
        self.steps.get(self.current_step)
    }

    pub fn current_state_mut(&mut self) -> Option<&mut HawkStepState> {
        self.step_state.get_mut(self.current_step)
    }

    pub fn current_state(&self) -> Option<&HawkStepState> {
        self.step_state.get(self.current_step)
    }

    pub fn advance(&mut self) {
        self.current_step += 1;
    }

    pub fn is_exhausted(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    pub fn replan(&mut self, new_steps: Vec<PlannedStep<HawkDomain>>) -> bool {
        if self.replan_count >= self.max_replans {
            return false;
        }
        let step_count = new_steps.len();
        self.steps = new_steps;
        self.step_state = vec![HawkStepState::default(); step_count];
        self.current_step = 0;
        self.replan_count += 1;
        true
    }
}

// ---------------------------------------------------------------------------
// HawkStepState — per-step runtime data
// ---------------------------------------------------------------------------

/// Runtime state for a single executing hawk step. Same shape as
/// [`FoxStepState`](super::fox_goap_plan::FoxStepState) — kept separate so
/// future hawk-specific fields (e.g. altitude, dive trajectory) don't
/// leak into the fox path.
#[derive(Debug, Clone, Default)]
pub struct HawkStepState {
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
        let plan = HawkGoapPlan::new(HawkDispositionKind::Hunting, 100, vec![]);
        assert_eq!(plan.current_step, 0);
        assert_eq!(plan.trips_done, 0);
        assert_eq!(plan.kind, HawkDispositionKind::Hunting);
        assert_eq!(plan.adopted_tick, 100);
        assert!(plan.is_exhausted()); // empty steps means already exhausted
    }

    #[test]
    fn advance_moves_step_index() {
        let steps = vec![
            PlannedStep::<HawkDomain> {
                action: HawkGoapActionKind::SpotPrey,
                cost: 2,
            },
            PlannedStep::<HawkDomain> {
                action: HawkGoapActionKind::DiveAttack,
                cost: 3,
            },
        ];
        let mut plan = HawkGoapPlan::new(HawkDispositionKind::Hunting, 100, steps);
        assert!(!plan.is_exhausted());
        assert_eq!(plan.current().unwrap().action, HawkGoapActionKind::SpotPrey);

        plan.advance();
        assert_eq!(
            plan.current().unwrap().action,
            HawkGoapActionKind::DiveAttack
        );

        plan.advance();
        assert!(plan.is_exhausted());
    }

    #[test]
    fn replan_respects_max_replans() {
        let mut plan = HawkGoapPlan::new(HawkDispositionKind::Hunting, 100, vec![]);
        for _ in 0..HawkGoapPlan::DEFAULT_MAX_REPLANS {
            assert!(plan.replan(vec![]));
        }
        assert!(!plan.replan(vec![]));
    }
}
