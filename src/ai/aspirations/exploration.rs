//! Exploration domain — two chains (Mapmaker, Beyond the Border).
//! Ported 1:1 from `assets/narrative/aspirations/exploration.ron`
//! (retired at 321). #329 fills `emits` on every milestone.

use super::{
    always_true, AspirationChain, ConflictClass, Emit, Milestone, Priority, ProgressTracker,
};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// 329 — Exploration Primary emit. Routes to `explore_method`
/// (Tier-1 Live), which binds `Action::Explore`. Shared across both
/// Exploration chains because at combine-and-test land the chains
/// differ in milestone gating (tile-discovery vs unique-region) but
/// not in their primitive action. Per-row `applicable_when` is
/// `always_true`; a follow-on balance pass refines it with
/// tile-confidence / fatigue predicates.
const EXPLORATION_EMITS: &[Emit] = &[Emit {
    label: "explore_territory",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

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
            emits: EXPLORATION_EMITS,
            narrative_on_complete: "{name} wanders past the tree line and comes back different.",
        },
        Milestone {
            name: "Trail Finder",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 12,
            },
            emits: EXPLORATION_EMITS,
            narrative_on_complete: "{name} finds paths where others see only brambles.",
        },
        Milestone {
            name: "Cartographer",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 25,
            },
            emits: EXPLORATION_EMITS,
            narrative_on_complete: "{name} knows the land by scent, by shadow, by the angle of light.",
        },
        Milestone {
            name: "Mapmaker",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 40,
            },
            emits: EXPLORATION_EMITS,
            narrative_on_complete: "The colony's borders grow wherever {name} treads.",
        },
    ],
    completion_narrative: "{name} is the Mapmaker. The world is larger because {subject} walked it.",
    incompatible_with: &[],
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
            emits: EXPLORATION_EMITS,
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
            emits: EXPLORATION_EMITS,
            narrative_on_complete: "{name} has been places none of them have names for.",
        },
        Milestone {
            name: "Beyond the Border",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Explore],
                count: 35,
            },
            emits: EXPLORATION_EMITS,
            narrative_on_complete:
                "{name} returns from beyond the border with stories no one quite believes.",
        },
    ],
    completion_narrative:
        "{name} has gone Beyond the Border. The unknown is just another path to {object}.",
    // §7.7.1 canonical hard-identity pair. BEYOND_THE_BORDER narratives
    // are "always looks toward the horizon when others look toward the
    // den" — explicit absence. VOICE_OF_THE_COLONY requires colony
    // presence ("when {name} calls, the colony moves"). Identity-
    // incoherent (solitary-wanderer vs colony-coordinator, spec
    // verbatim).
    incompatible_with: &[("Voice of the Colony", ConflictClass::HardIdentity)],
};
