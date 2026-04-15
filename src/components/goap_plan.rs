use bevy_ecs::prelude::*;

use crate::ai::planner::{GoapActionKind, PlannedStep};
use crate::components::disposition::{CraftingHint, DispositionKind};
use crate::components::personality::Personality;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// GoapPlan — replaces both Disposition and TaskChain for disposition-driven
// behavior. The planner produces the step sequence; the executor ticks it.
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GoapPlan {
    /// Ordered action steps from the planner.
    pub steps: Vec<PlannedStep>,
    /// Index of the currently executing step.
    pub current_step: usize,
    /// Which disposition this plan serves (label for scoring, narrative, respect).
    pub kind: DispositionKind,
    /// Tick when this plan's disposition was first adopted.
    pub adopted_tick: u64,
    /// Completed trip/interaction cycles.
    pub trips_done: u32,
    /// Personality-scaled target trips. Plan completes when trips_done >= target_trips.
    pub target_trips: u32,
    /// Number of times the plan has been regenerated after failure.
    pub replan_count: u32,
    /// Maximum replans before the plan is abandoned.
    pub max_replans: u32,
    /// For Crafting dispositions: which sub-mode the scorer selected.
    pub crafting_hint: Option<CraftingHint>,
    /// Per-step execution state. Parallel to `steps` — initialized when each
    /// step begins executing.
    #[serde(skip)]
    pub step_state: Vec<StepExecutionState>,
}

impl GoapPlan {
    /// Maximum replans allowed before a plan is abandoned.
    pub const DEFAULT_MAX_REPLANS: u32 = 3;

    pub fn new(
        kind: DispositionKind,
        tick: u64,
        personality: &Personality,
        steps: Vec<PlannedStep>,
        crafting_hint: Option<CraftingHint>,
    ) -> Self {
        let step_count = steps.len();
        Self {
            steps,
            current_step: 0,
            kind,
            adopted_tick: tick,
            trips_done: 0,
            target_trips: kind.target_completions(personality),
            replan_count: 0,
            max_replans: Self::DEFAULT_MAX_REPLANS,
            crafting_hint,
            step_state: vec![StepExecutionState::default(); step_count],
        }
    }

    /// The currently active step, if any.
    pub fn current(&self) -> Option<&PlannedStep> {
        self.steps.get(self.current_step)
    }

    /// Mutable access to the current step's execution state.
    pub fn current_state_mut(&mut self) -> Option<&mut StepExecutionState> {
        self.step_state.get_mut(self.current_step)
    }

    /// Read access to the current step's execution state.
    pub fn current_state(&self) -> Option<&StepExecutionState> {
        self.step_state.get(self.current_step)
    }

    /// Advance to the next step after the current one completes.
    pub fn advance(&mut self) {
        self.current_step += 1;
    }

    /// Whether all steps have been executed (plan is exhausted).
    pub fn is_exhausted(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    /// Replace plan steps with a new sequence (for replanning).
    /// Increments replan_count. Returns false if max replans exceeded.
    pub fn replan(&mut self, new_steps: Vec<PlannedStep>) -> bool {
        if self.replan_count >= self.max_replans {
            return false;
        }
        let step_count = new_steps.len();
        self.steps = new_steps;
        self.step_state = vec![StepExecutionState::default(); step_count];
        self.current_step = 0;
        self.replan_count += 1;
        true
    }
}

// ---------------------------------------------------------------------------
// StepExecutionState — per-step runtime data
// ---------------------------------------------------------------------------

/// Runtime state for a single executing step. Not part of the planner's model —
/// filled in by the executor as the step runs.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StepExecutionState {
    /// Ticks elapsed since this step started executing.
    pub ticks_elapsed: u64,
    /// Entity target resolved by the executor (e.g., prey, social partner).
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    /// Concrete position resolved from the abstract zone.
    pub target_position: Option<Position>,
    /// Pre-computed A* path for movement steps.
    #[serde(skip)]
    pub cached_path: Option<Vec<Position>>,
    /// Internal phase for multi-phase actions (e.g., EngagePrey).
    pub phase: StepPhase,
    /// Patrol direction for hunt/forage search patterns.
    pub patrol_dir: (i32, i32),
    /// Consecutive ticks with zero position change. Reset on any movement.
    pub no_move_ticks: u64,
}

/// Internal phase tracking for complex actions.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StepPhase {
    #[default]
    NotStarted,
    InProgress,
    // Hunt-specific phases (used by EngagePrey):
    Searching,
    Approaching,
    Stalking,
    Chasing,
    Pouncing,
}

// ---------------------------------------------------------------------------
// PlanNarrative — message emitted at plan lifecycle transitions
// ---------------------------------------------------------------------------

/// Emitted by the executor on plan lifecycle events. Consumed by
/// `emit_plan_narrative` to generate narrative log entries.
#[derive(Message, Debug, Clone)]
pub struct PlanNarrative {
    pub entity: Entity,
    pub kind: DispositionKind,
    pub event: PlanEvent,
    pub completions: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlanEvent {
    /// New plan created by evaluate_and_plan.
    Adopted,
    /// All steps succeeded and disposition target met.
    Completed,
    /// A step failed but the planner re-sequenced successfully.
    Replanned,
    /// Replan failed or retry limit hit — plan dropped.
    Abandoned,
}

// ---------------------------------------------------------------------------
// Helper: map GoapActionKind to Action for CurrentAction sync
// ---------------------------------------------------------------------------

use crate::ai::Action;

impl GoapActionKind {
    /// Map a GOAP action to the `Action` enum for `CurrentAction` sync
    /// and narrative template matching.
    pub fn to_action(self, disposition: DispositionKind) -> Action {
        match self {
            Self::TravelTo(_) => match disposition {
                DispositionKind::Hunting => Action::Hunt,
                DispositionKind::Foraging => Action::Forage,
                DispositionKind::Guarding => Action::Patrol,
                DispositionKind::Building => Action::Build,
                DispositionKind::Farming => Action::Farm,
                DispositionKind::Exploring => Action::Explore,
                _ => Action::Wander,
            },
            Self::SearchPrey | Self::EngagePrey | Self::DepositPrey => Action::Hunt,
            Self::ForageItem | Self::DepositFood => Action::Forage,
            Self::EatAtStores => Action::Eat,
            Self::Sleep => Action::Sleep,
            Self::SelfGroom | Self::GroomOther => Action::Groom,
            Self::PatrolArea | Self::Survey => Action::Patrol,
            Self::EngageThreat => Action::Fight,
            Self::SocializeWith => Action::Socialize,
            Self::MentorCat => Action::Mentor,
            Self::GatherMaterials | Self::DeliverMaterials | Self::Construct => Action::Build,
            Self::TendCrops | Self::HarvestCrops => Action::Farm,
            Self::GatherHerb | Self::PrepareRemedy | Self::ApplyRemedy | Self::SetWard => {
                Action::Herbcraft
            }
            Self::Scry | Self::SpiritCommunion | Self::CleanseCorruption | Self::HarvestCarcass => Action::PracticeMagic,
            Self::MateWith => Action::Mate,
            Self::FeedKitten => Action::Caretake,
            Self::DeliverDirective => Action::Coordinate,
            Self::ExploreSurvey => Action::Explore,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::planner::{GoapActionKind, PlannedStep, PlannerZone};

    fn test_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    fn sample_steps() -> Vec<PlannedStep> {
        vec![
            PlannedStep {
                action: GoapActionKind::TravelTo(PlannerZone::HuntingGround),
                cost: 3,
            },
            PlannedStep {
                action: GoapActionKind::SearchPrey,
                cost: 3,
            },
            PlannedStep {
                action: GoapActionKind::EngagePrey,
                cost: 2,
            },
        ]
    }

    #[test]
    fn new_plan_sets_target_trips() {
        let p = test_personality();
        let plan = GoapPlan::new(DispositionKind::Hunting, 100, &p, sample_steps(), None);
        assert_eq!(plan.target_trips, DispositionKind::Hunting.target_completions(&p));
        assert_eq!(plan.trips_done, 0);
        assert_eq!(plan.adopted_tick, 100);
        assert_eq!(plan.replan_count, 0);
    }

    #[test]
    fn advance_increments_step() {
        let p = test_personality();
        let mut plan = GoapPlan::new(DispositionKind::Hunting, 0, &p, sample_steps(), None);
        assert_eq!(plan.current_step, 0);
        assert!(!plan.is_exhausted());

        plan.advance();
        assert_eq!(plan.current_step, 1);

        plan.advance();
        plan.advance();
        assert!(plan.is_exhausted());
        assert!(plan.current().is_none());
    }

    #[test]
    fn replan_replaces_steps() {
        let p = test_personality();
        let mut plan = GoapPlan::new(DispositionKind::Hunting, 0, &p, sample_steps(), None);
        plan.advance(); // Move to step 1.

        let new_steps = vec![PlannedStep {
            action: GoapActionKind::ForageItem,
            cost: 3,
        }];
        assert!(plan.replan(new_steps));
        assert_eq!(plan.current_step, 0);
        assert_eq!(plan.replan_count, 1);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.step_state.len(), 1);
    }

    #[test]
    fn replan_respects_max_replans() {
        let p = test_personality();
        let mut plan = GoapPlan::new(DispositionKind::Hunting, 0, &p, sample_steps(), None);
        plan.max_replans = 2;

        assert!(plan.replan(sample_steps())); // replan_count = 1
        assert!(plan.replan(sample_steps())); // replan_count = 2
        assert!(!plan.replan(sample_steps())); // exceeds max
    }

    #[test]
    fn step_state_parallel_to_steps() {
        let p = test_personality();
        let plan = GoapPlan::new(DispositionKind::Hunting, 0, &p, sample_steps(), None);
        assert_eq!(plan.steps.len(), plan.step_state.len());
    }

    #[test]
    fn action_kind_to_action_mapping() {
        assert_eq!(
            GoapActionKind::SearchPrey.to_action(DispositionKind::Hunting),
            Action::Hunt
        );
        assert_eq!(
            GoapActionKind::EatAtStores.to_action(DispositionKind::Resting),
            Action::Eat
        );
        assert_eq!(
            GoapActionKind::TravelTo(PlannerZone::Stores).to_action(DispositionKind::Hunting),
            Action::Hunt
        );
        assert_eq!(
            GoapActionKind::TravelTo(PlannerZone::Stores).to_action(DispositionKind::Resting),
            Action::Wander
        );
    }
}
