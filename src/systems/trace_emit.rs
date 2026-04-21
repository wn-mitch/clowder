//! Focal-cat trace emitters — per §11 of the AI substrate refactor.
//!
//! Three emitters, one per substrate layer. All gate on
//! `resource_exists::<FocalTraceTarget>` so nothing fires in the
//! interactive build. Phase 1 ships **shim** implementations that read
//! today's scoring outputs rather than the trait-backed registry
//! Phase 3 introduces; the trace-record shapes in
//! `src/resources/trace_log.rs` are the Phase-3 schema, so the replay
//! format is stable across the refactor.
//!
//! Layer emission strategy (Phase 1 shim):
//!
//! - **L1** — one record per (focal cat × tick) with the fox-scent sample
//!   at the cat's position. Phase 2's `InfluenceMap` abstraction replaces
//!   this with registry-walking enumeration across the 13 L1 maps.
//!
//! - **L2** — one record per (focal cat × eligible action × tick). The
//!   shim walks `CurrentAction::last_scores` (the ranked, post-modifier
//!   score list already populated by `goap::evaluate_and_plan`) and
//!   emits a minimal record with `final_score` populated and
//!   `considerations`/`modifiers` empty. Phase 3's Dse trait lets the
//!   emitter capture per-consideration contributions.
//!
//! - **L3** — one record per (focal cat × tick) with the full ranked
//!   list, chosen action, and placeholder softmax/momentum summaries.
//!   Phase 6 fills in real softmax probabilities and the §7.4
//!   persistence-bonus-aware momentum trace.
//!
//! Schema slots that don't have values yet — top-N losing axes
//! (§7.W.6) and apophenia pairwise distance (§8.6) — are emitted as
//! empty/None so downstream tools can skip the field without crashing.

use bevy_ecs::prelude::*;

use crate::ai::CurrentAction;
use crate::components::disposition::Disposition;
use crate::components::goap_plan::GoapPlan;
use crate::components::identity::{Name, Species};
use crate::components::physical::{Dead, Position};
use crate::resources::fox_scent_map::FoxScentMap;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;
use crate::resources::trace_log::{
    AttenuationBreakdown, CompositionSummary, ConsiderationContribution, EligibilitySummary,
    FocalTraceTarget, IntentionSummary, ModifierApplication, MomentumSummary, SoftmaxSummary,
    TraceEntry, TraceLog, TraceRecord,
};

/// Resolves the focal cat's entity and emits L1/L2/L3 records for the
/// current tick. Gated on `FocalTraceTarget`; a no-op in every build
/// where the resource isn't inserted (i.e. every interactive build, and
/// every headless run without `--focal-cat`).
///
/// Runs after `goap::resolve_goap_plans` so `last_scores` reflects the
/// current tick's evaluation and `GoapPlan` is the plan the cat just
/// adopted.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn emit_focal_trace(
    mut target: ResMut<FocalTraceTarget>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    fox_scent_map: Option<Res<FoxScentMap>>,
    mut trace_log: ResMut<TraceLog>,
    cats: Query<
        (
            Entity,
            &Name,
            &Position,
            &CurrentAction,
            Option<&Disposition>,
            Option<&GoapPlan>,
        ),
        (With<Species>, Without<Dead>),
    >,
) {
    // Resolve focal entity by name if not already known, or re-resolve
    // if the cached entity no longer matches (covers spawn-after-start
    // and respawn-under-same-name edge cases).
    let focal = if let Some(e) = target.entity {
        cats.get(e).ok().map(|row| (e, row))
    } else {
        cats.iter()
            .find(|(_, name, _, _, _, _)| name.0 == target.name)
            .map(|row| (row.0, row))
    };

    let Some((entity, (_, name, pos, current, disposition, goap_plan))) = focal else {
        return;
    };

    if target.entity != Some(entity) {
        target.entity = Some(entity);
    }

    let tick = time.tick;
    let cat_name = name.0.clone();

    // -----------------------------------------------------------------
    // L1 — single fox-scent sample at the focal cat's position. Phase 2
    // replaces this with an InfluenceMap walk across all registered maps.
    // -----------------------------------------------------------------
    if let Some(ref scent) = fox_scent_map {
        let base_sample = scent.get(pos.x, pos.y);
        trace_log.push(TraceEntry {
            tick,
            cat: cat_name.clone(),
            record: TraceRecord::L1 {
                map: "fox_scent".into(),
                faction: "fox".into(),
                channel: "scent".into(),
                pos: (pos.x, pos.y),
                base_sample,
                attenuation: AttenuationBreakdown::default(),
                perceived: base_sample,
                top_contributors: Vec::new(),
            },
        });
    }

    // -----------------------------------------------------------------
    // L2 — one record per (action, score) in the ranked list. Phase 1
    // shim populates `final_score` only; `considerations`, `modifiers`,
    // `eligibility.markers_required` empty until Phase 3's Dse trait
    // lands.
    // -----------------------------------------------------------------
    for (action, score) in &current.last_scores {
        trace_log.push(TraceEntry {
            tick,
            cat: cat_name.clone(),
            record: TraceRecord::L2 {
                dse: format!("{action:?}"),
                eligibility: EligibilitySummary {
                    markers_required: Vec::new(),
                    passed: true,
                },
                considerations: Vec::<ConsiderationContribution>::new(),
                composition: CompositionSummary {
                    mode: "WeightedSum".into(),
                    raw: *score,
                },
                maslow_pregate: 1.0,
                modifiers: Vec::<ModifierApplication>::new(),
                final_score: *score,
                intention: IntentionSummary {
                    kind: "Activity".into(),
                    target: None,
                    goal_state: None,
                },
                top_losing: Vec::new(),
            },
        });
    }

    // -----------------------------------------------------------------
    // L3 — selection record. Ranked list comes from last_scores;
    // chosen action is CurrentAction. Softmax probabilities empty until
    // Phase 6 wires them through from `select_intention_softmax`.
    // -----------------------------------------------------------------
    let ranked: Vec<(String, f32)> = current
        .last_scores
        .iter()
        .map(|(a, s)| (format!("{a:?}"), *s))
        .collect();
    let chosen = format!("{:?}", current.action);
    let active_intention = disposition.map(|d| format!("{:?}", d.kind));
    let goap_plan_steps: Vec<String> = goap_plan
        .map(|p| {
            p.steps
                .iter()
                .map(|s| format!("{:?}", s.action))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    trace_log.push(TraceEntry {
        tick,
        cat: cat_name,
        record: TraceRecord::L3 {
            ranked,
            softmax: SoftmaxSummary {
                temperature: constants.scoring.disposition_softmax_temperature,
                probabilities: Vec::new(),
            },
            momentum: MomentumSummary {
                active_intention,
                commitment_strength: 0.0,
                margin_threshold: 0.0,
                preempted: false,
            },
            chosen,
            intention: IntentionSummary {
                kind: "Activity".into(),
                target: None,
                goal_state: None,
            },
            goap_plan: goap_plan_steps,
            apophenia: None,
        },
    });
}
