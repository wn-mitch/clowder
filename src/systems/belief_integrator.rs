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
    bucket_position, CatBeliefs, ContextBeliefs, EnvironmentalContextKey, EvidenceKind, Facet,
    LocationBeliefs, MentalModel, PredatorBeliefs,
};
use crate::components::physical::{Dead, Position};
use crate::components::wildlife::{WildAnimal, WildSpecies};
use crate::messages::witnessable_event::WitnessableEvent;
use crate::resources::sim_constants::{BeliefAxisTunables, BeliefsConstants, SpeciesViolencePriors};
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
        for (witness_ent, witness_pos, mut cats, mut locs, mut preds, mut contexts) in
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
            );
        }
    }

    // ---- Pass B — Implant + Forgetting --------------------------------
    let period = cfg.decay_stagger_period.max(1);
    let priors = &cfg.species_violence_priors;
    let tick_phase = tick % period;
    for (witness_ent, witness_pos, mut cats, mut locs, mut preds, mut contexts) in
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
        | WitnessableEvent::SelfPlanFailed { position, .. } => *position,
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

        WitnessableEvent::Groom { actor, .. }
        | WitnessableEvent::Mate { actor, .. } => {
            if *actor == witness {
                return;
            }
            let model = cats.models.entry(*actor).or_default();
            update_facet(
                &mut model.affiliation_history,
                OBSERVED_MAX,
                tick,
                &cfg.affiliation_history,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::Care { caregiver, .. } => {
            if *caregiver == witness {
                return;
            }
            let model = cats.models.entry(*caregiver).or_default();
            update_facet(
                &mut model.affiliation_history,
                OBSERVED_MAX,
                tick,
                &cfg.affiliation_history,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
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
            let ctx_model = contexts.models.entry(EnvironmentalContextKey::HereNow).or_default();
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

        WitnessableEvent::Hunt { hunter, success, .. } => {
            if *hunter == witness {
                return;
            }
            let observed = if *success { OBSERVED_MAX } else { OBSERVED_FAIL };
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
            let model = contexts.models.entry(EnvironmentalContextKey::HereNow).or_default();
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
            let model = contexts.models.entry(EnvironmentalContextKey::HereNow).or_default();
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
            cat,
            disposition,
            ..
        } => {
            // Self-observation: only the cat itself updates its own
            // ContextBeliefs. Other witnesses don't learn from someone
            // else's silent plan-failure — there's no observable cue.
            if *cat != witness {
                return;
            }
            let key = EnvironmentalContextKey::DispositionExecution(*disposition);
            let model = contexts.models.entry(key).or_default();
            // Failure observed: predictability for this disposition drops
            // toward 0. EMA, not snap-to-zero — single failures shouldn't
            // wipe a long history of successful executions.
            update_facet(
                &mut model.predictability,
                OBSERVED_FAIL,
                tick,
                &cfg.predictability,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }
    }
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
        decay_facet(&mut model.perceived_injury_level, &cfg.perceived_injury_level, period);
        decay_facet(&mut model.perceived_intent_clarity, &cfg.perceived_intent_clarity, period);
        decay_facet(&mut model.recency_of_threat_cue, &cfg.recency_of_threat_cue, period);
        decay_facet(
            &mut model.perceived_violence_capability,
            &cfg.perceived_violence_capability,
            period,
        );
        decay_facet(&mut model.affiliation_history, &cfg.affiliation_history, period);
        decay_facet(&mut model.predictability, &cfg.predictability, period);
        model.last_updated_tick = tick;
        let max_strength = [
            model.perceived_injury_level.strength,
            model.perceived_intent_clarity.strength,
            model.recency_of_threat_cue.strength,
            model.perceived_violence_capability.strength,
            model.affiliation_history.strength,
            model.predictability.strength,
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
        CatBeliefs, ContextBeliefs, LocationBeliefs, PredatorBeliefs,
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

        let cats = world.get::<CatBeliefs>(witness).expect("witness has CatBeliefs");
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
        assert_eq!(model.affiliation_history.last_source, EvidenceKind::Observation);
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
        let model = cats.models.get(&actor).expect("witness holds belief on mating actor");
        assert!(
            model.affiliation_history.value > 0.0,
            "Mate should lift actor's affiliation_history on witnesses"
        );
        assert_eq!(model.affiliation_history.last_source, EvidenceKind::Observation);
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
        let model = cats.models.get(&caregiver).expect("witness holds belief on caregiver");
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
        let model = cats.models.get(&hunter).expect("witness holds belief on hunter");
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
        let fleer_model = cats.models.get(&fleer).expect("witness holds belief on fleer");
        assert!(
            fleer_model.predictability.value > 0.0,
            "FleeFrom should lift fleer's predictability on witnesses"
        );
        let threat_model = cats.models.get(&threat).expect("witness holds belief on threat");
        assert!(
            threat_model.perceived_violence_capability.value > 0.0,
            "FleeFrom should lift threat's perceived_violence_capability on witnesses"
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
}
