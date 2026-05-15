//! Leadership domain — two chains (Voice of the Colony, The Unifier).
//! Ported 1:1 from `assets/narrative/aspirations/leadership.ron`
//! (retired at 321). #331 fills `emits` on every milestone.

use super::{
    always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker,
};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// 331 — Leadership Primary emit. Routes to `coordinate_method`
/// (Tier-1 Live), which binds `Action::Coordinate`. Shared across
/// both Leadership chains because at combine-and-test land they
/// differ only in milestone gating (Coordinate-count for VOICE,
/// mixed Socialize/Coordinate/Mentor for UNIFIER), not in the
/// primitive action a Leadership-aspiring cat reaches for. Per-row
/// `applicable_when` is `always_true`; a follow-on balance pass
/// refines it with role-acceptance / mentor-relationships
/// predicates.
const LEADERSHIP_EMITS: &[Emit] = &[Emit {
    label: "direct_colony",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

pub const VOICE_OF_THE_COLONY: AspirationChain = AspirationChain {
    name: "Voice of the Colony",
    domain: AspirationDomain::Leadership,
    milestones: &[
        Milestone {
            name: "First Direction",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Coordinate],
                count: 3,
            },
            emits: LEADERSHIP_EMITS,
            narrative_on_complete: "{name} speaks and another cat listens. Something shifts.",
        },
        Milestone {
            name: "Trusted Voice",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Coordinate],
                count: 15,
            },
            emits: LEADERSHIP_EMITS,
            narrative_on_complete: "When {name} calls, the colony moves.",
        },
        Milestone {
            name: "Voice of the Colony",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Coordinate],
                count: 30,
            },
            emits: LEADERSHIP_EMITS,
            narrative_on_complete: "{name}'s word carries the weight of experience and trust.",
        },
    ],
    completion_narrative:
        "{name} is the Voice of the Colony. When {subject} speaks, the colony listens.",
    // Pair with BEYOND_THE_BORDER authored on that side (one-direction
    // authoring per §7.7.1; reverse-walk in `can_adopt` handles it).
    incompatible_with: &[],
};

pub const THE_UNIFIER: AspirationChain = AspirationChain {
    name: "The Unifier",
    domain: AspirationDomain::Leadership,
    milestones: &[
        Milestone {
            name: "Bridge Builder",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 10,
            },
            emits: LEADERSHIP_EMITS,
            narrative_on_complete: "{name} connects cats who would never have spoken otherwise.",
        },
        Milestone {
            name: "Arbiter",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Coordinate],
                count: 10,
            },
            emits: LEADERSHIP_EMITS,
            narrative_on_complete: "{name} settles disputes with a look. Fair, always fair.",
        },
        Milestone {
            name: "The Unifier",
            gate: always_true,
            progress_tracker: ProgressTracker::Mentor { count: 5 },
            emits: LEADERSHIP_EMITS,
            narrative_on_complete: "Under {name}'s guidance, the colony pulls in one direction.",
        },
    ],
    completion_narrative: "{name} is The Unifier. The colony is one because {subject} made it so.",
    incompatible_with: &[],
};
