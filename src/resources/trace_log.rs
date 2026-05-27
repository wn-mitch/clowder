//! Focal-cat trace records — layer-by-layer observational surface for
//! the AI substrate refactor per §11 of `docs/systems/ai-substrate-refactor.md`.
//!
//! Headless-only emission. Systems that emit records gate on
//! `run_if(resource_exists::<FocalTraceTarget>)`. No interactive code path
//! sees the trace emitter. See §11.5.
//!
//! Shapes match §11.3 record sketches; the sidecar file
//! `logs/trace-<focal>.jsonl` is diff-joinable with `events.jsonl` via
//! the shared header (§11.4 joinability invariant).

use std::collections::VecDeque;
use std::sync::Mutex;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::system::{Res, SystemParam};

use crate::ai::dse::{DseId, Intention};
use crate::ai::eval::EvalTrace;
use crate::ai::scoring::SoftmaxCapture;

// ---------------------------------------------------------------------------
// FocalTraceTarget
// ---------------------------------------------------------------------------

/// Marker resource. When present, trace-emitter systems produce
/// layer-by-layer records for the named cat. Inserted only by the
/// headless runner (see `run_headless` in `src/main.rs`); never by
/// `SimulationPlugin`. Per §11.5 scope rule.
///
/// The target is identified by name at the CLI level; `entity` is
/// resolved lazily on the first tick the named cat is queryable.
/// Unresolved targets produce no records — the cat may not exist
/// yet (pre-birth), may have died, or the name may be typo'd.
#[derive(Resource, Debug, Clone)]
pub struct FocalTraceTarget {
    pub name: String,
    pub entity: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Shared sub-types — kept intentionally minimal at Phase 1 entry
// ---------------------------------------------------------------------------

/// Per-channel attenuation breakdown for L1 samples. Phase 2 wires
/// real values from the species × role × injury × environment pipeline
/// (§5.6.6). At Phase 1 the shim emits identity (1.0) for channels
/// the current scent/sensing code doesn't expose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AttenuationBreakdown {
    pub species_sens: f32,
    pub role_mod: f32,
    pub injury_deficit: f32,
    pub env_mul: f32,
}

impl Default for AttenuationBreakdown {
    fn default() -> Self {
        Self {
            species_sens: 1.0,
            role_mod: 1.0,
            injury_deficit: 0.0,
            env_mul: 1.0,
        }
    }
}

/// One contributor row — "which emitter drove this sample value?" —
/// load-bearing per §11.3 ("without the breakdown, you see 'scent is
/// high' but not *which* fox drove it").
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContributorRow {
    pub emitter: String,
    pub pos: (i32, i32),
    pub distance: i32,
    pub contribution: f32,
}

/// One consideration's contribution to an L2 DSE score. Fields mirror
/// §11.3 L2 record sketch; at Phase 1 entry the trait doesn't exist so
/// `curve` carries a descriptive string rather than a typed enum.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsiderationContribution {
    pub name: String,
    pub input: f32,
    /// Textual description of the response curve (e.g. `"Logistic(8,0.5)"`,
    /// `"Linear"`). Phase 3 will replace with a typed `Curve` enum.
    pub curve: String,
    pub score: f32,
    pub weight: f32,
    /// Optional spatial reference — set when this consideration reads
    /// an L1 map. Phase 2 enriches with per-consideration top-contributor
    /// join keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spatial: Option<SpatialRef>,
}

/// Per-candidate scoring breakdown for a target-taking DSE (§6.3).
/// Attached to a `TraceRecord::L2` at emit time — a target-taking DSE
/// scores multiple candidates through a single consideration bundle
/// and aggregates them, so the ranking sits at the DSE level rather
/// than the per-consideration level. `None` for non-target-taking
/// DSEs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TargetRanking {
    /// `"Best"` / `"SumTopN(3)"` / `"WeightedAverage"` (§6.3 modes).
    pub aggregation: String,
    pub candidates: Vec<TargetCandidate>,
    /// Entity-ish label for the winning target (typically a `Name`
    /// string or `Debug` representation). `None` when the candidate
    /// set was empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TargetCandidate {
    pub name: String,
    pub score: f32,
    /// True for the top-N candidates that contributed to the composed
    /// action score under `SumTopN`; always true under `Best` for the
    /// single winner; true for all under `WeightedAverage` (all
    /// contribute with decaying weight).
    pub contributed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpatialRef {
    pub map: String,
    pub best_target: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EligibilitySummary {
    pub markers_required: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CompositionSummary {
    /// `"WeightedSum"` / `"CompensatedProduct"` / `"Max"` (§3.1 modes;
    /// Phase 3 adds the enum). Phase 1 shim always emits `"WeightedSum"`
    /// since current scoring is additive.
    pub mode: String,
    pub raw: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModifierApplication {
    pub name: String,
    /// Set on additive modifiers (Pride bonus, Independence solo boost).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f32>,
    /// Set on multiplicative modifiers (Fox-territory suppression).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f32>,
    /// Ticket 400 — structured per-modifier breakdown for multi-axis
    /// modifiers whose `delta` doesn't carry enough information to
    /// debug. `ParentingActivityModifier` populates this with its five
    /// per-scale bias sums + Caretake suppression factor when it emits
    /// a non-zero lift; other modifiers leave it `None` (zero-cost
    /// when not tracing). Read by `frame-diff` and `just inspect`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Phase 3a lands a typed `Intention` enum (§L2.10.4); Phase 1 shim
/// captures the subset the current code produces.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IntentionSummary {
    /// `"Goal"` | `"Activity"` — §L2.10.5. Phase 1 shim emits `"Activity"`
    /// for today's DispositionKind-driven actions until the split lands.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_state: Option<String>,
}

/// Schema slot reserved for §7.W.6 top-N losing-axis logging. Populated
/// in Phase 6 when the Fulfillment register lands; empty vector at Phase 1.
/// Narrative emitters bind to "narrow winning axis + active losing
/// counter-axis + valence drop" triples via this field.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LosingAxisSlot {
    pub axis: String,
    pub score: f32,
    pub deficit: f32,
}

/// Schema slot reserved for §8.6 apophenia continuity canary: pairwise
/// behavioral distance across N sampled cats and same-cat autocorrelation
/// across K-day windows. Populated in Phase 6; `None` at Phase 1.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApopheniaSummary {
    pub pairwise_distance_sample: f32,
    pub self_autocorrelation_k_days: Vec<f32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SoftmaxSummary {
    pub temperature: f32,
    pub probabilities: Vec<f32>,
}

/// §7.2 belief-proxy triple emitted alongside the L3Commitment record.
/// Mirrors `crate::ai::commitment::BeliefProxies` but keeps the trace
/// schema independent of the internal type so downstream tooling
/// doesn't break if the proxy set grows.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BeliefProxySummary {
    pub achievement_believed: bool,
    pub achievable_believed: bool,
    pub still_goal: bool,
}

/// Plan-state snapshot emitted alongside the L3Commitment record. Lets
/// the trace reader reconstruct "was this a trip-counted achievement
/// or a replan-cap hit?" without cross-referencing `events.jsonl`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanStateSummary {
    pub trips_done: u32,
    pub target_trips: u32,
    pub replan_count: u32,
    pub max_replans: u32,
}

/// Per-§7 commitment layer. Phase 6 fills this with CommitmentStrategy +
/// persistence bonus; Phase 1 emits a best-effort shape with
/// `commitment_strength` mapping to today's patience bonus where relevant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MomentumSummary {
    pub active_intention: Option<String>,
    pub commitment_strength: f32,
    pub margin_threshold: f32,
    pub preempted: bool,
    /// Ticket 126 — DSE id of the cat's `HeldIntention` at the
    /// emit-tick (the actor-private commitment substrate
    /// `IntentionMomentum` lifts). `None` when no intention is held
    /// or the field hasn't been populated yet (C2 ships the writer).
    /// Distinct from `active_intention`, which is the displayed
    /// label on the L2 winner — `held_dse` answers "what was the cat
    /// already committed to before the softmax ran?".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_dse: Option<String>,
    /// Ticket 126 — runner-up margin
    /// (`chosen_score - runner_up_score`) recorded at adoption time.
    /// 0.0 when no live writer has populated it.
    #[serde(default)]
    pub runner_up_margin: f32,
    /// Ticket 126 — `IntentionMomentum` decay factor at the emit-tick
    /// (1.0 at adoption, ramps linearly to 0.0 at expiry). 0.0 when
    /// no intention is held or the field hasn't been populated yet.
    #[serde(default)]
    pub decay_factor: f32,
}

/// One frame from [`HeldGoalStack`](crate::components::held_goal_stack::HeldGoalStack)
/// serialized into an `L3Commitment` trace record. Per §11.5 registry-walk
/// discipline, the trace emitter walks the frames without any per-method
/// special-casing — new methods in the registry automatically appear here.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MethodFrameTraceRecord {
    /// Stable slug matching `MethodId.0`.
    pub method: String,
    /// `GoalFrame.goal_label` — the goal-state label this method was
    /// selected for.
    pub goal: String,
    /// Current cursor within the method's sub-goal list.
    pub sub_goal_index: usize,
    /// Total sub-goal count for the method (captured at push-time from
    /// `GoalFrame.sub_goal_count`). Serialized as `"of"` to match the
    /// `docs/systems/htn-methods.md` §Trace JSON schema (`"sub_goal_index": 0, "of": 4`).
    #[serde(rename = "of")]
    pub sub_goal_count: usize,
    /// Stable name slug for the bound target entity, if any. `None` when
    /// the method carries no bound target or name-resolution is unavailable
    /// at trace time. Emitted as `null` in JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Human-readable source — `"self"` / `"coordinator"` /
    /// `"aspiration:<chain-name>"`. Matches the `GoalFrameSnapshot.source`
    /// strings named in the §Trace inspection surface.
    pub source: String,
}

// ---------------------------------------------------------------------------
// TraceRecord — L1 / L2 / L3 variants per §11.3
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "layer")]
pub enum TraceRecord {
    /// L1 — one record per (focal cat × map × sample). Emitted lazily
    /// as a side-effect of an L2 consideration that reads the map; no
    /// every-tick × every-map emission.
    L1 {
        map: String,
        faction: String,
        channel: String,
        pos: (i32, i32),
        base_sample: f32,
        attenuation: AttenuationBreakdown,
        perceived: f32,
        top_contributors: Vec<ContributorRow>,
    },
    /// L2 — one record per (focal cat × eligible DSE × tick).
    L2 {
        dse: String,
        eligibility: EligibilitySummary,
        considerations: Vec<ConsiderationContribution>,
        composition: CompositionSummary,
        maslow_pregate: f32,
        modifiers: Vec<ModifierApplication>,
        final_score: f32,
        intention: IntentionSummary,
        /// Schema slot for §7.W.6 axis-capture logging — empty at Phase 1.
        top_losing: Vec<LosingAxisSlot>,
        /// Optional per-candidate target breakdown — set when this DSE
        /// is target-taking (§6.3) and the focal cat evaluated it this
        /// tick. `None` for regular (non-target-taking) DSEs and for
        /// target-taking DSEs that weren't evaluated against any
        /// candidates. Lets replay answer "why was Target#3 picked
        /// over Target#7?" without re-scoring.
        #[serde(skip_serializing_if = "Option::is_none")]
        targets: Option<TargetRanking>,
    },
    /// L3 — one record per (focal cat × tick). Closes the curvature
    /// loop: what the cat saw → wanted → planned to get.
    L3 {
        ranked: Vec<(String, f32)>,
        softmax: SoftmaxSummary,
        momentum: MomentumSummary,
        chosen: String,
        intention: IntentionSummary,
        goap_plan: Vec<String>,
        /// Action-keyed score Vec at `score_actions` exit. Locked
        /// invariant in `tests/scenarios.rs` asserts this equals
        /// `pre_penalty_pool` per-action.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pre_bonus_pool: Vec<(String, f32)>,
        /// Post-filter, pre-Independence-penalty pool the softmax saw.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pre_penalty_pool: Vec<(String, f32)>,
        /// Schema slot for §8.6 apophenia canary — `None` at Phase 1.
        #[serde(skip_serializing_if = "Option::is_none")]
        apophenia: Option<ApopheniaSummary>,
    },
    /// §7.2 commitment gate fired for the focal cat's held plan.
    /// Emitted once per gate evaluation (per cat, per tick where the
    /// gate ran). Captures which strategy and branch decided the
    /// outcome so regressions that track the 2026-04-23 lifted-
    /// condition bug class (pure helper passes tests, adjacent recipe
    /// wrong) are visible in the trace without re-running a bisection.
    ///
    /// At the time of introduction the pluggable Phase 6a gate is
    /// deferred (see `docs/systems/phase-6a-commitment-gate-attempt.md`)
    /// and this record is emitted from the de-facto commitment checks
    /// in `resolve_goap_plans`: the `disposition_complete` arm at
    /// `goap.rs:~1681` (the `achievement_believed` branch) and the
    /// `max_replans` exceeded arm at `goap.rs:~3144` (the
    /// `achievable_believed == false` / unachievable branch).
    L3Commitment {
        disposition: String,
        strategy: String,
        proxies: BeliefProxySummary,
        plan_state: PlanStateSummary,
        /// Which gate arm fired — `"achieved"` / `"unachievable"` /
        /// `"dropped_goal"` / `"retained"`. `"retained"` is emitted
        /// when the gate evaluated but decided to keep the plan;
        /// Phase 6a+ integrations should emit those rows too so the
        /// trace isn't silent on hold decisions.
        branch: String,
        /// Output of the gate — `true` means the plan is being removed.
        dropped: bool,
        /// Ticket 126 — momentum snapshot at gate evaluation. `None`
        /// when the cat has no `HeldIntention` (gate ran on the
        /// legacy `GoapPlan`-only path). C4 populates this on every
        /// gate evaluation against an intention-holding cat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        momentum: Option<MomentumSummary>,
        /// Ticket 126 — `IntentionAbandonReason::as_str()` slug when
        /// the gate fired a non-fulfilment drop. `None` for retained
        /// decisions, fulfilment drops, and pre-126 records.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        abandon_reason: Option<String>,
        /// Ticket 337 — snapshot of the focal cat's `HeldGoalStack` at
        /// gate-evaluation time, walked registry-style per §11.5. Empty
        /// when the cat has no active method frames (pre-128 behavior or
        /// cats running primitive Intentions with no method decomposition).
        /// Backward-compat: pre-337 records omit the field; diff tooling
        /// treats absence as an empty stack.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        method_stack: Vec<MethodFrameTraceRecord>,
    },
    /// Plan-failure branch fired — a plan was terminated by something
    /// other than `achievement_believed`. Distinct from `L3Commitment`
    /// because §7.5 Maslow preemption (`check_anxiety_interrupts`)
    /// bypasses the §7.2 gate entirely, and the `max_replans`-exceeded
    /// path emits both records (one framing it as the §7.2
    /// unachievable branch, one framing it as the executor-layer
    /// abandon) so replay tooling can distinguish whether the plan's
    /// own runtime or the commitment gate ended it.
    L3PlanFailure {
        /// `"replan_cap"` / `"anxiety_interrupt"` — the runtime cause
        /// that dropped the plan.
        reason: String,
        disposition: String,
        /// Tagged free-form detail — the replan path carries
        /// `{replan_count, max_replans}`, the anxiety path carries
        /// `{health_ratio, critical_threshold, preempted_step}`.
        detail: serde_json::Value,
    },
    /// Ticket 321 — per active aspiration per focal-cat tick. The
    /// picker (`crate::systems::aspiration_picker`) emits one record
    /// per `ActiveAspiration` after walking its current milestone's
    /// `emits[]` table. `emit_walk` carries one row per `Emit` row
    /// the picker considered before settling on the result; at 321's
    /// land most rows are emitted with `emit_walk: vec![]` (because
    /// most milestones declare `emits: &[]`). #338 enriches the
    /// record with the full registry-walked walk per
    /// `docs/systems/htn-methods.md` §Trace + inspection surface.
    L1Aspiration {
        /// Chain name (matches `ActiveAspiration.chain_name`).
        aspiration: String,
        /// Index into the chain's `milestones` slice that owned the
        /// `emits[]` table this record reports on.
        milestone: usize,
        /// One row per `Emit` row the picker considered. Populated
        /// only when the chosen milestone has authored emits;
        /// otherwise `vec![]`.
        emit_walk: Vec<EmitWalkRow>,
        /// `true` when the row reported in `emit_walk` (or the
        /// emission itself, if any) came from the §H step-3
        /// domain-affinity fallback rather than the milestone's
        /// own `emits[]` table.
        fallback_used: bool,
    },
    /// L4 — resolver-level modifier read (ticket 477). One record per
    /// (focal cat × resolver × tick) where a `resolve_*` step (or a
    /// combat system function) read the equipment-modifier aggregate
    /// and applied a non-trivial modifier. Makes "items have bite"
    /// legible: `damage_to_body_part` armor reduction, hunt-strike
    /// weapon bonus, and the prey-detection cloak/noise masks all
    /// surface here as named `ModifierApplication` rows, never as a
    /// hidden post-hoc bonus. Joinable with `events.jsonl` and the other
    /// trace layers by `(tick, cat)`.
    ///
    /// Modeled on the §3.5.1 DSE-side `ModifierApplication` shape so
    /// `frame-diff` / `just inspect` read it without schema changes; the
    /// `delta` field carries `post - pre`.
    L4Resolver {
        /// Resolver / system function name — `"damage_to_body_part"`,
        /// `"resolve_engage_prey"`, `"try_detect_cat"`.
        resolver: String,
        modifiers: Vec<ModifierApplication>,
    },
}

/// One row in the [`TraceRecord::L1Aspiration::emit_walk`] list — the
/// picker's per-`Emit`-row verdict. `applicable && method_live`
/// implies the row was eligible to emit; `emitted` names the row the
/// picker actually picked (at most one per aspiration per tick).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmitWalkRow {
    /// The `Emit.label` this row reports on.
    pub label: String,
    /// `true` when `Emit.applicable_when(world, entity)` returned
    /// true for the focal cat at this tick.
    pub applicable: bool,
    /// `true` when `MethodRegistry.lookup(label, world, entity)`
    /// returned `Some(_)` (a `Live`, currently-applicable method
    /// exists for this label).
    pub method_live: bool,
    /// `true` when this is the row the picker selected. At most one
    /// row per aspiration per tick carries `emitted: true`.
    pub emitted: bool,
}

// ---------------------------------------------------------------------------
// TraceEntry + TraceLog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceEntry {
    pub tick: u64,
    pub cat: String,
    #[serde(flatten)]
    pub record: TraceRecord,
}

/// In-memory buffer drained every tick by the headless runner's
/// `flush_trace_entries`. Follows the same `total_pushed` +
/// ring-buffer convention as `EventLog` so that flush is a single
/// forward-walk from `last_flushed` to `total_pushed`.
///
/// `capacity` is sized for one cat × ~30 DSEs × a handful of L1 samples
/// × L3 record per tick; flush-every-tick keeps live memory bounded.
#[derive(Resource, Debug)]
pub struct TraceLog {
    pub entries: VecDeque<TraceEntry>,
    pub capacity: usize,
    pub total_pushed: u64,
}

impl Default for TraceLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: 5000,
            total_pushed: 0,
        }
    }
}

impl TraceLog {
    pub fn push(&mut self, entry: TraceEntry) {
        self.entries.push_back(entry);
        self.total_pushed += 1;
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// FocalScoreCapture — per-tick rich L2/L3 capture surface
// ---------------------------------------------------------------------------

/// One DSE's worth of captured detail: the DSE id, its final score,
/// the full `EvalTrace` per-consideration + modifier breakdown, and the
/// emitted `Intention`. Populated by `score_dse_by_id` via
/// `evaluate_single_with_trace` when the scoring cat is the focal cat.
#[derive(Debug, Clone)]
pub struct CapturedDse {
    pub dse_id: DseId,
    pub raw_score: f32,
    pub gated_score: f32,
    pub final_score: f32,
    pub intention: Intention,
    pub trace: EvalTrace,
    /// §4 eligibility required-marker list, copied from the DSE's
    /// filter so §11.3's `eligibility.markers_required` is emitted
    /// verbatim.
    pub eligibility_required: Vec<&'static str>,
    pub eligibility_forbidden: Vec<&'static str>,
    /// Whether eligibility passed. `true` means the DSE was scored
    /// and `trace`/`*_score` fields carry real numbers. `false` means
    /// this is a stripped row emitted so "why didn't this DSE even
    /// appear?" is answerable from the trace — `trace` is default /
    /// empty, `final_score == 0.0`, and `intention` is a placeholder
    /// the caller records at the skip site.
    pub eligible: bool,
}

/// Per-tick focal-cat scoring capture. Populated during
/// `evaluate_and_plan` / `cat_scent_tick` (whichever system's scoring
/// pass runs for a given cat); drained and cleared by
/// `emit_focal_trace`.
///
/// The `Mutex` wrapper lets `EvalInputs` carry an immutable reference
/// that nonetheless mutates the capture — Bevy's `Resource` trait
/// requires `Send + Sync`, which rules out `RefCell`. The mutex is
/// uncontended in the single-threaded scoring path (no second writer
/// within a tick); the lock cost is negligible relative to the scoring
/// it guards. Making this a `Resource` means the plugin / main.rs
/// insert it once per run (alongside `FocalTraceTarget` + `TraceLog`)
/// and the capture persists across the system boundary from scoring to
/// emission.
#[derive(Resource, Debug, Default)]
pub struct FocalScoreCapture {
    pub inner: Mutex<FocalScoreCaptureInner>,
}

#[derive(Debug, Default)]
pub struct FocalScoreCaptureInner {
    /// One row per DSE scored this tick for the focal cat. Cleared on
    /// drain. Preserves push order so replay's L2 block matches scoring
    /// order.
    pub dses: Vec<CapturedDse>,
    /// Softmax capture — populated by `select_disposition_via_intention_softmax_with_trace`
    /// when the focal cat makes its disposition pick.
    pub softmax: Option<SoftmaxCapture>,
    /// §7.2 commitment-gate decisions observed this tick (§7.2 / de-
    /// facto branches in `resolve_goap_plans`). Emitted as
    /// `TraceRecord::L3Commitment` by `emit_focal_trace`. One cat can
    /// produce ≥1 rows per tick if multiple gate evaluations fire.
    pub commitment: Vec<CommitmentCapture>,
    /// Plan-failure branches observed this tick (replan-cap,
    /// anxiety-interrupt). Emitted as `TraceRecord::L3PlanFailure`.
    pub plan_failures: Vec<PlanFailureCapture>,
    /// Per-target-taking-DSE candidate rankings keyed by `DseId.0`.
    /// Merged into the matching L2 record's `targets` field at emit
    /// time — the DSE's own L2 capture carries the scalar score while
    /// this map carries the per-candidate breakdown. Stored by
    /// DseId key (not vec-index) because the scoring + target-
    /// resolution calls aren't guaranteed to interleave in the same
    /// order.
    pub target_rankings: std::collections::HashMap<&'static str, TargetRanking>,
    /// Ticket 118 — set to `true` when `check_modifier_preemption`
    /// fired on the focal cat this tick. `emit_focal_trace` flows this
    /// into `MomentumSummary.preempted` on the L3 record so trace
    /// consumers (and `clowder-focal-cat` reports) can see the
    /// substrate-driven preemption from the compact L3 row without
    /// having to scan `plan_failures` for the same tick.
    pub momentum_preempted: bool,
    /// Tick the capture was populated on. `emit_focal_trace` reads this
    /// to emit records with the correct `tick` field even when the
    /// capture is drained on a later tick (shouldn't happen under normal
    /// cadence, but we guard against drift).
    pub captured_tick: Option<u64>,
    /// Ticket 477 — resolver-level modifier reads observed this tick.
    /// One row per (resolver × modifier) the focal cat's resolvers
    /// applied. Grouped by `resolver` name into `TraceRecord::L4Resolver`
    /// records at emit time.
    pub resolver_modifiers: Vec<ResolverModifierCapture>,
}

/// One resolver-level modifier read (ticket 477). Carries the resolver
/// name, the modifier label, and the pre/post scalar so the trace
/// records `post - pre` as the `ModifierApplication.delta`. Pushed via
/// [`FocalResolverSink::record`] (focal-gated) and grouped by resolver
/// at emit time.
#[derive(Debug, Clone)]
pub struct ResolverModifierCapture {
    pub resolver: &'static str,
    pub modifier: String,
    pub pre: f32,
    pub post: f32,
}

/// One commitment-gate decision. Fields mirror the §11.3 L3Commitment
/// schema; the `FocalScoreCapture.push_commitment` API is the single
/// write path used both by the pluggable Phase 6a gate (once wired)
/// and by the de-facto branches in `resolve_goap_plans`.
#[derive(Debug, Clone)]
pub struct CommitmentCapture {
    pub disposition: String,
    pub strategy: &'static str,
    pub achievement_believed: bool,
    pub achievable_believed: bool,
    pub still_goal: bool,
    pub trips_done: u32,
    pub target_trips: u32,
    pub replan_count: u32,
    pub max_replans: u32,
    pub branch: &'static str,
    pub dropped: bool,
    /// Ticket 126 — `IntentionAbandonReason::as_str()` for non-
    /// fulfilment drops; `None` otherwise. C4 populates.
    pub abandon_reason: Option<&'static str>,
}

/// One plan-failure event (replan cap or anxiety interrupt).
#[derive(Debug, Clone)]
pub struct PlanFailureCapture {
    pub reason: &'static str,
    pub disposition: String,
    pub detail: serde_json::Value,
}

impl FocalScoreCapture {
    pub fn push_dse(&self, row: CapturedDse, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.dses.push(row);
        inner.captured_tick = Some(tick);
    }

    pub fn set_softmax(&self, softmax: SoftmaxCapture, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.softmax = Some(softmax);
        inner.captured_tick = Some(tick);
    }

    /// Record a §7.2 commitment-gate decision for the focal cat.
    /// Accumulates per tick; drained by `emit_focal_trace` into one
    /// `TraceRecord::L3Commitment` row each.
    pub fn push_commitment(&self, row: CommitmentCapture, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.commitment.push(row);
        inner.captured_tick = Some(tick);
    }

    /// Record a plan-failure event (replan cap, anxiety interrupt).
    pub fn push_plan_failure(&self, row: PlanFailureCapture, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.plan_failures.push(row);
        inner.captured_tick = Some(tick);
    }

    /// Ticket 118 — flag the L3 momentum row with a substrate-driven
    /// preempt this tick. `emit_focal_trace` flows this into
    /// `MomentumSummary.preempted`. Setting twice in the same tick is
    /// idempotent (one preempt per cat per tick is the contract anyway).
    pub fn set_momentum_preempted(&self, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.momentum_preempted = true;
        inner.captured_tick = Some(tick);
    }

    /// Record a target-taking DSE's per-candidate ranking. Overwrites
    /// any prior ranking captured for the same DSE this tick — the
    /// final call wins, matching the cat's actual selection pick.
    pub fn set_target_ranking(&self, dse_id: &'static str, ranking: TargetRanking, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.target_rankings.insert(dse_id, ranking);
        inner.captured_tick = Some(tick);
    }

    /// Ticket 477 — record a resolver-level modifier read for the focal
    /// cat. Accumulates per tick; grouped by resolver name into
    /// `TraceRecord::L4Resolver` rows by `emit_focal_trace`. Callers
    /// should prefer [`FocalResolverSink::record`], which gates on the
    /// focal entity before reaching this write path.
    pub fn push_resolver_modifier(&self, row: ResolverModifierCapture, tick: u64) {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        inner.resolver_modifiers.push(row);
        inner.captured_tick = Some(tick);
    }

    /// Drain captured data for emission. Returns the inner state by
    /// value and resets the capture for the next tick.
    pub fn drain(&self) -> FocalScoreCaptureInner {
        let mut inner = self
            .inner
            .lock()
            .expect("focal score capture mutex poisoned");
        std::mem::take(&mut *inner)
    }
}

// ---------------------------------------------------------------------------
// FocalResolverSink — focal-gated resolver-trace handle (ticket 477)
// ---------------------------------------------------------------------------

/// Borrowed handle a system constructs once (when a focal cat is
/// resolved) and threads into the resolvers it calls. Resolvers receive
/// `Option<&FocalResolverSink>` and call [`record`](Self::record)
/// unconditionally on any non-trivial modifier read — the focal-cat gate
/// lives inside `record`, so a non-focal cat's call is a cheap no-op.
///
/// This is deliberately NOT routed through `NarrativeEmitter`: narrative
/// fires for every cat, whereas resolver-trace emission is focal-only
/// like the rest of the §11 trace surface. Bundling the capture handle +
/// focal entity + tick here keeps resolver signatures from widening into
/// three separate parameters.
pub struct FocalResolverSink<'a> {
    capture: &'a FocalScoreCapture,
    focal: Entity,
    tick: u64,
}

impl<'a> FocalResolverSink<'a> {
    /// Build a sink for the resolved focal cat. Returns `None` when the
    /// focal target hasn't resolved to an entity yet, so callers can
    /// pass `Option<&FocalResolverSink>` straight through.
    pub fn new(
        capture: Option<&'a FocalScoreCapture>,
        target: Option<&FocalTraceTarget>,
        tick: u64,
    ) -> Option<Self> {
        let capture = capture?;
        let focal = target?.entity?;
        Some(Self {
            capture,
            focal,
            tick,
        })
    }

    /// Record a modifier read iff `cat` is the focal cat. No-op
    /// otherwise — callers don't need to pre-check the focal identity.
    pub fn record(&self, cat: Entity, resolver: &'static str, modifier: &str, pre: f32, post: f32) {
        if cat != self.focal {
            return;
        }
        self.capture.push_resolver_modifier(
            ResolverModifierCapture {
                resolver,
                modifier: modifier.to_string(),
                pre,
                post,
            },
            self.tick,
        );
    }
}

/// Bundles the two focal-trace resources (ticket 477) into one
/// `SystemParam` so a system that wants to emit resolver-level trace
/// rows spends a single param slot instead of two — relevant because
/// equipment-aware systems like `resolve_combat` already sit at Bevy's
/// 16-param ceiling. Both resources are headless-only (`Option`), so the
/// param is a no-op in windowed/interactive runs.
#[derive(SystemParam)]
pub struct FocalTraceParam<'w> {
    pub capture: Option<Res<'w, FocalScoreCapture>>,
    pub target: Option<Res<'w, FocalTraceTarget>>,
}

impl FocalTraceParam<'_> {
    /// Construct a focal sink for `tick`, or `None` when tracing is off
    /// or the focal cat hasn't resolved to an entity yet.
    pub fn sink(&self, tick: u64) -> Option<FocalResolverSink<'_>> {
        FocalResolverSink::new(self.capture.as_deref(), self.target.as_deref(), tick)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::markers;

    fn make_l3() -> TraceRecord {
        TraceRecord::L3 {
            ranked: vec![("Hunt".into(), 0.72), ("Eat".into(), 0.68)],
            softmax: SoftmaxSummary {
                temperature: 0.15,
                probabilities: vec![0.58, 0.42],
            },
            momentum: MomentumSummary {
                active_intention: Some("Hunt".into()),
                commitment_strength: 0.6,
                margin_threshold: 0.09,
                preempted: false,
                held_dse: None,
                runner_up_margin: 0.0,
                decay_factor: 0.0,
            },
            chosen: "Hunt".into(),
            intention: IntentionSummary {
                kind: "Goal".into(),
                target: Some("Mouse#42".into()),
                goal_state: Some("prey_caught".into()),
            },
            goap_plan: vec!["MoveToTile(15,10)".into(), "PouncePrey(Mouse#42)".into()],
            pre_bonus_pool: Vec::new(),
            pre_penalty_pool: Vec::new(),
            apophenia: None,
        }
    }

    #[test]
    fn push_counts_entries() {
        let mut log = TraceLog::default();
        log.push(TraceEntry {
            tick: 1,
            cat: "Simba".into(),
            record: make_l3(),
        });
        log.push(TraceEntry {
            tick: 2,
            cat: "Simba".into(),
            record: make_l3(),
        });
        assert_eq!(log.total_pushed, 2);
        assert_eq!(log.entries.len(), 2);
    }

    #[test]
    fn ring_buffer_evicts_old() {
        let mut log = TraceLog::default();
        log.capacity = 3;
        for i in 0..5u64 {
            log.push(TraceEntry {
                tick: i,
                cat: "Simba".into(),
                record: make_l3(),
            });
        }
        assert_eq!(log.total_pushed, 5);
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].tick, 2);
    }

    #[test]
    fn l3_record_serializes_with_layer_tag() {
        let entry = TraceEntry {
            tick: 100,
            cat: "Simba".into(),
            record: make_l3(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"layer\":\"L3\""));
        assert!(json.contains("\"tick\":100"));
        assert!(json.contains("\"cat\":\"Simba\""));
        assert!(json.contains("\"chosen\":\"Hunt\""));
        // apophenia is None → field omitted
        assert!(!json.contains("apophenia"));
    }

    #[test]
    fn l1_record_serializes_with_attenuation() {
        let entry = TraceEntry {
            tick: 100,
            cat: "Simba".into(),
            record: TraceRecord::L1 {
                map: "fox_scent".into(),
                faction: "fox".into(),
                channel: "scent".into(),
                pos: (14, 9),
                base_sample: 0.42,
                attenuation: AttenuationBreakdown::default(),
                perceived: 0.42,
                top_contributors: vec![],
            },
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"layer\":\"L1\""));
        assert!(json.contains("\"species_sens\":1.0"));
    }

    #[test]
    fn l1_aspiration_record_serializes_with_emit_walk() {
        let entry = TraceEntry {
            tick: 42,
            cat: "Whiskers".into(),
            record: TraceRecord::L1Aspiration {
                aspiration: "hunting-mastery".into(),
                milestone: 2,
                emit_walk: vec![
                    EmitWalkRow {
                        label: "hunt_high_value_prey".into(),
                        applicable: true,
                        method_live: true,
                        emitted: true,
                    },
                    EmitWalkRow {
                        label: "hunt_patrol_domain".into(),
                        applicable: false,
                        method_live: true,
                        emitted: false,
                    },
                ],
                fallback_used: false,
            },
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"layer\":\"L1Aspiration\""));
        assert!(json.contains("\"aspiration\":\"hunting-mastery\""));
        assert!(json.contains("\"milestone\":2"));
        assert!(json.contains("\"hunt_high_value_prey\""));
        assert!(json.contains("\"method_live\":true"));
        assert!(json.contains("\"emitted\":true"));
        assert!(json.contains("\"fallback_used\":false"));
        // Two-row walk is present.
        assert!(json.contains("\"hunt_patrol_domain\""));
    }

    #[test]
    fn l1_aspiration_fallback_record_serializes() {
        // Verify the fallback_used flag round-trips correctly.
        let entry = TraceEntry {
            tick: 100,
            cat: "Simba".into(),
            record: TraceRecord::L1Aspiration {
                aspiration: "warrior-path".into(),
                milestone: 0,
                emit_walk: vec![EmitWalkRow {
                    label: "engage_threat".into(),
                    applicable: true,
                    method_live: true,
                    emitted: true,
                }],
                fallback_used: true,
            },
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"layer\":\"L1Aspiration\""));
        assert!(json.contains("\"fallback_used\":true"));
        assert!(json.contains("\"emitted\":true"));
    }

    #[test]
    fn focal_capture_accumulates_and_drains() {
        use crate::ai::dse::{ActivityKind, CommitmentStrategy, DseId, Intention, Termination};
        use crate::ai::eval::EvalTrace;

        let capture = FocalScoreCapture::default();
        let dummy_intention = Intention::Activity {
            kind: ActivityKind::Idle,
            termination: Termination::UntilInterrupt,
            strategy: CommitmentStrategy::OpenMinded,
        };

        capture.push_dse(
            CapturedDse {
                dse_id: DseId("eat"),
                raw_score: 0.4,
                gated_score: 0.3,
                final_score: 0.35,
                intention: dummy_intention.clone(),
                trace: EvalTrace::default(),
                eligibility_required: vec![markers::HasStoredFood::KEY],
                eligibility_forbidden: vec![],
                eligible: true,
            },
            42,
        );
        capture.push_dse(
            CapturedDse {
                dse_id: DseId("sleep"),
                raw_score: 0.2,
                gated_score: 0.2,
                final_score: 0.2,
                intention: dummy_intention,
                trace: EvalTrace::default(),
                eligibility_required: vec![],
                eligibility_forbidden: vec![],
                eligible: true,
            },
            42,
        );

        let drained = capture.drain();
        assert_eq!(drained.dses.len(), 2);
        assert_eq!(drained.captured_tick, Some(42));

        // Second drain is empty — the first drain reset the state.
        let drained = capture.drain();
        assert!(drained.dses.is_empty());
        assert!(drained.softmax.is_none());
    }
}
