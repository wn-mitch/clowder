use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::ai::planner::{GoapActionKind, PlannedStep};
use crate::components::disposition::DispositionKind;
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
    /// The exact L3 sub-action the softmax picked. Threaded from
    /// scoring to the executor so `GoapActionKind::to_action` can
    /// label `CurrentAction.action` accurately mid-chain.
    ///
    /// 155: replaces the retired `crafting_hint: Option<CraftingHint>`
    /// field. Carries the sub-mode for the new Herbalism / Witchcraft
    /// dispositions; for single-constituent dispositions it equals the
    /// disposition's only constituent action.
    pub chosen_action: Action,
    /// Per-step execution state. Parallel to `steps` — initialized when each
    /// step begins executing.
    #[serde(skip)]
    pub step_state: Vec<StepExecutionState>,
    /// Target position for ward placement (from coordinator directive).
    #[serde(skip)]
    pub ward_placement_pos: Option<Position>,
    /// Action kinds that failed during this plan's lifetime. Filtered out
    /// during replanning to avoid regenerating identical impossible plans.
    #[serde(skip, default)]
    pub failed_actions: HashSet<GoapActionKind>,
}

// ---------------------------------------------------------------------------
// AbandonReason / AbandonedPlanState — `plan_substrate::abandon_plan` shapes
// ---------------------------------------------------------------------------

/// Why a plan is being abandoned. Used by `plan_substrate::abandon_plan`
/// (072) to classify the abandonment for §7.2 commitment-drop branches
/// and downstream `RecentTargetFailures` accounting (073).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbandonReason {
    /// `replan_count` exceeded `max_replans` after a step failure.
    ReplanCap,
    /// Replanning produced no plan (planner returned `None`).
    NoPlanPossible,
    /// Caller-driven abandonment (e.g., preempt cleanup); 072 does not
    /// route through this variant today but the API accepts it for
    /// future use by 075/076.
    External,
}

/// Snapshot of cross-plan memory the caller may want to preserve after
/// `plan_substrate::abandon_plan` consumes the abandoning plan. 072
/// returns an empty struct (the inline call sites carry no cross-plan
/// state forward); 073 extends this with the `failed_actions` set so
/// per-cat target-failure memory persists across replans.
#[derive(Debug, Clone, Default)]
pub struct AbandonedPlanState;

impl GoapPlan {
    /// Maximum replans allowed before a plan is abandoned.
    pub const DEFAULT_MAX_REPLANS: u32 = 3;

    pub fn new(
        kind: DispositionKind,
        chosen_action: Action,
        tick: u64,
        personality: &Personality,
        steps: Vec<PlannedStep>,
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
            chosen_action,
            step_state: vec![StepExecutionState::default(); step_count],
            ward_placement_pos: None,
            failed_actions: HashSet::new(),
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
// PlanFailureReason — categorized reason a plan step failed
// ---------------------------------------------------------------------------

/// Categorical reason a plan step failed. Used by the `plan_substrate`
/// API (`record_step_failure`, `abandon_plan`) to classify failures so
/// downstream tickets (073 — `RecentTargetFailures`, 074 —
/// `EligibilityFilter::require_alive`) can react differently per kind.
///
/// 072 introduces the enum; only `TargetDespawned` is referenced by
/// downstream tickets today (074 uses it for dead-target failures).
/// `Other` is the catch-all the existing inline call sites map to,
/// preserving behavior in 072.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanFailureReason {
    /// The step's target entity has been despawned. Used by 074 when
    /// `validate_target` rejects a dead entity at step entry.
    TargetDespawned,
    /// Catch-all for non-target-related step failures (timeout,
    /// engagement-loss, etc.). 072 routes every existing inline failure
    /// site through this variant — finer classification lands later.
    Other,
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
    /// Manhattan distance between cat and target at attempt start, captured
    /// on the first tick a target-bound step (e.g. EngagePrey) actually runs.
    /// Ticket 149 — used to bin discrete-attempt success by approach
    /// difficulty in the `HuntAttempt` event payload. `None` until first
    /// observation; remains `Some` for the lifetime of this `StepExecutionState`.
    #[serde(default)]
    pub attempt_start_distance: Option<i32>,
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
// PendingUrgencies — step-boundary need evaluation (replaces per-tick interrupts)
// ---------------------------------------------------------------------------

/// Categories of urgency that accumulate between step boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrgencyKind {
    /// Hunger below critical threshold.
    Starvation,
    /// Energy below critical threshold.
    Exhaustion,
    /// Safety below critical threshold.
    CriticalSafety,
    /// Predator detected nearby (contextual intensity).
    ThreatNearby,
}

/// A single pending urgency with Maslow priority and intensity.
#[derive(Debug, Clone)]
pub struct UrgentNeed {
    pub kind: UrgencyKind,
    /// Maslow level: 1 = physiological, 2 = safety.
    pub maslow_level: u8,
    /// How severe this urgency is (0.0–1.0).
    pub intensity: f32,
    /// For ThreatNearby: position to flee away from.
    pub threat_pos: Option<Position>,
}

/// Urgencies accumulated each tick and evaluated at step boundaries.
/// Replaces per-tick interrupts for all non-damage conditions.
#[derive(Component, Debug, Clone, Default)]
pub struct PendingUrgencies {
    pub needs: Vec<UrgentNeed>,
}

impl PendingUrgencies {
    /// Returns the highest-priority urgency (lowest maslow_level, then highest
    /// intensity). Returns None if empty.
    pub fn highest(&self) -> Option<&UrgentNeed> {
        self.needs.iter().min_by(|a, b| {
            a.maslow_level.cmp(&b.maslow_level).then_with(|| {
                b.intensity
                    .partial_cmp(&a.intensity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
    }
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
    ///
    /// 155: `chosen_action` is the L3-picked sub-action carried on
    /// `GoapPlan::chosen_action`. For Herbalism / Witchcraft chains
    /// where multiple GoapActionKind steps share a sub-action label
    /// (e.g., `GatherHerb` is reused by all three Herbalism sub-modes),
    /// this disambiguates correctly: `GatherHerb` mid-chain reports
    /// the chain's Action label rather than collapsing to a single
    /// `Herbcraft` umbrella.
    pub fn to_action(self, disposition: DispositionKind, chosen_action: Action) -> Action {
        match self {
            Self::TravelTo(_) => match disposition {
                DispositionKind::Hunting => Action::Hunt,
                DispositionKind::Foraging => Action::Forage,
                DispositionKind::Guarding => Action::Patrol,
                DispositionKind::Building => Action::Build,
                DispositionKind::Farming => Action::Farm,
                DispositionKind::Exploring => Action::Explore,
                // 155: TravelTo legs in Herbalism / Witchcraft / Cooking
                // chains report the chain's chosen sub-action so
                // CurrentAction stays stable through the chain.
                DispositionKind::Herbalism
                | DispositionKind::Witchcraft
                | DispositionKind::Cooking => chosen_action,
                _ => Action::Wander,
            },
            Self::SearchPrey | Self::EngagePrey | Self::DepositPrey => Action::Hunt,
            Self::ForageItem | Self::DepositFood => Action::Forage,
            Self::EatAtStores => Action::Eat,
            Self::Sleep => Action::Sleep,
            Self::SelfGroom => Action::GroomSelf,
            Self::GroomOther => Action::GroomOther,
            Self::PatrolArea | Self::Survey => Action::Patrol,
            Self::EngageThreat => Action::Fight,
            Self::SocializeWith => Action::Socialize,
            Self::MentorCat => Action::Mentor,
            Self::GatherMaterials | Self::DeliverMaterials | Self::Construct => Action::Build,
            Self::TendCrops | Self::HarvestCrops => Action::Farm,
            // 155: Herbcraft sub-actions split — each step inherits the
            // chosen sub-action label so a `GatherHerb` step in a
            // `HerbcraftRemedy` chain reports `HerbcraftRemedy`, not
            // `HerbcraftGather`.
            Self::GatherHerb | Self::PrepareRemedy | Self::ApplyRemedy => chosen_action,
            // SetWard is shared between Herbalism (HerbcraftSetWard) and
            // Witchcraft (MagicDurableWard); the chosen_action carries
            // the disambiguation.
            Self::SetWard => chosen_action,
            // 155: Magic sub-actions split — each Witchcraft step
            // inherits the chosen sub-action label.
            Self::Scry => chosen_action,
            Self::SpiritCommunion => chosen_action,
            Self::CleanseCorruption => chosen_action,
            Self::HarvestCarcass => chosen_action,
            Self::MateWith => Action::Mate,
            Self::FeedKitten | Self::RetrieveFoodForKitten => Action::Caretake,
            Self::DeliverDirective => Action::Coordinate,
            Self::ExploreSurvey => Action::Explore,
            Self::RetrieveRawFood | Self::Cook | Self::DepositCookedFood => Action::Cook,
            // 176: inventory-disposal GOAP actions map directly to
            // their parent L3 Action labels.
            Self::DropItem => Action::Drop,
            Self::TrashItemAtMidden => Action::Trash,
            Self::HandoffItem => Action::Handoff,
            Self::PickUpItemFromGround => Action::PickUp,
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
        let plan = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 100, &p, sample_steps());
        assert_eq!(
            plan.target_trips,
            DispositionKind::Hunting.target_completions(&p)
        );
        assert_eq!(plan.trips_done, 0);
        assert_eq!(plan.adopted_tick, 100);
        assert_eq!(plan.replan_count, 0);
    }

    #[test]
    fn advance_increments_step() {
        let p = test_personality();
        let mut plan = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 0, &p, sample_steps());
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
        let mut plan = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 0, &p, sample_steps());
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
        let mut plan = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 0, &p, sample_steps());
        plan.max_replans = 2;

        assert!(plan.replan(sample_steps())); // replan_count = 1
        assert!(plan.replan(sample_steps())); // replan_count = 2
        assert!(!plan.replan(sample_steps())); // exceeds max
    }

    #[test]
    fn step_state_parallel_to_steps() {
        let p = test_personality();
        let plan = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 0, &p, sample_steps());
        assert_eq!(plan.steps.len(), plan.step_state.len());
    }

    #[test]
    fn failed_actions_persist_across_replans() {
        let p = test_personality();
        let mut plan = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 0, &p, sample_steps());
        assert!(plan.failed_actions.is_empty());

        plan.failed_actions.insert(GoapActionKind::SearchPrey);
        assert!(plan.replan(sample_steps()));
        // failed_actions should persist after replan.
        assert!(plan.failed_actions.contains(&GoapActionKind::SearchPrey));
        assert_eq!(plan.replan_count, 1);

        // A fresh plan starts with empty failed_actions.
        let fresh = GoapPlan::new(DispositionKind::Hunting, Action::Hunt, 100, &p, sample_steps());
        assert!(fresh.failed_actions.is_empty());
    }

    #[test]
    fn action_kind_to_action_mapping() {
        assert_eq!(
            GoapActionKind::SearchPrey.to_action(DispositionKind::Hunting, Action::Hunt),
            Action::Hunt
        );
        assert_eq!(
            GoapActionKind::EatAtStores.to_action(DispositionKind::Eating, Action::Eat),
            Action::Eat
        );
        assert_eq!(
            GoapActionKind::TravelTo(PlannerZone::Stores)
                .to_action(DispositionKind::Hunting, Action::Hunt),
            Action::Hunt
        );
        assert_eq!(
            GoapActionKind::TravelTo(PlannerZone::Stores)
                .to_action(DispositionKind::Resting, Action::Sleep),
            Action::Wander
        );
    }
}
