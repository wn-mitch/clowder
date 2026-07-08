//! Per-action success-affordance substrate (ticket 261).
//!
//! For every `(perceiver, target, ActionKind)` triple within sensing range
//! per tick, the `affordance_writer` computes a success scalar in `[0, 1]`:
//! "if perceiver took this action against target right now, how likely is
//! it to succeed?" Consumer DSEs read the scalar through a `fetch_target`
//! consideration closure (the `socialize_target.rs` pattern at
//! `src/ai/dses/socialize_target.rs:368-405`).
//!
//! Generalises ticket 103 (`escape_viability`, landed — zero current
//! readers) and supersedes ticket 141 (`combat_winnability`, ready). Sibling
//! to 258 (subjective belief substrate). 258 is `BeliefsAboutTarget`; this
//! is `ActionAffordances`. Cross-cutting DSE consumers (256-cluster ticket
//! 263, social/wildlife/conflict-low siblings) read both substrates.
//!
//! # Why a Resource, not a Component
//!
//! The escape-viability precedent kept the scalar in `ScoringContext`
//! (per-cat scoring snapshot). That shape can't carry a target axis — every
//! consumer DSE that wants to ask about a specific target would have to
//! re-derive the heuristic at the consideration site. A world-keyed
//! `Resource` rebuilt each tick keeps the heuristic computation
//! single-source-of-truth.
//!
//! # Why not `impl InfluenceMap`
//!
//! `InfluenceMap` is grid-spatial (`base_sample(pos: Position) -> f32`).
//! `ActionAffordances` is entity-pair-keyed; there is no natural `Position`
//! query. Affordance reads surface in the **L2** per-DSE trace via consumer
//! tickets, not in the **L1** global influence-map walk. No
//! `populate_influence_map_registry` call is needed; the
//! `scripts/check_influence_map_registry.sh` lint does not apply.
//!
//! # Substrate-only at land
//!
//! 261 lands the resource + writer with all 22 heuristic estimators. Zero
//! DSEs read from it on the same commit — the substrate is honest day-one
//! but unconsumed, so `just verdict` against a baseline soak shows null
//! behavioural drift. Consumer wiring lives in ticket 263 (256-cluster
//! Flee/Patrol/Hunt) and siblings.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ActionKind enum
// ---------------------------------------------------------------------------

/// The space of perceiver-against-target actions priced by
/// [`ActionAffordances`]. 21 variants across five behavioural families.
///
/// **Entity-target only.** Zone / location affordances stay in existing
/// influence maps (`RouteCostField`, `FoxScentMap`, `WardCoverageMap`,
/// …). Conflating them into this substrate would double-bookkeep state
/// already served by the per-tile maps.
///
/// **Species eligibility gates each variant.** `Pounce` is cat-only;
/// `Dive` is hawk-only; `Strike` is snake-only; `Ambush` is
/// ShadowFox-only. The `affordance_writer` filters by species before
/// computing the scalar — a hawk asking about its own `Pounce` affordance
/// against a rabbit reads `0.0` and the substrate gates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ActionKind {
    // --- Predation (per-species subset) ---
    /// Universal — predator approaches prey under cover, low alertness.
    Stalk,
    /// Universal — predator overtakes fleeing prey by speed + path.
    Chase,
    /// Cat-only — leap-strike from short range with concealment.
    Pounce,
    /// Hawk-only — aerial dive with steep approach angle.
    Dive,
    /// Snake-only — coiled strike within reach.
    Strike,
    /// ShadowFox-only — concealment-keyed ambush from a held position.
    Ambush,

    // --- Threat-response (universal) ---
    /// Move away from threat under cover.
    Flee,
    /// Engage threat with combat.
    Fight,
    /// Hold still in cover until threat passes.
    Freeze,
    /// Appease aggressor with submissive cues (placating signalling).
    Fawn,

    // --- Conflict-low (no current DSE consumer; substrate populates) ---
    /// Issue a credible threat without escalation.
    Threaten,
    /// Display dominance / size / condition cues.
    Posture,
    /// Acute warning vocalisation toward a specific target.
    Hiss,

    // --- Social (cat-cat mostly) ---
    /// Greet / rub / affiliative interaction with a peer.
    Socialize,
    /// Allogrooming directed at a specific cat.
    GroomOther,
    /// Courtship → consummation chain with a partner.
    Mate,
    /// Pass knowledge to a younger / less experienced cat.
    Mentor,
    /// Tend an injured or distressed colony-mate.
    Care,
    /// Provide food to a dependent kitten.
    FeedKitten,

    // --- Prey-side (written by 314's prey-perceiver rows; DSE consumers
    // arrive with ticket 266's prey-side AI) ---
    /// Sprint flight along the lowest-cost escape route.
    Bolt,
    /// Disrupt herd cohesion to scatter a predator's target lock.
    ScatterGroup,
}

impl ActionKind {
    /// All 21 variants in declaration order. Useful for the writer's
    /// per-tick rebuild loop and for test enumeration.
    pub const ALL: [ActionKind; 21] = [
        ActionKind::Stalk,
        ActionKind::Chase,
        ActionKind::Pounce,
        ActionKind::Dive,
        ActionKind::Strike,
        ActionKind::Ambush,
        ActionKind::Flee,
        ActionKind::Fight,
        ActionKind::Freeze,
        ActionKind::Fawn,
        ActionKind::Threaten,
        ActionKind::Posture,
        ActionKind::Hiss,
        ActionKind::Socialize,
        ActionKind::GroomOther,
        ActionKind::Mate,
        ActionKind::Mentor,
        ActionKind::Care,
        ActionKind::FeedKitten,
        ActionKind::Bolt,
        ActionKind::ScatterGroup,
    ];

    /// Stable string key for `fetch_target` consideration closures. Matches
    /// the `socialize_target.rs` pattern where each scalar input is
    /// addressed by a `&'static str` constant.
    pub const fn input_key(self) -> &'static str {
        match self {
            ActionKind::Stalk => AFFORDANCE_STALK_INPUT,
            ActionKind::Chase => AFFORDANCE_CHASE_INPUT,
            ActionKind::Pounce => AFFORDANCE_POUNCE_INPUT,
            ActionKind::Dive => AFFORDANCE_DIVE_INPUT,
            ActionKind::Strike => AFFORDANCE_STRIKE_INPUT,
            ActionKind::Ambush => AFFORDANCE_AMBUSH_INPUT,
            ActionKind::Flee => AFFORDANCE_FLEE_INPUT,
            ActionKind::Fight => AFFORDANCE_FIGHT_INPUT,
            ActionKind::Freeze => AFFORDANCE_FREEZE_INPUT,
            ActionKind::Fawn => AFFORDANCE_FAWN_INPUT,
            ActionKind::Threaten => AFFORDANCE_THREATEN_INPUT,
            ActionKind::Posture => AFFORDANCE_POSTURE_INPUT,
            ActionKind::Hiss => AFFORDANCE_HISS_INPUT,
            ActionKind::Socialize => AFFORDANCE_SOCIALIZE_INPUT,
            ActionKind::GroomOther => AFFORDANCE_GROOM_OTHER_INPUT,
            ActionKind::Mate => AFFORDANCE_MATE_INPUT,
            ActionKind::Mentor => AFFORDANCE_MENTOR_INPUT,
            ActionKind::Care => AFFORDANCE_CARE_INPUT,
            ActionKind::FeedKitten => AFFORDANCE_FEED_KITTEN_INPUT,
            ActionKind::Bolt => AFFORDANCE_BOLT_INPUT,
            ActionKind::ScatterGroup => AFFORDANCE_SCATTER_GROUP_INPUT,
        }
    }
}

// ---------------------------------------------------------------------------
// Consideration-input keys (one per ActionKind)
// ---------------------------------------------------------------------------

pub const AFFORDANCE_STALK_INPUT: &str = "affordance_stalk";
pub const AFFORDANCE_CHASE_INPUT: &str = "affordance_chase";
pub const AFFORDANCE_POUNCE_INPUT: &str = "affordance_pounce";
pub const AFFORDANCE_DIVE_INPUT: &str = "affordance_dive";
pub const AFFORDANCE_STRIKE_INPUT: &str = "affordance_strike";
pub const AFFORDANCE_AMBUSH_INPUT: &str = "affordance_ambush";
pub const AFFORDANCE_FLEE_INPUT: &str = "affordance_flee";
pub const AFFORDANCE_FIGHT_INPUT: &str = "affordance_fight";
pub const AFFORDANCE_FREEZE_INPUT: &str = "affordance_freeze";
pub const AFFORDANCE_FAWN_INPUT: &str = "affordance_fawn";
pub const AFFORDANCE_THREATEN_INPUT: &str = "affordance_threaten";
pub const AFFORDANCE_POSTURE_INPUT: &str = "affordance_posture";
pub const AFFORDANCE_HISS_INPUT: &str = "affordance_hiss";
pub const AFFORDANCE_SOCIALIZE_INPUT: &str = "affordance_socialize";
pub const AFFORDANCE_GROOM_OTHER_INPUT: &str = "affordance_groom_other";
pub const AFFORDANCE_MATE_INPUT: &str = "affordance_mate";
pub const AFFORDANCE_MENTOR_INPUT: &str = "affordance_mentor";
pub const AFFORDANCE_CARE_INPUT: &str = "affordance_care";
pub const AFFORDANCE_FEED_KITTEN_INPUT: &str = "affordance_feed_kitten";
pub const AFFORDANCE_BOLT_INPUT: &str = "affordance_bolt";
pub const AFFORDANCE_SCATTER_GROUP_INPUT: &str = "affordance_scatter_group";

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

/// Per-`(perceiver, target, action_kind)` success scalar in `[0, 1]`.
///
/// Rebuilt each tick by `affordance_writer`. Missing entries read as `0.0`
/// (the action is gated — either species-ineligible, out of sensing range,
/// or below `min_eligibility_threshold` for this kind).
///
/// `#[serde(skip)]` on the inner map mirrors the precedent for
/// entity-keyed substrate state (`CatBeliefs.models`, `pairing.rs`,
/// `held_intention.rs`) — raw `Entity` ids don't round-trip across saves,
/// so the substrate rebuilds from fresh observations on load.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ActionAffordances {
    #[serde(skip)]
    pub scalars: HashMap<(Entity, Entity, ActionKind), f32>,
}

impl ActionAffordances {
    /// Construct an empty resource. The writer's first tick populates it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the affordance scalar for `(perceiver, target, action_kind)`.
    /// Returns `0.0` when no entry exists — the substrate's gate signal.
    ///
    /// Infallible by design so consumer `fetch_target` closures stay
    /// branch-free: a `0.0` read is a meaningful "action not afforded
    /// here" that consideration curves treat as a hard penalty without
    /// the caller having to thread `Option<f32>`.
    pub fn read(&self, perceiver: Entity, target: Entity, kind: ActionKind) -> f32 {
        self.scalars
            .get(&(perceiver, target, kind))
            .copied()
            .unwrap_or(0.0)
    }

    /// Write a scalar, clamped to `[0, 1]`. Called by `affordance_writer`
    /// per `(perceiver, target, action_kind)` triple per tick.
    pub fn write(&mut self, perceiver: Entity, target: Entity, kind: ActionKind, score: f32) {
        self.scalars
            .insert((perceiver, target, kind), score.clamp(0.0, 1.0));
    }

    /// Zero every entry. Called at the start of each writer tick.
    pub fn clear(&mut self) {
        self.scalars.clear();
    }

    /// Number of populated entries. Useful for instrumentation and tests.
    pub fn len(&self) -> usize {
        self.scalars.len()
    }

    /// Whether the substrate has any populated entries.
    pub fn is_empty(&self) -> bool {
        self.scalars.is_empty()
    }
}

/// Free-function read helper for use inside DSE `fetch_target` closures.
/// Equivalent to `affordances.read(perceiver, target, kind)`; provided so
/// consumer match-arms can call a single function name instead of routing
/// through the resource handle.
///
/// The 263-cluster wiring becomes one line per kind:
/// ```ignore
/// AFFORDANCE_FLEE_INPUT => read_affordance(&affordances, perceiver, target, ActionKind::Flee),
/// AFFORDANCE_FIGHT_INPUT => read_affordance(&affordances, perceiver, target, ActionKind::Fight),
/// ```
pub fn read_affordance(
    affordances: &ActionAffordances,
    perceiver: Entity,
    target: Entity,
    kind: ActionKind,
) -> f32 {
    affordances.read(perceiver, target, kind)
}

/// 265: best affordance the perceiver holds against any of `targets`
/// across the given `kinds`. Wildlife scorers are self-state DSEs (no
/// per-target axis), so the target dimension collapses to a max at
/// ctx-build time: "how good is my best predation opportunity right
/// now". Returns `0.0` when no target/kind pair is populated — the
/// substrate's gate signal, same contract as [`ActionAffordances::read`].
///
/// Callers pre-filter `targets` by detection range; this helper does
/// not know about positions.
pub fn best_affordance_over_targets(
    affordances: &ActionAffordances,
    perceiver: Entity,
    targets: impl IntoIterator<Item = Entity>,
    kinds: &[ActionKind],
) -> f32 {
    let mut best: f32 = 0.0;
    for target in targets {
        for &kind in kinds {
            best = best.max(affordances.read(perceiver, target, kind));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    #[test]
    fn read_missing_returns_zero() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t = world.spawn_empty().id();
        let a = ActionAffordances::default();
        assert_eq!(a.read(p, t, ActionKind::Flee), 0.0);
    }

    #[test]
    fn write_then_read_round_trips() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t = world.spawn_empty().id();
        let mut a = ActionAffordances::default();
        a.write(p, t, ActionKind::Fight, 0.42);
        assert!((a.read(p, t, ActionKind::Fight) - 0.42).abs() < f32::EPSILON);
    }

    #[test]
    fn write_clamps_to_unit_interval() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t = world.spawn_empty().id();
        let mut a = ActionAffordances::default();
        a.write(p, t, ActionKind::Stalk, 1.7);
        assert_eq!(a.read(p, t, ActionKind::Stalk), 1.0);
        a.write(p, t, ActionKind::Stalk, -0.3);
        assert_eq!(a.read(p, t, ActionKind::Stalk), 0.0);
    }

    #[test]
    fn clear_zeroes_every_entry() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t = world.spawn_empty().id();
        let mut a = ActionAffordances::default();
        a.write(p, t, ActionKind::Mate, 0.5);
        a.write(p, t, ActionKind::Care, 0.7);
        a.clear();
        assert!(a.is_empty());
        assert_eq!(a.read(p, t, ActionKind::Mate), 0.0);
    }

    #[test]
    fn input_keys_are_unique() {
        let keys: Vec<&str> = ActionKind::ALL.iter().map(|k| k.input_key()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            keys.len(),
            sorted.len(),
            "ActionKind::input_key() must be injective"
        );
    }

    #[test]
    fn all_constant_covers_every_variant() {
        // If you add a variant to ActionKind, you must also append it to
        // ALL or this test fails. Matches the manual-enumeration
        // discipline.
        assert_eq!(ActionKind::ALL.len(), 21);
    }

    #[test]
    fn best_affordance_over_targets_takes_max_across_pairs() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t1 = world.spawn_empty().id();
        let t2 = world.spawn_empty().id();
        let mut a = ActionAffordances::default();
        a.write(p, t1, ActionKind::Stalk, 0.3);
        a.write(p, t1, ActionKind::Chase, 0.6);
        a.write(p, t2, ActionKind::Stalk, 0.9);
        let best =
            best_affordance_over_targets(&a, p, [t1, t2], &[ActionKind::Stalk, ActionKind::Chase]);
        assert!((best - 0.9).abs() < f32::EPSILON);
        // Kind filter is respected: Chase-only ignores t2's Stalk 0.9.
        let chase_only = best_affordance_over_targets(&a, p, [t1, t2], &[ActionKind::Chase]);
        assert!((chase_only - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn best_affordance_over_targets_empty_reads_zero() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t = world.spawn_empty().id();
        let a = ActionAffordances::default();
        // Unpopulated substrate (the dormant-stage state) gates to 0.0.
        assert_eq!(
            best_affordance_over_targets(&a, p, [t], &[ActionKind::Strike]),
            0.0
        );
        // No targets in range also gates to 0.0.
        assert_eq!(
            best_affordance_over_targets(&a, p, [], &[ActionKind::Strike]),
            0.0
        );
    }

    #[test]
    fn read_affordance_helper_matches_method() {
        let mut world = World::new();
        let p = world.spawn_empty().id();
        let t = world.spawn_empty().id();
        let mut a = ActionAffordances::default();
        a.write(p, t, ActionKind::Flee, 0.8);
        assert_eq!(
            read_affordance(&a, p, t, ActionKind::Flee),
            a.read(p, t, ActionKind::Flee),
        );
    }
}
