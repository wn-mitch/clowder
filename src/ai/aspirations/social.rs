//! Social domain — two chains (Heart of the Colony, The Beloved).
//! Ported 1:1 from `assets/narrative/aspirations/social.ron` (retired
//! at 321). #326 fills `emits` on every milestone.

use super::{always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;
use crate::resources::relationships::BondType;

/// 326 — HEART_OF_THE_COLONY Primary emit. Routes to `socialize_method`
/// (Tier-1 Live), which binds `Action::Socialize` against the Socialize-
/// DSE's existing target picker. Shared across all four
/// HEART_OF_THE_COLONY milestones because at combine-and-test land they
/// differ only in milestone gating (`FormBond { Friends }` vs
/// `ActionCount { Socialize }` thresholds), not in their primitive
/// action. Per-row `applicable_when` is `always_true`; a follow-on
/// balance pass refines it with bond-state / nearby-cat predicates.
const HEART_OF_THE_COLONY_EMITS: &[Emit] = &[Emit {
    label: "socialize",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// 326 — THE_BELOVED Primary emit. Routes to `groom_other_method`
/// (Tier-1 Live), which binds `Action::GroomOther` against the
/// GroomOther-DSE's existing target picker. Shared across all three
/// THE_BELOVED milestones because at combine-and-test land they differ
/// only in milestone gating (`ActionCount { GroomOther }` vs `FormBond
/// { Partners }`), not in their primitive action. Per-row
/// `applicable_when` is `always_true`; a follow-on balance pass refines
/// it with kin / partner / grooming-deficit predicates.
const THE_BELOVED_EMITS: &[Emit] = &[Emit {
    label: "groom_other",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

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
            emits: HEART_OF_THE_COLONY_EMITS,
            narrative_on_complete:
                "{name} has found a friend. The world feels a little less large.",
        },
        Milestone {
            name: "Trusted Ear",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 15,
            },
            emits: HEART_OF_THE_COLONY_EMITS,
            narrative_on_complete: "Cats seek {name} out when the days are hard.",
        },
        Milestone {
            name: "Peacekeeper",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 30,
            },
            emits: HEART_OF_THE_COLONY_EMITS,
            narrative_on_complete: "When voices rise, {name}'s calm settles them.",
        },
        Milestone {
            name: "Heart of the Colony",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Socialize],
                count: 50,
            },
            emits: HEART_OF_THE_COLONY_EMITS,
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
            emits: THE_BELOVED_EMITS,
            narrative_on_complete: "{name} grooms with the gentleness of a parent.",
        },
        Milestone {
            name: "Steady Presence",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::GroomOther],
                count: 25,
            },
            emits: THE_BELOVED_EMITS,
            narrative_on_complete: "The kits gravitate to {name} without being called.",
        },
        Milestone {
            name: "The Beloved",
            gate: always_true,
            progress_tracker: ProgressTracker::FormBond {
                bond_type: BondType::Partners,
            },
            emits: THE_BELOVED_EMITS,
            narrative_on_complete: "When {name} enters a room, worry leaves it.",
        },
    ],
    completion_narrative:
        "{name} is The Beloved. The colony would be a colder place without {object}.",
    incompatible_with: &[],
};
