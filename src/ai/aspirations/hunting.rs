//! Hunting domain — two chains (Master of the Hunt, Provider of the
//! Colony). Ported 1:1 from `assets/narrative/aspirations/hunting.ron`
//! (retired at 321) with one combine-and-test addition: the
//! [`MASTER_OF_THE_HUNT`] chain's first milestone ("First Blood")
//! carries a single `Emit` row pointing at the `hunt_prey` label so
//! ticket 321's soak exercises the picker→L2-wrap→320-gate path
//! end-to-end. #325 filled the remaining milestones: every milestone
//! past First Blood emits `hunt_prey` (Primary), and the
//! Master-of-the-Hunt chain's two skill-progression milestones
//! additionally emit `stealth_gear_acquired` (Secondary) — the
//! worked-example stealth-cloak arc, Live via 334's self-craft method.

use super::{always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// "First Blood" emits `hunt_prey` so the L2 wrap site replaces
/// `Intention::Activity { Idle }` with `Intention::Goal { hunt_prey }`
/// — the 320 HTN gate then catches the Goal shape and pushes the
/// `hunt_method` frame. Combine-and-test slice; a follow-on balance
/// pass replaces the `always_true` applicability check with a
/// prey-in-range belief predicate.
const FIRST_BLOOD_EMITS: &[Emit] = &[Emit {
    label: "hunt_prey",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// 325 — Hunting Primary emit, shared by every milestone past First
/// Blood. Routes to `hunt_method` (Live, 321), binding `Action::Hunt`.
/// Byte-identical to [`FIRST_BLOOD_EMITS`]; kept separate only to
/// preserve that const's 321 combine-and-test provenance.
const HUNT_EMITS: &[Emit] = &[Emit {
    label: "hunt_prey",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// 325 — mastery-tier emit: Primary hunt + Secondary stealth-gear
/// acquisition. The Secondary catches `acquire_stealth_via_self_craft`
/// (Live as of 334; its own `can_self_craft_stealth` predicate gates
/// lacks-cloak ∧ can-craft, so the `always_true` emit gate here is not
/// redundant-harmful) and falls through to the dormant commission
/// sibling (#481) when self-craft is inapplicable. Applied to the
/// Master-of-the-Hunt chain's two skill-progression milestones — the
/// worked-example stealth-cloak arc (`docs/systems/htn-methods.md`
/// §Worked example). A follow-on balance pass tightens the per-row
/// `applicable_when` predicates.
const HUNT_MASTERY_EMITS: &[Emit] = &[
    Emit {
        label: "hunt_prey",
        applicable_when: always_true,
        strategy: CommitmentStrategy::SingleMinded,
        priority: Priority::Primary,
    },
    Emit {
        label: "stealth_gear_acquired",
        applicable_when: always_true,
        strategy: CommitmentStrategy::SingleMinded,
        priority: Priority::Secondary,
    },
];

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
            emits: HUNT_EMITS,
            narrative_on_complete: "{name} reads the undergrowth like a story now.",
        },
        Milestone {
            name: "Shadow Stalker",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 25,
            },
            emits: HUNT_MASTERY_EMITS,
            narrative_on_complete: "Prey doesn't hear {name} coming anymore.",
        },
        Milestone {
            name: "Master of the Hunt",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 50,
            },
            emits: HUNT_MASTERY_EMITS,
            narrative_on_complete:
                "They will tell stories of {name}'s hunts long after {subject} is gone.",
        },
    ],
    completion_narrative:
        "{name} has walked the full path of the hunt. The Leaping Flame blazes bright.",
    incompatible_with: &[],
    expected_valence_target: 0.30,
};

/// Ethological colony-start: the caching/gathering-provider emit. Reuses
/// the `hunt_prey` label (Live method, Hunting domain) per the v1
/// "reuse an existing method label" path — the Hunting domain's
/// `matching_actions` already spans `[Hunt, Forage]`, so a cat holding this
/// chain gets its Forage (surplus-cache) score lifted through the same
/// `AspirationLift` path Provider-of-the-Colony uses. A follow-on may mint a
/// dedicated `cache_surplus` method + `Provisioning` domain if PickUp needs
/// its own domain-lift; for v1 the caching bias rides Forage.
const CACHE_EMITS: &[Emit] = &[Emit {
    label: "hunt_prey",
    applicable_when: always_true,
    strategy: CommitmentStrategy::SingleMinded,
    priority: Priority::Primary,
}];

/// Ethological colony-start: the forager-cacher identity arc — the cat who
/// gathers the windfall and stocks the larder, distinct from
/// [`PROVIDER_OF_THE_COLONY`]'s hunter-provider. Progress tracks the caching
/// actions (Forage + PickUp) rather than Hunt; same Hunting domain so the
/// Forage surplus-cache axis is lifted. Milestone narratives are the
/// mythic-texture payoff (a named "Keeper of the Larder" per healthy sim
/// year contributes to the mythic-texture continuity canary).
pub const KEEPER_OF_THE_LARDER: AspirationChain = AspirationChain {
    name: "Keeper of the Larder",
    domain: AspirationDomain::Hunting,
    milestones: &[
        Milestone {
            name: "First Cache",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Forage, Action::PickUp],
                count: 5,
            },
            emits: CACHE_EMITS,
            narrative_on_complete: "{name} tucks the first windfall into the stores, satisfied.",
        },
        Milestone {
            name: "Full Shelves",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Forage, Action::PickUp],
                count: 20,
            },
            emits: CACHE_EMITS,
            narrative_on_complete: "Nothing edible escapes {name}'s notice for long.",
        },
        Milestone {
            name: "Keeper of the Larder",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Forage, Action::PickUp],
                count: 40,
            },
            emits: CACHE_EMITS,
            narrative_on_complete: "The stores are never bare while {name} keeps them.",
        },
    ],
    completion_narrative:
        "{name} is the Keeper of the Larder -- the colony eats through winter because {subject} gathered.",
    incompatible_with: &[],
    expected_valence_target: 0.20,
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
            emits: HUNT_EMITS,
            narrative_on_complete: "{name} drops a mouse at the colony stores without a word.",
        },
        Milestone {
            name: "Reliable Paws",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 20,
            },
            emits: HUNT_EMITS,
            narrative_on_complete: "When bellies rumble, eyes turn to {name}.",
        },
        Milestone {
            name: "Provider",
            gate: always_true,
            progress_tracker: ProgressTracker::ActionCount {
                actions: &[Action::Hunt],
                count: 40,
            },
            emits: HUNT_EMITS,
            narrative_on_complete: "The colony has never gone hungry while {name} hunts.",
        },
    ],
    completion_narrative: "{name} is the Provider -- the one who feeds the many.",
    incompatible_with: &[],
    expected_valence_target: 0.20,
};
