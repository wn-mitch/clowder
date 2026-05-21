//! Combat domain — two chains (Warrior's Path, Shadow Fighter).
//! Ported 1:1 from `assets/narrative/aspirations/combat.ron` (retired
//! at 321). #327 filled WARRIORS_PATH emits with `fight_method` /
//! `flee_method` Live wiring; #347 finishes the domain by filling
//! SHADOW_FIGHTER emits with `patrol_method` + re-used `flee_method`.

use super::{
    always_true, AspirationChain, ConflictClass, Emit, Milestone, Priority, ProgressTracker,
    SkillKind,
};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// WARRIORS_PATH emit table — applies uniformly to all four
/// milestones. Primary `engage_threat` catches `fight_method`; Tertiary
/// `flee_to_safety` catches `flee_method` as a survival fallback when
/// the picker's per-row `applicable_when` (currently `always_true`,
/// tightened in a follow-on balance pass) starts gating Fight on
/// threat-in-range and Flee on wounded.
const WARRIOR_EMITS: &[Emit] = &[
    Emit {
        label: "engage_threat",
        applicable_when: always_true,
        strategy: CommitmentStrategy::SingleMinded,
        priority: Priority::Primary,
    },
    Emit {
        label: "flee_to_safety",
        applicable_when: always_true,
        strategy: CommitmentStrategy::SingleMinded,
        priority: Priority::Tertiary,
    },
];

/// 347 — SHADOW_FIGHTER emit table — applies uniformly to all three
/// milestones. Primary `patrol_route` catches `patrol_method` (this
/// ticket); Tertiary `flee_to_safety` re-uses the already-Live
/// `flee_method` from 327 as the survival fallback — same survival
/// logic whether the Combat cat's track is Fight- or Patrol-based. The
/// per-row `applicable_when` gate is `always_true` at 347 land; a
/// follow-on balance pass tightens Patrol on a perimeter-unwatched
/// predicate and Flee on a wounded predicate.
const SHADOW_FIGHTER_EMITS: &[Emit] = &[
    Emit {
        label: "patrol_route",
        applicable_when: always_true,
        strategy: CommitmentStrategy::SingleMinded,
        priority: Priority::Primary,
    },
    Emit {
        label: "flee_to_safety",
        applicable_when: always_true,
        strategy: CommitmentStrategy::SingleMinded,
        priority: Priority::Tertiary,
    },
];

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
            emits: WARRIOR_EMITS,
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
            emits: WARRIOR_EMITS,
            narrative_on_complete: "{name} wears {possessive} scars without shame.",
        },
        Milestone {
            name: "Battle-Tested",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Fight],
                count: 25,
            },
            emits: WARRIOR_EMITS,
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
            emits: WARRIOR_EMITS,
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
            emits: SHADOW_FIGHTER_EMITS,
            narrative_on_complete: "{name} volunteers for the night patrol. No one asks why.",
        },
        Milestone {
            name: "Eyes in the Dark",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Patrol],
                count: 20,
            },
            emits: SHADOW_FIGHTER_EMITS,
            narrative_on_complete: "{name} hears the threats before they arrive.",
        },
        Milestone {
            name: "Shadow Fighter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Combat,
                level: 1.5,
            },
            emits: SHADOW_FIGHTER_EMITS,
            narrative_on_complete: "{name} fights like {possessive} shadow has claws of its own.",
        },
    ],
    completion_narrative: "They call {name} the Shadow Fighter. The borders have never been safer.",
    incompatible_with: &[],
};
