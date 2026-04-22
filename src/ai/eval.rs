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

use super::considerations::{CenterPolicy, Consideration};
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
    pub target_taking_dses: Vec<Box<dyn Dse>>,
    pub fox_dses: Vec<Box<dyn Dse>>,
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

    pub fn total(&self) -> usize {
        self.cat_dses.len()
            + self.target_taking_dses.len()
            + self.fox_dses.len()
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
    fn add_target_taking_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
    fn add_fox_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self;
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

    fn add_target_taking_dse(&mut self, dse: Box<dyn Dse>) -> &mut Self {
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
pub trait ScoreModifier: Send + Sync + 'static {
    /// Apply the modifier to `score` for DSE `dse_id`. Return the
    /// transformed score. A modifier that doesn't apply to `dse_id`
    /// (per the §3.5.2 applicability matrix) returns `score` unchanged.
    fn apply(&self, dse_id: DseId, score: f32, ctx: &EvalCtx) -> f32;

    fn name(&self) -> &'static str;
}

/// Ordered modifier pipeline. Phase 3b.1 ships the container shape;
/// concrete modifier implementations (Pride, Tradition, etc.) land
/// in Phase 3c alongside the DSEs that consume them.
#[derive(Resource, Default)]
pub struct ModifierPipeline {
    passes: Vec<Box<dyn ScoreModifier>>,
}

impl ModifierPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, modifier: Box<dyn ScoreModifier>) -> &mut Self {
        self.passes.push(modifier);
        self
    }

    /// Apply every registered modifier in registration order. Pure
    /// function of (dse_id, input_score, ctx).
    pub fn apply(&self, dse_id: DseId, mut score: f32, ctx: &EvalCtx) -> f32 {
        for pass in &self.passes {
            score = pass.apply(dse_id, score, ctx);
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
    if !passes_eligibility(dse.eligibility(), cat, ctx) {
        return None;
    }

    let considerations = dse.considerations();
    let composition = dse.composition();

    // Score each consideration via the appropriate input source.
    let per_consideration: Vec<f32> = considerations
        .iter()
        .map(|c| score_consideration(c, cat, ctx, fetch_scalar))
        .collect();

    // Compose to raw score.
    let raw_score = composition.compose(&per_consideration);

    // Maslow pre-gate. u8::MAX opts out (non-Maslow DSEs — coordinator
    // election, narrative selection).
    let gated_score = if dse.maslow_tier() == u8::MAX {
        raw_score
    } else {
        maslow_pre_gate(dse.maslow_tier()) * raw_score
    };

    // Post-scoring modifier pipeline.
    let final_score = modifiers.apply(dse.id(), gated_score, ctx);

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

/// Score one consideration by dispatching on its flavor. Scalar
/// considerations pull from `fetch_scalar`; spatial considerations
/// resolve a position + `ctx.sample_map`; marker considerations
/// consult `ctx.has_marker`.
fn score_consideration(
    consideration: &Consideration,
    cat: Entity,
    ctx: &EvalCtx,
    fetch_scalar: &dyn Fn(&str, Entity) -> f32,
) -> f32 {
    match consideration {
        Consideration::Scalar(s) => s.score(fetch_scalar(s.name, cat)),
        Consideration::Spatial(s) => {
            let pos = match s.center {
                CenterPolicy::SelfPosition => ctx.self_position,
                CenterPolicy::TargetPosition => match ctx.target_position {
                    Some(p) => p,
                    // Target-taking DSE scored without a target: zero.
                    // The target-taking evaluator (§6, Phase 4) must
                    // populate `ctx.target_position` before calling in.
                    None => return 0.0,
                },
            };
            s.score((ctx.sample_map)(s.map_key, pos))
        }
        Consideration::Marker(m) => m.score((ctx.has_marker)(m.marker, cat)),
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
    use super::*;
    use crate::ai::composition::Composition;
    use crate::ai::considerations::{Consideration, ScalarConsideration};
    use crate::ai::curves::hangry;
    use crate::ai::dse::{ActivityKind, DseId, Termination};
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
        let filter = EligibilityFilter::new().require("HasStoredFood");
        let entity = Entity::from_raw_u32(1).unwrap();

        // Missing required → fail.
        let has_marker = |_: &str, _: Entity| false;
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        assert!(!passes_eligibility(&filter, entity, &ctx));

        // Present required → pass.
        let has_marker = |m: &str, _: Entity| m == "HasStoredFood";
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        assert!(passes_eligibility(&filter, entity, &ctx));
    }

    #[test]
    fn eligibility_forbidden_marker() {
        let filter = EligibilityFilter::new().forbid("Incapacitated");
        let entity = Entity::from_raw_u32(1).unwrap();
        let sample = |_: &str, _: Position| 0.0;

        let has_incap = |m: &str, _: Entity| m == "Incapacitated";
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_incap,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        assert!(!passes_eligibility(&filter, entity, &ctx));

        let none = |_: &str, _: Entity| false;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &none,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        assert!(passes_eligibility(&filter, entity, &ctx));
    }

    #[test]
    fn evaluate_single_returns_none_on_ineligible() {
        let mut dse = test_dse("eat", "hunger");
        dse.eligibility = EligibilityFilter::new().require("HasStoredFood");
        let entity = Entity::from_raw_u32(1).unwrap();

        let has_marker = |_: &str, _: Entity| false;
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
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
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };

        // Maslow pre-gate: tier 1 suppression = 0.5 (simulating phys
        // half-satisfied).
        let maslow = |tier: u8| if tier == 1 { 0.5 } else { 1.0 };
        let modifiers = ModifierPipeline::new();
        // Hunger = 0.75 → Logistic(8, 0.75) evaluates to ~0.5.
        let fetch = |name: &str, _: Entity| if name == "hunger" { 0.75 } else { 0.0 };

        let scored = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch)
            .expect("eligible");
        assert!((scored.raw_score - 0.5).abs() < 0.01, "raw: {}", scored.raw_score);
        assert!((scored.gated_score - 0.25).abs() < 0.01, "gated: {}", scored.gated_score);
        // No modifiers → final == gated.
        assert!((scored.final_score - scored.gated_score).abs() < 1e-6);
    }

    #[test]
    fn non_maslow_dse_skips_pre_gate() {
        let mut dse = test_dse("narrative", "dummy");
        dse.tier = u8::MAX;
        let entity = Entity::from_raw_u32(1).unwrap();

        let has_marker = |_: &str, _: Entity| false;
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        // Maslow returns 0 for tier MAX, but the evaluator should
        // skip the gate entirely.
        let maslow = |_: u8| 0.0;
        let modifiers = ModifierPipeline::new();
        let fetch = |_: &str, _: Entity| 0.75;

        let scored = evaluate_single(&dse, entity, &ctx, &maslow, &modifiers, &fetch)
            .expect("eligible");
        assert!((scored.gated_score - scored.raw_score).abs() < 1e-6);
    }

    #[test]
    fn modifier_pipeline_applies_in_order() {
        struct AddOne;
        impl ScoreModifier for AddOne {
            fn apply(&self, _: DseId, score: f32, _: &EvalCtx) -> f32 {
                score + 0.1
            }
            fn name(&self) -> &'static str {
                "add_one"
            }
        }
        struct DoubleIt;
        impl ScoreModifier for DoubleIt {
            fn apply(&self, _: DseId, score: f32, _: &EvalCtx) -> f32 {
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
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
        };
        // 0.5 → 0.6 → 1.2
        let out = pipeline.apply(DseId("eat"), 0.5, &ctx);
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
        let sample = |_: &str, _: Position| 0.0;
        let ctx = EvalCtx {
            cat: entity,
            tick: 0,
            sample_map: &sample,
            has_marker: &has_marker,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
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

}
