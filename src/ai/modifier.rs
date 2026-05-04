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
/// Ticket 156 — `KittenCryCaretakeLift` Modifier trigger input.
/// Already published by `scoring::ctx_scalars` as the per-cat
/// sample of `KittenCryMap` at the cat's tile. Read directly so the
/// lift fires on perceived distress alone, independent of the
/// cat's own `kitten_urgency` axis (which is a non-spatial in-engine
/// urgency, not a spatial perception).
const KITTEN_CRY_PERCEIVED: &str = "kitten_cry_perceived";
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

// Disposition-failure cooldown scalar keys, one per failure-prone
// DispositionKind. 1.0 = no recent failure (no damp);
// 0.0 = just failed (full damp).
const DISPOSITION_FAILURE_SIGNAL_HUNTING: &str = "disposition_failure_signal_hunting";
const DISPOSITION_FAILURE_SIGNAL_FORAGING: &str = "disposition_failure_signal_foraging";
const DISPOSITION_FAILURE_SIGNAL_CRAFTING: &str = "disposition_failure_signal_crafting";
const DISPOSITION_FAILURE_SIGNAL_CARETAKING: &str = "disposition_failure_signal_caretaking";
const DISPOSITION_FAILURE_SIGNAL_BUILDING: &str = "disposition_failure_signal_building";
const DISPOSITION_FAILURE_SIGNAL_MATING: &str = "disposition_failure_signal_mating";
const DISPOSITION_FAILURE_SIGNAL_MENTORING: &str = "disposition_failure_signal_mentoring";

// Memory-event proximity sums — Σ proximity × strength across the
// cat's memory entries filtered by event type. Aggregated at
// ScoringContext build time.
const MEMORY_RESOURCE_FOUND_PROXIMITY_SUM: &str = "memory_resource_found_proximity_sum";
const MEMORY_DEATH_PROXIMITY_SUM: &str = "memory_death_proximity_sum";
const MEMORY_THREAT_SEEN_PROXIMITY_SUM: &str = "memory_threat_seen_proximity_sum";

// ColonyKnowledge proximity sums — same shape as memory sums but
// across colony-wide hearsay entries.
const COLONY_KNOWLEDGE_RESOURCE_PROXIMITY: &str = "colony_knowledge_resource_proximity";
const COLONY_KNOWLEDGE_THREAT_PROXIMITY: &str = "colony_knowledge_threat_proximity";

// ColonyPriority ordinal: -1 = none, 0 Food, 1 Defense, 2 Building,
// 3 Exploration. Read by `ColonyPriorityLift`.
const COLONY_PRIORITY_ORDINAL: &str = "colony_priority_ordinal";
const COLONY_PRIORITY_FOOD: f32 = 0.0;
const COLONY_PRIORITY_DEFENSE: f32 = 1.0;
const COLONY_PRIORITY_BUILDING: f32 = 2.0;
const COLONY_PRIORITY_EXPLORATION: f32 = 3.0;

// Per-action cascade-count keys, read by `NeighborActionCascade`.
const CASCADE_COUNT_HUNT: &str = "cascade_count_hunt";
const CASCADE_COUNT_FORAGE: &str = "cascade_count_forage";
const CASCADE_COUNT_EAT: &str = "cascade_count_eat";
const CASCADE_COUNT_SLEEP: &str = "cascade_count_sleep";
const CASCADE_COUNT_WANDER: &str = "cascade_count_wander";
const CASCADE_COUNT_IDLE: &str = "cascade_count_idle";
const CASCADE_COUNT_SOCIALIZE: &str = "cascade_count_socialize";
const CASCADE_COUNT_GROOM: &str = "cascade_count_groom";
const CASCADE_COUNT_EXPLORE: &str = "cascade_count_explore";
const CASCADE_COUNT_FLEE: &str = "cascade_count_flee";
const CASCADE_COUNT_PATROL: &str = "cascade_count_patrol";
const CASCADE_COUNT_BUILD: &str = "cascade_count_build";
const CASCADE_COUNT_FARM: &str = "cascade_count_farm";
const CASCADE_COUNT_HERBCRAFT: &str = "cascade_count_herbcraft";
const CASCADE_COUNT_PRACTICEMAGIC: &str = "cascade_count_practicemagic";
const CASCADE_COUNT_COORDINATE: &str = "cascade_count_coordinate";
const CASCADE_COUNT_MENTOR: &str = "cascade_count_mentor";
const CASCADE_COUNT_MATE: &str = "cascade_count_mate";
const CASCADE_COUNT_CARETAKE: &str = "cascade_count_caretake";
const CASCADE_COUNT_COOK: &str = "cascade_count_cook";
const CASCADE_COUNT_HIDE: &str = "cascade_count_hide";

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
    // against known variants. Values outside `[0, 14]` are treated as
    // "no active disposition" defensively. 150 R5a appends `Eating` as
    // ordinal 13; 154 appends `Mentoring` as ordinal 14. Existing
    // 1..=12 ordinals stay stable across both extensions.
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
        // 154: Socializing drops MENTOR (which moved to the new
        // Mentoring disposition at ordinal 14). Socialize + Groom
        // remain.
        5 => Some(&[SOCIALIZE, GROOM_SELF, GROOM_OTHER]),
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
        // 154: Mentoring → Mentor. Single-action disposition; lifts
        // apply to the Mentor DSE alone while the cat is committed.
        14 => Some(&[MENTOR]),
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
    curve_exponent: f32,
}

impl HungerUrgency {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.hunger_urgency_threshold,
            eat_lift: sc.hunger_urgency_eat_lift,
            hunt_lift: sc.hunger_urgency_hunt_lift,
            forage_lift: sc.hunger_urgency_forage_lift,
            curve_exponent: sc.hunger_urgency_curve_exponent,
        }
    }

    /// Returns the lift fraction `[0, 1]` for the given `hunger_urgency`.
    /// Below `threshold` returns 0; above, scales to 1.0 at urgency = 1.0.
    ///
    /// Curve shape is `((urgency − threshold) / (1 − threshold))^k` where
    /// `k = curve_exponent`. **Default `k = 1.0` is linear** (preserves
    /// shipped behavior). Sub-linear `k < 1.0` (e.g. `0.4`) gives a
    /// *leading nerve-impulse* shape: the lift saturates fast in the
    /// early band and plateaus near max well before hunger drops into
    /// starvation territory. This is the input-curve half of the
    /// 032 leading/trailing pair (input leads damage); see
    /// `docs/balance/starvation-rebalance.md` for the matched-pair
    /// rationale.
    fn ramp(&self, urgency: f32) -> f32 {
        if urgency <= self.threshold {
            return 0.0;
        }
        let raw = ((urgency - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0);
        if self.curve_exponent == 1.0 {
            raw
        } else {
            raw.powf(self.curve_exponent)
        }
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
// KittenCryCaretakeLift — ticket 156
// ---------------------------------------------------------------------------

/// Ticket 156 — additive lift on the `caretake` DSE for non-kitten
/// cats hearing a kitten distress cry. Reads the per-cat
/// `kitten_cry_perceived` scalar (sampled from `KittenCryMap` at the
/// cat's tile during `ScoringContext` construction) and adds a
/// linearly-ramped lift to the Caretake DSE.
///
/// **Why a modifier vs a CaretakeDse axis.** Phase 4 of ticket 156
/// initially added cry as a fourth `WeightedSum` axis on
/// `CaretakeDse`, requiring a weight rebalance from `[0.45, 0.30,
/// 0.25]` to `[0.40, 0.25, 0.20, 0.15]`. The rebalance compressed
/// the legacy three axes by ~40% to make room for the new fourth
/// axis. Empirically (seed-42 soak post-Phase-4) Caretake count
/// dropped from 56 → 51 because the cry axis is mostly 0 (no kitten
/// crying near most adults at most ticks), so the average score
/// dropped 40% while the cry boost only fired occasionally.
///
/// The modifier-layer consumer fixes the regression: when no cry is
/// heard, the post-modifier score is bit-identical to the pre-156
/// baseline. When cry is heard, an additive lift on top of the
/// legacy score promotes Caretake without compressing its base.
/// Same composition pattern as `KittenEatBoost` for the kitten
/// cohort — life-stage / cohort effects compose at the modifier
/// layer per the user-global "single-axis perception scalars"
/// discipline.
///
/// **Trigger:** non-kitten cat (`Kitten` marker absent — adults,
/// elders, young) AND `kitten_cry_perceived > threshold`.
///
/// **Transform:** linear ramp from `threshold` (lift 0) to
/// `kitten_cry_perceived = 1.0` (lift = `lift`).
///
/// **Composition:** Registered after `KittenEatBoost` (which targets
/// kittens' Eat). Composes additively with the legacy Caretake
/// weighted-sum score and with `Patience` / `Tradition` etc.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<=
/// 0` — perceived cry doesn't conjure caretake-eligibility into
/// existence (mirrors the 088 / 047 / 102 / 106 conventions).
pub struct KittenCryCaretakeLift {
    threshold: f32,
    lift: f32,
}

impl KittenCryCaretakeLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.kitten_cry_caretake_lift_threshold,
            lift: sc.kitten_cry_caretake_lift,
        }
    }
}

impl ScoreModifier for KittenCryCaretakeLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if dse_id.0 != CARETAKE {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        if (ctx.has_marker)(crate::components::markers::Kitten::KEY, ctx.cat) {
            return score;
        }
        if self.lift <= 0.0 {
            return score;
        }
        let perceived = fetch(KITTEN_CRY_PERCEIVED, ctx.cat).clamp(0.0, 1.0);
        if perceived <= self.threshold {
            return score;
        }
        let ramp = ((perceived - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0);
        score + ramp * self.lift
    }

    fn name(&self) -> &'static str {
        "kitten_cry_caretake_lift"
    }
}

// ---------------------------------------------------------------------------
// KittenEatBoost — ticket 156
// ---------------------------------------------------------------------------

/// Ticket 156 — kitten-cohort Eat lift. Multiplicatively boosts the
/// `eat` DSE score for cats carrying the `Kitten` lifestage marker
/// once their hunger urgency exceeds `kitten_eat_boost_threshold`,
/// reshaping the L2 breakdown so a hungry kitten's physiological
/// priorities (Eat) dominate the social / grooming DSEs instead of
/// being dominated by them.
///
/// **Why a per-cohort modifier vs a per-cohort DSE variant.** The
/// existing `EatDse` is a single weighted-product DSE with a hunger
/// curve and a stores-distance spatial axis. Splitting Eat into
/// `EatDse` + `KittenEatDse` would duplicate the struct + planner
/// wiring + completion proxy across two DseIds for a kitten cohort
/// that doesn't actually act on its own (kittens are passive feeders
/// — the action layer reduces all kitten DSE winners to Idle). The
/// load-bearing value of this modifier is **breakdown honesty** —
/// `just inspect` and the focal-trace L2 score panel must reflect
/// the kitten's physiological priorities, even when the action layer
/// can't act on them. A multiplicative lift composes correctly with
/// the existing `HungerUrgency` modifier (additive on the same DSE)
/// and with the kitten cohort's spatial-discount (the modifier
/// doesn't touch the spatial axis).
///
/// **Trigger:** `Kitten` marker present AND `hunger_urgency >
/// threshold`.
///
/// **Transform:** linear ramp from `threshold` (multiplier 1.0) to
/// `urgency = 1.0` (multiplier `multiplier`). At intermediate
/// urgencies the multiplier is `1 + ramp × (multiplier − 1)`.
///
/// **Composition:** Registered after `HungerUrgency` so the kitten
/// boost amplifies the post-Urgency lifted score, not the bare DSE
/// score. The two pass orders compose to a kitten-Eat score that
/// dominates other physiological DSEs as soon as urgency clears the
/// threshold.
///
/// **Gated-boost contract:** returns `score` unchanged on score `<= 0`
/// — the lift doesn't conjure food into existence (mirrors the 088
/// / 047 / 102 / 106 conventions).
pub struct KittenEatBoost {
    threshold: f32,
    multiplier: f32,
}

impl KittenEatBoost {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            threshold: sc.kitten_eat_boost_threshold,
            multiplier: sc.kitten_eat_boost_multiplier,
        }
    }
}

impl ScoreModifier for KittenEatBoost {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if dse_id.0 != EAT {
            return score;
        }
        if score <= 0.0 {
            return score;
        }
        if !(ctx.has_marker)(crate::components::markers::Kitten::KEY, ctx.cat) {
            return score;
        }
        if self.multiplier <= 1.0 {
            return score;
        }
        let urgency = fetch(HUNGER_URGENCY, ctx.cat).clamp(0.0, 1.0);
        if urgency <= self.threshold {
            return score;
        }
        let ramp = ((urgency - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0);
        let mul = 1.0 + ramp * (self.multiplier - 1.0);
        score * mul
    }

    fn name(&self) -> &'static str {
        "kitten_eat_boost"
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
// DispositionFailureCooldown
// ---------------------------------------------------------------------------

/// §3.5.1 disposition-failure cooldown damp.
///
/// **Trigger:** the DSE belongs to one of the seven failure-prone
/// dispositions (Hunting, Foraging, Crafting, Caretaking, Building,
/// Mating, Mentoring) AND that disposition's failure signal `< 1.0`.
///
/// **Transform:** multiplicative `score *= 0.1 + 0.9 * signal`. Signal
/// 0 → 0.1× (just-failed floor); signal 1 → 1.0× (no damp). Linear
/// interpolation inside the cooldown window.
///
/// **Applies to:**
/// - Hunting: `hunt`
/// - Foraging: `forage`
/// - Crafting: `cook`, `herbcraft_gather`, `herbcraft_prepare`,
///   `herbcraft_ward`, `magic_scry`, `magic_durable_ward`,
///   `magic_cleanse`, `magic_colony_cleanse`, `magic_harvest`,
///   `magic_commune`
/// - Caretaking: `caretake`
/// - Building: `build`
/// - Mating: `mate`
/// - Mentoring: `mentor`
///
/// Resting / Guarding / Socializing / Farming / Coordinating /
/// Exploring are exempt — their step graphs don't share the
/// `make_plan → None` retry pattern (different completion semantics
/// or different planner-failure modes).
///
/// **Pipeline position:** prepended *before* every other modifier so
/// additive bonuses (Pride / Independence / Patience / …) compose on
/// already-damped scores when the cat is in cooldown.
pub struct DispositionFailureCooldown;

impl DispositionFailureCooldown {
    pub fn new() -> Self {
        Self
    }

    fn signal_key(dse_id: DseId) -> Option<&'static str> {
        match dse_id.0 {
            HUNT => Some(DISPOSITION_FAILURE_SIGNAL_HUNTING),
            FORAGE => Some(DISPOSITION_FAILURE_SIGNAL_FORAGING),
            COOK
            | HERBCRAFT_GATHER
            | HERBCRAFT_PREPARE
            | HERBCRAFT_WARD
            | MAGIC_SCRY
            | MAGIC_DURABLE_WARD
            | MAGIC_CLEANSE
            | MAGIC_COLONY_CLEANSE
            | MAGIC_HARVEST
            | MAGIC_COMMUNE => Some(DISPOSITION_FAILURE_SIGNAL_CRAFTING),
            CARETAKE => Some(DISPOSITION_FAILURE_SIGNAL_CARETAKING),
            BUILD => Some(DISPOSITION_FAILURE_SIGNAL_BUILDING),
            MATE => Some(DISPOSITION_FAILURE_SIGNAL_MATING),
            MENTOR => Some(DISPOSITION_FAILURE_SIGNAL_MENTORING),
            _ => None,
        }
    }
}

impl Default for DispositionFailureCooldown {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoreModifier for DispositionFailureCooldown {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let Some(signal_key) = Self::signal_key(dse_id) else {
            return score;
        };
        let signal = fetch(signal_key, ctx.cat);
        if signal >= 1.0 {
            return score;
        }
        let damp = 0.1 + 0.9 * signal;
        score * damp
    }

    fn name(&self) -> &'static str {
        "disposition_failure_cooldown"
    }
}

// ---------------------------------------------------------------------------
// MemoryResourceFoundLift
// ---------------------------------------------------------------------------

/// §3.5.1 additive lift on Hunt / Forage when the cat remembers a
/// nearby `ResourceFound` event. Reads
/// `memory_resource_found_proximity_sum` (Σ proximity × strength
/// across qualifying entries; aggregated at ScoringContext build
/// time). `score += sum × memory_resource_bonus`.
pub struct MemoryResourceFoundLift {
    bonus: f32,
}

impl MemoryResourceFoundLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.memory_resource_bonus,
        }
    }
}

impl ScoreModifier for MemoryResourceFoundLift {
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
        let sum = fetch(MEMORY_RESOURCE_FOUND_PROXIMITY_SUM, ctx.cat);
        if sum <= 0.0 {
            return score;
        }
        score + sum * self.bonus
    }

    fn name(&self) -> &'static str {
        "memory_resource_found_lift"
    }
}

// ---------------------------------------------------------------------------
// MemoryDeathPenalty
// ---------------------------------------------------------------------------

/// §3.5.1 subtractive lift on Wander / Idle when the cat remembers a
/// nearby `Death` event (safety instinct). Reads
/// `memory_death_proximity_sum`. `score -= sum × memory_death_penalty`.
pub struct MemoryDeathPenalty {
    penalty: f32,
}

impl MemoryDeathPenalty {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            penalty: sc.memory_death_penalty,
        }
    }
}

impl ScoreModifier for MemoryDeathPenalty {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, WANDER | IDLE) {
            return score;
        }
        let sum = fetch(MEMORY_DEATH_PROXIMITY_SUM, ctx.cat);
        if sum <= 0.0 {
            return score;
        }
        score - sum * self.penalty
    }

    fn name(&self) -> &'static str {
        "memory_death_penalty"
    }
}

// ---------------------------------------------------------------------------
// MemoryThreatSeenSuppress
// ---------------------------------------------------------------------------

/// §3.5.1 subtractive lift on Wander / Explore / Hunt when the cat
/// remembers a nearby `ThreatSeen` event. Reads
/// `memory_threat_seen_proximity_sum`. `score -= sum × memory_threat_penalty`.
pub struct MemoryThreatSeenSuppress {
    penalty: f32,
}

impl MemoryThreatSeenSuppress {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            penalty: sc.memory_threat_penalty,
        }
    }
}

impl ScoreModifier for MemoryThreatSeenSuppress {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, WANDER | EXPLORE | HUNT) {
            return score;
        }
        let sum = fetch(MEMORY_THREAT_SEEN_PROXIMITY_SUM, ctx.cat);
        if sum <= 0.0 {
            return score;
        }
        score - sum * self.penalty
    }

    fn name(&self) -> &'static str {
        "memory_threat_seen_suppress"
    }
}

// ---------------------------------------------------------------------------
// ColonyKnowledgeLift
// ---------------------------------------------------------------------------

/// §3.5.1 additive lift driven by colony-wide hearsay. Two arms keyed
/// by event type:
/// - resource arm: `score += colony_knowledge_resource_proximity × scale`
///   on hunt, forage.
/// - threat arm: `score += colony_knowledge_threat_proximity × scale`
///   on patrol.
///
/// Both sums are pre-aggregated at ScoringContext build time over
/// `ColonyKnowledge.entries` filtered by event type and proximity.
pub struct ColonyKnowledgeLift {
    scale: f32,
}

impl ColonyKnowledgeLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            scale: sc.colony_knowledge_bonus_scale,
        }
    }
}

impl ScoreModifier for ColonyKnowledgeLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let resource_arm = matches!(dse_id.0, HUNT | FORAGE);
        let threat_arm = dse_id.0 == PATROL;
        if !resource_arm && !threat_arm {
            return score;
        }
        let key = if resource_arm {
            COLONY_KNOWLEDGE_RESOURCE_PROXIMITY
        } else {
            COLONY_KNOWLEDGE_THREAT_PROXIMITY
        };
        let sum = fetch(key, ctx.cat);
        if sum <= 0.0 {
            return score;
        }
        score + sum * self.scale
    }

    fn name(&self) -> &'static str {
        "colony_knowledge_lift"
    }
}

// ---------------------------------------------------------------------------
// ColonyPriorityLift
// ---------------------------------------------------------------------------

/// §3.5.1 additive lift on the DSE family aligned with the active
/// `ColonyPriority`. Reads `colony_priority_ordinal` (-1 = none) and
/// adds `priority_bonus` to:
/// - Food: hunt, forage, farm
/// - Defense: patrol, fight
/// - Building: build
/// - Exploration: explore
pub struct ColonyPriorityLift {
    bonus: f32,
}

impl ColonyPriorityLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.priority_bonus,
        }
    }
}

impl ScoreModifier for ColonyPriorityLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let ordinal = fetch(COLONY_PRIORITY_ORDINAL, ctx.cat);
        let matches_priority = if (ordinal - COLONY_PRIORITY_FOOD).abs() < 0.5 {
            matches!(dse_id.0, HUNT | FORAGE | FARM)
        } else if (ordinal - COLONY_PRIORITY_DEFENSE).abs() < 0.5 {
            matches!(dse_id.0, PATROL | FIGHT)
        } else if (ordinal - COLONY_PRIORITY_BUILDING).abs() < 0.5 {
            dse_id.0 == BUILD
        } else if (ordinal - COLONY_PRIORITY_EXPLORATION).abs() < 0.5 {
            dse_id.0 == EXPLORE
        } else {
            false
        };
        if !matches_priority {
            return score;
        }
        score + self.bonus
    }

    fn name(&self) -> &'static str {
        "colony_priority_lift"
    }
}

// ---------------------------------------------------------------------------
// NeighborActionCascade
// ---------------------------------------------------------------------------

/// §3.5.1 additive lift driven by what nearby cats are currently doing.
/// Reads `cascade_count_<action>` (count of cats within
/// `cascading_bonus_range` performing that action) and adds
/// `count × cascading_bonus_per_cat`.
///
/// Routes each DSE id to the cascade key for its parent Action,
/// collapsing per-DSE-within-Action sibling DSEs to one shared count
/// (groom_self / groom_other → cascade_count_groom; herbcraft sub-modes
/// → cascade_count_herbcraft; magic sub-modes → cascade_count_practicemagic).
///
/// **Fight is excluded** — Fight has its own `fight_ally_bonus_per_cat`
/// inline; cascading on top would create a positive feedback loop.
pub struct NeighborActionCascade {
    bonus_per_cat: f32,
}

impl NeighborActionCascade {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus_per_cat: sc.cascading_bonus_per_cat,
        }
    }

    fn cascade_key(dse_id: DseId) -> Option<&'static str> {
        match dse_id.0 {
            HUNT => Some(CASCADE_COUNT_HUNT),
            FORAGE => Some(CASCADE_COUNT_FORAGE),
            EAT => Some(CASCADE_COUNT_EAT),
            SLEEP => Some(CASCADE_COUNT_SLEEP),
            WANDER => Some(CASCADE_COUNT_WANDER),
            IDLE => Some(CASCADE_COUNT_IDLE),
            SOCIALIZE => Some(CASCADE_COUNT_SOCIALIZE),
            GROOM_SELF | GROOM_OTHER => Some(CASCADE_COUNT_GROOM),
            EXPLORE => Some(CASCADE_COUNT_EXPLORE),
            FLEE => Some(CASCADE_COUNT_FLEE),
            PATROL => Some(CASCADE_COUNT_PATROL),
            BUILD => Some(CASCADE_COUNT_BUILD),
            FARM => Some(CASCADE_COUNT_FARM),
            HERBCRAFT_GATHER | HERBCRAFT_PREPARE | HERBCRAFT_WARD => {
                Some(CASCADE_COUNT_HERBCRAFT)
            }
            MAGIC_SCRY | MAGIC_DURABLE_WARD | MAGIC_CLEANSE | MAGIC_COLONY_CLEANSE
            | MAGIC_HARVEST | MAGIC_COMMUNE => Some(CASCADE_COUNT_PRACTICEMAGIC),
            COORDINATE => Some(CASCADE_COUNT_COORDINATE),
            MENTOR => Some(CASCADE_COUNT_MENTOR),
            MATE => Some(CASCADE_COUNT_MATE),
            CARETAKE => Some(CASCADE_COUNT_CARETAKE),
            COOK => Some(CASCADE_COUNT_COOK),
            HIDE => Some(CASCADE_COUNT_HIDE),
            // FIGHT excluded by design.
            _ => None,
        }
    }
}

impl ScoreModifier for NeighborActionCascade {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let Some(key) = Self::cascade_key(dse_id) else {
            return score;
        };
        let count = fetch(key, ctx.cat);
        if count <= 0.0 {
            return score;
        }
        score + count * self.bonus_per_cat
    }

    fn name(&self) -> &'static str {
        "neighbor_action_cascade"
    }
}

// ---------------------------------------------------------------------------
// AspirationLift
// ---------------------------------------------------------------------------

/// §3.5.1 additive lift: `score += count × aspiration_bonus`, where
/// `count` is the number of active aspirations whose domain includes
/// the DSE's parent action. Reads `aspiration_action_<action>` keyed
/// by the DSE's parent Action via `cascade_key` (collapse rules
/// shared with `NeighborActionCascade`).
pub struct AspirationLift {
    bonus: f32,
}

impl AspirationLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.aspiration_bonus,
        }
    }

    fn aspiration_key(dse_id: DseId) -> Option<&'static str> {
        match dse_id.0 {
            HUNT => Some("aspiration_action_hunt"),
            FORAGE => Some("aspiration_action_forage"),
            EAT => Some("aspiration_action_eat"),
            SLEEP => Some("aspiration_action_sleep"),
            WANDER => Some("aspiration_action_wander"),
            IDLE => Some("aspiration_action_idle"),
            SOCIALIZE => Some("aspiration_action_socialize"),
            GROOM_SELF | GROOM_OTHER => Some("aspiration_action_groom"),
            EXPLORE => Some("aspiration_action_explore"),
            FLEE => Some("aspiration_action_flee"),
            FIGHT => Some("aspiration_action_fight"),
            PATROL => Some("aspiration_action_patrol"),
            BUILD => Some("aspiration_action_build"),
            FARM => Some("aspiration_action_farm"),
            HERBCRAFT_GATHER | HERBCRAFT_PREPARE | HERBCRAFT_WARD => {
                Some("aspiration_action_herbcraft")
            }
            MAGIC_SCRY | MAGIC_DURABLE_WARD | MAGIC_CLEANSE | MAGIC_COLONY_CLEANSE
            | MAGIC_HARVEST | MAGIC_COMMUNE => Some("aspiration_action_practicemagic"),
            COORDINATE => Some("aspiration_action_coordinate"),
            MENTOR => Some("aspiration_action_mentor"),
            MATE => Some("aspiration_action_mate"),
            CARETAKE => Some("aspiration_action_caretake"),
            COOK => Some("aspiration_action_cook"),
            HIDE => Some("aspiration_action_hide"),
            _ => None,
        }
    }
}

impl ScoreModifier for AspirationLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let Some(key) = Self::aspiration_key(dse_id) else {
            return score;
        };
        let count = fetch(key, ctx.cat);
        if count <= 0.0 {
            return score;
        }
        score + count * self.bonus
    }

    fn name(&self) -> &'static str {
        "aspiration_lift"
    }
}

// ---------------------------------------------------------------------------
// PreferenceLift / PreferencePenalty
// ---------------------------------------------------------------------------

fn preference_key(dse_id: DseId) -> Option<&'static str> {
    match dse_id.0 {
        HUNT => Some("preference_for_hunt"),
        FORAGE => Some("preference_for_forage"),
        EAT => Some("preference_for_eat"),
        SLEEP => Some("preference_for_sleep"),
        WANDER => Some("preference_for_wander"),
        IDLE => Some("preference_for_idle"),
        SOCIALIZE => Some("preference_for_socialize"),
        GROOM_SELF | GROOM_OTHER => Some("preference_for_groom"),
        EXPLORE => Some("preference_for_explore"),
        FLEE => Some("preference_for_flee"),
        FIGHT => Some("preference_for_fight"),
        PATROL => Some("preference_for_patrol"),
        BUILD => Some("preference_for_build"),
        FARM => Some("preference_for_farm"),
        HERBCRAFT_GATHER | HERBCRAFT_PREPARE | HERBCRAFT_WARD => Some("preference_for_herbcraft"),
        MAGIC_SCRY | MAGIC_DURABLE_WARD | MAGIC_CLEANSE | MAGIC_COLONY_CLEANSE | MAGIC_HARVEST
        | MAGIC_COMMUNE => Some("preference_for_practicemagic"),
        COORDINATE => Some("preference_for_coordinate"),
        MENTOR => Some("preference_for_mentor"),
        MATE => Some("preference_for_mate"),
        CARETAKE => Some("preference_for_caretake"),
        COOK => Some("preference_for_cook"),
        HIDE => Some("preference_for_hide"),
        _ => None,
    }
}

/// §3.5.1 additive lift on actions the cat Likes:
/// `score += preference_like_bonus` when the DSE's parent action has
/// preference signal `+1.0`.
pub struct PreferenceLift {
    bonus: f32,
}

impl PreferenceLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.preference_like_bonus,
        }
    }
}

impl ScoreModifier for PreferenceLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let Some(key) = preference_key(dse_id) else {
            return score;
        };
        let signal = fetch(key, ctx.cat);
        if signal <= 0.0 {
            return score;
        }
        score + self.bonus
    }

    fn name(&self) -> &'static str {
        "preference_lift"
    }
}

/// §3.5.1 subtractive lift on actions the cat Dislikes:
/// `score -= preference_dislike_penalty` when the DSE's parent action
/// has preference signal `-1.0`.
pub struct PreferencePenalty {
    penalty: f32,
}

impl PreferencePenalty {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            penalty: sc.preference_dislike_penalty,
        }
    }
}

impl ScoreModifier for PreferencePenalty {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let Some(key) = preference_key(dse_id) else {
            return score;
        };
        let signal = fetch(key, ctx.cat);
        if signal >= 0.0 {
            return score;
        }
        score - self.penalty
    }

    fn name(&self) -> &'static str {
        "preference_penalty"
    }
}

// ---------------------------------------------------------------------------
// FatedLoveLift / FatedRivalLift
// ---------------------------------------------------------------------------

const FATED_LOVE_VISIBLE: &str = "fated_love_visible";
const FATED_RIVAL_NEARBY: &str = "fated_rival_nearby";

/// §3.5.1 additive lift on socialize / groom_other / mate when the
/// cat's `FatedLove` partner is awakened and visible (`fated_love_visible
/// == 1.0`). `score += fated_love_social_bonus`.
///
/// Per-DSE specificity: targets `groom_other` only (not `groom_self`),
/// ticking up the social-direction interpretation of the legacy
/// per-Action `Action::Groom` bonus. When `groom_self` outscores
/// `groom_other` pre-bonus, the post-bonus winner is whichever
/// `score_actions`'s `max(self, other)` resolves — same selection
/// shape, but the signal is now per-DSE-honest.
pub struct FatedLoveLift {
    bonus: f32,
}

impl FatedLoveLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.fated_love_social_bonus,
        }
    }
}

impl ScoreModifier for FatedLoveLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, SOCIALIZE | GROOM_OTHER | MATE) {
            return score;
        }
        let visible = fetch(FATED_LOVE_VISIBLE, ctx.cat);
        if visible <= 0.0 {
            return score;
        }
        score + self.bonus
    }

    fn name(&self) -> &'static str {
        "fated_love_lift"
    }
}

/// §3.5.1 additive lift on hunt / patrol / fight / explore when the
/// cat's `FatedRival` is awakened and nearby (`fated_rival_nearby
/// == 1.0`). `score += fated_rival_competition_bonus`.
pub struct FatedRivalLift {
    bonus: f32,
}

impl FatedRivalLift {
    pub fn new(sc: &ScoringConstants) -> Self {
        Self {
            bonus: sc.fated_rival_competition_bonus,
        }
    }
}

impl ScoreModifier for FatedRivalLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        if !matches!(dse_id.0, HUNT | PATROL | FIGHT | EXPLORE) {
            return score;
        }
        let nearby = fetch(FATED_RIVAL_NEARBY, ctx.cat);
        if nearby <= 0.0 {
            return score;
        }
        score + self.bonus
    }

    fn name(&self) -> &'static str {
        "fated_rival_lift"
    }
}

// ---------------------------------------------------------------------------
// ActiveDirectiveLift
// ---------------------------------------------------------------------------

const ACTIVE_DIRECTIVE_ACTION_ORDINAL: &str = "active_directive_action_ordinal";
const ACTIVE_DIRECTIVE_BONUS: &str = "active_directive_bonus";

/// §3.5.1 additive lift on the single Action a coordinator's directive
/// targets. The bonus magnitude is pre-computed at ScoringContext
/// build time (priority × coordinator_social_weight × base_weight ×
/// diligence × fondness × (1 - independence_penalty) × (1 -
/// stubbornness_penalty)) — identical to the legacy `apply_directive_bonus`
/// arithmetic.
///
/// Reads `active_directive_action_ordinal` (`-1` = no directive,
/// otherwise `Action as usize`) and `active_directive_bonus`. The
/// modifier resolves each DSE id to its parent Action via
/// `parent_action_ordinal`, applies the bonus when the ordinal matches.
pub struct ActiveDirectiveLift;

impl ActiveDirectiveLift {
    pub fn new() -> Self {
        Self
    }

    fn parent_action_ordinal(dse_id: DseId) -> Option<i32> {
        use crate::ai::Action;
        let action = match dse_id.0 {
            HUNT => Action::Hunt,
            FORAGE => Action::Forage,
            EAT => Action::Eat,
            SLEEP => Action::Sleep,
            WANDER => Action::Wander,
            IDLE => Action::Idle,
            SOCIALIZE => Action::Socialize,
            GROOM_SELF | GROOM_OTHER => Action::Groom,
            EXPLORE => Action::Explore,
            FLEE => Action::Flee,
            FIGHT => Action::Fight,
            PATROL => Action::Patrol,
            BUILD => Action::Build,
            FARM => Action::Farm,
            HERBCRAFT_GATHER | HERBCRAFT_PREPARE | HERBCRAFT_WARD => Action::Herbcraft,
            MAGIC_SCRY | MAGIC_DURABLE_WARD | MAGIC_CLEANSE | MAGIC_COLONY_CLEANSE
            | MAGIC_HARVEST | MAGIC_COMMUNE => Action::PracticeMagic,
            COORDINATE => Action::Coordinate,
            MENTOR => Action::Mentor,
            MATE => Action::Mate,
            CARETAKE => Action::Caretake,
            COOK => Action::Cook,
            HIDE => Action::Hide,
            _ => return None,
        };
        Some(action as i32)
    }
}

impl Default for ActiveDirectiveLift {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoreModifier for ActiveDirectiveLift {
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        let target_ordinal = fetch(ACTIVE_DIRECTIVE_ACTION_ORDINAL, ctx.cat);
        if target_ordinal < 0.0 {
            return score;
        }
        let Some(dse_ordinal) = Self::parent_action_ordinal(dse_id) else {
            return score;
        };
        if (target_ordinal - dse_ordinal as f32).abs() > 0.5 {
            return score;
        }
        let bonus = fetch(ACTIVE_DIRECTIVE_BONUS, ctx.cat);
        if bonus <= 0.0 {
            return score;
        }
        score + bonus
    }

    fn name(&self) -> &'static str {
        "active_directive_lift"
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
    // DispositionFailureCooldown is multiplicative; registers first so
    // the additive bonuses below compose on already-damped scores.
    pipeline.push(Box::new(DispositionFailureCooldown::new()));
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
    // Memory family — additive lift on Hunt/Forage near remembered
    // resource finds; subtractive on Wander/Idle near remembered Death;
    // subtractive on Wander/Explore/Hunt near remembered ThreatSeen.
    // Each reads a pre-aggregated proximity sum from ctx_scalars.
    pipeline.push(Box::new(MemoryResourceFoundLift::new(sc)));
    pipeline.push(Box::new(MemoryDeathPenalty::new(sc)));
    pipeline.push(Box::new(MemoryThreatSeenSuppress::new(sc)));
    pipeline.push(Box::new(ColonyKnowledgeLift::new(sc)));
    pipeline.push(Box::new(ColonyPriorityLift::new(sc)));
    pipeline.push(Box::new(NeighborActionCascade::new(sc)));
    pipeline.push(Box::new(AspirationLift::new(sc)));
    pipeline.push(Box::new(PreferenceLift::new(sc)));
    pipeline.push(Box::new(PreferencePenalty::new(sc)));
    pipeline.push(Box::new(FatedLoveLift::new(sc)));
    pipeline.push(Box::new(FatedRivalLift::new(sc)));
    pipeline.push(Box::new(ActiveDirectiveLift::new()));
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
    // Ticket 156 — `KittenEatBoost` multiplies the kitten cohort's
    // Eat score after `HungerUrgency` so the kitten boost amplifies
    // the post-urgency lifted score rather than the bare DSE score.
    // Behavior-neutral for non-kitten cats (Kitten-marker gate).
    pipeline.push(Box::new(KittenEatBoost::new(sc)));
    // Ticket 156 — `KittenCryCaretakeLift` adds an additive lift on
    // non-kitten cats' Caretake DSE when the cat is hearing a kitten
    // distress cry painted by `update_kitten_cry_map`. Composes
    // additively with the legacy three-axis Caretake score, avoiding
    // the Phase-4 weight-rebalance regression (verified empirically
    // — see KittenCryCaretakeLift doc-comment).
    pipeline.push(Box::new(KittenCryCaretakeLift::new(sc)));
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
    fn default_pipeline_registers_expected_modifier_count() {
        let constants = crate::resources::sim_constants::SimConstants::default();
        let pipeline = default_modifier_pipeline(&constants);
        assert_eq!(pipeline.len(), 33, "expected 33 registered modifiers");
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
            curve_exponent: 1.0,
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

    // -----------------------------------------------------------------------
    // ticket 156 KittenCryCaretakeLift
    // -----------------------------------------------------------------------

    fn test_kitten_cry_caretake_lift() -> KittenCryCaretakeLift {
        KittenCryCaretakeLift {
            threshold: 0.05,
            lift: 0.5,
        }
    }

    #[test]
    fn kitten_cry_caretake_lift_no_lift_for_kitten() {
        // Kittens shouldn't be lifted on Caretake — they're the
        // recipients, not providers. The Kitten-marker gate suppresses.
        let modifier = test_kitten_cry_caretake_lift();
        let (_, ctx) = test_ctx_with_kitten_marker();
        let fetch = |name: &str, _: Entity| match name {
            KITTEN_CRY_PERCEIVED => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(CARETAKE), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.5).abs() < 1e-6,
            "kitten Caretake unchanged regardless of cry; got {out}"
        );
    }

    #[test]
    fn kitten_cry_caretake_lift_no_lift_below_threshold() {
        // Adult cat hearing a tiny / sub-threshold cry: no lift.
        let modifier = test_kitten_cry_caretake_lift();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            KITTEN_CRY_PERCEIVED => 0.04,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(CARETAKE), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.5).abs() < 1e-6,
            "below-threshold Caretake unchanged; got {out}"
        );
    }

    #[test]
    fn kitten_cry_caretake_lift_lifts_above_threshold() {
        // Adult perceiving full-volume cry: full lift = 0.5.
        // ramp = (1.0 - 0.05) / (1.0 - 0.05) = 1.0; lift = 0.5.
        let modifier = test_kitten_cry_caretake_lift();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            KITTEN_CRY_PERCEIVED => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(CARETAKE), 0.5, &ctx, &fetch);
        assert!(
            (out - 1.0).abs() < 1e-5,
            "full-cry Caretake lift; got {out}"
        );
    }

    #[test]
    fn kitten_cry_caretake_lift_targets_only_caretake() {
        // Non-Caretake DSEs pass through unchanged even with full cry.
        let modifier = test_kitten_cry_caretake_lift();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            KITTEN_CRY_PERCEIVED => 1.0,
            _ => 0.0,
        };
        for dse in [
            EAT, HUNT, FORAGE, SLEEP, GROOM_SELF, GROOM_OTHER, FLEE, FIGHT, MATE, COORDINATE,
            BUILD, MENTOR, SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Caretake dse {dse} unchanged at full cry; got {out}"
            );
        }
    }

    #[test]
    fn kitten_cry_caretake_lift_does_not_resurrect_zero_score() {
        // Gated-boost contract — perceived cry doesn't conjure
        // caretake-eligibility into existence.
        let modifier = test_kitten_cry_caretake_lift();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            KITTEN_CRY_PERCEIVED => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(CARETAKE), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero-score Caretake stays zero — no resurrection");
    }

    // -----------------------------------------------------------------------
    // ticket 156 KittenEatBoost
    // -----------------------------------------------------------------------

    fn test_kitten_eat_boost() -> KittenEatBoost {
        KittenEatBoost {
            threshold: 0.4,
            multiplier: 4.0,
        }
    }

    fn test_ctx_with_kitten_marker() -> (Entity, EvalCtx<'static>) {
        static KITTEN_MARKER: fn(&str, Entity) -> bool = |name, _| {
            name == crate::components::markers::Kitten::KEY
        };
        static NO_ENTITY_POS: fn(Entity) -> Option<Position> = |_| None;
        static NO_ANCHOR_POS: fn(LandmarkAnchor) -> Option<Position> = |_| None;
        let entity = Entity::from_raw_u32(1).unwrap();
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &NO_ENTITY_POS,
            anchor_position: &NO_ANCHOR_POS,
            has_marker: &KITTEN_MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        (entity, ctx)
    }

    #[test]
    fn kitten_eat_boost_no_lift_for_non_kitten() {
        // Adult cat (no Kitten marker): Eat passes through unchanged
        // even at full hunger urgency.
        let modifier = test_kitten_eat_boost();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(EAT), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.5).abs() < 1e-6,
            "non-kitten Eat unchanged; got {out}"
        );
    }

    #[test]
    fn kitten_eat_boost_no_lift_below_threshold() {
        // Kitten with hunger urgency below threshold: no boost.
        let modifier = test_kitten_eat_boost();
        let (_, ctx) = test_ctx_with_kitten_marker();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 0.3,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(EAT), 0.5, &ctx, &fetch);
        assert!(
            (out - 0.5).abs() < 1e-6,
            "below-threshold kitten Eat unchanged; got {out}"
        );
    }

    #[test]
    fn kitten_eat_boost_lifts_above_threshold() {
        // Kitten at urgency=0.8 (hunger=0.2). ramp = (0.8 - 0.4) / 0.6 = 0.667.
        // multiplier = 1.0 + 0.667 * (4.0 - 1.0) = 3.0. Eat 0.39 → 1.17.
        // The kitten Eat now beats the empirical Groom 1.13 baseline.
        let modifier = test_kitten_eat_boost();
        let (_, ctx) = test_ctx_with_kitten_marker();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 0.8,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(EAT), 0.39, &ctx, &fetch);
        // 0.39 * 3.0 = 1.17
        assert!(
            (out - 1.17).abs() < 1e-3,
            "kitten Eat boosted past Groom baseline; got {out}"
        );
        assert!(
            out > 1.13,
            "kitten Eat must exceed empirical Groom 1.13 baseline; got {out}"
        );
    }

    #[test]
    fn kitten_eat_boost_max_lift_at_full_urgency() {
        // Kitten at urgency=1.0: full multiplier 4.0× applied.
        let modifier = test_kitten_eat_boost();
        let (_, ctx) = test_ctx_with_kitten_marker();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(EAT), 0.5, &ctx, &fetch);
        assert!(
            (out - 2.0).abs() < 1e-5,
            "full-urgency kitten Eat = score × 4.0; got {out}"
        );
    }

    #[test]
    fn kitten_eat_boost_targets_only_eat() {
        // Even with Kitten marker + full urgency, only the Eat DSE is
        // boosted; every other DSE id passes through unchanged.
        let modifier = test_kitten_eat_boost();
        let (_, ctx) = test_ctx_with_kitten_marker();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        for dse in [
            HUNT, FORAGE, SLEEP, GROOM_SELF, GROOM_OTHER, FLEE, FIGHT, MATE, COORDINATE, BUILD,
            MENTOR, CARETAKE, SOCIALIZE, PATROL, COOK, FARM, WANDER, EXPLORE, IDLE,
        ] {
            let out = modifier.apply(DseId(dse), 0.5, &ctx, &fetch);
            assert!(
                (out - 0.5).abs() < 1e-6,
                "non-Eat dse {dse} unchanged for kitten; got {out}"
            );
        }
    }

    #[test]
    fn kitten_eat_boost_does_not_resurrect_zero_score() {
        // Gated-boost contract — multiplying by zero stays zero.
        let modifier = test_kitten_eat_boost();
        let (_, ctx) = test_ctx_with_kitten_marker();
        let fetch = |name: &str, _: Entity| match name {
            HUNGER_URGENCY => 1.0,
            _ => 0.0,
        };
        let out = modifier.apply(DseId(EAT), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0, "zero-score Eat stays zero — no resurrection");
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

    // -----------------------------------------------------------------------
    // DispositionFailureCooldown
    // -----------------------------------------------------------------------

    #[test]
    fn cooldown_passes_through_when_signal_full() {
        let m = DispositionFailureCooldown::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            DISPOSITION_FAILURE_SIGNAL_HUNTING => 1.0,
            _ => 1.0,
        };
        let out = m.apply(DseId(HUNT), 0.7, &ctx, &fetch);
        assert!((out - 0.7).abs() < 1e-6);
    }

    #[test]
    fn cooldown_full_damp_at_fresh_failure() {
        let m = DispositionFailureCooldown::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            DISPOSITION_FAILURE_SIGNAL_HUNTING => 0.0,
            _ => 1.0,
        };
        // signal 0 → damp 0.1 → 0.7 × 0.1 = 0.07
        let out = m.apply(DseId(HUNT), 0.7, &ctx, &fetch);
        assert!((out - 0.07).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn cooldown_midpoint_damps_to_55_percent() {
        let m = DispositionFailureCooldown::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            DISPOSITION_FAILURE_SIGNAL_CRAFTING => 0.5,
            _ => 1.0,
        };
        // signal 0.5 → damp 0.1 + 0.45 = 0.55 → 1.0 × 0.55 = 0.55
        let out = m.apply(DseId(COOK), 1.0, &ctx, &fetch);
        assert!((out - 0.55).abs() < 1e-6, "got {out}");
    }

    #[test]
    fn cooldown_routes_each_dse_to_its_disposition_signal() {
        let m = DispositionFailureCooldown::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            DISPOSITION_FAILURE_SIGNAL_HUNTING => 0.0,
            DISPOSITION_FAILURE_SIGNAL_FORAGING => 0.0,
            DISPOSITION_FAILURE_SIGNAL_CRAFTING => 0.0,
            DISPOSITION_FAILURE_SIGNAL_CARETAKING => 0.0,
            DISPOSITION_FAILURE_SIGNAL_BUILDING => 0.0,
            DISPOSITION_FAILURE_SIGNAL_MATING => 0.0,
            DISPOSITION_FAILURE_SIGNAL_MENTORING => 0.0,
            _ => 1.0,
        };
        for dse in [
            HUNT,
            FORAGE,
            COOK,
            HERBCRAFT_GATHER,
            HERBCRAFT_PREPARE,
            HERBCRAFT_WARD,
            MAGIC_SCRY,
            MAGIC_DURABLE_WARD,
            MAGIC_CLEANSE,
            MAGIC_COLONY_CLEANSE,
            MAGIC_HARVEST,
            MAGIC_COMMUNE,
            CARETAKE,
            BUILD,
            MATE,
            MENTOR,
        ] {
            let out = m.apply(DseId(dse), 1.0, &ctx, &fetch);
            assert!((out - 0.1).abs() < 1e-6, "dse {dse} got {out}");
        }
    }

    #[test]
    fn cooldown_skips_exempt_dispositions() {
        let m = DispositionFailureCooldown::new();
        let (_, ctx) = test_ctx();
        // Even with every signal pinned at full damp, exempt DSEs pass
        // through unchanged because `signal_key` returns None for them.
        let fetch = |_: &str, _: Entity| 0.0;
        for dse in [SLEEP, EAT, PATROL, FIGHT, COORDINATE, EXPLORE, WANDER, FARM, SOCIALIZE, GROOM_SELF, GROOM_OTHER, FLEE, IDLE, HIDE] {
            let out = m.apply(DseId(dse), 0.6, &ctx, &fetch);
            assert!((out - 0.6).abs() < 1e-6, "dse {dse} got {out}");
        }
    }

    #[test]
    fn cooldown_zero_score_stays_zero() {
        let m = DispositionFailureCooldown::new();
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            DISPOSITION_FAILURE_SIGNAL_HUNTING => 0.0,
            _ => 1.0,
        };
        let out = m.apply(DseId(HUNT), 0.0, &ctx, &fetch);
        assert_eq!(out, 0.0);
    }

    // -----------------------------------------------------------------------
    // Memory family
    // -----------------------------------------------------------------------

    #[test]
    fn memory_resource_found_lift_boosts_hunt_and_forage() {
        let m = MemoryResourceFoundLift { bonus: 0.2 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            MEMORY_RESOURCE_FOUND_PROXIMITY_SUM => 1.0,
            _ => 0.0,
        };
        // 0.5 + 1.0 × 0.2 = 0.7
        assert!((m.apply(DseId(HUNT), 0.5, &ctx, &fetch) - 0.7).abs() < 1e-6);
        assert!((m.apply(DseId(FORAGE), 0.5, &ctx, &fetch) - 0.7).abs() < 1e-6);
        // Other DSEs are unchanged.
        assert!((m.apply(DseId(EAT), 0.5, &ctx, &fetch) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn memory_resource_found_lift_inert_when_sum_zero() {
        let m = MemoryResourceFoundLift { bonus: 0.2 };
        let (_, ctx) = test_ctx();
        let fetch = |_: &str, _: Entity| 0.0;
        assert!((m.apply(DseId(HUNT), 0.5, &ctx, &fetch) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn memory_death_penalty_subtracts_from_wander_and_idle() {
        let m = MemoryDeathPenalty { penalty: 0.1 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            MEMORY_DEATH_PROXIMITY_SUM => 1.0,
            _ => 0.0,
        };
        // 0.6 - 1.0 × 0.1 = 0.5
        assert!((m.apply(DseId(WANDER), 0.6, &ctx, &fetch) - 0.5).abs() < 1e-6);
        assert!((m.apply(DseId(IDLE), 0.6, &ctx, &fetch) - 0.5).abs() < 1e-6);
        assert!((m.apply(DseId(HUNT), 0.6, &ctx, &fetch) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn memory_threat_seen_suppress_subtracts_from_wander_explore_hunt() {
        let m = MemoryThreatSeenSuppress { penalty: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            MEMORY_THREAT_SEEN_PROXIMITY_SUM => 1.0,
            _ => 0.0,
        };
        // 0.6 - 1.0 × 0.15 = 0.45
        assert!((m.apply(DseId(WANDER), 0.6, &ctx, &fetch) - 0.45).abs() < 1e-6);
        assert!((m.apply(DseId(EXPLORE), 0.6, &ctx, &fetch) - 0.45).abs() < 1e-6);
        assert!((m.apply(DseId(HUNT), 0.6, &ctx, &fetch) - 0.45).abs() < 1e-6);
        assert!((m.apply(DseId(FORAGE), 0.6, &ctx, &fetch) - 0.6).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // ColonyKnowledgeLift
    // -----------------------------------------------------------------------

    #[test]
    fn colony_knowledge_resource_arm_lifts_hunt_and_forage() {
        let m = ColonyKnowledgeLift { scale: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            COLONY_KNOWLEDGE_RESOURCE_PROXIMITY => 1.0,
            _ => 0.0,
        };
        // 0.5 + 1.0 × 0.15 = 0.65
        assert!((m.apply(DseId(HUNT), 0.5, &ctx, &fetch) - 0.65).abs() < 1e-6);
        assert!((m.apply(DseId(FORAGE), 0.5, &ctx, &fetch) - 0.65).abs() < 1e-6);
    }

    #[test]
    fn colony_knowledge_threat_arm_lifts_patrol_only() {
        let m = ColonyKnowledgeLift { scale: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            COLONY_KNOWLEDGE_THREAT_PROXIMITY => 1.0,
            _ => 0.0,
        };
        assert!((m.apply(DseId(PATROL), 0.5, &ctx, &fetch) - 0.65).abs() < 1e-6);
        // Hunt is in the resource arm; with threat-only sum it's inert.
        assert!((m.apply(DseId(HUNT), 0.5, &ctx, &fetch) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn colony_knowledge_skips_unrelated_dses() {
        let m = ColonyKnowledgeLift { scale: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |_: &str, _: Entity| 1.0;
        for dse in [EAT, SLEEP, BUILD, MATE, MENTOR, COORDINATE, IDLE] {
            assert!((m.apply(DseId(dse), 0.5, &ctx, &fetch) - 0.5).abs() < 1e-6);
        }
    }

    // -----------------------------------------------------------------------
    // ColonyPriorityLift
    // -----------------------------------------------------------------------

    #[test]
    fn colony_priority_food_lifts_hunt_forage_farm() {
        let m = ColonyPriorityLift { bonus: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            COLONY_PRIORITY_ORDINAL => 0.0, // Food
            _ => 0.0,
        };
        for dse in [HUNT, FORAGE, FARM] {
            assert!((m.apply(DseId(dse), 0.4, &ctx, &fetch) - 0.55).abs() < 1e-6);
        }
        // Patrol/Fight aren't in Food's list.
        assert!((m.apply(DseId(PATROL), 0.4, &ctx, &fetch) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn colony_priority_defense_lifts_patrol_fight() {
        let m = ColonyPriorityLift { bonus: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            COLONY_PRIORITY_ORDINAL => 1.0, // Defense
            _ => 0.0,
        };
        for dse in [PATROL, FIGHT] {
            assert!((m.apply(DseId(dse), 0.4, &ctx, &fetch) - 0.55).abs() < 1e-6);
        }
        assert!((m.apply(DseId(HUNT), 0.4, &ctx, &fetch) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn colony_priority_none_inert() {
        let m = ColonyPriorityLift { bonus: 0.15 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            COLONY_PRIORITY_ORDINAL => -1.0,
            _ => 0.0,
        };
        for dse in [HUNT, FORAGE, FARM, PATROL, FIGHT, BUILD, EXPLORE] {
            assert!((m.apply(DseId(dse), 0.4, &ctx, &fetch) - 0.4).abs() < 1e-6);
        }
    }

    // -----------------------------------------------------------------------
    // NeighborActionCascade
    // -----------------------------------------------------------------------

    #[test]
    fn cascade_lifts_hunt_proportional_to_count() {
        let m = NeighborActionCascade { bonus_per_cat: 0.08 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            CASCADE_COUNT_HUNT => 3.0,
            _ => 0.0,
        };
        // 0.5 + 3 × 0.08 = 0.74
        assert!((m.apply(DseId(HUNT), 0.5, &ctx, &fetch) - 0.74).abs() < 1e-6);
    }

    #[test]
    fn cascade_collapses_groom_siblings_to_shared_count() {
        let m = NeighborActionCascade { bonus_per_cat: 0.08 };
        let (_, ctx) = test_ctx();
        let fetch = |name: &str, _: Entity| match name {
            CASCADE_COUNT_GROOM => 2.0,
            _ => 0.0,
        };
        // Both groom DSEs read the same cascade key.
        assert!((m.apply(DseId(GROOM_SELF), 0.5, &ctx, &fetch) - 0.66).abs() < 1e-6);
        assert!((m.apply(DseId(GROOM_OTHER), 0.5, &ctx, &fetch) - 0.66).abs() < 1e-6);
    }

    #[test]
    fn cascade_carves_out_fight() {
        let m = NeighborActionCascade { bonus_per_cat: 0.08 };
        let (_, ctx) = test_ctx();
        let fetch = |_: &str, _: Entity| 5.0;
        assert!((m.apply(DseId(FIGHT), 0.5, &ctx, &fetch) - 0.5).abs() < 1e-6);
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
