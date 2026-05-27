//! Crafting domain — `CRAFT_ITEM_ASPIRATION` (ticket 463).
//!
//! Spec: `docs/systems/ai-substrate-refactor.md` §L2.10.5 +
//! `docs/systems/crafting.md` — the "downwind of the stat I'm trying to
//! improve" framing for recipe selection. Per-tick the picker scores
//! each satisfiable recipe along three axes, picks the winner, and
//! emits one `Intention::Goal(GoalKind::HaveItem(<best>.output))` into
//! the L2 pool. The held HaveItem Intention runs through the
//! `craft_have_item_actions` plan template (ticket 462 + 463 commit 2)
//! which prefixes a `RetrieveCraftInputs(recipe.id)` step before
//! `CraftAt<Station>`.
//!
//! # Why this exists as a separate domain
//!
//! Distinct from the Phase 5 mastery domains (`Weaving`, `BoneShaping`,
//! …) because mastery arcs track *skill advancement* — the cat wants
//! to *get good* at a discipline. `CRAFT_ITEM_ASPIRATION` tracks *item
//! production* — the cat wants to *have* a specific warrior's-kit item.
//! A cat under threat with low Hidework wants to craft hide armor;
//! that's `Crafting` aspiration, not `Hidework` mastery.
//!
//! Mastery and Crafting compose multiplicatively in scoring: the
//! Hidework mastery arc's modifier lifts Crafting's score when both
//! are active, and within `CraftItemAspiration` the discipline-skill-
//! affinity term `(1 - skill_value)` picks the recipe whose discipline
//! the cat is *least* developed at — pursuing growth in the very axis
//! the mastery arc tracks.
//!
//! # Skeleton-only landing (commit 5 of 463)
//!
//! At this commit the chain registers in
//! [`crate::ai::aspirations::ALL_CHAINS`] with **no live emit row**.
//! The single milestone's `emits` slice carries a sentinel
//! `"craft_item"` label, but the picker's step-2 method-live gate
//! (`MethodRegistry::lookup("craft_item", …).is_some()`) returns
//! `false` because no method carries that label. The row never reaches
//! `AspirationEmissions::rows`; soak behavior is byte-identical to
//! pre-463.
//!
//! Commit 6 lifts the weight: the picker special-cases this chain to
//! bypass the static method-live gate, runs the per-recipe scoring
//! loop, and attaches a dynamic `goal_kind: Some(GoalKind::HaveItem(
//! winner.output))` to the emitted `EmissionRow` so the L2 wrap at
//! `goap.rs:3192` constructs a typed Goal directly (path wired by
//! commit 1).
//!
//! # §7.7.1 conflict class
//!
//! `incompatible_with` is empty at 463. Crafting is a daily-driver
//! aspiration; it composes with the warrior's-path / shadow-fighter /
//! mastery arcs rather than excluding them.

use super::{always_true, AspirationChain, Emit, Milestone, Priority, ProgressTracker};
use crate::ai::dse::CommitmentStrategy;
use crate::ai::Action;
use crate::components::aspirations::AspirationDomain;

/// Sentinel emit row. The label `"craft_item"` is overwritten per cat
/// per tick by the picker's recipe-scoring loop (commit 6) — by the
/// time the row reaches the L2 wrap site, its `goal_kind` carries
/// the actual `GoalKind::HaveItem(item)` and the label derives from
/// `ItemKind::goal_label` ("have_<item>"). At commit 5 the row never
/// reaches L2 because no static method carries the `"craft_item"`
/// label — the picker's step-2 gate filters it out.
///
/// `applicable_when: always_true` so the chain advances past the
/// per-cat gate; the *real* gate (does this cat have any satisfiable
/// recipe) lives in commit 6's picker extension where the recipe-
/// scoring loop runs.
///
/// `Priority::Tertiary` keeps the row below the Primary/Secondary rows
/// of existing chains (Hunting, Kinship, etc.) so first-light
/// activation doesn't preempt established arcs. Lifts to Secondary in
/// commit 7 once the variation gate confirms emission.
const CRAFT_ITEM_EMITS: &[Emit] = &[Emit {
    label: "craft_item",
    applicable_when: always_true,
    strategy: CommitmentStrategy::OpenMinded,
    priority: Priority::Tertiary,
}];

/// `CraftItemAspiration` — "I want to make warrior's-kit items".
/// Single milestone: "First Tool", progresses via `ActionCount` over
/// `Action::Craft`. Completion at `count: 9999` matches the §7.M.2
/// lifetime-arc precedent — the chain stays active across the cat's
/// lifetime and re-emits per tick the picker's recipe-scoring loop
/// finds a satisfiable recipe.
pub const CRAFT_ITEM_ASPIRATION: AspirationChain = AspirationChain {
    name: "Craft Item",
    domain: AspirationDomain::Crafting,
    milestones: &[Milestone {
        name: "First Tool",
        gate: always_true,
        progress_tracker: ProgressTracker::ActionCount {
            actions: &[Action::Craft],
            count: 9999,
        },
        emits: CRAFT_ITEM_EMITS,
        narrative_on_complete: "{name} fits a fresh tool to {possessive} paw. The work has begun.",
    }],
    completion_narrative:
        "{name} has made the kit {subject} needed. The colony's edge is sharper for it.",
    incompatible_with: &[],
    // Crafting under threat is anxious / focused; outside threat it's
    // a satisfying daily-driver activity. Author-time best guess; tune
    // via ticket 055's mood-drift detector if a parked-Crafting valence
    // floor emerges.
    expected_valence_target: 0.20,
};
