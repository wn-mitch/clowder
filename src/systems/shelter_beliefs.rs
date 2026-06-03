//! Shelter-belief substrate systems (ticket 374).
//!
//! Three concerns, kept in one module for cohesion:
//!
//! - **`claim_home_dens`** — per-stagger pass that assigns
//!   `ShelterBeliefs.home_den` to cats without one (nearest functional
//!   Den within `claim_search_radius`) and clears it when the claimed
//!   Den entity is missing or non-functional. Emits `DenClaimed` /
//!   `DenLost` for the integrator.
//! - **`update_shelter_continuity`** — per-stagger accrual or decay of
//!   `ShelterFacet.continuity` based on proximity to the cat's
//!   `home_den`. Not event-driven — continuity tracks lived experience.
//! - **`emit_den_condition_events`** — per-stagger scan of `Structure`
//!   conditions; emits `DenDamaged` / `DenRepaired` on threshold
//!   crossings (between effectiveness knees at 0.2 and 0.5).
//! - **`detect_den_sieges`** — per-stagger scan of fox counts within
//!   `siege_proximity` of each Den; emits `DenSieged` /
//!   `DenSiegeBroken` on 0↔positive transitions.
//!
//! All four systems run on the same stagger cadence
//! (`BeliefsConstants::decay_stagger_period`) as the belief integrator's
//! Pass B — amortizes cost and keeps the shelter substrate on the same
//! observation cadence as the rest of the belief layer.

use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::beliefs::{ShelterBeliefs, ShelterFacet};
use crate::components::building::{Structure, StructureType};
use crate::components::physical::{Dead, Position};
use crate::components::wildlife::{WildAnimal, WildSpecies};
use crate::messages::witnessable_event::{DenLostReason, WitnessableEvent};
use crate::resources::sim_constants::ShelterBeliefConstants;
use crate::resources::time::TimeState;
use crate::resources::SimConstants;

/// Distance within which a homeless cat will claim the nearest
/// functional Den. Generous enough to cover the colony spawn area at
/// world setup so founders pick up a home_den on the first stagger.
const CLAIM_SEARCH_RADIUS: f32 = 40.0;

/// 374: per-cat shelter security in `[0.0, 1.0]`. Used by both the
/// welfare rollup (`colony_score::compute_shelter`) and the
/// coordinator pressure accumulator. The four sub-axes compose
/// multiplicatively: `belonging * quality * (1 - threat)` is the base
/// security; `continuity` enters as a confidence multiplier mixed by
/// `continuity_weight` (0 = ignore continuity, 1 = full weighting).
///
/// Range guarantee: every factor is `[0, 1]` and the final clamp
/// guards against floating-point drift; callers can rely on the
/// result being in `[0, 1]` exactly.
pub fn shelter_security(facet: &ShelterFacet, cfg: &ShelterBeliefConstants) -> f32 {
    let base = facet.belonging * facet.quality * (1.0 - facet.threat);
    let continuity_factor =
        cfg.continuity_weight * facet.continuity + (1.0 - cfg.continuity_weight);
    (base * continuity_factor).clamp(0.0, 1.0)
}

/// 374: per-cat housing insecurity — the complement of
/// [`shelter_security`] without the continuity factor. Used by the
/// coordinator to decide which cats are insecure enough to count
/// against `pressure.shelter`. Continuity is intentionally omitted
/// from the pressure trigger — a cat *just* claimed a den (high
/// belonging, but continuity still ramping) shouldn't keep firing
/// build pressure as if it were homeless.
pub fn housing_insecurity(facet: &ShelterFacet) -> f32 {
    (1.0 - facet.belonging * facet.quality * (1.0 - facet.threat)).clamp(0.0, 1.0)
}

/// Per-stagger pass: any living cat with `home_den == None` claims the
/// nearest functional Den within [`CLAIM_SEARCH_RADIUS`]. Any cat whose
/// claimed Den has been despawned or has fallen below the structural
/// effectiveness floor loses the claim and emits `DenLost`.
///
/// The cadence (every `decay_stagger_period` ticks, phase-staggered by
/// `entity.index()`) matches the belief integrator's Pass B so cats
/// don't drift between observations on different beats.
pub fn claim_home_dens(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut events: MessageWriter<WitnessableEvent>,
    mut cats: Query<(Entity, &Position, &mut ShelterBeliefs), Without<Dead>>,
    dens: Query<(Entity, &Position, &Structure)>,
) {
    let tick = time.tick;
    let period = constants.beliefs.decay_stagger_period.max(1);
    let tick_phase = tick % period;

    for (cat_ent, cat_pos, mut shelter) in cats.iter_mut() {
        if (cat_ent.index_u32() as u64) % period != tick_phase {
            continue;
        }

        // Check existing claim's liveness. A despawned Den entity
        // resolves to Err from the dens query; a decayed Den resolves
        // to Ok but with `effectiveness() == 0.0`. Both lose the claim.
        if let Some(claimed) = shelter.home_den {
            let lost = match dens.get(claimed) {
                Err(_) => Some((DenLostReason::Destroyed, *cat_pos)),
                Ok((_, den_pos, structure)) => {
                    if structure.kind != StructureType::Den || structure.effectiveness() <= 0.0 {
                        Some((DenLostReason::Destroyed, *den_pos))
                    } else {
                        None
                    }
                }
            };
            if let Some((reason, position)) = lost {
                events.write(WitnessableEvent::DenLost {
                    cat: cat_ent,
                    den: claimed,
                    reason,
                    position,
                    tick,
                });
                // The integrator will clear home_den on the event; we
                // also clear here so this pass's claim attempt below
                // sees the None state and can re-claim immediately
                // rather than waiting another stagger.
                shelter.home_den = None;
            }
        }

        if shelter.home_den.is_some() {
            continue;
        }

        // Find nearest functional Den within search radius.
        let mut best: Option<(Entity, Position, f32)> = None;
        for (den_ent, den_pos, structure) in dens.iter() {
            if structure.kind != StructureType::Den || structure.effectiveness() <= 0.0 {
                continue;
            }
            let center = structure.center(den_pos);
            let d = cat_pos.distance_to(&center);
            if d > CLAIM_SEARCH_RADIUS {
                continue;
            }
            if best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                best = Some((den_ent, center, d));
            }
        }
        if let Some((den_ent, center, _)) = best {
            // Re-fetch to read condition; the loop borrow above
            // already released `dens` reads.
            let condition = dens
                .get(den_ent)
                .map(|(_, _, s)| s.condition)
                .unwrap_or(1.0);
            events.write(WitnessableEvent::DenClaimed {
                cat: cat_ent,
                den: den_ent,
                position: center,
                condition,
                tick,
            });
            // Set immediately so a follow-up emit pass in the same
            // tick doesn't double-claim. The integrator's lift on
            // `belonging` is idempotent if home_den is already set.
            shelter.home_den = Some(den_ent);
        }
    }
}

/// Per-stagger continuity update. For each cat with a claimed
/// `home_den`, accrue continuity when the cat is within
/// `home_den_radius` of the den's center; decay otherwise. Cats
/// without a `home_den` have nothing to track — continuity is reset to
/// zero by the `DenLost` integrator arm and stays there until a fresh
/// claim re-arms accrual.
pub fn update_shelter_continuity(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut cats: Query<(Entity, &Position, &mut ShelterBeliefs), Without<Dead>>,
    dens: Query<(&Position, &Structure)>,
) {
    let tick = time.tick;
    let cfg = &constants.shelter_beliefs;
    let period = constants.beliefs.decay_stagger_period.max(1);
    let tick_phase = tick % period;

    for (cat_ent, cat_pos, mut shelter) in cats.iter_mut() {
        if (cat_ent.index_u32() as u64) % period != tick_phase {
            continue;
        }
        let Some(home) = shelter.home_den else {
            continue;
        };
        let Ok((den_pos, structure)) = dens.get(home) else {
            continue;
        };
        let center = structure.center(den_pos);
        let at_home = cat_pos.distance_to(&center) <= cfg.home_den_radius;
        let delta = if at_home {
            cfg.continuity_accrual_per_stagger
        } else {
            -cfg.continuity_decay_per_stagger
        };
        shelter.facet.continuity = (shelter.facet.continuity + delta).clamp(0.0, 1.0);
        shelter.facet.last_updated_tick = tick;
    }
}

/// Per-stagger scan of `Structure::condition` transitions across the
/// effectiveness knees (default 0.2 / 0.5). Emits `DenDamaged` on
/// downward crossings and `DenRepaired` on upward crossings. Tracks
/// previous condition in a `Local<HashMap>` so re-runs across ticks
/// detect transitions without a per-Den marker component.
pub fn emit_den_condition_events(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut events: MessageWriter<WitnessableEvent>,
    dens: Query<(Entity, &Position, &Structure)>,
    mut prev: Local<HashMap<Entity, f32>>,
) {
    let tick = time.tick;
    let period = constants.beliefs.decay_stagger_period.max(1);
    if !tick.is_multiple_of(period) {
        return;
    }
    let cfg = &constants.shelter_beliefs;
    let thresholds = [cfg.damage_threshold_low, cfg.damage_threshold_high];

    // Collect current live Den entities so we can drop stale prev
    // entries without iterating the whole map per-tick.
    let mut seen: Vec<Entity> = Vec::with_capacity(prev.len());
    for (den_ent, den_pos, structure) in dens.iter() {
        if structure.kind != StructureType::Den {
            continue;
        }
        seen.push(den_ent);
        let center = structure.center(den_pos);
        let current = structure.condition;
        let previous = prev.get(&den_ent).copied().unwrap_or(current);
        for &t in &thresholds {
            // Downward crossing: previous strictly above, current at/below.
            if previous > t && current <= t {
                events.write(WitnessableEvent::DenDamaged {
                    den: den_ent,
                    position: center,
                    old_condition: previous,
                    new_condition: current,
                    tick,
                });
            }
            // Upward crossing.
            if previous < t && current >= t {
                events.write(WitnessableEvent::DenRepaired {
                    den: den_ent,
                    position: center,
                    old_condition: previous,
                    new_condition: current,
                    tick,
                });
            }
        }
        prev.insert(den_ent, current);
    }
    // Drop entries for despawned Dens.
    if prev.len() > seen.len() {
        let live: std::collections::HashSet<Entity> = seen.into_iter().collect();
        prev.retain(|e, _| live.contains(e));
    }
}

/// Per-stagger siege detector. For each Den, count nearby foxes
/// (Fox / ShadowFox); emit `DenSieged` on a 0→positive transition and
/// `DenSiegeBroken` on a positive→0 transition. Previous fox count is
/// kept in a `Local<HashMap>` keyed by Den entity.
pub fn detect_den_sieges(
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    mut events: MessageWriter<WitnessableEvent>,
    dens: Query<(Entity, &Position, &Structure)>,
    foxes: Query<(&Position, &WildAnimal), Without<Dead>>,
    mut prev: Local<HashMap<Entity, u32>>,
) {
    let tick = time.tick;
    let period = constants.beliefs.decay_stagger_period.max(1);
    if !tick.is_multiple_of(period) {
        return;
    }
    let cfg = &constants.shelter_beliefs;

    let mut seen: Vec<Entity> = Vec::with_capacity(prev.len());
    for (den_ent, den_pos, structure) in dens.iter() {
        if structure.kind != StructureType::Den {
            continue;
        }
        seen.push(den_ent);
        let center = structure.center(den_pos);
        let mut foxes_present: u32 = 0;
        for (fox_pos, animal) in foxes.iter() {
            if !matches!(animal.species, WildSpecies::Fox | WildSpecies::ShadowFox) {
                continue;
            }
            if fox_pos.distance_to(&center) <= cfg.siege_proximity {
                foxes_present += 1;
            }
        }
        let previous = prev.get(&den_ent).copied().unwrap_or(0);
        if previous == 0 && foxes_present > 0 {
            events.write(WitnessableEvent::DenSieged {
                den: den_ent,
                position: center,
                foxes_present,
                tick,
            });
        } else if previous > 0 && foxes_present == 0 {
            events.write(WitnessableEvent::DenSiegeBroken {
                den: den_ent,
                position: center,
                tick,
            });
        }
        prev.insert(den_ent, foxes_present);
    }
    if prev.len() > seen.len() {
        let live: std::collections::HashSet<Entity> = seen.into_iter().collect();
        prev.retain(|e, _| live.contains(e));
    }
}
