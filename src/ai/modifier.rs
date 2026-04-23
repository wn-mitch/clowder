//! §3.5 post-scoring modifiers (`docs/systems/ai-substrate-refactor.md`).
//!
//! A `ScoreModifier` is a pure post-composition pass: given a DSE's id,
//! its gated score, the cat's eval context, and the canonical scalar
//! fetcher, it returns a transformed score. The pipeline applies every
//! registered modifier in registration order — ch 13 §"Layered
//! Weighting Models / Propagation of Change" calls this the filter-stage
//! shape.
//!
//! Phase 4.2 ported the Herbcraft / PracticeMagic emergency-bonus
//! retargets out of the inline `score_actions` block into first-class
//! `ScoreModifier` implementations:
//!
//! - [`WardCorruptionEmergency`] — boosts ward-setting DSEs when the
//!   colony has low ward coverage *and* territory corruption is
//!   detected. Applies to `herbcraft_gather` (pre-gather
//!   thornbriar), `herbcraft_ward`, and `magic_durable_ward`.
//! - [`CleanseEmergency`] — boosts corruption-cleansing DSEs when
//!   territory corruption is detected. Applies to `magic_cleanse`
//!   and `magic_colony_cleanse`.
//! - [`SensedRotBoost`] — proactive boost to ward / colony-cleanse
//!   DSEs when the cat smells corruption on nearby tiles even before
//!   standing on it. Applies to `magic_durable_ward` and
//!   `magic_colony_cleanse`.
//!
//! The seven §3.5.1 foundational modifiers follow the same pattern —
//! ported here out of the old `score_actions` 666–750 inline block:
//!
//! - [`Pride`] — additive boost to status-granting DSEs (Hunt /
//!   Fight / Patrol / Build / Coordinate) when the cat's respect need
//!   is below threshold.
//! - [`IndependenceSolo`] — additive boost to solo DSEs (Explore /
//!   Wander / Hunt) scaled by the cat's independence personality axis.
//! - [`IndependenceGroup`] — subtractive penalty to group DSEs
//!   (Socialize / Coordinate / Mentor) scaled by independence, clamped
//!   to `>= 0` so a high-independence cat doesn't go negative on a
//!   group action.
//! - [`Patience`] — additive commitment bonus to every DSE that is a
//!   constituent of the cat's *active disposition*. Membership uses
//!   the same `DispositionKind::constituent_actions()` map as the
//!   retiring inline block; see `DISPOSITION_CONSTITUENT_DSES` for the
//!   (kind, dse_id) table.
//! - [`Tradition`] — flat additive location bonus applied to **every
//!   DSE** when the caller pre-computes a positive
//!   `tradition_location_bonus`. The unfiltered per-DSE loop is a
//!   known bug (`ai-substrate-refactor.md` §3.5.3 item 1) —
//!   preserved here as a faithful port, tracked separately for a
//!   balance-methodology-scoped fix.
//! - [`FoxTerritorySuppression`] — multiplicative damp on Hunt /
//!   Explore / Forage / Patrol / Wander when fox scent is above the
//!   suppression threshold, plus an additive boost on `Flee`
//!   proportional to the same suppression (spec §3.5.3 item 2).
//! - [`CorruptionTerritorySuppression`] — multiplicative damp on
//!   Explore / Wander / Idle when the cat is standing on a corrupted
//!   tile above the suppression threshold. Shape mirrors fox
//!   suppression; no Flee-boost secondary effect.

use bevy::prelude::Entity;

use crate::ai::dse::{DseId, EvalCtx};
use crate::ai::eval::{ModifierPipeline, ScoreModifier};
use crate::resources::sim_constants::ScoringConstants;

// ---------------------------------------------------------------------------
// Scalar keys
// ---------------------------------------------------------------------------
//
// The modifiers below read their trigger inputs through the canonical
// scalar surface (`ctx_scalars` in `scoring.rs`). Keys are duplicated
// here as `&'static str` constants so drift between modifier triggers
// and `ctx_scalars` producers is visible at grep time.

const WARD_DEFICIT: &str = "ward_deficit";
const TERRITORY_MAX_CORRUPTION: &str = "territory_max_corruption";
const NEARBY_CORRUPTION_LEVEL: &str = "nearby_corruption_level";
const MASLOW_L2_SUPPRESSION: &str = "maslow_level_2_suppression";
const HAS_HERBS_NEARBY: &str = "has_herbs_nearby";
const HAS_WARD_HERBS: &str = "has_ward_herbs";
const THORNBRIAR_AVAILABLE: &str = "thornbriar_available";

// §3.5.1 modifier trigger inputs.
const RESPECT: &str = "respect";
const PRIDE: &str = "pride";
const INDEPENDENCE: &str = "independence";
const PATIENCE: &str = "patience";
const TRADITION_LOCATION_BONUS: &str = "tradition_location_bonus";
const FOX_SCENT_LEVEL: &str = "fox_scent_level";
const TILE_CORRUPTION: &str = "tile_corruption";
const ACTIVE_DISPOSITION_ORDINAL: &str = "active_disposition_ordinal";

// ---------------------------------------------------------------------------
// DSE ids the modifiers target
// ---------------------------------------------------------------------------

const HERBCRAFT_GATHER: &str = "herbcraft_gather";
const HERBCRAFT_WARD: &str = "herbcraft_ward";
const MAGIC_DURABLE_WARD: &str = "magic_durable_ward";
const MAGIC_CLEANSE: &str = "magic_cleanse";
const MAGIC_COLONY_CLEANSE: &str = "magic_colony_cleanse";

// §3.5.1 foundational modifier DSE targets.
const EAT: &str = "eat";
const SLEEP: &str = "sleep";
const HUNT: &str = "hunt";
const FORAGE: &str = "forage";
const GROOM_SELF: &str = "groom_self";
const GROOM_OTHER: &str = "groom_other";
const FLEE: &str = "flee";
const FIGHT: &str = "fight";
const PATROL: &str = "patrol";
const BUILD: &str = "build";
const FARM: &str = "farm";
const SOCIALIZE: &str = "socialize";
const EXPLORE: &str = "explore";
const WANDER: &str = "wander";
const COOK: &str = "cook";
const HERBCRAFT_PREPARE: &str = "herbcraft_prepare";
const MAGIC_SCRY: &str = "magic_scry";
const MAGIC_HARVEST: &str = "magic_harvest";
const MAGIC_COMMUNE: &str = "magic_commune";
const COORDINATE: &str = "coordinate";
const MENTOR: &str = "mentor";
const MATE: &str = "mate";
const CARETAKE: &str = "caretake";
const IDLE: &str = "idle";

// ---------------------------------------------------------------------------
// WardCorruptionEmergency
// ---------------------------------------------------------------------------

/// Port of `ward_corruption_emergency_bonus` from the retiring inline
/// Herbcraft/PracticeMagic blocks.
///
/// **Trigger:** `ward_deficit > 0 && territory_max_corruption > 0`.
///
/// **Transform:** additive `ward_corruption_emergency_bonus *
/// maslow_level_2_suppression`. For `herbcraft_gather` the trigger is
/// narrowed further to `has_herbs_nearby && !has_ward_herbs &&
/// thornbriar_available` — matching the inline `gather_emergency`
/// pre-gate that only boosted gather-for-ward when harvestable
/// thornbriar was reachable and no ward herbs were already held.
///
/// **Applies to:** `herbcraft_gather`, `herbcraft_ward`,
/// `magic_durable_ward`.
pub struct WardCorruptionEmergency {
    bonus: f32,
}

impl WardCorruptionEmergency {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.ward_corruption_emergency_bonus,
        }
    }
}

impl ScoreModifier for WardCorruptionEmergency {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        // Skip DSEs that aren't ward-related — no work to do.
        let targets_ward_action = matches!(
            dse_id.0,
            HERBCRAFT_GATHER | HERBCRAFT_WARD | MAGIC_DURABLE_WARD
        );
        if !targets_ward_action {
            return score;
        }
        // Score of zero means the outer scoring-layer gate suppressed
        // this DSE entirely (e.g. `has_herbs_nearby` false for gather).
        // Emergency bonuses don't resurrect a suppressed DSE — they
        // boost one that's already eligible to compete.
        if score <= 0.0 {
            return score;
        }

        let ward_deficit = fetch(WARD_DEFICIT, ctx.cat);
        let territory_max = fetch(TERRITORY_MAX_CORRUPTION, ctx.cat);
        if ward_deficit <= 0.0 || territory_max <= 0.0 {
            return score;
        }

        // For `herbcraft_gather`, apply the narrower pre-gate that the
        // inline block enforced: only boost if thornbriar is harvestable
        // and the cat doesn't already have ward herbs in hand.
        if dse_id.0 == HERBCRAFT_GATHER {
            let has_herbs = fetch(HAS_HERBS_NEARBY, ctx.cat) > 0.5;
            let has_ward_herbs = fetch(HAS_WARD_HERBS, ctx.cat) > 0.5;
            let thornbriar = fetch(THORNBRIAR_AVAILABLE, ctx.cat) > 0.5;
            if !(has_herbs && !has_ward_herbs && thornbriar) {
                return score;
            }
        }

        let suppression = fetch(MASLOW_L2_SUPPRESSION, ctx.cat);
        score + self.bonus * suppression
    }

    fn name(&self) -> &'static str {
        "ward_corruption_emergency"
    }
}

// ---------------------------------------------------------------------------
// CleanseEmergency
// ---------------------------------------------------------------------------

/// Port of `cleanse_corruption_emergency_bonus` from the retiring
/// inline PracticeMagic block.
///
/// **Trigger:** `territory_max_corruption > 0`.
///
/// **Transform:** additive `cleanse_corruption_emergency_bonus *
/// maslow_level_2_suppression`.
///
/// **Applies to:** `magic_cleanse`, `magic_colony_cleanse`. The inline
/// `magic_cleanse` path has an outer eligibility gate
/// (`on_corrupted_tile && tile_corruption > threshold`) that remains
/// in the scoring layer — a DSE suppressed by the gate scores 0 and
/// the modifier's `score <= 0.0` short-circuit skips it.
pub struct CleanseEmergency {
    bonus: f32,
}

impl CleanseEmergency {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.cleanse_corruption_emergency_bonus,
        }
    }
}

impl ScoreModifier for CleanseEmergency {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, MAGIC_CLEANSE | MAGIC_COLONY_CLEANSE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        if fetch(TERRITORY_MAX_CORRUPTION, ctx.cat) <= 0.0 {
            return score;
        }
        let suppression = fetch(MASLOW_L2_SUPPRESSION, ctx.cat);
        score + self.bonus * suppression
    }

    fn name(&self) -> &'static str {
        "cleanse_emergency"
    }
}

// ---------------------------------------------------------------------------
// SensedRotBoost
// ---------------------------------------------------------------------------

/// Port of `corruption_sensed_response_bonus` from the retiring inline
/// PracticeMagic block.
///
/// **Trigger:** `nearby_corruption_level > 0.1`. The 0.1 floor matches
/// the inline gate's lower bound; below that, scent is treated as
/// ambient drift rather than a response-worthy signal.
///
/// **Transform:** additive `corruption_sensed_response_bonus *
/// nearby_corruption_level * maslow_level_2_suppression`. Scales with
/// how much rot the cat can smell — the modifier reads the raw
/// `nearby_corruption_level` scalar (0-1 float) rather than a thresholded
/// gate, so the boost rises smoothly with nearby corruption intensity.
///
/// **Applies to:** `magic_durable_ward`, `magic_colony_cleanse`. Gives
/// the cat a proactive response trigger before stepping onto
/// corruption (which the standing-on-it `CleanseEmergency` would
/// require).
pub struct SensedRotBoost {
    scale: f32,
}

impl SensedRotBoost {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            scale: sc.corruption_sensed_response_bonus,
        }
    }
}

impl ScoreModifier for SensedRotBoost {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, MAGIC_DURABLE_WARD | MAGIC_COLONY_CLEANSE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        let nearby = fetch(NEARBY_CORRUPTION_LEVEL, ctx.cat);
        if nearby <= 0.1 {
            return score;
        }
        let suppression = fetch(MASLOW_L2_SUPPRESSION, ctx.cat);
        score + self.scale * nearby * suppression
    }

    fn name(&self) -> &'static str {
        "sensed_rot_boost"
    }
}

// ---------------------------------------------------------------------------
// Pride
// ---------------------------------------------------------------------------

/// §3.5.1 Pride bonus. Port of the retiring `scoring.rs:956–967` inline
/// block.
///
/// **Trigger:** `respect < pride_respect_threshold` (default 0.5).
///
/// **Transform:** `score += personality.pride × pride_bonus` —
/// additive, personality-scaled. Default bonus `0.1` → adds up to
/// `[0.0, 0.1]` across the `personality.pride` axis.
///
/// **Applies to:** `hunt`, `fight`, `patrol`, `build`, `coordinate`
/// per §3.5.2.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// so the additive bonus doesn't resurrect a DSE the Maslow pre-gate
/// (or its outer scoring-layer gate) suppressed — matching the
/// Phase 4.2 emergency-bonus pattern (`WardCorruptionEmergency`,
/// `CleanseEmergency`, `SensedRotBoost`).
pub struct Pride {
    respect_threshold: f32,
    bonus: f32,
}

impl Pride {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            respect_threshold: sc.pride_respect_threshold,
            bonus: sc.pride_bonus,
        }
    }
}

impl ScoreModifier for Pride {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, HUNT | FIGHT | PATROL | BUILD | COORDINATE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        let respect = fetch(RESPECT, ctx.cat);
        if respect >= self.respect_threshold {
            return score;
        }
        let pride = fetch(PRIDE, ctx.cat);
        score + pride * self.bonus
    }

    fn name(&self) -> &'static str {
        "pride"
    }
}

// ---------------------------------------------------------------------------
// IndependenceSolo
// ---------------------------------------------------------------------------

/// §3.5.1 Independence (solo boost). Port of the retiring
/// `scoring.rs:970–983` inline block's solo-action arm.
///
/// **Trigger:** always active (no threshold gate).
///
/// **Transform:** `score += personality.independence ×
/// independence_solo_bonus` — additive, personality-scaled. Default
/// bonus `0.1`.
///
/// **Applies to:** `explore`, `wander`, `hunt` per §3.5.2.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — even though this modifier is "always active," the additive
/// bonus still shouldn't resurrect a DSE the outer gate or Maslow
/// pre-gate suppressed.
pub struct IndependenceSolo {
    bonus: f32,
}

impl IndependenceSolo {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.independence_solo_bonus,
        }
    }
}

impl ScoreModifier for IndependenceSolo {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, EXPLORE | WANDER | HUNT) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        let ind = fetch(INDEPENDENCE, ctx.cat);
        score + ind * self.bonus
    }

    fn name(&self) -> &'static str {
        "independence_solo"
    }
}

// ---------------------------------------------------------------------------
// IndependenceGroup
// ---------------------------------------------------------------------------

/// §3.5.1 Independence (group penalty). Port of the retiring
/// `scoring.rs:970–983` inline block's group-action arm.
///
/// **Trigger:** always active.
///
/// **Transform:** `score = (score − personality.independence ×
/// independence_group_penalty).max(0.0)` — subtractive,
/// personality-scaled, clamped to `>= 0`. Default penalty `0.1`.
///
/// **Applies to:** `socialize`, `coordinate`, `mentor` per §3.5.2.
pub struct IndependenceGroup {
    penalty: f32,
}

impl IndependenceGroup {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            penalty: sc.independence_group_penalty,
        }
    }
}

impl ScoreModifier for IndependenceGroup {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, SOCIALIZE | COORDINATE | MENTOR) {
            return score;
        }
        let ind = fetch(INDEPENDENCE, ctx.cat);
        (score - ind * self.penalty).max(0.0)
    }

    fn name(&self) -> &'static str {
        "independence_group"
    }
}

// ---------------------------------------------------------------------------
// Patience
// ---------------------------------------------------------------------------

/// §3.5.1 Patience commitment bonus. Port of the retiring
/// `scoring.rs:986–994` inline block.
///
/// **Trigger:** `active_disposition.is_some()` — the cat has committed
/// to a sustained behavioral orientation.
///
/// **Transform:** `score += personality.patience ×
/// patience_commitment_bonus` — additive, personality-scaled, applied
/// to the DSEs that are constituent actions of the active disposition
/// (mirrors `DispositionKind::constituent_actions()` in the retiring
/// inline block). Default bonus `0.15`.
///
/// **Applies to:** dynamic per §3.5.2 — any DSE in the active
/// disposition's constituent list. See
/// [`DISPOSITION_CONSTITUENT_DSES`] for the (`DispositionKind`,
/// `DseId`) table.
///
/// **Scalar-surface contract:** the caller's `ctx_scalars` emits
/// `active_disposition_ordinal` as `0.0` for no active disposition
/// and `1.0..=12.0` per variant in declaration order. The modifier
/// decodes the ordinal, looks up the DSE-id set for the kind, and
/// checks membership of the incoming `dse_id`.
///
/// **Gated-boost contract:** returns `score` unchanged on score
/// `<= 0` — the additive bonus doesn't resurrect a suppressed DSE.
pub struct Patience {
    bonus: f32,
}

impl Patience {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.patience_commitment_bonus,
        }
    }
}

impl ScoreModifier for Patience {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if score <= 0.0 {
            return score;
        }
        let ordinal = fetch(ACTIVE_DISPOSITION_ORDINAL, ctx.cat);
        let Some(constituents) = constituent_dses_for_ordinal(ordinal) else {
            return score;
        };
        if !constituents.contains(&dse_id.0) {
            return score;
        }
        let patience = fetch(PATIENCE, ctx.cat);
        score + patience * self.bonus
    }

    fn name(&self) -> &'static str {
        "patience"
    }
}

/// Map `active_disposition_ordinal` (emitted by
/// `scoring.rs::active_disposition_ordinal`) to the set of DSE ids
/// that inherit Patience's additive bonus. Mirrors
/// `DispositionKind::constituent_actions()` in `components/disposition.rs`
/// — when that table changes, update this table in the same commit.
///
/// Returns `None` when `ordinal == 0.0` (no active disposition).
/// Returns `Some(&[])` is unreachable — every variant maps to at least
/// one DSE.
fn constituent_dses_for_ordinal(ordinal: f32) -> Option<&'static [&'static str]> {
    // Round to nearest integer in case of float-fetch noise; compare
    // against known variants. Values outside `[0, 12]` are treated as
    // "no active disposition" defensively.
    let rounded = ordinal.round() as i32;
    match rounded {
        0 => None,
        // Resting → Eat, Sleep, Groom (both self + other, matching
        // the `Action::Groom` aggregation in the retiring inline
        // block).
        1 => Some(&[EAT, SLEEP, GROOM_SELF, GROOM_OTHER]),
        // Hunting → Hunt.
        2 => Some(&[HUNT]),
        // Foraging → Forage.
        3 => Some(&[FORAGE]),
        // Guarding → Patrol, Fight.
        4 => Some(&[PATROL, FIGHT]),
        // Socializing → Socialize, Groom, Mentor.
        5 => Some(&[SOCIALIZE, GROOM_SELF, GROOM_OTHER, MENTOR]),
        // Building → Build.
        6 => Some(&[BUILD]),
        // Farming → Farm.
        7 => Some(&[FARM]),
        // Crafting → Herbcraft (all 3 siblings), PracticeMagic (all 6
        // siblings), Cook. The retiring inline block applied the
        // bonus to `Action::Herbcraft` and `Action::PracticeMagic`
        // (the composite-score entries); in modifier-pipeline form
        // the bonus applies to every sibling individually. The
        // composite `best = max(siblings)` equation absorbs this —
        // `max(a + Δ, b + Δ, c + Δ) = max(a, b, c) + Δ` — so the
        // outer Herbcraft/PracticeMagic score receives the same Δ.
        8 => Some(&[
            HERBCRAFT_GATHER,
            HERBCRAFT_PREPARE,
            HERBCRAFT_WARD,
            MAGIC_SCRY,
            MAGIC_DURABLE_WARD,
            MAGIC_CLEANSE,
            MAGIC_COLONY_CLEANSE,
            MAGIC_HARVEST,
            MAGIC_COMMUNE,
            COOK,
        ]),
        // Coordinating → Coordinate.
        9 => Some(&[COORDINATE]),
        // Exploring → Explore, Wander.
        10 => Some(&[EXPLORE, WANDER]),
        // Mating → Mate.
        11 => Some(&[MATE]),
        // Caretaking → Caretake.
        12 => Some(&[CARETAKE]),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tradition
// ---------------------------------------------------------------------------

/// §3.5.1 Tradition location bonus. Port of the retiring
/// `scoring.rs:997–1004` inline block.
///
/// **Trigger:** `tradition_location_bonus > 0.0` (caller pre-computes
/// as `personality.tradition × 0.1` when the cat has a history of
/// successful actions at this tile; else `0.0`).
///
/// **Transform:** `score += tradition_location_bonus` — flat additive.
///
/// **Applies to:** **every DSE** per §3.5.2 (unfiltered loop — see
/// §3.5.3 item 1). The inline block iterates `scores.iter_mut()`
/// without filtering by action; the modifier preserves this semantics
/// by not short-circuiting on `dse_id`. The §3.5.3 fix (per-action
/// history-matched bonus) is filed separately as a balance-
/// methodology-scoped change, not folded into this port.
///
/// **Gated-boost contract:** returns `score` unchanged on score
/// `<= 0` — matches the other additive modifiers' "don't resurrect
/// suppressed DSE" stance. Today the caller sets
/// `tradition_location_bonus = 0.0` in production (`goap.rs:900`), so
/// the modifier is a no-op on live scoring paths and the gate-
/// behavior is invisible.
pub struct Tradition;

impl Tradition {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Tradition {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoreModifier for Tradition {
    fn apply(
        &self,
        _dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if score <= 0.0 {
            return score;
        }
        let bonus = fetch(TRADITION_LOCATION_BONUS, ctx.cat);
        if bonus <= 0.0 {
            return score;
        }
        score + bonus
    }

    fn name(&self) -> &'static str {
        "tradition"
    }
}

// ---------------------------------------------------------------------------
// FoxTerritorySuppression
// ---------------------------------------------------------------------------

/// §3.5.1 Fox-territory suppression. Port of the retiring
/// `scoring.rs:1009–1027` inline block.
///
/// **Trigger:** `fox_scent_level > fox_scent_suppression_threshold`
/// (default `0.3`).
///
/// **Transform:** two shapes, keyed by DSE id:
/// - **Hunt / Explore / Forage / Patrol / Wander** — multiplicative
///   damp: `score *= (1 − suppression).max(0.0)` where
///   `suppression = ((fox_scent − threshold) / (1 − threshold)) ×
///   fox_scent_suppression_scale` (scale `0.8`).
/// - **Flee** — additive boost: `score += suppression × 0.5` (spec
///   §3.5.3 item 2). The secondary Flee boost is part of this
///   modifier's transform, not a separate impl — keeps the
///   "fox-scent response" as one registered pass.
///
/// **Applies to:** Hunt / Explore / Forage / Patrol / Wander (damp) +
/// Flee (boost).
///
/// **Gate semantics:** the multiplicative damp is naturally safe on
/// `score <= 0` (0 × anything = 0), so no short-circuit is needed on
/// the damp DSEs. The Flee additive boost *is* a gated boost, but the
/// inline block applied it unconditionally once the fox-scent
/// threshold was exceeded — this port preserves that by not
/// short-circuiting on Flee's `score <= 0`. Flee's outer gate
/// (`has_threat_nearby || safety < flee_safety_threshold`) already
/// keeps it out of scoring when the modifier wouldn't make sense.
pub struct FoxTerritorySuppression {
    threshold: f32,
    scale: f32,
    flee_boost_scale: f32,
}

impl FoxTerritorySuppression {
    /// `flee_boost_scale` is the spec's `0.5` coefficient on the Flee
    /// additive boost — hard-coded rather than promoted to
    /// `ScoringConstants` because no other scoring site reads it and
    /// the §3.5.3 discovery commentary calls out "already invisible
    /// in §2.3's original matrix row; §3.5.1 now names it
    /// explicitly." If balance work surfaces a reason to promote it,
    /// add a dedicated `fox_scent_flee_boost_scale` constant.
    const FLEE_BOOST_SCALE: f32 = 0.5;

    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.fox_scent_suppression_threshold,
            scale: sc.fox_scent_suppression_scale,
            flee_boost_scale: Self::FLEE_BOOST_SCALE,
        }
    }

    fn suppression(&self, fox_scent: f32) -> f32 {
        if fox_scent <= self.threshold {
            return 0.0;
        }
        // Normalize `(fox_scent - threshold) / (1 - threshold)` into
        // `[0, 1]` above the threshold, then scale.
        ((fox_scent - self.threshold) / (1.0 - self.threshold)) * self.scale
    }
}

impl ScoreModifier for FoxTerritorySuppression {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        // Early-out on DSEs the modifier doesn't touch — avoids the
        // fox-scent scalar fetch on every Eat / Sleep / Coordinate /
        // etc. apply.
        let is_damped = matches!(dse_id.0, HUNT | EXPLORE | FORAGE | PATROL | WANDER);
        let is_flee = dse_id.0 == FLEE;
        if !is_damped && !is_flee {
            return score;
        }

        let fox_scent = fetch(FOX_SCENT_LEVEL, ctx.cat);
        let suppression = self.suppression(fox_scent);
        if suppression <= 0.0 {
            return score;
        }

        if is_flee {
            score + suppression * self.flee_boost_scale
        } else {
            score * (1.0 - suppression).max(0.0)
        }
    }

    fn name(&self) -> &'static str {
        "fox_territory_suppression"
    }
}

// ---------------------------------------------------------------------------
// CorruptionTerritorySuppression
// ---------------------------------------------------------------------------

/// §3.5.1 Corruption-territory suppression. Port of the retiring
/// `scoring.rs:1031–1040` inline block.
///
/// **Trigger:** `tile_corruption > corruption_suppression_threshold`
/// (default `0.3`).
///
/// **Transform:** multiplicative damp with the same shape as
/// `FoxTerritorySuppression`:
/// `score *= (1 − suppression).max(0.0)` where
/// `suppression = ((tile_corruption − threshold) / (1 − threshold)) ×
/// corruption_suppression_scale` (scale `0.6`).
///
/// **Applies to:** `explore`, `wander`, `idle` per §3.5.2. The
/// asymmetry vs. fox-suppression (Idle is damped; Hunt is not) is
/// intentional — corruption expresses metaphysical malaise, not prey
/// flight.
pub struct CorruptionTerritorySuppression {
    threshold: f32,
    scale: f32,
}

impl CorruptionTerritorySuppression {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.corruption_suppression_threshold,
            scale: sc.corruption_suppression_scale,
        }
    }
}

impl ScoreModifier for CorruptionTerritorySuppression {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, EXPLORE | WANDER | IDLE) {
            return score;
        }
        let corruption = fetch(TILE_CORRUPTION, ctx.cat);
        if corruption <= self.threshold {
            return score;
        }
        let suppression = ((corruption - self.threshold) / (1.0 - self.threshold)) * self.scale;
        score * (1.0 - suppression).max(0.0)
    }

    fn name(&self) -> &'static str {
        "corruption_territory_suppression"
    }
}

// ---------------------------------------------------------------------------
// Default pipeline builder
// ---------------------------------------------------------------------------

/// Build the full §3.5 modifier pipeline. Registration order mirrors
/// the retiring inline `score_actions:576–1040` block order:
///
/// 1. Three corruption-response emergency bonuses
///    (`WardCorruptionEmergency`, `CleanseEmergency`, `SensedRotBoost`)
///    — ported in Phase 4.2. These previously ran inside each sibling
///    DSE's scoring path (`score_dse_by_id → evaluate_single`), so in
///    pre-Phase-4.2 code they ran *before* the inline §3.5 block.
/// 2. Seven §3.5.1 foundational modifiers — `Pride`,
///    `IndependenceSolo`, `IndependenceGroup`, `Patience`,
///    `Tradition`, `FoxTerritorySuppression`,
///    `CorruptionTerritorySuppression`. These previously ran as
///    imperative passes after per-DSE scoring in `score_actions`,
///    matching the order they're registered in here.
///
/// Emergency bonuses are additive and mutually order-invariant (each
/// applies to a disjoint DSE-id slice); the §3.5.1 modifiers mix
/// additive, subtractive, and multiplicative transforms — the
/// registered order here (additive bonuses before multiplicative
/// damps) matches the retiring inline block so a multi-axis DSE like
/// `hunt` receives Pride + Independence-solo boosts *before* the
/// fox-scent multiplicative damp, keeping the post-pipeline final
/// score within the Phase 4a noise band.
///
/// Mirror sites — `src/plugins/simulation.rs`, `src/main.rs`
/// `setup_world` + `run_new_game`, save-load restore — each call
/// this helper to produce the same pipeline shape.
pub fn default_modifier_pipeline(sc: &ScoringConstants) -> ModifierPipeline {
    let mut pipeline = ModifierPipeline::new();
    pipeline.push(Box::new(WardCorruptionEmergency::new(sc)));
    pipeline.push(Box::new(CleanseEmergency::new(sc)));
    pipeline.push(Box::new(SensedRotBoost::new(sc)));
    pipeline.push(Box::new(Pride::new(sc)));
    pipeline.push(Box::new(IndependenceSolo::new(sc)));
    pipeline.push(Box::new(IndependenceGroup::new(sc)));
    pipeline.push(Box::new(Patience::new(sc)));
    pipeline.push(Box::new(Tradition::new()));
    pipeline.push(Box::new(FoxTerritorySuppression::new(sc)));
    pipeline.push(Box::new(CorruptionTerritorySuppression::new(sc)));
    pipeline
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Position;

    fn test_ctx() -> (Entity, EvalCtx<'static>) {
        // Static closures satisfy `EvalCtx<'ctx>` lifetime where we don't
        // need real per-tick access.
        static MARKER: fn(&str, Entity) -> bool = |_, _| false;
        static SAMPLE: fn(&str, Position) -> f32 = |_, _| 0.0;
        let entity = Entity::from_raw_u32(1).unwrap();
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &SAMPLE,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        (entity, ctx)
    }

    #[test]
    fn ward_emergency_skips_non_ward_dses() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        // Not a ward DSE — no transform.
        let out = modifier.apply(DseId("eat"), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ward_emergency_fires_on_herbcraft_ward_when_corruption_present() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 0.8,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_WARD), 0.3, &ctx, &fetch);
        // 0.3 + 1.0 × 0.8 = 1.1
        assert!((out - 1.1).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn ward_emergency_respects_gather_narrow_gate() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        // Gather fires only if has_herbs_nearby && !has_ward_herbs &&
        // thornbriar_available.
        let without_thornbriar = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            HAS_HERBS_NEARBY => 1.0,
            HAS_WARD_HERBS => 0.0,
            THORNBRIAR_AVAILABLE => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_GATHER), 0.4, &ctx, &without_thornbriar);
        assert!((out - 0.4).abs() < 1e-6, "no thornbriar ⇒ no boost");

        let with_gate_open = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            HAS_HERBS_NEARBY => 1.0,
            HAS_WARD_HERBS => 0.0,
            THORNBRIAR_AVAILABLE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_GATHER), 0.4, &ctx, &with_gate_open);
        assert!((out - 1.4).abs() < 1e-6, "gate open ⇒ boost; got {out}");
    }

    #[test]
    fn ward_emergency_skips_when_no_territory_corruption() {
        let modifier = WardCorruptionEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            WARD_DEFICIT => 1.0,
            TERRITORY_MAX_CORRUPTION => 0.0,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HERBCRAFT_WARD), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn cleanse_emergency_fires_on_both_cleanse_dses() {
        let modifier = CleanseEmergency { bonus: 0.6 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TERRITORY_MAX_CORRUPTION => 0.4,
            MASLOW_L2_SUPPRESSION => 0.5,
            _ => 0.0,
        };
        for dse in [MAGIC_CLEANSE, MAGIC_COLONY_CLEANSE] {
            let out = modifier.apply(DseId(dse), 0.2, &ctx, &fetch);
            assert!((out - (0.2 + 0.6 * 0.5)).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn cleanse_emergency_does_not_resurrect_suppressed_dse() {
        // Outer scoring gate suppressed the DSE (e.g. cleanse on non-
        // corrupted tile → score = 0). The modifier must not revive it.
        let modifier = CleanseEmergency { bonus: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TERRITORY_MAX_CORRUPTION => 0.5,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(MAGIC_CLEANSE), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn sensed_rot_scales_with_nearby_level() {
        let modifier = SensedRotBoost { scale: 1.0 };
        let (_, ctx) = test_ctx();
        let make_fetch = |nearby: f32| {
            move |name: &str, _: Entity| match name {
                NEARBY_CORRUPTION_LEVEL => nearby,
                MASLOW_L2_SUPPRESSION => 1.0,
                _ => 0.0,
            }
        };
        // Below 0.1 floor → no boost.
        let f = make_fetch(0.05);
        let out = modifier.apply(DseId(MAGIC_DURABLE_WARD), 0.3, &ctx, &f);
        assert!((out - 0.3).abs() < 1e-6);

        // 0.5 nearby corruption → 0.3 + 1.0 × 0.5 × 1.0 = 0.8
        let f = make_fetch(0.5);
        let out = modifier.apply(DseId(MAGIC_DURABLE_WARD), 0.3, &ctx, &f);
        assert!((out - 0.8).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn sensed_rot_skips_unrelated_dses() {
        let modifier = SensedRotBoost { scale: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            NEARBY_CORRUPTION_LEVEL => 0.9,
            MASLOW_L2_SUPPRESSION => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId("hunt"), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // §3.5.1 foundational modifiers
    // -----------------------------------------------------------------------

    #[test]
    fn pride_fires_on_hunt_when_respect_is_low() {
        let modifier = Pride {
            respect_threshold: 0.5,
            bonus: 0.1,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            RESPECT => 0.2,
            PRIDE => 0.8,
            _ => 0.0,
        };
        // 0.4 + 0.8 × 0.1 = 0.48
        let out = modifier.apply(DseId(HUNT), 0.4, &ctx, &fetch);
        assert!((out - 0.48).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn pride_skips_non_applicable_dses() {
        let modifier = Pride {
            respect_threshold: 0.5,
            bonus: 0.1,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            RESPECT => 0.0,
            PRIDE => 1.0,
            _ => 0.0,
        };
        // Socialize isn't in Pride's DSE list — passes through unchanged.
        let out = modifier.apply(DseId(SOCIALIZE), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn pride_skips_when_respect_above_threshold() {
        let modifier = Pride {
            respect_threshold: 0.5,
            bonus: 0.1,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            RESPECT => 0.8,
            PRIDE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.4, &ctx, &fetch);
        assert!((out - 0.4).abs() < 1e-6, "high respect ⇒ no pride boost");
    }

    #[test]
    fn pride_does_not_resurrect_zero_score() {
        let modifier = Pride {
            respect_threshold: 0.5,
            bonus: 0.1,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            RESPECT => 0.0,
            PRIDE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn independence_solo_adds_bonus_to_solo_dses() {
        let modifier = IndependenceSolo { bonus: 0.1 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            INDEPENDENCE => 0.7,
            _ => 0.0,
        };
        for dse in [EXPLORE, WANDER, HUNT] {
            // 0.5 + 0.7 × 0.1 = 0.57
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.57).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn independence_solo_skips_non_solo_dses() {
        let modifier = IndependenceSolo { bonus: 0.1 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            INDEPENDENCE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(SOCIALIZE), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn independence_group_subtracts_penalty() {
        let modifier = IndependenceGroup { penalty: 0.1 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            INDEPENDENCE => 0.7,
            _ => 0.0,
        };
        for dse in [SOCIALIZE, COORDINATE, MENTOR] {
            // 0.5 − 0.7 × 0.1 = 0.43
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.43).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn independence_group_clamps_negative_to_zero() {
        // High independence + low group score → inline clamp at 0.
        let modifier = IndependenceGroup { penalty: 1.0 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            INDEPENDENCE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(SOCIALIZE), 0.3, &ctx, &fetch);
        assert_eq!(out, 0.0, "clamped to 0 rather than going negative");
    }

    #[test]
    fn patience_fires_on_constituent_dse_of_active_disposition() {
        let modifier = Patience { bonus: 0.15 };
        let (_, ctx) = test_ctx();
        // Hunting disposition (ordinal 2) → Hunt is constituent.
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            PATIENCE => 0.8,
            _ => 0.0,
        };
        // 0.4 + 0.8 × 0.15 = 0.52
        let out = modifier.apply(DseId(HUNT), 0.4, &ctx, &fetch);
        assert!((out - 0.52).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn patience_skips_non_constituent_dse() {
        let modifier = Patience { bonus: 0.15 };
        let (_, ctx) = test_ctx();
        // Hunting disposition (ordinal 2) — Socialize isn't constituent.
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            PATIENCE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(SOCIALIZE), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn patience_skips_when_no_active_disposition() {
        let modifier = Patience { bonus: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 0.0,
            PATIENCE => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        assert!((out - 0.5).abs() < 1e-6);
    }

    #[test]
    fn patience_crafting_applies_to_all_sibling_dses() {
        // Crafting disposition (ordinal 8) spans Herbcraft + PracticeMagic
        // + Cook siblings. The composite `max(siblings) + Δ = max + Δ`
        // identity means the outer Action::Herbcraft/PracticeMagic
        // scores absorb the same Δ the inline block added.
        let modifier = Patience { bonus: 0.1 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 8.0,
            PATIENCE => 1.0,
            _ => 0.0,
        };
        for dse in [
            HERBCRAFT_GATHER,
            HERBCRAFT_PREPARE,
            HERBCRAFT_WARD,
            MAGIC_SCRY,
            MAGIC_DURABLE_WARD,
            MAGIC_CLEANSE,
            MAGIC_COLONY_CLEANSE,
            MAGIC_HARVEST,
            MAGIC_COMMUNE,
            COOK,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.6).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn tradition_applies_to_every_dse_unfiltered() {
        // §3.5.3 item 1: Tradition's inline loop ignores the DSE — the
        // port preserves that. The fix is filed separately as a
        // balance-methodology-scoped change.
        let modifier = Tradition::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TRADITION_LOCATION_BONUS => 0.05,
            _ => 0.0,
        };
        for dse in [
            EAT, SLEEP, HUNT, FORAGE, SOCIALIZE, EXPLORE, WANDER, FLEE, FIGHT, PATROL, BUILD,
            FARM, COORDINATE, MENTOR, MATE, COOK, CARETAKE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.4, &ctx, &fetch);
            assert!((out - 0.45).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn tradition_skips_when_bonus_is_zero() {
        // Live-production path: caller sets bonus to 0.0 in goap.rs,
        // so the modifier is a no-op on every DSE.
        let modifier = Tradition::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TRADITION_LOCATION_BONUS => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.4, &ctx, &fetch);
        assert!((out - 0.4).abs() < 1e-6);
    }

    #[test]
    fn fox_suppression_damps_hunt_when_scent_above_threshold() {
        // Threshold 0.3, scale 0.8, fox_scent 0.8 →
        // suppression = (0.8 − 0.3) / (1 − 0.3) × 0.8 ≈ 0.5714.
        // Hunt score 0.5 → 0.5 × (1 − 0.5714) ≈ 0.2143.
        let modifier = FoxTerritorySuppression {
            threshold: 0.3,
            scale: 0.8,
            flee_boost_scale: 0.5,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOX_SCENT_LEVEL => 0.8,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        let expected = 0.5_f32 * (1.0_f32 - ((0.8 - 0.3) / (1.0 - 0.3)) * 0.8).max(0.0);
        assert!((out - expected).abs() < 1e-6, "got {out}, expected {expected}");
    }

    #[test]
    fn fox_suppression_boosts_flee_additively() {
        // §3.5.3 item 2: Flee gets an additive boost, not a damp.
        let modifier = FoxTerritorySuppression {
            threshold: 0.3,
            scale: 0.8,
            flee_boost_scale: 0.5,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOX_SCENT_LEVEL => 0.8,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        let suppression = ((0.8 - 0.3) / (1.0 - 0.3)) * 0.8;
        let expected = 0.5 + suppression * 0.5;
        assert!((out - expected).abs() < 1e-6, "got {out}, expected {expected}");
    }

    #[test]
    fn fox_suppression_skips_when_below_threshold() {
        let modifier = FoxTerritorySuppression {
            threshold: 0.3,
            scale: 0.8,
            flee_boost_scale: 0.5,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOX_SCENT_LEVEL => 0.1,
            _ => 0.0,
        };
        for dse in [HUNT, EXPLORE, FORAGE, PATROL, WANDER, FLEE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.5).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn fox_suppression_skips_non_applicable_dses() {
        // Eat, Fight, Socialize, etc. are not touched by fox-suppression
        // even when scent is high — only the damped five + Flee.
        let modifier = FoxTerritorySuppression {
            threshold: 0.3,
            scale: 0.8,
            flee_boost_scale: 0.5,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOX_SCENT_LEVEL => 0.9,
            _ => 0.0,
        };
        for dse in [EAT, SLEEP, SOCIALIZE, FIGHT, BUILD, IDLE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.5).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn corruption_suppression_damps_explore_wander_idle() {
        let modifier = CorruptionTerritorySuppression {
            threshold: 0.3,
            scale: 0.6,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TILE_CORRUPTION => 0.8,
            _ => 0.0,
        };
        for dse in [EXPLORE, WANDER, IDLE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            let expected = 0.5_f32 * (1.0_f32 - ((0.8 - 0.3) / (1.0 - 0.3)) * 0.6).max(0.0);
            assert!((out - expected).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn corruption_suppression_skips_non_applicable_dses() {
        // Hunt is damped by fox-suppression but NOT by corruption —
        // the asymmetry is intentional (spec §3.5.2 observation).
        let modifier = CorruptionTerritorySuppression {
            threshold: 0.3,
            scale: 0.6,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TILE_CORRUPTION => 0.9,
            _ => 0.0,
        };
        for dse in [HUNT, EAT, SLEEP, SOCIALIZE, FIGHT, FLEE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.5).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn corruption_suppression_skips_when_below_threshold() {
        let modifier = CorruptionTerritorySuppression {
            threshold: 0.3,
            scale: 0.6,
        };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            TILE_CORRUPTION => 0.1,
            _ => 0.0,
        };
        for dse in [EXPLORE, WANDER, IDLE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!((out - 0.5).abs() < 1e-6, "dse {dse}: got {out}");
        }
    }

    #[test]
    fn constituent_dses_for_ordinal_matches_disposition_kind_constituent_actions() {
        // Sanity-check: the Patience modifier's ordinal→DSE-id table
        // must stay aligned with `DispositionKind::constituent_actions()`.
        // If this regresses, the Patience bonus applies to the wrong
        // DSEs and the balance of the retiring inline block is broken.
        use crate::ai::Action;
        use crate::components::disposition::DispositionKind;

        fn action_to_dse_ids(action: Action) -> &'static [&'static str] {
            match action {
                Action::Eat => &[EAT],
                Action::Sleep => &[SLEEP],
                Action::Hunt => &[HUNT],
                Action::Forage => &[FORAGE],
                Action::Socialize => &[SOCIALIZE],
                Action::Groom => &[GROOM_SELF, GROOM_OTHER],
                Action::Explore => &[EXPLORE],
                Action::Wander => &[WANDER],
                Action::Flee => &[FLEE],
                Action::Fight => &[FIGHT],
                Action::Patrol => &[PATROL],
                Action::Build => &[BUILD],
                Action::Farm => &[FARM],
                Action::Herbcraft => &[HERBCRAFT_GATHER, HERBCRAFT_PREPARE, HERBCRAFT_WARD],
                Action::PracticeMagic => &[
                    MAGIC_SCRY,
                    MAGIC_DURABLE_WARD,
                    MAGIC_CLEANSE,
                    MAGIC_COLONY_CLEANSE,
                    MAGIC_HARVEST,
                    MAGIC_COMMUNE,
                ],
                Action::Coordinate => &[COORDINATE],
                Action::Mentor => &[MENTOR],
                Action::Mate => &[MATE],
                Action::Caretake => &[CARETAKE],
                Action::Cook => &[COOK],
                Action::Idle => &[IDLE],
            }
        }

        for (ordinal, kind) in (1..).zip(DispositionKind::ALL.iter().copied()) {
            let expected: Vec<&'static str> = kind
                .constituent_actions()
                .iter()
                .flat_map(|a| action_to_dse_ids(*a).iter().copied())
                .collect();
            let actual = constituent_dses_for_ordinal(ordinal as f32)
                .expect("every variant maps");
            let actual_sorted = {
                let mut v = actual.to_vec();
                v.sort();
                v
            };
            let expected_sorted = {
                let mut v = expected.clone();
                v.sort();
                v
            };
            assert_eq!(
                actual_sorted, expected_sorted,
                "mismatch for disposition {kind:?} (ordinal {ordinal})"
            );
        }

        // None ordinal maps to no constituents.
        assert!(constituent_dses_for_ordinal(0.0).is_none());
    }

    #[test]
    fn default_pipeline_registers_ten_modifiers() {
        // The three Phase 4.2 emergency modifiers plus the seven
        // §3.5.1 foundational modifiers.
        let sc = ScoringConstants::default();
        let pipeline = default_modifier_pipeline(&sc);
        assert_eq!(pipeline.len(), 10, "expected 10 registered modifiers");
    }
}
