//! Social domain — two chains (Heart of the Colony, The Beloved).
//! Ported 1:1 from `assets/narrative/aspirations/social.ron` (retired
//! at 321). Empty `emits` on every milestone — #326 fills them.

use super::{always_true, AspirationChain, Milestone, ProgressTracker};
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;
use crate::resources::relationships::BondType;

pub const HEART_OF_THE_COLONY: AspirationChain = AspirationChain {
    name: "Heart of the Colony",
    domain: AspirationDomain::Social,
    milestones: &[
        Milestone {
            name: "First Friend",
            gate: always_true,
            progress_tracker: ProgressTracker::FormBond {
                bond_type: BondType::Friends,
            },
            emits: &[],
            narrative_on_complete: "{name} has found a friend. The world feels a little less large.",
        },
        Milestone {
            name: "Trusted Ear",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 15,
            },
            emits: &[],
            narrative_on_complete: "Cats seek {name} out when the days are hard.",
        },
        Milestone {
            name: "Peacekeeper",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 30,
            },
            emits: &[],
            narrative_on_complete: "When voices rise, {name}'s calm settles them.",
        },
        Milestone {
            name: "Heart of the Colony",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 50,
            },
            emits: &[],
            narrative_on_complete: "The colony breathes easier when {name} is near.",
        },
    ],
    completion_narrative:
        "{name} has become the Heart of the Colony. Everyone knows {possessive} warmth.",
    incompatible_with: &[],
};

pub const THE_BELOVED: AspirationChain = AspirationChain {
    name: "The Beloved",
    domain: AspirationDomain::Social,
    milestones: &[
        Milestone {
            name: "Gentle Touch",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::GroomOther],
                count: 10,
            },
            emits: &[],
            narrative_on_complete: "{name} grooms with the gentleness of a parent.",
        },
        Milestone {
            name: "Steady Presence",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::GroomOther],
                count: 25,
            },
            emits: &[],
            narrative_on_complete: "The kits gravitate to {name} without being called.",
        },
        Milestone {
            name: "The Beloved",
            gate: always_true,
            progress_tracker: ProgressTracker::FormBond {
                bond_type: BondType::Partners,
            },
            emits: &[],
            narrative_on_complete: "When {name} enters a room, worry leaves it.",
        },
    ],
    completion_narrative:
        "{name} is The Beloved. The colony would be a colder place without {object}.",
    incompatible_with: &[],
};
