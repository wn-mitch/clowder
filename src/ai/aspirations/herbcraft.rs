//! Herbcraft domain — two chains (Whiskerweaver's Apprentice, Healer's
//! Calling). Ported 1:1 from `assets/narrative/aspirations/herbcraft.ron`
//! (retired at 321). #328 fills `emits` on every milestone.
//!
//! Pre-321 the RON `ActionCount(action: "Herbcraft")` reference never
//! matched any `Action` variant (ticket 155 fanned `Action::Herbcraft`
//! into `HerbcraftGather`/`Remedy`/`SetWard` and the RON was never
//! refreshed). The migration captures the authorial intent by listing
//! all three post-155 sub-actions in the slice — any herbcraft work
//! counts toward the milestone. This is a behavior fix landed with
//! 321; the affected milestones now make forward progress where they
//! previously sat at zero.

use super::{always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker, SkillKind};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// All three post-155 herbcraft sub-actions. Used by the Herbcraft
/// `ProgressTracker::ActionCount` milestones — any of the three
/// counts toward progress, mirroring
/// `AspirationDomain::Herbcraft.matching_actions()`.
const HERBCRAFT_ACTIONS: &[Action] = &[
    Action::HerbcraftGather,
    Action::HerbcraftRemedy,
    Action::HerbcraftSetWard,
];

/// 328 — Apprentice Primary emit. Routes to `gather_herbs_method`
/// (Tier-1 Live), which binds `Action::HerbcraftGather`. The chain's
/// narrative metaphor — gathering as the apprentice's first discipline
/// — drives the label choice; chains share `AspirationDomain::Herbcraft`
/// so the picker's §H step-3 fallback can still find Live methods
/// across the chain split. Per-row `applicable_when` is `always_true`
/// at 328 land; a follow-on balance pass adds herb-in-range gating.
const WHISKERWEAVERS_EMITS: &[Emit] = &[Emit {
    label: "gather_herbs",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// 328 — Healer Primary emit. Routes to `prepare_remedy_method`
/// (Tier-1 Live), which binds `Action::HerbcraftRemedy`. Mirror of
/// `WHISKERWEAVERS_EMITS` with the chain's remedy-focused metaphor.
const HEALERS_EMITS: &[Emit] = &[Emit {
    label: "prepare_remedy",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

pub const WHISKERWEAVERS_APPRENTICE: AspirationChain = AspirationChain {
    name: "Whiskerweaver's Apprentice",
    domain: AspirationDomain::Herbcraft,
    milestones: &[
        Milestone {
            name: "First Gathering",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: HERBCRAFT_ACTIONS,
                count: 3,
            },
            emits: WHISKERWEAVERS_EMITS,
            narrative_on_complete: "{name} learns which leaves heal and which sting.",
        },
        Milestone {
            name: "Steady Paws",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: HERBCRAFT_ACTIONS,
                count: 15,
            },
            emits: WHISKERWEAVERS_EMITS,
            narrative_on_complete: "{name}'s paws no longer tremble over the mortar.",
        },
        Milestone {
            name: "Whiskerweaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Herbcraft,
                level: 1.0,
            },
            emits: WHISKERWEAVERS_EMITS,
            narrative_on_complete: "{name} weaves remedies from root and petal like breathing.",
        },
    ],
    completion_narrative:
        "{name} has earned the title Whiskerweaver. The colony's hurts mend faster for it.",
    incompatible_with: &[],
    expected_valence_target: 0.25,
};

pub const HEALERS_CALLING: AspirationChain = AspirationChain {
    name: "Healer's Calling",
    domain: AspirationDomain::Herbcraft,
    milestones: &[
        Milestone {
            name: "First Remedy",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: HERBCRAFT_ACTIONS,
                count: 5,
            },
            emits: HEALERS_EMITS,
            narrative_on_complete:
                "{name} presses moss to a wound and feels purpose settle in.",
        },
        Milestone {
            name: "Night Vigil",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: HERBCRAFT_ACTIONS,
                count: 25,
            },
            emits: HEALERS_EMITS,
            narrative_on_complete: "{name} stays through the fever-watches when others sleep.",
        },
        Milestone {
            name: "Healer",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Herbcraft,
                level: 1.5,
            },
            emits: HEALERS_EMITS,
            narrative_on_complete:
                "They bring the sick to {name} now, and {subject} sends them back whole.",
        },
    ],
    completion_narrative:
        "{name} has answered the Healer's Calling. {Subject} is the colony's remedy against the dark.",
    // Pair with WARRIORS_PATH authored on that side (one-direction
    // authoring per §7.7.1; reverse-walk in `can_adopt` handles it).
    incompatible_with: &[],
    expected_valence_target: 0.30,
};
