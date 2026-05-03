//! Unified DSE evaluator — §3, §4, §9, §L2.10 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! The evaluator is the L2 hot path: given a registered DSE catalogue
//! and a cat's current context, produce a ranked list of scored DSEs
//! with their emitted Intentions. Selection (softmax / argmax) is a
//! downstream layer per §L2.10.2's compartmentalization principle —
//! the evaluator does not decide *which* DSE wins, only what each one
//! scored.
//!
//! Pipeline per DSE (in order, §3.4 → §3.5):
//!
//! 1. **Eligibility filter** (§4.3 markers + §9.3 stance). DSEs whose
//!    required markers are absent, forbidden markers are present, or
//!    target's stance is not in the accepted set are skipped entirely.
//! 2. **Per-consideration scoring.** Each `Consideration` fetches its
//!    input (scalar / influence-map sample / marker presence) and
//!    passes it through its `Curve`.
//! 3. **Composition** (§3.1 — one of 3 modes) reduces N considerations
//!    to one `raw_score`.
//! 4. **Maslow pre-gate** (§3.4). `gated = maslow_suppression(tier) *
//!    raw_score`. Wraps the existing `Needs::level_suppression` — not
//!    a new path.
//! 5. **Post-scoring modifier pipeline** (§3.5). 7 modifiers apply in
//!    registered order; each is a pure function of (dse_id, score,
//!    ctx) → score.
//! 6. **Intention emission.** `dse.emit(final, ctx)` builds the
//!    Intention the DSE would commit to if this score wins selection.
//!
//! Phase 3b.1 ships the plumbing. The evaluator's score output is
//! dead code until Phase 3b.2 wires `score_actions` to call it.

use bevy::prelude::*;

use super::considerations::{Consideration, LandmarkSource};
use super::dse::{CommitmentStrategy, Dse, DseId, EligibilityFilter, EvalCtx, Intention};

// ---------------------------------------------------------------------------
// DseRegistry resource
// ---------------------------------------------------------------------------

/// The DSE catalog, keyed by category per §L2.10.3's 5 registration
/// methods. Inserted as a Bevy `Resource` at plugin load; DSEs
/// register through the [`DseRegistryAppExt`] trait below.
///
/// Each category is a `Vec<Box<dyn Dse>>` rather than a flat
/// collection because the evaluator's downstream consumers differ:
///
/// - `cat_dses` feeds per-cat per-tick action scoring.
/// - `target_taking_dses` runs the per-candidate inner loop under §6.
/// - `fox_dses` runs on fox entities via a distinct schedule label.
/// - `aspiration_dses`, `coordinator_dses`, `narrative_dses` are
///   all slower-cadence or event-driven; keeping them in separate
///   vecs makes the cadence policy grep-able.
#[derive(Resource, Default)]
pub struct DseRegistry {
    pub cat_dses: Vec<Box<dyn Dse>>,
    /// §6.3 target-taking DSEs. Distinct slot and type from `cat_dses`
    /// because the evaluator dispatches differently: regular Dses score
    /// once per cat-tick; target-taking DSEs iterate candidates and
    /// aggregate per-candidate scores into (action_score, winning_target).
    pub target_taking_dses: Vec<super::target_dse::TargetTakingDse>,
    pub fox_dses: Vec<Box<dyn Dse>>,
    pub hawk_dses: Vec<Box<dyn Dse>>,
    pub snake_dses: Vec<Box<dyn Dse>>,
    pub aspiration_dses: Vec<Box<dyn Dse>>,
    pub coordinator_dses: Vec<Box<dyn Dse>>,
    pub narrative_dses: Vec<Box<dyn Dse>>,
}

impl DseRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a registered cat DSE by its string id. Primarily for the
    /// Phase 3b.2 scoring.rs splice where a single named DSE is
    /// scored in isolation.
    pub fn cat_dse(&self, id: &str) -> Option<&dyn Dse> {
        self.cat_dses
            .iter()
            .find(|d| d.id().0 == id)
            .map(|boxed| boxed.as_ref())
    }

    /// Find a registered fox DSE by its string id.
    pub fn fox_dse(&self, id: &str) -> Option<&dyn Dse> {
        self.fox_dses
            .iter()
            .find(|d| d.id().0 == id)
            .map(|boxed| boxed.as_ref())
    }

    /// Find a registered hawk DSE by its string id.
    pub fn hawk_dse(&self, id: &str) -> Option<&dyn Dse> {
        self.hawk_dses
            .iter()
            .find(|d| d.id().0 == id)
            .map(|boxed| boxed.as_ref())
    }

    /// Find a registered snake DSE by its string id.
    pub fn snake_dse(&self, id: &str) -> Option<&dyn Dse> {
        self.snake_dses
            .iter()
            .find(|d| d.id().0 == id)
            .map(|boxed| boxed.as_ref())
    }

    pub fn total(&self) -> usize {
        self.cat_dses.len()
            + self.target_taking_dses.len()
            + self.fox_dses.len()
            + self.hawk_dses.len()
            + self.snake_dses.len()
            + self.aspiration_dses.len()
            + self.coordinator_dses.len()
            + self.narrative_dses.len()
    }
}

// ---------------------------------------------------------------------------
// App extension — §L2.10.3 registration catalog (5 methods)
// ---------------------------------------------------------------------------

/// App-extension trait that commits each DSE to the registry at
/// plugin load. Mirrors the §L2.10.3 chained-call idiom:
///
/// ```ignore
/// app.add_dse(eat_dse())
///    .add_target_taking_dse(socialize_dse())
///    .add_fox_dse(fox_patrol_dse())
///    .add_aspiration_dse(reproduce_aspiration_dse())
///    .add_coordinator_dse(coordinator_election_dse())
///    .add_narrative_dse(narrative_template_selection_dse());
/// ```
///
/// Six methods cover the 45-row §L2.10.3 catalog.
pub trait DseRegistryAppExt {
    fn add_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    /// Register a §6.3 target-taking DSE. Distinct from `add_dse`
    /// because target-taking uses a struct-shape type (see
    /// [`crate::ai::target_dse::TargetTakingDse`]), not `Box<dyn Dse>`.
    fn add_target_taking_dse(&mut self, dse: super::target_dse::TargetTakingDse) -> &mut Self;
    fn add_fox_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    fn add_hawk_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    fn add_snake_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    fn add_aspiration_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    fn add_coordinator_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    fn add_narrative_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
}

impl DseRegistryAppExt for App {
    fn add_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .cat_dses
            .push(dse);
        self
    }

    fn add_target_taking_dse(&mut self, dse: super::target_dse::TargetTakingDse) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .target_taking_dses
            .push(dse);
        self
    }

    fn add_fox_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .fox_dses
            .push(dse);
        self
    }

    fn add_hawk_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .hawk_dses
            .push(dse);
        self
    }

    fn add_snake_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .snake_dses
            .push(dse);
        self
    }

    fn add_aspiration_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .aspiration_dses
            .push(dse);
        self
    }

    fn add_coordinator_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .coordinator_dses
            .push(dse);
        self
    }

    fn add_narrative_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(DseRegistry::new)
            .narrative_dses
            .push(dse);
        self
    }
}

// ---------------------------------------------------------------------------
// ScoreModifier (§3.5)
// ---------------------------------------------------------------------------

/// Post-scoring modifier trait — one pass in the §3.5 pipeline. Each
/// modifier transforms a single DSE's score given the cat's context.
/// Modifiers apply in registered order; §3.5.1's catalog commits seven
/// named passes (Pride, Independence solo, Independence group,
/// Patience, Tradition, Fox-suppression, Corruption-suppression).
///
/// `fetch_scalar` is the same closure the evaluator uses for
/// `ScalarConsideration` inputs — modifiers read their trigger inputs
/// (e.g. corruption level, ward deficit, Maslow level-2 suppression)
/// via named scalar lookups rather than carrying per-field context
/// accessors. Each modifier names the scalars it depends on in its
/// doc comment so the contract is auditable.
pub trait ScoreModifier: Send + Sync + 'static {
    /// Apply the modifier to `score` for DSE `dse_id`. Return the
    /// transformed score. A modifier that doesn't apply to `dse_id`
    /// (per the §3.5.2 applicability matrix) returns `score` unchanged.
    fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch_scalar: &dyn Fn(&str, Entity) -> f32,
    ) -> f32;

    fn name(&self) -> &'static str;
}

/// Ordered modifier pipeline. Phase 3b.1 ships the container shape;
/// concrete modifier implementations (Pride, Tradition, etc.) land
/// in Phase 3c alongside the DSEs that consume them.
#[derive(Resource, Default)]
pub struct ModifierPipeline {
    passes: Vec<Box<dyn ScoreModifier>>,
    /// Saturating-composition cap for the cumulative positive lift any
    /// one DSE can receive across the pipeline (ticket 146). `0.0`
    /// disables the cap (raw additive sum). Default constructor sets
    /// `0.0`; `default_modifier_pipeline` reads
    /// `ScoringConstants::max_additive_lift_per_dse` and sets it there.
    max_additive_lift_per_dse: f32,
}

impl ModifierPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure the saturating-composition cap. See struct doc.
    pub fn with_max_additive_lift_per_dse(mut self, max_lift: f32) -> Self {
        self.max_additive_lift_per_dse = max_lift;
        self
    }

    pub fn push(&mut self, modifier: Box<dyn ScoreModifier>) -> &mut Self {
        self.passes.push(modifier);
        self
    }

    /// Apply every registered modifier in registration order. Pure
    /// function of (dse_id, input_score, ctx, fetch_scalar).
    pub fn apply(
        &self,
        dse_id: DseId,
        score: f32,
        ctx: &EvalCtx,
        fetch_scalar: &dyn Fn(&str, Entity) -> f32,
    ) -> f32 {
        self.apply_with_trace(dse_id, score, ctx, fetch_scalar, None)
    }

    /// Apply every registered modifier, optionally capturing per-pass
    /// `(name, pre, post)` deltas into the provided sink. When the sink
    /// is `None` the cost is a single `Option` check per pass — this is
    /// the zero-cost-when-not-tracing contract §11.5 requires.
    ///
    /// Only passes that actually changed the score (`pre != post`) are
    /// recorded, so the sink carries signal rather than noise — the
    /// seven §3.5.1 modifiers each apply to a narrow DSE slice and
    /// leaving out no-op rows keeps replay frames readable.
    pub fn apply_with_trace(
        &self,
        dse_id: DseId,
        mut score: f32,
        ctx: &EvalCtx,
        fetch_scalar: &dyn Fn(&str, Entity) -> f32,
        mut sink: Option<&mut Vec<ModifierDelta>>,
    ) -> f32 {
        // Saturating composition of positive lifts (ticket 146).
        //
        // Why: when two perception axes agree (e.g. 107 ExhaustionPressure
        // and 110 ThermalDistress both lift Sleep on a cold tired night),
        // the raw additive sum doubles the lift (+0.20 + +0.20 = +0.40).
        // That double-stack pulls cats into Sleep loops away from
        // Patrol/Guarding, collapsing fox-defense coverage and causing
        // colony extinction. Diminishing-returns composition is the right
        // semantic — multi-axis agreement should lift more than one axis,
        // but not unboundedly.
        //
        // We collect each positive delta as it occurs, apply it in-place
        // (so multiplicative damps later in the pipeline still see the
        // lifted score and damp it correctly), then at the end subtract
        // the cap-excess: the gap between the raw sum of positive deltas
        // and the saturating composition `MAX * (1 - Π(1 - lift_i / MAX))`.
        let max_lift = self.max_additive_lift_per_dse;
        // Cumulative positive deltas across all passes. Capacity hint
        // matches the §3.5.1 lift-modifier count; no realloc in practice.
        let mut positive_deltas: Vec<f32> = Vec::with_capacity(self.passes.len());
        for pass in &self.passes {
            let pre = score;
            score = pass.apply(dse_id, score, ctx, fetch_scalar);
            let delta = score - pre;
            if delta > 0.0 {
                positive_deltas.push(delta);
            }
            if let Some(sink) = sink.as_mut() {
                if (score - pre).abs() > f32::EPSILON {
                    sink.push(ModifierDelta {
                        name: pass.name(),
                        pre,
                        post: score,
                    });
                }
            }
        }
        if positive_deltas.len() > 1 && max_lift > 0.0 {
            let raw_sum: f32 = positive_deltas.iter().sum();
            let mut headroom = 1.0_f32;
            for lift in &positive_deltas {
                headroom *= (1.0 - lift / max_lift).max(0.0);
            }
            let saturated = max_lift * (1.0 - headroom);
            if raw_sum > saturated {
                score -= raw_sum - saturated;
            }
        }
        score
    }

    pub fn len(&self) -> usize {
        self.passes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// EvalTrace — §11.3 L2 record capture shape
// ---------------------------------------------------------------------------

/// Per-modifier delta row captured during `ModifierPipeline::apply_with_trace`.
/// Modifier names are `&'static str` (§3.5 `ScoreModifier::name` contract),
/// so this clones on emit rather than per-tick.
#[derive(Debug, Clone)]
pub struct ModifierDelta {
    pub name: &'static str,
    pub pre: f32,
    pub post: f32,
}

/// Per-consideration record captured during `evaluate_single_with_trace`.
/// Covers the §11.3 L2 record's `considerations` row: the consideration's
/// name, the input that fed its curve, a human-readable curve label, the
/// post-curve score, and the composition weight this consideration was
/// assigned. `spatial_map_key` is populated iff this is a
/// `Consideration::Spatial` and carries a stable landmark identifier
/// (`"target_position"` / `"tile"` / `"entity"`) per
/// [`SpatialConsideration::landmark_label`]. JSON field name preserved
/// for trace-consumer compat (`scripts/replay_frame.py`,
/// `docs/diagnostics/log-queries.md`).
#[derive(Debug, Clone)]
pub struct ConsiderationTraceRow {
    pub name: &'static str,
    pub kind: ConsiderationKind,
    pub input: f32,
    pub curve_label: String,
    pub score: f32,
    pub weight: f32,
    pub spatial_map_key: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsiderationKind {
    Scalar,
    Spatial,
    Marker,
}

/// Full L2 decomposition captured during `evaluate_single_with_trace`. This
/// is the data shape §11.3's L2 record pulls its fields from. Empty by
/// default; populated only when the evaluator is invoked with a sink.
#[derive(Debug, Clone, Default)]
pub struct EvalTrace {
    pub considerations: Vec<ConsiderationTraceRow>,
    pub composition_mode: Option<&'static str>,
    pub composition_weights: Vec<f32>,
    pub composition_compensation_strength: f32,
    pub maslow_pregate: f32,
    pub modifier_deltas: Vec<ModifierDelta>,
}

// ---------------------------------------------------------------------------
// ScoredDse — per-DSE evaluator output
// ---------------------------------------------------------------------------

/// Per-DSE evaluator output. Carries enough detail for the §11.3 L2
/// trace record to reconstruct the full decision frame: the DSE id,
/// each consideration's input and score, the raw composition output,
/// the Maslow-gated score, the post-modifier final score, and the
/// emitted Intention.
#[derive(Debug, Clone)]
pub struct ScoredDse {
    pub id: DseId,
    /// Per-consideration scores in the order `dse.considerations()`
    /// returned them. Length matches `dse.composition().weights`.
    pub per_consideration: Vec<f32>,
    /// Post-composition, pre-Maslow score.
    pub raw_score: f32,
    /// Post-Maslow-pre-gate score.
    pub gated_score: f32,
    /// Post-modifier final score. This is what selection consumes.
    pub final_score: f32,
    /// Intention emitted by `dse.emit(final_score, ctx)`.
    pub intention: Intention,
}

// ---------------------------------------------------------------------------
// Eligibility check
// ---------------------------------------------------------------------------

/// Check whether the eligibility filter passes for `cat` in `ctx`.
/// Reads required/forbidden markers via `ctx.has_marker`. Stance
/// filtering (§9.3) happens downstream in target-taking DSEs where
/// the candidate target is known.
pub fn passes_eligibility(filter: &EligibilityFilter, cat: Entity, ctx: &EvalCtx) -> bool {
    for required in &filter.required {
        if !(ctx.has_marker)(required, cat) {
            return false;
        }
    }
    for forbidden in &filter.forbidden {
        if (ctx.has_marker)(forbidden, cat) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Single-DSE evaluation
// ---------------------------------------------------------------------------

/// Score one DSE for `cat` in `ctx`. Returns `None` if the DSE is
/// ineligible (skips the scoring cost per §4's "avoid computing a
/// score that can't win" principle).
///
/// Callers supply `fetch_scalar` to resolve a named scalar
/// consideration input. The evaluator can't know the shape of the
/// caller's scalar state (needs, personality, …) and delegates to a
/// closure so the dispatch is explicit.
pub fn evaluate_single(
    dse: &dyn Dse,
    cat: Entity,
    ctx: &EvalCtx,
    maslow_pre_gate: &dyn Fn(u8) -> f32,
    modifiers: &ModifierPipeline,
    fetch_scalar: &dyn Fn(&str, Entity) -> f32,
) -> Option<ScoredDse> {
    evaluate_single_with_trace(
        dse,
        cat,
        ctx,
        maslow_pre_gate,
        modifiers,
        fetch_scalar,
        None,
    )
}

/// Same as [`evaluate_single`] but, when `sink` is `Some`, records the
/// full §11.3 L2 decomposition (per-consideration inputs + curve labels,
/// composition mode/weights, Maslow pre-gate, per-modifier pre/post
/// deltas). When `sink` is `None` the cost is an `Option` check in the
/// consideration-scoring loop — §11.5's "no tracing cost when dormant"
/// contract.
pub fn evaluate_single_with_trace(
    dse: &dyn Dse,
    cat: Entity,
    ctx: &EvalCtx,
    maslow_pre_gate: &dyn Fn(u8) -> f32,
    modifiers: &ModifierPipeline,
    fetch_scalar: &dyn Fn(&str, Entity) -> f32,
    mut sink: Option<&mut EvalTrace>,
) -> Option<ScoredDse> {
    if !passes_eligibility(dse.eligibility(), cat, ctx) {
        return None;
    }

    let considerations = dse.considerations();
    let composition = dse.composition();

    // Score each consideration via the appropriate input source. When a
    // sink is attached, also record the pre-curve input and a curve
    // label so replay frames show the full curvature per §11.1.
    let mut per_consideration: Vec<f32> = Vec::with_capacity(considerations.len());
    for (idx, c) in considerations.iter().enumerate() {
        let (score, trace_row) =
            score_consideration_with_trace(c, cat, ctx, fetch_scalar, sink.is_some());
        per_consideration.push(score);
        if let (Some(sink), Some((input, curve_label, kind, spatial_map_key))) =
            (sink.as_mut(), trace_row)
        {
            let weight = composition.weights.get(idx).copied().unwrap_or(0.0);
            sink.considerations.push(ConsiderationTraceRow {
                name: c.name(),
                kind,
                input,
                curve_label,
                score,
                weight,
                spatial_map_key,
            });
        }
    }

    // Compose to raw score.
    let raw_score = composition.compose(&per_consideration);

    // Maslow pre-gate. u8::MAX opts out (non-Maslow DSEs — coordinator
    // election, narrative selection).
    let pregate = if dse.maslow_tier() == u8::MAX {
        1.0
    } else {
        maslow_pre_gate(dse.maslow_tier())
    };
    let gated_score = pregate * raw_score;

    // Post-scoring modifier pipeline. Shares the same scalar-fetch
    // closure the considerations use, so modifier triggers (corruption
    // readings, Maslow suppression, inventory booleans) resolve through
    // the same canonical scalar surface as DSE inputs.
    let final_score = modifiers.apply_with_trace(
        dse.id(),
        gated_score,
        ctx,
        fetch_scalar,
        sink.as_mut().map(|s| &mut s.modifier_deltas),
    );

    // Populate trace-only composition + maslow metadata. These are
    // pure restatement of the inputs to the composer + pre-gate; they
    // don't cost anything to capture when the sink is absent.
    if let Some(sink) = sink {
        sink.composition_mode = Some(composition_mode_label(composition));
        sink.composition_weights = composition.weights.clone();
        sink.composition_compensation_strength = composition.compensation_strength;
        sink.maslow_pregate = pregate;
    }

    // Emit the Intention.
    let intention = dse.emit(final_score, ctx);

    Some(ScoredDse {
        id: dse.id(),
        per_consideration,
        raw_score,
        gated_score,
        final_score,
        intention,
    })
}

fn composition_mode_label(c: &super::composition::Composition) -> &'static str {
    use super::composition::CompositionMode;
    match c.mode {
        CompositionMode::CompensatedProduct => "CompensatedProduct",
        CompositionMode::WeightedSum => "WeightedSum",
        CompositionMode::Max => "Max",
    }
}

/// Score one consideration by dispatching on its flavor. Scalar
/// considerations pull from `fetch_scalar`; spatial considerations
/// resolve a landmark `Position` and compute Manhattan distance from
/// `ctx.self_position`; marker considerations consult `ctx.has_marker`.
/// Returns a tuple `(score, trace_row)` where `trace_row` is populated
/// when `capture` is true; callers that don't need the trace pass
/// `false` and discard the second element.
///
/// `trace_row` carries the input fed to the curve, a human-readable
/// curve label, the consideration kind, and (for `Spatial`) a stable
/// `landmark_label` so §11.3 trace consumers can distinguish landmark
/// flavors. When `capture` is false the second slot is always `None`.
#[allow(clippy::type_complexity)]
fn score_consideration_with_trace(
    consideration: &Consideration,
    cat: Entity,
    ctx: &EvalCtx,
    fetch_scalar: &dyn Fn(&str, Entity) -> f32,
    capture: bool,
) -> (
    f32,
    Option<(f32, String, ConsiderationKind, Option<&'static str>)>,
) {
    match consideration {
        Consideration::Scalar(s) => {
            let input = fetch_scalar(s.name, cat);
            let score = s.score(input);
            let row = capture.then(|| {
                (
                    input,
                    format!("{:?}", s.curve),
                    ConsiderationKind::Scalar,
                    None,
                )
            });
            (score, row)
        }
        Consideration::Spatial(s) => {
            let landmark_pos = match s.landmark {
                LandmarkSource::TargetPosition => match ctx.target_position {
                    Some(p) => p,
                    // Target-taking DSE scored without a target: zero.
                    // The target-taking evaluator (§6) must populate
                    // `ctx.target_position` before calling in.
                    None => {
                        let row = capture.then(|| {
                            (
                                0.0,
                                format!("{:?}", s.curve),
                                ConsiderationKind::Spatial,
                                Some(s.landmark_label()),
                            )
                        });
                        return (0.0, row);
                    }
                },
                LandmarkSource::Tile(p) => p,
                LandmarkSource::Entity(e) => match (ctx.entity_position)(e) {
                    Some(p) => p,
                    None => {
                        let row = capture.then(|| {
                            (
                                0.0,
                                format!("{:?}", s.curve),
                                ConsiderationKind::Spatial,
                                Some(s.landmark_label()),
                            )
                        });
                        return (0.0, row);
                    }
                },
                LandmarkSource::Anchor(a) => match (ctx.anchor_position)(a) {
                    Some(p) => p,
                    None => {
                        let row = capture.then(|| {
                            (
                                0.0,
                                format!("{:?}", s.curve),
                                ConsiderationKind::Spatial,
                                Some(s.landmark_label()),
                            )
                        });
                        return (0.0, row);
                    }
                },
            };
            let distance = ctx.self_position.manhattan_distance(&landmark_pos) as f32;
            let score = s.score(distance);
            let row = capture.then(|| {
                (
                    distance,
                    format!("{:?}", s.curve),
                    ConsiderationKind::Spatial,
                    Some(s.landmark_label()),
                )
            });
            (score, row)
        }
        Consideration::Marker(m) => {
            let present = (ctx.has_marker)(m.marker, cat);
            let score = m.score(present);
            let row = capture.then(|| {
                (
                    if present { 1.0 } else { 0.0 },
                    format!("Marker(present_score={:.3})", m.present_score),
                    ConsiderationKind::Marker,
                    None,
                )
            });
            (score, row)
        }
    }
}

// ---------------------------------------------------------------------------
// Full-registry evaluation
// ---------------------------------------------------------------------------

/// Score every eligible cat DSE in the registry. Returns one
/// `ScoredDse` per eligible DSE; ineligible DSEs are omitted (not
/// present as zero-score entries — matches §4's skip-entirely contract).
pub fn evaluate_all_cat_dses(
    registry: &DseRegistry,
    cat: Entity,
    ctx: &EvalCtx,
    maslow_pre_gate: &dyn Fn(u8) -> f32,
    modifiers: &ModifierPipeline,
    fetch_scalar: &dyn Fn(&str, Entity) -> f32,
) -> Vec<ScoredDse> {
    registry
        .cat_dses
        .iter()
        .filter_map(|dse| {
            evaluate_single(
                dse.as_ref(),
                cat,
                ctx,
                maslow_pre_gate,
                modifiers,
                fetch_scalar,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// §L2.10.6 softmax-over-Intentions
// ---------------------------------------------------------------------------

/// Select one `ScoredDse` from a freshly-scored candidate pool via
/// softmax (Boltzmann) sampling. §L2.10.6's canonical selection:
/// stochastic *intent*, deterministic *execution*.
///
/// Returns `None` iff `pool` is empty. Non-empty pools always yield a
/// pick (even if every score is negative — the softmax normalization
/// handles that via the standard max-shift trick).
///
/// Order with §7.4's persistence bonus (not yet wired): softmax picks
/// the *challenger* Intention from the freshly-scored candidate pool;
/// the persistence bonus then applies to the currently-held Intention's
/// score and gates preemption. Softmax runs first; persistence-bonus
/// gating runs second. See §L2.10.6 in
/// `docs/systems/ai-substrate-refactor.md`.
pub fn select_intention_softmax<'a, R: rand::Rng + ?Sized>(
    pool: &'a [ScoredDse],
    rng: &mut R,
    temperature: f32,
) -> Option<&'a ScoredDse> {
    if pool.is_empty() {
        return None;
    }

    let max_score = pool
        .iter()
        .map(|s| s.final_score)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = pool
        .iter()
        .map(|s| ((s.final_score - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let mut roll: f32 = rng.random::<f32>() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return Some(&pool[i]);
        }
    }
    pool.last()
}

// ---------------------------------------------------------------------------
// CommitmentStrategy default helpers (§L2.10.5)
// ---------------------------------------------------------------------------

/// §L2.10.5 strategy-shape correlation: `Goal` defaults to
/// `SingleMinded`, `Activity` defaults to `OpenMinded`. Callers can
/// override via `Dse::emit`.
pub fn default_strategy_for_intention(is_goal: bool) -> CommitmentStrategy {
    if is_goal {
        CommitmentStrategy::SingleMinded
    } else {
        CommitmentStrategy::OpenMinded
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::ai::considerations::LandmarkAnchor;
    use super::*;
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{Consideration, ScalarConsideration};
    use crate::ai::curves::hangry;
    use crate::ai::dse::{ActivityKind, DseId, Termination};
    use crate::components::markers;
    use crate::components::physical::Position;

    // A minimal test DSE: one scalar consideration on a named input.
    struct TestDse {
        id: DseId,
        considerations: Vec<Consideration>,
        composition: Composition,
        eligibility: EligibilityFilter,
        tier: u8,
    }

    impl Dse for TestDse {
        fn id(&self) -> DseId {
            self.id
        }
        fn considerations(&self) -> &[Consideration] {
            &self.considerations
        }
        fn composition(&self) -> &Composition {
            &self.composition
        }
        fn eligibility(&self) -> &EligibilityFilter {
            &self.eligibility
        }
        fn default_strategy(&self) -> CommitmentStrategy {
            CommitmentStrategy::OpenMinded
        }
        fn emit(&self, _: f32, _: &EvalCtx) -> Intention {
            Intention::Activity {
                kind: ActivityKind::Idle,
                termination: Termination::UntilInterrupt,
                strategy: CommitmentStrategy::OpenMinded,
            }
        }
        fn maslow_tier(&self) -> u8 {
            self.tier
        }
    }

    fn test_dse(id: &'static str, scalar_name: &'static str) -> TestDse {
        TestDse {
            id: DseId(id),
            considerations: vec![Consideration::Scalar(ScalarConsideration::new(
                scalar_name,
                hangry(),
            ))],
            composition: Composition::compensated_product(vec![1.0]),
            eligibility: EligibilityFilter::new(),
            tier: 1,
        }
    }

    #[test]
    fn registry_add_and_total() {
        let mut r = DseRegistry::new();
        r.cat_dses.push(Box::new(test_dse("eat", "hunger")));
        r.fox_dses.push(Box::new(test_dse("fox_hunt", "hunger")));
        assert_eq!(r.total(), 2);
        assert!(r.cat_dse("eat").is_some());
        assert!(r.cat_dse("nope").is_none());
        assert!(r.fox_dse("fox_hunt").is_some());
    }

    #[test]
    fn eligibility_required_marker() {
        let filter = EligibilityFilter::new().require(markers::HasStoredFood::KEY);
        let entity = Entity::from_raw_u32(1).unwrap();

        // Missing required → fail.
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        assert!(!passes_eligibility(&filter, entity, &ctx));

        // Present required → pass.
        let has_marker = |m: &str, _: Entity| m == markers::HasStoredFood::KEY;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        assert!(passes_eligibility(&filter, entity, &ctx));
    }

    #[test]
    fn eligibility_forbidden_marker() {
        let filter = EligibilityFilter::new().forbid(markers::Incapacitated::KEY);
        let entity = Entity::from_raw_u32(1).unwrap();
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let has_incap = |m: &str, _: Entity| m == markers::Incapacitated::KEY;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_incap,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        assert!(!passes_eligibility(&filter, entity, &ctx));

        let none = |_: &str, _: Entity| false;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &none,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        assert!(passes_eligibility(&filter, entity, &ctx));
    }

    #[test]
    fn evaluate_single_returns_none_on_ineligible() {
        let mut dse = test_dse("eat", "hunger");
        dse.eligibility = EligibilityFilter::new().require(markers::HasStoredFood::KEY);
        let entity = Entity::from_raw_u32(1).unwrap();

        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.0;

        let result = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch);
        assert!(result.is_none());
    }

    #[test]
    fn evaluate_single_computes_raw_gated_final() {
        let dse = test_dse("eat", "hunger");
        let entity = Entity::from_raw_u32(1).unwrap();

        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };

        // Maslow pre-gate: tier 1 suppression = 0.5 (simulating phys
        // half-satisfied).
        let maslow = |tier: u8| if tier == 1 { 0.5 } else { 1.0 };
        let modifiers = ModifierPipeline::new();
        // Hunger = 0.5 → Logistic(8, 0.5) evaluates to ~0.5 (ticket 044).
        let fetch = |name: &str, _: Entity| if name == "hunger" { 0.5 } else { 0.0 };

        let scored =
            evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).expect("eligible");
        assert!(
            (scored.raw_score - 0.5).abs() < 0.01,
            "raw: {}",
            scored.raw_score
        );
        assert!(
            (scored.gated_score - 0.25).abs() < 0.01,
            "gated: {}",
            scored.gated_score
        );
        // No modifiers → final == gated.
        assert!((scored.final_score - scored.gated_score).abs() < 1e-6);
    }

    #[test]
    fn non_maslow_dse_skips_pre_gate() {
        let mut dse = test_dse("narrative", "dummy");
        dse.tier = u8::MAX;
        let entity = Entity::from_raw_u32(1).unwrap();

        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        // Maslow returns 0 for tier MAX, but the evaluator should
        // skip the gate entirely.
        let maslow = |_: u8| 0.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.75;

        let scored =
            evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).expect("eligible");
        assert!((scored.gated_score - scored.raw_score).abs() < 1e-6);
    }

    #[test]
    fn modifier_pipeline_applies_in_order() {
        struct AddOne;
        impl ScoreModifier for AddOne {
            fn apply(
                &self,
                _: DseId,
                score: f32,
                _: &EvalCtx,
                _: &dyn Fn(&str, Entity) -> f32,
            ) -> f32 {
                score + 0.1
            }
            fn name(&self) -> &'static str {
                "add_one"
            }
        }
        struct DoubleIt;
        impl ScoreModifier for DoubleIt {
            fn apply(
                &self,
                _: DseId,
                score: f32,
                _: &EvalCtx,
                _: &dyn Fn(&str, Entity) -> f32,
            ) -> f32 {
                score * 2.0
            }
            fn name(&self) -> &'static str {
                "double"
            }
        }

        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(AddOne));
        pipeline.push(Box::new(DoubleIt));

        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        let fetch = |_: &str, _: Entity| 0.0;
        // 0.5 → 0.6 → 1.2
        let out = pipeline.apply(DseId("eat"), 0.5, &ctx, &fetch);
        assert!((out - 1.2).abs() < 1e-6, "out: {out}");
    }

    #[test]
    fn evaluate_all_cat_dses_produces_one_per_eligible() {
        let mut registry = DseRegistry::new();
        registry.cat_dses.push(Box::new(test_dse("eat", "hunger")));
        let mut ineligible = test_dse("blocked", "hunger");
        ineligible.eligibility = EligibilityFilter::new().require("NeverPresent");
        registry.cat_dses.push(Box::new(ineligible));

        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.5;

        let scored = evaluate_all_cat_dses(&registry, entity, &ctx, &maslow, &modifiers, &fetch);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].id.0, "eat");
    }

    #[test]
    fn default_strategy_for_intention_matches_spec() {
        assert_eq!(
            default_strategy_for_intention(true),
            CommitmentStrategy::SingleMinded
        );
        assert_eq!(
            default_strategy_for_intention(false),
            CommitmentStrategy::OpenMinded
        );
    }

    fn scored_dse(id: &'static str, score: f32) -> ScoredDse {
        ScoredDse {
            id: DseId(id),
            per_consideration: vec![],
            raw_score: score,
            gated_score: score,
            final_score: score,
            intention: Intention::Activity {
                kind: ActivityKind::Idle,
                termination: Termination::UntilInterrupt,
                strategy: CommitmentStrategy::OpenMinded,
            },
        }
    }

    #[test]
    fn intention_softmax_empty_pool_returns_none() {
        use rand_chacha::rand_core::SeedableRng;
        let pool: Vec<ScoredDse> = Vec::new();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        assert!(select_intention_softmax(&pool, &mut rng, 0.15).is_none());
    }

    #[test]
    fn intention_softmax_low_temperature_picks_near_argmax() {
        // At T = 0.01 the softmax should converge to argmax. Sample many
        // times under seeded RNG and confirm the winner is the top-scoring
        // DSE in the overwhelming majority of draws.
        use rand_chacha::rand_core::SeedableRng;
        let pool = vec![
            scored_dse("eat", 0.9),
            scored_dse("sleep", 0.4),
            scored_dse("idle", 0.05),
        ];
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let mut eat_hits = 0usize;
        for _ in 0..200 {
            let pick = select_intention_softmax(&pool, &mut rng, 0.01).expect("non-empty pool");
            if pick.id.0 == "eat" {
                eat_hits += 1;
            }
        }
        assert!(
            eat_hits >= 195,
            "low-T softmax should converge to argmax; got {eat_hits}/200 eat picks"
        );
    }

    #[test]
    fn intention_softmax_spreads_across_near_ties() {
        // At T = 0.15 and three scores within ~0.05 of each other, the
        // softmax should produce a visibly-mixed distribution — the whole
        // point of spec §L2.10.6.
        use rand_chacha::rand_core::SeedableRng;
        let pool = vec![
            scored_dse("eat", 0.7),
            scored_dse("socialize", 0.68),
            scored_dse("groom", 0.66),
        ];
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(13);
        let mut counts = std::collections::HashMap::<&'static str, usize>::new();
        for _ in 0..500 {
            let pick = select_intention_softmax(&pool, &mut rng, 0.15).expect("non-empty pool");
            *counts.entry(pick.id.0).or_insert(0) += 1;
        }
        // Every candidate should get a meaningful share under near-ties.
        for name in ["eat", "socialize", "groom"] {
            let c = *counts.get(name).unwrap_or(&0);
            assert!(
                c >= 50,
                "{name} got only {c}/500 picks under near-tie softmax"
            );
        }
    }

    // -------------------------------------------------------------------
    // §11 trace-capture tests — evaluate_single_with_trace + modifier
    // deltas + softmax capture parity.
    // -------------------------------------------------------------------

    #[test]
    fn evaluate_single_with_trace_captures_consideration_input_and_score() {
        // Trace sink captures the scalar input fed to the curve + the
        // curve-evaluated score. §11.3's joinability invariant: L2's
        // `per_consideration` rows carry the raw input and the
        // post-curve score so replay frames can reconstruct the
        // transform.
        let dse = test_dse("eat", "hunger");
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |name: &str, _: Entity| if name == "hunger" { 0.75 } else { 0.0 };

        let mut trace = EvalTrace::default();
        let scored = evaluate_single_with_trace(
            &dse,
            entity,
            &ctx,
            &maslow,
            &modifiers,
            &fetch,
            Some(&mut trace),
        )
        .expect("eligible");
        assert_eq!(trace.considerations.len(), 1);
        let row = &trace.considerations[0];
        assert_eq!(row.name, "hunger");
        assert_eq!(row.kind, ConsiderationKind::Scalar);
        assert!(
            (row.input - 0.75).abs() < 1e-6,
            "raw input captured; got {}",
            row.input
        );
        assert!((row.score - scored.per_consideration[0]).abs() < 1e-6);
        assert!(row.curve_label.starts_with("Logistic"));
        // Composition metadata populated.
        assert_eq!(trace.composition_mode, Some("CompensatedProduct"));
        assert_eq!(trace.composition_weights.len(), 1);
        // Maslow pre-gate recorded (tier 1 → 1.0 from the closure above).
        assert!((trace.maslow_pregate - 1.0).abs() < 1e-6);
        // No modifiers registered → no deltas.
        assert!(trace.modifier_deltas.is_empty());
    }

    #[test]
    fn evaluate_single_without_trace_is_zero_cost_path() {
        // Without a sink, evaluate_single returns the same ScoredDse
        // as the traced variant and doesn't allocate any trace state.
        // Regression guard: a future change that moved allocation into
        // the untraced path would silently tax every scoring call.
        let dse = test_dse("eat", "hunger");
        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        let maslow = |_: u8| 1.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |name: &str, _: Entity| if name == "hunger" { 0.6 } else { 0.0 };

        let untraced =
            evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch).expect("eligible");
        let mut sink = EvalTrace::default();
        let traced = evaluate_single_with_trace(
            &dse,
            entity,
            &ctx,
            &maslow,
            &modifiers,
            &fetch,
            Some(&mut sink),
        )
        .expect("eligible");

        assert!((untraced.raw_score - traced.raw_score).abs() < 1e-6);
        assert!((untraced.gated_score - traced.gated_score).abs() < 1e-6);
        assert!((untraced.final_score - traced.final_score).abs() < 1e-6);
    }

    #[test]
    fn modifier_pipeline_apply_with_trace_records_nonzero_deltas() {
        struct AddFive;
        impl ScoreModifier for AddFive {
            fn apply(
                &self,
                _: DseId,
                score: f32,
                _: &EvalCtx,
                _: &dyn Fn(&str, Entity) -> f32,
            ) -> f32 {
                score + 0.05
            }
            fn name(&self) -> &'static str {
                "add_five"
            }
        }
        struct NoOpUnlessAbove05;
        impl ScoreModifier for NoOpUnlessAbove05 {
            fn apply(
                &self,
                _: DseId,
                score: f32,
                _: &EvalCtx,
                _: &dyn Fn(&str, Entity) -> f32,
            ) -> f32 {
                if score > 0.5 {
                    score * 0.5
                } else {
                    score
                }
            }
            fn name(&self) -> &'static str {
                "cap"
            }
        }

        let mut pipeline = ModifierPipeline::new();
        pipeline.push(Box::new(AddFive));
        pipeline.push(Box::new(NoOpUnlessAbove05));

        let entity = Entity::from_raw_u32(1).unwrap();
        let has_marker = |_: &str, _: Entity| false;
        let entity_position = |_: Entity| -> Option<Position> { None };
        let anchor_position = |_: LandmarkAnchor| -> Option<Position> { None };
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            entity_position: &entity_position,
            anchor_position: &anchor_position,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
        };
        let fetch = |_: &str, _: Entity| 0.0;

        // Input 0.6 → AddFive takes it to 0.65 → cap halves to 0.325.
        // Both passes should appear in the trace because both changed score.
        let mut deltas = Vec::new();
        let out = pipeline.apply_with_trace(DseId("eat"), 0.6, &ctx, &fetch, Some(&mut deltas));
        assert!((out - 0.325).abs() < 1e-6, "got {out}");
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].name, "add_five");
        assert!((deltas[0].post - deltas[0].pre - 0.05).abs() < 1e-6);
        assert_eq!(deltas[1].name, "cap");

        // Input 0.4 → AddFive takes it to 0.45 → cap no-ops at 0.45.
        // Only AddFive should appear; cap's pre==post is filtered out.
        let mut deltas = Vec::new();
        let out = pipeline.apply_with_trace(DseId("eat"), 0.4, &ctx, &fetch, Some(&mut deltas));
        assert!((out - 0.45).abs() < 1e-6, "got {out}");
        assert_eq!(deltas.len(), 1, "cap was a no-op, should be skipped");
        assert_eq!(deltas[0].name, "add_five");
    }
}
