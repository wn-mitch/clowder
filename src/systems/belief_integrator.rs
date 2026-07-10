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
//!   `strength` → 0 entries are removed). Wildlife (265) and prey
//!   (314) run symmetric Pass-B loops on the same stagger discipline:
//!   wildlife implant cat models into their `CatBeliefs` from the
//!   wildlife-perceiver prior rows; prey implant threat models (cats
//!   AND wildlife) into their `PredatorBeliefs` from the
//!   prey-perceiver rows. Prey get no Pass-A observation subset.
//!
//! Substrate-only as of 258 — no consumers read facets yet. The four
//! v1 scenarios under `src/scenarios/` assert the EMA + decay shapes
//! directly. Consumer tickets (263–270) wire DSE considerations against
//! facets and earn their own canary entries.

use bevy_ecs::prelude::*;

use crate::components::beliefs::{
    bucket_position, CatBeliefs, ColonyReservesBelief, ContextBeliefs, EnvironmentalContextKey,
    EvidenceKind, Facet, LocationBeliefs, MentalModel, PredatorBeliefs, ReserveBelief,
    ShelterBeliefs,
};
use crate::components::magic::{Inventory, ResourceKind};
use crate::components::physical::{Dead, Position};
use crate::components::prey::PreyAnimal;
use crate::components::wildlife::{WildAnimal, WildSpecies};
use crate::messages::witnessable_event::WitnessableEvent;
use crate::resources::sim_constants::{
    BeliefAxisTunables, BeliefsConstants, ShelterBeliefConstants, SpeciesViolencePriors,
};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeState;
use crate::resources::GroundSurplusMap;
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

/// 293: EMA observed-value for `WitnessableEvent::HuntScentDetected` — sits
/// between neutral (0.5) and `OBSERVED_MAX` so scent yields a mild positive
/// lift, matching the legacy ratio of `record_scent` (+0.05) to
/// `record_catch` (+0.15) ≈ 1/3 of the catch lift.
const SCENT_OBSERVED_VALUE: f32 = 0.65;

/// 265 activation: EMA observed-value for the FleeFrom
/// witness-learns-threat-is-violent write into `PredatorBeliefs`.
/// Watching a colony-mate run from something is real but indirect
/// evidence of its violence — above neutral (0.5), below a witnessed
/// Attack (`OBSERVED_MAX`), same in-between class as
/// `SCENT_OBSERVED_VALUE`. Also sits at the fox/hawk flee-eligibility
/// threshold, so flee cues alone can propagate fear to the edge of
/// eligibility but witnessed violence is needed to push past it.
const FLEE_CUE_OBSERVED_VALUE: f32 = 0.75;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
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
            &mut ShelterBeliefs,
        ),
        (Without<Dead>, Without<WildAnimal>, Without<PreyAnimal>),
    >,
    wildlife: Query<(Entity, &Position, &WildAnimal), Without<Dead>>,
    // 265: wildlife witnesses — every WildAnimal carries CatBeliefs
    // (required component) holding its own mental models of cats.
    // Disjoint from the cat `witnesses` query via the paired
    // `With<WildAnimal>` / `Without<WildAnimal>` filters.
    mut wildlife_witnesses: Query<
        (Entity, &Position, &WildAnimal, &mut CatBeliefs),
        (With<WildAnimal>, Without<Dead>),
    >,
    // 314: prey witnesses — every PreyAnimal carries PredatorBeliefs
    // (required component) holding its models of encountered threats,
    // cats and wildlife alike. Implant + Forgetting only — prey get
    // NO Pass-A observation subset: their threat picture is instinct
    // (implanted priors) plus decay, and unread observation channels
    // would be pure ballast (ticket-505 lesson). Disjoint from the
    // cat `witnesses` query (which also holds `&mut PredatorBeliefs`)
    // via the paired `With<PreyAnimal>` / `Without<PreyAnimal>`
    // filters.
    mut prey_witnesses: Query<
        (Entity, &Position, &mut PredatorBeliefs),
        (With<PreyAnimal>, Without<Dead>),
    >,
    // Ethological colony-start: ground-food source read by Pass B into the
    // per-cat `surplus_food` location belief. `Option` so belief-integrator
    // unit tests (which don't build the influence-map/activation resources)
    // still run — the surplus authoring is simply skipped when absent.
    ground_surplus: Option<Res<GroundSurplusMap>>,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    let tick = time.tick;
    let cfg = &constants.beliefs;
    let shelter_cfg = &constants.shelter_beliefs;

    // 292 — entity-kind routing sets for `TargetActionFailed`. A failed
    // step's target can be a cat, a wildlife predator, prey, a corpse,
    // or a structure; only the first two have per-entity mental-model
    // homes (`CatBeliefs` / `PredatorBeliefs`). Membership is decided
    // here (component truth) rather than trusted from the emitter —
    // and anything in neither set is deliberately NOT modeled, per the
    // ticket-505 ballast lesson (prey/corpse churn must not accumulate
    // decay-load entries).
    let cat_set: std::collections::HashSet<Entity> = witnesses.iter().map(|(e, ..)| e).collect();
    let wildlife_set: std::collections::HashSet<Entity> =
        wildlife.iter().map(|(e, ..)| e).collect();
    // 265: cat positions for the wildlife implant pass (Pass B seeds a
    // wildlife witness's model of each cat in range with the
    // species-perceiver violence prior, symmetric to the cat-side
    // PredatorBeliefs implant below).
    let cat_positions: Vec<(Entity, Position)> =
        witnesses.iter().map(|(e, p, ..)| (e, *p)).collect();

    // ---- Pass A — Observation -----------------------------------------
    for ev in events.read() {
        // 374: shelter den-state events (DenDamaged / DenRepaired /
        // DenSieged / DenSiegeBroken) broadcast to any cat whose
        // home_den matches the den, regardless of sensing range — a
        // cat at work learns their home is being threatened. The
        // four self-keyed shelter events (DenClaimed / DenLost) are
        // gated on `witness == cat` inside the dispatcher, which is
        // independent of range. Other events keep the standard
        // range-gated path.
        let range_gated = !is_shelter_broadcast_event(ev);
        let pos = event_position(ev);
        for (
            witness_ent,
            witness_pos,
            mut cats,
            mut locs,
            mut preds,
            mut contexts,
            mut reserves,
            mut shelter,
        ) in witnesses.iter_mut()
        {
            if range_gated && !within_range(witness_pos, &pos) {
                continue;
            }
            apply_observation(
                ev,
                witness_ent,
                tick,
                cfg,
                shelter_cfg,
                &cat_set,
                &wildlife_set,
                &mut cats,
                &mut locs,
                &mut preds,
                &mut contexts,
                &mut reserves,
                &mut shelter,
            );
        }

        // 265: wildlife witnesses integrate the violence-relevant
        // subset (Attack + Hunt by cat actors) into their own
        // CatBeliefs. Same range gate as cat witnesses.
        for (witness_ent, witness_pos, _, mut cat_models) in wildlife_witnesses.iter_mut() {
            if range_gated && !within_range(witness_pos, &pos) {
                continue;
            }
            apply_wildlife_observation(ev, witness_ent, tick, cfg, &cat_set, &mut cat_models);
        }
    }

    // ---- Pass B — Implant + Forgetting --------------------------------
    let period = cfg.decay_stagger_period.max(1);
    let priors = &cfg.species_violence_priors;
    let tick_phase = tick % period;
    for (
        witness_ent,
        witness_pos,
        mut cats,
        mut locs,
        mut preds,
        mut contexts,
        mut reserves,
        _shelter,
    ) in witnesses.iter_mut()
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

        // Ethological colony-start: passive stagger-tick read of
        // `GroundSurplusMap` into the per-cat `surplus_food` location belief
        // at the cat's own bucket. NOT event-driven — scattered ground food
        // is a slowly-changing spatial fact, so emitting a `WitnessableEvent`
        // per food cluster per tick would be a per-tick flood the house rules
        // forbid. Mirrors `ShelterBeliefs.continuity`'s passive per-stagger
        // read. Authored after decay so the fresh observation isn't clipped
        // by the same-tick forgetting sweep. The map's linear-falloff disc
        // already folds in nearby food, so a point-read at the cat's position
        // reflects the surrounding windfall.
        if let Some(map) = ground_surplus.as_deref() {
            let observed = map.get(witness_pos.x(), witness_pos.y());
            if observed > 0.0 {
                let key = bucket_position(witness_pos.x(), witness_pos.y());
                let loc_model = locs.models.entry(key).or_default();
                update_facet(
                    &mut loc_model.surplus_food,
                    observed,
                    tick,
                    &cfg.surplus_food,
                );
                loc_model.last_updated_tick = tick;
                loc_model.evidence_count = loc_model.evidence_count.saturating_add(1);
                if let Some(activation) = activation.as_deref_mut() {
                    activation.record(Feature::SurplusFoodBeliefFormed);
                }
            }
        }
    }

    // ---- Pass B (265) — wildlife Implant + Forgetting ------------------
    // Symmetric to the cat pass above: a wildlife witness's first
    // encounter with a cat seeds its model with the species-perceiver
    // violence prior ("this snake instinctively fears cats"), and its
    // CatBeliefs decay on the same stagger discipline.
    for (witness_ent, witness_pos, wild, mut cat_models) in wildlife_witnesses.iter_mut() {
        if (witness_ent.index_u32() as u64) % period != tick_phase {
            continue;
        }

        for (cat_ent, cat_pos) in &cat_positions {
            if !within_range(witness_pos, cat_pos) {
                continue;
            }
            cat_models.models.entry(*cat_ent).or_insert_with(|| {
                let prior = cat_violence_prior_perceived_by(priors, wild.species);
                MentalModel {
                    perceived_violence_capability: Facet::from_prior(prior),
                    last_updated_tick: tick,
                    ..MentalModel::default()
                }
            });
        }

        decay_models(&mut cat_models.models, tick, cfg, period);
    }

    // ---- Pass B (314) — prey Implant + Forgetting ----------------------
    // A prey's first encounter with a threat (cat or wildlife) seeds
    // its PredatorBeliefs with the prey-perceiver instinct prior. The
    // affordance writer's prey-side `Bolt` heuristic reads the
    // resulting `perceived_violence_capability` facet each tick; DSE
    // consumers arrive with ticket 266.
    for (witness_ent, witness_pos, mut threat_models) in prey_witnesses.iter_mut() {
        if (witness_ent.index_u32() as u64) % period != tick_phase {
            continue;
        }

        for (cat_ent, cat_pos) in &cat_positions {
            if !within_range(witness_pos, cat_pos) {
                continue;
            }
            threat_models
                .models
                .entry(*cat_ent)
                .or_insert_with(|| MentalModel {
                    perceived_violence_capability: Facet::from_prior(priors.cat_perceived_by_prey),
                    last_updated_tick: tick,
                    ..MentalModel::default()
                });
        }

        for (wl_ent, wl_pos, wl) in wildlife.iter() {
            if !within_range(witness_pos, wl_pos) {
                continue;
            }
            threat_models.models.entry(wl_ent).or_insert_with(|| {
                let prior = violence_prior_perceived_by_prey(priors, wl.species);
                MentalModel {
                    perceived_violence_capability: Facet::from_prior(prior),
                    last_updated_tick: tick,
                    ..MentalModel::default()
                }
            });
        }

        decay_models(&mut threat_models.models, tick, cfg, period);
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
        for slot in &inventory.pouch {
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
        | WitnessableEvent::TargetActionFailed { position, .. }
        | WitnessableEvent::ReserveDeposited { position, .. }
        | WitnessableEvent::ReserveConsumed { position, .. }
        | WitnessableEvent::InventoryObserved { position, .. }
        | WitnessableEvent::PlayBow { position, .. }
        | WitnessableEvent::ReciprocalAdvance { position, .. }
        | WitnessableEvent::SustainedCoPresence { position, .. }
        | WitnessableEvent::CarriesFesteringWound { position, .. }
        | WitnessableEvent::PredatorAmbush { position, .. }
        | WitnessableEvent::HuntSearchYieldedNoPrey { position, .. }
        | WitnessableEvent::HuntScentDetected { position, .. }
        | WitnessableEvent::DenClaimed { position, .. }
        | WitnessableEvent::DenLost { position, .. }
        | WitnessableEvent::DenDamaged { position, .. }
        | WitnessableEvent::DenRepaired { position, .. }
        | WitnessableEvent::DenSieged { position, .. }
        | WitnessableEvent::DenSiegeBroken { position, .. } => *position,
    }
}

/// 374: shelter den-state events broadcast to any cat whose `home_den`
/// matches, independent of sensing range. Returns true only for
/// `DenDamaged`/`DenRepaired`/`DenSieged`/`DenSiegeBroken` — the two
/// cat-keyed shelter events (`DenClaimed`/`DenLost`) still flow through
/// the standard range-gated path because their `witness == cat` gate
/// in the dispatcher provides the equivalent filter and the cat is
/// trivially within range of its own event position.
fn is_shelter_broadcast_event(ev: &WitnessableEvent) -> bool {
    matches!(
        ev,
        WitnessableEvent::DenDamaged { .. }
            | WitnessableEvent::DenRepaired { .. }
            | WitnessableEvent::DenSieged { .. }
            | WitnessableEvent::DenSiegeBroken { .. }
    )
}

fn within_range(a: &Position, b: &Position) -> bool {
    (a.x() - b.x()).abs() + (a.y() - b.y()).abs() <= WITNESS_RANGE
}

fn species_violence_prior(priors: &SpeciesViolencePriors, species: WildSpecies) -> f32 {
    match species {
        WildSpecies::Fox => priors.fox,
        WildSpecies::Hawk => priors.hawk,
        WildSpecies::Snake => priors.snake,
        WildSpecies::ShadowFox => priors.shadow_fox,
    }
}

/// 265: how dangerous a cat looks to a given wildlife perceiver species
/// — the wildlife-perceiver rows of the violence-prior table, implanted
/// into a wildlife entity's `CatBeliefs` on first encounter.
fn cat_violence_prior_perceived_by(priors: &SpeciesViolencePriors, species: WildSpecies) -> f32 {
    match species {
        WildSpecies::Fox => priors.cat_perceived_by_fox,
        WildSpecies::Hawk => priors.cat_perceived_by_hawk,
        WildSpecies::Snake => priors.cat_perceived_by_snake,
        WildSpecies::ShadowFox => priors.cat_perceived_by_shadow_fox,
    }
}

/// 314: how dangerous a wildlife species looks to prey — the
/// prey-perceiver rows of the violence-prior table, implanted into a
/// prey entity's `PredatorBeliefs` on first encounter. (The cat row
/// is read directly as `priors.cat_perceived_by_prey` at the call
/// site — cats aren't a `WildSpecies`.)
fn violence_prior_perceived_by_prey(priors: &SpeciesViolencePriors, species: WildSpecies) -> f32 {
    match species {
        WildSpecies::Fox => priors.fox_perceived_by_prey,
        WildSpecies::Hawk => priors.hawk_perceived_by_prey,
        WildSpecies::Snake => priors.snake_perceived_by_prey,
        WildSpecies::ShadowFox => priors.shadow_fox_perceived_by_prey,
    }
}

/// 265: wildlife-witness observation path. Wildlife model only cats and
/// only the violence-relevant channels — witnessed `Attack` (a cat
/// fighting is direct violence evidence) and `Hunt` (a successful
/// hunter is a capable killer). The full cat-side channel set
/// (affiliation, receptivity, reserves, shelter…) is deliberately NOT
/// mirrored: wildlife have no social/economic stake in the colony, and
/// the ticket-505 ballast lesson says unread entries are pure decay
/// load.
fn apply_wildlife_observation(
    ev: &WitnessableEvent,
    witness: Entity,
    tick: u64,
    cfg: &BeliefsConstants,
    cat_set: &std::collections::HashSet<Entity>,
    cat_models: &mut CatBeliefs,
) {
    match ev {
        WitnessableEvent::Attack {
            actor,
            target,
            severity,
            ..
        } => {
            if !cat_set.contains(actor) {
                return;
            }
            let model = cat_models.models.entry(*actor).or_default();
            update_facet(
                &mut model.perceived_violence_capability,
                *severity,
                tick,
                &cfg.perceived_violence_capability,
            );
            update_facet(
                &mut model.recency_of_threat_cue,
                OBSERVED_MAX,
                tick,
                &cfg.recency_of_threat_cue,
            );
            // Aggression directed at the witness itself is hostility
            // evidence; a cat fighting some other creature is not.
            if *target == witness {
                update_facet(
                    &mut model.perceived_hostility,
                    *severity,
                    tick,
                    &cfg.perceived_hostility,
                );
            }
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::Hunt {
            hunter, success, ..
        } => {
            if !cat_set.contains(hunter) {
                return;
            }
            let observed = if *success {
                OBSERVED_MAX
            } else {
                OBSERVED_FAIL
            };
            let model = cat_models.models.entry(*hunter).or_default();
            update_facet(
                &mut model.perceived_violence_capability,
                observed,
                tick,
                &cfg.perceived_violence_capability,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }

        // Every other channel is cat-side only (see fn rustdoc).
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_observation(
    ev: &WitnessableEvent,
    witness: Entity,
    tick: u64,
    cfg: &BeliefsConstants,
    shelter_cfg: &ShelterBeliefConstants,
    // 292 — entity-kind routing for `TargetActionFailed` (see the
    // set construction in `integrate_beliefs`).
    cat_set: &std::collections::HashSet<Entity>,
    wildlife_set: &std::collections::HashSet<Entity>,
    cats: &mut CatBeliefs,
    locs: &mut LocationBeliefs,
    preds: &mut PredatorBeliefs,
    contexts: &mut ContextBeliefs,
    reserves: &mut ColonyReservesBelief,
    shelter: &mut ShelterBeliefs,
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
            let loc_key = bucket_position(position.x(), position.y());
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
            // Ticket 505 kept the threat write OUT of the cat-keyed
            // `cats.models` (wildlife churn there was pure decay
            // ballast — 300-700 unread entries per cat, 14.1% self
            // CPU). 265's activation lands it in the home 505 named:
            // `PredatorBeliefs`, which the flee/patrol consumers
            // actually read and whose wildlife entries the Implant
            // pass already creates — so no new ballast class. Gated
            // on `wildlife_set` membership (component truth, not the
            // emitter's word) per the 292 routing discipline, and on
            // third-party witnesses only: the fleer's own flee
            // decision derives FROM its beliefs, and a self-write
            // would make fleeing self-confirming.
            if *fleer != witness && wildlife_set.contains(threat) {
                let threat_model = preds.models.entry(*threat).or_default();
                update_facet(
                    &mut threat_model.perceived_violence_capability,
                    FLEE_CUE_OBSERVED_VALUE,
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
            hunter,
            position,
            success,
            ..
        } => {
            // 293: location-side prey-yield update fires for any witness
            // (self and third-party). A successful hunt is direct evidence
            // that the bucket holds prey; a failed attempt is weaker
            // negative evidence. Cat-side updates (perceived_violence_capability,
            // predictability) below still skip the self-witness path.
            let loc_key = bucket_position(position.x(), position.y());
            let loc_model = locs.models.entry(loc_key).or_default();
            update_facet(
                &mut loc_model.prey_yield,
                if *success {
                    OBSERVED_MAX
                } else {
                    OBSERVED_FAIL
                },
                tick,
                &cfg.prey_yield,
            );
            loc_model.last_updated_tick = tick;
            loc_model.evidence_count = loc_model.evidence_count.saturating_add(1);

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

        WitnessableEvent::TargetActionFailed { actor, target, .. } => {
            // Self-observation: only the actor learns from its own step
            // failure — a third party sees no cue (same convention as
            // SelfPlanFailed above). The learning is about the TARGET:
            // "this entity is unreliable for my purposes."
            if *actor != witness {
                return;
            }
            // Route by entity kind, decided from component truth. Prey,
            // corpses, and structures have no per-entity mental-model
            // home — deliberately unmodeled (505 ballast lesson); their
            // failure memory is the ticket's pre-registered pivot (b)
            // if the hypothesize pass shows the loss matters.
            let model = if cat_set.contains(target) {
                cats.models.entry(*target).or_default()
            } else if wildlife_set.contains(target) {
                preds.models.entry(*target).or_default()
            } else {
                return;
            };
            // The cooldown must RECOVER toward "reliable" between
            // failures: pin the facet's decay target to 1.0 regardless
            // of which arm created the model — the struct-default
            // prior (0.0) would make a single failure a permanent
            // verdict (290's `from_prior(1.0)` seeding, restated for a
            // facet other arms may have touched first). With
            // predictability's `learning_rate = 1.0`, the update below
            // snaps value to OBSERVED_FAIL from any starting point, so
            // only the recovery target needs pinning.
            model.predictability.prior = 1.0;
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

        WitnessableEvent::PredatorAmbush { position, .. } => {
            // 294: witnesses within sensing range lift their
            // `LocationBeliefs[bucket(pos)].recency_of_threat_cue` to
            // `OBSERVED_MAX`. Per-cat substrate replacement for the
            // retired colony-wide `RecentAmbushMap`. The integrator's
            // outer loop has already filtered by `within_range`; here we
            // just bump the facet.
            let loc_key = bucket_position(position.x(), position.y());
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

        WitnessableEvent::HuntSearchYieldedNoPrey {
            actor,
            position,
            tiles_searched,
            ..
        } => {
            // 293: searching a region and coming up empty is a self-only
            // observation — other cats don't witness an absence. The
            // searcher's `LocationBeliefs[bucket(pos)].prey_yield` is
            // pulled toward `OBSERVED_FAIL` with magnitude scaled by
            // effort (`tiles_searched`). The legacy
            // `HuntingPriors::record_failed_search` applied a linear
            // `-tiles_searched / 2000.0` delta; we match the shape here
            // by intensity-scaling the EMA step.
            if *actor != witness {
                return;
            }
            let loc_key = bucket_position(position.x(), position.y());
            let loc_model = locs.models.entry(loc_key).or_default();
            let intensity = (*tiles_searched as f32 / 2000.0).clamp(0.0, 1.0);
            let tun = &cfg.prey_yield;
            loc_model.prey_yield.value +=
                tun.learning_rate * intensity * (OBSERVED_FAIL - loc_model.prey_yield.value);
            loc_model.prey_yield.strength =
                (loc_model.prey_yield.strength + tun.strength_per_observation * intensity).min(1.0);
            loc_model.prey_yield.last_source = EvidenceKind::Observation;
            loc_model.prey_yield.last_updated_tick = tick;
            loc_model.last_updated_tick = tick;
            loc_model.evidence_count = loc_model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::HuntScentDetected {
            actor, position, ..
        } => {
            // 293: smelling prey is a self-only observation that weakly
            // lifts the searcher's prey_yield at the bucket. Magnitude
            // sits between neutral (0.5) and OBSERVED_MAX — preserves
            // the legacy `record_scent`'s mild positive contribution
            // (+0.05 vs +0.15 for catch) within the EMA convention.
            if *actor != witness {
                return;
            }
            let loc_key = bucket_position(position.x(), position.y());
            let loc_model = locs.models.entry(loc_key).or_default();
            update_facet(
                &mut loc_model.prey_yield,
                SCENT_OBSERVED_VALUE,
                tick,
                &cfg.prey_yield,
            );
            loc_model.last_updated_tick = tick;
            loc_model.evidence_count = loc_model.evidence_count.saturating_add(1);
        }

        WitnessableEvent::CarriesFesteringWound {
            actor, severity, ..
        } => {
            // 472: observer sees a peer carrying a festering wound.
            // Lifts `perceived_injury_level` on the actor scaled by
            // observed severity (the part's current tissue_damage at
            // emit time). Self-witness skipped — self-festering is the
            // 089 `OwnInjurySite` anchor's job, not the social belief
            // layer's. Source attribution rides on the message for
            // narrative consumers (`source_kind`); the belief lift is
            // source-agnostic — what witnesses perceive is "this cat
            // is wounded badly," not "wounded by X."
            if *actor == witness {
                return;
            }
            let model = cats.models.entry(*actor).or_default();
            update_facet(
                &mut model.perceived_injury_level,
                *severity,
                tick,
                &cfg.perceived_injury_level,
            );
            model.last_updated_tick = tick;
            model.evidence_count = model.evidence_count.saturating_add(1);
        }
        // 374: shelter belief integrator arms. Self-keyed events
        // (DenClaimed/DenLost) gate on `witness == cat`. Den-state
        // events (Damaged/Repaired/Sieged/SiegeBroken) gate on the
        // witness's `home_den == Some(den)`. The Pass-A range filter
        // is already skipped for the four broadcast variants by
        // `is_shelter_broadcast_event`.
        WitnessableEvent::DenClaimed {
            cat,
            den,
            condition,
            ..
        } => {
            if *cat != witness {
                return;
            }
            shelter.home_den = Some(*den);
            shelter.facet.belonging = lerp_to(
                shelter.facet.belonging,
                1.0,
                shelter_cfg.belonging_learning_rate,
            );
            // Seed quality from the den's current condition — without
            // this, a healthy newly-built Den never emits a damage or
            // repair threshold-crossing, so `quality` would stay at 0
            // and the cat's security contribution would silently zero
            // regardless of belonging.
            shelter.facet.quality = lerp_to(
                shelter.facet.quality,
                condition.clamp(0.0, 1.0),
                shelter_cfg.quality_learning_rate,
            );
            shelter.facet.last_updated_tick = tick;
        }
        WitnessableEvent::DenLost { cat, reason, .. } => {
            if *cat != witness {
                return;
            }
            shelter.home_den = None;
            shelter.facet.belonging = lerp_to(
                shelter.facet.belonging,
                0.0,
                shelter_cfg.belonging_learning_rate,
            );
            // Quality and threat lose their subject when the home_den
            // is dropped; reset to neutral so a fresh claim starts
            // clean rather than inheriting stale beliefs about a
            // different den.
            shelter.facet.quality = 0.0;
            shelter.facet.threat = 0.0;
            // Continuity is preserved on Abandoned (the cat carries
            // the felt time-at-home memory), reset on Destroyed
            // (the substrate of the felt-time-at-home is gone),
            // partially decayed on Displaced (memory persists but
            // attenuates). v1 zeroes all three for simplicity;
            // refinement is a 374 follow-on.
            let _ = reason;
            shelter.facet.continuity = 0.0;
            shelter.facet.last_updated_tick = tick;
        }
        WitnessableEvent::DenDamaged {
            den, new_condition, ..
        } => {
            if shelter.home_den != Some(*den) {
                return;
            }
            shelter.facet.quality = lerp_to(
                shelter.facet.quality,
                new_condition.clamp(0.0, 1.0),
                shelter_cfg.quality_learning_rate,
            );
            shelter.facet.last_updated_tick = tick;
        }
        WitnessableEvent::DenRepaired {
            den, new_condition, ..
        } => {
            if shelter.home_den != Some(*den) {
                return;
            }
            shelter.facet.quality = lerp_to(
                shelter.facet.quality,
                new_condition.clamp(0.0, 1.0),
                shelter_cfg.quality_learning_rate,
            );
            shelter.facet.last_updated_tick = tick;
        }
        WitnessableEvent::DenSieged { den, .. } => {
            if shelter.home_den != Some(*den) {
                return;
            }
            shelter.facet.threat =
                lerp_to(shelter.facet.threat, 1.0, shelter_cfg.threat_learning_rate);
            shelter.facet.last_updated_tick = tick;
        }
        WitnessableEvent::DenSiegeBroken { den, .. } => {
            if shelter.home_den != Some(*den) {
                return;
            }
            shelter.facet.threat =
                lerp_to(shelter.facet.threat, 0.0, shelter_cfg.threat_learning_rate);
            shelter.facet.last_updated_tick = tick;
        }
    }
}

/// 374: single EMA step on a `[0.0, 1.0]` scalar. Mirrors the
/// `update_facet` shape but operates on plain floats since
/// `ShelterFacet` sub-axes aren't `Facet`s — they're raw `f32`s
/// without separate prior/strength bookkeeping. Clamps the result
/// so callers don't need to.
fn lerp_to(current: f32, observed: f32, learning_rate: f32) -> f32 {
    (current + learning_rate * (observed - current)).clamp(0.0, 1.0)
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
        decay_facet(&mut model.prey_yield, &cfg.prey_yield, period);
        decay_facet(&mut model.surplus_food, &cfg.surplus_food, period);
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
            model.prey_yield.strength,
            model.surplus_food.strength,
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
                ShelterBeliefs::default(),
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

    #[test]
    fn ground_surplus_lifts_surplus_food_belief() {
        use crate::resources::GroundSurplusMap;

        let (mut world, mut schedule) = test_world(0);
        // Ungathered food stamped near where the cat stands.
        let mut map = GroundSurplusMap::default_map();
        map.stamp(10, 10, 1.0, 10.0);
        assert!(map.get(10, 10) > 0.0, "sanity: stamp should read nonzero");
        world.insert_resource(map);

        let cat = spawn_cat(&mut world, Position::new(10, 10));

        // Run past one stagger phase so Pass B authors the belief at least once.
        let period = SimConstants::default().beliefs.decay_stagger_period;
        for _ in 0..(period + 1) {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let locs = world.get::<LocationBeliefs>(cat).unwrap();
        let key = bucket_position(10, 10);
        let model = locs
            .models
            .get(&key)
            .expect("surplus_food belief should be authored at the cat's bucket");
        assert!(
            model.surplus_food.value > 0.0,
            "surplus_food should lift off zero from GroundSurplusMap; got {}",
            model.surplus_food.value
        );
        assert!(model.surplus_food.strength > 0.0);
        assert_eq!(model.surplus_food.last_source, EvidenceKind::Observation);
    }

    #[test]
    fn no_ground_surplus_leaves_belief_at_zero() {
        use crate::resources::GroundSurplusMap;

        let (mut world, mut schedule) = test_world(0);
        // Empty map — no ungathered food anywhere.
        world.insert_resource(GroundSurplusMap::default_map());
        let cat = spawn_cat(&mut world, Position::new(10, 10));

        let period = SimConstants::default().beliefs.decay_stagger_period;
        for _ in 0..(period + 1) {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let locs = world.get::<LocationBeliefs>(cat).unwrap();
        let key = bucket_position(10, 10);
        // Either no entry, or an entry with a zero surplus_food value.
        let v = locs
            .models
            .get(&key)
            .map(|m| m.surplus_food.value)
            .unwrap_or(0.0);
        assert_eq!(v, 0.0, "surplus_food should stay zero with no ground food");
    }

    // 265 — wildlife-witness coverage: implant, observation subset,
    // range gate.

    #[test]
    fn wildlife_implant_seeds_cat_violence_prior_on_first_encounter() {
        let (mut world, mut schedule) = test_world(0);
        let cat = spawn_cat(&mut world, Position::new(0, 0));
        let snake = spawn_wildlife(&mut world, WildSpecies::Snake, Position::new(2, 0));

        let period = SimConstants::default().beliefs.decay_stagger_period;
        for _ in 0..(period + 1) {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        // The snake carries CatBeliefs via WildAnimal's required
        // component and Pass B implanted the perceiver-row prior.
        let beliefs = world
            .get::<CatBeliefs>(snake)
            .expect("WildAnimal requires CatBeliefs");
        let model = beliefs
            .models
            .get(&cat)
            .expect("wildlife should seed a cat model on first encounter");
        let expected = SimConstants::default()
            .beliefs
            .species_violence_priors
            .cat_perceived_by_snake;
        assert!(
            (model.perceived_violence_capability.value - expected).abs() < 1e-5,
            "snake's cat prior should be {expected}; got {}",
            model.perceived_violence_capability.value
        );
        assert_eq!(
            model.perceived_violence_capability.last_source,
            EvidenceKind::Implant
        );
    }

    #[test]
    fn wildlife_witness_integrates_cat_attack() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let victim = spawn_cat(&mut world, Position::new(11, 10));
        let fox = spawn_wildlife(&mut world, WildSpecies::Fox, Position::new(12, 10));

        world.write_message(WitnessableEvent::Attack {
            actor,
            target: victim,
            position: Position::new(10, 10),
            severity: 0.8,
            tick: 100,
        });

        schedule.run(&mut world);

        let beliefs = world.get::<CatBeliefs>(fox).unwrap();
        let model = beliefs
            .models
            .get(&actor)
            .expect("fox should hold belief about the attacking cat");
        assert!(
            model.perceived_violence_capability.value > 0.0,
            "witnessed Attack should lift violence capability; got {}",
            model.perceived_violence_capability.value
        );
        assert!(model.recency_of_threat_cue.value > 0.0);
        // Aggression was against another cat, not the fox — no
        // hostility-toward-me evidence.
        assert_eq!(model.perceived_hostility.value, 0.0);
        assert_eq!(
            model.perceived_violence_capability.last_source,
            EvidenceKind::Observation
        );
        // The victim is not modeled — wildlife track only the
        // violence-relevant actor channels.
        assert!(!beliefs.models.contains_key(&victim));
    }

    #[test]
    fn wildlife_witness_attack_on_self_lifts_hostility() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let fox = spawn_wildlife(&mut world, WildSpecies::Fox, Position::new(11, 10));

        world.write_message(WitnessableEvent::Attack {
            actor,
            target: fox,
            position: Position::new(10, 10),
            severity: 0.6,
            tick: 100,
        });

        schedule.run(&mut world);

        let beliefs = world.get::<CatBeliefs>(fox).unwrap();
        let model = beliefs.models.get(&actor).expect("fox models its attacker");
        assert!(
            model.perceived_hostility.value > 0.0,
            "attack on the witness itself is hostility evidence"
        );
    }

    #[test]
    fn out_of_range_wildlife_witness_does_not_update() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(0, 0));
        let victim = spawn_cat(&mut world, Position::new(1, 0));
        let far_fox = spawn_wildlife(&mut world, WildSpecies::Fox, Position::new(50, 50));

        world.write_message(WitnessableEvent::Attack {
            actor,
            target: victim,
            position: Position::new(0, 0),
            severity: 0.8,
            tick: 100,
        });

        schedule.run(&mut world);

        let beliefs = world.get::<CatBeliefs>(far_fox).unwrap();
        assert!(
            beliefs.models.is_empty(),
            "out-of-range wildlife witnesses should not update beliefs"
        );
    }

    #[test]
    fn wildlife_witness_integrates_cat_hunt_success() {
        let (mut world, mut schedule) = test_world(100);
        let hunter = spawn_cat(&mut world, Position::new(10, 10));
        let hawk = spawn_wildlife(&mut world, WildSpecies::Hawk, Position::new(12, 10));

        world.write_message(WitnessableEvent::Hunt {
            hunter,
            prey_kind: crate::components::prey::PreyKind::Rabbit,
            position: Position::new(10, 10),
            success: true,
            tick: 100,
        });

        schedule.run(&mut world);

        let beliefs = world.get::<CatBeliefs>(hawk).unwrap();
        let model = beliefs
            .models
            .get(&hunter)
            .expect("hawk should model a successful hunter");
        assert!(
            model.perceived_violence_capability.value > 0.0,
            "witnessed successful hunt is violence-capability evidence"
        );
    }

    // 314 — prey-witness coverage: implant of cat + wildlife threat
    // priors, and no Pass-A observation channel.

    fn spawn_prey(world: &mut World, position: Position) -> Entity {
        // PreyAnimal's required component adds PredatorBeliefs.
        world
            .spawn((
                crate::components::prey::PreyAnimal,
                position,
                Health::default(),
            ))
            .id()
    }

    #[test]
    fn prey_implant_seeds_cat_and_wildlife_threat_priors() {
        let (mut world, mut schedule) = test_world(0);
        let cat = spawn_cat(&mut world, Position::new(0, 0));
        let fox = spawn_wildlife(&mut world, WildSpecies::Fox, Position::new(3, 0));
        let mouse = spawn_prey(&mut world, Position::new(1, 1));

        let period = SimConstants::default().beliefs.decay_stagger_period;
        for _ in 0..(period + 1) {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let beliefs = world
            .get::<PredatorBeliefs>(mouse)
            .expect("PreyAnimal requires PredatorBeliefs");
        let priors = SimConstants::default().beliefs.species_violence_priors;
        let cat_model = beliefs
            .models
            .get(&cat)
            .expect("prey should seed a cat threat model on first encounter");
        assert!(
            (cat_model.perceived_violence_capability.value - priors.cat_perceived_by_prey).abs()
                < 1e-5,
            "prey's cat prior should be {}; got {}",
            priors.cat_perceived_by_prey,
            cat_model.perceived_violence_capability.value
        );
        assert_eq!(
            cat_model.perceived_violence_capability.last_source,
            EvidenceKind::Implant
        );
        let fox_model = beliefs
            .models
            .get(&fox)
            .expect("prey should seed a fox threat model on first encounter");
        assert!(
            (fox_model.perceived_violence_capability.value - priors.fox_perceived_by_prey).abs()
                < 1e-5,
            "prey's fox prior should be {}; got {}",
            priors.fox_perceived_by_prey,
            fox_model.perceived_violence_capability.value
        );
    }

    #[test]
    fn prey_witness_has_no_observation_channel() {
        // An Attack witnessed in range must NOT create or update prey
        // models — prey run Pass B only (instinct priors + decay). The
        // implant entry the pass seeds carries EvidenceKind::Implant,
        // never Observation.
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let victim = spawn_cat(&mut world, Position::new(11, 10));
        let mouse = spawn_prey(&mut world, Position::new(12, 10));

        world.write_message(WitnessableEvent::Attack {
            actor,
            target: victim,
            position: Position::new(10, 10),
            severity: 0.8,
            tick: 100,
        });

        // Single tick: Pass A runs for cat/wildlife witnesses; the
        // prey's stagger phase may or may not land, but Observation
        // evidence must never appear either way.
        schedule.run(&mut world);

        let beliefs = world.get::<PredatorBeliefs>(mouse).unwrap();
        for model in beliefs.models.values() {
            assert_eq!(
                model.perceived_violence_capability.last_source,
                EvidenceKind::Implant,
                "prey models must only carry implanted priors, not observations"
            );
        }
    }

    #[test]
    fn out_of_range_prey_gets_no_implants() {
        let (mut world, mut schedule) = test_world(0);
        spawn_cat(&mut world, Position::new(0, 0));
        spawn_wildlife(&mut world, WildSpecies::Hawk, Position::new(1, 0));
        let far_mouse = spawn_prey(&mut world, Position::new(50, 50));

        let period = SimConstants::default().beliefs.decay_stagger_period;
        for _ in 0..(period + 1) {
            schedule.run(&mut world);
            let mut time = world.resource_mut::<TimeState>();
            time.tick += 1;
        }

        let beliefs = world.get::<PredatorBeliefs>(far_mouse).unwrap();
        assert!(
            beliefs.models.is_empty(),
            "threats beyond WITNESS_RANGE must not implant"
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

    /// Ticket 505: FleeFrom lifts the FLEER's predictability on
    /// witnesses, and writes NOTHING keyed on the threat entity into
    /// the cat-keyed map — the pre-505 arm leaked wildlife threats
    /// into `cats.models` as unread decay ballast (300-700 entries
    /// per cat at soak scale). The 265 activation landed the
    /// witness-learns-threat-violence signal on `PredatorBeliefs`
    /// (wildlife-gated — see the sibling test below); a CAT-keyed
    /// threat still writes nowhere.
    #[test]
    fn flee_from_event_lifts_fleer_predictability_only() {
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
        assert!(
            !cats.models.contains_key(&threat),
            "FleeFrom must not create a CatBeliefs entry keyed on the threat \
             (wildlife ballast — ticket 505)"
        );
        let preds = world.get::<PredatorBeliefs>(witness).unwrap();
        assert!(
            !preds.models.contains_key(&threat),
            "a cat-keyed threat must not create a PredatorBeliefs entry \
             either — the 265 write is wildlife-gated"
        );
    }

    /// 265 activation: the substrate-correct home 505 pointed at —
    /// a third-party witness watching a colony-mate flee a WILDLIFE
    /// threat lifts its `PredatorBeliefs` violence model of that
    /// threat toward `FLEE_CUE_OBSERVED_VALUE`. The fleer itself gets
    /// no write (fleeing must not be self-confirming).
    #[test]
    fn flee_from_wildlife_threat_lifts_witness_predator_belief() {
        let (mut world, mut schedule) = test_world(100);
        let fleer = spawn_cat(&mut world, Position::new(10, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10));
        let fox = world
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                Position::new(11, 10),
                crate::components::physical::Health::default(),
            ))
            .id();

        world.write_message(WitnessableEvent::FleeFrom {
            fleer,
            threat: fox,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let preds = world.get::<PredatorBeliefs>(witness).unwrap();
        let model = preds
            .models
            .get(&fox)
            .expect("witness must hold a PredatorBeliefs entry on the fox threat");
        assert!(
            model.perceived_violence_capability.value > 0.0
                && model.perceived_violence_capability.strength > 0.0,
            "flee cue must lift the witness's violence model of the threat"
        );

        // The fleer's own PredatorBeliefs may hold an IMPLANT-seeded
        // entry (Pass B runs on its stagger tick), but the flee cue
        // itself must not have counted as evidence for the fleer.
        if let Some(fleer_preds) = world.get::<PredatorBeliefs>(fleer) {
            if let Some(m) = fleer_preds.models.get(&fox) {
                assert_eq!(
                    m.evidence_count, 0,
                    "the fleer's own flee must not self-confirm as observation evidence"
                );
            }
        }
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

    // ---------------------------------------------------------------------
    // 292 — TargetActionFailed target-keyed predictability
    //
    // The target-keyed sibling of the 290 block above: the ACTOR's own
    // failure against a target EMAs the actor's model of the TARGET.
    // These pin the shape the cutover sensor
    // (`target_cooldown_signal`) will read.
    // ---------------------------------------------------------------------

    #[test]
    fn target_action_failed_drops_actor_model_of_cat_target() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let target = spawn_cat(&mut world, Position::new(11, 10));

        world.write_message(WitnessableEvent::TargetActionFailed {
            actor,
            action: crate::ai::planner::GoapActionKind::SocializeWith,
            target,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        let model = world
            .get::<CatBeliefs>(actor)
            .unwrap()
            .models
            .get(&target)
            .expect("actor should hold a model of the failed target");
        assert!(
            model.predictability.value < 0.05,
            "failure must snap the target's predictability toward 0; got {}",
            model.predictability.value
        );
        assert_eq!(
            model.predictability.prior, 1.0,
            "recovery target must be pinned to reliable (1.0)"
        );

        // First-person only: the TARGET (an in-range witness of the
        // event) must NOT learn anything from the actor's silent
        // failure — no model of the actor appears.
        assert!(
            !world
                .get::<CatBeliefs>(target)
                .unwrap()
                .models
                .contains_key(&actor),
            "third parties must not learn from someone else's step failure"
        );
    }

    #[test]
    fn target_action_failed_routes_wildlife_target_to_predator_beliefs() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        let fox = spawn_wildlife(&mut world, WildSpecies::Fox, Position::new(12, 10));

        world.write_message(WitnessableEvent::TargetActionFailed {
            actor,
            action: crate::ai::planner::GoapActionKind::EngageThreat,
            target: fox,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        assert!(
            !world
                .get::<CatBeliefs>(actor)
                .unwrap()
                .models
                .contains_key(&fox),
            "wildlife targets must not leak into CatBeliefs (505 ballast rule)"
        );
        let model = world
            .get::<PredatorBeliefs>(actor)
            .unwrap()
            .models
            .get(&fox)
            .expect("wildlife target routes to PredatorBeliefs");
        assert!(model.predictability.value < 0.05);
        assert_eq!(model.predictability.prior, 1.0);
    }

    #[test]
    fn target_action_failed_ignores_unmodeled_target_kinds() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(10, 10));
        // A bare entity (stands in for prey / corpse / structure —
        // anything in neither the cat nor the wildlife set).
        let prey = world.spawn(Position::new(11, 10)).id();

        world.write_message(WitnessableEvent::TargetActionFailed {
            actor,
            action: crate::ai::planner::GoapActionKind::EngagePrey,
            target: prey,
            position: Position::new(10, 10),
            tick: 100,
        });
        schedule.run(&mut world);

        assert!(
            !world
                .get::<CatBeliefs>(actor)
                .unwrap()
                .models
                .contains_key(&prey),
            "unmodeled target kinds must not create CatBeliefs entries"
        );
        assert!(
            !world
                .get::<PredatorBeliefs>(actor)
                .unwrap()
                .models
                .contains_key(&prey),
            "unmodeled target kinds must not create PredatorBeliefs entries"
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

    // ---- 294 — PredatorAmbush variant ---------------------------------

    #[test]
    fn predator_ambush_lifts_witness_location_recency() {
        let (mut world, mut schedule) = test_world(100);
        let predator = spawn_wildlife(&mut world, WildSpecies::ShadowFox, Position::new(10, 10));
        let victim = spawn_cat(&mut world, Position::new(10, 10));
        let witness = spawn_cat(&mut world, Position::new(12, 10)); // within WITNESS_RANGE=10

        world.write_message(WitnessableEvent::PredatorAmbush {
            predator,
            victim,
            position: Position::new(10, 10),
            tick: 100,
        });

        schedule.run(&mut world);

        let locs = world
            .get::<LocationBeliefs>(witness)
            .expect("witness has LocationBeliefs");
        let key = crate::components::beliefs::bucket_position(10, 10);
        let model = locs
            .models
            .get(&key)
            .expect("witness should hold a LocationBeliefs entry at the ambush bucket");
        assert!(
            model.recency_of_threat_cue.value > 0.0,
            "recency_of_threat_cue should lift on PredatorAmbush; got {}",
            model.recency_of_threat_cue.value
        );
        assert!(model.recency_of_threat_cue.strength > 0.0);
        assert_eq!(
            model.recency_of_threat_cue.last_source,
            EvidenceKind::Observation
        );
    }

    #[test]
    fn predator_ambush_out_of_range_witness_does_not_update() {
        let (mut world, mut schedule) = test_world(100);
        let predator = spawn_wildlife(&mut world, WildSpecies::ShadowFox, Position::new(0, 0));
        let victim = spawn_cat(&mut world, Position::new(0, 0));
        let far = spawn_cat(&mut world, Position::new(50, 50));

        world.write_message(WitnessableEvent::PredatorAmbush {
            predator,
            victim,
            position: Position::new(0, 0),
            tick: 100,
        });

        schedule.run(&mut world);

        let locs = world.get::<LocationBeliefs>(far).unwrap();
        assert!(
            locs.models.is_empty(),
            "out-of-range cats should not learn first-hand about an ambush"
        );
    }

    // ---- 293 — Hunt-yield + Scent + Failed-search variants ----------------

    #[test]
    fn hunt_success_lifts_witness_location_prey_yield() {
        let (mut world, mut schedule) = test_world(100);
        let hunter = spawn_cat(&mut world, Position::new(20, 20));
        let witness = spawn_cat(&mut world, Position::new(22, 20)); // within WITNESS_RANGE
        world.write_message(WitnessableEvent::Hunt {
            hunter,
            prey_kind: crate::components::prey::PreyKind::Mouse,
            position: Position::new(20, 20),
            success: true,
            tick: 100,
        });
        schedule.run(&mut world);
        let key = crate::components::beliefs::bucket_position(20, 20);

        // Hunter (self-witness) — location-side update fires even though
        // the cat-side updates short-circuit.
        let hunter_locs = world.get::<LocationBeliefs>(hunter).unwrap();
        let hunter_model = hunter_locs
            .models
            .get(&key)
            .expect("hunter's own prey_yield should lift on a successful catch");
        assert!(hunter_model.prey_yield.value > 0.0);
        assert!(hunter_model.prey_yield.strength > 0.0);

        // Third-party witness — also lifts.
        let witness_locs = world.get::<LocationBeliefs>(witness).unwrap();
        let witness_model = witness_locs
            .models
            .get(&key)
            .expect("witness location prey_yield should lift");
        assert!(witness_model.prey_yield.value > 0.0);
    }

    #[test]
    fn hunt_failure_pulls_prey_yield_toward_zero() {
        let (mut world, mut schedule) = test_world(100);
        let hunter = spawn_cat(&mut world, Position::new(30, 30));
        // Seed a pre-existing positive belief so the EMA pull is observable.
        {
            let mut locs = world.get_mut::<LocationBeliefs>(hunter).unwrap();
            let key = crate::components::beliefs::bucket_position(30, 30);
            let model = locs.models.entry(key).or_default();
            model.prey_yield.value = 0.8;
            model.prey_yield.strength = 0.5;
        }
        world.write_message(WitnessableEvent::Hunt {
            hunter,
            prey_kind: crate::components::prey::PreyKind::Mouse,
            position: Position::new(30, 30),
            success: false,
            tick: 100,
        });
        schedule.run(&mut world);
        let key = crate::components::beliefs::bucket_position(30, 30);
        let model = world
            .get::<LocationBeliefs>(hunter)
            .unwrap()
            .models
            .get(&key)
            .unwrap();
        assert!(
            model.prey_yield.value < 0.8,
            "failed Hunt should pull prey_yield toward 0.0; got {}",
            model.prey_yield.value
        );
    }

    #[test]
    fn search_yielded_no_prey_drops_actor_prey_yield() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(40, 40));
        let bystander = spawn_cat(&mut world, Position::new(41, 40)); // close but not the actor
        {
            let mut locs = world.get_mut::<LocationBeliefs>(actor).unwrap();
            let key = crate::components::beliefs::bucket_position(40, 40);
            let model = locs.models.entry(key).or_default();
            model.prey_yield.value = 0.7;
        }
        world.write_message(WitnessableEvent::HuntSearchYieldedNoPrey {
            actor,
            position: Position::new(40, 40),
            tiles_searched: 2000, // intensity = 1.0
            tick: 100,
        });
        schedule.run(&mut world);
        let key = crate::components::beliefs::bucket_position(40, 40);
        let actor_model = world
            .get::<LocationBeliefs>(actor)
            .unwrap()
            .models
            .get(&key)
            .unwrap();
        assert!(
            actor_model.prey_yield.value < 0.7,
            "actor's prey_yield should drop on a fruitless search; got {}",
            actor_model.prey_yield.value
        );
        // Third-party bystander does NOT witness absence.
        let bystander_locs = world.get::<LocationBeliefs>(bystander).unwrap();
        assert!(
            bystander_locs.models.is_empty(),
            "third-party cats don't witness a non-event"
        );
    }

    #[test]
    fn search_yielded_no_prey_intensity_scales_with_tiles() {
        // Two actors search the same bucket with very different efforts;
        // the longer search produces a larger drop in prey_yield.
        let (mut world, mut schedule) = test_world(100);
        let short_searcher = spawn_cat(&mut world, Position::new(60, 60));
        let long_searcher = spawn_cat(&mut world, Position::new(80, 80));
        for actor in [short_searcher, long_searcher] {
            let mut locs = world.get_mut::<LocationBeliefs>(actor).unwrap();
            let key = if actor == short_searcher {
                crate::components::beliefs::bucket_position(60, 60)
            } else {
                crate::components::beliefs::bucket_position(80, 80)
            };
            let model = locs.models.entry(key).or_default();
            model.prey_yield.value = 0.5;
        }
        world.write_message(WitnessableEvent::HuntSearchYieldedNoPrey {
            actor: short_searcher,
            position: Position::new(60, 60),
            tiles_searched: 200, // intensity = 0.1
            tick: 100,
        });
        world.write_message(WitnessableEvent::HuntSearchYieldedNoPrey {
            actor: long_searcher,
            position: Position::new(80, 80),
            tiles_searched: 2000, // intensity = 1.0
            tick: 100,
        });
        schedule.run(&mut world);
        let short_v = world
            .get::<LocationBeliefs>(short_searcher)
            .unwrap()
            .models
            .get(&crate::components::beliefs::bucket_position(60, 60))
            .unwrap()
            .prey_yield
            .value;
        let long_v = world
            .get::<LocationBeliefs>(long_searcher)
            .unwrap()
            .models
            .get(&crate::components::beliefs::bucket_position(80, 80))
            .unwrap()
            .prey_yield
            .value;
        assert!(
            long_v < short_v,
            "longer search ({long_v}) should drop prey_yield more than shorter ({short_v})"
        );
    }

    #[test]
    fn scent_detected_weakly_lifts_actor_prey_yield() {
        let (mut world, mut schedule) = test_world(100);
        let actor = spawn_cat(&mut world, Position::new(50, 50));
        world.write_message(WitnessableEvent::HuntScentDetected {
            actor,
            prey_kind: crate::components::prey::PreyKind::Mouse,
            position: Position::new(50, 50),
            tick: 100,
        });
        schedule.run(&mut world);
        let key = crate::components::beliefs::bucket_position(50, 50);
        let model = world
            .get::<LocationBeliefs>(actor)
            .unwrap()
            .models
            .get(&key)
            .expect("scent should establish a LocationBeliefs entry");
        // Scent's observed value sits between neutral (0.5) and
        // OBSERVED_MAX; the lift moves the default (0.0) toward it but
        // less than a successful Hunt would.
        assert!(model.prey_yield.value > 0.0);
        assert!(model.prey_yield.strength > 0.0);
    }
}
