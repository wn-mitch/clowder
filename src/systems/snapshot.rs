use bevy_ecs::prelude::*;

use crate::ai::CurrentAction;
use crate::components::aspirations::Aspirations;
use crate::components::fulfillment::Fulfillment;
use crate::components::held_goal_stack::HeldGoalStack;
use crate::components::held_intention::IntentionSource;
use crate::components::identity::{Age, Gender, Name, Orientation};
use crate::components::mental::Mood;
use crate::components::parenting_activity::{ParentalKind, ParentingActivity};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::pregnancy::Pregnant;
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::event_log::{
    AspirationSnapshot, CatGoalState, EventKind, EventLog, GoalFrameSnapshot, ParentingSummary,
    RelationshipEntry,
};
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::SimConstants;
use crate::resources::snapshot_config::SnapshotConfig;
use crate::resources::time::{SimConfig, TimeState};
use crate::systems::parenting_activity::{
    parental_engagement_asymptote, scale_autonomy, scale_cultural, scale_presence, scale_protection,
    scale_provision, ParentingScalars,
};

// ---------------------------------------------------------------------------
// emit_cat_snapshots system
// ---------------------------------------------------------------------------

/// Emit a `CatSnapshot` event for every living cat at the configured interval.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn emit_cat_snapshots(
    config: Res<SnapshotConfig>,
    sim_config: Res<SimConfig>,
    time: Res<TimeState>,
    query: Query<
        (
            (
                Entity,
                &Name,
                &Position,
                &Personality,
                &Needs,
                &Skills,
                &Mood,
                &Health,
            ),
            (
                &Corruption,
                &MagicAffinity,
                &CurrentAction,
                &Age,
                &Gender,
                &Orientation,
                Option<&Pregnant>,
                Option<&Fulfillment>,
                Option<&ParentingActivity>,
                Option<&HeldGoalStack>,
                Option<&Aspirations>,
            ),
        ),
        Without<Dead>,
    >,
    names: Query<&Name>,
    relationships: Res<Relationships>,
    parenting_scalars: Res<ParentingScalars>,
    constants: Res<SimConstants>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let Some(ref mut log) = event_log else { return };
    let interval = config.full_snapshot_interval;
    if interval == 0 || !time.tick.is_multiple_of(interval) {
        return;
    }
    let season = time.season(&sim_config);

    // Ticket 431 Stage D — system-level arena Vec for the top-3-by-fondness
    // sort. Pre-Stage-D this allocated a fresh `Vec<(Entity, &Relationship)>`
    // per cat per snapshot tick via `Relationships::all_for(entity)`
    // (catalog row #5 in the 2026-05-20 flamegraph — 2.36% inclusive CPU
    // dominated by the Vec materialization). Arena owns `Relationship`
    // clones (cheap struct copy — 5 small fields, ~40 bytes) so the Vec's
    // lifetime can outlive each loop iteration; one allocation per system
    // run instead of one per cat per run.
    let mut rels_arena: Vec<(Entity, crate::resources::relationships::Relationship)> =
        Vec::with_capacity(32);

    for (
        (entity, name, pos, personality, needs, skills, mood, health),
        (
            corruption,
            magic_aff,
            current,
            age,
            gender,
            orientation,
            pregnant,
            fulfillment,
            parenting_activity,
            goal_stack,
            aspirations,
        ),
    ) in &query
    {
        let life_stage = age.stage(time.tick, sim_config.ticks_per_season);
        // Build top-3 relationships by |fondness|.
        rels_arena.clear();
        rels_arena.extend(
            relationships
                .iter_for(entity)
                .map(|(other, rel)| (other, rel.clone())),
        );
        rels_arena.sort_by(|(_, a), (_, b)| {
            b.fondness
                .abs()
                .partial_cmp(&a.fondness.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_rels: Vec<RelationshipEntry> = rels_arena
            .iter()
            .take(3)
            .filter_map(|(other, rel)| {
                let other_name = names.get(*other).ok()?;
                Some(RelationshipEntry {
                    cat: other_name.0.clone(),
                    fondness: rel.fondness,
                    familiarity: rel.familiarity,
                    romantic: rel.romantic,
                    bond: rel.bond.as_ref().map(|b| format!("{b:?}")),
                })
            })
            .collect();

        let effective_valence = mood.valence + mood.modifiers.iter().map(|m| m.amount).sum::<f32>();

        // Ticket 400 — assemble ParentingActivity snapshot for cats
        // carrying the Component. Asymptote-vs-engagement-max gap is
        // diagnostic for "is engagement converged" in `just inspect`.
        let parenting = parenting_activity.map(|pa| Box::new({
            let bundle = parenting_scalars.get(entity);
            let pc = &constants.parenting;
            let mut bio = 0;
            let mut inl = 0;
            let mut bnd = 0;
            let mut adp = 0;
            for rel in &pa.relationships {
                match rel.kind {
                    ParentalKind::Biological => bio += 1,
                    ParentalKind::InLaw => inl += 1,
                    ParentalKind::BondFormed => bnd += 1,
                    ParentalKind::Adopted => adp += 1,
                }
            }
            ParentingSummary {
                asymptote: parental_engagement_asymptote(personality, 0.0, pc),
                scale_presence: scale_presence(personality),
                scale_provision: scale_provision(personality),
                scale_protection: scale_protection(personality),
                scale_cultural: scale_cultural(personality),
                scale_autonomy: scale_autonomy(personality, 0.0),
                caretake_bias_sum: bundle.caretake_bias_sum,
                provision_bias_sum: bundle.provision_bias_sum,
                protect_bias_sum: bundle.protect_bias_sum,
                cultural_teach_bias_sum: bundle.cultural_teach_bias_sum,
                autonomy_teach_bias_sum: bundle.autonomy_teach_bias_sum,
                caretake_suppression_factor: bundle.caretake_suppression_factor,
                parental_engagement_max: bundle.parental_engagement_max,
                biological_count: bio,
                in_law_count: inl,
                bond_formed_count: bnd,
                adopted_count: adp,
            }
        }));

        // Ticket 339 — serialize HTN goal-stack as stable string slugs.
        let goal_stack_snap: Vec<GoalFrameSnapshot> = goal_stack
            .map(|stack| {
                stack
                    .frames
                    .iter()
                    .map(|f| GoalFrameSnapshot {
                        method: f.method.0.to_string(),
                        goal_label: f.goal_label.to_string(),
                        sub_goal_index: f.sub_goal_index,
                        sub_goal_count: f.sub_goal_count,
                        target: f
                            .target
                            .and_then(|e| names.get(e).ok())
                            .map(|n| n.0.clone()),
                        source: match &f.source {
                            IntentionSource::SelfMotivated => "self_motivated".to_string(),
                            IntentionSource::CoordinatorDirective { .. } => {
                                "coordinator_directive".to_string()
                            }
                            IntentionSource::AspirationEmitted { chain } => {
                                format!("aspiration:{chain}")
                            }
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Ticket 339 — serialize active aspirations.
        let active_aspirations_snap: Vec<AspirationSnapshot> = aspirations
            .map(|asps| {
                asps.active
                    .iter()
                    .map(|a| AspirationSnapshot {
                        chain_name: a.chain_name.clone(),
                        domain: a.domain,
                        current_milestone: a.current_milestone,
                        progress: a.progress,
                        adopted_tick: a.adopted_tick,
                        last_progress_tick: a.last_progress_tick,
                    })
                    .collect()
            })
            .unwrap_or_default();

        log.push(
            time.tick,
            EventKind::CatSnapshot {
                cat: name.0.clone(),
                position: (pos.x, pos.y),
                personality: personality.clone(),
                needs: needs.clone(),
                skills: Box::new(skills.clone()),
                mood_valence: effective_valence.clamp(-1.0, 1.0),
                mood_modifier_count: mood.modifiers.len(),
                health: health.current,
                corruption: corruption.0,
                magic_affinity: magic_aff.0,
                current_action: current.action,
                relationships: top_rels,
                last_scores: current.last_scores.clone(),
                life_stage: format!("{life_stage:?}"),
                sex: format!("{gender:?}"),
                orientation: format!("{orientation:?}"),
                is_pregnant: pregnant.is_some(),
                season: format!("{season:?}"),
                social_warmth: fulfillment.map_or(0.6, |f| f.social_warmth),
                parenting,
                goal_state: Box::new(CatGoalState {
                    goal_stack: goal_stack_snap,
                    active_aspirations: active_aspirations_snap,
                }),
            },
        );
    }
}

// ---------------------------------------------------------------------------
// emit_position_traces system
// ---------------------------------------------------------------------------

/// Lightweight per-tick position trace. Disabled by default — enable via
/// `--trace-positions <interval>`.
pub fn emit_position_traces(
    config: Res<SnapshotConfig>,
    time: Res<TimeState>,
    query: Query<(&Name, &Position, &CurrentAction), Without<Dead>>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let interval = config.position_trace_interval;
    if interval == 0 {
        return;
    }
    let Some(ref mut log) = event_log else { return };
    if !time.tick.is_multiple_of(interval) {
        return;
    }

    for (name, pos, current) in &query {
        log.push(
            time.tick,
            EventKind::PositionTrace {
                cat: name.0.clone(),
                position: (pos.x, pos.y),
                action: current.action,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// emit_spatial_snapshots system
// ---------------------------------------------------------------------------

/// Emits the four spatial map-overlay events (WildlifePositions, PreyPositions,
/// DenSnapshot, HuntingBeliefSnapshot) on their respective intervals. All are
/// additive and default to reasonable-but-off-by-a-longer-cadence so they
/// don't bloat the log on a standard run.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn emit_spatial_snapshots(
    config: Res<SnapshotConfig>,
    time: Res<TimeState>,
    wildlife: Query<(&crate::components::wildlife::WildAnimal, &Position)>,
    prey: Query<
        (&crate::components::prey::PreyConfig, &Position),
        With<crate::components::prey::PreyAnimal>,
    >,
    prey_dens: Query<(&crate::components::prey::PreyDen, &Position)>,
    fox_dens: Query<(&crate::components::wildlife::FoxDen, &Position)>,
    colony_map: Option<Res<crate::resources::colony_hunting_map::ColonyHuntingMap>>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let Some(ref mut log) = event_log else { return };
    let tick = time.tick;

    if config.spatial_interval > 0 && tick.is_multiple_of(config.spatial_interval) {
        let positions: Vec<crate::resources::event_log::WildlifePosRow> = wildlife
            .iter()
            .map(|(w, p)| crate::resources::event_log::WildlifePosRow {
                species: format!("{:?}", w.species),
                x: p.x,
                y: p.y,
            })
            .collect();
        log.push(tick, EventKind::WildlifePositions { positions });

        let prey_positions: Vec<crate::resources::event_log::PreyPosRow> = prey
            .iter()
            .map(|(cfg, p)| crate::resources::event_log::PreyPosRow {
                species: format!("{:?}", cfg.kind),
                x: p.x,
                y: p.y,
            })
            .collect();
        log.push(
            tick,
            EventKind::PreyPositions {
                positions: prey_positions,
            },
        );
    }

    if config.den_snapshot_interval > 0 && tick.is_multiple_of(config.den_snapshot_interval) {
        let prey_den_rows: Vec<crate::resources::event_log::PreyDenRow> = prey_dens
            .iter()
            .map(|(den, pos)| crate::resources::event_log::PreyDenRow {
                species: format!("{:?}", den.kind),
                x: pos.x,
                y: pos.y,
                spawns_remaining: den.spawns_remaining,
                capacity: den.capacity,
                predation_pressure: den.predation_pressure,
            })
            .collect();
        let fox_den_rows: Vec<crate::resources::event_log::FoxDenRow> = fox_dens
            .iter()
            .map(|(den, pos)| crate::resources::event_log::FoxDenRow {
                x: pos.x,
                y: pos.y,
                cubs_present: den.cubs_present,
                territory_radius: den.territory_radius,
                scent_strength: den.scent_strength,
            })
            .collect();
        log.push(
            tick,
            EventKind::DenSnapshot {
                prey_dens: prey_den_rows,
                fox_dens: fox_den_rows,
            },
        );
    }

    if config.hunting_belief_interval > 0 && tick.is_multiple_of(config.hunting_belief_interval) {
        if let Some(map) = colony_map.as_ref() {
            let priors = &map.beliefs;
            let (w, h, values) =
                downsample_belief_grid(&priors.beliefs, priors.grid_w, priors.grid_h, 32, 32);
            log.push(
                tick,
                EventKind::HuntingBeliefSnapshot {
                    cat: None,
                    width: w as u32,
                    height: h as u32,
                    values,
                },
            );
        }
    }
}

/// Downsamples a row-major belief grid to at most `target_w × target_h`
/// cells using block-averaging. Guarantees a bounded payload regardless of
/// map size.
fn downsample_belief_grid(
    src: &[f32],
    src_w: usize,
    src_h: usize,
    target_w: usize,
    target_h: usize,
) -> (usize, usize, Vec<f32>) {
    if src_w == 0 || src_h == 0 || src.is_empty() {
        return (0, 0, Vec::new());
    }
    let out_w = target_w.min(src_w).max(1);
    let out_h = target_h.min(src_h).max(1);
    let mut out = vec![0.0f32; out_w * out_h];
    let mut counts = vec![0u32; out_w * out_h];
    for sy in 0..src_h {
        let oy = (sy * out_h) / src_h;
        for sx in 0..src_w {
            let ox = (sx * out_w) / src_w;
            let idx = oy * out_w + ox;
            out[idx] += src[sy * src_w + sx];
            counts[idx] += 1;
        }
    }
    for (o, c) in out.iter_mut().zip(counts.iter()) {
        if *c > 0 {
            *o /= *c as f32;
        }
    }
    (out_w, out_h, out)
}
