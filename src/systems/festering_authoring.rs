//! Ticket 472 — author festering wounds from magic-misfire `WoundTransfer`
//! events.
//!
//! Drains `MessageReader<MisfireEffect>` each tick, filters for the
//! `WoundTransfer` arm, rolls `misfire_festering_chance`, and on hit
//! applies a `WoundKind::Festering` wound to a randomly-selected body
//! part of the caster via `damage_to_body_part_with_kind` — which in
//! turn emits a `BodyPartInjury` (caught downstream by ticket 471's
//! `cache_last_body_part_injury` for death attribution, and consumed
//! by the festering observation emitter for the belief-layer
//! perception lift).
//!
//! Substrate-honest stance per CLAUDE.md "All multi-tick aspirations
//! are HTN methods": the festering wound is the *substrate anchor*
//! the `SeekHealing` HTN method (dormant on `blocker: 473`) becomes
//! applicable for. Authoring lives here rather than inside
//! `apply_misfire` itself because the alternative — threading
//! `&mut CatBodyModel` / `&mut MessageWriter<BodyPartInjury>` /
//! `&mut SimRng` / `&mut SystemActivation` through 4 step resolvers
//! and 2 Bevy systems — exceeded the cost of a 1-tick deferred
//! authoring (no DSE reads the body model intra-tick in a way that
//! depends on this).

use bevy_ecs::prelude::*;

use crate::components::body_zones::{BodyPart, CatBodyModel, WoundKind};
use crate::components::magic::MisfireEffectKind;
use crate::components::mental::FesteringObservationCooldown;
use crate::components::physical::{Dead, InjurySource, Position};
use crate::messages::body_part_injury::BodyPartInjury;
use crate::messages::misfire_effect::MisfireEffect;
use crate::messages::witnessable_event::WitnessableEvent;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::SystemActivation;
use crate::resources::time::TimeState;
use rand::Rng;

/// 472 — drain `MisfireEffect` and author `WoundKind::Festering` on the
/// caster's randomly-selected body part for the `WoundTransfer` arm,
/// gated by `MagicConstants::misfire_festering_chance`.
pub fn author_festering_from_misfire(
    mut reader: MessageReader<MisfireEffect>,
    mut cats: Query<&mut CatBodyModel, Without<Dead>>,
    mut rng: ResMut<SimRng>,
    mut writer: MessageWriter<BodyPartInjury>,
    mut activation: ResMut<SystemActivation>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
) {
    let combat = &constants.combat;
    let magic = &constants.magic;
    for msg in reader.read() {
        if msg.kind != MisfireEffectKind::WoundTransfer {
            continue;
        }
        let Ok(mut body_model) = cats.get_mut(msg.entity) else {
            continue;
        };
        if rng.rng.random::<f32>() >= magic.misfire_festering_chance {
            continue;
        }
        // 472 — apply festering damage on a random body part. The
        // `damage_to_body_part_with_kind` helper handles the
        // negligible-damage guard, RNG body-part selection, condition
        // promotion, and `BodyPartInjury` emit (which 471's
        // `cache_last_body_part_injury` drains for death attribution).
        crate::systems::combat::damage_to_body_part_with_kind(
            msg.entity,
            &mut body_model,
            magic.festering_seed_damage,
            WoundKind::Festering,
            time.tick,
            InjurySource::MagicMisfire,
            combat,
            &mut rng,
            &mut writer,
            &mut activation,
            // 477 — physical armor does not blunt magical / festering
            // damage (deliberate doctrine call); pass no equipment.
            None,
            None,
        );
    }
}

/// 472 — broadcast `CarriesFesteringWound` from cats with a festering
/// wound at a throttled cadence. Festering is a *persistent state*
/// (the slow heal rate makes it linger), so the emit fires once per
/// `festering_observation_interval_ticks` per cat rather than every
/// tick. `belief_integrator::apply_observation` handles the witness-
/// range filter; this system's job is just emitting one message per
/// festering cat per interval.
///
/// Per-part: the system emits one event for the most-severe festering
/// part (highest `tissue_damage`). Multiple festering parts collapse
/// into one observation — the belief lift is on the *cat*, not per-
/// part, so the strongest signal dominates.
pub fn emit_festering_observations(
    mut commands: Commands,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut events: MessageWriter<WitnessableEvent>,
    cats: Query<
        (
            Entity,
            &Position,
            &CatBodyModel,
            Option<&FesteringObservationCooldown>,
        ),
        Without<Dead>,
    >,
) {
    let tick = time.tick;
    let interval = constants.magic.festering_observation_interval_ticks;
    for (entity, pos, body_model, cooldown) in &cats {
        // Pick the most-severe festering part (and skip the cat if
        // none are festering).
        let mut most_severe: Option<(BodyPart, f32)> = None;
        for (part, state) in body_model.iter() {
            if state.kind != WoundKind::Festering {
                continue;
            }
            let severity = state.tissue_damage;
            if most_severe.map(|(_, s)| severity > s).unwrap_or(true) {
                most_severe = Some((part, severity));
            }
        }
        let Some((body_part, severity)) = most_severe else {
            continue;
        };
        // Throttle: emit once per interval per cat.
        let on_cooldown = cooldown
            .and_then(|c| c.last_emit_tick)
            .is_some_and(|last| tick.saturating_sub(last) < interval);
        if on_cooldown {
            continue;
        }
        events.write(WitnessableEvent::CarriesFesteringWound {
            actor: entity,
            body_part,
            // Pre-473 the only festering author is the magic-misfire
            // WoundTransfer path. When the corrupted-kin perception
            // map (473) starts emitting from other sources, the
            // source_kind will diverge per festering author.
            source_kind: InjurySource::MagicMisfire,
            severity,
            position: *pos,
            tick,
        });
        commands
            .entity(entity)
            .insert(FesteringObservationCooldown {
                last_emit_tick: Some(tick),
            });
    }
}
