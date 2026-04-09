use bevy_ecs::prelude::*;

use crate::components::magic::{RemedyKind, WardKind};
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// TaskChain — generic sequential task system
// ---------------------------------------------------------------------------

/// A sequence of failable steps attached to an entity (typically a cat).
///
/// When present alongside a `CurrentAction`, the TaskChain drives behavior
/// instead of the normal `ticks_remaining` countdown. The `Action` enum value
/// is still set on `CurrentAction` so narrative/scoring systems can reference it.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskChain {
    pub steps: Vec<TaskStep>,
    pub current_step: usize,
    pub on_failure: FailurePolicy,
}

/// A single step in a task chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskStep {
    pub kind: StepKind,
    pub status: StepStatus,
    pub target_position: Option<Position>,
    /// Entity target (e.g. the construction site or garden).
    #[serde(skip)]
    pub target_entity: Option<Entity>,
}

impl TaskStep {
    pub fn new(kind: StepKind) -> Self {
        Self {
            kind,
            status: StepStatus::Pending,
            target_position: None,
            target_entity: None,
        }
    }

    pub fn with_position(mut self, pos: Position) -> Self {
        self.target_position = Some(pos);
        self
    }

    pub fn with_entity(mut self, entity: Entity) -> Self {
        self.target_entity = Some(entity);
        self
    }
}

// ---------------------------------------------------------------------------
// StepKind — what the step does
// ---------------------------------------------------------------------------

/// The kinds of work a step can represent.
///
/// Covers building, magic, and disposition-driven behaviors (hunting, foraging,
/// social, patrol, etc.). Dispositions create TaskChains from these steps.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StepKind {
    // --- Movement ---
    /// Walk to `target_position`.
    MoveTo,

    // --- Building/construction ---
    /// Gather material at a resource tile. Takes ~5 ticks.
    Gather { material: Material, amount: u32 },
    /// Deposit gathered material at a construction site. Instant.
    Deliver { material: Material, amount: u32 },
    /// Contribute construction progress to a `ConstructionSite`.
    Construct,
    /// Repair a damaged `Structure`.
    Repair,

    // --- Farming ---
    /// Tend crops at a `Garden`, advancing `CropState.growth`.
    Tend,
    /// Harvest mature crops into `FoodStores`.
    Harvest,

    // --- Magic/herbcraft ---
    /// Harvest a herb entity into the cat's inventory. ~5 ticks.
    GatherHerb,
    /// Prepare a remedy from inventory herbs. 10 ticks at workshop, 15 without.
    PrepareRemedy { remedy: RemedyKind },
    /// Move to target cat and apply the prepared remedy. Instant.
    ApplyRemedy { remedy: RemedyKind },
    /// Place a ward entity at the current position. 8 ticks.
    SetWard { kind: WardKind },
    /// Sit still and scry a distant tile. 10 ticks.
    Scry,
    /// Reduce corruption on the current tile each tick.
    CleanseCorruption,
    /// Meditate at a special location for mood boost. 15 ticks.
    SpiritCommunion,

    // --- Disposition-driven behaviors ---
    /// Active hunt: scent-search → stalk → pounce. Cat moves every tick.
    /// `patrol_dir` is the initial search direction (into the wind).
    HuntPrey { patrol_dir: (i32, i32) },
    /// Active forage: patrol through terrain checking each tile for yield.
    /// `patrol_dir` is the initial movement direction.
    ForageItem { patrol_dir: (i32, i32) },
    /// Deposit all carried food at a Stores building. Instant on arrival.
    DepositAtStores,
    /// Eat from Stores building. Restores hunger. ~5 ticks.
    EatAtStores,
    /// Sleep in place. Restores energy. Duration in ticks.
    Sleep { ticks: u64 },
    /// Self-groom. Restores warmth. ~8 ticks.
    SelfGroom,
    /// Socialize with target entity. ~10 ticks. Requires proximity.
    Socialize,
    /// Groom another cat. ~8 ticks. Requires proximity.
    GroomOther,
    /// Mentor a nearby cat (skill transfer). ~12 ticks. Requires proximity.
    MentorCat,
    /// Walk to position while scanning for threats. ~20 ticks.
    PatrolTo,
    /// Fight a wildlife threat. ~30 ticks. Requires proximity.
    FightThreat,
    /// Observe surroundings at current position. ~5 ticks.
    Survey,
    /// Deliver a coordinator directive to target cat. ~5 ticks on arrival.
    DeliverDirective,
}

/// Materials used in construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Material {
    Wood,
    Stone,
    Herbs,
}

// ---------------------------------------------------------------------------
// StepStatus
// ---------------------------------------------------------------------------

/// Tracks the progress of a single step.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress { ticks_elapsed: u64 },
    Succeeded,
    Failed(String),
}

impl StepStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, StepStatus::Succeeded | StepStatus::Failed(_))
    }
}

// ---------------------------------------------------------------------------
// FailurePolicy
// ---------------------------------------------------------------------------

/// What happens when a step fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FailurePolicy {
    /// Stop the entire chain. Cat reassesses.
    AbortChain,
    /// Retry the failed step up to N times before aborting.
    RetryStep(u8),
    /// Mark step as failed and continue to the next.
    SkipStep,
}

// ---------------------------------------------------------------------------
// TaskChain methods
// ---------------------------------------------------------------------------

impl TaskChain {
    pub fn new(steps: Vec<TaskStep>, on_failure: FailurePolicy) -> Self {
        Self {
            steps,
            current_step: 0,
            on_failure,
        }
    }

    /// The currently active step, if any.
    pub fn current(&self) -> Option<&TaskStep> {
        self.steps.get(self.current_step)
    }

    /// Mutable access to the currently active step.
    pub fn current_mut(&mut self) -> Option<&mut TaskStep> {
        self.steps.get_mut(self.current_step)
    }

    /// Mark the current step as succeeded and advance to the next.
    pub fn advance(&mut self) {
        if let Some(step) = self.steps.get_mut(self.current_step) {
            step.status = StepStatus::Succeeded;
        }
        self.current_step += 1;
    }

    /// Handle failure of the current step according to `on_failure` policy.
    ///
    /// Returns `true` if the chain should continue (retry or skip), `false`
    /// if the chain is aborted.
    pub fn fail_current(&mut self, reason: String) -> bool {
        if let Some(step) = self.steps.get_mut(self.current_step) {
            match self.on_failure {
                FailurePolicy::AbortChain => {
                    step.status = StepStatus::Failed(reason);
                    // Set current_step past end to signal abort
                    self.current_step = self.steps.len();
                    false
                }
                FailurePolicy::RetryStep(max_retries) => {
                    // Count how many times this step has already failed
                    let attempts = match &step.status {
                        StepStatus::Failed(_) => max_retries, // shouldn't happen, but be safe
                        _ => {
                            // We track retries by resetting to InProgress.
                            // If we've exhausted retries, abort.
                            // For simplicity: decrement the retry counter on the policy.
                            if max_retries == 0 {
                                step.status = StepStatus::Failed(reason);
                                self.current_step = self.steps.len();
                                return false;
                            }
                            self.on_failure = FailurePolicy::RetryStep(max_retries - 1);
                            step.status = StepStatus::Pending;
                            return true;
                        }
                    };
                    if attempts == 0 {
                        step.status = StepStatus::Failed(reason);
                        self.current_step = self.steps.len();
                        false
                    } else {
                        step.status = StepStatus::Pending;
                        true
                    }
                }
                FailurePolicy::SkipStep => {
                    step.status = StepStatus::Failed(reason);
                    self.current_step += 1;
                    true
                }
            }
        } else {
            false
        }
    }

    /// Whether all steps have completed (succeeded or skipped past).
    pub fn is_complete(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    /// Whether the chain was aborted due to a failed step.
    pub fn is_failed(&self) -> bool {
        self.is_complete()
            && self
                .steps
                .iter()
                .any(|s| matches!(s.status, StepStatus::Failed(_)))
    }

    /// Whether all steps succeeded (none failed).
    pub fn is_succeeded(&self) -> bool {
        self.is_complete()
            && self
                .steps
                .iter()
                .all(|s| matches!(s.status, StepStatus::Succeeded))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_chain() -> TaskChain {
        TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(Position::new(5, 5)),
                TaskStep::new(StepKind::Gather {
                    material: Material::Wood,
                    amount: 3,
                }),
                TaskStep::new(StepKind::Deliver {
                    material: Material::Wood,
                    amount: 3,
                }),
            ],
            FailurePolicy::AbortChain,
        )
    }

    #[test]
    fn advance_walks_through_steps() {
        let mut chain = simple_chain();
        assert_eq!(chain.current_step, 0);
        assert!(!chain.is_complete());

        chain.advance();
        assert_eq!(chain.current_step, 1);
        assert!(matches!(chain.steps[0].status, StepStatus::Succeeded));

        chain.advance();
        assert_eq!(chain.current_step, 2);

        chain.advance();
        assert!(chain.is_complete());
        assert!(chain.is_succeeded());
        assert!(!chain.is_failed());
    }

    #[test]
    fn fail_aborts_chain() {
        let mut chain = simple_chain();
        let cont = chain.fail_current("path blocked".into());
        assert!(!cont);
        assert!(chain.is_complete());
        assert!(chain.is_failed());
        assert!(!chain.is_succeeded());
    }

    #[test]
    fn fail_with_retry_retries_then_aborts() {
        let mut chain = TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo),
                TaskStep::new(StepKind::Construct),
            ],
            FailurePolicy::RetryStep(2),
        );

        // First failure: retries remaining = 1
        let cont = chain.fail_current("stuck".into());
        assert!(cont);
        assert_eq!(chain.current_step, 0);
        assert!(matches!(chain.steps[0].status, StepStatus::Pending));

        // Second failure: retries remaining = 0
        let cont = chain.fail_current("still stuck".into());
        assert!(cont);
        assert_eq!(chain.current_step, 0);

        // Third failure: no retries left, abort
        let cont = chain.fail_current("giving up".into());
        assert!(!cont);
        assert!(chain.is_complete());
        assert!(chain.is_failed());
    }

    #[test]
    fn fail_with_skip_continues() {
        let mut chain = TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo),
                TaskStep::new(StepKind::Construct),
            ],
            FailurePolicy::SkipStep,
        );

        let cont = chain.fail_current("optional step failed".into());
        assert!(cont);
        assert_eq!(chain.current_step, 1);
        assert!(matches!(chain.steps[0].status, StepStatus::Failed(_)));

        // Complete second step normally
        chain.advance();
        assert!(chain.is_complete());
        // Has a failed step but chain completed
        assert!(chain.is_failed()); // still true — at least one step failed
        assert!(!chain.is_succeeded());
    }

    #[test]
    fn current_returns_none_when_complete() {
        let mut chain = simple_chain();
        chain.advance();
        chain.advance();
        chain.advance();
        assert!(chain.current().is_none());
    }

    #[test]
    fn step_builder_sets_fields() {
        let step = TaskStep::new(StepKind::MoveTo)
            .with_position(Position::new(10, 20));
        assert_eq!(step.target_position, Some(Position::new(10, 20)));
        assert!(step.target_entity.is_none());
        assert!(matches!(step.status, StepStatus::Pending));
    }

    #[test]
    fn status_is_terminal() {
        assert!(!StepStatus::Pending.is_terminal());
        assert!(!StepStatus::InProgress { ticks_elapsed: 0 }.is_terminal());
        assert!(StepStatus::Succeeded.is_terminal());
        assert!(StepStatus::Failed("oops".into()).is_terminal());
    }
}
