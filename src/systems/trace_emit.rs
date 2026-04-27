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
//! Layer emission strategy:
//!
//! - **L1** (Phase 2 enrichment) — one record per (focal cat × registered
//!   influence map × tick). Walks every `InfluenceMap`-implementing
//!   resource (FoxScentMap, CatPresenceMap, ExplorationMap as of
//!   Phase 2A) and emits a record carrying the map's metadata, base
//!   sample at the focal cat's position, and per-channel attenuation
//!   from the §5.6.6 pipeline. Scent-from-on-demand and corruption
//!   migrations in Phase 2B/2C extend the walk to those maps.
//!
//! - **L2** (Phase 1 shim) — one record per (focal cat × eligible
//!   action × tick). The shim walks `CurrentAction::last_scores` (the
//!   ranked, post-modifier score list already populated by
//!   `goap::evaluate_and_plan`) and emits a minimal record with
//!   `final_score` populated and `considerations`/`modifiers` empty.
//!   Phase 3's Dse trait lets the emitter capture per-consideration
//!   contributions.
//!
//! - **L3** (Phase 1 shim) — one record per (focal cat × tick) with
//!   the full ranked list, chosen action, and placeholder softmax /
//!   momentum summaries. Phase 6 fills in real softmax probabilities
//!   and the §7.4 persistence-bonus-aware momentum trace.
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
use crate::components::sensing::SensorySpecies;
use crate::resources::cat_presence_map::CatPresenceMap;
use crate::resources::exploration_map::ExplorationMap;
use crate::resources::fox_scent_map::FoxScentMap;
use crate::resources::map::TileMap;
use crate::resources::carcass_scent_map::CarcassScentMap;
use crate::resources::prey_scent_map::PreyScentMap;
use crate::resources::ward_coverage_map::WardCoverageMap;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;
use crate::resources::trace_log::{
    AttenuationBreakdown, BeliefProxySummary, CapturedDse, CommitmentCapture, CompositionSummary,
    ConsiderationContribution, EligibilitySummary, FocalScoreCapture, FocalTraceTarget,
    IntentionSummary, ModifierApplication, MomentumSummary, PlanFailureCapture, PlanStateSummary,
    SoftmaxSummary, SpatialRef, TraceEntry, TraceLog, TraceRecord,
};
use crate::systems::influence_map::{
    channel_label, Attenuation, CorruptionLens, Faction, InfluenceMap, MapMetadata,
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
    prey_scent_map: Option<Res<PreyScentMap>>,
    carcass_scent_map: Option<Res<CarcassScentMap>>,
    cat_presence_map: Option<Res<CatPresenceMap>>,
    exploration_map: Option<Res<ExplorationMap>>,
    ward_coverage_map: Option<Res<WardCoverageMap>>,
    tile_map: Option<Res<TileMap>>,
    mut trace_log: ResMut<TraceLog>,
    focal_capture: Res<FocalScoreCapture>,
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
    // L1 — one record per registered InfluenceMap. The walk now covers
    // seven maps: FoxScentMap, PreyScentMap, CarcassScentMap (Phase 2C),
    // CatPresenceMap, ExplorationMap, WardCoverageMap, and the
    // CorruptionLens borrow adapter for TileMap.corruption. Cat is the
    // observer species, so species-sens is looked up against
    // `SensorySpecies::Cat` on each channel via the §5.6.6 attenuation
    // pipeline. Phase 2D will replace this hardcoded sequence with a
    // registry walk.
    // -----------------------------------------------------------------
    if let Some(ref m) = fox_scent_map {
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants);
    }
    if let Some(ref m) = prey_scent_map {
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants);
    }
    if let Some(ref m) = carcass_scent_map {
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants);
    }
    if let Some(ref m) = cat_presence_map {
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants);
    }
    if let Some(ref m) = exploration_map {
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants);
    }
    if let Some(ref m) = ward_coverage_map {
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &**m, &constants);
    }
    if let Some(ref m) = tile_map {
        let lens = CorruptionLens(m);
        emit_l1_for_map(&mut trace_log, tick, &cat_name, *pos, &lens, &constants);
    }

    // -----------------------------------------------------------------
    // Drain the tick's rich L2 + L3 capture. `score_dse_by_id` and
    // `select_disposition_via_intention_softmax_with_trace` populated
    // this during the scoring pass; we read it back post-`resolve_goap_plans`
    // so everything emits in one coherent frame. `drain()` clears the
    // capture for the next tick — the mutex is uncontested here since
    // scoring has already finished for this tick.
    //
    // `evaluate_and_plan` only fires when a cat's plan expires or needs
    // replanning (not every tick), so the capture is empty on ticks
    // where the focal cat is mid-plan. On those ticks we skip L2/L3
    // emission entirely — spec §11.4's "one record per tick-selection"
    // means every *selection*, not every tick wall-clock. L1 emission
    // (senses + influence maps) continues every tick and is the only
    // trace surface that samples at full cadence.
    // -----------------------------------------------------------------
    let captured = focal_capture.drain();
    // A "planning tick" used to mean scoring ran; with commitment /
    // plan-failure capture we also need to emit on ticks where the
    // gate fired but no re-score happened. Any captured data keeps
    // the emitter active.
    let has_capture = !captured.dses.is_empty()
        || captured.softmax.is_some()
        || !captured.commitment.is_empty()
        || !captured.plan_failures.is_empty();

    if !has_capture {
        return;
    }

    // -----------------------------------------------------------------
    // L3Commitment + L3PlanFailure — decision-point records captured
    // by the de-facto commitment branches (§7.2) and plan-failure
    // paths (§7.5 anxiety, replan-cap). Emitted first so a reader
    // scanning by tick sees the gate decision before the resulting
    // re-score, which matches the runtime order in `resolve_goap_plans`.
    // -----------------------------------------------------------------
    for row in &captured.commitment {
        trace_log.push(TraceEntry {
            tick,
            cat: cat_name.clone(),
            record: l3_commitment_record(row),
        });
    }
    for row in &captured.plan_failures {
        trace_log.push(TraceEntry {
            tick,
            cat: cat_name.clone(),
            record: l3_plan_failure_record(row),
        });
    }

    // -----------------------------------------------------------------
    // L2 — one record per captured DSE. §11.3 schema: eligibility
    // (markers_required + passed), per-consideration (name, input,
    // curve-label, score, weight, optional spatial ref), composition
    // (mode, raw), maslow_pregate, modifier deltas, final_score,
    // intention summary, optional target-ranking for target-taking
    // DSEs (§6.3). `top_losing` stays empty until §7.W.6 lands.
    // -----------------------------------------------------------------
    for dse in &captured.dses {
        trace_log.push(TraceEntry {
            tick,
            cat: cat_name.clone(),
            record: l2_record_for(dse, &captured.target_rankings),
        });
    }

    // L3 emission requires DSE scoring or softmax capture — if only
    // commitment / plan-failure rows were captured this tick, skip.
    if captured.dses.is_empty() && captured.softmax.is_none() {
        return;
    }

    // -----------------------------------------------------------------
    // L3 — selection record for the planning tick. Ranked list comes
    // from the softmax pool (the post-bonus, post-penalty scores the
    // softmax actually saw), probabilities from the captured
    // distribution, roll from the RNG draw.
    // -----------------------------------------------------------------
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

    let (ranked, softmax_summary) = if let Some(sm) = &captured.softmax {
        let ranked: Vec<(String, f32)> = sm
            .pool
            .iter()
            .map(|(a, s)| (format!("{a:?}"), *s))
            .collect();
        let summary = SoftmaxSummary {
            temperature: sm.temperature,
            probabilities: sm.probabilities.clone(),
        };
        (ranked, summary)
    } else {
        // Edge case: L2 captured but softmax didn't (e.g. ineligible
        // pool after filtering). Fall back to the pre-softmax ranking
        // from `current.last_scores`; probabilities stay empty so
        // replay tooling can distinguish "softmax ran" from "softmax
        // fallthrough".
        let ranked: Vec<(String, f32)> = current
            .last_scores
            .iter()
            .map(|(a, s)| (format!("{a:?}"), *s))
            .collect();
        let summary = SoftmaxSummary {
            temperature: constants.scoring.intention_softmax_temperature,
            probabilities: Vec::new(),
        };
        (ranked, summary)
    };

    trace_log.push(TraceEntry {
        tick,
        cat: cat_name,
        record: TraceRecord::L3 {
            ranked,
            softmax: softmax_summary,
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

/// Build a §11.3 L2 record from one captured DSE evaluation. Pulls
/// consideration-trace rows (name, input, curve label, score, weight,
/// optional spatial map key), composition summary, Maslow pre-gate,
/// modifier deltas, and the emitted Intention. Factored out so the
/// main emit loop reads as a forward-walk and the per-row conversions
/// stay readable.
fn l2_record_for(
    dse: &CapturedDse,
    target_rankings: &std::collections::HashMap<
        &'static str,
        crate::resources::trace_log::TargetRanking,
    >,
) -> TraceRecord {
    let considerations = dse
        .trace
        .considerations
        .iter()
        .map(|row| ConsiderationContribution {
            name: row.name.to_string(),
            input: row.input,
            curve: row.curve_label.clone(),
            score: row.score,
            weight: row.weight,
            spatial: row.spatial_map_key.map(|map_key| SpatialRef {
                map: map_key.to_string(),
                best_target: None,
            }),
        })
        .collect();
    let composition = CompositionSummary {
        mode: dse.trace.composition_mode.unwrap_or("Unknown").to_string(),
        raw: dse.raw_score,
    };
    let modifiers = dse
        .trace
        .modifier_deltas
        .iter()
        .map(|d| ModifierApplication {
            name: d.name.to_string(),
            // Emitted as an additive delta (`post - pre`); downstream
            // tooling treats `delta`-only rows as additive and
            // `multiplier`-only rows as multiplicative. The live
            // §3.5.1 modifier catalog is additive-only today, so
            // `multiplier` stays None.
            delta: Some(d.post - d.pre),
            multiplier: None,
        })
        .collect();
    let intention = intention_summary(&dse.intention);
    TraceRecord::L2 {
        dse: dse.dse_id.0.to_string(),
        eligibility: EligibilitySummary {
            markers_required: dse
                .eligibility_required
                .iter()
                .map(|s| s.to_string())
                .collect(),
            passed: dse.eligible,
        },
        considerations,
        composition,
        maslow_pregate: dse.trace.maslow_pregate,
        modifiers,
        final_score: dse.final_score,
        intention,
        top_losing: Vec::new(),
        // Target-taking DSEs emit their ranking under the suffixed id
        // (`"socialize_target"`), but the matching L2 record comes from
        // the self-state peer (`"socialize"`). Try the suffixed key
        // first so a standalone target-taking DSE that *does* get its
        // own L2 record still matches, then fall back to the bare id.
        targets: target_rankings
            .get(format!("{}_target", dse.dse_id.0).as_str())
            .or_else(|| target_rankings.get(dse.dse_id.0))
            .cloned(),
    }
}

/// Build a §11.3 L3Commitment record from one captured gate decision.
fn l3_commitment_record(row: &CommitmentCapture) -> TraceRecord {
    TraceRecord::L3Commitment {
        disposition: row.disposition.clone(),
        strategy: row.strategy.to_string(),
        proxies: BeliefProxySummary {
            achievement_believed: row.achievement_believed,
            achievable_believed: row.achievable_believed,
            still_goal: row.still_goal,
        },
        plan_state: PlanStateSummary {
            trips_done: row.trips_done,
            target_trips: row.target_trips,
            replan_count: row.replan_count,
            max_replans: row.max_replans,
        },
        branch: row.branch.to_string(),
        dropped: row.dropped,
    }
}

/// Build a §11.3 L3PlanFailure record from a captured plan-failure
/// event. `detail` is free-form `serde_json::Value` because the
/// replan-cap path and the anxiety-interrupt path carry different
/// fields — the reason string discriminates.
fn l3_plan_failure_record(row: &PlanFailureCapture) -> TraceRecord {
    TraceRecord::L3PlanFailure {
        reason: row.reason.to_string(),
        disposition: row.disposition.clone(),
        detail: row.detail.clone(),
    }
}

fn intention_summary(intention: &crate::ai::dse::Intention) -> IntentionSummary {
    use crate::ai::dse::Intention;
    match intention {
        Intention::Goal { state, .. } => IntentionSummary {
            kind: "Goal".to_string(),
            target: None,
            goal_state: Some(format!("{state:?}")),
        },
        Intention::Activity { kind, .. } => IntentionSummary {
            kind: "Activity".to_string(),
            target: None,
            goal_state: Some(format!("{kind:?}")),
        },
    }
}

/// Emit one L1 record for a focal-cat read of an `InfluenceMap` —
/// base sample at the cat's position + attenuation breakdown per
/// §5.6.6 (species-sensitivity on the map's channel; role / injury /
/// env at Phase 2A identity).
///
/// Kept generic over `M: InfluenceMap` so new map impls in Phase 2B/2C
/// plug in without touching the caller. `top_contributors` stays
/// empty at Phase 2A — populating it requires per-emitter reverse
/// lookup (§5.1's "which fox drove this scent reading"), which is
/// Phase 2B work.
fn emit_l1_for_map<M: InfluenceMap + ?Sized>(
    trace_log: &mut TraceLog,
    tick: u64,
    cat_name: &str,
    pos: Position,
    map: &M,
    constants: &SimConstants,
) {
    let MapMetadata {
        name,
        channel,
        faction,
    } = map.metadata();
    let base_sample = map.base_sample(pos);
    let attenuation =
        Attenuation::for_species_channel(&constants.sensory, SensorySpecies::Cat, channel);
    let perceived = attenuation.apply(base_sample);

    trace_log.push(TraceEntry {
        tick,
        cat: cat_name.to_string(),
        record: TraceRecord::L1 {
            map: name.to_string(),
            faction: faction_slug(&faction),
            channel: channel_label(channel).to_string(),
            pos: (pos.x, pos.y),
            base_sample,
            attenuation: AttenuationBreakdown {
                species_sens: attenuation.species_sens,
                role_mod: attenuation.role_mod,
                injury_deficit: attenuation.injury_deficit,
                env_mul: attenuation.env_mul,
            },
            perceived,
            top_contributors: Vec::new(),
        },
    });
}

/// Compact kebab-case slug for the `Faction` enum, used in the L1
/// record's `faction` field. Keeps JSON output short and greppable;
/// the full enum debug form (`"Species(Wild(Fox))"`) is noisier than
/// downstream tooling wants.
fn faction_slug(faction: &Faction) -> String {
    match faction {
        Faction::Species(s) => match s {
            SensorySpecies::Cat => "cat".to_string(),
            SensorySpecies::Wild(w) => format!("{w:?}").to_lowercase(),
            SensorySpecies::Prey(p) => format!("{p:?}").to_lowercase(),
        },
        Faction::Neutral => "neutral".to_string(),
        Faction::Colony => "colony".to_string(),
        Faction::Observer => "observer".to_string(),
    }
}
