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
//!   resource (FoxScentMap, CatScentMap, ExplorationMap as of
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
use bevy_ecs::system::SystemState;

use crate::ai::CurrentAction;
use crate::components::disposition::Disposition;
use crate::components::goap_plan::GoapPlan;
use crate::components::held_goal_stack::HeldGoalStack;
use crate::components::held_intention::IntentionSource;
use crate::components::identity::{Name, Species};
use crate::components::physical::{Dead, Position};
use crate::components::sensing::SensorySpecies;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;
use crate::resources::trace_log::{
    AttenuationBreakdown, BeliefProxySummary, CapturedDse, CommitmentCapture, CompositionSummary,
    ConsiderationContribution, EligibilitySummary, FocalScoreCapture, FocalTraceTarget,
    IntentionSummary, MethodFrameTraceRecord, ModifierApplication, MomentumSummary,
    PlanFailureCapture, PlanStateSummary, ResolverModifierCapture, SoftmaxSummary, SpatialRef,
    TraceEntry, TraceLog, TraceRecord,
};
use crate::systems::influence_map::{
    channel_label, Attenuation, Faction, InfluenceMapRegistry, MapMetadata,
};

/// Resolves the focal cat's entity and emits L1/L2/L3 records for the
/// current tick. Gated on `FocalTraceTarget`; a no-op in every build
/// where the resource isn't inserted (i.e. every interactive build, and
/// every headless run without `--focal-cat`).
///
/// Runs after `goap::resolve_goap_plans` so `last_scores` reflects the
/// current tick's evaluation and `GoapPlan` is the plan the cat just
/// adopted.
///
/// **Exclusive system** (ticket 207). Walks `InfluenceMapRegistry`
/// for L1 emission instead of bundling each map as a `SystemParam`
/// field. The exclusive form is necessary because the registry's
/// walkers take `&World` — they fetch their target `Resource` by
/// type at call time. The system is gated on three `resource_exists`
/// `run_if`s and never fires in interactive builds, so the
/// single-threaded scheduling cost is paid only on focal-cat soaks.
pub fn emit_focal_trace(world: &mut World) {
    // SystemState bundles the per-tick params that the L2/L3 paths
    // still need. The block scope releases the world borrow before
    // we walk the registry below — `register::<M>` walkers fetch
    // their target resource via `world.get_resource::<M>()`, which
    // requires `&World` access that conflicts with SystemState's
    // `&mut World`.
    type FocalParams<'w, 's> = (
        ResMut<'w, FocalTraceTarget>,
        Res<'w, TimeState>,
        Res<'w, FocalScoreCapture>,
        Query<
            'w,
            's,
            (
                Entity,
                &'static Name,
                &'static Position,
                &'static CurrentAction,
                Option<&'static Disposition>,
                Option<&'static GoapPlan>,
                Option<&'static HeldGoalStack>,
            ),
            (With<Species>, Without<Dead>),
        >,
    );

    let mut state: SystemState<FocalParams> = SystemState::new(world);

    // Phase 1 — extract focal data + drain capture inside the
    // SystemState scope; everything we need post-walk gets cloned
    // into owned values so the World borrow can be released.
    struct FocalSnapshot {
        tick: u64,
        cat_name: String,
        pos: Position,
        chosen: String,
        active_intention: Option<String>,
        last_scores: Vec<(crate::ai::Action, f32)>,
        goap_plan_steps: Vec<String>,
        momentum_preempted: bool,
        /// Ticket 337 — `HeldGoalStack` frames walked at snapshot time.
        /// Empty when the focal cat has no active method frames.
        method_stack: Vec<MethodFrameTraceRecord>,
    }
    let snapshot_and_capture: Option<(
        FocalSnapshot,
        crate::resources::trace_log::FocalScoreCaptureInner,
    )> = {
        let (mut target, time, focal_capture, cats) = state.get_mut(world);

        // Resolve focal entity by name if not already known, or
        // re-resolve if the cached entity no longer matches (covers
        // spawn-after-start and respawn-under-same-name edge cases).
        let focal = if let Some(e) = target.entity {
            cats.get(e).ok().map(|row| (e, row))
        } else {
            cats.iter()
                .find(|(_, name, _, _, _, _, _)| name.0 == target.name)
                .map(|row| (row.0, row))
        };

        let Some((entity, (_, name, pos, current, disposition, goap_plan, held_goal_stack))) =
            focal
        else {
            return;
        };

        if target.entity != Some(entity) {
            target.entity = Some(entity);
        }

        let captured = focal_capture.drain();
        let snapshot = FocalSnapshot {
            tick: time.tick,
            cat_name: name.0.clone(),
            pos: *pos,
            chosen: format!("{:?}", current.action),
            active_intention: disposition.map(|d| format!("{:?}", d.kind)),
            last_scores: current.last_scores.clone(),
            goap_plan_steps: goap_plan
                .map(|p| {
                    p.steps
                        .iter()
                        .map(|s| format!("{:?}", s.action))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            momentum_preempted: captured.momentum_preempted,
            method_stack: held_goal_stack
                .map(method_stack_from_goal_stack)
                .unwrap_or_default(),
        };
        Some((snapshot, captured))
    };

    let Some((snapshot, captured)) = snapshot_and_capture else {
        return;
    };

    // -----------------------------------------------------------------
    // L1 — one record per registered InfluenceMap. The trace emitter
    // owns no static knowledge of which maps exist; it walks
    // `InfluenceMapRegistry` (populated at startup by
    // `populate_influence_map_registry` in `simulation.rs`). New
    // `impl InfluenceMap` blocks register themselves there, with
    // zero edits to this file.
    //
    // Cat is the observer species — species-sens is looked up against
    // `SensorySpecies::Cat` on each map's channel via §5.6.6.
    // -----------------------------------------------------------------
    let samples: Vec<(MapMetadata, f32)> = {
        let registry = world.resource::<InfluenceMapRegistry>();
        let mut out = Vec::with_capacity(registry.len());
        for walker in registry.walkers() {
            if let Some(sample) = walker(world, snapshot.pos) {
                out.push(sample);
            }
        }
        out
    };

    // Snapshot the SimConstants references we need for attenuation +
    // softmax-fallback temperature so we can release the world borrow
    // before grabbing `&mut TraceLog`.
    //
    // Ticket 232 — when the softmax didn't actually run this tick (the
    // ineligible-pool fallback path), the L3 record reports the
    // ceiling as a placeholder. The empty `probabilities` vector is
    // the load-bearing "softmax fallthrough" signal for replay tools;
    // this temperature is informational only.
    let constants = world.resource::<SimConstants>();
    let softmax_fallback_temperature = constants.scoring.softmax_temperature_ceiling;
    let attenuations: Vec<(MapMetadata, f32, Attenuation)> = samples
        .into_iter()
        .map(|(metadata, base)| {
            let att = Attenuation::for_species_channel(
                &constants.sensory,
                SensorySpecies::Cat,
                metadata.channel,
            );
            (metadata, base, att)
        })
        .collect();

    // Phase 2 — emit records. Drop all immutable world borrows by
    // re-grabbing &mut TraceLog directly.
    let mut trace_log = world.resource_mut::<TraceLog>();

    for (metadata, base_sample, attenuation) in attenuations {
        emit_l1_record(
            &mut trace_log,
            snapshot.tick,
            &snapshot.cat_name,
            snapshot.pos,
            metadata,
            base_sample,
            attenuation,
        );
    }

    // -----------------------------------------------------------------
    // L4Resolver — resolver-level modifier reads (ticket 477). Emitted
    // independent of the L2/L3 capture gate because combat / detection
    // resolvers fire on any tick, not just planning ticks. Grouped by
    // resolver name into one record each so a tick's worth of one
    // resolver's modifier reads stays on a single line.
    // -----------------------------------------------------------------
    for record in l4_resolver_records(&captured.resolver_modifiers) {
        trace_log.push(TraceEntry {
            tick: snapshot.tick,
            cat: snapshot.cat_name.clone(),
            record,
        });
    }

    // L2/L3 paths only fire when the scoring pass produced capture
    // data (planning ticks); mid-plan ticks emit only L1.
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
    // paths (§7.5 anxiety, replan-cap). Emitted before L2 so a reader
    // scanning by tick sees the gate decision before the resulting
    // re-score, which matches the runtime order in `resolve_goap_plans`.
    // -----------------------------------------------------------------
    for row in &captured.commitment {
        trace_log.push(TraceEntry {
            tick: snapshot.tick,
            cat: snapshot.cat_name.clone(),
            record: l3_commitment_record(row, snapshot.method_stack.clone()),
        });
    }
    for row in &captured.plan_failures {
        trace_log.push(TraceEntry {
            tick: snapshot.tick,
            cat: snapshot.cat_name.clone(),
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
            tick: snapshot.tick,
            cat: snapshot.cat_name.clone(),
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
    let (ranked, softmax_summary, pre_bonus_pool, pre_penalty_pool) =
        if let Some(sm) = &captured.softmax {
            let ranked: Vec<(String, f32)> = sm
                .pool
                .iter()
                .map(|(a, s)| (format!("{a:?}"), *s))
                .collect();
            let summary = SoftmaxSummary {
                temperature: sm.temperature,
                probabilities: sm.probabilities.clone(),
            };
            let pre_bonus: Vec<(String, f32)> = sm
                .pre_bonus_pool
                .iter()
                .map(|(a, s)| (format!("{a:?}"), *s))
                .collect();
            let pre_penalty: Vec<(String, f32)> = sm
                .pool_pre_penalty
                .iter()
                .map(|(a, s)| (format!("{a:?}"), *s))
                .collect();
            (ranked, summary, pre_bonus, pre_penalty)
        } else {
            // Edge case: L2 captured but softmax didn't (e.g.
            // ineligible pool after filtering). Fall back to the
            // pre-softmax ranking from the snapshot's last_scores;
            // probabilities stay empty so replay tooling can
            // distinguish "softmax ran" from "softmax fallthrough".
            let ranked: Vec<(String, f32)> = snapshot
                .last_scores
                .iter()
                .map(|(a, s)| (format!("{a:?}"), *s))
                .collect();
            let summary = SoftmaxSummary {
                temperature: softmax_fallback_temperature,
                probabilities: Vec::new(),
            };
            (ranked, summary, Vec::new(), Vec::new())
        };

    trace_log.push(TraceEntry {
        tick: snapshot.tick,
        cat: snapshot.cat_name,
        record: TraceRecord::L3 {
            ranked,
            softmax: softmax_summary,
            momentum: MomentumSummary {
                active_intention: snapshot.active_intention,
                commitment_strength: 0.0,
                margin_threshold: 0.0,
                // Ticket 118 — flipped to `true` by
                // `check_modifier_preemption` via
                // `FocalScoreCapture::set_momentum_preempted` when an
                // acute-class modifier preempted the focal cat's plan
                // this tick.
                preempted: snapshot.momentum_preempted,
                // Ticket 126 — populated by C3's L2 author site once
                // HeldIntention is wired; legacy emission stays at
                // `None` / 0.0 (omitted under skip_serializing_if).
                held_dse: None,
                runner_up_margin: 0.0,
                decay_factor: 0.0,
            },
            chosen: snapshot.chosen,
            intention: IntentionSummary {
                kind: "Activity".into(),
                target: None,
                goal_state: None,
            },
            goap_plan: snapshot.goap_plan_steps,
            pre_bonus_pool,
            pre_penalty_pool,
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
            // Ticket 400 — `details` populated by the trace builder
            // for modifiers whose internal state is informative beyond
            // delta (e.g., `parenting_activity` carries 5 scale sums
            // + suppression factor). None for additive-only modifiers.
            details: None,
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
/// `method_stack` is the focal cat's `HeldGoalStack` walked at snapshot
/// time (ticket 337). Empty for cats running primitive Intentions.
fn l3_commitment_record(
    row: &CommitmentCapture,
    method_stack: Vec<MethodFrameTraceRecord>,
) -> TraceRecord {
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
        // Ticket 126 — momentum + abandon_reason populated by C4's
        // HeldIntention drop path; legacy plan-only branches emit
        // `None` (omitted under skip_serializing_if for back-compat).
        momentum: None,
        abandon_reason: row.abandon_reason.map(str::to_string),
        method_stack,
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

/// Group a tick's captured resolver-modifier reads (ticket 477) by
/// resolver name into one [`TraceRecord::L4Resolver`] per resolver.
/// Preserves first-seen resolver order and per-resolver push order so
/// the trace is stable across runs. Each modifier becomes a
/// [`ModifierApplication`] with `delta = post - pre` (the §3.5.1 shape).
fn l4_resolver_records(rows: &[ResolverModifierCapture]) -> Vec<TraceRecord> {
    let mut order: Vec<&'static str> = Vec::new();
    let mut grouped: std::collections::HashMap<&'static str, Vec<ModifierApplication>> =
        std::collections::HashMap::new();
    for row in rows {
        if !grouped.contains_key(row.resolver) {
            order.push(row.resolver);
        }
        grouped
            .entry(row.resolver)
            .or_default()
            .push(ModifierApplication {
                name: row.modifier.clone(),
                delta: Some(row.post - row.pre),
                multiplier: None,
                details: None,
            });
    }
    order
        .into_iter()
        .map(|resolver| TraceRecord::L4Resolver {
            resolver: resolver.to_string(),
            modifiers: grouped.remove(resolver).unwrap_or_default(),
        })
        .collect()
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
/// `(metadata, base_sample, attenuation)` is supplied by the
/// `InfluenceMapRegistry` walk; this helper only formats the record
/// shape.
///
/// `top_contributors` stays empty at Phase 2A — populating it
/// requires per-emitter reverse lookup (§5.1's "which fox drove this
/// scent reading"), which is Phase 2B work.
fn emit_l1_record(
    trace_log: &mut TraceLog,
    tick: u64,
    cat_name: &str,
    pos: Position,
    metadata: MapMetadata,
    base_sample: f32,
    attenuation: Attenuation,
) {
    let MapMetadata {
        name,
        channel,
        faction,
    } = metadata;
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

/// Walk a `HeldGoalStack` and return one `MethodFrameTraceRecord` per
/// frame, in adoption order (root first, active leaf last). Per §11.5:
/// no per-method special-casing — the walk is purely structural.
///
/// `target` is emitted as `None` in Phase 1 because `GoalFrame.target`
/// holds an `Entity` (runtime-unstable) and name-resolution would
/// require an additional world query. Downstream tooling treats `null`
/// as "no named target" (same meaning as an unbound method).
fn method_stack_from_goal_stack(stack: &HeldGoalStack) -> Vec<MethodFrameTraceRecord> {
    stack
        .frames
        .iter()
        .map(|frame| MethodFrameTraceRecord {
            method: frame.method.to_string(),
            goal: frame.goal_label.to_string(),
            sub_goal_index: frame.sub_goal_index,
            sub_goal_count: frame.sub_goal_count,
            target: None,
            source: intention_source_slug(&frame.source),
        })
        .collect()
}

/// Convert an `IntentionSource` to the canonical source-slug string used
/// in `MethodFrameTraceRecord.source` and `GoalFrameSnapshot.source`.
/// Matches the schema defined in `docs/systems/htn-methods.md` §Trace:
/// `"self"` / `"coordinator"` / `"aspiration:<chain-name>"`.
fn intention_source_slug(source: &IntentionSource) -> String {
    match source {
        IntentionSource::SelfMotivated => "self".to_string(),
        IntentionSource::CoordinatorDirective { .. } => "coordinator".to_string(),
        IntentionSource::AspirationEmitted { chain } => format!("aspiration:{chain}"),
    }
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
