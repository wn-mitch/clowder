//! C3 subjective belief substrate integrator (ticket 258).
//!
//! Two passes per tick:
//!
//! - **Pass A — Observation**: drain
//!   [`WitnessableEvent`](crate::messages::witnessable_event::WitnessableEvent)
//!   messages emitted by action resolvers; for each event, find cats
//!   within `WITNESS_RANGE` of the event position and apply per-facet EMA
//!   updates to the matching mental-model slots.
//! - **Pass B — Implant + Forgetting**: on every cat's stagger tick
//!   (`entity.index() % decay_stagger_period`), (1) seed missing
//!   [`PredatorBeliefs`] entries from `SpeciesViolencePriors` for any
//!   nearby wildlife (Implant), and (2) decay every facet toward its
//!   `prior` value across all four belief Components (Forgetting on
//!   `strength` → 0 entries are removed).
//!
//! Substrate-only as of 258 — no consumers read facets yet. The four
//! v1 scenarios under `src/scenarios/` assert the EMA + decay shapes
//! directly. Consumer tickets (263–270) wire DSE considerations against
//! facets and earn their own canary entries.

use bevy_ecs::prelude::*;

use crate::components::beliefs::{
    bucket_position, CatBeliefs, ColonyReservesBelief, ContextBeliefs, EnvironmentalContextKey,
    EvidenceKind, Facet, LocationBeliefs, MentalModel, PredatorBeliefs, ReserveBelief,
};
use crate::components::magic::{Inventory, ResourceKind};
use crate::components::physical::{Dead, Position};
use crate::components::wildlife::{WildAnimal, WildSpecies};
use crate::messages::witnessable_event::WitnessableEvent;
use crate::resources::sim_constants::{
    BeliefAxisTunables, BeliefsConstants, SpeciesViolencePriors,
};
use crate::resources::time::TimeState;
use crate::resources::SimConstants;

/// Manhattan-distance witness radius for v1. Mirrors the
/// `social_target_range` scoring constant (10 tiles); consumer tickets
/// can specialize this per event family (audible vs visible) via
/// 244's audible-cue range falloff work.
const WITNESS_RANGE: i32 = 10;

/// Strongest EMA observed-value used when a facet maxes (e.g.
/// `recency_of_threat_cue` on `WitnessedAttack`).
const OBSERVED_MAX: f32 = 1.0;

/// Negative-evidence amplitude for failures (e.g. failed Hunt dampens
/// hunter's perceived-violence-capability). Used as a small downward
/// pull toward 0 rather than the full `OBSERVED_MAX`.
const OBSERVED_FAIL: f32 = 0.0;

/// Half-strength EMA observed-value used when the witness is a third party
/// to a directed cue (e.g. play-bow contributes a weaker `perceived_receptivity`
/// lift than the explicit-accept signal from Groom/Mate; sustained-co-presence
/// from third-party perspective is weaker than the recipient's perspective).
const OBSERVED_HALF: f32 = 0.5;

#[allow(clippy::type_complexity)]
pub fn integrate_beliefs(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut events: MessageReader<WitnessableEvent>,
    mut witnesses: Query<
        (
            Entity,
            &Position,
            &mut CatBeliefs,
            &mut LocationBeliefs,
            &mut PredatorBeliefs,
            &mut ContextBeliefs,
            &mut ColonyReservesBelief,
        ),
        Without<Dead>,
    >,
    wildlife: Query<(Entity, &Position, &WildAnimal), Without<Dead>>,
) {
    let tick = time.tick;
    let cfg = &constants.beliefs;

    // ---- Pass A — Observation -----------------------------------------
    for ev in events.read() {
        let pos = event_position(ev);
        for (witness_ent, witness_pos, mut cats, mut locs, mut preds, mut contexts, mut reserves) in
            witnesses.iter_mut()
        {
            if !within_range(witness_pos, &pos) {
                continue;
            }
            apply_observation(
                ev,
                witness_ent,
                tick,
                cfg,
                &mut cats,
                &mut locs,
                &mut preds,
                &mut contexts,
                &mut reserves,
            );
        }
    }

    // ---- Pass B — Implant + Forgetting --------------------------------
    let period = cfg.decay_stagger_period.max(1);
    let priors = &cfg.species_violence_priors;
    let tick_phase = tick % period;
    for (witness_ent, witness_pos, mut cats, mut locs, mut preds, mut contexts, mut reserves) in
        witnesses.iter_mut()
    {
        if (witness_ent.index_u32() as u64) % period != tick_phase {
            continue;
        }

        // Implant — seed missing PredatorBeliefs for nearby wildlife.
        for (wl_ent, wl_pos, wl) in wildlife.iter() {
            if !within_range(witness_pos, wl_pos) {
                continue;
            }
            preds.models.entry(wl_ent).or_insert_with(|| {
                let prior = species_violence_prior(priors, wl.species);
                MentalModel {
                    perceived_violence_capability: Facet::from_prior(prior),
                    last_updated_tick: tick,
                    ..MentalModel::default()
                }
            });
        }

        // Forgetting — decay all four belief maps. Disjoint `&mut`
        // references, single iteration.
        decay_models(&mut cats.models, tick, cfg, period);
        decay_models(&mut locs.models, tick, cfg, period);
        decay_models(&mut preds.models, tick, cfg, period);
        decay_models(&mut contexts.models, tick, cfg, period);
        decay_reserves(&mut reserves.reserves, cfg);
    }
}

/// 308: per-cat stagger broadcast of the cat's current inventory contents.
/// Narrative framing: "cats gossip about what they're carrying." Implementation:
/// god-eye sensor that emits one `WitnessableEvent::InventoryObserved` per cat
/// per stagger phase tick, integrated by `integrate_beliefs`'s Pass A on the
/// same tick (we schedule this before `integrate_beliefs` in `SimulationPlugin`).
///
/// Self-observation is authoritative; nearby cats integrate the snapshot as
/// additive lower-bound evidence about the colony reserves pool.
pub fn gossip_inventory_observations(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut events: MessageWriter<WitnessableEvent>,
    cats: Query<(Entity, &Position, &Inventory), Without<Dead>>,
) {
    let tick = time.tick;
    let period = constants.beliefs.decay_stagger_period.max(1);
    let tick_phase = tick % period;
    for (entity, pos, inventory) in cats.iter() {
        if (entity.index_u32() as u64) % period != tick_phase {
            continue;
        }
        let mut thornbriar = 0u32;
        let mut remedy = 0u32;
        for slot in &inventory.slots {
            match ResourceKind::from_item_kind(slot.kind) {
                Some(ResourceKind::Thornbriar) => thornbriar += 1,
                Some(ResourceKind::RemedyHerb) => remedy += 1,
                None => {}
            }
        }
        if thornbriar == 0 && remedy == 0 {
            continue;
        }
        let mut payload = Vec::with_capacity(2);
        if thornbriar > 0 {
            payload.push((ResourceKind::Thornbriar, thornbriar));
        }
        if remedy > 0 {
            payload.push((ResourceKind::RemedyHerb, remedy));
        }
        events.write(WitnessableEvent::InventoryObserved {
            actor: entity,
            position: *pos,
            inventory: payload,
            tick,
        });
    }
}

fn event_position(ev: &WitnessableEvent) -> Position {
    match ev {
        WitnessableEvent::Attack { position, .. }
        | WitnessableEvent::Groom { position, .. }
        | WitnessableEvent::Mate { position, .. }
        | WitnessableEvent::Care { position, .. }
        | WitnessableEvent::FleeFrom { position, .. }
        | WitnessableEvent::Hunt { position, .. }
        | WitnessableEvent::ConspecificStartle { position, .. }
        | WitnessableEvent::AmbientShock { position, .. }
        | WitnessableEvent::SelfPlanFailed { position, .. }
        | WitnessableEvent::ReserveDeposited { position, .. }
        | WitnessableEvent::ReserveConsumed { position, .. }
        | WitnessableEvent::InventoryObserved { position, .. }
        | WitnessableEvent::PlayBow { position, .. }
        | WitnessableEvent::ReciprocalAdvance { position, .. }
        | WitnessableEvent::SustainedCoPresence { position, .. } => *position,
    }
}

fn within_range(a: &Position, b: &Position) -> bool {
    (a.x - b.x).abs() + (a.y - b.y).abs() <= WITNESS_RANGE
}

fn species_violence_prior(priors: &SpeciesViolencePriors, species: WildSpecies) -> f32 {
    match species {
        WildSpecies::Fox => priors.fox,
        WildSpecies::Hawk => priors.hawk,
        WildSpecies::Snake => priors.snake,
        WildSpecies::ShadowFox => priors.shadow_fox,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_observation(
    ev: &WitnessableEvent,
    witness: Entity,
    tick: u64,
    cfg: &BeliefsConstants,
    cats: &mut CatBeliefs,
    locs: &mut LocationBeliefs,
    _preds: &mut PredatorBeliefs,
    contexts: &mut ContextBeliefs,
    reserves: &mut ColonyReservesBelief,
) {
    match ev {
        WitnessableEvent::Attack {
            actor,
            target,
            position,
            severity,
            ..
        } => {
            if *actor == witness {
                // Self-witness of own attack — skip (would conflate
                // perceiver/subject; consumer tickets decide whether
                // self-action declarations update self-belief).
                return;
            }
            let actor_model = cats.models.entry(*actor).or_default();
            update_facet(
                &mut actor_model.perceived_violence_capability,
                *severity,
                tick,
                &cfg.perceived_violence_capability,
            );
            update_facet(
                &mut actor_model.recency_of_threat_cue,
                OBSERVED_MAX,
                tick,
                &cfg.recency_of_threat_cue,
            );
            // 261: observed aggression updates witness's hostility-belief
            // about the actor. Scale by severity so a glancing nip lifts
            // less than a deep bite; clamped to [0, 1] by `update_facet`.
            update_facet(
                &mut actor_model.perceived_hostility,
                *severity,
                tick,
                &cfg.perceived_hostility,
            );
            actor_model.last_updated_tick = tick;
            actor_model.evidence_count = actor_model.evidence_count.saturating_add(1);

            if *target != witness {
                let target_model = cats.models.entry(*target).or_default();
                update_facet(
                    &mut target_model.perceived_injury_level,
                    *severity,
                    tick,
                    &cfg.perceived_injury_level,
                );
                target_model.last_updated_tick = tick;
                target_model.evidence_count = target_model.evidence_count.saturating_add(1);
            }
            let loc_key = bucket_position(position.x, position.y);
            let loc_model = locs.models.entry(loc_key).or_default();
            update_facet(
                &mut loc_model.recency_of_threat_cue,
                OBSERVED_MAX,
                tick,
                &cfg.recency_of_threat_cue,
            );
            loc_model.last_updated_tick = tick;
            loc_model.evidence_count = loc_model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::Groom { actor, target, .. }
        | WitnessableEvent::Mate { actor, target, .. } => {
            // Both participants are visibly engaged in affiliative
            // practice: the actor initiated, the target accepted. The
            // witness lifts affiliation_history on the actor (the
            // existing 258 signal) AND lifts perceived_receptivity on
            // *both* participants — 261's addition. Skipping self-witness
            // entries preserves the 258 invariant that own-action
            // declarations don't update own beliefs.
            if *actor != witness {
                let model = cats.models.entry(*actor).or_default();
                update_facet(
                    &mut model.affiliation_history,
                    OBSERVED_MAX,
                    tick,
                    &cfg.affiliation_history,
                );
                update_facet(
                    &mut model.perceived_receptivity,
                    OBSERVED_MAX,
                    tick,
                    &cfg.perceived_receptivity,
                );
                model.last_updated_tick = tick;
                model.evidence_count = model.evidence_count.saturating_add(1);
            }
            if *target != witness {
                let model = cats.models.entry(*target).or_default();
                update_facet(
                    &mut model.perceived_receptivity,
                    OBSERVED_MAX,
                    tick,
                    &cfg.perceived_receptivity,
                );
                model.last_updated_tick = tick;
                model.evidence_count = model.evidence_count.saturating_add(1);
            }
        }

        WitnessableEvent::Care {
            caregiver, kitten, ..
        } => {
            if *caregiver != witness {
                let model = cats.models.entry(*caregiver).or_default();
                update_facet(
                    &mut model.affiliation_history,
                    OBSERVED_MAX,
                    tick,
                    &cfg.affiliation_history,
                );
                update_facet(
                    &mut model.perceived_receptivity,
                    OBSERVED_MAX,
                    tick,
                    &cfg.perceived_receptivity,
                );
                model.last_updated_tick = tick;
                model.evidence_count = model.evidence_count.saturating_add(1);
            }
            // 261: kitten accepting care is itself a receptivity signal.
            if *kitten != witness {
                let model = cats.models.entry(*kitten).or_default();
                update_facet(
                    &mut model.perceived_receptivity,
                    OBSERVED_MAX,
                    tick,
                    &cfg.perceived_receptivity,
                );
                model.last_updated_tick = tick;
                model.evidence_count = model.evidence_count.saturating_add(1);
            }
        }

        WitnessableEvent::FleeFrom {
            fleer,
            threat,
            position,
            ..
        } => {
            if *fleer != witness {
                let fleer_model = cats.models.entry(*fleer).or_default();
                update_facet(
                    &mut fleer_model.predictability,
                    OBSERVED_MAX,
                    tick,
                    &cfg.predictability,
                );
                fleer_model.last_updated_tick = tick;
                fleer_model.evidence_count = fleer_model.evidence_count.saturating_add(1);
            }
            if *threat != witness {
                let threat_model = cats.models.entry(*threat).or_default();
                update_facet(
                    &mut threat_model.perceived_violence_capability,
                    OBSERVED_MAX,
                    tick,
                    &cfg.perceived_violence_capability,
                );
                threat_model.last_updated_tick = tick;
                threat_model.evidence_count = threat_model.evidence_count.saturating_add(1);
            }
            let ctx_model = contexts
                .models
                .entry(EnvironmentalContextKey::HereNow)
                .or_default();
            update_facet(
                &mut ctx_model.recency_of_threat_cue,
                OBSERVED_MAX,
                tick,
                &cfg.recency_of_threat_cue,
            );
            ctx_model.last_updated_tick = tick;
            ctx_model.evidence_count = ctx_model.evidence_count.saturating_add(1);
            let _ = position;
        }

        WitnessableEvent::Hunt {
            hunter, success, ..
        } => {
            if *hunter == witness {
                return;
            }
            let observed = if *success {
                OBSERVED_MAX
            } else {
                OBSERVED_FAIL
            };
            let model = cats.models.entry(*hunter).or_default();
            update_facet(
                &mut model.perceived_violence_capability,
                observed,
                tick,
                &cfg.perceived_violence_capability,
            );
            update_facet(
                &mut model.predictability,
                if *success { OBSERVED_MAX } else { 0.5 },
                tick,
                &cfg.predictability,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::ConspecificStartle {
            startled,
            relay_state,
            ..
        } => {
            // v1: emit the lift on contexts[HereNow] with credibility
            // weight derived solely from relay_state. The full
            // perception-acuity × stoicism × state product lands when
            // ticket 242 ships body-cue substrate (door-slam scenario).
            let credibility = relay_credibility(*relay_state);
            let model = contexts
                .models
                .entry(EnvironmentalContextKey::HereNow)
                .or_default();
            // Scale observed by credibility so a sleepy relay
            // contributes less than an alert one.
            update_facet(
                &mut model.recency_of_threat_cue,
                OBSERVED_MAX * credibility,
                tick,
                &cfg.recency_of_threat_cue,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
            let _ = startled;
        }

        WitnessableEvent::AmbientShock { intensity, .. } => {
            let model = contexts
                .models
                .entry(EnvironmentalContextKey::HereNow)
                .or_default();
            update_facet(
                &mut model.recency_of_threat_cue,
                intensity.clamp(0.0, 1.0),
                tick,
                &cfg.recency_of_threat_cue,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::SelfPlanFailed {
            cat, disposition, ..
        } => {
            // Self-observation: only the cat itself updates its own
            // ContextBeliefs. Other witnesses don't learn from someone
            // else's silent plan-failure — there's no observable cue.
            if *cat != witness {
                return;
            }
            let key = EnvironmentalContextKey::DispositionExecution(*disposition);
            // Predictability's no-observations baseline is 1.0 (reliable),
            // not the `Facet::default()` zero. Seed via `Facet::from_prior(1.0)`
            // on first touch so the EMA step from `value=1.0` toward
            // `OBSERVED_FAIL=0.0` produces a real drop and Pass-B decay
            // recovers toward the correct prior. (290: this seeding is the
            // load-bearing fix that lets the sensor at
            // `plan_substrate::sensors::disposition_cooldown_signal` read a
            // signal that actually recovers between failures.)
            let model = contexts.models.entry(key).or_insert_with(|| MentalModel {
                predictability: Facet::from_prior(1.0),
                ..MentalModel::default()
            });
            update_facet(
                &mut model.predictability,
                OBSERVED_FAIL,
                tick,
                &cfg.predictability,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::ReserveDeposited { kind, .. } => {
            let entry = reserves.reserves.entry(*kind).or_default();
            entry.estimated_count = entry.estimated_count.saturating_add(1);
            bump_reserve_strength(entry, cfg, tick);
        }

        WitnessableEvent::ReserveConsumed { kind, .. } => {
            let entry = reserves.reserves.entry(*kind).or_default();
            entry.estimated_count = entry.estimated_count.saturating_sub(1);
            bump_reserve_strength(entry, cfg, tick);
        }

        WitnessableEvent::InventoryObserved {
            actor, inventory, ..
        } => {
            let is_self = *actor == witness;
            for (kind, count) in inventory {
                let entry = reserves.reserves.entry(*kind).or_default();
                if is_self {
                    // Authoritative replacement — the cat directly knows
                    // what they're holding right now.
                    entry.estimated_count = *count;
                } else {
                    // Additive lower-bound — "I see Mocha is holding 2
                    // thornbriar, so the colony pool is at least 2."
                    entry.estimated_count = entry.estimated_count.max(*count);
                }
                bump_reserve_strength(entry, cfg, tick);
            }
        }

        WitnessableEvent::PlayBow { actor, .. } => {
            // 279: play-bow is the strongest play-engagement cue. Lifts
            // `perceived_intent_clarity` (full strength — the actor is
            // publicly signaling an unambiguous play solicitation) and
            // `perceived_receptivity` at half strength (a solicitation is
            // also a tractable receptivity tell, but weaker than the
            // explicit accept signals from Groom/Mate/Care). Self-witness
            // skipped per the 258 invariant.
            if *actor == witness {
                return;
            }
            let model = cats.models.entry(*actor).or_default();
            update_facet(
                &mut model.perceived_intent_clarity,
                OBSERVED_MAX,
                tick,
                &cfg.perceived_intent_clarity,
            );
            update_facet(
                &mut model.perceived_receptivity,
                OBSERVED_HALF,
                tick,
                &cfg.perceived_receptivity,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::ReciprocalAdvance { actor, target, .. } => {
            // 279: actor advanced toward target after a prior play-bow or
            // reciprocal-advance. When `target == witness`, this is "they
            // advanced toward *me*" — full-strength intent-clarity lift.
            // Third-party witnesses lift at half strength (still observable,
            // but the recipient gets the cleanest signal).
            if *actor == witness {
                return;
            }
            let model = cats.models.entry(*actor).or_default();
            let observed = if *target == witness {
                OBSERVED_MAX
            } else {
                OBSERVED_HALF
            };
            update_facet(
                &mut model.perceived_intent_clarity,
                observed,
                tick,
                &cfg.perceived_intent_clarity,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::SustainedCoPresence {
            actor,
            target,
            ticks_held,
            ..
        } => {
            // 279: continuous in-range duration. Lift scales by
            // `ticks_held / saturation_ticks` — short windows produce
            // weak evidence, long windows saturate at OBSERVED_MAX. When
            // `target == witness`, lift at full scaled strength (the
            // co-presence is *with me*); third-party witnesses lift at
            // half the scaled value.
            if *actor == witness {
                return;
            }
            let saturation = cfg.sustained_copresence_saturation_ticks.max(1) as f32;
            let scale = (*ticks_held as f32 / saturation).clamp(0.0, 1.0);
            let observed = if *target == witness {
                OBSERVED_MAX * scale
            } else {
                OBSERVED_HALF * scale
            };
            let model = cats.models.entry(*actor).or_default();
            update_facet(
                &mut model.perceived_intent_clarity,
                observed,
                tick,
                &cfg.perceived_intent_clarity,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }
    }
}

fn bump_reserve_strength(entry: &mut ReserveBelief, cfg: &BeliefsConstants, tick: u64) {
    entry.strength = (entry.strength + cfg.reserve_strength_per_observation).min(1.0);
    entry.last_source = EvidenceKind::Observation;
    entry.last_updated_tick = tick;
}

fn decay_reserves(
    map: &mut std::collections::HashMap<ResourceKind, ReserveBelief>,
    cfg: &BeliefsConstants,
) {
    map.retain(|_kind, rb| {
        rb.strength = (rb.strength - cfg.reserve_decay_per_stagger).max(0.0);
        if rb.strength <= f32::EPSILON {
            rb.last_source = EvidenceKind::Forgetting;
            false
        } else {
            true
        }
    });
}

fn relay_credibility(state: crate::messages::witnessable_event::RelayState) -> f32 {
    use crate::messages::witnessable_event::RelayState;
    match state {
        RelayState::Sleeping => 0.3,
        RelayState::Resting => 0.5,
        RelayState::Alert => 0.9,
        RelayState::Engaged => 0.7,
    }
}

/// Apply one EMA step to a facet. Bumps strength and records the source.
fn update_facet(facet: &mut Facet, observed: f32, tick: u64, tun: &BeliefAxisTunables) {
    facet.value += tun.learning_rate * (observed - facet.value);
    facet.strength = (facet.strength + tun.strength_per_observation).min(1.0);
    facet.last_source = EvidenceKind::Observation;
    facet.last_updated_tick = tick;
}

fn decay_models<K: std::hash::Hash + Eq + Copy>(
    map: &mut std::collections::HashMap<K, MentalModel>,
    tick: u64,
    cfg: &BeliefsConstants,
    period: u64,
) {
    map.retain(|_k, model| {
        decay_facet(
            &mut model.perceived_injury_level,
            &cfg.perceived_injury_level,
            period,
        );
        decay_facet(
            &mut model.perceived_intent_clarity,
            &cfg.perceived_intent_clarity,
            period,
        );
        decay_facet(
            &mut model.recency_of_threat_cue,
            &cfg.recency_of_threat_cue,
            period,
        );
        decay_facet(
            &mut model.perceived_violence_capability,
            &cfg.perceived_violence_capability,
            period,
        );
        decay_facet(
            &mut model.affiliation_history,
            &cfg.affiliation_history,
            period,
        );
        decay_facet(&mut model.predictability, &cfg.predictability, period);
        decay_facet(
            &mut model.perceived_hostility,
            &cfg.perceived_hostility,
            period,
        );
        decay_facet(
            &mut model.perceived_receptivity,
            &cfg.perceived_receptivity,
            period,
        );
        model.last_updated_tick = tick;
        let max_strength = [
            model.perceived_injury_level.strength,
            model.perceived_intent_clarity.strength,
            model.recency_of_threat_cue.strength,
            model.perceived_violence_capability.strength,
            model.affiliation_history.strength,
            model.predictability.strength,
            model.perceived_hostility.strength,
            model.perceived_receptivity.strength,
        ]
        .into_iter()
        .fold(0.0f32, f32::max);
        max_strength > f32::EPSILON
    });
}

fn decay_facet(facet: &mut Facet, tun: &BeliefAxisTunables, period: u64) {
    let period_f = period as f32;
    facet.value += tun.decay_rate_to_prior * period_f * (facet.prior - facet.value);
    facet.strength = (facet.strength - tun.strength_decay_per_tick * period_f).max(0.0);
    if facet.strength <= f32::EPSILON {
        facet.last_source = EvidenceKind::Forgetting;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::beliefs::{
        CatBeliefs, ColonyReservesBelief, ContextBeliefs, LocationBeliefs, PredatorBeliefs,
    };
    use crate::components::physical::{Health, Needs, Position};
    use crate::components::wildlife::WildAnimal;
    use crate::messages::witnessable_event::WitnessableEvent;
    use crate::resources::time::TimeState;
    use crate::resources::SimConstants;
    use bevy_ecs::schedule::Schedule;

    fn test_world(tick: u64) -> (World, Schedule) {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = tick;
        world.insert_resource(time);
        world.insert_resource(SimConstants::default());
        bevy_ecs::message::MessageRegistry::register_message::<WitnessableEvent>(&mut world);
        let mut schedule = Schedule::default();
        schedule.add_systems(bevy_ecs::message::message_update_system);
        schedule.add_systems(integrate_beliefs);
        (world, schedule)
    }

    fn spawn_cat(world: &mut World, position: Position) -> Entity {
        world
            .spawn((
                position,
                Health::default(),
                Needs::default(),
                CatBeliefs::default(),
                LocationBeliefs::default(),
                PredatorBeliefs::default(),
                ContextBeliefs::default(),
                ColonyReservesBelief::default(),
            ))
            .id()
    }

    fn spawn_wildlife(world: &mut World, species: WildSpecies, position: Position) -> Entity {
        world
            .spawn((WildAnimal::new(species), position, Health::default()))
            .id()
    }

    #[test]
    fn groom_event_lifts_witness_affiliation() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Groom {
            actor,
            target,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world
            .get::<CatBeliefs>(witness)
            .expect("witness has CatBeliefs");
        let model = cats
            .models
            .get(&actor)
            .expect("witness should hold belief about actor");
        assert!(
            model.affiliation_history.value > 0.0,
            "affiliation_history should lift after WitnessedGroom; got {}",
            model.affiliation_history.value
        );
        assert!(model.affiliation_history.strength > 0.0);
        assert_eq!(
            model.affiliation_history.last_source,
            EvidenceKind::Observation
        );
    }

    #[test]
    fn out_of_range_witnesses_dont_update() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(0, 0));
        let target = spawn_cat(&mut world, Position::new(1, 0));
        let far_witness = spawn_cat(&mut world, Position::new(50, 50));

        world.write_message(WitnessableEvent::Groom {
            actor,
            target,
            position: Position::new(0, 0),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(far_witness).expect("far_witness");
        assert!(
            cats.models.is_empty(),
            "far-out-of-range witnesses should not update beliefs"
        );
    }

    #[test]
    fn implant_seeds_predator_belief_on_first_encounter() {
        let (mut world, mut schedule) = test_world(0);
        let cat = spawn_cat(&mut world, Position::new(0, 0));
        let fox = spawn_wildlife(&mut world, WildSpecies::ShadowFox, Position::new(2, 0));

        // Run enough ticks (≥ `decay_stagger_period`) to guarantee
        // the cat's stagger phase is hit at least once.
        let period = SimConstants::default().beliefs.decay_stagger_period;
        for _ in 0..(period + 1) {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let preds = world.get::<PredatorBeliefs>(cat).unwrap();
        let model = preds
            .models
            .get(&fox)
            .expect("PredatorBeliefs should seed an entry on first encounter");
        let expected = SimConstants::default()
            .beliefs
            .species_violence_priors
            .shadow_fox;
        assert!(
            (model.perceived_violence_capability.value - expected).abs() < 1e-5,
            "ShadowFox prior should be {expected}; got {}",
            model.perceived_violence_capability.value
        );
        assert_eq!(
            model.perceived_violence_capability.last_source,
            EvidenceKind::Implant
        );
    }

    // 295 — emit-site coverage. Each test fires one new variant and
    // asserts the integrator updates the documented facet. Out-of-range
    // and self-witness gating is already covered by `Groom` tests above;
    // these focus on the per-variant facet path.

    #[test]
    fn mate_event_lifts_witness_affiliation() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Mate {
            actor,
            target,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let model = cats
            .models
            .get(&actor)
            .expect("witness holds belief on mating actor");
        assert!(
            model.affiliation_history.value > 0.0,
            "Mate should lift actor's affiliation_history on witnesses"
        );
        assert_eq!(
            model.affiliation_history.last_source,
            EvidenceKind::Observation
        );
    }

    #[test]
    fn care_event_lifts_witness_affiliation_on_caregiver() {
        let (mut world, mut schedule) = test_world(100);
        let caregiver = spawn_cat(&mut world, Position::new(10, 10));
        let kitten = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Care {
            caregiver,
            kitten,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let model = cats
            .models
            .get(&caregiver)
            .expect("witness holds belief on caregiver");
        assert!(
            model.affiliation_history.value > 0.0,
            "Care should lift caregiver's affiliation_history on witnesses"
        );
    }

    #[test]
    fn hunt_success_lifts_hunter_violence_and_predictability() {
        let (mut world, mut schedule) = test_world(100);
        let hunter = spawn_cat(&mut world, Position::new(10, 10));
        let witness = spawn_cat(&mut world, Position::new(11, 10));

        world.write_message(WitnessableEvent::Hunt {
            hunter,
            prey_kind: crate::components::prey::PreyKind::Mouse,
            position: Position::new(10, 10),
            success: true,
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let model = cats
            .models
            .get(&hunter)
            .expect("witness holds belief on hunter");
        assert!(
            model.perceived_violence_capability.value > 0.0,
            "Hunt success should lift hunter's perceived_violence_capability"
        );
        assert!(
            model.predictability.value > 0.0,
            "Hunt success should lift hunter's predictability"
        );
    }

    #[test]
    fn flee_from_event_lifts_fleer_predictability_and_threat_violence() {
        let (mut world, mut schedule) = test_world(100);
        let fleer = spawn_cat(&mut world, Position::new(10, 10));
        let threat = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::FleeFrom {
            fleer,
            threat,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let fleer_model = cats
            .models
            .get(&fleer)
            .expect("witness holds belief on fleer");
        assert!(
            fleer_model.predictability.value > 0.0,
            "FleeFrom should lift fleer's predictability on witnesses"
        );
        let threat_model = cats
            .models
            .get(&threat)
            .expect("witness holds belief on threat");
        assert!(
            threat_model.perceived_violence_capability.value > 0.0,
            "FleeFrom should lift threat's perceived_violence_capability on witnesses"
        );
    }

    // 261 — perceived_hostility + perceived_receptivity emit-site coverage.

    #[test]
    fn attack_event_lifts_hostility_on_actor() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Attack {
            actor,
            target,
            position: Position::new(10, 10),
            severity: 0.8,
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let model = cats
            .models
            .get(&actor)
            .expect("witness holds belief on attacker");
        assert!(
            model.perceived_hostility.value > 0.0,
            "Attack should lift actor's perceived_hostility on witnesses; got {}",
            model.perceived_hostility.value
        );
        assert_eq!(
            model.perceived_hostility.last_source,
            EvidenceKind::Observation
        );
    }

    #[test]
    fn groom_event_lifts_receptivity_on_both_participants() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Groom {
            actor,
            target,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let actor_model = cats.models.get(&actor).expect("belief on actor");
        let target_model = cats.models.get(&target).expect("belief on target");
        assert!(
            actor_model.perceived_receptivity.value > 0.0,
            "Groom should lift actor receptivity; got {}",
            actor_model.perceived_receptivity.value
        );
        assert!(
            target_model.perceived_receptivity.value > 0.0,
            "Groom should lift target receptivity; got {}",
            target_model.perceived_receptivity.value
        );
    }

    #[test]
    fn mate_event_lifts_receptivity_on_both_participants() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Mate {
            actor,
            target,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        assert!(cats.models.get(&actor).unwrap().perceived_receptivity.value > 0.0);
        assert!(
            cats.models
                .get(&target)
                .unwrap()
                .perceived_receptivity
                .value
                > 0.0
        );
    }

    #[test]
    fn care_event_lifts_receptivity_on_caregiver_and_kitten() {
        let (mut world, mut schedule) = test_world(100);
        let caregiver = spawn_cat(&mut world, Position::new(10, 10));
        let kitten = spawn_cat(&mut world, Position::new(11, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Care {
            caregiver,
            kitten,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        assert!(
            cats.models
                .get(&caregiver)
                .unwrap()
                .perceived_receptivity
                .value
                > 0.0,
            "Care should lift caregiver receptivity"
        );
        assert!(
            cats.models
                .get(&kitten)
                .unwrap()
                .perceived_receptivity
                .value
                > 0.0,
            "Care should lift kitten receptivity"
        );
    }

    #[test]
    fn passive_decay_pulls_value_toward_prior() {
        let (mut world, mut schedule) = test_world(20);
        let cat = spawn_cat(&mut world, Position::new(0, 0));
        // Spawn a second cat to use as the subject of the preloaded
        // belief — avoids manual Entity construction (Bevy 0.18
        // tightened `Entity::from_raw` away).
        let stub_subject = spawn_cat(&mut world, Position::new(80, 80));

        let mut model = MentalModel::default();
        model.affiliation_history = Facet {
            value: 0.9,
            prior: 0.0,
            strength: 1.0,
            last_source: EvidenceKind::Observation,
            last_updated_tick: 0,
        };
        world
            .get_mut::<CatBeliefs>(cat)
            .unwrap()
            .models
            .insert(stub_subject, model);

        // Tick 20 happens to be the stagger tick for entity.index() % 20
        // depending on which entity index the cat got — not guaranteed.
        // Run repeatedly until decay actually fires (any tick that
        // matches the cat's stagger phase).
        for _ in 0..40 {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let cats = world.get::<CatBeliefs>(cat).unwrap();
        // After ≥1 decay step toward prior = 0, value should have dropped.
        if let Some(model) = cats.models.get(&stub_subject) {
            assert!(
                model.affiliation_history.value < 0.9,
                "passive decay should pull value below the initial 0.9; got {}",
                model.affiliation_history.value
            );
        }
        // If the entry was removed, decay also achieved its goal
        // (strength hit zero → Forgetting).
    }

    // ---------------------------------------------------------------------
    // 308 — ColonyReservesBelief integrator coverage
    // ---------------------------------------------------------------------

    #[test]
    fn reserve_deposited_lifts_witness_count() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let witness = spawn_cat(&mut world, Position::new(11, 10));

        world.write_message(WitnessableEvent::ReserveDeposited {
            actor,
            kind: ResourceKind::Thornbriar,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        for (label, ent) in [("actor", actor), ("witness", witness)] {
            let belief = world
                .get::<ColonyReservesBelief>(ent)
                .expect("cat has ColonyReservesBelief");
            let entry = belief
                .reserves
                .get(&ResourceKind::Thornbriar)
                .unwrap_or_else(|| panic!("{label} should hold a Thornbriar reserve belief"));
            assert_eq!(entry.estimated_count, 1, "{label} count should bump to 1");
            assert!(entry.strength > 0.0, "{label} strength should lift");
        }
    }

    #[test]
    fn reserve_consumed_decrements_count_saturating() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        // Pre-seed a belief at count=2 so the decrement has somewhere to go.
        {
            let mut belief = world.get_mut::<ColonyReservesBelief>(actor).unwrap();
            belief.reserves.insert(
                ResourceKind::Thornbriar,
                crate::components::beliefs::ReserveBelief {
                    estimated_count: 2,
                    strength: 1.0,
                    last_source: EvidenceKind::Observation,
                    last_updated_tick: 90,
                },
            );
        }
        world.write_message(WitnessableEvent::ReserveConsumed {
            actor,
            kind: ResourceKind::Thornbriar,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        let belief = world.get::<ColonyReservesBelief>(actor).unwrap();
        let entry = belief.reserves.get(&ResourceKind::Thornbriar).unwrap();
        assert_eq!(entry.estimated_count, 1);
    }

    #[test]
    fn inventory_observed_self_replaces_count() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        // Pre-seed a stale belief that overestimates.
        {
            let mut belief = world.get_mut::<ColonyReservesBelief>(actor).unwrap();
            belief.reserves.insert(
                ResourceKind::Thornbriar,
                crate::components::beliefs::ReserveBelief {
                    estimated_count: 5,
                    strength: 1.0,
                    last_source: EvidenceKind::Observation,
                    last_updated_tick: 90,
                },
            );
        }
        // Self-observation: I'm holding only 2.
        world.write_message(WitnessableEvent::InventoryObserved {
            actor,
            position: Position::new(10, 10),
            inventory: vec![(ResourceKind::Thornbriar, 2)],
            tick: 100,
        });
        schedule.run(&mut world);

        let belief = world.get::<ColonyReservesBelief>(actor).unwrap();
        let entry = belief.reserves.get(&ResourceKind::Thornbriar).unwrap();
        assert_eq!(
            entry.estimated_count, 2,
            "self-observation is authoritative — should replace the stale 5 with 2"
        );
    }

    #[test]
    fn inventory_observed_other_takes_lower_bound_max() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let witness = spawn_cat(&mut world, Position::new(11, 10));
        // Witness already believes the colony has 3 thornbriar.
        {
            let mut belief = world.get_mut::<ColonyReservesBelief>(witness).unwrap();
            belief.reserves.insert(
                ResourceKind::Thornbriar,
                crate::components::beliefs::ReserveBelief {
                    estimated_count: 3,
                    strength: 1.0,
                    last_source: EvidenceKind::Observation,
                    last_updated_tick: 90,
                },
            );
        }
        // Actor announces holding 2.
        world.write_message(WitnessableEvent::InventoryObserved {
            actor,
            position: Position::new(10, 10),
            inventory: vec![(ResourceKind::Thornbriar, 2)],
            tick: 100,
        });
        schedule.run(&mut world);

        let belief = world.get::<ColonyReservesBelief>(witness).unwrap();
        let entry = belief.reserves.get(&ResourceKind::Thornbriar).unwrap();
        assert_eq!(
            entry.estimated_count, 3,
            "additive lower-bound: max(existing 3, witnessed 2) = 3"
        );
    }

    // ---------------------------------------------------------------------
    // 290 — SelfPlanFailed predictability EMA trajectory
    //
    // These tests assert the load-bearing shape that
    // `plan_substrate::sensors::disposition_cooldown_signal` reads. The
    // first failure must snap predictability.value to 0.0 (matches the
    // legacy RDF `age=0 → 0.0` contract), and Pass-B decay must recover
    // toward `prior = 1.0`. Without the `Facet::from_prior(1.0)` seed at
    // entry-creation, both invariants silently fail (value pins at 0.0).
    // ---------------------------------------------------------------------

    #[test]
    fn self_plan_failed_snaps_predictability_to_zero_on_first_failure() {
        use crate::components::DispositionKind;

        let (mut world, mut schedule) = test_world(100);
        let cat = spawn_cat(&mut world, Position::new(10, 10));

        world.write_message(WitnessableEvent::SelfPlanFailed {
            cat,
            disposition: DispositionKind::Hunting,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        let beliefs = world
            .get::<ContextBeliefs>(cat)
            .expect("cat has ContextBeliefs");
        let model = beliefs
            .models
            .get(&EnvironmentalContextKey::DispositionExecution(
                DispositionKind::Hunting,
            ))
            .expect("SelfPlanFailed should seed a DispositionExecution(Hunting) model");
        // Pass A snaps `value` 1.0 → 0.0 with lr=1.0. Pass B may fire
        // within the same `schedule.run` when the cat's entity index is
        // stagger-aligned with the tick, adding one decay step
        // (`decay_rate_to_prior=0.00075` × `period=20` × gap=1.0 = 0.015).
        // Bound the assertion to validate the snap contract without
        // depending on Bevy's nondeterministic entity-index assignment.
        assert!(
            model.predictability.value < 0.05,
            "single SelfPlanFailed event with lr=1.0 must snap value to ~0.0 \
             (allowing one Pass-B decay step); got {}",
            model.predictability.value,
        );
        assert_eq!(
            model.predictability.prior, 1.0,
            "predictability prior should be seeded to 1.0 (no-observations baseline)"
        );
        assert!(model.predictability.strength > 0.0);
        assert_eq!(model.evidence_count, 1);
    }

    #[test]
    fn self_plan_failed_predictability_recovers_toward_prior_via_passive_decay() {
        use crate::components::DispositionKind;

        let (mut world, mut schedule) = test_world(100);
        let cat = spawn_cat(&mut world, Position::new(10, 10));

        world.write_message(WitnessableEvent::SelfPlanFailed {
            cat,
            disposition: DispositionKind::Foraging,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        // Sanity: post-failure value is at or near 0.0 (see snap test
        // for the Pass-B-alignment caveat).
        let value_after_failure = world
            .get::<ContextBeliefs>(cat)
            .unwrap()
            .models
            .get(&EnvironmentalContextKey::DispositionExecution(
                DispositionKind::Foraging,
            ))
            .unwrap()
            .predictability
            .value;
        assert!(value_after_failure < 0.05);

        // Run for enough ticks to cross several stagger periods (Pass B
        // fires once per `decay_stagger_period` per cat). The default
        // period is 20; 400 ticks → ≥20 passes regardless of phase.
        for _ in 0..400 {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let beliefs = world.get::<ContextBeliefs>(cat).unwrap();
        let model = beliefs
            .models
            .get(&EnvironmentalContextKey::DispositionExecution(
                DispositionKind::Foraging,
            ))
            .expect("model should still exist (strength has not decayed to zero in 400 ticks)");
        assert!(
            model.predictability.value > value_after_failure,
            "Pass B decay should pull value above the post-failure baseline {} \
             toward prior=1.0; got {}",
            value_after_failure,
            model.predictability.value,
        );
        assert!(
            model.predictability.value < 1.0,
            "400 ticks is not long enough for full recovery toward prior=1.0; got {}",
            model.predictability.value
        );
    }

    // 279 — play-engagement cue coverage. Each test fires one new variant
    // and asserts the integrator lifts `perceived_intent_clarity` (and, for
    // PlayBow, `perceived_receptivity`) on the witness's model of the actor.

    #[test]
    fn playbow_event_lifts_witness_intent_clarity_and_receptivity() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let witness = spawn_cat(&mut world, Position::new(11, 10));

        world.write_message(WitnessableEvent::PlayBow {
            actor,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let cats = world.get::<CatBeliefs>(witness).unwrap();
        let model = cats
            .models
            .get(&actor)
            .expect("witness holds belief on play-bowing actor");
        assert!(
            model.perceived_intent_clarity.value > 0.0,
            "PlayBow should lift perceived_intent_clarity; got {}",
            model.perceived_intent_clarity.value
        );
        assert_eq!(
            model.perceived_intent_clarity.last_source,
            EvidenceKind::Observation
        );
        // Receptivity also lifts, but at half strength — its EMA step from a
        // 0.5 observation is strictly smaller than the intent-clarity step
        // from a 1.0 observation under the same axis math, so receptivity
        // ends below intent-clarity is NOT guaranteed (different axes), but
        // receptivity must be strictly positive.
        assert!(
            model.perceived_receptivity.value > 0.0,
            "PlayBow should lift perceived_receptivity; got {}",
            model.perceived_receptivity.value
        );
    }

    #[test]
    fn playbow_self_witness_skips_facet_update() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));

        world.write_message(WitnessableEvent::PlayBow {
            actor,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        // The actor is its own witness (in range of its own position) but
        // must not form a belief about itself — preserves the 258 invariant
        // that own-action declarations don't update own beliefs.
        let cats = world.get::<CatBeliefs>(actor).unwrap();
        assert!(
            !cats.models.contains_key(&actor),
            "self-witness of own PlayBow must not seed a self-belief"
        );
    }

    #[test]
    fn reciprocal_advance_target_self_lifts_more_than_third_party() {
        // Witness == target: "they advanced toward me" → full-strength lift.
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target_witness = spawn_cat(&mut world, Position::new(11, 10));

        world.write_message(WitnessableEvent::ReciprocalAdvance {
            actor,
            target: target_witness,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        let self_lift = world
            .get::<CatBeliefs>(target_witness)
            .unwrap()
            .models
            .get(&actor)
            .expect("target witness holds belief on advancing actor")
            .perceived_intent_clarity
            .value;

        // Third-party witness: same event, witness is neither actor nor
        // target → half-strength lift.
        let (mut world2, mut schedule2) = test_world(100);
        let actor2 = spawn_cat(&mut world2, Position::new(10, 10));
        let target2 = spawn_cat(&mut world2, Position::new(11, 10));
        let third_party = spawn_cat(&mut world2, Position::new(12, 10));

        world2.write_message(WitnessableEvent::ReciprocalAdvance {
            actor: actor2,
            target: target2,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule2.run(&mut world2);

        let third_party_lift = world2
            .get::<CatBeliefs>(third_party)
            .unwrap()
            .models
            .get(&actor2)
            .expect("third-party witness holds belief on advancing actor")
            .perceived_intent_clarity
            .value;

        assert!(
            self_lift > third_party_lift,
            "recipient lift ({self_lift}) should exceed third-party lift ({third_party_lift})"
        );
        assert!(
            third_party_lift > 0.0,
            "third-party lift should be positive"
        );
    }

    #[test]
    fn sustained_copresence_scales_by_ticks_held() {
        // Short window → small lift; long (saturated) window → larger lift.
        let saturation = SimConstants::default()
            .beliefs
            .sustained_copresence_saturation_ticks;

        let lift_for = |ticks_held: u32| -> f32 {
            let (mut world, mut schedule) = test_world(100);
            let actor = spawn_cat(&mut world, Position::new(10, 10));
            let witness = spawn_cat(&mut world, Position::new(11, 10));
            world.write_message(WitnessableEvent::SustainedCoPresence {
                actor,
                target: witness,
                ticks_held,
                position: Position::new(10, 10),
                tick: 100,
            });
            schedule.run(&mut world);
            world
                .get::<CatBeliefs>(witness)
                .unwrap()
                .models
                .get(&actor)
                .expect("witness holds belief on co-present actor")
                .perceived_intent_clarity
                .value
        };

        let short = lift_for(saturation / 4);
        let saturated = lift_for(saturation);
        assert!(
            saturated > short,
            "saturated co-presence lift ({saturated}) should exceed short-window lift ({short})"
        );
        assert!(short > 0.0, "short-window lift should still be positive");
    }

    #[test]
    fn sustained_copresence_target_self_lifts_more_than_third_party() {
        let saturation = SimConstants::default()
            .beliefs
            .sustained_copresence_saturation_ticks;

        // Witness == target.
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target_witness = spawn_cat(&mut world, Position::new(11, 10));
        world.write_message(WitnessableEvent::SustainedCoPresence {
            actor,
            target: target_witness,
            ticks_held: saturation,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);
        let self_lift = world
            .get::<CatBeliefs>(target_witness)
            .unwrap()
            .models
            .get(&actor)
            .unwrap()
            .perceived_intent_clarity
            .value;

        // Third-party witness.
        let (mut world2, mut schedule2) = test_world(100);
        let actor2 = spawn_cat(&mut world2, Position::new(10, 10));
        let target2 = spawn_cat(&mut world2, Position::new(11, 10));
        let third_party = spawn_cat(&mut world2, Position::new(12, 10));
        world2.write_message(WitnessableEvent::SustainedCoPresence {
            actor: actor2,
            target: target2,
            ticks_held: saturation,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule2.run(&mut world2);
        let third_party_lift = world2
            .get::<CatBeliefs>(third_party)
            .unwrap()
            .models
            .get(&actor2)
            .unwrap()
            .perceived_intent_clarity
            .value;

        assert!(
            self_lift > third_party_lift,
            "recipient lift ({self_lift}) should exceed third-party lift ({third_party_lift})"
        );
        assert!(third_party_lift > 0.0);
    }
}
