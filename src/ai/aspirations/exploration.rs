//! Exploration domain — two chains (Mapmaker, Beyond the Border).
//! Ported 1:1 from `assets/narrative/aspirations/exploration.ron`
//! (retired at 321). Empty `emits` on every milestone — #329 fills them.

use super::{always_true, AspirationChain, Milestone, ProgressTracker};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

pub const MAPMAKER: AspirationChain = AspirationChain {
    name: "Mapmaker",
    domain: AspirationDomain::Exploration,
    milestones: &[
        Milestone {
            name: "First Steps",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 3,
            },
            emits: &[],
            narrative_on_complete: "{name} wanders past the tree line and comes back different.",
        },
        Milestone {
            name: "Trail Finder",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 12,
            },
            emits: &[],
            narrative_on_complete: "{name} finds paths where others see only brambles.",
        },
        Milestone {
            name: "Cartographer",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 25,
            },
            emits: &[],
            narrative_on_complete: "{name} knows the land by scent, by shadow, by the angle of light.",
        },
        Milestone {
            name: "Mapmaker",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 40,
            },
            emits: &[],
            narrative_on_complete: "The colony's borders grow wherever {name} treads.",
        },
    ],
    completion_narrative: "{name} is the Mapmaker. The world is larger because {subject} walked it.",
};

pub const BEYOND_THE_BORDER: AspirationChain = AspirationChain {
    name: "Beyond the Border",
    domain: AspirationDomain::Exploration,
    milestones: &[
        Milestone {
            name: "Restless Paws",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Wander],
                count: 10,
            },
            emits: &[],
            narrative_on_complete:
                "{name} always looks toward the horizon when others look toward the den.",
        },
        Milestone {
            name: "Far Walker",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 15,
            },
            emits: &[],
            narrative_on_complete: "{name} has been places none of them have names for.",
        },
        Milestone {
            name: "Beyond the Border",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 35,
            },
            emits: &[],
            narrative_on_complete:
                "{name} returns from beyond the border with stories no one quite believes.",
        },
    ],
    completion_narrative:
        "{name} has gone Beyond the Border. The unknown is just another path to {object}.",
};
