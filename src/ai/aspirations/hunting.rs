//! Hunting domain — two chains (Master of the Hunt, Provider of the
//! Colony). Ported 1:1 from `assets/narrative/aspirations/hunting.ron`
//! (retired at 321) with one combine-and-test addition: the
//! [`MASTER_OF_THE_HUNT`] chain's first milestone ("First Blood")
//! carries a single `Emit` row pointing at the `hunt_prey` label so
//! ticket 321's soak exercises the picker→L2-wrap→320-gate path
//! end-to-end. The remaining Hunting milestones land empty (`#325`
//! fills them).

use super::{always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// "First Blood" emits `hunt_prey` so the L2 wrap site replaces
/// `Intention::Activity { Idle }` with `Intention::Goal { hunt_prey }`
/// — the 320 HTN gate then catches the Goal shape and pushes the
/// `hunt_method` frame. Combine-and-test slice; #325 will replace the
/// `always_true` applicability check with a prey-in-range belief
/// predicate.
const FIRST_BLOOD_EMITS: &[Emit] = &[Emit {
    label: "hunt_prey",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

pub const MASTER_OF_THE_HUNT: AspirationChain = AspirationChain {
    name: "Master of the Hunt",
    domain: AspirationDomain::Hunting,
    milestones: &[
        Milestone {
            name: "First Blood",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 1,
            },
            emits: FIRST_BLOOD_EMITS,
            narrative_on_complete: "{name} catches {possessive} first prey. A hunter is born.",
        },
        Milestone {
            name: "Keen Eye",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 10,
            },
            emits: &[],
            narrative_on_complete: "{name} reads the undergrowth like a story now.",
        },
        Milestone {
            name: "Shadow Stalker",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 25,
            },
            emits: &[],
            narrative_on_complete: "Prey doesn't hear {name} coming anymore.",
        },
        Milestone {
            name: "Master of the Hunt",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 50,
            },
            emits: &[],
            narrative_on_complete:
                "They will tell stories of {name}'s hunts long after {subject} is gone.",
        },
    ],
    completion_narrative:
        "{name} has walked the full path of the hunt. The Leaping Flame blazes bright.",
    incompatible_with: &[],
};

pub const PROVIDER_OF_THE_COLONY: AspirationChain = AspirationChain {
    name: "Provider of the Colony",
    domain: AspirationDomain::Hunting,
    milestones: &[
        Milestone {
            name: "Shared Catch",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 5,
            },
            emits: &[],
            narrative_on_complete: "{name} drops a mouse at the colony stores without a word.",
        },
        Milestone {
            name: "Reliable Paws",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 20,
            },
            emits: &[],
            narrative_on_complete: "When bellies rumble, eyes turn to {name}.",
        },
        Milestone {
            name: "Provider",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 40,
            },
            emits: &[],
            narrative_on_complete: "The colony has never gone hungry while {name} hunts.",
        },
    ],
    completion_narrative: "{name} is the Provider -- the one who feeds the many.",
    incompatible_with: &[],
};
