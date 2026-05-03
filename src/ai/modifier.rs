//! §3.5 post-scoring modifiers (`docs/systems/ai-substrate-refactor.md`).
//!
//! A `ScoreModifier` is a pure post-composition pass: given a DSE's id,
//! its gated score, the cat's eval context, and the canonical scalar
//! fetcher, it returns a transformed score. The pipeline applies every
//! registered modifier in registration order — ch 13 §"Layered
//! Weighting Models / Propagation of Change" calls this the filter-stage
//! shape.
//!
//! Phase 4.2 first ported three corruption-response emergency-bonus
//! modifiers (`WardCorruptionEmergency`, `CleanseEmergency`,
//! `SensedRotBoost`) out of the inline `score_actions` block into this
//! layer. §13.1 retired all three once the per-axis Logistic curves
//! that absorb their contribution landed in the corresponding sibling
//! DSEs (`herbcraft_gather`, `herbcraft_ward`, `magic_durable_ward`,
//! `magic_cleanse`, `magic_colony_cleanse`) — see §2.3 "Retired
//! constants" rows 4–6 for the shape-unification rationale.
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

// §3.5.1 modifier trigger inputs.
const RESPECT: &str = "respect";
const PRIDE: &str = "pride";
const INDEPENDENCE: &str = "independence";
const PATIENCE: &str = "patience";
const TRADITION_LOCATION_BONUS: &str = "tradition_location_bonus";
const FOX_SCENT_LEVEL: &str = "fox_scent_level";
const TILE_CORRUPTION: &str = "tile_corruption";
/// §3.5.1 `StockpileSatiation` Modifier trigger input. The cat-side
/// scalar `ctx_scalars` already publishes is `food_scarcity` =
/// `1.0 - food_fraction` (see `scoring.rs::ctx_scalars`); the modifier
/// fetches scarcity and inverts to `food_fraction` for the threshold
/// check. Centralising the constant here keeps the
/// modifier-trigger surface alongside its peers.
const FOOD_SCARCITY: &str = "food_scarcity";
/// Ticket 088 — `BodyDistressPromotion` Modifier trigger input. The
/// 087 perception substrate computes
/// `interoception::body_distress_composite` as max of
/// {hunger_urgency, energy_deficit, thermal_deficit, health_deficit}
/// and `scoring::ctx_scalars` publishes the value under this key.
const BODY_DISTRESS_COMPOSITE: &str = "body_distress_composite";
/// Ticket 047 — `AcuteHealthAdrenalineFlee` trigger input.
/// `health_deficit` = `1 - health.current/health.max`, already published
/// by `scoring::ctx_scalars`. Read directly (not through the
/// `body_distress_composite` `max()` flatten) so the lurch fires on
/// injury alone, even when other axes are quiet — matching the
/// predator-injury scenario the legacy `CriticalHealth` interrupt was
/// built to catch.
const HEALTH_DEFICIT: &str = "health_deficit";
/// Ticket 102 — `AcuteHealthAdrenalineFight` viability gate input.
/// `escape_viability` ∈ [0, 1]: 0 = cornered/dependent-burdened, 1 =
/// open terrain with no escape penalty. Authored by 103's pure helper
/// in `interoception::escape_viability` and published into
/// `ScoringContext.escape_viability` / `ctx_scalars`. Returns 1.0 (no
/// gate trip) when no threat is present; downstream Fight branch
/// short-circuits there because there's nothing to fight.
const ESCAPE_VIABILITY: &str = "escape_viability";
/// Ticket 106 — `HungerUrgency` Modifier trigger input. Already
/// published by `scoring::ctx_scalars` as `(1 - needs.hunger).clamp(0,1)`.
/// Read directly (not through `body_distress_composite`) so hunger alone
/// fires the lift even when other axes are quiet — matching the
/// slow-starvation regime where the legacy `Starvation` interrupt
/// fires. The substrate threshold (0.6) engages well before the legacy
/// interrupt (`hunger < 0.15` ⇒ urgency > 0.85), giving the contest
/// time to re-rank Eat / Hunt / Forage before crisis.
const HUNGER_URGENCY: &str = "hunger_urgency";
/// Ticket 107 — `ExhaustionPressure` Modifier trigger input. Already
/// published by `scoring::ctx_scalars` as `(1 - needs.energy).clamp(0,1)`.
/// Read directly so exhaustion alone fires the Sleep / GroomSelf lift
/// even when `body_distress_composite` is below 088's threshold. Engages
/// before the legacy `Exhaustion` interrupt (`energy < 0.10` ⇒
/// energy_deficit > 0.90).
const ENERGY_DEFICIT: &str = "energy_deficit";
/// Ticket 110 — `ThermalDistress` Modifier trigger input. Already
/// published by `scoring::ctx_scalars` as the cat's distance from the
/// thermal comfort band. Read directly (not through composite) so cold
/// alone fires the shelter-seeking lift on Sleep. No legacy interrupt
/// to retire here — pure perception-richness lever per ticket §Why.
const THERMAL_DEFICIT: &str = "thermal_deficit";
/// Ticket 108 — `ThreatProximityAdrenalineFlee` Modifier trigger
/// input. Adrenaline lurch on **rising** threat proximity (the cat
/// noticed danger getting worse this tick), not on a steady-state
/// scalar — adrenaline is about change-detection, not absolute level.
///
/// **Phase 1 stub:** this scalar is currently published as 0.0 from
/// `scoring::ctx_scalars`. Computing the actual derivative requires a
/// `PrevSafetyDeficit(f32)` per-cat Component plus a per-tick update
/// system (snapshot `safety_deficit_now → prev` after the scoring
/// pass runs). That ECS plumbing lands in the same commit that
/// activates 108's lift (default 0.0 → swept-validated value), per
/// the "ship inert until verified sufficient" 047 playbook. With
/// the lift at 0.0 (this commit), the modifier never fires
/// regardless of scalar value, so the stub is bit-identical to the
/// pre-Wave-1 baseline.
const THREAT_PROXIMITY_DERIVATIVE: &str = "threat_proximity_derivative";
/// Ticket 109 — `IntraspeciesConflictResponseFlight` Modifier trigger
/// input. The social-status pressure scalar — distinct from physical
/// body distress and from predator threat. Composes status differential
/// vs the nearest cat with a proximity / intrusion factor.
///
/// **Phase 1 stub:** published as 0.0 from `scoring::ctx_scalars`. The
/// v1 composition `(status_diff_to_nearest_cat × proximity_factor)`
/// requires (a) a defensible status-differential signal — no explicit
/// dominance hierarchy exists yet; `needs.respect` and bond strength
/// are candidate proxies — and (b) per-cat nearest-cat resolution
/// during scoring. Both land with the lift activation (Phase 3 spec).
/// With the lift at 0.0, the stub is bit-identical to baseline.
const SOCIAL_STATUS_DISTRESS: &str = "social_status_distress";
const ACTIVE_DISPOSITION_ORDINAL: &str = "active_disposition_ordinal";
/// §075 — `CommitmentTenure` Modifier. Drift between this name and
/// `plan_substrate::COMMITMENT_TENURE_INPUT` (the canonical
/// `&'static str` the substrate publishes) becomes a build-time error
/// via the `const _: () = assert!(...)` below — see the
/// `CommitmentTenure` doc-comment for the rationale.
const COMMITMENT_TENURE_PROGRESS: &str =
    crate::systems::plan_substrate::COMMITMENT_TENURE_INPUT;

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
/// Ticket 104 — Hide/Freeze DSE id. The "remain still and hope"
/// predator-avoidance valence; companion to FLEE / FIGHT. Targeted
/// by ticket 105 (`AcuteHealthAdrenalineFreeze`) — also will be
/// lifted by ticket 142 (`IntraspeciesConflictResponseFreeze`) once
/// it lands.
const HIDE: &str = "hide";
const HERBCRAFT_PREPARE: &str = "herbcraft_prepare";
const MAGIC_SCRY: &str = "magic_scry";
const MAGIC_HARVEST: &str = "magic_harvest";
const MAGIC_COMMUNE: &str = "magic_commune";
const COORDINATE: &str = "coordinate";
const MENTOR: &str = "mentor";
const MATE: &str = "mate";
const CARETAKE: &str = "caretake";
const IDLE: &str = "idle";

/// Ticket 088 — the "self-care" DSE class lifted by
/// `BodyDistressPromotion` when `body_distress_composite` is high.
/// Authored as a `&[&str]` constant so the class membership is
/// grep-discoverable; the modifier's `apply` matches the same set
/// inline via `matches!` for compile-time efficiency. The two must
/// stay in sync — the BodyDistressPromotion test suite iterates this
/// constant against the matches! pattern's effective behavior, so
/// drift is caught at test time. Note: the 088 ticket lists "Rest"
/// but no `Rest` DSE exists — `Sleep` covers the energy-recovery
/// role.
#[cfg_attr(not(test), allow(dead_code))]
const SELF_CARE_DSES: &[&str] = &[FLEE, SLEEP, EAT, HUNT, FORAGE, GROOM_SELF];

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
/// (or its outer scoring-layer gate) suppressed — preserving the
/// gated-additive-boost pattern Phase 4.2 established for its three
/// corruption-emergency modifiers (since retired in §13.1).
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
    // against known variants. Values outside `[0, 13]` are treated as
    // "no active disposition" defensively. 150 R5a appends `Eating` as
    // ordinal 13 so existing 1..=12 ordinals stay stable.
    let rounded = ordinal.round() as i32;
    match rounded {
        0 => None,
        // 150 R5a: Resting drops EAT (which moved to the new Eating
        // disposition at ordinal 13). Sleep + Groom remain.
        1 => Some(&[SLEEP, GROOM_SELF, GROOM_OTHER]),
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
        // 150 R5a: Eating → Eat. Single-action disposition; the
        // Patience and CommitmentTenure lifts apply to the Eat DSE
        // alone while the cat is committed to Eating.
        13 => Some(&[EAT]),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// CommitmentTenure
// ---------------------------------------------------------------------------

/// §3.5.1 commitment-tenure hysteresis (ticket 075, parent 071).
///
/// **Why:** when two dispositions score within ε of each other, the
/// IAUS softmax oscillates every tick — picking different incumbents
/// from one re-evaluation to the next. Oscillation churns plans,
/// defeats single-disposition commitment, and wastes the score
/// economy. Audit gap #5 in the planning-substrate hardening sub-epic
/// (`docs/open-work/tickets/071-planning-substrate-hardening.md`).
///
/// **Trigger:** `commitment_tenure_progress < 1.0` AND the cat has an
/// active disposition. `commitment_tenure_progress` is published by
/// `scoring::commitment_tenure_progress` (see its doc-comment) — it
/// reads `Disposition::disposition_started_tick` (written by
/// `plan_substrate::record_disposition_switch`, ticket 072) and the
/// current tick, returning the fraction of `min_disposition_tenure_ticks`
/// that has elapsed since the last switch. `1.0` means the window has
/// closed and no lift is applied.
///
/// **Transform:** `score += oscillation_score_lift` — flat additive,
/// applied to every DSE that is a constituent action of the cat's
/// active disposition. Same membership table as [`Patience`]; reuses
/// [`constituent_dses_for_ordinal`] so the table stays single-sourced.
///
/// **Applies to:** dynamic per the active-disposition's constituent
/// list (see [`constituent_dses_for_ordinal`]). Cats with no active
/// disposition see no lift on any DSE — the
/// `commitment_tenure_progress` scalar reports `1.0` in that case
/// and the modifier short-circuits before the table lookup.
///
/// **Architectural guardrail (071's "machined gears" doctrine):**
/// anti-oscillation is a `Modifier` in the §3.5.1 pipeline — additive
/// lift on the incumbent disposition's constituent DSEs so it wins
/// the IAUS pick during the tenure window through the natural softmax
/// economy. **NOT** a switch-gate that overrides the IAUS pick.
/// Inspectable in the same modifier-pipeline trace as `Pride` /
/// `Patience` via `ModifierPipeline::apply_with_trace`.
///
/// **Stacking with Patience:** both modifiers register and apply to
/// constituent DSEs of the active disposition. The combined lift is
/// `patience_commitment_bonus * personality.patience +
/// oscillation_score_lift` while the cat is inside the tenure window.
/// The two are intentionally independent: Patience encodes the cat's
/// personality-driven commitment to its current behavior, whereas
/// `CommitmentTenure` is a flat anti-oscillation pad that doesn't
/// depend on personality. Conservative defaults
/// (`oscillation_score_lift = 0.10` <
/// `patience_commitment_bonus = 0.15`) keep the two roughly balanced.
///
/// **Gated-boost contract:** returns `score` unchanged on score
/// `<= 0` so the additive lift doesn't resurrect a DSE the Maslow
/// pre-gate (or its outer scoring-layer gate) suppressed. Matches the
/// stance of `Pride` / `Patience` / `IndependenceSolo`.
pub struct CommitmentTenure {
    lift: f32,
}

impl CommitmentTenure {
    pub fn new(sc: &crate::resources::sim_constants::SimConstants) -> Self {
        Self {
            lift: sc.disposition.oscillation_score_lift,
        }
    }
}

impl ScoreModifier for CommitmentTenure {
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
        // Tenure-window check: progress < 1.0 means we're still inside
        // the window (saturates at 1.0 once the window has elapsed).
        // The progress producer also returns 1.0 when the cat has no
        // active disposition, so this single comparison subsumes the
        // "no active disposition" short-circuit.
        let progress = fetch(COMMITMENT_TENURE_PROGRESS, ctx.cat);
        if progress >= 1.0 {
            return score;
        }
        // Membership check: only DSEs in the active disposition's
        // constituent list receive the lift. Reuses Patience's
        // ordinal→DSE-id table so the two modifiers stay aligned;
        // when `DispositionKind::constituent_actions()` changes, the
        // single table in `constituent_dses_for_ordinal` updates and
        // both modifiers track it.
        let ordinal = fetch(ACTIVE_DISPOSITION_ORDINAL, ctx.cat);
        let Some(constituents) = constituent_dses_for_ordinal(ordinal) else {
            return score;
        };
        if !constituents.contains(&dse_id.0) {
            return score;
        }
        score + self.lift
    }

    fn name(&self) -> &'static str {
        "commitment_tenure"
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
// StockpileSatiation
// ---------------------------------------------------------------------------

/// §3.5.1 stockpile-satiation suppression — companion to
/// `FoxTerritorySuppression` for the food economy. Ticket 094.
///
/// **Why:** Hunt and Forage have *no* spatial axis — they score the
/// same anywhere on the map. Eat composes hunger × stores-distance
/// under `CompensatedProduct`, so its score collapses multiplicatively
/// with distance to stores. A bold/diligent cat at the territory
/// boundary near forageable terrain will keep electing
/// Hunt/Forage even when the colony's stockpile is brimming, because
/// the personal-hunger axis on Hunt/Forage stays high while Eat's
/// long-range score is gated by the spatial multiplier. Result:
/// the colony hauls food into Stores constantly but rarely consumes
/// it; hunger silently decays toward starvation. This Modifier
/// re-balances the contest by damping the *acquisition* DSEs
/// proportional to colony abundance — when the stockpile is full the
/// score "I should go get more food" naturally decays, leaving the
/// IAUS contest in Eat's favor at any range.
///
/// **Trigger:** `food_fraction > stockpile_satiation_threshold`
/// (default `0.5`). The cat-side scalar surface publishes
/// `food_scarcity = 1.0 - food_fraction`; the modifier fetches
/// `food_scarcity` and inverts.
///
/// **Transform:** multiplicative damp on Hunt and Forage — same shape
/// as `FoxTerritorySuppression`'s damp branch:
///   `score *= (1 − suppression).max(0.0)` where
///   `suppression = ((food_fraction − threshold) / (1 − threshold)) ×
///   stockpile_satiation_scale`.
///
/// **Applies to:** `hunt`, `forage`. *Not* `eat` (the destination of
/// the contest), *not* `cook` (cooks raw food the colony already has,
/// so abundance is its *reason* to fire), *not* `farm`/`herbcraft_*`
/// (different ecological axis — herb/ward demand, ticket 084), *not*
/// the Sleep/Groom/etc self-care class.
///
/// **Desperation case preserved:** at `food_fraction = 0`,
/// `suppression = 0` (clamped via the threshold gate), so a starving
/// colony's Hunt/Forage scores are unchanged. The modifier is
/// asymmetric — it only damps when the stockpile is full enough that
/// going to acquire more is *unnecessary*, never when acquisition is
/// urgent.
///
/// **Composition with later Modifiers:** when ticket 088's
/// `BodyDistressPromotion` lands (additive lift on the self-care
/// class incl. Eat), the two compose cleanly: the additive lift on
/// Eat fires before this multiplicative damp on Hunt/Forage, so the
/// IAUS contest tilts even harder toward Eat under combined high
/// stockpile + high body distress. Registering this modifier *after*
/// the additive ones (Pride / Patience / CommitmentTenure / Tradition)
/// matches the existing convention from `FoxTerritorySuppression` /
/// `CorruptionTerritorySuppression`.
///
/// **Gate semantics:** the multiplicative damp is naturally safe on
/// `score <= 0` (0 × anything = 0), so no short-circuit is needed.
/// Mirrors `FoxTerritorySuppression`'s damp branch.
pub struct StockpileSatiation {
    threshold: f32,
    scale: f32,
}

impl StockpileSatiation {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.stockpile_satiation_threshold,
            scale: sc.stockpile_satiation_scale,
        }
    }

    /// Returns the suppression coefficient in `[0, 1]` given the
    /// colony's `food_fraction`. Below `threshold` returns 0; above,
    /// scales linearly to `scale` at `food_fraction = 1.0`.
    fn suppression(&self, food_fraction: f32) -> f32 {
        if food_fraction <= self.threshold {
            return 0.0;
        }
        ((food_fraction - self.threshold) / (1.0 - self.threshold)) * self.scale
    }
}

impl ScoreModifier for StockpileSatiation {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, HUNT | FORAGE) {
            return score;
        }
        let food_fraction = (1.0 - fetch(FOOD_SCARCITY, ctx.cat)).clamp(0.0, 1.0);
        let suppression = self.suppression(food_fraction);
        if suppression <= 0.0 {
            return score;
        }
        score * (1.0 - suppression).max(0.0)
    }

    fn name(&self) -> &'static str {
        "stockpile_satiation"
    }
}

// ---------------------------------------------------------------------------
// BodyDistressPromotion
// ---------------------------------------------------------------------------

/// §3.5.1 / Ticket 088 body-distress promotion. Companion to
/// `StockpileSatiation` for the body-state economy.
///
/// **Why:** A cat near death from accumulated injury + hunger + cold
/// can still score Fight or Guarding above Flee/Eat/Sleep when threats
/// are nearby — each self-care DSE individually competes against
/// non-self-care DSEs without knowing the body is in collapse. This
/// modifier reads 087's unified `body_distress_composite` scalar and
/// lifts the whole self-care class as a unit, so the IAUS contest
/// tilts toward body recovery before the cat dies in a replan loop
/// (the failure mode 047 documents). Additive (not multiplicative) so
/// a zero pre-score self-care DSE can be promoted above a positive
/// non-self-care competitor when distress is critical.
///
/// **Trigger:** `body_distress_composite > body_distress_promotion_threshold`
/// (default 0.7). Set above 087's `body_distress_threshold` (0.6) so
/// the marker fires first as a perception event and the modifier
/// engages later as a stronger response.
///
/// **Transform:** additive lift on every self-care DSE:
///   `score += ((distress − threshold) / (1 − threshold)) × lift_scale`
/// (lift_scale default 0.20). At distress = 1.0 every self-care DSE
/// receives the full +0.20 lift; below threshold the modifier is a
/// no-op for every DSE.
///
/// **Applies to:** Flee, Sleep, Eat, Hunt, Forage, GroomSelf — the
/// "self-care" DSE class. Roster lives in `SELF_CARE_DSES` for
/// grep-discoverability; `apply` matches the same set inline for
/// compile-time efficiency. There is no separate `Rest` DSE; Sleep
/// covers the energy-recovery role.
///
/// **Composition:** Registered with the additive bonuses (Pride /
/// IndependenceSolo / Patience / CommitmentTenure / Tradition) before
/// the multiplicative damps (Fox / Corruption / Stockpile). Under
/// combined high stockpile + high body distress, this lift on Eat
/// fires *before* `StockpileSatiation` damps Hunt/Forage — the
/// contest tilts twice toward Eat (once by lift, once by damp). The
/// 094 `StockpileSatiation` doc-comment pre-described this exact
/// composition order.
///
/// **Substrate role:** Prerequisite for ticket 047 (CriticalHealth-
/// interrupt retirement). 047 cannot safely remove its per-tick
/// interrupt branch until this modifier provides magnitude sufficient
/// to suppress non-self-care DSEs through the IAUS contest alone.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — matches the established additive-modifier convention (Pride /
/// IndependenceSolo / Patience / CommitmentTenure / Tradition). High
/// body-distress should re-rank what's already accessible to the cat,
/// not resurrect DSEs the Maslow pre-gate or outer scoring layer
/// already ruled ineligible (no food in range, no safe sleep spot,
/// etc.). Ecologically: distress doesn't conjure resources into
/// existence.
pub struct BodyDistressPromotion {
    threshold: f32,
    lift_scale: f32,
}

impl BodyDistressPromotion {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.body_distress_promotion_threshold,
            lift_scale: sc.body_distress_promotion_lift,
        }
    }

    /// Returns the additive lift in `[0, lift_scale]` given the cat's
    /// `body_distress_composite`. Below `threshold` returns 0; above,
    /// scales linearly to `lift_scale` at `distress = 1.0`.
    fn lift(&self, distress: f32) -> f32 {
        if distress <= self.threshold {
            return 0.0;
        }
        ((distress - self.threshold) / (1.0 - self.threshold)) * self.lift_scale
    }
}

impl ScoreModifier for BodyDistressPromotion {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        // Early-out on DSEs outside the self-care class — avoids the
        // body_distress scalar fetch on every Mate / Coordinate /
        // Build / etc. apply.
        if !matches!(dse_id.0, FLEE | SLEEP | EAT | HUNT | FORAGE | GROOM_SELF) {
            return score;
        }
        // Gated-boost contract: don't resurrect a DSE the Maslow
        // pre-gate or outer scoring layer suppressed.
        if score <= 0.0 {
            return score;
        }
        let distress = fetch(BODY_DISTRESS_COMPOSITE, ctx.cat).clamp(0.0, 1.0);
        let lift = self.lift(distress);
        if lift <= 0.0 {
            return score;
        }
        score + lift
    }

    fn name(&self) -> &'static str {
        "body_distress_promotion"
    }
}

// ---------------------------------------------------------------------------
// AcuteHealthAdrenalineFlee — ticket 047
// ---------------------------------------------------------------------------

/// Ticket 047 — `AcuteHealthAdrenalineFlee` Modifier. Models the
/// adrenaline / fight-or-flight lurch on injury, the substrate that
/// retires the per-tick `CriticalHealth` interrupt branch.
///
/// **Trigger:** `health_deficit >= acute_health_adrenaline_threshold`
/// (default 0.4 — aligned with `disposition.critical_health_threshold`
/// so the substrate fires in the same regime the legacy interrupt did).
///
/// **Transform:** narrow smoothstep from `threshold` to
/// `threshold + transition_width` (width 0.1) ramping to a per-DSE lift
/// magnitude. Distinct from 088's `BodyDistressPromotion` linear ramp
/// in two ways: (1) reads `health_deficit` directly rather than the
/// `body_distress_composite = max(deficits)` flatten, so injury alone
/// fires it; (2) sigmoid-style sharp onset rather than gentle ramp,
/// because adrenaline is a phase transition not a weighted preference.
///
/// **Applies to:** Flee (lift `acute_health_adrenaline_flee_lift`) and
/// Sleep (lift `acute_health_adrenaline_sleep_lift`). **Both default
/// to 0.0** so the modifier ships inert; the proposed magnitudes
/// (Flee 0.60, Sleep 0.50) are enabled via `CLOWDER_OVERRIDES` for the
/// Phase 3 hypothesis sweep, validated, then promoted to defaults in
/// Phase 4 alongside the legacy interrupt's removal. Flee is the
/// primary lurch target; Sleep is the in-pool partner because Flee is
/// filtered from the disposition softmax
/// (`scoring.rs::select_disposition_via_intention_softmax_…`).
/// The Sleep lift is what flips the disposition contest away from
/// Guarding/Crafting under injury — Sleep routes to a den, mechanically
/// expressing retreat.
///
/// **Composition:** Registered immediately after `BodyDistressPromotion`
/// in the additive section of the pipeline. Under combined high
/// composite + high health-deficit, Sleep sees both lifts add (e.g.
/// 088's +0.20 + this +0.50 = +0.70 total), strongly tilting the
/// contest. This double-stacking is intentional: composite-distress
/// names "the cat is unwell on average," while health-deficit names
/// "the cat is being injured *now*"; both lifting Sleep simultaneously
/// is the right ecological answer.
///
/// **Substrate role:** This modifier is the substrate-over-override
/// retirement of the `CriticalHealth` interrupt branch in
/// `disposition.rs:301-302` and `goap.rs:493-498`. Phase 4 of ticket
/// 047 removes those branches once this modifier is verified to flip
/// Sleep above Fight/Build/Forage at the right magnitude in the IAUS
/// contest.
///
/// **Future split (follow-on tickets):** the `Flee` valence shipped
/// here is one of three predator-response branches in the planned
/// N-valence framework — Fight (when escape is not viable but combat
/// is winnable) and Freeze (requires new Hide/Freeze DSE) follow as
/// independent tickets. Each shares the `health_deficit` scalar but
/// gates on a different perception predicate.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — matches the established additive-modifier convention. Adrenaline
/// doesn't conjure a Flee path into existence if the cat has nowhere
/// to flee to (nothing scoring above zero in the Flee DSE means no
/// safe direction was found by the underlying score function).
pub struct AcuteHealthAdrenalineFlee {
    threshold: f32,
    flee_lift: f32,
    sleep_lift: f32,
}

impl AcuteHealthAdrenalineFlee {
    /// Smoothstep transition width above `threshold` over which the
    /// lift ramps from 0 to its full magnitude. Narrow (0.1) so the
    /// onset feels like a phase transition rather than a graded
    /// preference. At `health_deficit = threshold + 0.1` the lift is
    /// at full magnitude; between threshold and threshold + 0.1 it
    /// follows the canonical smoothstep `3t² - 2t³`.
    const TRANSITION_WIDTH: f32 = 0.1;

    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.acute_health_adrenaline_threshold,
            flee_lift: sc.acute_health_adrenaline_flee_lift,
            sleep_lift: sc.acute_health_adrenaline_sleep_lift,
        }
    }

    /// Returns the smoothstep ramp `[0, 1]` for the given `health_deficit`.
    /// Below `threshold` returns 0; above `threshold + TRANSITION_WIDTH`
    /// returns 1; in between, `3t² - 2t³` for `t = (deficit - threshold) /
    /// TRANSITION_WIDTH`.
    fn ramp(&self, health_deficit: f32) -> f32 {
        if health_deficit <= self.threshold {
            return 0.0;
        }
        let t = ((health_deficit - self.threshold) / Self::TRANSITION_WIDTH).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

impl ScoreModifier for AcuteHealthAdrenalineFlee {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let lift_scale = match dse_id.0 {
            FLEE => self.flee_lift,
            SLEEP => self.sleep_lift,
            _ => return score,
        };
        if score <= 0.0 {
            return score;
        }
        let deficit = fetch(HEALTH_DEFICIT, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(deficit);
        if ramp <= 0.0 {
            return score;
        }
        score + ramp * lift_scale
    }

    fn name(&self) -> &'static str {
        "acute_health_adrenaline_flee"
    }
}

// ---------------------------------------------------------------------------
// AcuteHealthAdrenalineFight — ticket 102
// ---------------------------------------------------------------------------

/// Ticket 102 — `AcuteHealthAdrenalineFight` Modifier. The Fight valence
/// of the N-valence framework codified in 047 (Flee branch) and 103
/// (`escape_viability` substrate). Models cornered-cat ferocity and
/// maternal defense: when a wounded cat can't realistically escape
/// (low `escape_viability` from walled terrain or held-back by
/// kittens / a partner), the same adrenaline scalar that drives 047's
/// Flee branch instead promotes Fight.
///
/// **Trigger:** `health_deficit >= acute_health_adrenaline_threshold`
/// (shared with 047) **AND** `escape_viability < acute_health_adrenaline_fight_viability_threshold`
/// (the new gate). Above the viability threshold, the Flee branch owns
/// the response; below, this branch takes over.
///
/// **Transform:** same narrow smoothstep ramp on `health_deficit` as
/// 047's Flee. The viability check is binary (gate trip) rather than
/// graded — it's a "which valence" predicate, not a magnitude knob.
///
/// **Applies to:** Fight (lift `acute_health_adrenaline_fight_lift`,
/// default 0.0 — inert at ship). Additionally **suppresses Flee by
/// the same magnitude** so the cornered cat doesn't see Flee promoted
/// by 047's branch on the same tick: when the gate trips, the modifier
/// elects Fight and zeroes Flee's adrenaline lift in one composition
/// step. The two branches are mutually exclusive by construction.
///
/// **Composition:** Registered immediately after `AcuteHealthAdrenalineFlee`
/// so the Flee suppression applies *after* 047's Flee lift was added —
/// net Flee score under the gate is `(base + flee_lift) − fight_lift`,
/// which is approximately `base` when the lifts are equal (the design
/// target). Order within the additive section is preserved: this lift
/// runs before Stockpile / Fox / Corruption multiplicative damps.
///
/// **Substrate role:** Completes the 047 framework's first two
/// valences (Flee + Fight). Freeze (ticket 105) requires the Hide DSE
/// from ticket 104 and lands separately. The intraspecies fawn valence
/// (ticket 109) reads a different scalar and lives in its own modifier.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`,
/// matching 047. Adrenaline doesn't conjure a Fight path into existence
/// when the underlying Fight DSE scored zero (no winnable contest in
/// view). Same convention for the Flee suppression — won't drag a
/// zero-base Flee below zero, so non-additive composition doesn't
/// accidentally invent suppression for absent paths.
pub struct AcuteHealthAdrenalineFight {
    threshold: f32,
    fight_lift: f32,
    viability_threshold: f32,
}

impl AcuteHealthAdrenalineFight {
    /// Same smoothstep transition width as 047's Flee — the two branches
    /// share the adrenaline scalar, so they share the lurch shape.
    const TRANSITION_WIDTH: f32 = 0.1;

    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.acute_health_adrenaline_threshold,
            fight_lift: sc.acute_health_adrenaline_fight_lift,
            viability_threshold: sc.acute_health_adrenaline_fight_viability_threshold,
        }
    }

    /// Smoothstep ramp on `health_deficit`. Identical to 047's Flee
    /// ramp; duplicated because the modifier types are siblings rather
    /// than a hierarchy and folding the ramp into a shared helper would
    /// couple their evolution unnecessarily (e.g. if 102 ever wants a
    /// different transition width or threshold offset).
    fn ramp(&self, health_deficit: f32) -> f32 {
        if health_deficit <= self.threshold {
            return 0.0;
        }
        let t = ((health_deficit - self.threshold) / Self::TRANSITION_WIDTH).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Returns `true` when `escape_viability` is below the gate
    /// threshold (cat is cornered / burdened — fight, don't flee).
    /// Returns `false` when viability is high (above gate — let 047's
    /// Flee branch handle it).
    fn gate_trips(&self, escape_viability: f32) -> bool {
        escape_viability < self.viability_threshold
    }
}

impl ScoreModifier for AcuteHealthAdrenalineFight {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        // Two action surfaces: Fight (additive lift) and Flee
        // (additive suppression by the same magnitude). Anything else
        // is untouched.
        let suppress = match dse_id.0 {
            FIGHT => false,
            FLEE => true,
            _ => return score,
        };
        if score <= 0.0 {
            return score;
        }
        let deficit = fetch(HEALTH_DEFICIT, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(deficit);
        if ramp <= 0.0 {
            return score;
        }
        let viability = fetch(ESCAPE_VIABILITY, ctx.cat).clamp(0.0, 1.0);
        if !self.gate_trips(viability) {
            return score;
        }
        let delta = ramp * self.fight_lift;
        if suppress {
            // Flee suppression — clamp so we never push a positive
            // base below zero (consistent with the gated-boost
            // contract — no invented adrenaline-driven suppression
            // for a Flee path the underlying scoring already rejected).
            (score - delta).max(0.0)
        } else {
            score + delta
        }
    }

    fn name(&self) -> &'static str {
        "acute_health_adrenaline_fight"
    }
}

// ---------------------------------------------------------------------------
// AcuteHealthAdrenalineFreeze — ticket 105
// ---------------------------------------------------------------------------

/// Ticket 105 — `AcuteHealthAdrenalineFreeze` Modifier. The third
/// valence in the §6 N-valence framework that 047 / 102 set up. When
/// escape is not viable AND combat is not winnable (predator
/// approaching, cat overmatched, no cover within sprint range), real
/// cats freeze flat against the ground. The substrate that completes
/// the predator-response trio (Flee / Fight / Freeze).
///
/// **Trigger:** `health_deficit >= acute_health_adrenaline_threshold`
/// (shared with 047 / 102) AND `escape_viability <
/// acute_health_adrenaline_freeze_viability_threshold` (cornered).
/// Phase 1 uses `1.0 - escape_viability` as a `combat_winnability`
/// proxy — a future ticket lands a dedicated `combat_winnability`
/// scalar (per ticket §Out-of-scope guidance), but the proxy is
/// directionally correct: low escape viability correlates with
/// terrain-locked / dependent-burdened scenarios where combat is
/// also unwinnable.
///
/// **Transform:** same narrow smoothstep ramp on `health_deficit`
/// as 047's Flee and 102's Fight — adrenaline lurch shape shared
/// across the three valences. The viability gate is binary: under
/// it, this branch fires; above it, 102's Fight branch fires (when
/// 102's gate trips for combat-winnable scenarios) or 047's Flee
/// (open terrain).
///
/// **Applies to:** Hide (lift `acute_health_adrenaline_freeze_lift`,
/// default 0.0). The Hide DSE (ticket 104) is the action surface
/// for this valence; Phase 1 ships the modifier inert AND with Hide
/// dormant via `HideEligible`, so this commit is bit-identical to
/// baseline. Activation requires both (a) the `HideEligible`
/// authoring system to land and (b) this lift to promote from 0.0
/// to the swept-validated magnitude (~0.70 per ticket §Scope).
///
/// **Composition:** Registered after `IntraspeciesConflictResponseFlight`
/// (109A) and before `FoxTerritorySuppression`. Order with respect
/// to 047 / 102 doesn't load-bear because Freeze targets a
/// different DSE (Hide), but the three valences share the
/// adrenaline scalar so they compose cleanly: under combined high
/// deficit + cornered terrain, 102's Fight gate + 105's Freeze gate
/// can both trip; the choice between them is owned by the relative
/// magnitudes of `acute_health_adrenaline_fight_lift` and
/// `acute_health_adrenaline_freeze_lift`. The 105 spec proposes
/// freeze_lift > fight_lift (0.70 vs 0.50) so freeze is the
/// preferred valence under the cornered gate.
///
/// **Substrate role:** Completes the 047 N-valence framework's third
/// branch. With Phase 1 inert, the framework is structurally
/// present but behaviorally dormant pending each branch's
/// activation commit.
///
/// **Gated-boost contract:** returns `score` unchanged on score
/// `<= 0` — adrenaline doesn't conjure a Hide path into existence.
/// In Phase 1 Hide always scores 0 (gated off by `HideEligible`),
/// so the modifier is doubly inert.
pub struct AcuteHealthAdrenalineFreeze {
    threshold: f32,
    freeze_lift: f32,
    viability_threshold: f32,
}

impl AcuteHealthAdrenalineFreeze {
    /// Same smoothstep transition width as 047 / 102. Adrenaline-
    /// lurch shape shared across the three valences.
    const TRANSITION_WIDTH: f32 = 0.1;

    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.acute_health_adrenaline_threshold,
            freeze_lift: sc.acute_health_adrenaline_freeze_lift,
            viability_threshold: sc.acute_health_adrenaline_freeze_viability_threshold,
        }
    }

    fn ramp(&self, health_deficit: f32) -> f32 {
        if health_deficit <= self.threshold {
            return 0.0;
        }
        let t = ((health_deficit - self.threshold) / Self::TRANSITION_WIDTH).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Returns `true` when the cat is cornered enough that Freeze
    /// owns the response. Strict-less-than mirrors 102's gate.
    fn gate_trips(&self, escape_viability: f32) -> bool {
        escape_viability < self.viability_threshold
    }
}

impl ScoreModifier for AcuteHealthAdrenalineFreeze {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, HIDE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        let deficit = fetch(HEALTH_DEFICIT, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(deficit);
        if ramp <= 0.0 {
            return score;
        }
        let viability = fetch(ESCAPE_VIABILITY, ctx.cat).clamp(0.0, 1.0);
        if !self.gate_trips(viability) {
            return score;
        }
        score + ramp * self.freeze_lift
    }

    fn name(&self) -> &'static str {
        "acute_health_adrenaline_freeze"
    }
}

// ---------------------------------------------------------------------------
// HungerUrgency — ticket 106
// ---------------------------------------------------------------------------

/// Ticket 106 — `HungerUrgency` Modifier. Pressure-shape lift on the
/// food-acquisition class (Eat / Hunt / Forage) gated by
/// `hunger_urgency`. The substrate that replaced the per-tick
/// `InterruptReason::Starvation` override branch — that arm at
/// `disposition.rs:312-325` was retired in the same wave (Phase 2
/// focal-trace verified the interrupt fired 0/run even under 2× hunger
/// decay, structurally vestigial behind the Hunting/Foraging
/// exemption). The GOAP urgency arm at `goap.rs:615-626` remains as
/// the actual food-routing driver.
///
/// **Trigger:** `hunger_urgency >= hunger_urgency_threshold`
/// (default 0.6 — the cat is at hunger 0.4 or below). Engages well
/// before the legacy interrupt (`hunger < 0.15`) so the IAUS contest
/// has time to re-rank Eat / Hunt / Forage above Guarding / Crafting /
/// Patrol before crisis.
///
/// **Transform:** linear ramp from `threshold` to 1.0 ramping to a
/// per-DSE lift magnitude. Sibling shape to 088's
/// `BodyDistressPromotion` (gradual physiological build, not a phase
/// transition) — distinct from 047's smoothstep lurch on injury.
///
/// **Applies to:** Eat (largest lift — direct solution), Hunt (smaller
/// — upstream of Eat), Forage (symmetric to Hunt). All default to 0.0
/// so the modifier ships inert; proposed magnitudes (Eat 0.40, Hunt
/// 0.20, Forage 0.20) are enabled via `CLOWDER_OVERRIDES` for the
/// Phase 3 hypothesize sweep.
///
/// **Composition:** Registered after `AcuteHealthAdrenalineFight` (102)
/// and before `FoxTerritorySuppression`. Order within the additive
/// section matters: under combined high urgency + low stockpile, the
/// Eat lift fires *before* `StockpileSatiation` damps Hunt/Forage —
/// composing toward the same Eat-wins-the-contest target as 088 + 094.
///
/// **Substrate role:** ships inert — defaults 0.0 keep the
/// post-modifier scores bit-identical to pre-Wave-1 baseline.
/// Activation of meaningful lifts is gated on the plan-completion
/// momentum work (ticket 118 / sibling) — see ticket 106 §Log for
/// the Phase 2 verdict. The legacy `disposition.rs` Starvation arm
/// is already retired; the `goap.rs::accumulate_urgencies`
/// Starvation arm at 615-626 is the live food-routing driver and
/// remains in place.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — hunger doesn't conjure food into existence. Mirrors the 088 / 047
/// / 102 convention.
pub struct HungerUrgency {
    threshold: f32,
    eat_lift: f32,
    hunt_lift: f32,
    forage_lift: f32,
}

impl HungerUrgency {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.hunger_urgency_threshold,
            eat_lift: sc.hunger_urgency_eat_lift,
            hunt_lift: sc.hunger_urgency_hunt_lift,
            forage_lift: sc.hunger_urgency_forage_lift,
        }
    }

    /// Returns the linear ramp `[0, 1]` for the given `hunger_urgency`.
    /// Below `threshold` returns 0; above, scales linearly to 1.0 at
    /// urgency = 1.0. Mirror of `BodyDistressPromotion::lift` shape.
    fn ramp(&self, urgency: f32) -> f32 {
        if urgency <= self.threshold {
            return 0.0;
        }
        ((urgency - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0)
    }
}

impl ScoreModifier for HungerUrgency {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let lift_scale = match dse_id.0 {
            EAT => self.eat_lift,
            HUNT => self.hunt_lift,
            FORAGE => self.forage_lift,
            _ => return score,
        };
        if score <= 0.0 {
            return score;
        }
        let urgency = fetch(HUNGER_URGENCY, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(urgency);
        if ramp <= 0.0 {
            return score;
        }
        score + ramp * lift_scale
    }

    fn name(&self) -> &'static str {
        "hunger_urgency"
    }
}

// ---------------------------------------------------------------------------
// ExhaustionPressure — ticket 107
// ---------------------------------------------------------------------------

/// Ticket 107 — `ExhaustionPressure` Modifier. Pressure-shape lift on
/// the rest class (Sleep, GroomSelf) gated by `energy_deficit`. The
/// substrate that replaced the per-tick `InterruptReason::Exhaustion`
/// override branch — that arm at `disposition.rs:312-325` was retired
/// in the same wave (Phase 2 focal-trace verified the interrupt fired
/// 0/run even under 2× energy decay). The GOAP urgency arm at
/// `goap.rs:642-651` remains as the live rest-routing driver.
/// Sibling to 106's `HungerUrgency` on the energy axis.
///
/// **Trigger:** `energy_deficit >= exhaustion_pressure_threshold`
/// (default 0.7 — the cat is at energy 0.3 or below). Engages before
/// the legacy interrupt (`energy < 0.10` ⇒ deficit > 0.90).
///
/// **Transform:** linear ramp from `threshold` to 1.0. Pressure shape:
/// fatigue is a gradual physiological build, not a phase transition.
///
/// **Applies to:** Sleep (largest lift — direct rest), GroomSelf
/// (smaller — exhausted cats sometimes groom-then-sleep as a settling
/// ritual per ticket §Scope). Defaults 0.0 (inert).
///
/// **Composition:** Registered after `HungerUrgency` (106), before
/// `FoxTerritorySuppression`. Order within the additive section:
/// composes additively with 088's `BodyDistressPromotion` Sleep lift
/// (when both fire, total Sleep lift can saturate around 1.0).
///
/// **Substrate role:** ships inert — defaults 0.0 keep the
/// post-modifier scores bit-identical to pre-Wave-2 baseline.
/// Activation of meaningful lifts is gated on the plan-completion
/// momentum work (ticket 118 / sibling) — see ticket 107 §Log for
/// the Phase 2 verdict (Nettle stuck Foraging at energy 0.0 despite
/// Sleep winning L2). The legacy `disposition.rs` Exhaustion arm is
/// already retired; the `goap.rs::accumulate_urgencies` Exhaustion
/// arm at 642-651 is the live rest-routing driver.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — fatigue doesn't conjure a safe sleep spot into existence.
pub struct ExhaustionPressure {
    threshold: f32,
    sleep_lift: f32,
    groom_lift: f32,
}

impl ExhaustionPressure {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.exhaustion_pressure_threshold,
            sleep_lift: sc.exhaustion_pressure_sleep_lift,
            groom_lift: sc.exhaustion_pressure_groom_lift,
        }
    }

    fn ramp(&self, deficit: f32) -> f32 {
        if deficit <= self.threshold {
            return 0.0;
        }
        ((deficit - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0)
    }
}

impl ScoreModifier for ExhaustionPressure {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let lift_scale = match dse_id.0 {
            SLEEP => self.sleep_lift,
            GROOM_SELF => self.groom_lift,
            _ => return score,
        };
        if score <= 0.0 {
            return score;
        }
        let deficit = fetch(ENERGY_DEFICIT, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(deficit);
        if ramp <= 0.0 {
            return score;
        }
        score + ramp * lift_scale
    }

    fn name(&self) -> &'static str {
        "exhaustion_pressure"
    }
}

// ---------------------------------------------------------------------------
// ThermalDistress — ticket 110
// ---------------------------------------------------------------------------

/// Ticket 110 — `ThermalDistress` Modifier. Pressure-shape lift on
/// Sleep (find a den / hearth — routes the cat to a warm tile) gated
/// by `thermal_deficit`. Lower-priority sibling to 106/107 because no
/// legacy `InterruptReason::ThermalCritical` exists to retire — this
/// is purely a perception-richness lever (the "shake the tree" pattern
/// from 047's design — richer cat understanding ⇒ more levers).
///
/// **Trigger:** `thermal_deficit >= thermal_distress_threshold`
/// (default 0.7 — the cat is well outside its thermal comfort band).
///
/// **Transform:** linear ramp from `threshold` to 1.0. Pressure shape:
/// thermal stress is gradual.
///
/// **Applies to:** Sleep only in v1 (Build-shelter lift deferred per
/// ticket §Out-of-scope — needs a "BuildShelter" disposition variant
/// to make sense). Default 0.0 (inert).
///
/// **Composition:** Registered after `ExhaustionPressure` (107),
/// before `FoxTerritorySuppression`. Composes additively with 088 and
/// 107 on Sleep when multiple axes fire simultaneously.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — cold doesn't conjure a den path into existence.
pub struct ThermalDistress {
    threshold: f32,
    sleep_lift: f32,
}

impl ThermalDistress {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.thermal_distress_threshold,
            sleep_lift: sc.thermal_distress_sleep_lift,
        }
    }

    fn ramp(&self, deficit: f32) -> f32 {
        if deficit <= self.threshold {
            return 0.0;
        }
        ((deficit - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0)
    }
}

impl ScoreModifier for ThermalDistress {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let lift_scale = match dse_id.0 {
            SLEEP => self.sleep_lift,
            _ => return score,
        };
        if score <= 0.0 {
            return score;
        }
        let deficit = fetch(THERMAL_DEFICIT, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(deficit);
        if ramp <= 0.0 {
            return score;
        }
        score + ramp * lift_scale
    }

    fn name(&self) -> &'static str {
        "thermal_distress"
    }
}

// ---------------------------------------------------------------------------
// ThreatProximityAdrenalineFlee — ticket 108
// ---------------------------------------------------------------------------

/// Ticket 108 — `ThreatProximityAdrenalineFlee` Modifier. The Flee
/// valence of the threat-perception N-valence framework — sibling to
/// 047's `AcuteHealthAdrenalineFlee` (same lurch shape, different
/// scalar source). Models the adrenaline lurch on **rising** threat
/// proximity: change-detection, not steady-state. The substrate that
/// retires the per-tick `InterruptReason::CriticalSafety` override
/// branch.
///
/// **Trigger:** `threat_proximity_derivative >= threat_proximity_adrenaline_threshold`
/// AND `escape_viability >= threat_proximity_adrenaline_viability_threshold`
/// (the Flee gate — open-terrain cat that just noticed danger
/// approaching). Above the viability threshold, this Flee branch owns
/// the response; below, the future Fight valence (108b) takes over.
///
/// **Transform:** narrow smoothstep from `threshold` to
/// `threshold + transition_width` (width 0.1) ramping to per-DSE lift
/// magnitudes. Same lurch shape as 047 — adrenaline is a phase
/// transition, not a graded preference.
///
/// **Applies to:** Flee (lift `threat_proximity_adrenaline_flee_lift`)
/// and Sleep (lift `threat_proximity_adrenaline_sleep_lift`, the
/// in-pool partner — Flee is filtered from the disposition softmax,
/// Sleep routes the cat to a den). Both default to 0.0 (inert).
///
/// **Composition:** Registered after `ThermalDistress` (110) and
/// before `FoxTerritorySuppression`. Composes additively with 047's
/// AcuteHealthAdrenalineFlee on Flee and with 088 / 107 / 110 on
/// Sleep when multiple axes fire simultaneously.
///
/// **Substrate role:** Phase 4 (gated on Phase 3 sufficiency) retires
/// `disposition.rs::check_interrupt` `CriticalSafety` arm.
///
/// **Phase 1 perception coupling:** the input scalar
/// `threat_proximity_derivative` is currently published as 0.0
/// (stub). The actual derivative requires a `PrevSafetyDeficit`
/// per-cat Component + per-tick update system; that plumbing lands
/// alongside the lift's promotion from 0.0 to the swept-validated
/// magnitude in the same Phase-3-or-Phase-4 commit. With the lift at
/// 0.0 here, the modifier never fires regardless of the scalar's
/// value, so this commit is score-bit-identical to the pre-Wave-1
/// baseline.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — adrenaline doesn't conjure a Flee path or a safe sleep spot
/// where none exists. Mirrors the 047 / 102 convention.
pub struct ThreatProximityAdrenalineFlee {
    threshold: f32,
    flee_lift: f32,
    sleep_lift: f32,
    viability_threshold: f32,
}

impl ThreatProximityAdrenalineFlee {
    /// Smoothstep transition width — narrow (0.1), matching 047's Flee
    /// to keep the lurch shape sibling. The two adrenaline branches
    /// share curve shape because they share the underlying biological
    /// mechanism; the only differences are the scalar input and the
    /// viability gate.
    const TRANSITION_WIDTH: f32 = 0.1;

    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.threat_proximity_adrenaline_threshold,
            flee_lift: sc.threat_proximity_adrenaline_flee_lift,
            sleep_lift: sc.threat_proximity_adrenaline_sleep_lift,
            viability_threshold: sc.threat_proximity_adrenaline_viability_threshold,
        }
    }

    fn ramp(&self, derivative: f32) -> f32 {
        if derivative <= self.threshold {
            return 0.0;
        }
        let t = ((derivative - self.threshold) / Self::TRANSITION_WIDTH).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Returns `true` when escape is viable enough that this Flee
    /// branch owns the response. When viability < threshold, the
    /// future Fight valence (108b) takes over (same scalar input,
    /// different action surface).
    fn gate_trips(&self, escape_viability: f32) -> bool {
        escape_viability >= self.viability_threshold
    }
}

impl ScoreModifier for ThreatProximityAdrenalineFlee {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let lift_scale = match dse_id.0 {
            FLEE => self.flee_lift,
            SLEEP => self.sleep_lift,
            _ => return score,
        };
        if score <= 0.0 {
            return score;
        }
        let derivative = fetch(THREAT_PROXIMITY_DERIVATIVE, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(derivative);
        if ramp <= 0.0 {
            return score;
        }
        let viability = fetch(ESCAPE_VIABILITY, ctx.cat).clamp(0.0, 1.0);
        if !self.gate_trips(viability) {
            return score;
        }
        score + ramp * lift_scale
    }

    fn name(&self) -> &'static str {
        "threat_proximity_adrenaline_flee"
    }
}

// ---------------------------------------------------------------------------
// IntraspeciesConflictResponseFlight — ticket 109 (Phase A)
// ---------------------------------------------------------------------------

/// Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight`
/// Modifier. The social analog of 047's `AcuteHealthAdrenalineFlee`,
/// but reading `social_status_distress` instead of `health_deficit`.
/// Predators don't accept appeasement; cats *do* — intraspecies
/// conflict has a fuller four-valence response repertoire. Phase A
/// ships only the Flight (subordinate retreat) valence; Freeze, Fight,
/// and Fawn open as sub-tickets (109b/c/d).
///
/// **Trigger:** `social_status_distress >= intraspecies_conflict_flight_threshold`.
///
/// **Transform:** linear ramp (pressure shape — social-status pressure
/// is gradual, not a phase transition like physical adrenaline). Same
/// curve as the 088 / 106 / 107 / 110 family.
///
/// **Applies to:** Flee (subordinate-retreat valence — the cat
/// withdraws from the dominant). Default 0.0 (inert).
///
/// **Composition:** Registered after `ThreatProximityAdrenalineFlee`
/// (108) and before `FoxTerritorySuppression`. Composes additively
/// with 047's Flee adrenaline lift if both fire — physically wounded
/// AND socially subordinate cat sees both lifts.
///
/// **Phase 1 perception coupling:** the input scalar
/// `social_status_distress` is published as 0.0 (stub). The actual
/// composition `(status_diff_to_nearest_cat × proximity_factor)`
/// requires a defensible status-differential signal and per-cat
/// nearest-cat resolution; both land with lift activation.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`.
pub struct IntraspeciesConflictResponseFlight {
    threshold: f32,
    flee_lift: f32,
}

impl IntraspeciesConflictResponseFlight {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.intraspecies_conflict_flight_threshold,
            flee_lift: sc.intraspecies_conflict_flight_lift,
        }
    }

    fn ramp(&self, distress: f32) -> f32 {
        if distress <= self.threshold {
            return 0.0;
        }
        ((distress - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0)
    }
}

impl ScoreModifier for IntraspeciesConflictResponseFlight {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, FLEE) {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        let distress = fetch(SOCIAL_STATUS_DISTRESS, ctx.cat).clamp(0.0, 1.0);
        let ramp = self.ramp(distress);
        if ramp <= 0.0 {
            return score;
        }
        score + ramp * self.flee_lift
    }

    fn name(&self) -> &'static str {
        "intraspecies_conflict_flight"
    }
}

// ---------------------------------------------------------------------------
// Default pipeline builder
// ---------------------------------------------------------------------------

/// Build the full §3.5 modifier pipeline. Registration order matches
/// the seven §3.5.1 foundational modifiers — `Pride`,
/// `IndependenceSolo`, `IndependenceGroup`, `Patience`, `Tradition`,
/// `FoxTerritorySuppression`, `CorruptionTerritorySuppression`. These
/// previously ran as imperative passes after per-DSE scoring in
/// `score_actions`, matching the order they're registered in here.
///
/// The three corruption-response emergency bonuses
/// (`WardCorruptionEmergency`, `CleanseEmergency`, `SensedRotBoost`)
/// retired in §13.1 once the per-axis Logistic curves that absorb
/// their contribution landed in the corresponding sibling DSEs; the
/// workaround modifier layer is gone by construction.
///
/// The §3.5.1 modifiers mix additive, subtractive, and multiplicative
/// transforms — the registered order here (additive bonuses before
/// multiplicative damps) matches the retiring inline block so a
/// multi-axis DSE like `hunt` receives Pride + Independence-solo
/// boosts *before* the fox-scent multiplicative damp.
///
/// Mirror sites — `src/plugins/simulation.rs`, `src/main.rs`
/// `setup_world` + `run_new_game`, save-load restore — each call
/// this helper to produce the same pipeline shape.
pub fn default_modifier_pipeline(
    constants: &crate::resources::sim_constants::SimConstants,
) -> ModifierPipeline {
    let sc = &constants.scoring;
    // Ticket 146 — saturating-composition cap. Cumulative positive lift
    // across the pipeline saturates at `max_additive_lift_per_dse` via
    // `MAX * (1 - Π(1 - lift_i / MAX))`. Ships at 0.0 (disabled);
    // activate by setting `max_additive_lift_per_dse > 0` (0.60 matches
    // 047 single-modifier Flee design value while bounding multi-axis
    // pile-ups like the 107+110 Sleep double-stack documented in 146).
    let mut pipeline =
        ModifierPipeline::new().with_max_additive_lift_per_dse(sc.max_additive_lift_per_dse);
    pipeline.push(Box::new(Pride::new(sc)));
    pipeline.push(Box::new(IndependenceSolo::new(sc)));
    pipeline.push(Box::new(IndependenceGroup::new(sc)));
    pipeline.push(Box::new(Patience::new(sc)));
    // §075 — `CommitmentTenure` registers between Patience and
    // Tradition. Order doesn't load-bear (its lift is additive and
    // commutes with every other §3.5.1 transform), but it sits next
    // to Patience so the trace shows the two related additive lifts
    // adjacent in the modifier-pipeline output. The tunable lives on
    // `DispositionConstants` (alongside `min_disposition_tenure_ticks`),
    // which is why this helper takes `&SimConstants` rather than
    // `&ScoringConstants`.
    pipeline.push(Box::new(CommitmentTenure::new(constants)));
    pipeline.push(Box::new(Tradition::new()));
    // §3.5.1 ticket 088 — `BodyDistressPromotion` registers with the
    // additive bonuses before the multiplicative damps (Fox /
    // Corruption / Stockpile). Order matters here: when stockpile is
    // full *and* the cat is body-distressed, this lift on Eat fires
    // *before* `StockpileSatiation` damps Hunt/Forage, doubly tilting
    // the IAUS contest toward Eat. The 094 `StockpileSatiation`
    // doc-comment pre-described this exact composition order.
    pipeline.push(Box::new(BodyDistressPromotion::new(sc)));
    // Ticket 047 — `AcuteHealthAdrenalineFlee` registers immediately
    // after `BodyDistressPromotion` so under combined high composite
    // distress + high health deficit, both lifts compose additively on
    // Sleep before the multiplicative damps run. Order matters within
    // the additive section: the two lifts must both apply before
    // Stockpile / Fox / Corruption damp Hunt/Forage/etc. — composing
    // injury-driven Sleep/Flee promotion with stockpile-driven
    // Hunt/Forage suppression yields the strongest contest-tilt away
    // from Guarding/Crafting under injury, which is the ticket-047
    // behavioral target.
    pipeline.push(Box::new(AcuteHealthAdrenalineFlee::new(sc)));
    // Ticket 102 — `AcuteHealthAdrenalineFight` registers immediately
    // after the Flee branch so its Flee-suppression component runs
    // *after* 047's Flee lift was added: under the viability gate,
    // (base + flee_lift) − fight_lift ≈ base when the magnitudes match
    // (047 0.60 vs 102 0.50 — close enough that the cornered cat sees
    // Flee held near baseline rather than promoted, while Fight gets
    // the full lurch). Order within the additive section is preserved:
    // this composes before the multiplicative damps run.
    pipeline.push(Box::new(AcuteHealthAdrenalineFight::new(sc)));
    // Ticket 105 — `AcuteHealthAdrenalineFreeze` registers immediately
    // after the Fight branch. The three adrenaline valences (047 Flee
    // / 102 Fight / 105 Freeze) share the `health_deficit` scalar but
    // target disjoint DSEs (Flee+Sleep / Fight+Flee-suppress / Hide),
    // so registration order among them doesn't load-bear on a single
    // DSE's score. Adjacency in the pipeline keeps the trace output
    // visually grouped.
    pipeline.push(Box::new(AcuteHealthAdrenalineFreeze::new(sc)));
    // Tickets 106 / 107 / 110 — pressure modifiers on the per-axis
    // physiological deficits (hunger, energy, thermal). Registered
    // after the adrenaline-lurch valences (047 / 102) and before the
    // multiplicative damps (Fox / Corruption / Stockpile) so each
    // pressure lift composes additively with 088's
    // `BodyDistressPromotion` and the adrenaline lifts before any
    // multiplicative pass. Order among the three doesn't load-bear:
    // each gates on a different scalar and a different DSE matrix
    // (Eat/Hunt/Forage vs Sleep/GroomSelf vs Sleep), so they commute.
    pipeline.push(Box::new(HungerUrgency::new(sc)));
    pipeline.push(Box::new(ExhaustionPressure::new(sc)));
    pipeline.push(Box::new(ThermalDistress::new(sc)));
    // Ticket 108 — `ThreatProximityAdrenalineFlee` registers after the
    // pressure modifiers (106 / 107 / 110) and before the
    // multiplicative damps. Composes additively with 047's Flee
    // adrenaline lift on Flee and with 088 / 107 / 110 on Sleep when
    // multiple axes fire simultaneously. Phase 1 ships with both lifts
    // at 0.0 default (inert) and reads a stub
    // `threat_proximity_derivative` scalar (also 0.0) — actual
    // perception coupling lands with lift activation.
    pipeline.push(Box::new(ThreatProximityAdrenalineFlee::new(sc)));
    // Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight`
    // registers after the predator-threat adrenaline branch (108) and
    // before the multiplicative damps. The social Flight valence is
    // the substrate analog to 047 / 102 on the social axis.
    pipeline.push(Box::new(IntraspeciesConflictResponseFlight::new(sc)));
    pipeline.push(Box::new(FoxTerritorySuppression::new(sc)));
    pipeline.push(Box::new(CorruptionTerritorySuppression::new(sc)));
    // §3.5.1 ticket 094 — `StockpileSatiation` registers after the
    // existing multiplicative damps (Fox + Corruption). Order doesn't
    // load-bear among the damps: each is gated by a different DSE-id
    // matrix and a different scalar trigger. Sitting next to its
    // sibling `FoxTerritorySuppression` keeps the food-economy and
    // territory-pressure modifiers visually adjacent in the trace
    // output.
    pipeline.push(Box::new(StockpileSatiation::new(sc)));
    pipeline
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::ai::considerations::LandmarkAnchor;
    use super::*;
    use crate::components::physical::Position;

    fn test_ctx() -> (Entity, EvalCtx<'static>) {
        // Static closures satisfy `EvalCtx<'ctx>` lifetime where we don't
        // need real per-tick access.
        static MARKER: fn(&str, Entity) -> bool = |_, _| false;
        static NO_ENTITY_POS: fn(Entity) -> Option<Position> = |_| None;
        static NO_ANCHOR_POS: fn(LandmarkAnchor) -> Option<Position> = |_| None;
        let entity = Entity::from_raw_u32(1).unwrap();
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &NO_ENTITY_POS,
            anchor_position: &NO_ANCHOR_POS,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        (entity, ctx)
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
            EAT, SLEEP, HUNT, FORAGE, SOCIALIZE, EXPLORE, WANDER, FLEE, FIGHT, PATROL, BUILD, FARM,
            COORDINATE, MENTOR, MATE, COOK, CARETAKE, IDLE,
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
        assert!(
            (out - expected).abs() < 1e-6,
            "got {out}, expected {expected}"
        );
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
        assert!(
            (out - expected).abs() < 1e-6,
            "got {out}, expected {expected}"
        );
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
                // Ticket 104 — Hide is anxiety-interrupt class (no
                // parent disposition), so it has no constituent role
                // in this map. Test never queries it. Listed for
                // exhaustivity.
                Action::Hide => &[HIDE],
            }
        }

        for (ordinal, kind) in (1..).zip(DispositionKind::ALL.iter().copied()) {
            let expected: Vec<&'static str> = kind
                .constituent_actions()
                .iter()
                .flat_map(|a| action_to_dse_ids(*a).iter().copied())
                .collect();
            let actual = constituent_dses_for_ordinal(ordinal as f32).expect("every variant maps");
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

    // -----------------------------------------------------------------------
    // §3.5.1 / §075 CommitmentTenure
    // -----------------------------------------------------------------------

    #[test]
    fn commitment_tenure_lifts_constituent_dse_inside_window() {
        // Hunting disposition (ordinal 2) → Hunt is constituent.
        // Inside the tenure window: progress = 0.4 → modifier applies
        // its flat lift to the Hunt score.
        let modifier = CommitmentTenure { lift: 0.10 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            COMMITMENT_TENURE_PROGRESS => 0.4,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.6).abs() < 1e-6,
            "0.5 + 0.10 = 0.6 inside-window; got {out}"
        );
    }

    #[test]
    fn commitment_tenure_no_lift_outside_window() {
        // progress = 1.0 → window has elapsed; no lift on any DSE.
        let modifier = CommitmentTenure { lift: 0.10 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            COMMITMENT_TENURE_PROGRESS => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.5).abs() < 1e-6,
            "outside-window ⇒ no lift; got {out}"
        );
    }

    #[test]
    fn commitment_tenure_no_lift_when_no_active_disposition() {
        // The scoring producer reports progress = 1.0 when the cat
        // has no active disposition; the modifier short-circuits on
        // that. Verify that even with progress = 0.0 (defensive: a
        // future producer could miss the no-disposition gate), the
        // ordinal = 0.0 short-circuit also blocks the lift.
        let modifier = CommitmentTenure { lift: 0.10 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 0.0,
            COMMITMENT_TENURE_PROGRESS => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.5).abs() < 1e-6,
            "no active disposition ⇒ no lift; got {out}"
        );
    }

    #[test]
    fn commitment_tenure_targets_only_constituent_dses_of_active_disposition() {
        // Hunting disposition (ordinal 2). Inside the window. Hunt is
        // constituent; Socialize / Eat / Sleep are not — they get no
        // lift.
        let modifier = CommitmentTenure { lift: 0.10 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            COMMITMENT_TENURE_PROGRESS => 0.0,
            _ => 0.0,
        };
        // Hunt is constituent → lifted.
        let hunt_out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        assert!((hunt_out - 0.6).abs() < 1e-6, "Hunt lifted; got {hunt_out}");
        // Non-constituent DSEs pass through unchanged.
        for dse in [SOCIALIZE, EAT, SLEEP, FORAGE, FIGHT, BUILD] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "dse {dse} (non-constituent) unchanged; got {out}"
            );
        }
    }

    #[test]
    fn commitment_tenure_does_not_resurrect_zero_score() {
        // Gated-boost contract: a DSE the Maslow pre-gate suppressed
        // (score == 0) MUST stay at 0 — the additive lift can't bring
        // a suppressed action back into the pool.
        let modifier = CommitmentTenure { lift: 0.10 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            COMMITMENT_TENURE_PROGRESS => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero score stays zero — no resurrection");
    }

    #[test]
    fn commitment_tenure_lift_independent_of_personality() {
        // §075 design: the lift is a flat anti-oscillation pad that
        // doesn't scale with personality (unlike Patience which scales
        // with `personality.patience`). Verify by sweeping the patience
        // scalar and confirming the same constant lift fires.
        let modifier = CommitmentTenure { lift: 0.10 };
        let (_, ctx) = test_ctx();
        for patience in [0.0, 0.3, 0.7, 1.0] {
            let fetch = |name: &str, _: Entity| match name {
                ACTIVE_DISPOSITION_ORDINAL => 2.0,
                COMMITMENT_TENURE_PROGRESS => 0.5,
                PATIENCE => patience,
                _ => 0.0,
            };
            let out = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.6).abs() < 1e-6,
                "patience {patience} ⇒ lift unchanged; got {out}"
            );
        }
    }

    #[test]
    fn commitment_tenure_breaks_disposition_oscillation_synthetic() {
        // §075 synthetic regression test. Setup: two dispositions tied
        // within ε of each other (Hunting + Foraging at score 0.50).
        // Without the modifier registered, the IAUS softmax (Boltzmann
        // weights) would split probability ~50/50 across them, making
        // the cat oscillate every re-evaluation. With the modifier
        // registered and the cat already on Hunting (active ordinal
        // 2), the additive lift biases Hunting's softmax weight.
        //
        // We don't run the full softmax here (it's a stochastic
        // sampler); instead we verify the mechanical claim that the
        // modifier *changes* the Hunting score by exactly `lift` while
        // leaving Foraging untouched. That score gap is what the
        // softmax then converts into a probability mass favoring the
        // incumbent.
        use crate::ai::eval::ModifierPipeline;

        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(CommitmentTenure { lift: 0.10 }));

        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0, // Hunting is incumbent
            COMMITMENT_TENURE_PROGRESS => 0.0, // tick == disposition_started_tick
            _ => 0.0,
        };

        // Hunting (incumbent's constituent) gets the lift.
        let hunt = pipeline.apply(DseId(HUNT), 0.50, &ctx, &fetch);
        // Forage (alternate disposition's constituent) does not.
        let forage = pipeline.apply(DseId(FORAGE), 0.50, &ctx, &fetch);
        assert!(
            (hunt - 0.60).abs() < 1e-6,
            "incumbent disposition's constituent lifted; got {hunt}"
        );
        assert!(
            (forage - 0.50).abs() < 1e-6,
            "non-incumbent disposition's constituent unchanged; got {forage}"
        );
        // The score gap (hunt - forage = 0.10) is precisely what
        // breaks the tie that would otherwise oscillate the softmax.
        assert!(
            hunt > forage,
            "score gap favors incumbent: hunt={hunt}, forage={forage}"
        );

        // Sanity: outside the tenure window the gap collapses back to 0.
        let fetch_outside = |name: &str, _: Entity| match name {
            ACTIVE_DISPOSITION_ORDINAL => 2.0,
            COMMITMENT_TENURE_PROGRESS => 1.0, // window has elapsed
            _ => 0.0,
        };
        let hunt_after = pipeline.apply(DseId(HUNT), 0.50, &ctx, &fetch_outside);
        let forage_after = pipeline.apply(DseId(FORAGE), 0.50, &ctx, &fetch_outside);
        assert!(
            (hunt_after - forage_after).abs() < 1e-6,
            "post-tenure: scores tie again ⇒ softmax can switch freely"
        );
    }

    #[test]
    fn default_pipeline_registers_eighteen_modifiers() {
        // Eighteen §3.5.1 foundational modifiers — the seven original
        // (`Pride`, `IndependenceSolo`, `IndependenceGroup`, `Patience`,
        // `Tradition`, `FoxTerritorySuppression`,
        // `CorruptionTerritorySuppression`) plus §075's
        // `CommitmentTenure`, ticket 094's `StockpileSatiation`,
        // ticket 088's `BodyDistressPromotion`, ticket 047's
        // `AcuteHealthAdrenalineFlee`, ticket 102's
        // `AcuteHealthAdrenalineFight`, ticket 105's
        // `AcuteHealthAdrenalineFreeze`, ticket 106's `HungerUrgency`,
        // ticket 107's `ExhaustionPressure`, ticket 110's
        // `ThermalDistress`, ticket 108's
        // `ThreatProximityAdrenalineFlee`, and ticket 109 Phase A's
        // `IntraspeciesConflictResponseFlight`. The three Phase 4.2
        // emergency modifiers (`WardCorruptionEmergency`,
        // `CleanseEmergency`, `SensedRotBoost`) retired in §13.1.
        let constants = crate::resources::sim_constants::SimConstants::default();
        let pipeline = default_modifier_pipeline(&constants);
        assert_eq!(pipeline.len(), 18, "expected 18 registered modifiers");
    }

    // -----------------------------------------------------------------------
    // §3.5.1 ticket 094 StockpileSatiation
    // -----------------------------------------------------------------------

    #[test]
    fn stockpile_satiation_no_damp_below_threshold() {
        // food_fraction = 0.4 < threshold (0.5). Hunt and Forage pass
        // through unchanged. The desperation-hunting case: a starving
        // colony's food-acquisition DSEs MUST NOT be suppressed.
        let modifier = StockpileSatiation { threshold: 0.5, scale: 0.85 };
        let (_, ctx) = test_ctx();
        // food_scarcity = 0.6 ⇒ food_fraction = 0.4
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.6,
            _ => 0.0,
        };
        let hunt = modifier.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        let forage = modifier.apply(DseId(FORAGE), 0.61, &ctx, &fetch);
        assert!((hunt - 0.85).abs() < 1e-6, "below-threshold Hunt unchanged; got {hunt}");
        assert!((forage - 0.61).abs() < 1e-6, "below-threshold Forage unchanged; got {forage}");
    }

    #[test]
    fn stockpile_satiation_damps_hunt_and_forage_above_threshold() {
        // food_fraction = 0.96 (post-091 baseline level).
        // suppression = (0.96 - 0.5) / (1 - 0.5) * 0.85 = 0.782
        // Hunt 0.85 × (1 - 0.782) = ~0.185; Forage 0.61 × ~0.218 = ~0.133
        let modifier = StockpileSatiation { threshold: 0.5, scale: 0.85 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.04,
            _ => 0.0,
        };
        let hunt = modifier.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        let forage = modifier.apply(DseId(FORAGE), 0.61, &ctx, &fetch);
        // Hunt: 0.85 × 0.218 = 0.1853 (allow 1e-3 for f32 rounding).
        assert!((hunt - 0.1853).abs() < 1e-3, "Hunt damped; got {hunt}");
        assert!((forage - 0.13298).abs() < 1e-3, "Forage damped; got {forage}");
    }

    #[test]
    fn stockpile_satiation_full_stores_collapses_acquisition_dses() {
        // food_fraction = 1.0 ⇒ suppression = 0.85 ⇒ score × 0.15.
        let modifier = StockpileSatiation { threshold: 0.5, scale: 0.85 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.0,
            _ => 0.0,
        };
        let hunt = modifier.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        // 0.85 × (1 - 0.85) = 0.85 × 0.15 = 0.1275
        assert!((hunt - 0.1275).abs() < 1e-3, "full-stores Hunt collapses; got {hunt}");
    }

    #[test]
    fn stockpile_satiation_targets_only_hunt_and_forage() {
        // Eat (the destination of the contest), Cook (consumes raw
        // food, abundance is its *reason* to fire), Sleep / Groom /
        // etc. self-care must pass through unchanged. The asymmetry
        // is the point: damp the *acquisition* DSEs, leave the
        // *consumption* and self-care DSEs alone so the IAUS contest
        // tilts toward Eat at the existing food.
        let modifier = StockpileSatiation { threshold: 0.5, scale: 0.85 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.0, // food_fraction = 1.0
            _ => 0.0,
        };
        for dse in [EAT, SLEEP, GROOM_SELF, GROOM_OTHER, FLEE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "dse {dse} (non-acquisition) unchanged; got {out}"
            );
        }
    }

    #[test]
    fn stockpile_satiation_zero_score_stays_zero() {
        // Multiplicative damp on score == 0 is naturally safe (0 × x = 0).
        // No resurrection.
        let modifier = StockpileSatiation { threshold: 0.5, scale: 0.85 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HUNT), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero score stays zero — multiplicative damp is safe");
    }

    #[test]
    fn stockpile_satiation_lever_breaks_lark_contest_synthetic() {
        // Ticket 094 synthetic regression: at hunger=0.20 (urgency 0.80)
        // with food_fraction = 0.96 (post-091 baseline) and Lark's
        // bold/diligent personality near forageable terrain, Hunt ≈ 0.85
        // and Eat (long-range) ≈ 0.27. Without StockpileSatiation, Hunt
        // dominates and the cat re-elects Hunt forever. With it, Hunt
        // collapses to ~0.19 and Eat (unchanged) wins the contest.
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(StockpileSatiation { threshold: 0.5, scale: 0.85 }));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.04, // food_fraction = 0.96
            _ => 0.0,
        };
        let hunt_after = pipeline.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        let eat_after = pipeline.apply(DseId(EAT), 0.27, &ctx, &fetch);
        assert!(
            eat_after > hunt_after,
            "post-modifier: Eat ({eat_after}) wins over Hunt ({hunt_after}) under abundant stockpile"
        );
        // Concrete bounds — Hunt drops to ~0.185, Eat stays at 0.27.
        assert!(hunt_after < 0.20, "Hunt sufficiently damped; got {hunt_after}");
        assert!((eat_after - 0.27).abs() < 1e-6, "Eat unchanged by modifier; got {eat_after}");
    }

    #[test]
    fn stockpile_satiation_preserves_desperation_hunting() {
        // Symmetric guarantee: at food_fraction = 0 (starving colony,
        // empty stockpile), Hunt and Forage MUST be unchanged so the
        // food-acquisition response can fire at full strength. This
        // is the "this modifier never kicks in when acquisition is
        // urgent" property — the modifier is asymmetric by design.
        let modifier = StockpileSatiation { threshold: 0.5, scale: 0.85 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 1.0, // food_fraction = 0.0
            _ => 0.0,
        };
        let hunt = modifier.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        let forage = modifier.apply(DseId(FORAGE), 0.61, &ctx, &fetch);
        assert!((hunt - 0.85).abs() < 1e-6, "starving Hunt unchanged; got {hunt}");
        assert!((forage - 0.61).abs() < 1e-6, "starving Forage unchanged; got {forage}");
    }

    // -----------------------------------------------------------------------
    // §3.5.1 ticket 088 BodyDistressPromotion
    // -----------------------------------------------------------------------

    #[test]
    fn body_distress_promotion_no_lift_below_threshold() {
        // distress = 0.5 < threshold (0.7). Every self-care DSE passes
        // through unchanged. The "modifier engages only at high distress"
        // property — below the threshold the cat's score landscape is
        // identical to the no-modifier case.
        let modifier = BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 0.5,
            _ => 0.0,
        };
        for dse in SELF_CARE_DSES {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "below-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn body_distress_promotion_zero_lift_at_threshold() {
        // distress = 0.7 exactly. The `<= threshold` short-circuit
        // gives zero lift — boundary semantics matter so the modifier
        // doesn't quietly drift on the edge.
        let modifier = BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 0.7,
            _ => 0.0,
        };
        for dse in SELF_CARE_DSES {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "at-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn body_distress_promotion_lifts_above_threshold() {
        // distress = 0.85, threshold = 0.7, lift_scale = 0.20.
        // expected lift = ((0.85 - 0.7) / 0.3) * 0.20 = 0.5 * 0.20 = 0.10
        let modifier = BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 0.85,
            _ => 0.0,
        };
        for dse in SELF_CARE_DSES {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.6).abs() < 1e-6,
                "dse {dse} lifted by 0.10; got {out}"
            );
        }
    }

    #[test]
    fn body_distress_promotion_max_lift_at_full_distress() {
        // distress = 1.0 ⇒ full lift_scale (0.20) added to every
        // self-care DSE.
        let modifier = BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 1.0,
            _ => 0.0,
        };
        for dse in SELF_CARE_DSES {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.7).abs() < 1e-6,
                "dse {dse} lifted by full 0.20; got {out}"
            );
        }
    }

    #[test]
    fn body_distress_promotion_targets_only_self_care_class() {
        // At full distress, non-self-care DSEs MUST pass through
        // unchanged — the substrate property the lift relies on. If
        // Mate / Coordinate / Build / Mentor / Caretake / Socialize /
        // Patrol / Fight / Cook / Farm / Wander / Explore / Idle were
        // also lifted, the modifier wouldn't tilt the contest toward
        // self-care; it would just inflate every score equally.
        let modifier = BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 1.0,
            _ => 0.0,
        };
        for dse in [
            MATE,
            COORDINATE,
            BUILD,
            MENTOR,
            CARETAKE,
            SOCIALIZE,
            PATROL,
            FIGHT,
            COOK,
            FARM,
            WANDER,
            EXPLORE,
            IDLE,
            GROOM_OTHER,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-self-care dse {dse} unchanged at full distress; got {out}"
            );
        }
    }

    #[test]
    fn body_distress_promotion_does_not_resurrect_zero_score() {
        // Gated-boost contract — matches Pride / IndependenceSolo /
        // Patience / CommitmentTenure / Tradition. A self-care DSE the
        // Maslow pre-gate or outer scoring layer suppressed (score == 0)
        // MUST stay at 0; high body-distress doesn't conjure food into
        // existence or create a safe sleep spot. The modifier only
        // re-ranks already-accessible considerations.
        let modifier = BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 1.0,
            _ => 0.0,
        };
        for dse in SELF_CARE_DSES {
            let out = modifier.apply(DseId(dse), 0.0, &ctx, &fetch);
            assert_eq!(
                out, 0.0,
                "zero-score dse {dse} stays zero — no resurrection"
            );
        }
    }

    // -----------------------------------------------------------------------
    // ticket 047 AcuteHealthAdrenalineFlee
    // -----------------------------------------------------------------------

    fn test_adrenaline() -> AcuteHealthAdrenalineFlee {
        AcuteHealthAdrenalineFlee {
            threshold: 0.4,
            flee_lift: 0.60,
            sleep_lift: 0.50,
        }
    }

    #[test]
    fn acute_health_adrenaline_no_lift_below_threshold() {
        // health_deficit = 0.3 < threshold (0.4). Flee and Sleep
        // unchanged. Mirrors the "modifier engages only at high
        // distress" property of BodyDistressPromotion but on the
        // health-deficit axis directly.
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.3,
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "below-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn acute_health_adrenaline_zero_lift_at_threshold() {
        // Boundary semantics — `<= threshold` short-circuits to zero
        // lift, so the smoothstep doesn't drift off the edge.
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.4,
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "at-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn acute_health_adrenaline_full_lift_above_transition_band() {
        // health_deficit = 0.55 > threshold + transition_width (0.5).
        // Smoothstep saturates at 1.0 ⇒ Flee + 0.60 = 1.10, Sleep + 0.50 = 1.00.
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((flee - 1.10).abs() < 1e-5, "Flee saturated lift; got {flee}");
        assert!((sleep - 1.00).abs() < 1e-5, "Sleep saturated lift; got {sleep}");
    }

    #[test]
    fn acute_health_adrenaline_smoothstep_midpoint() {
        // health_deficit = 0.45 = threshold + half-width. Smoothstep at
        // t = 0.5 evaluates to 3*0.25 - 2*0.125 = 0.5. So Flee gets
        // half its full lift (+0.30) and Sleep gets half (+0.25).
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.45,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((flee - 0.80).abs() < 1e-5, "Flee half-lift; got {flee}");
        assert!((sleep - 0.75).abs() < 1e-5, "Sleep half-lift; got {sleep}");
    }

    #[test]
    fn acute_health_adrenaline_targets_only_flee_and_sleep() {
        // The lurch is two-DSE only — Hunt / Forage / Eat / GroomSelf
        // (also self-care) and the entire non-self-care class must pass
        // through unchanged. If the lift bled into Fight or Hunt the
        // contest tilt would invert (Fight rising under injury defeats
        // the substrate's purpose).
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            _ => 0.0,
        };
        for dse in [
            EAT, HUNT, FORAGE, GROOM_SELF, MATE, COORDINATE, BUILD, MENTOR, CARETAKE, SOCIALIZE,
            PATROL, FIGHT, COOK, FARM, WANDER, EXPLORE, IDLE, GROOM_OTHER,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Flee/Sleep dse {dse} unchanged at full deficit; got {out}"
            );
        }
    }

    #[test]
    fn acute_health_adrenaline_does_not_resurrect_zero_score() {
        // Gated-boost contract — adrenaline doesn't conjure a Flee
        // path into existence if the cat has nowhere to flee to. Sleep
        // gated to zero (no safe rest spot in range) similarly stays
        // suppressed.
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.0, &ctx, &fetch);
            assert_eq!(
                out, 0.0,
                "zero-score dse {dse} stays zero — no adrenaline resurrection"
            );
        }
    }

    #[test]
    fn acute_health_adrenaline_composes_additively_with_body_distress_promotion() {
        // Under combined high body_distress_composite (088) AND high
        // health_deficit (047), Sleep sees both lifts add. This is
        // intentional: composite says "the cat is unwell on average,"
        // health_deficit says "the cat is being injured *now*"; both
        // lifting Sleep simultaneously is the right ecological response.
        // Mirrors the Mallow scenario where the cat would have died
        // even with 088 alone (composite ~0.61 sat below 088's 0.7
        // threshold), but the 047 modifier reading health_deficit
        // directly fires.
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(BodyDistressPromotion {
            threshold: 0.7,
            lift_scale: 0.20,
        }));
        pipeline.push(Box::new(test_adrenaline()));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 1.0,
            HEALTH_DEFICIT => 1.0,
            _ => 0.0,
        };
        // Sleep: 088 lifts +0.20 (composite at 1.0), 047 lifts +0.50
        // (deficit at 1.0). Pre-modifier 0.30 → 0.30 + 0.20 + 0.50 = 1.00.
        let sleep_after = pipeline.apply(DseId(SLEEP), 0.30, &ctx, &fetch);
        assert!(
            (sleep_after - 1.00).abs() < 1e-5,
            "Sleep after combined lift = 1.00; got {sleep_after}"
        );
        // Flee: 088 lifts +0.20, 047 lifts +0.60. Pre 0.06 → 0.86.
        let flee_after = pipeline.apply(DseId(FLEE), 0.06, &ctx, &fetch);
        assert!(
            (flee_after - 0.86).abs() < 1e-5,
            "Flee after combined lift; got {flee_after}"
        );
    }

    #[test]
    fn acute_health_adrenaline_fires_when_088_does_not_mallow_scenario() {
        // The Mallow scenario from logs/collapse-probe-42-fix-043-044
        // tick 1216300: health=0.637 → health_deficit=0.363. The 047
        // modifier with threshold=0.4 does NOT fire here — confirming
        // the alignment: the substrate engages where the legacy
        // CriticalHealth interrupt did (health < 0.4), not earlier.
        // Once damage takes Mallow below health=0.4 (deficit > 0.6),
        // the lurch saturates immediately. This test pins the alignment
        // so future threshold tweaks don't silently break it.
        let modifier = test_adrenaline();
        let (_, ctx) = test_ctx();
        // Mallow's last-snapshot deficit, BEFORE the next-tick injury
        // tick that would have crossed 0.4: no lift expected.
        let fetch_pre = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.363,
            _ => 0.0,
        };
        let sleep_pre = modifier.apply(DseId(SLEEP), 0.30, &ctx, &fetch_pre);
        assert!(
            (sleep_pre - 0.30).abs() < 1e-6,
            "deficit 0.363 below 0.4 threshold — no lift; got {sleep_pre}"
        );
        // One more injury tick puts deficit > 0.5 (transition band end):
        // saturated lift fires.
        let fetch_post = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            _ => 0.0,
        };
        let sleep_post = modifier.apply(DseId(SLEEP), 0.30, &ctx, &fetch_post);
        assert!(
            (sleep_post - 0.80).abs() < 1e-5,
            "deficit 0.55 saturates 0.50 lift; got {sleep_post}"
        );
    }

    // -----------------------------------------------------------------------
    // ticket 102 AcuteHealthAdrenalineFight
    // -----------------------------------------------------------------------

    fn test_adrenaline_fight() -> AcuteHealthAdrenalineFight {
        AcuteHealthAdrenalineFight {
            threshold: 0.4,
            fight_lift: 0.50,
            viability_threshold: 0.4,
        }
    }

    #[test]
    fn adrenaline_fight_no_lift_below_health_threshold() {
        // Deficit below threshold ⇒ no lift, even when escape_viability
        // is far below the gate. The two predicates AND together;
        // failing either short-circuits.
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.3,
            ESCAPE_VIABILITY => 0.1, // cornered, but cat is not yet wounded enough
            _ => 0.0,
        };
        let fight = modifier.apply(DseId(FIGHT), 0.5, &ctx, &fetch);
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((fight - 0.5).abs() < 1e-6, "Fight unchanged when deficit below threshold; got {fight}");
        assert!((flee - 0.5).abs() < 1e-6, "Flee unchanged when deficit below threshold; got {flee}");
    }

    #[test]
    fn adrenaline_fight_no_lift_when_escape_viable() {
        // Deficit saturated, but escape_viability >= gate threshold ⇒
        // 047's Flee branch owns the response, this branch stays quiet.
        // The viability predicate is the substrate that splits Flee from
        // Fight under the same adrenaline scalar.
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            ESCAPE_VIABILITY => 0.8, // open terrain, no dependents
            _ => 0.0,
        };
        let fight = modifier.apply(DseId(FIGHT), 0.5, &ctx, &fetch);
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((fight - 0.5).abs() < 1e-6, "Fight unchanged when escape viable; got {fight}");
        assert!((flee - 0.5).abs() < 1e-6, "Flee unchanged when escape viable; got {flee}");
    }

    #[test]
    fn adrenaline_fight_full_lift_under_gate() {
        // Deficit saturates the smoothstep (>= 0.5), viability below gate.
        // Fight gets +0.50, Flee gets -0.50.
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            ESCAPE_VIABILITY => 0.1,
            _ => 0.0,
        };
        let fight = modifier.apply(DseId(FIGHT), 0.5, &ctx, &fetch);
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((fight - 1.0).abs() < 1e-5, "Fight saturated lift; got {fight}");
        assert!((flee - 0.0).abs() < 1e-5, "Flee suppressed to base − lift; got {flee}");
    }

    #[test]
    fn adrenaline_fight_smoothstep_midpoint_under_gate() {
        // Deficit at threshold + half-width = 0.45, viability below gate.
        // Smoothstep ramp = 0.5 ⇒ Fight +0.25, Flee -0.25.
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.45,
            ESCAPE_VIABILITY => 0.1,
            _ => 0.0,
        };
        let fight = modifier.apply(DseId(FIGHT), 0.5, &ctx, &fetch);
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((fight - 0.75).abs() < 1e-5, "Fight half-lift; got {fight}");
        assert!((flee - 0.25).abs() < 1e-5, "Flee half-suppression; got {flee}");
    }

    #[test]
    fn adrenaline_fight_targets_only_fight_and_flee() {
        // Lurch is two-DSE only — Sleep / Hunt / Forage / Eat / etc. all
        // pass through unchanged. If the lift bled into Sleep we'd
        // double-promote it (047 Flee branch already lifts Sleep).
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            ESCAPE_VIABILITY => 0.0,
            _ => 0.0,
        };
        for dse in [
            EAT, SLEEP, HUNT, FORAGE, GROOM_SELF, MATE, COORDINATE, BUILD, MENTOR, CARETAKE,
            SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE, GROOM_OTHER,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Fight/Flee dse {dse} unchanged at full deficit + corner; got {out}"
            );
        }
    }

    #[test]
    fn adrenaline_fight_does_not_resurrect_zero_score() {
        // Gated-boost contract — Fight at zero stays at zero (no
        // adrenaline-conjured combat). Flee at zero stays at zero (no
        // negative-resurrection: suppressing a non-existent path
        // shouldn't drag the score below zero).
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            ESCAPE_VIABILITY => 0.0,
            _ => 0.0,
        };
        for dse in [FIGHT, FLEE] {
            let out = modifier.apply(DseId(dse), 0.0, &ctx, &fetch);
            assert_eq!(
                out, 0.0,
                "zero-score dse {dse} stays zero — no resurrection / no negative dive"
            );
        }
    }

    #[test]
    fn adrenaline_fight_gate_strict_inequality_at_boundary() {
        // Boundary semantics — `escape_viability == threshold` does NOT
        // trip the gate (strict `<`). Above-or-at threshold belongs to
        // 047's Flee branch; only strictly-below trips into Fight.
        let modifier = test_adrenaline_fight();
        let (_, ctx) = test_ctx();
        let fetch_at = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            ESCAPE_VIABILITY => 0.4, // exactly at threshold
            _ => 0.0,
        };
        let fight_at = modifier.apply(DseId(FIGHT), 0.5, &ctx, &fetch_at);
        assert!(
            (fight_at - 0.5).abs() < 1e-6,
            "viability at threshold does not trip gate; got {fight_at}"
        );
        let fetch_below = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            ESCAPE_VIABILITY => 0.39,
            _ => 0.0,
        };
        let fight_below = modifier.apply(DseId(FIGHT), 0.5, &ctx, &fetch_below);
        assert!(
            (fight_below - 1.0).abs() < 1e-5,
            "viability below threshold trips gate; got {fight_below}"
        );
    }

    #[test]
    fn adrenaline_fight_mutual_exclusion_with_flee_branch() {
        // Pipeline composition test: under the gate (cornered cat with
        // saturated deficit), 047 lifts Flee by +0.60 and 102 suppresses
        // by -0.50, netting +0.10 — Flee is held near baseline rather
        // than promoted, while Fight gets the full +0.50 lurch. This is
        // the design target: under the gate, the cornered cat fights
        // instead of fleeing, and the two valences don't both fire.
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(test_adrenaline()));
        pipeline.push(Box::new(test_adrenaline_fight()));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            ESCAPE_VIABILITY => 0.1,
            _ => 0.0,
        };
        // Flee: pre 0.50 → +0.60 (047) → -0.50 (102) = 0.60.
        let flee_after = pipeline.apply(DseId(FLEE), 0.50, &ctx, &fetch);
        assert!(
            (flee_after - 0.60).abs() < 1e-5,
            "Flee net under gate ≈ base + (flee_lift − fight_lift); got {flee_after}"
        );
        // Fight: pre 0.50 → +0.50 (102 only — 047 doesn't touch Fight) = 1.00.
        let fight_after = pipeline.apply(DseId(FIGHT), 0.50, &ctx, &fetch);
        assert!(
            (fight_after - 1.00).abs() < 1e-5,
            "Fight under gate gets full Fight lift; got {fight_after}"
        );
    }

    #[test]
    fn body_distress_promotion_composes_with_stockpile_satiation() {
        // Substrate composition: under high stockpile + high body
        // distress, BodyDistressPromotion lifts Eat (+0.20) BEFORE
        // StockpileSatiation damps Hunt/Forage. Eat wins by a margin
        // larger than either modifier alone could produce. Mirrors
        // the registration order documented at modifier.rs:883
        // (additive lifts before multiplicative damps) and the 094
        // doc-comment that pre-described this composition.
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(BodyDistressPromotion { threshold: 0.7, lift_scale: 0.20 }));
        pipeline.push(Box::new(StockpileSatiation { threshold: 0.5, scale: 0.85 }));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.04,           // food_fraction = 0.96 (full stockpile)
            BODY_DISTRESS_COMPOSITE => 1.0,  // full body distress
            _ => 0.0,
        };
        // Hunt: lift +0.20 then damp × (1 - 0.782) = ~0.218 ⇒
        //   pre-modifier 0.85 → +0.20 = 1.05 → × 0.218 ≈ 0.229
        let hunt_after = pipeline.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        // Eat: lift +0.20, no damp ⇒ 0.27 → 0.47.
        let eat_after = pipeline.apply(DseId(EAT), 0.27, &ctx, &fetch);
        assert!(
            eat_after > hunt_after,
            "post-pipeline: Eat ({eat_after}) wins over Hunt ({hunt_after}) under high stockpile + high distress"
        );
        // Eat lifted by exactly +0.20 (no damp on Eat).
        assert!(
            (eat_after - 0.47).abs() < 1e-6,
            "Eat lifted by +0.20; got {eat_after}"
        );
        // Hunt lifted then damped — bounded check rather than exact
        // (the f32 chain through both transforms accumulates ~1e-5).
        assert!(
            hunt_after < 0.25,
            "Hunt damped despite lift; got {hunt_after}"
        );
    }

    // -----------------------------------------------------------------------
    // ticket 105 AcuteHealthAdrenalineFreeze
    // -----------------------------------------------------------------------

    fn test_adrenaline_freeze() -> AcuteHealthAdrenalineFreeze {
        AcuteHealthAdrenalineFreeze {
            threshold: 0.4,
            freeze_lift: 0.70,
            viability_threshold: 0.4,
        }
    }

    #[test]
    fn adrenaline_freeze_no_lift_below_health_threshold() {
        let modifier = test_adrenaline_freeze();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.3,
            ESCAPE_VIABILITY => 0.1,
            _ => 0.0,
        };
        let hide = modifier.apply(DseId(HIDE), 0.5, &ctx, &fetch);
        assert!((hide - 0.5).abs() < 1e-6, "Hide unchanged below threshold; got {hide}");
    }

    #[test]
    fn adrenaline_freeze_no_lift_when_escape_viable() {
        // 047's Flee / 102's Fight take the response when escape is
        // viable. Freeze stays quiet.
        let modifier = test_adrenaline_freeze();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            ESCAPE_VIABILITY => 0.8,
            _ => 0.0,
        };
        let hide = modifier.apply(DseId(HIDE), 0.5, &ctx, &fetch);
        assert!((hide - 0.5).abs() < 1e-6, "Hide unchanged when escape viable; got {hide}");
    }

    #[test]
    fn adrenaline_freeze_full_lift_under_gate() {
        let modifier = test_adrenaline_freeze();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.55,
            ESCAPE_VIABILITY => 0.1,
            _ => 0.0,
        };
        let hide = modifier.apply(DseId(HIDE), 0.5, &ctx, &fetch);
        // 0.5 + 1.0 * 0.70 = 1.20
        assert!((hide - 1.20).abs() < 1e-5, "Hide saturated lift; got {hide}");
    }

    #[test]
    fn adrenaline_freeze_smoothstep_midpoint_under_gate() {
        let modifier = test_adrenaline_freeze();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 0.45,
            ESCAPE_VIABILITY => 0.1,
            _ => 0.0,
        };
        let hide = modifier.apply(DseId(HIDE), 0.5, &ctx, &fetch);
        // 0.5 + 0.5 * 0.70 = 0.85
        assert!((hide - 0.85).abs() < 1e-5, "Hide half-lift; got {hide}");
    }

    #[test]
    fn adrenaline_freeze_targets_only_hide() {
        let modifier = test_adrenaline_freeze();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            ESCAPE_VIABILITY => 0.0,
            _ => 0.0,
        };
        for dse in [
            EAT, SLEEP, HUNT, FORAGE, GROOM_SELF, GROOM_OTHER, FLEE, FIGHT, MATE, COORDINATE,
            BUILD, MENTOR, CARETAKE, SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Hide dse {dse} unchanged at full deficit + corner; got {out}"
            );
        }
    }

    #[test]
    fn adrenaline_freeze_does_not_resurrect_zero_score() {
        // The double-inert contract: in Phase 1, Hide always scores
        // 0 (gated off by `HideEligible`), so even if 105's lift
        // were nonzero the gated-boost contract keeps Hide at 0.
        let modifier = test_adrenaline_freeze();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            ESCAPE_VIABILITY => 0.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(HIDE), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero-score Hide stays zero — double-inert");
    }

    #[test]
    fn adrenaline_freeze_default_inert() {
        // Phase 1 substrate contract: with the shipped 0.0 lift
        // default, 105 MUST be score-bit-identical to baseline
        // regardless of inputs. This is the modifier-level half of
        // the double-inert contract; the DSE-level half is `HideDse`'s
        // never-eligible gate.
        let constants = crate::resources::sim_constants::SimConstants::default();
        let modifier = AcuteHealthAdrenalineFreeze::new(&constants.scoring);
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HEALTH_DEFICIT => 1.0,
            ESCAPE_VIABILITY => 0.0,
            _ => 0.0,
        };
        let hide = modifier.apply(DseId(HIDE), 0.5, &ctx, &fetch);
        assert!(
            (hide - 0.5).abs() < 1e-6,
            "Phase-1 inert: Hide unchanged at default 0.0 lift; got {hide}"
        );
    }

    // -----------------------------------------------------------------------
    // ticket 106 HungerUrgency
    // -----------------------------------------------------------------------

    fn test_hunger_urgency() -> HungerUrgency {
        HungerUrgency {
            threshold: 0.6,
            eat_lift: 0.40,
            hunt_lift: 0.20,
            forage_lift: 0.20,
        }
    }

    #[test]
    fn hunger_urgency_no_lift_below_threshold() {
        // urgency = 0.5 < threshold (0.6). Eat / Hunt / Forage unchanged.
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 0.5,
            _ => 0.0,
        };
        for dse in [EAT, HUNT, FORAGE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "below-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn hunger_urgency_zero_lift_at_threshold() {
        // urgency = 0.6 exactly. `<= threshold` short-circuits to zero
        // lift — boundary semantics.
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 0.6,
            _ => 0.0,
        };
        for dse in [EAT, HUNT, FORAGE] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "at-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn hunger_urgency_lifts_above_threshold() {
        // urgency = 0.8, threshold = 0.6. ramp = (0.8 - 0.6) / 0.4 = 0.5.
        // Eat: 0.5 + 0.5 * 0.40 = 0.70. Hunt / Forage: 0.5 + 0.5 * 0.20 = 0.60.
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 0.8,
            _ => 0.0,
        };
        let eat = modifier.apply(DseId(EAT), 0.5, &ctx, &fetch);
        let hunt = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        let forage = modifier.apply(DseId(FORAGE), 0.5, &ctx, &fetch);
        assert!((eat - 0.70).abs() < 1e-5, "Eat half-ramp lift; got {eat}");
        assert!((hunt - 0.60).abs() < 1e-5, "Hunt half-ramp lift; got {hunt}");
        assert!((forage - 0.60).abs() < 1e-5, "Forage half-ramp lift; got {forage}");
    }

    #[test]
    fn hunger_urgency_max_lift_at_full_urgency() {
        // urgency = 1.0 ⇒ ramp = 1.0 ⇒ full per-DSE lift.
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        let eat = modifier.apply(DseId(EAT), 0.5, &ctx, &fetch);
        let hunt = modifier.apply(DseId(HUNT), 0.5, &ctx, &fetch);
        let forage = modifier.apply(DseId(FORAGE), 0.5, &ctx, &fetch);
        assert!((eat - 0.90).abs() < 1e-5, "Eat full lift +0.40; got {eat}");
        assert!((hunt - 0.70).abs() < 1e-5, "Hunt full lift +0.20; got {hunt}");
        assert!((forage - 0.70).abs() < 1e-5, "Forage full lift +0.20; got {forage}");
    }

    #[test]
    fn hunger_urgency_targets_only_eat_hunt_forage() {
        // At full urgency, every non-target DSE passes through unchanged.
        // The asymmetry is the substrate property: only food-acquisition
        // class is lifted; other self-care DSEs (Sleep / GroomSelf /
        // Flee) and the entire non-self-care class stay flat.
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        for dse in [
            SLEEP, GROOM_SELF, GROOM_OTHER, FLEE, FIGHT, MATE, COORDINATE, BUILD, MENTOR, CARETAKE,
            SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-food-class dse {dse} unchanged at full urgency; got {out}"
            );
        }
    }

    #[test]
    fn hunger_urgency_does_not_resurrect_zero_score() {
        // Gated-boost contract — hunger doesn't conjure food into
        // existence. Zero-score Eat / Hunt / Forage stays zero.
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        for dse in [EAT, HUNT, FORAGE] {
            let out = modifier.apply(DseId(dse), 0.0, &ctx, &fetch);
            assert_eq!(out, 0.0, "zero-score dse {dse} stays zero — no resurrection");
        }
    }

    #[test]
    fn hunger_urgency_composes_with_stockpile_satiation() {
        // Substrate composition: under high urgency + high stockpile,
        // Eat is lifted (+0.40 at full urgency) BEFORE StockpileSatiation
        // damps Hunt / Forage. Mirrors the 088 / 094 composition test.
        // The Eat-wins-the-contest target is preserved.
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(test_hunger_urgency()));
        pipeline.push(Box::new(StockpileSatiation { threshold: 0.5, scale: 0.85 }));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            FOOD_SCARCITY => 0.04, // food_fraction = 0.96
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        let eat_after = pipeline.apply(DseId(EAT), 0.27, &ctx, &fetch);
        let hunt_after = pipeline.apply(DseId(HUNT), 0.85, &ctx, &fetch);
        // Eat: 0.27 + 0.40 = 0.67 (no damp).
        assert!((eat_after - 0.67).abs() < 1e-5, "Eat lifted by +0.40; got {eat_after}");
        // Hunt: (0.85 + 0.20) × ~0.218 ≈ 0.229.
        assert!(
            eat_after > hunt_after,
            "post-pipeline: Eat ({eat_after}) wins over Hunt ({hunt_after}) under abundance + urgency"
        );
    }

    #[test]
    fn hunger_urgency_starvation_regime_pinned() {
        // Pin the alignment between substrate and legacy interrupt:
        // at urgency 0.85 (hunger 0.15 — exactly the legacy
        // `starvation_interrupt_threshold`), the substrate is well
        // into the lift regime. ramp = (0.85 - 0.6) / 0.4 = 0.625.
        // Eat: 0.5 + 0.625 * 0.40 = 0.75 (substantial promotion).
        let modifier = test_hunger_urgency();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 0.85,
            _ => 0.0,
        };
        let eat = modifier.apply(DseId(EAT), 0.5, &ctx, &fetch);
        assert!(
            (eat - 0.75).abs() < 1e-5,
            "Eat at legacy-interrupt-equivalent urgency 0.85; got {eat}"
        );
    }

    // -----------------------------------------------------------------------
    // ticket 107 ExhaustionPressure
    // -----------------------------------------------------------------------

    fn test_exhaustion_pressure() -> ExhaustionPressure {
        ExhaustionPressure {
            threshold: 0.7,
            sleep_lift: 0.40,
            groom_lift: 0.10,
        }
    }

    #[test]
    fn exhaustion_pressure_no_lift_below_threshold() {
        let modifier = test_exhaustion_pressure();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ENERGY_DEFICIT => 0.5,
            _ => 0.0,
        };
        for dse in [SLEEP, GROOM_SELF] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "below-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn exhaustion_pressure_zero_lift_at_threshold() {
        let modifier = test_exhaustion_pressure();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ENERGY_DEFICIT => 0.7,
            _ => 0.0,
        };
        for dse in [SLEEP, GROOM_SELF] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "at-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn exhaustion_pressure_lifts_above_threshold() {
        // deficit = 0.85, threshold = 0.7. ramp = 0.5.
        // Sleep: 0.5 + 0.5 * 0.40 = 0.70. Groom: 0.5 + 0.5 * 0.10 = 0.55.
        let modifier = test_exhaustion_pressure();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ENERGY_DEFICIT => 0.85,
            _ => 0.0,
        };
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        let groom = modifier.apply(DseId(GROOM_SELF), 0.5, &ctx, &fetch);
        assert!((sleep - 0.70).abs() < 1e-5, "Sleep half-lift; got {sleep}");
        assert!((groom - 0.55).abs() < 1e-5, "Groom half-lift; got {groom}");
    }

    #[test]
    fn exhaustion_pressure_max_lift_at_full_deficit() {
        let modifier = test_exhaustion_pressure();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ENERGY_DEFICIT => 1.0,
            _ => 0.0,
        };
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        let groom = modifier.apply(DseId(GROOM_SELF), 0.5, &ctx, &fetch);
        assert!((sleep - 0.90).abs() < 1e-5, "Sleep full lift +0.40; got {sleep}");
        assert!((groom - 0.60).abs() < 1e-5, "Groom full lift +0.10; got {groom}");
    }

    #[test]
    fn exhaustion_pressure_targets_only_sleep_and_groom_self() {
        // Eat / Hunt / Forage / Flee + non-self-care class all unchanged.
        let modifier = test_exhaustion_pressure();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ENERGY_DEFICIT => 1.0,
            _ => 0.0,
        };
        for dse in [
            EAT, HUNT, FORAGE, GROOM_OTHER, FLEE, FIGHT, MATE, COORDINATE, BUILD, MENTOR, CARETAKE,
            SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-rest-class dse {dse} unchanged at full deficit; got {out}"
            );
        }
    }

    #[test]
    fn exhaustion_pressure_does_not_resurrect_zero_score() {
        let modifier = test_exhaustion_pressure();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            ENERGY_DEFICIT => 1.0,
            _ => 0.0,
        };
        for dse in [SLEEP, GROOM_SELF] {
            let out = modifier.apply(DseId(dse), 0.0, &ctx, &fetch);
            assert_eq!(out, 0.0, "zero-score dse {dse} stays zero — no resurrection");
        }
    }

    #[test]
    fn exhaustion_pressure_composes_additively_with_body_distress() {
        // Under high body_distress_composite (088) AND high
        // energy_deficit (107), Sleep sees both lifts add. 088 lift
        // +0.20 at composite 1.0 + 107 sleep_lift 0.40 = +0.60 total.
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(BodyDistressPromotion {
            threshold: 0.7,
            lift_scale: 0.20,
        }));
        pipeline.push(Box::new(test_exhaustion_pressure()));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            BODY_DISTRESS_COMPOSITE => 1.0,
            ENERGY_DEFICIT => 1.0,
            _ => 0.0,
        };
        let sleep_after = pipeline.apply(DseId(SLEEP), 0.30, &ctx, &fetch);
        // 0.30 + 0.20 (088) + 0.40 (107) = 0.90.
        assert!(
            (sleep_after - 0.90).abs() < 1e-5,
            "Sleep additive composition; got {sleep_after}"
        );
    }

    // -----------------------------------------------------------------------
    // ticket 110 ThermalDistress
    // -----------------------------------------------------------------------

    fn test_thermal_distress() -> ThermalDistress {
        ThermalDistress {
            threshold: 0.7,
            sleep_lift: 0.30,
        }
    }

    #[test]
    fn thermal_distress_no_lift_below_threshold() {
        let modifier = test_thermal_distress();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THERMAL_DEFICIT => 0.5,
            _ => 0.0,
        };
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((sleep - 0.5).abs() < 1e-6, "below-threshold Sleep unchanged; got {sleep}");
    }

    #[test]
    fn thermal_distress_zero_lift_at_threshold() {
        let modifier = test_thermal_distress();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THERMAL_DEFICIT => 0.7,
            _ => 0.0,
        };
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((sleep - 0.5).abs() < 1e-6, "at-threshold Sleep unchanged; got {sleep}");
    }

    #[test]
    fn thermal_distress_lifts_above_threshold() {
        // deficit = 0.85, threshold = 0.7. ramp = 0.5.
        // Sleep: 0.5 + 0.5 * 0.30 = 0.65.
        let modifier = test_thermal_distress();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THERMAL_DEFICIT => 0.85,
            _ => 0.0,
        };
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((sleep - 0.65).abs() < 1e-5, "Sleep half-lift; got {sleep}");
    }

    #[test]
    fn thermal_distress_max_lift_at_full_deficit() {
        let modifier = test_thermal_distress();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THERMAL_DEFICIT => 1.0,
            _ => 0.0,
        };
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((sleep - 0.80).abs() < 1e-5, "Sleep full lift +0.30; got {sleep}");
    }

    #[test]
    fn thermal_distress_targets_only_sleep() {
        // GroomSelf is *not* lifted by ThermalDistress in v1 (Build
        // also out-of-scope per ticket §Out-of-scope). Pure
        // sleep-lift modifier.
        let modifier = test_thermal_distress();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THERMAL_DEFICIT => 1.0,
            _ => 0.0,
        };
        for dse in [
            EAT, HUNT, FORAGE, GROOM_SELF, GROOM_OTHER, FLEE, FIGHT, MATE, COORDINATE, BUILD,
            MENTOR, CARETAKE, SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Sleep dse {dse} unchanged at full thermal deficit; got {out}"
            );
        }
    }

    #[test]
    fn thermal_distress_does_not_resurrect_zero_score() {
        let modifier = test_thermal_distress();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THERMAL_DEFICIT => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(SLEEP), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero-score Sleep stays zero — no resurrection");
    }

    // -----------------------------------------------------------------------
    // ticket 108 ThreatProximityAdrenalineFlee
    // -----------------------------------------------------------------------

    fn test_threat_proximity_adrenaline() -> ThreatProximityAdrenalineFlee {
        ThreatProximityAdrenalineFlee {
            threshold: 0.4,
            flee_lift: 0.60,
            sleep_lift: 0.50,
            viability_threshold: 0.4,
        }
    }

    #[test]
    fn threat_proximity_adrenaline_no_lift_below_derivative_threshold() {
        // derivative = 0.3 < threshold (0.4). Flee / Sleep unchanged
        // even with viability above gate.
        let modifier = test_threat_proximity_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 0.3,
            ESCAPE_VIABILITY => 0.8,
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "below-derivative-threshold dse {dse} unchanged; got {out}"
            );
        }
    }

    #[test]
    fn threat_proximity_adrenaline_no_lift_when_escape_not_viable() {
        // Derivative saturated, but escape_viability < gate threshold ⇒
        // future Fight valence (108b) owns the response, this Flee
        // branch stays quiet. The viability predicate is what splits
        // Flee from Fight under the same threat-proximity scalar.
        let modifier = test_threat_proximity_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 0.55,
            ESCAPE_VIABILITY => 0.1, // cornered ⇒ Fight branch territory
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "no Flee lift when escape not viable; dse {dse} got {out}"
            );
        }
    }

    #[test]
    fn threat_proximity_adrenaline_full_lift_under_viability_gate() {
        // Saturated derivative + viable escape. Flee +0.60, Sleep +0.50.
        let modifier = test_threat_proximity_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 0.55,
            ESCAPE_VIABILITY => 0.8,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((flee - 1.10).abs() < 1e-5, "Flee saturated lift; got {flee}");
        assert!((sleep - 1.00).abs() < 1e-5, "Sleep saturated lift; got {sleep}");
    }

    #[test]
    fn threat_proximity_adrenaline_smoothstep_midpoint() {
        // derivative = 0.45 = threshold + half-width. Smoothstep at
        // t = 0.5 ⇒ ramp = 0.5 ⇒ Flee +0.30, Sleep +0.25.
        let modifier = test_threat_proximity_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 0.45,
            ESCAPE_VIABILITY => 0.8,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        let sleep = modifier.apply(DseId(SLEEP), 0.5, &ctx, &fetch);
        assert!((flee - 0.80).abs() < 1e-5, "Flee half-lift; got {flee}");
        assert!((sleep - 0.75).abs() < 1e-5, "Sleep half-lift; got {sleep}");
    }

    #[test]
    fn threat_proximity_adrenaline_targets_only_flee_and_sleep() {
        let modifier = test_threat_proximity_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 1.0,
            ESCAPE_VIABILITY => 1.0,
            _ => 0.0,
        };
        for dse in [
            EAT, HUNT, FORAGE, GROOM_SELF, GROOM_OTHER, FIGHT, MATE, COORDINATE, BUILD, MENTOR,
            CARETAKE, SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Flee/Sleep dse {dse} unchanged at full derivative; got {out}"
            );
        }
    }

    #[test]
    fn threat_proximity_adrenaline_does_not_resurrect_zero_score() {
        let modifier = test_threat_proximity_adrenaline();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 1.0,
            ESCAPE_VIABILITY => 1.0,
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.0, &ctx, &fetch);
            assert_eq!(out, 0.0, "zero-score dse {dse} stays zero — no resurrection");
        }
    }

    #[test]
    fn threat_proximity_adrenaline_default_inert() {
        // Phase 1 substrate contract: with the shipped 0.0 lift defaults,
        // 108 MUST be score-bit-identical to baseline regardless of
        // derivative or viability inputs.
        let constants = crate::resources::sim_constants::SimConstants::default();
        let modifier = ThreatProximityAdrenalineFlee::new(&constants.scoring);
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            THREAT_PROXIMITY_DERIVATIVE => 1.0,
            ESCAPE_VIABILITY => 1.0,
            _ => 0.0,
        };
        for dse in [FLEE, SLEEP] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "Phase-1 inert: dse {dse} unchanged at default 0.0 lifts; got {out}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // ticket 109 IntraspeciesConflictResponseFlight (Phase A)
    // -----------------------------------------------------------------------

    fn test_intraspecies_conflict_flight() -> IntraspeciesConflictResponseFlight {
        IntraspeciesConflictResponseFlight {
            threshold: 0.6,
            flee_lift: 0.30,
        }
    }

    #[test]
    fn intraspecies_conflict_flight_no_lift_below_threshold() {
        let modifier = test_intraspecies_conflict_flight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            SOCIAL_STATUS_DISTRESS => 0.5,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((flee - 0.5).abs() < 1e-6, "below-threshold Flee unchanged; got {flee}");
    }

    #[test]
    fn intraspecies_conflict_flight_lifts_above_threshold() {
        // distress = 0.8, threshold = 0.6. ramp = 0.5. Flee += 0.15.
        let modifier = test_intraspecies_conflict_flight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            SOCIAL_STATUS_DISTRESS => 0.8,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((flee - 0.65).abs() < 1e-5, "Flee half-lift; got {flee}");
    }

    #[test]
    fn intraspecies_conflict_flight_max_lift_at_full_distress() {
        let modifier = test_intraspecies_conflict_flight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            SOCIAL_STATUS_DISTRESS => 1.0,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!((flee - 0.80).abs() < 1e-5, "Flee full lift +0.30; got {flee}");
    }

    #[test]
    fn intraspecies_conflict_flight_targets_only_flee() {
        let modifier = test_intraspecies_conflict_flight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            SOCIAL_STATUS_DISTRESS => 1.0,
            _ => 0.0,
        };
        for dse in [
            EAT, HUNT, FORAGE, GROOM_SELF, SLEEP, GROOM_OTHER, FIGHT, MATE, COORDINATE, BUILD,
            MENTOR, CARETAKE, SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Flee dse {dse} unchanged at full social distress; got {out}"
            );
        }
    }

    #[test]
    fn intraspecies_conflict_flight_does_not_resurrect_zero_score() {
        let modifier = test_intraspecies_conflict_flight();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            SOCIAL_STATUS_DISTRESS => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(FLEE), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero-score Flee stays zero — no resurrection");
    }

    #[test]
    fn intraspecies_conflict_flight_default_inert() {
        let constants = crate::resources::sim_constants::SimConstants::default();
        let modifier = IntraspeciesConflictResponseFlight::new(&constants.scoring);
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            SOCIAL_STATUS_DISTRESS => 1.0,
            _ => 0.0,
        };
        let flee = modifier.apply(DseId(FLEE), 0.5, &ctx, &fetch);
        assert!(
            (flee - 0.5).abs() < 1e-6,
            "Phase-1 inert: Flee unchanged at default 0.0 lift; got {flee}"
        );
    }

    #[test]
    fn pressure_modifiers_default_inert() {
        // Phase-1 substrate contract: with the shipped 0.0 lift defaults,
        // the new pressure modifiers MUST be score-bit-identical to the
        // pre-Wave-1 baseline regardless of scalar inputs. Composes the
        // three at default magnitude against a saturated-everything
        // scalar profile and asserts no score moves.
        let constants = crate::resources::sim_constants::SimConstants::default();
        let sc = &constants.scoring;
        use crate::ai::eval::ModifierPipeline;
        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(HungerUrgency::new(sc)));
        pipeline.push(Box::new(ExhaustionPressure::new(sc)));
        pipeline.push(Box::new(ThermalDistress::new(sc)));
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY | ENERGY_DEFICIT | THERMAL_DEFICIT => 1.0,
            _ => 0.0,
        };
        for dse in [EAT, HUNT, FORAGE, SLEEP, GROOM_SELF] {
            let out = pipeline.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "Phase-1 inert: dse {dse} unchanged with 0.0 default lifts; got {out}"
            );
        }
    }
}
