//! L2 JointIntention — ticket 127 mutually-public substrate for two-cat
//! practices. Codified body language; mismatched stages are codified irony.
//!
//! # Semantic category
//!
//! Three substrate categories on a cat:
//!
//! | Category | Example | Read by | Authored by |
//! |---|---|---|---|
//! | **Actor-private commitment** | `HeldIntention` (126) | actor's own scoring + drop | L2 evaluator on softmax winner |
//! | **Public physical state** | `Dead`, `Injured`, `Pregnant`, `HeadDownCurled` (242) | anyone via markers / `MarkerSnapshot` | per-system step resolvers |
//! | **Publicly-performed practice state** | `JointIntention` *(this module)* | both partners + cascading drop-gate | practice author system (matchmaker) |
//!
//! `JointIntention` is **mutually-public substrate** — not an exception to
//! 126's actor-private rule, a distinct semantic category. Real cats perceive
//! each other's persistent practice-engagement through scent, posture,
//! mounting tolerance, repeated proximity. We don't have those perception
//! channels yet, so the substrate stands in. Reading the partner's
//! `JointIntention` is reading publicly-performed practice state, not the
//! partner's internal commitment.
//!
//! # Field discipline
//!
//! `JointIntention` carries *only observables* — fields that map to long-term
//! body language a real cat could perceive: practice, partner, role, stage,
//! tick markers. Internal heart-state (`commitment_strength`, `expiry`,
//! `source`) stays in the actor's own `HeldIntention`. If a field can't be
//! expressed as observable practice-state, it does not belong here.
//!
//! # Codified irony
//!
//! When two paired cats hold mismatched stages — one believes they're in
//! `CourtshipCourting`; the other still holds `CourtshipApproach` — the gap
//! IS dramatic irony, mechanically codified. One cat is wooing while the
//! other is just being friendly; the audience and the diagnostic tooling
//! can see it; neither cat does. The field discipline IS the literary
//! device. Codified — meaning produced as a measurable side-effect of the
//! field discipline, not as authored content.
//!
//! Mismatch is observable via `Feature::JointStageMismatchTickAccrued` and
//! the focal-trace `joint_stage_mismatch_ticks_total` field. Stage
//! progression is **independent per cat**, evaluated against
//! self-observable proxies; partners may briefly mismatch and that's
//! narrative texture, not a bug to suppress.
//!
//! # Substrate vs search-state (§4.7)
//!
//! `JointIntention` is substrate: not mutated by A* (`StateEffect::Set*`
//! operates on `PlannerState`, never on cat Components), externally
//! authored by [`crate::ai::joint_intention::author_joint_intentions`].
//! Consumed by:
//!
//! - actor's own scoring pipeline (resolver bias readers read self-JI),
//! - actor's own drop-gate (reads self-JI proxies),
//! - **partner's drop-gate** for `PartnerLeftPractice` cascade detection —
//!   the explicit category-3 read; JointIntention IS publicly-performed
//!   practice state, so this is realistic observation rather than
//!   mind-reading.
//!
//! # Co-existence with `HeldIntention`
//!
//! A cat in a JointIntention also holds a `HeldIntention` reflecting the
//! practice's stage-N held action (e.g., for `CourtshipCourting`,
//! `held_action = Action::Pair`). When the joint stage advances, the cat's
//! `HeldIntention` is re-authored to match.
//! `HeldIntention.target == JointIntention.partner` for paired holds. The
//! `IntentionMomentum` modifier reads `HeldIntention`; the JointIntention
//! doesn't supply a separate momentum scalar — its job is the *structural*
//! commitment (practice membership), not the scoring lift.
//!
//! # Migration from `PairingActivity`
//!
//! `JointIntention { practice: Courtship }` subsumes `PairingActivity`
//! 1:1. The five `PairingDropBranch` variants become carry-over
//! `JointDropBranch` variants; four novel branches (`PartnerLeftPractice`,
//! `StageStalled`, `CompatibilityLost`, `Completed`) are added for
//! cross-practice generality. In ticket 127 Commit A, only the five
//! carry-over branches are wired from [`should_drop_joint`]; Commit B
//! activates the novel branches.

use bevy_ecs::prelude::*;

use crate::components::fertility::FertilityPhase;
use crate::components::identity::{Gender, LifeStage, Orientation};
use crate::resources::relationships::BondType;
use crate::resources::time::Season;

// ---------------------------------------------------------------------------
// Practice taxonomy
// ---------------------------------------------------------------------------

/// Which practice a JointIntention encodes. Drives matchmaker
/// compatibility, stage-advancement, drop branches, and per-practice
/// `Feature` parameterization.
///
/// `#[non_exhaustive]` so future practices (CoMentoring, JointCacheStocking,
/// PlayBout) can extend the enum without breaking archived trace
/// deserialization.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[non_exhaustive]
pub enum PracticeKind {
    /// Mating-pipeline practice. The first and only consumer in 127.
    /// Subsumes the entire `PairingActivity` substrate.
    Courtship,
    /// Ticket 276 — play-bout practice. Hosts the `play` continuity
    /// canary on JointIntention substrate (retires the four-AND × RNG
    /// direct-emit at `personality_events.rs:80-90`). Approach → Bouting
    /// → Cooldown stage progression; Cooldown→done fires
    /// `JointDropBranch::Completed` and emits
    /// `EventKind::JointPlayBoutCompleted` which feeds
    /// `continuity_tallies["play"]`.
    PlayBout,
}

impl PracticeKind {
    /// Stable slug for trace serialization, Feature display names, and
    /// `expected_to_fire_per_soak` canary registration.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Courtship => "courtship",
            Self::PlayBout => "play_bout",
        }
    }
}

/// Role this cat performs in the practice. Courtship uses `Mutual` to
/// preserve `PairingActivity`'s symmetric shape 1:1 — both partners hold
/// the same role and the matchmaker is symmetric. Future practices may
/// use asymmetric roles (Mentor/Apprentice, CacheHost/CacheStocker,
/// Initiator/Responder).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[non_exhaustive]
pub enum PracticeRole {
    /// Symmetric — both partners hold the same role.
    Mutual,
}

/// Current stage the cat believes the practice is in. **Stages may
/// briefly mismatch between partners** — see §Codified irony.
///
/// Stages are self-observable. A cat advances its own stage based on
/// proxies it can read about itself (own bond tier, own fertility phase,
/// own season). It cannot read the partner's internal stage; it can read
/// the partner's *substrate* (this Component is publicly-performed
/// practice state).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[non_exhaustive]
pub enum PracticeStage {
    /// L1 eligibility opened (Friends bond + reproductive-eligible).
    /// `PairingActivity`'s "matched, no bias yet" semantics map here.
    CourtshipApproach,
    /// Bias readers active; resolver picks multiplied;
    /// fondness/familiarity accruing. `PairingActivity`'s "Active" state
    /// maps here.
    CourtshipCourting,
    /// Partners-or-Mates bond + fertile window for the queen-side.
    /// MateWithGoal predominantly fires from this stage.
    CourtshipMating,
    /// Post-conception or post-Mates-bond settled state. Bias still
    /// active; mating-DSE eligibility paused. Today's "PairingActivity
    /// held during pregnancy" maps here.
    CourtshipBonded,
    /// Ticket 276 — PlayBout Approach. Both cats matched but no
    /// paired-resolver interaction tick has fired yet (mirrors
    /// `CourtshipApproach`). Advances to `PlayBoutBouting` on the first
    /// bias-amplified Socialize interaction.
    PlayBoutApproach,
    /// Ticket 276 — PlayBout Bouting. The cats are actively playing
    /// together; the Bouting-stage cascade (mood-lift to nearby
    /// witnesses, narrative entry) fires from this stage. Advances to
    /// `PlayBoutCooldown` after `bouting_duration_ticks` in stage.
    PlayBoutBouting,
    /// Ticket 276 — PlayBout Cooldown. Bout has wound down; the cats
    /// stay associated for `cooldown_duration_ticks` before
    /// `JointDropBranch::Completed` fires, emitting
    /// `EventKind::JointPlayBoutCompleted` and removing the JI.
    PlayBoutCooldown,
}

impl PracticeStage {
    /// Stable slug for trace serialization.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CourtshipApproach => "approach",
            Self::CourtshipCourting => "courting",
            Self::CourtshipMating => "mating",
            Self::CourtshipBonded => "bonded",
            Self::PlayBoutApproach => "play_approach",
            Self::PlayBoutBouting => "play_bouting",
            Self::PlayBoutCooldown => "play_cooldown",
        }
    }
}

/// Branch identifying which §7.M drop gate fired. Carry-overs (first
/// five) preserve `PairingDropBranch` semantics 1:1. Novel branches
/// (last four) generalize to cross-practice cascade detection.
///
/// Useful for trace observability and per-branch granularity in
/// narrative output. The `Feature::JointIntentionDropped { practice }`
/// counter collapses across branches; the per-branch label lives on the
/// focal-cat trace's `JointIntentionCapture` (added in Commit B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum JointDropBranch {
    // ---------- 5 carry-overs from PairingDropBranch ----------
    /// Partner is `Dead`, `Banished`, `Incapacitated`, or despawned.
    PartnerInvalid,
    /// Bond no longer reaches the practice floor (Friends+ for
    /// Courtship). Defensive — `social::check_bonds` upgrades only
    /// today, but a future ticket may add downgrade.
    BondLost,
    /// Both `romantic < romantic_floor` AND `fondness < fondness_floor`.
    /// Mates-bonded post-conception cooldown (high fondness,
    /// zero-romantic) does NOT drop on the romantic axis alone — the
    /// conjunction-floor protects long-term bonds.
    DesireDrift,
    /// Tom-side: `season == Winter`. Queens / Nonbinaries:
    /// `Fertility { phase ∈ {Anestrus, Postpartum} }`. The
    /// photoperiodic / cycle-out drop §7.M.4 names.
    SeasonOut,
    /// Cat no longer meets the L1-equivalent reproductive eligibility
    /// gate (life-stage transitioned past Adult/Elder, became Asexual,
    /// or Pregnant). The L1 cascade-drop.
    AspirationCascade,

    // ---------- 4 novel for JointIntention ----------
    /// **Cascade trigger.** Partner no longer holds a compatible
    /// `JointIntention` (no JI, different practice, or different
    /// partner field). The drop on this side propagates from the
    /// partner's prior drop within 1 tick (Bevy buffers `Commands`
    /// inserts/removes until system boundary, so within one
    /// author-system tick a partner-removed JI is still visible to the
    /// drop-gate; cascade fires the *following* tick). Wired in Commit
    /// B.
    PartnerLeftPractice,
    /// Self has been in `current_stage` for more than
    /// `stage_stall_ticks` ticks without advancing. Catches degenerate
    /// pairs where the structural compatibility holds but the
    /// observable proxies don't progress (e.g., a Tom-Queen pair where
    /// the bond never tips into Partners). Wired in Commit B.
    StageStalled,
    /// `is_practice_compatible(self, partner, practice)` flipped false
    /// post-adoption (e.g., orientation re-classification, life-stage
    /// transition). Distinct from `AspirationCascade` which fires on
    /// self-only invalidity — `CompatibilityLost` fires on
    /// self-partner-pair invalidity (e.g., both became Asexual). Wired
    /// in Commit B.
    CompatibilityLost,
    /// Terminal stage reached with natural-termination semantics.
    /// **Dead code for Courtship in 127** — `CourtshipBonded` persists
    /// until `PartnerInvalid` / `BondLost` / `PartnerLeftPractice`
    /// fires, matching today's PairingActivity-during-pregnancy
    /// behavior. Activated by ticket 276 for `PracticeKind::PlayBout`:
    /// fires when a cat in `PracticeStage::PlayBoutCooldown` has been
    /// in stage for `playbout_cooldown_ticks`. The author emits
    /// `EventKind::JointPlayBoutCompleted` before removing the JI
    /// (which increments `continuity_tallies["play"]`).
    Completed,
}

// ---------------------------------------------------------------------------
// JointIntention Component
// ---------------------------------------------------------------------------

/// L2 JointIntention Component persisted on the cat. Ticket 127.
///
/// Inserted by `crate::ai::joint_intention::author_joint_intentions` on a
/// matched candidate; removed by the same system on any
/// `JointDropBranch` trigger. The component is the source of truth —
/// there is no parallel ZST marker. Bias readers query
/// `Option<&JointIntention>` directly.
///
/// Only `Serialize` is derived (not `Deserialize`) because `partner:
/// Entity` has no `Default` and the component is pure runtime state —
/// no save/load path round-trips it. The trace pipeline reads it via
/// `Serialize` only.
///
/// All seven fields are observable practice-state per §Field discipline.
/// Notably absent: `commitment_strength`, `expiry_tick`, `source` —
/// those live on the actor's `HeldIntention`, which a cat in a joint
/// practice ALSO holds.
#[derive(Component, Debug, Clone, serde::Serialize)]
pub struct JointIntention {
    /// Which practice this is. Drives matchmaker compatibility,
    /// stage-advancement, drop branches, and per-practice `Feature`
    /// emission.
    pub practice: PracticeKind,
    /// The other participant. `Entity` has no `Default`, so
    /// `#[serde(skip)]` (mirrors `PairingActivity.partner`).
    #[serde(skip)]
    pub partner: Entity,
    /// Role this cat performs in the practice. Courtship uses `Mutual`
    /// to preserve the existing symmetric matchmaker shape 1:1.
    pub role: PracticeRole,
    /// Current stage this cat believes the practice is in. Stages MAY
    /// mismatch between partners — see §Codified irony.
    pub stage: PracticeStage,
    /// Tick the JointIntention was first authored on this cat.
    pub adopted_tick: u64,
    /// Tick this cat last entered its current stage. Drives the
    /// `StageStalled` drop branch and trace `ticks_in_stage`. Equal to
    /// `adopted_tick` until the first stage transition.
    pub stage_entered_tick: u64,
    /// Most-recent observed partnered interaction (any practice-biased
    /// resolver pick). Refreshed by resolvers that already call the
    /// bias multiplier today. Equal to `adopted_tick` at emission time.
    pub last_interaction_tick: u64,
}

impl JointIntention {
    /// Convenience constructor used by the author system. Starts at
    /// `CourtshipApproach` for Courtship; future practices add their
    /// own entry-stage logic to this constructor or a per-practice
    /// factory.
    pub fn new(practice: PracticeKind, partner: Entity, tick: u64) -> Self {
        let stage = match practice {
            PracticeKind::Courtship => PracticeStage::CourtshipApproach,
            PracticeKind::PlayBout => PracticeStage::PlayBoutApproach,
        };
        Self {
            practice,
            partner,
            role: PracticeRole::Mutual,
            stage,
            adopted_tick: tick,
            stage_entered_tick: tick,
            last_interaction_tick: tick,
        }
    }
}

// ---------------------------------------------------------------------------
// Bias-reader helpers — preserve the 257 Commit B contract
// ---------------------------------------------------------------------------

/// Ticket 257 / Commit B bias-reader helper, generalized to practice
/// filter. Given an actor's optional `JointIntention` and a resolver
/// target, return the multiplier the resolver should apply to its
/// fondness / familiarity delta.
///
/// The amplification fires when ALL THREE conditions hold:
/// 1. The actor holds a `JointIntention`,
/// 2. Its `practice` field equals the resolver's expected practice,
/// 3. Its `partner` field equals the resolver's target.
///
/// Any other case returns `(1.0, false)`. The `bool` is `true` exactly
/// when the multiplier was the bias value — caller emits
/// `Feature::JointBiasApplied { practice }` in that case.
pub fn joint_bias_multiplier(
    joint: Option<&JointIntention>,
    target: Entity,
    practice: PracticeKind,
    bias_multiplier: f32,
) -> (f32, bool) {
    let partner = joint.and_then(|j| (j.practice == practice).then_some(j.partner));
    joint_bias_for(partner, Some(target), bias_multiplier)
}

/// Snapshot-friendly variant of [`joint_bias_multiplier`]. Takes a
/// pre-extracted `Option<Entity>` partner snapshot and an
/// `Option<Entity>` target.
///
/// Used from chain-step contexts where `&mut Commands` and
/// `Query<&JointIntention>` conflict in the same system. Callers build
/// a `HashMap<Entity, Entity>` snapshot during a query pass — the
/// **query itself filters by practice** (e.g.,
/// `Query<&JointIntention>` filtered on `.practice == Courtship`), so
/// the snapshot only contains partners for the relevant practice. This
/// keeps the call-site signature identical to the prior
/// `pairing_bias_for` and pushes the practice filter to one place
/// (snapshot construction).
///
/// `target == None` is the "resolver had no target this tick" case
/// and produces `(1.0, false)` so callers can use the same helper on
/// every branch without conditional logic.
pub fn joint_bias_for(
    partner: Option<Entity>,
    target: Option<Entity>,
    bias_multiplier: f32,
) -> (f32, bool) {
    match (partner, target) {
        (Some(p), Some(t)) if p == t => (bias_multiplier, true),
        _ => (1.0, false),
    }
}

// ---------------------------------------------------------------------------
// Drop-gate predicate
// ---------------------------------------------------------------------------

/// Per-cat snapshot of every field [`should_drop_joint`] reads. Built
/// once per author-system tick (a small struct rather than a fresh world
/// query per check). Mirrors the `PairingProxies` snapshot pattern.
///
/// In Commit A, the four novel-branch fields (`partner_joint`,
/// `partner_is_pregnant`, `still_compatible`, `now_tick`) are populated
/// but the corresponding drop branches stay dead. Commit B activates
/// them by wiring the author system to read partner state + a fresh
/// compatibility re-check.
#[derive(Debug, Clone, Copy)]
pub struct JointIntentionProxies {
    // ---------- Self physical / identity ----------
    pub self_stage: LifeStage,
    pub self_orientation: Orientation,
    pub self_gender: Gender,
    pub self_is_pregnant: bool,
    pub self_fertility_phase: Option<FertilityPhase>,
    // ---------- Partner state ----------
    /// `true` when partner is `Dead`, `Banished`, `Incapacitated`, or
    /// despawned. Triggers `PartnerInvalid` (highest precedence).
    pub partner_invalid: bool,
    /// `true` when partner currently holds a compatible
    /// `JointIntention` pointing back at this cat. Author computes this
    /// from a per-tick partner-state snapshot:
    /// `partner_ji.is_some_and(|p| p.practice == self.practice && p.partner == self_entity)`.
    /// `false` triggers `PartnerLeftPractice` (cascade). **Commit A
    /// passes `true` unconditionally** so the cascade branch stays
    /// dead; Commit B computes the actual value and activates the
    /// branch.
    pub partner_in_practice: bool,
    /// `true` when `Pregnant` is on partner. Drives stage
    /// `Mating → Bonded` advancement (the stage-advance function reads
    /// this; the drop function does not).
    pub partner_is_pregnant: bool,
    // ---------- Relationship ----------
    /// Current bond between self and partner. `None`, `Acquaintance`,
    /// and `Hostile` trigger `BondLost`.
    pub bond: Option<BondType>,
    pub romantic: f32,
    pub fondness: f32,
    // ---------- Environment ----------
    pub season: Season,
    // ---------- Practice context ----------
    pub practice: PracticeKind,
    pub current_stage: PracticeStage,
    pub stage_entered_tick: u64,
    pub now_tick: u64,
    /// `is_practice_compatible(self, partner, practice)` re-evaluated
    /// at this tick. `false` triggers `CompatibilityLost`. **Commit A
    /// passes `true` unconditionally** so the branch stays dead; Commit
    /// B exposes per-practice compatibility dispatch.
    pub still_compatible: bool,
}

/// Constants the drop gate consults. Lifted into the function signature
/// so unit tests can construct a deterministic floor without standing
/// up a `Res<SimConstants>`.
#[derive(Debug, Clone, Copy)]
pub struct JointIntentionDropConfig {
    pub romantic_floor: f32,
    pub fondness_floor: f32,
    /// Max ticks a cat may sit in a single `PracticeStage` before
    /// `StageStalled` fires. Default 10_000 ≈ 10 sim-days at default
    /// constants — generous enough to absorb temporary blockers
    /// (winter pause, gestation) while still catching never-progresses
    /// pairs.
    pub stage_stall_ticks: u64,
    /// Ticket 276 — ticks a PlayBout cat must sit in
    /// `PracticeStage::PlayBoutCooldown` before `JointDropBranch::Completed`
    /// fires. Read only when `current_stage == PlayBoutCooldown`; otherwise
    /// the Completed arm is inert.
    pub playbout_cooldown_ticks: u64,
}

/// Pure drop gate. Returns `Some(branch)` iff any drop trigger fires;
/// `None` means hold the JointIntention for another tick.
///
/// Branch precedence (first-match wins):
///
/// 1. `PartnerInvalid`
/// 2. `PartnerLeftPractice` *(novel; wired Commit B)*
/// 3. `BondLost`
/// 4. `AspirationCascade`
/// 5. `SeasonOut`
/// 6. `CompatibilityLost` *(novel; wired Commit B)*
/// 7. `StageStalled` *(novel; wired Commit B)*
/// 8. `DesireDrift`
/// 9. `Completed` *(novel; dead code for Courtship in 127)*
///
/// Picked so that cascade triggers (`PartnerInvalid`,
/// `PartnerLeftPractice`) outrank slow-collapse checks, and so that
/// "the partner is Dead" reports `PartnerInvalid` rather than the
/// downstream `DesireDrift` that follows from a Dead-partner
/// relationship row.
pub fn should_drop_joint(
    proxies: &JointIntentionProxies,
    config: &JointIntentionDropConfig,
) -> Option<JointDropBranch> {
    if proxies.partner_invalid {
        return Some(JointDropBranch::PartnerInvalid);
    }
    // PartnerLeftPractice — partner no longer holds a compatible JI
    // pointing back at us. Author computes the flag from a per-tick
    // partner-state snapshot; Commit A passes `true` unconditionally so
    // this branch stays dead, Commit B activates it.
    //
    // Cascade semantics: when partner drops on tick T, their JI is
    // removed via `commands` which buffers until system boundary. The
    // partner-snapshot is built BEFORE per-cat evaluation, so on tick T
    // the partner's JI is still in the snapshot and this branch does
    // NOT fire. Tick T+1 the snapshot reflects the removal and the
    // cascade fires — meeting the §Exit criterion 3 "within 1 tick"
    // budget.
    if !proxies.partner_in_practice {
        return Some(JointDropBranch::PartnerLeftPractice);
    }
    // Practice-specific eligibility gates. Courtship requires
    // Friends+ bond + fertility eligibility + season-in; PlayBout is
    // broadly permissive (just Adult/Elder life-stage). Without these
    // carve-outs, ticket 276's matchmaker emits PlayBout JIs at full
    // throughput and the drop gate cascades them on tick T+1
    // (BondLost on any non-Friends pair, etc.) producing 10k+
    // emission churn with zero stage progression.
    match proxies.practice {
        PracticeKind::Courtship => {
            // BondLost — Courtship requires Friends-or-better.
            match proxies.bond {
                Some(BondType::Friends | BondType::Partners | BondType::Mates) => {}
                _ => return Some(JointDropBranch::BondLost),
            }
            // AspirationCascade — self no longer reproductive-eligible.
            let aspiration_cascade =
                !matches!(proxies.self_stage, LifeStage::Adult | LifeStage::Elder)
                    || matches!(proxies.self_orientation, Orientation::Asexual)
                    || proxies.self_is_pregnant;
            if aspiration_cascade {
                return Some(JointDropBranch::AspirationCascade);
            }
            // SeasonOut — Tom-in-Winter or Queen-Anestrus/Postpartum.
            let season_out = matches!(proxies.self_gender, Gender::Tom)
                && matches!(proxies.season, Season::Winter)
                || matches!(
                    proxies.self_fertility_phase,
                    Some(FertilityPhase::Anestrus) | Some(FertilityPhase::Postpartum)
                );
            if season_out {
                return Some(JointDropBranch::SeasonOut);
            }
        }
        PracticeKind::PlayBout => {
            // PlayBout has no bond floor, no orientation gate, no
            // fertility gate, no season gate. The only life-stage
            // requirement is Adult/Elder (kittens get Caretake,
            // not PlayBout). A cat aging out of Adult/Elder is the
            // only AspirationCascade trigger here.
            if !matches!(proxies.self_stage, LifeStage::Adult | LifeStage::Elder) {
                return Some(JointDropBranch::AspirationCascade);
            }
        }
    }
    // CompatibilityLost — author re-ran is_practice_compatible.
    if !proxies.still_compatible {
        return Some(JointDropBranch::CompatibilityLost);
    }
    // StageStalled — too long in current stage.
    let ticks_in_stage = proxies.now_tick.saturating_sub(proxies.stage_entered_tick);
    if ticks_in_stage > config.stage_stall_ticks {
        return Some(JointDropBranch::StageStalled);
    }
    // DesireDrift — both axes collapse. Skipped for PlayBout (the
    // romantic/fondness floors are calibrated for Courtship; PlayBout
    // pairs may have low romantic and still play happily together).
    if !matches!(proxies.practice, PracticeKind::PlayBout)
        && proxies.romantic < config.romantic_floor
        && proxies.fondness < config.fondness_floor
    {
        return Some(JointDropBranch::DesireDrift);
    }
    // Completed — PlayBout cooldown elapsed. Activates the previously-
    // dead `Completed` branch (ticket 276). Gated on practice + stage
    // so Courtship's Bonded stage stays terminal-by-drop-cascade
    // (matching the pre-276 behavior).
    if matches!(proxies.practice, PracticeKind::PlayBout)
        && matches!(proxies.current_stage, PracticeStage::PlayBoutCooldown)
        && ticks_in_stage > config.playbout_cooldown_ticks
    {
        return Some(JointDropBranch::Completed);
    }
    None
}

// ---------------------------------------------------------------------------
// Stage progression — observable, not synchronized
// ---------------------------------------------------------------------------

/// Per-cat snapshot of every field [`next_stage`] reads. Built once per
/// author-system tick from the same query data as
/// `JointIntentionProxies`. Stage advancement is **independent per cat**;
/// partners may briefly mismatch (§Codified irony).
#[derive(Debug, Clone, Copy)]
pub struct StageAdvanceProxies {
    pub current_stage: PracticeStage,
    pub last_interaction_tick: u64,
    pub adopted_tick: u64,
    pub bond: Option<BondType>,
    pub self_gender: Gender,
    pub season: Season,
    pub self_fertility_phase: Option<FertilityPhase>,
    pub self_is_pregnant: bool,
    pub partner_is_pregnant: bool,
    /// Ticket 276 — current tick. Drives the PlayBout
    /// `Approach → Bouting` and `Bouting → Cooldown` tick-based
    /// advances.
    pub now_tick: u64,
    /// Ticket 276 — tick the cat entered `current_stage`. Drives the
    /// PlayBout tick-based stage advances.
    pub stage_entered_tick: u64,
    /// Ticket 276 — `Approach → Bouting` gate (ticks in stage). No
    /// bias-reader dependency — the matchmaker already matched on
    /// co-presence within `candidate_range`, so this gate models the
    /// brief approach window before play kicks off.
    pub playbout_approach_duration_ticks: u64,
    /// Ticket 276 — `Bouting → Cooldown` gate (ticks in stage).
    pub playbout_bouting_duration_ticks: u64,
}

/// Pure stage-advance predicate. Returns `Some(new_stage)` when the
/// cat's observable proxies satisfy a forward transition; `None`
/// otherwise. **Wired in Commit B.**
///
/// Transition table (Courtship):
///
/// | From | To | Predicate (self-observable) |
/// |---|---|---|
/// | Approach | Courting | `last_interaction_tick != adopted_tick` (any paired-resolver tick has fired since adoption) |
/// | Courting | Mating | `bond ∈ {Partners, Mates}` AND fertile (Tom: not Winter; Queen: Estrus) |
/// | Mating | Bonded | `bond == Mates` OR self-pregnant OR partner-pregnant |
/// | Bonded | — | terminal for Courtship; persistence handled by drop gate |
pub fn next_stage(proxies: &StageAdvanceProxies) -> Option<PracticeStage> {
    use PracticeStage::*;
    match proxies.current_stage {
        CourtshipApproach => {
            if proxies.last_interaction_tick != proxies.adopted_tick {
                Some(CourtshipCourting)
            } else {
                None
            }
        }
        CourtshipCourting => {
            let bond_ok = matches!(proxies.bond, Some(BondType::Partners | BondType::Mates));
            let fertile = match proxies.self_gender {
                Gender::Tom => !matches!(proxies.season, Season::Winter),
                _ => matches!(proxies.self_fertility_phase, Some(FertilityPhase::Estrus)),
            };
            if bond_ok && fertile {
                Some(CourtshipMating)
            } else {
                None
            }
        }
        CourtshipMating => {
            let mates = matches!(proxies.bond, Some(BondType::Mates));
            if mates || proxies.self_is_pregnant || proxies.partner_is_pregnant {
                Some(CourtshipBonded)
            } else {
                None
            }
        }
        CourtshipBonded => None,
        // Ticket 276 — PlayBout stage progression.
        //
        // | From | To | Predicate |
        // |---|---|---|
        // | Approach | Bouting | `now_tick - stage_entered_tick > playbout_approach_duration_ticks` |
        // | Bouting | Cooldown | `now_tick - stage_entered_tick > playbout_bouting_duration_ticks` |
        // | Cooldown | — | terminal in `next_stage`; advances via `should_drop_joint`'s `Completed` arm |
        //
        // All three transitions are tick-elapsed gates — PlayBout
        // doesn't depend on `JointInteractionObserved` bias-reader
        // messages (which Courtship uses for `Approach → Courting`).
        // The matchmaker emission predicate already gates on
        // co-presence within `candidate_range`, so the cats start the
        // practice co-located. Bias-reader wiring lands in a follow-on
        // ticket; the substrate is self-contained without it.
        PlayBoutApproach => {
            let ticks_in_stage = proxies.now_tick.saturating_sub(proxies.stage_entered_tick);
            if ticks_in_stage > proxies.playbout_approach_duration_ticks {
                Some(PlayBoutBouting)
            } else {
                None
            }
        }
        PlayBoutBouting => {
            let ticks_in_stage = proxies.now_tick.saturating_sub(proxies.stage_entered_tick);
            if ticks_in_stage > proxies.playbout_bouting_duration_ticks {
                Some(PlayBoutCooldown)
            } else {
                None
            }
        }
        PlayBoutCooldown => None,
    }
}

// ---------------------------------------------------------------------------
// Compatibility predicate (matchmaker)
// ---------------------------------------------------------------------------

/// Per-practice compatibility predicate evaluated by the matchmaker and
/// the post-adoption `CompatibilityLost` drop check.
///
/// For Courtship in 127, this delegates to the existing courtship
/// matchmaker (orientation compatibility + reproductive eligibility +
/// Friends-or-better bond + within range + score ≥ threshold). The
/// dispatch lives here so future practices declare their own predicate
/// without re-touching the author system.
///
/// In Commit A, this is unused — the author wraps the existing
/// `crate::ai::pairing::pick_partner` logic verbatim and `still_compatible`
/// in the proxies is hard-coded `true`. Commit B exposes it.
pub const fn is_practice_compatible(_practice: PracticeKind) -> bool {
    // Stub: Commit B fills in the per-practice dispatch.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn happy_proxies() -> JointIntentionProxies {
        JointIntentionProxies {
            self_stage: LifeStage::Adult,
            self_orientation: Orientation::Straight,
            self_gender: Gender::Queen,
            self_is_pregnant: false,
            self_fertility_phase: Some(FertilityPhase::Estrus),
            partner_invalid: false,
            partner_in_practice: true,
            partner_is_pregnant: false,
            bond: Some(BondType::Friends),
            romantic: 0.4,
            fondness: 0.5,
            season: Season::Spring,
            practice: PracticeKind::Courtship,
            current_stage: PracticeStage::CourtshipApproach,
            stage_entered_tick: 0,
            now_tick: 100,
            still_compatible: true,
        }
    }

    fn config() -> JointIntentionDropConfig {
        JointIntentionDropConfig {
            romantic_floor: 0.05,
            fondness_floor: 0.30,
            stage_stall_ticks: 10_000,
            playbout_cooldown_ticks: 30,
        }
    }

    // -----------------------------------------------------------------
    // Carry-over drop branches (mirrors `pairing.rs` test depth).
    // -----------------------------------------------------------------

    #[test]
    fn happy_path_holds_intention() {
        assert_eq!(should_drop_joint(&happy_proxies(), &config()), None);
    }

    #[test]
    fn drops_when_partner_invalid() {
        let mut p = happy_proxies();
        p.partner_invalid = true;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::PartnerInvalid)
        );
    }

    #[test]
    fn partner_invalid_outranks_other_drop_branches() {
        // Every drop trigger fires simultaneously — precedence must
        // report PartnerInvalid (highest).
        let p = JointIntentionProxies {
            partner_invalid: true,
            partner_in_practice: false,
            bond: None,
            self_stage: LifeStage::Kitten,
            self_orientation: Orientation::Asexual,
            self_is_pregnant: true,
            self_fertility_phase: Some(FertilityPhase::Anestrus),
            self_gender: Gender::Tom,
            season: Season::Winter,
            romantic: 0.0,
            fondness: 0.0,
            partner_is_pregnant: false,
            practice: PracticeKind::Courtship,
            current_stage: PracticeStage::CourtshipApproach,
            stage_entered_tick: 0,
            now_tick: u64::MAX / 2,
            still_compatible: false,
        };
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::PartnerInvalid)
        );
    }

    #[test]
    fn drops_when_bond_lost() {
        let mut p = happy_proxies();
        p.bond = None;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::BondLost)
        );
    }

    #[test]
    fn holds_for_partners_and_mates_bonds() {
        for bond in [BondType::Partners, BondType::Mates] {
            let mut p = happy_proxies();
            p.bond = Some(bond);
            assert_eq!(should_drop_joint(&p, &config()), None);
        }
    }

    #[test]
    fn drops_when_kitten() {
        let mut p = happy_proxies();
        p.self_stage = LifeStage::Kitten;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn drops_when_asexual() {
        let mut p = happy_proxies();
        p.self_orientation = Orientation::Asexual;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn drops_when_pregnant() {
        let mut p = happy_proxies();
        p.self_is_pregnant = true;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn drops_tom_in_winter() {
        let mut p = happy_proxies();
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Winter;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::SeasonOut)
        );
    }

    #[test]
    fn holds_tom_outside_winter() {
        let mut p = happy_proxies();
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Summer;
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn drops_queen_in_anestrus() {
        let mut p = happy_proxies();
        p.self_fertility_phase = Some(FertilityPhase::Anestrus);
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::SeasonOut)
        );
    }

    #[test]
    fn drops_queen_in_postpartum() {
        let mut p = happy_proxies();
        p.self_fertility_phase = Some(FertilityPhase::Postpartum);
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::SeasonOut)
        );
    }

    #[test]
    fn drops_when_both_axes_collapse() {
        let mut p = happy_proxies();
        p.romantic = 0.01;
        p.fondness = 0.05;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::DesireDrift)
        );
    }

    #[test]
    fn holds_when_only_romantic_collapses() {
        // Mates-bonded post-conception cooldown shape.
        let mut p = happy_proxies();
        p.bond = Some(BondType::Mates);
        p.romantic = 0.0;
        p.fondness = 0.85;
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn holds_when_only_fondness_collapses() {
        let mut p = happy_proxies();
        p.fondness = 0.0;
        p.romantic = 0.6;
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    // -----------------------------------------------------------------
    // Novel drop branches — wired in Commit B but predicate is tested
    // now to lock the precedence + transition semantics. Commit A's
    // author passes `partner_in_practice: true` + `still_compatible:
    // true` unconditionally, so the cascade branches stay dead from the
    // author. The predicate itself is exercised by these tests.
    // -----------------------------------------------------------------

    #[test]
    fn drops_when_partner_left_practice() {
        let mut p = happy_proxies();
        // Author in Commit B will set this `false` when partner's JI
        // shape no longer matches `(self.practice, self_entity)`.
        p.partner_in_practice = false;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::PartnerLeftPractice)
        );
    }

    #[test]
    fn partner_left_practice_outranks_bond_lost() {
        // Precedence: PartnerLeftPractice (2) > BondLost (3).
        let mut p = happy_proxies();
        p.partner_in_practice = false;
        p.bond = None;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::PartnerLeftPractice)
        );
    }

    #[test]
    fn drops_when_stage_stalled() {
        let mut p = happy_proxies();
        p.stage_entered_tick = 0;
        p.now_tick = 20_000;
        // 20_000 > 10_000 stage_stall_ticks default; should fire.
        // Carry-over branches must not fire (happy proxies otherwise).
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::StageStalled)
        );
    }

    #[test]
    fn drops_when_compatibility_lost() {
        let mut p = happy_proxies();
        p.still_compatible = false;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::CompatibilityLost)
        );
    }

    #[test]
    fn compatibility_lost_outranks_stage_stalled() {
        // CompatibilityLost (precedence 6) outranks StageStalled (7).
        let mut p = happy_proxies();
        p.still_compatible = false;
        p.stage_entered_tick = 0;
        p.now_tick = 20_000;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::CompatibilityLost)
        );
    }

    #[test]
    fn season_out_outranks_compatibility_lost() {
        // SeasonOut (5) outranks CompatibilityLost (6) and StageStalled (7).
        let mut p = happy_proxies();
        p.self_fertility_phase = Some(FertilityPhase::Anestrus);
        p.still_compatible = false;
        p.stage_entered_tick = 0;
        p.now_tick = 20_000;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::SeasonOut)
        );
    }

    // -----------------------------------------------------------------
    // Bias-multiplier helper invariants. Mirrors `pairing.rs` depth +
    // adds the practice-filter case.
    // -----------------------------------------------------------------

    fn entity(idx: u32) -> Entity {
        let mut world = bevy_ecs::world::World::new();
        for _ in 0..idx.saturating_sub(1) {
            world.spawn_empty();
        }
        world.spawn_empty().id()
    }

    #[test]
    fn bias_multiplier_no_joint_returns_one() {
        let target = entity(1);
        let (mult, amped) = joint_bias_multiplier(None, target, PracticeKind::Courtship, 1.5);
        assert_eq!(mult, 1.0);
        assert!(!amped);
    }

    #[test]
    fn bias_multiplier_partner_and_practice_match_amplifies() {
        let partner = entity(7);
        let joint = JointIntention::new(PracticeKind::Courtship, partner, 100);
        let (mult, amped) =
            joint_bias_multiplier(Some(&joint), partner, PracticeKind::Courtship, 1.5);
        assert_eq!(mult, 1.5);
        assert!(amped);
    }

    #[test]
    fn bias_multiplier_partner_differs_from_target_returns_one() {
        let partner = entity(7);
        let other = entity(8);
        let joint = JointIntention::new(PracticeKind::Courtship, partner, 100);
        let (mult, amped) =
            joint_bias_multiplier(Some(&joint), other, PracticeKind::Courtship, 1.5);
        assert_eq!(mult, 1.0);
        assert!(!amped);
    }

    #[test]
    fn bias_multiplier_target_none_returns_one() {
        let partner = entity(7);
        let (mult, amped) = joint_bias_for(Some(partner), None, 1.5);
        assert_eq!(mult, 1.0);
        assert!(!amped);
    }

    // -----------------------------------------------------------------
    // Stage progression — pure predicate.
    // -----------------------------------------------------------------

    fn stage_proxies() -> StageAdvanceProxies {
        StageAdvanceProxies {
            current_stage: PracticeStage::CourtshipApproach,
            last_interaction_tick: 100,
            adopted_tick: 100,
            bond: Some(BondType::Friends),
            self_gender: Gender::Queen,
            season: Season::Spring,
            self_fertility_phase: Some(FertilityPhase::Estrus),
            self_is_pregnant: false,
            partner_is_pregnant: false,
            now_tick: 100,
            stage_entered_tick: 100,
            playbout_approach_duration_ticks: 30,
            playbout_bouting_duration_ticks: 60,
        }
    }

    #[test]
    fn approach_holds_until_first_interaction() {
        // last_interaction_tick == adopted_tick → no interactions yet
        // → stay in Approach.
        let p = stage_proxies();
        assert_eq!(next_stage(&p), None);
    }

    #[test]
    fn approach_advances_to_courting_on_first_interaction() {
        let mut p = stage_proxies();
        p.last_interaction_tick = 150; // Distinct from adopted_tick.
        assert_eq!(next_stage(&p), Some(PracticeStage::CourtshipCourting));
    }

    #[test]
    fn courting_advances_to_mating_when_partners_and_estrus() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipCourting;
        p.bond = Some(BondType::Partners);
        assert_eq!(next_stage(&p), Some(PracticeStage::CourtshipMating));
    }

    #[test]
    fn courting_holds_when_partners_but_anestrus() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipCourting;
        p.bond = Some(BondType::Partners);
        p.self_fertility_phase = Some(FertilityPhase::Anestrus);
        // Queen-side fertility blocks; SeasonOut drop fires first via
        // `should_drop_joint`, but the stage predicate alone holds.
        assert_eq!(next_stage(&p), None);
    }

    #[test]
    fn courting_advances_to_mating_for_tom_outside_winter() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipCourting;
        p.bond = Some(BondType::Partners);
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Summer;
        assert_eq!(next_stage(&p), Some(PracticeStage::CourtshipMating));
    }

    #[test]
    fn courting_holds_for_tom_in_winter() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipCourting;
        p.bond = Some(BondType::Partners);
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Winter;
        // Same shape as the SeasonOut drop — but the stage predicate
        // alone is what we're testing here.
        assert_eq!(next_stage(&p), None);
    }

    #[test]
    fn mating_advances_to_bonded_when_mates_bond() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipMating;
        p.bond = Some(BondType::Mates);
        assert_eq!(next_stage(&p), Some(PracticeStage::CourtshipBonded));
    }

    #[test]
    fn mating_advances_to_bonded_when_self_pregnant() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipMating;
        p.bond = Some(BondType::Partners);
        p.self_is_pregnant = true;
        assert_eq!(next_stage(&p), Some(PracticeStage::CourtshipBonded));
    }

    #[test]
    fn mating_advances_to_bonded_when_partner_pregnant() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipMating;
        p.bond = Some(BondType::Partners);
        p.partner_is_pregnant = true;
        assert_eq!(next_stage(&p), Some(PracticeStage::CourtshipBonded));
    }

    #[test]
    fn bonded_is_terminal() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::CourtshipBonded;
        p.bond = Some(BondType::Mates);
        assert_eq!(next_stage(&p), None);
    }

    // -----------------------------------------------------------------
    // Constructor invariants.
    // -----------------------------------------------------------------

    #[test]
    fn new_starts_at_courtship_approach() {
        let partner = entity(3);
        let j = JointIntention::new(PracticeKind::Courtship, partner, 200);
        assert_eq!(j.practice, PracticeKind::Courtship);
        assert_eq!(j.partner, partner);
        assert_eq!(j.role, PracticeRole::Mutual);
        assert_eq!(j.stage, PracticeStage::CourtshipApproach);
        assert_eq!(j.adopted_tick, 200);
        assert_eq!(j.stage_entered_tick, 200);
        assert_eq!(j.last_interaction_tick, 200);
    }

    // -----------------------------------------------------------------
    // Ticket 276 — PlayBout practice.
    // -----------------------------------------------------------------

    #[test]
    fn new_starts_play_bout_at_play_bout_approach() {
        let partner = entity(3);
        let j = JointIntention::new(PracticeKind::PlayBout, partner, 200);
        assert_eq!(j.practice, PracticeKind::PlayBout);
        assert_eq!(j.stage, PracticeStage::PlayBoutApproach);
    }

    #[test]
    fn play_bout_approach_advances_to_bouting_after_duration() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::PlayBoutApproach;
        p.stage_entered_tick = 100;
        p.now_tick = 200; // 100 ticks elapsed > 30-tick default duration.
        assert_eq!(next_stage(&p), Some(PracticeStage::PlayBoutBouting));
    }

    #[test]
    fn play_bout_approach_holds_within_duration() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::PlayBoutApproach;
        p.stage_entered_tick = 100;
        p.now_tick = 110; // 10 ticks elapsed < 30-tick duration.
        assert_eq!(next_stage(&p), None);
    }

    #[test]
    fn play_bout_bouting_advances_to_cooldown_after_duration() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::PlayBoutBouting;
        p.stage_entered_tick = 100;
        p.now_tick = 200; // 100 ticks elapsed > 60-tick default duration.
        assert_eq!(next_stage(&p), Some(PracticeStage::PlayBoutCooldown));
    }

    #[test]
    fn play_bout_bouting_holds_within_duration() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::PlayBoutBouting;
        p.stage_entered_tick = 100;
        p.now_tick = 130; // 30 ticks elapsed < 60-tick duration.
        assert_eq!(next_stage(&p), None);
    }

    #[test]
    fn play_bout_cooldown_terminal_in_next_stage() {
        let mut p = stage_proxies();
        p.current_stage = PracticeStage::PlayBoutCooldown;
        p.now_tick = u64::MAX / 2;
        assert_eq!(next_stage(&p), None);
    }

    fn play_bout_proxies() -> JointIntentionProxies {
        JointIntentionProxies {
            self_stage: LifeStage::Adult,
            self_orientation: Orientation::Straight,
            self_gender: Gender::Queen,
            self_is_pregnant: false,
            self_fertility_phase: Some(FertilityPhase::Estrus),
            partner_invalid: false,
            partner_in_practice: true,
            partner_is_pregnant: false,
            bond: Some(BondType::Friends),
            romantic: 0.0,
            fondness: 0.0,
            season: Season::Spring,
            practice: PracticeKind::PlayBout,
            current_stage: PracticeStage::PlayBoutBouting,
            stage_entered_tick: 100,
            now_tick: 120,
            still_compatible: true,
        }
    }

    #[test]
    fn play_bout_skips_desire_drift() {
        // PlayBout with low romantic/fondness should NOT drop on
        // DesireDrift — the floors are Courtship-calibrated.
        let p = play_bout_proxies();
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn play_bout_skips_bond_lost() {
        // PlayBout has no bond floor — `None` / `Acquaintance` /
        // `Hostile` bonds are still eligible. (Two cats can play
        // without being Friends-or-better bonded; the bond grows from
        // co-presence in social DSEs, not vice versa.) Without this
        // carve-out, the matchmaker's permissive eligibility (just
        // playfulness + mood + co-presence) collides with the BondLost
        // gate, dropping every fresh JI on tick T+1.
        let mut p = play_bout_proxies();
        p.bond = None;
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn play_bout_skips_season_out_for_tom_in_winter() {
        // Tom-in-Winter shouldn't drop a PlayBout (the SeasonOut
        // gate is fertility-photoperiodic, irrelevant to play).
        let mut p = play_bout_proxies();
        p.self_gender = Gender::Tom;
        p.self_fertility_phase = None;
        p.season = Season::Winter;
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn play_bout_skips_aspiration_cascade_for_pregnant() {
        // Pregnant queens can still play (orthogonal to reproductive
        // eligibility). AspirationCascade for PlayBout only fires on
        // life-stage transitions away from Adult/Elder.
        let mut p = play_bout_proxies();
        p.self_is_pregnant = true;
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn play_bout_drops_kitten_via_aspiration_cascade() {
        // The one AspirationCascade trigger that DOES apply to
        // PlayBout: life-stage outside Adult/Elder.
        let mut p = play_bout_proxies();
        p.self_stage = LifeStage::Kitten;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::AspirationCascade)
        );
    }

    #[test]
    fn play_bout_completed_fires_when_cooldown_elapsed() {
        let mut p = play_bout_proxies();
        p.current_stage = PracticeStage::PlayBoutCooldown;
        p.stage_entered_tick = 100;
        p.now_tick = 200; // 100 ticks in cooldown > 30-tick default.
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::Completed)
        );
    }

    #[test]
    fn play_bout_cooldown_holds_within_duration() {
        let mut p = play_bout_proxies();
        p.current_stage = PracticeStage::PlayBoutCooldown;
        p.stage_entered_tick = 100;
        p.now_tick = 110; // 10 ticks in cooldown < 30-tick duration.
        assert_eq!(should_drop_joint(&p, &config()), None);
    }

    #[test]
    fn completed_does_not_fire_for_courtship() {
        // Courtship in Bonded stage with enormous ticks_in_stage still
        // doesn't fire Completed — that branch is PlayBout-only.
        // StageStalled fires instead (precedence-correct).
        let mut p = happy_proxies();
        p.current_stage = PracticeStage::CourtshipBonded;
        p.stage_entered_tick = 0;
        p.now_tick = 50_000;
        assert_eq!(
            should_drop_joint(&p, &config()),
            Some(JointDropBranch::StageStalled)
        );
    }
}
