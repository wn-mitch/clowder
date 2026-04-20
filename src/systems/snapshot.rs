use bevy_ecs::prelude::*;

use crate::ai::CurrentAction;
use crate::components::identity::{Age, Gender, Name, Orientation};
use crate::components::mental::Mood;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::pregnancy::Pregnant;
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::event_log::{EventKind, EventLog, RelationshipEntry};
use crate::resources::relationships::Relationships;
use crate::resources::snapshot_config::SnapshotConfig;
use crate::resources::time::{SimConfig, TimeState};

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
            Entity,
            &Name,
            &Position,
            &Personality,
            &Needs,
            &Skills,
            &Mood,
            &Health,
            &Corruption,
            &MagicAffinity,
            &CurrentAction,
            &Age,
            &Gender,
            &Orientation,
            Option<&Pregnant>,
        ),
        Without<Dead>,
    >,
    names: Query<&Name>,
    relationships: Res<Relationships>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let Some(ref mut log) = event_log else { return };
    let interval = config.full_snapshot_interval;
    if interval == 0 || !time.tick.is_multiple_of(interval) {
        return;
    }
    let season = time.season(&sim_config);

    for (
        entity,
        name,
        pos,
        personality,
        needs,
        skills,
        mood,
        health,
        corruption,
        magic_aff,
        current,
        age,
        gender,
        orientation,
        pregnant,
    ) in &query
    {
        let life_stage = age.stage(time.tick, sim_config.ticks_per_season);
        // Build top-3 relationships by |fondness|.
        let mut rels: Vec<(Entity, &crate::resources::relationships::Relationship)> =
            relationships.all_for(entity);
        rels.sort_by(|(_, a), (_, b)| {
            b.fondness
                .abs()
                .partial_cmp(&a.fondness.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_rels: Vec<RelationshipEntry> = rels
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

        log.push(
            time.tick,
            EventKind::CatSnapshot {
                cat: name.0.clone(),
                position: (pos.x, pos.y),
                personality: personality.clone(),
                needs: needs.clone(),
                skills: skills.clone(),
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
