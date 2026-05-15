//! Building domain — two chains (Den Shaper, The Architect). Ported
//! 1:1 from `assets/narrative/aspirations/building.ron` (retired at
//! 321). #330 fills `emits` on every milestone.

use super::{
    always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker, SkillKind,
};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// 330 — Building Primary emit. Routes to `build_method` (Tier-1
/// Live), which binds `Action::Build`. Shared across both Building
/// chains because at combine-and-test land they differ only in
/// milestone gating (build-count thresholds vs skill-level checkpoints),
/// not in their primitive action. Per-row `applicable_when` is
/// `always_true`; a follow-on balance pass refines it with
/// site-claimed / materials-on-hand predicates.
const BUILDING_EMITS: &[Emit] = &[Emit {
    label: "construct",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

pub const DEN_SHAPER: AspirationChain = AspirationChain {
    name: "Den Shaper",
    domain: AspirationDomain::Building,
    milestones: &[
        Milestone {
            name: "First Wall",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Build],
                count: 3,
            },
            emits: BUILDING_EMITS,
            narrative_on_complete: "{name} sets the first stone and steps back to look.",
        },
        Milestone {
            name: "Steady Builder",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Build],
                count: 15,
            },
            emits: BUILDING_EMITS,
            narrative_on_complete: "{name}'s constructions stand through rain and wind.",
        },
        Milestone {
            name: "Den Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Building,
                level: 1.0,
            },
            emits: BUILDING_EMITS,
            narrative_on_complete: "{name} shapes wood and stone like clay.",
        },
    ],
    completion_narrative:
        "{name} is the Den Shaper. The colony stands stronger because {subject} built it.",
    incompatible_with: &[],
};

pub const THE_ARCHITECT: AspirationChain = AspirationChain {
    name: "The Architect",
    domain: AspirationDomain::Building,
    milestones: &[
        Milestone {
            name: "Blueprint Mind",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Build],
                count: 5,
            },
            emits: BUILDING_EMITS,
            narrative_on_complete: "{name} sees the finished structure before the first beam is laid.",
        },
        Milestone {
            name: "Master Carpenter",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Build],
                count: 25,
            },
            emits: BUILDING_EMITS,
            narrative_on_complete: "Others bring {name} their plans. {Subject} makes them possible.",
        },
        Milestone {
            name: "The Architect",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Building,
                level: 2.0,
            },
            emits: BUILDING_EMITS,
            narrative_on_complete: "{name}'s mark is on every wall that matters.",
        },
    ],
    completion_narrative: "{name} is The Architect. The colony is {possessive} monument.",
    incompatible_with: &[],
};
