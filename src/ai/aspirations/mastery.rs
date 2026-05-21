//! Mastery domain — 016 Phase 5 precursor (366). Five mastery arcs
//! that gate Phase 5 crafting recipes once 372 wires the discipline
//! actions + skill writers.
//!
//! Per `docs/systems/crafting.md` §Phase 5: each elevated recipe
//! requires at least one cat in the colony to have *practiced enough*
//! on a relevant discipline. Mastery is a **latent colony enabler**,
//! never a per-cast bonus and never an in-fiction artisan rank.
//!
//! # Adoption is event-driven, not passive
//!
//! These arcs are registered in `ALL_CHAINS` but skipped by both
//! `select_aspirations` and `check_second_aspiration_slot` —
//! identical to the [`super::kinship`] (398) pattern. The skip lives
//! BEFORE the `rng.random_range` call inside `score_chain`, so
//! registering five new chains does NOT perturb seed-42 determinism
//! (per `learning_bevy_schedule_edge_perturbation`). Event-driven
//! adoption wiring — adopt on first relevant craft action,
//! analogous to Kinship's post-partum trigger — lands in 372 as part
//! of the discipline-action substrate. Until then the chains are
//! discoverable via [`super::ALL_CHAINS`] + `chain_by_name` but never
//! held by a cat.
//!
//! # Tier structure
//!
//! Each arc is a 6-tier guild ladder:
//!
//! | Tier        | Threshold |
//! |-------------|-----------|
//! | Novice      | 0.0       |
//! | Apprentice  | 0.2       |
//! | Journeyman  | 0.4       |
//! | Adept       | 0.6       |
//! | Master      | 0.8       |
//! | Paragon     | 0.95      |
//!
//! Novice (0.0) fires on the first `track_milestones` tick after
//! adoption — the narrative beat is "began the practice." The
//! remaining five milestones require Phase 5 craft actions (372) to
//! grant the matching skill XP.
//!
//! `emits` tables ship empty. A 327-series follow-on (mirroring the
//! 325 Hunting wrapper) fills them once the L2 craft-method registry
//! lands.

use super::{always_true, AspirationChain, Milestone, ProgressTracker, SkillKind};
use crate::components::aspirations::AspirationDomain;

pub const WEAVING_MASTERY: AspirationChain = AspirationChain {
    name: "Weaving Mastery",
    domain: AspirationDomain::Weaving,
    milestones: &[
        Milestone {
            name: "Novice Weaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Weaving,
                level: 0.0,
            },
            emits: &[],
            narrative_on_complete: "{name} takes up the first plait. The work has begun.",
        },
        Milestone {
            name: "Apprentice Weaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Weaving,
                level: 0.2,
            },
            emits: &[],
            narrative_on_complete:
                "{name} ties {possessive} first knot that holds without prompting.",
        },
        Milestone {
            name: "Journeyman Weaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Weaving,
                level: 0.4,
            },
            emits: &[],
            narrative_on_complete: "{name}'s rows run straight. Other cats borrow {possessive} hands.",
        },
        Milestone {
            name: "Adept Weaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Weaving,
                level: 0.6,
            },
            emits: &[],
            narrative_on_complete:
                "{name} works without watching the weave; the fiber answers to {possessive} paws.",
        },
        Milestone {
            name: "Master Weaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Weaving,
                level: 0.8,
            },
            emits: &[],
            narrative_on_complete:
                "{name} reads fiber like other cats read scent -- every thread tells.",
        },
        Milestone {
            name: "Paragon Weaver",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Weaving,
                level: 0.95,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s weave is unmistakable. The colony's seasons hang in {possessive} work.",
        },
    ],
    completion_narrative:
        "{name} is the Paragon Weaver. There is nothing fiber can do that {subject} cannot teach it.",
    incompatible_with: &[],
};

pub const BONE_SHAPING_MASTERY: AspirationChain = AspirationChain {
    name: "Bone-Shaping Mastery",
    domain: AspirationDomain::BoneShaping,
    milestones: &[
        Milestone {
            name: "Novice Bone-Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::BoneShaping,
                level: 0.0,
            },
            emits: &[],
            narrative_on_complete:
                "{name} sets the first bone to the grinding stone. The work has begun.",
        },
        Milestone {
            name: "Apprentice Bone-Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::BoneShaping,
                level: 0.2,
            },
            emits: &[],
            narrative_on_complete: "{name} files {possessive} first true needle from rib-shard.",
        },
        Milestone {
            name: "Journeyman Bone-Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::BoneShaping,
                level: 0.4,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s combs and toggles pass between paws across the colony.",
        },
        Milestone {
            name: "Adept Bone-Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::BoneShaping,
                level: 0.6,
            },
            emits: &[],
            narrative_on_complete:
                "{name} reads grain in every shard -- where it will hold, where it will break.",
        },
        Milestone {
            name: "Master Bone-Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::BoneShaping,
                level: 0.8,
            },
            emits: &[],
            narrative_on_complete:
                "{name} can find the spear inside the deer-thigh before the first cut.",
        },
        Milestone {
            name: "Paragon Bone-Shaper",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::BoneShaping,
                level: 0.95,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s work outlasts the cats who carried it. Bone remembers {possessive} paws.",
        },
    ],
    completion_narrative:
        "{name} is the Paragon Bone-Shaper. The dead give up their last form for {object}.",
    incompatible_with: &[],
};

pub const HIDEWORK_MASTERY: AspirationChain = AspirationChain {
    name: "Hidework Mastery",
    domain: AspirationDomain::Hidework,
    milestones: &[
        Milestone {
            name: "Novice Hideworker",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Hidework,
                level: 0.0,
            },
            emits: &[],
            narrative_on_complete: "{name} pegs out {possessive} first hide. The work has begun.",
        },
        Milestone {
            name: "Apprentice Hideworker",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Hidework,
                level: 0.2,
            },
            emits: &[],
            narrative_on_complete: "{name} cures a pelt that does not stiffen at the first frost.",
        },
        Milestone {
            name: "Journeyman Hideworker",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Hidework,
                level: 0.4,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s bracers and pouches are sought before {possessive} own kin.",
        },
        Milestone {
            name: "Adept Hideworker",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Hidework,
                level: 0.6,
            },
            emits: &[],
            narrative_on_complete:
                "{name} can read a fresh hide and name what it will best become.",
        },
        Milestone {
            name: "Master Hideworker",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Hidework,
                level: 0.8,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s leather softens under the paw and stiffens against the blow.",
        },
        Milestone {
            name: "Paragon Hideworker",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Hidework,
                level: 0.95,
            },
            emits: &[],
            narrative_on_complete:
                "{name} wears the work of {possessive} own paws into the long seasons.",
        },
    ],
    completion_narrative:
        "{name} is the Paragon Hideworker. Every cured hide in the colony bears {possessive} mark.",
    incompatible_with: &[],
};

pub const PIGMENT_MASTERY: AspirationChain = AspirationChain {
    name: "Pigment Mastery",
    domain: AspirationDomain::Pigment,
    milestones: &[
        Milestone {
            name: "Novice Pigmenter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Pigment,
                level: 0.0,
            },
            emits: &[],
            narrative_on_complete:
                "{name} grinds {possessive} first mark from berry and ash. The work has begun.",
        },
        Milestone {
            name: "Apprentice Pigmenter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Pigment,
                level: 0.2,
            },
            emits: &[],
            narrative_on_complete:
                "{name} mixes a color that holds against a wash of rain.",
        },
        Milestone {
            name: "Journeyman Pigmenter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Pigment,
                level: 0.4,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s dyes are recognized across the colony's textiles.",
        },
        Milestone {
            name: "Adept Pigmenter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Pigment,
                level: 0.6,
            },
            emits: &[],
            narrative_on_complete:
                "{name} can call a color from earth and ash with the season's first idea.",
        },
        Milestone {
            name: "Master Pigmenter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Pigment,
                level: 0.8,
            },
            emits: &[],
            narrative_on_complete:
                "{name} reads the year in the ground colors {subject} carries home.",
        },
        Milestone {
            name: "Paragon Pigmenter",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Pigment,
                level: 0.95,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s pigments outlast the cats who wore them. The colony reads its history in {possessive} colors.",
        },
    ],
    completion_narrative:
        "{name} is the Paragon Pigmenter. The colony's seasons leave their mark through {possessive} paws.",
    incompatible_with: &[],
};

pub const CAIRN_MASTERY: AspirationChain = AspirationChain {
    name: "Cairn Mastery",
    domain: AspirationDomain::Cairn,
    milestones: &[
        Milestone {
            name: "Novice Cairn-Wright",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Cairn,
                level: 0.0,
            },
            emits: &[],
            narrative_on_complete:
                "{name} sets the first stone in place. The work has begun.",
        },
        Milestone {
            name: "Apprentice Cairn-Wright",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Cairn,
                level: 0.2,
            },
            emits: &[],
            narrative_on_complete:
                "{name} stacks a cairn that stands through the next wind.",
        },
        Milestone {
            name: "Journeyman Cairn-Wright",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Cairn,
                level: 0.4,
            },
            emits: &[],
            narrative_on_complete:
                "{name} knaps blades and grinding stones that hold their edge a season.",
        },
        Milestone {
            name: "Adept Cairn-Wright",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Cairn,
                level: 0.6,
            },
            emits: &[],
            narrative_on_complete:
                "{name} reads weight and grain in fieldstone; the stack tells {object} how to lie.",
        },
        Milestone {
            name: "Master Cairn-Wright",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Cairn,
                level: 0.8,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s cairns mark the colony's edges; other cats pace by them.",
        },
        Milestone {
            name: "Paragon Cairn-Wright",
            gate: always_true,
            progress_tracker: ProgressTracker::SkillLevel {
                skill: SkillKind::Cairn,
                level: 0.95,
            },
            emits: &[],
            narrative_on_complete:
                "{name}'s stones outlast the wind. The colony's history sits in {possessive} stacks.",
        },
    ],
    completion_narrative:
        "{name} is the Paragon Cairn-Wright. The colony's bones are the stones {subject} has set.",
    incompatible_with: &[],
};
