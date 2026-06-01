//! `PlayBow` + `ReciprocalAdvance` emitters for the play-engagement perception
//! substrate (ticket 279).
//!
//! `PlayBow` is a probabilistic per-tick emitter — playful cats in a positive
//! mood and a play-eligible action will occasionally signal a solicitation when
//! a peer is in candidate range. Cooldown via `PlayBowCooldown` prevents serial
//! spam.
//!
//! `ReciprocalAdvance` is a `CatMoved`-driven emitter — when a cat moves, any
//! nearby peer that recently solicited play (or reciprocated) becomes a
//! candidate for "actor advanced toward target" emission. Cooldown via the
//! `reciprocal_window_ticks` knob keeps the chain bounded.
//!
//! Both emitters write `WitnessableEvent` through `MessageWriter`, the
//! convention established by 258's `belief_integrator` (see also `goap.rs`'s
//! `Groom`/`Mate`/`Care`/`Hunt` emit sites).

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::mental::{Mood, PlayBowCooldown};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Position};
use crate::messages::cat_moved::CatMoved;
use crate::messages::witnessable_event::WitnessableEvent;
use crate::resources::near_pair_cache::NearPairCache;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::time::TimeState;

/// Actions during which a cat is eligible to spontaneously emit a PlayBow.
/// Active dispositions (Hunt, Mate, Build, Coordinate, …) suppress the cue
/// — playfulness can't surface mid-task. Excludes Sleep/Flee/Fight/Eat
/// (negative-state or wholly-absorbing).
fn is_playbow_eligible_action(a: Action) -> bool {
    matches!(
        a,
        Action::Idle | Action::Wander | Action::Socialize | Action::Explore
    )
}

/// Per-tick PlayBow emitter. Iterates playful + positive-mood cats in
/// eligible actions; if a peer is in candidate range and the cooldown has
/// elapsed, rolls `playbow_emit_chance_per_tick` and emits.
#[allow(clippy::type_complexity)]
pub fn emit_play_bows(
    mut commands: Commands,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut rng: ResMut<SimRng>,
    mut events: MessageWriter<WitnessableEvent>,
    cats: Query<
        (
            Entity,
            &Position,
            &Personality,
            &Mood,
            &CurrentAction,
            Option<&PlayBowCooldown>,
        ),
        Without<Dead>,
    >,
    peers: Query<(Entity, &Position), Without<Dead>>,
) {
    let tick = time.tick;
    let cfg = &constants.play_cue_emission;

    for (entity, pos, personality, mood, action, cooldown) in &cats {
        if personality.playfulness < cfg.playbow_min_playfulness {
            continue;
        }
        if mood.valence < cfg.playbow_min_mood_valence {
            continue;
        }
        if !is_playbow_eligible_action(action.action) {
            continue;
        }

        // Cooldown gate — keeps serial-soliciter spam down.
        let on_cooldown = cooldown
            .as_ref()
            .and_then(|cd| cd.last_playbow_tick)
            .is_some_and(|last| tick.saturating_sub(last) < cfg.playbow_cooldown_ticks);
        if on_cooldown {
            continue;
        }

        // Candidate range: at least one peer within range. Skip emit if the
        // cat is alone — a play-bow signal with no audience is wasted RNG.
        let has_candidate = peers.iter().any(|(other, other_pos)| {
            other != entity && pos.distance_to(other_pos) <= cfg.playbow_candidate_range_tiles
        });
        if !has_candidate {
            continue;
        }

        if rng.rng.random::<f32>() >= cfg.playbow_emit_chance_per_tick {
            continue;
        }

        events.write(WitnessableEvent::PlayBow {
            actor: entity,
            position: *pos,
            tick,
        });
        commands.entity(entity).insert(PlayBowCooldown {
            last_playbow_tick: Some(tick),
            last_reciprocal_advance_tick: cooldown
                .as_ref()
                .and_then(|cd| cd.last_reciprocal_advance_tick),
        });
    }
}

/// Per-tick ReciprocalAdvance emitter. Reads `CatMoved` messages and for each
/// moved cat checks whether any nearby peer recently emitted PlayBow (or a
/// reciprocal advance) within the configured window. If so, emit one
/// `ReciprocalAdvance { actor: moved, target: peer, ... }` per qualifying
/// peer and stamp `last_reciprocal_advance_tick` to chain reciprocity.
#[allow(clippy::type_complexity)]
pub fn emit_reciprocal_advances(
    mut commands: Commands,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut moved: MessageReader<CatMoved>,
    mut events: MessageWriter<WitnessableEvent>,
    _cache: Res<NearPairCache>,
    cats: Query<(Entity, &Position, Option<&PlayBowCooldown>), Without<Dead>>,
) {
    let tick = time.tick;
    let cfg = &constants.play_cue_emission;
    let window = cfg.reciprocal_window_ticks;
    let range = cfg.reciprocal_engagement_range_tiles;

    for ev in moved.read() {
        let actor = ev.entity;
        let Ok((_, actor_pos, actor_cd)) = cats.get(actor) else {
            continue;
        };

        let mut chained = false;
        for (target, target_pos, target_cd) in &cats {
            if target == actor {
                continue;
            }
            if actor_pos.distance_to(target_pos) > range {
                continue;
            }
            // Target must have recently emitted PlayBow or ReciprocalAdvance.
            let Some(cd) = target_cd else {
                continue;
            };
            let recent_solicit = cd
                .last_playbow_tick
                .is_some_and(|t| tick.saturating_sub(t) <= window);
            let recent_reciprocal = cd
                .last_reciprocal_advance_tick
                .is_some_and(|t| tick.saturating_sub(t) <= window);
            if !recent_solicit && !recent_reciprocal {
                continue;
            }

            events.write(WitnessableEvent::ReciprocalAdvance {
                actor,
                target,
                position: ev.to,
                tick,
            });
            chained = true;
        }

        if chained {
            // Stamp the actor's reciprocal-advance tick so a counter-advance
            // from any prior solicitor can chain in turn.
            commands.entity(actor).insert(PlayBowCooldown {
                last_playbow_tick: actor_cd.and_then(|cd| cd.last_playbow_tick),
                last_reciprocal_advance_tick: Some(tick),
            });
        }
    }
}
