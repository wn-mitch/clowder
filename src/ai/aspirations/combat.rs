//! Combat domain — two chains (Warrior's Path, Shadow Fighter).
//! Ported 1:1 from `assets/narrative/aspirations/combat.ron` (retired
//! at 321). Empty `emits` on every milestone — #327 fills them.

use super::{
    always_true, AspirationChain, ConflictClass, Milestone, ProgressTracker, SkillKind,
};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

pub const WARRIORS_PATH: AspirationChain = AspirationChain {
    name: "Warrior's Path",
    domain: AspirationDomain::Combat,
    milestones: &[
        Milestone {
            name: "Claws Out",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Fight],
                count: 1,
            },
            emits: &[],
            narrative_on_complete:
                "{name} bares {possessive} claws for the first time and means it.",
        },
        Milestone {
            name: "Scarred",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Fight],
                count: 10,
            },
            emits: &[],
            narrative_on_complete: "{name} wears {possessive} scars without shame.",
        },
        Milestone {
            name: "Battle-Tested",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Fight],
                count: 25,
            },
            emits: &[],
            narrative_on_complete:
                "The younger cats fall silent when {name} speaks of fighting.",
        },
        Milestone {
            name: "Warrior",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Fight],
                count: 50,
            },
            emits: &[],
            narrative_on_complete:
                "{name} has become the blade the colony reaches for in the dark.",
        },
    ],
    completion_narrative:
        "{name} has walked the Warrior's Path. Nothing enters the colony without {possessive} knowing.",
    // §7.7.1 canonical hard-logical pair. WARRIORS_PATH milestones
    // count `Action::Fight`; HEALERS_CALLING is fever-watches and
    // remedy-pressing. Simultaneous active state is coherence-
    // incoherent (warrior-path vs pacifist-mentor, spec verbatim).
    incompatible_with: &[("Healer's Calling", ConflictClass::HardLogical)],
};

pub const SHADOW_FIGHTER: AspirationChain = AspirationChain {
    name: "Shadow Fighter",
    domain: AspirationDomain::Combat,
    milestones: &[
        Milestone {
            name: "First Watch",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Patrol],
                count: 5,
            },
            emits: &[],
            narrative_on_complete:
                "{name} volunteers for the night patrol. No one asks why.",
        },
        Milestone {
            name: "Eyes in the Dark",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Patrol],
                count: 20,
            },
            emits: &[],
            narrative_on_complete: "{name} hears the threats before they arrive.",
        },
        Milestone {
            name: "Shadow Fighter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Combat,
                level: 1.5,
            },
            emits: &[],
            narrative_on_complete: "{name} fights like {possessive} shadow has claws of its own.",
        },
    ],
    completion_narrative: "They call {name} the Shadow Fighter. The borders have never been safer.",
    incompatible_with: &[],
};
