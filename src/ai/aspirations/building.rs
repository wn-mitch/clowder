//! Building domain — two chains (Den Shaper, The Architect). Ported
//! 1:1 from `assets/narrative/aspirations/building.ron` (retired at
//! 321). Empty `emits` on every milestone — #330 fills them.

use super::{always_true, AspirationChain, Milestone, ProgressTracker, SkillKind};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

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
            emits: &[],
            narrative_on_complete: "{name} sets the first stone and steps back to look.",
        },
        Milestone {
            name: "Steady Builder",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Build],
                count: 15,
            },
            emits: &[],
            narrative_on_complete: "{name}'s constructions stand through rain and wind.",
        },
        Milestone {
            name: "Den Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Building,
                level: 1.0,
            },
            emits: &[],
            narrative_on_complete: "{name} shapes wood and stone like clay.",
        },
    ],
    completion_narrative:
        "{name} is the Den Shaper. The colony stands stronger because {subject} built it.",
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
            emits: &[],
            narrative_on_complete: "{name} sees the finished structure before the first beam is laid.",
        },
        Milestone {
            name: "Master Carpenter",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Build],
                count: 25,
            },
            emits: &[],
            narrative_on_complete: "Others bring {name} their plans. {Subject} makes them possible.",
        },
        Milestone {
            name: "The Architect",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Building,
                level: 2.0,
            },
            emits: &[],
            narrative_on_complete: "{name}'s mark is on every wall that matters.",
        },
    ],
    completion_narrative: "{name} is The Architect. The colony is {possessive} monument.",
};
