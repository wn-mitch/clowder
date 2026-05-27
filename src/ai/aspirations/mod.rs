//! Ticket 321 — `&'static`-shaped aspiration substrate.
//!
//! Replaces the RON-deserialized `Milestone` / `AspirationChain` shape
//! that lived in [`crate::components::aspirations`] (pre-321) with
//! code-defined const data. The migration is required by the
//! per-milestone `emits: &'static [Emit]` table from
//! [`docs/systems/htn-methods.md`](../../../../docs/systems/htn-methods.md)
//! §H — `Emit::applicable_when: fn(&World, Entity) -> bool` is a
//! function pointer and cannot deserialize from RON.
//!
//! All 14 production chains land in this module across seven domain
//! files; [`ALL_CHAINS`] is the registry walked at app build by
//! [`crate::resources::aspiration_registry::AspirationRegistry::build_static`].
//!
//! # Per-milestone `emits` (the 321 picker contract)
//!
//! Each milestone carries `emits: &'static [Emit]`. The L1→L2 emission
//! picker (`crate::systems::aspiration_picker`) walks the active
//! cat's current milestone's `emits` list per tick and, for each row,
//! checks (a) that the named goal-label resolves to a Live method in
//! `MethodRegistry` and (b) that the per-row `applicable_when`
//! predicate holds. The first matching row produces an
//! `Intention::Goal` candidate that ticket 320's HTN gate catches.
//!
//! At 321's combine-and-test land only `hunting::MASTER_OF_THE_HUNT`'s
//! "First Blood" milestone carries a non-empty `emits` table; every
//! other milestone declares `emits: &[]`. Per-chain wrapper tickets
//! (#325–#331) fill the rest.

use bevy_ecs::prelude::*;

use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;
use crate::components::mental::MemoryType;
use crate::resources::relationships::BondType;

pub mod building;
pub mod combat;
pub mod crafting;
pub mod exploration;
pub mod herbcraft;
pub mod hunting;
pub mod kinship;
pub mod leadership;
pub mod mastery;
pub mod social;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Default `gate` predicate — every cat with the chain active is
/// eligible. Per-milestone life-stage / colony-state gating lands as
/// follow-on work when authoring needs surface.
pub fn always_true(_world: &World, _entity: Entity) -> bool {
    true
}

/// Inert-predicate sentinel. Used on emit rows that are structurally
/// authored but not yet meant to fire — the substrate ships dormant
/// until the rest of the supporting infrastructure lands. Ticket 398's
/// `RaiseOffspringAspiration` emit row uses this until §L2.10.6's
/// unified softmax + §7.4 per-tier persistence-bonus arrive (phases
/// 1c/1d in the 398 plan); the row flips to `has_juvenile_dependent`
/// at that point.
pub fn always_false(_world: &World, _entity: Entity) -> bool {
    false
}

// ---------------------------------------------------------------------------
// SkillKind — typed skill axis (replaces stringly-typed RON keys)
// ---------------------------------------------------------------------------

/// Typed skill axis for [`ProgressTracker::SkillLevel`]. Mirrors the
/// numeric fields on [`crate::components::skills::Skills`]; the
/// resolver reads the matching field at progress-check time.
///
/// Phase 5 axes (366 — Weaving / BoneShaping / Hidework / Pigment /
/// Cairn) ship reader-only; their writers — the discipline-specific
/// craft actions — land in 372.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum SkillKind {
    Hunting,
    Foraging,
    Herbcraft,
    Building,
    Combat,
    Magic,
    Weaving,
    BoneShaping,
    Hidework,
    Pigment,
    Cairn,
}

impl SkillKind {
    /// Read the corresponding numeric value from a `Skills` snapshot.
    pub fn value(self, skills: &crate::components::skills::Skills) -> f32 {
        match self {
            Self::Hunting => skills.hunting,
            Self::Foraging => skills.foraging,
            Self::Herbcraft => skills.herbcraft,
            Self::Building => skills.building,
            Self::Combat => skills.combat,
            Self::Magic => skills.magic,
            Self::Weaving => skills.weaving,
            Self::BoneShaping => skills.bone_shaping,
            Self::Hidework => skills.hidework,
            Self::Pigment => skills.pigment,
            Self::Cairn => skills.cairn,
        }
    }
}

// ---------------------------------------------------------------------------
// ProgressTracker — what increments milestone progress
// ---------------------------------------------------------------------------

/// Per-milestone progress / completion predicate. Replaces the
/// stringly-typed pre-321 `MilestoneCondition`. Each variant maps 1:1
/// to a former RON case; `ActionCount` is now a slice of `Action`
/// variants so a milestone can count any action in a domain bucket
/// (the herbcraft chains' "Herbcraft" RON key fanned across the three
/// `HerbcraftX` Action variants post-155; using a slice captures that
/// without minting an `Action::Herbcraft` alias).
#[derive(Debug, Clone, Copy)]
pub enum ProgressTracker {
    /// Count completions of any listed action. Pre-321's RON-only
    /// `ActionCount(action: "Herbcraft", ...)` referenced a string
    /// that no `Action` variant stringifies to (the 155 fan-out left
    /// the milestone silently broken); the slice form lets a milestone
    /// count any of the post-155 `HerbcraftX` actions toward a single
    /// progress total. Single-action milestones use a one-element slice.
    ActionCount {
        actions: &'static [Action],
        count: u32,
    },
    SkillLevel {
        skill: SkillKind,
        level: f32,
    },
    FormBond {
        bond_type: BondType,
    },
    WitnessEvent {
        event_type: MemoryType,
        count: u32,
    },
    Mentor {
        count: u32,
    },
}

// ---------------------------------------------------------------------------
// Priority + Emit — the picker's emission tuple
// ---------------------------------------------------------------------------

/// Per-`Emit` row tier. The picker walks `emits` in `Priority` order
/// (Primary first), then by registration order within tier. First
/// match wins per htn-methods.md §H.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Priority {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

/// Conflict class between two concurrent aspirations (spec §7.7.1).
/// Only the hard classes appear in the runtime matrix; soft-resource
/// is the matrix default (absence from `incompatible_with`). Soft-
/// emotional drops via §7.7 reconsideration events (ticket 055), not
/// at the adoption gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictClass {
    /// Mutually-exclusive end-states — e.g. "warrior path" vs
    /// "pacifist mentor". Spec §7.7.1's first canonical example.
    HardLogical,
    /// Incompatible life-paths — e.g. "solitary wanderer" vs
    /// "colony coordinator". Spec §7.7.1's second canonical example.
    HardIdentity,
}

/// One row in a milestone's `emits` table. Names a candidate
/// `Intention::Goal` label, a per-cat applicability predicate, the
/// `CommitmentStrategy` the emitted Intention carries, and the tier.
#[derive(Debug, Clone, Copy)]
pub struct Emit {
    pub label: &'static str,
    pub applicable_when: fn(&World, Entity) -> bool,
    pub strategy: CommitmentStrategy,
    pub priority: Priority,
}

// ---------------------------------------------------------------------------
// Milestone + AspirationChain — `&'static`-shaped (was RON pre-321)
// ---------------------------------------------------------------------------

/// One milestone in an aspiration chain. Per htn-methods.md §H the
/// shape carries:
///
/// - `name`: short-form label for narrative + trace emission.
/// - `gate`: per-cat eligibility predicate (life-stage / colony-state
///   gating). 321 land: every milestone declares `gate: always_true`;
///   per-milestone gates land as follow-on work.
/// - `progress_tracker`: what increments progress + completion check.
/// - `emits`: the L1→L2 picker's per-row candidate table.
/// - `narrative_on_complete`: narrative-emitter template with the
///   existing `{name}`, `{possessive}`, `{subject}`, `{object}` slots.
#[derive(Debug, Clone, Copy)]
pub struct Milestone {
    pub name: &'static str,
    pub gate: fn(&World, Entity) -> bool,
    pub progress_tracker: ProgressTracker,
    pub emits: &'static [Emit],
    pub narrative_on_complete: &'static str,
}

/// A full aspiration chain — ordered milestones plus a chain-completion
/// narrative. Lives as `&'static` const data per `&'static [Milestone]`
/// + `&'static str` fields; listed in [`ALL_CHAINS`] so the registry
///   walk doesn't need to know individual chain names.
#[derive(Debug, Clone, Copy)]
pub struct AspirationChain {
    pub name: &'static str,
    pub domain: AspirationDomain,
    pub milestones: &'static [Milestone],
    pub completion_narrative: &'static str,
    /// Spec §7.7.1 hard-pair list — chain names this chain cannot be
    /// held alongside. Sparse: only genuinely-contradictory pairs need
    /// listing; matrix default is *compatible*. Authored one direction
    /// per pair (no symmetry requirement); the runtime walks both
    /// directions in [`can_adopt`].
    pub incompatible_with: &'static [(&'static str, ConflictClass)],
    /// Spec §7.7.1 per-arc expected valence baseline. The emotional
    /// tone a cat pursuing this chain is expected to feel on average.
    /// Read by ticket 055's mood drift-threshold detector: sustained
    /// `Mood::valence` below this target for ≥ N seasons signals
    /// arc misalignment and fires §7.7.d reconsideration. Range
    /// `[-1.0, 1.0]`. Author-time best guesses; balance tuning is
    /// follow-on work to ticket 055.
    pub expected_valence_target: f32,
}

// ---------------------------------------------------------------------------
// ALL_CHAINS — the registry walk surface
// ---------------------------------------------------------------------------

/// Every production aspiration chain in registration order. Walked at
/// app build by `AspirationRegistry::build_static`. Adding a chain is
/// one line here plus a `pub const` in the matching domain module.
pub const ALL_CHAINS: &[&AspirationChain] = &[
    &hunting::MASTER_OF_THE_HUNT,
    &hunting::PROVIDER_OF_THE_COLONY,
    &combat::WARRIORS_PATH,
    &combat::SHADOW_FIGHTER,
    &social::HEART_OF_THE_COLONY,
    &social::THE_BELOVED,
    &herbcraft::WHISKERWEAVERS_APPRENTICE,
    &herbcraft::HEALERS_CALLING,
    &exploration::MAPMAKER,
    &exploration::BEYOND_THE_BORDER,
    &building::DEN_SHAPER,
    &building::THE_ARCHITECT,
    &leadership::VOICE_OF_THE_COLONY,
    &leadership::THE_UNIFIER,
    &kinship::RAISE_OFFSPRING_ASPIRATION,
    // 366 — Phase 5 mastery arcs (016 Phase 5 precursor).
    &mastery::WEAVING_MASTERY,
    &mastery::BONE_SHAPING_MASTERY,
    &mastery::HIDEWORK_MASTERY,
    &mastery::PIGMENT_MASTERY,
    &mastery::CAIRN_MASTERY,
    // 463 — CraftItemAspiration. Daily-driver "I want to make warrior's-
    // kit items" chain whose picker (commit 6) scores per-recipe and
    // emits typed `Goal(HaveItem(_))` Intentions into the L2 pool.
    &crafting::CRAFT_ITEM_ASPIRATION,
];

// ---------------------------------------------------------------------------
// §7.7.1 expected-valence reader (ticket 344)
// ---------------------------------------------------------------------------

/// Resolve the per-arc `expected_valence_target` for a chain by name.
/// Ticket 055's mood drift-threshold detector calls this to compare a
/// cat's sustained `Mood::valence` against the active arc's expected
/// emotional baseline. Returns `None` if `chain_name` is not registered.
pub fn expected_valence_for(
    chain_name: &str,
    registry: &crate::resources::aspiration_registry::AspirationRegistry,
) -> Option<f32> {
    registry
        .chain_by_name(chain_name)
        .map(|c| c.expected_valence_target)
}

// ---------------------------------------------------------------------------
// §7.7.1 adoption gate
// ---------------------------------------------------------------------------

/// Spec §7.7.1 adoption gate. Returns `None` if `candidate` is
/// compatible with every chain in `existing`; returns
/// `Some((blocker_name, class))` if a hard-conflict pair blocks
/// adoption.
///
/// Walks both directions of the sparse matrix: `candidate.incompatible_with`
/// AND each existing chain's `incompatible_with` for `candidate`. This
/// makes one-sided declaration sufficient — authoring the pair on
/// either chain blocks adoption from both directions. Spec §7.7.1
/// (`docs/systems/ai-substrate-refactor.md:4810`).
pub fn can_adopt(
    existing: &[crate::components::aspirations::ActiveAspiration],
    candidate: &AspirationChain,
    registry: &crate::resources::aspiration_registry::AspirationRegistry,
) -> Option<(&'static str, ConflictClass)> {
    for active in existing {
        // Direction 1: candidate declares the conflict.
        if let Some(&(_, class)) = candidate
            .incompatible_with
            .iter()
            .find(|(name, _)| *name == active.chain_name.as_str())
        {
            let blocker = registry
                .chain_by_name(&active.chain_name)
                .map(|c| c.name)
                .unwrap_or("(unknown)");
            return Some((blocker, class));
        }
        // Direction 2: the existing chain declares the conflict.
        if let Some(active_chain) = registry.chain_by_name(&active.chain_name) {
            if let Some(&(_, class)) = active_chain
                .incompatible_with
                .iter()
                .find(|(name, _)| *name == candidate.name)
            {
                return Some((active_chain.name, class));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crate::components::aspirations::{ActiveAspiration, AspirationDomain};
    use crate::resources::aspiration_registry::AspirationRegistry;

    fn registry() -> AspirationRegistry {
        AspirationRegistry::build_static()
    }

    /// Helper: build a one-element `existing` slice for a chain by name.
    fn existing_with(name: &str, domain: AspirationDomain) -> Vec<ActiveAspiration> {
        vec![ActiveAspiration {
            chain_name: name.to_string(),
            domain,
            current_milestone: 0,
            progress: 0,
            adopted_tick: 0,
            last_progress_tick: 0,
            misaligned_since_tick: None,
        }]
    }

    #[test]
    fn warriors_path_blocks_healers_calling_hard_logical() {
        let r = registry();
        let healers = r.chain_by_name("Healer's Calling").unwrap();
        let existing = existing_with("Warrior's Path", AspirationDomain::Combat);
        let outcome = can_adopt(&existing, healers, &r);
        let (blocker, class) = outcome.expect("expected hard-logical conflict");
        assert_eq!(blocker, "Warrior's Path");
        assert_eq!(class, ConflictClass::HardLogical);
    }

    #[test]
    fn healers_calling_blocks_warriors_path_by_reverse_walk() {
        // Pair is authored on WARRIORS_PATH only; the reverse-direction
        // walk in `can_adopt` must still fire when Healer's Calling is
        // held first.
        let r = registry();
        let warriors = r.chain_by_name("Warrior's Path").unwrap();
        let existing = existing_with("Healer's Calling", AspirationDomain::Herbcraft);
        let outcome = can_adopt(&existing, warriors, &r);
        let (blocker, class) = outcome.expect("reverse-direction walk should fire");
        assert_eq!(blocker, "Healer's Calling");
        assert_eq!(class, ConflictClass::HardLogical);
    }

    #[test]
    fn beyond_the_border_blocks_voice_of_the_colony_hard_identity() {
        let r = registry();
        let voice = r.chain_by_name("Voice of the Colony").unwrap();
        let existing = existing_with("Beyond the Border", AspirationDomain::Exploration);
        let outcome = can_adopt(&existing, voice, &r);
        let (blocker, class) = outcome.expect("expected hard-identity conflict");
        assert_eq!(blocker, "Beyond the Border");
        assert_eq!(class, ConflictClass::HardIdentity);
    }

    #[test]
    fn voice_of_the_colony_blocks_beyond_the_border_by_reverse_walk() {
        let r = registry();
        let border = r.chain_by_name("Beyond the Border").unwrap();
        let existing = existing_with("Voice of the Colony", AspirationDomain::Leadership);
        let outcome = can_adopt(&existing, border, &r);
        let (blocker, class) = outcome.expect("reverse-direction walk should fire");
        assert_eq!(blocker, "Voice of the Colony");
        assert_eq!(class, ConflictClass::HardIdentity);
    }

    #[test]
    fn all_chains_have_no_self_conflict() {
        let r = registry();
        for chain in r.all_chains() {
            let existing = existing_with(chain.name, chain.domain);
            assert!(
                can_adopt(&existing, chain, &r).is_none(),
                "chain '{}' conflicts with itself",
                chain.name,
            );
        }
    }

    #[test]
    fn mastery_chains_registered() {
        // 366 — Phase 5 precursor. The five mastery arcs must be
        // present in the registry so `select_aspirations` can score
        // them and `is_phase5_unlocked` finds them by name. Domain
        // and name pairing is asserted to catch a copy-paste swap
        // (e.g. CAIRN_MASTERY tagged AspirationDomain::Pigment).
        let r = registry();
        for (chain_name, domain) in [
            ("Weaving Mastery", AspirationDomain::Weaving),
            ("Bone-Shaping Mastery", AspirationDomain::BoneShaping),
            ("Hidework Mastery", AspirationDomain::Hidework),
            ("Pigment Mastery", AspirationDomain::Pigment),
            ("Cairn Mastery", AspirationDomain::Cairn),
        ] {
            let chain = r
                .chain_by_name(chain_name)
                .unwrap_or_else(|| panic!("missing chain '{chain_name}'"));
            assert_eq!(chain.domain, domain, "{chain_name} domain mismatch");
            assert_eq!(chain.milestones.len(), 6, "{chain_name} milestone count");
        }
    }

    #[test]
    fn unrelated_pair_is_compatible() {
        // Pins the matrix-default-is-compatible contract: an unrelated
        // cross-domain pair must produce no conflict.
        let r = registry();
        let mapmaker = r.chain_by_name("Mapmaker").unwrap();
        let existing = existing_with("Den Shaper", AspirationDomain::Building);
        assert!(can_adopt(&existing, mapmaker, &r).is_none());
    }

    #[test]
    fn incompatible_with_strings_resolve_to_real_chains() {
        // Catches typos like `"Warriors' Path"` vs `"Warrior's Path"`.
        let r = registry();
        for chain in r.all_chains() {
            for (other_name, _) in chain.incompatible_with {
                assert!(
                    r.chain_by_name(other_name).is_some(),
                    "chain '{}' lists unknown chain '{}' in incompatible_with",
                    chain.name,
                    other_name,
                );
            }
        }
    }

    #[test]
    fn expected_valence_targets_are_in_unit_range() {
        // Ticket 344 — every chain's expected_valence_target must sit
        // in [-1.0, 1.0]; out-of-range values would corrupt 055's
        // sustained-mood comparator.
        for c in ALL_CHAINS {
            assert!(
                (-1.0..=1.0).contains(&c.expected_valence_target),
                "chain '{}' expected_valence_target {} out of [-1.0, 1.0]",
                c.name,
                c.expected_valence_target,
            );
        }
    }

    #[test]
    fn expected_valence_for_resolves_every_chain() {
        // Ticket 344 — the free-fn reader must resolve for every name
        // in the registry, so 055 doesn't silently fall through to
        // `None` for any active arc.
        let r = registry();
        for c in ALL_CHAINS {
            assert_eq!(
                expected_valence_for(c.name, &r),
                Some(c.expected_valence_target),
                "expected_valence_for failed to resolve '{}'",
                c.name,
            );
        }
    }

    #[test]
    fn expected_valence_for_unknown_chain_returns_none() {
        let r = registry();
        assert_eq!(expected_valence_for("Not A Chain", &r), None);
    }

    #[test]
    fn symmetric_pairs_agree_on_class() {
        // One-sided declaration is legal (the reverse-walk in
        // `can_adopt` handles it). But if BOTH sides happen to list
        // each other, the class must agree — otherwise the gate is
        // direction-dependent and inconsistent.
        let r = registry();
        for a in r.all_chains() {
            for &(b_name, a_class) in a.incompatible_with {
                let Some(b) = r.chain_by_name(b_name) else {
                    continue;
                };
                if let Some(&(_, b_class)) = b.incompatible_with.iter().find(|(n, _)| *n == a.name)
                {
                    assert_eq!(
                        a_class, b_class,
                        "symmetric pair ('{}', '{}') disagrees on class",
                        a.name, b_name,
                    );
                }
            }
        }
    }
}
