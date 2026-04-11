use bevy_ecs::prelude::*;

use crate::ai::CurrentAction;
use crate::components::identity::Name;
use crate::components::mental::Mood;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::resources::event_log::{EventKind, EventLog, RelationshipEntry};
use crate::resources::relationships::Relationships;
use crate::resources::snapshot_config::SnapshotConfig;
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// emit_cat_snapshots system
// ---------------------------------------------------------------------------

/// Emit a `CatSnapshot` event for every living cat at the configured interval.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn emit_cat_snapshots(
    config: Res<SnapshotConfig>,
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
    ) in &query
    {
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
