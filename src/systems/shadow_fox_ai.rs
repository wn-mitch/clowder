//! Shadow-fox decision stack, extracted verbatim from `wildlife.rs`
//! (ticket 351 — pure code motion, byte-identical gate).
//!
//! Every system here is shadow-fox-only in production: `wildlife_ai`'s
//! query excludes `FoxState` / `HawkState` / `SnakeState` (all other wild
//! species run their own GOAP loops), and `predator_stalk_cats` filters
//! `With<ShadowFoxDrives>`. The legacy system names are kept by this
//! ticket; renames ship with ticket 310's first behavior stage so the
//! byte-identity gate stays a pure-motion claim.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::identity::Name;
use crate::components::magic::Ward;
use crate::components::mental::{Mood, MoodModifier, MoodSource};
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::PreyAnimal;
use crate::components::wildlife::{
    FoxState, HawkState, ShadowFoxDrives, SnakeState, WildAnimal, WildSpecies, WildlifeAiState,
};
use crate::resources::cat_scent_map::CatScentMap;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// Wildlife AI system
// ---------------------------------------------------------------------------

/// Move each wild animal according to its behavior pattern.
///
/// Ticket 025 Phase 2 — `Without<FoxState>` was the legacy fox-cutover
/// filter; the cutover commit widens it to also exclude
/// `HawkState` / `SnakeState` so post-cutover hawks and snakes flow
/// through their own GOAP loops (`hawk_goap.rs` / `snake_goap.rs`).
/// ShadowFox (which carries `ShadowFoxDrives` and none of the
/// `*State` markers) still uses this legacy `Circling`/`Waiting`
/// state machine.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
/// ## Movement contract (140 step 11 / ticket 310 seam)
///
/// Decision layers write [`DesiredVelocity`] via steering — never
/// `Position`; `MovementBudget.per_tick` is the speed cap the Chain-4
/// integrator enforces. Every arm keeps its **tile-grid decision
/// reads** (ward-coverage / cat-scent lookahead samples the tile ahead
/// along the heading; patrol-terrain and wildlife-passability checks
/// gate the aimed tile) per the epic constraint — only the *motion*
/// is continuous. The pre-140 `budget.try_spend_step()` direct writes
/// are retired; the integrator's wall-slide + anti-strand hatch own
/// collision (the old Fleeing arm's terrain-unchecked write — the
/// fox-lake strand class — is structurally closed by this migration).
pub fn wildlife_ai(
    mut query: Query<
        (
            &WildAnimal,
            &Position,
            &mut crate::components::physical::DesiredVelocity,
            &mut WildlifeAiState,
            Has<ShadowFoxDrives>,
        ),
        (Without<FoxState>, Without<HawkState>, Without<SnakeState>),
    >,
    wards: Query<(&Ward, &Position), Without<WildAnimal>>,
    cat_positions: Query<
        &Position,
        (
            With<Needs>,
            Without<Dead>,
            Without<PreyAnimal>,
            Without<WildAnimal>,
        ),
    >,
    mut map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    ward_coverage: Res<crate::resources::WardCoverageMap>,
    cat_scent: Res<CatScentMap>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.wildlife;
    let ward_multiplier = constants.magic.shadow_fox_ward_repel_multiplier;
    let ward_avoid_threshold = c.shadow_fox_ward_avoid_threshold;
    let cat_scent_avoid_threshold = constants.fox_ecology.cat_scent_avoidance_threshold;

    // 260: snapshot kept *only* for siege state (ward identity at
    // siege start, alive-check during encircle, orbit radius). The
    // avoidance decision now reads `WardCoverageMap` directly, making
    // shadow-fox-vs-ward avoidance trace-visible through the
    // `ward_coverage` InfluenceMap metadata. Pre-260, this snapshot
    // doubled as the avoidance signal — a hardcoded side-channel
    // CLAUDE.md's "Substrate over hacks" pillar prohibits.
    let ward_positions: Vec<(Position, f32)> = wards
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius() * ward_multiplier))
        .collect();

    for (animal, pos, mut desired, mut ai_state, is_shadow_fox) in &mut query {
        let species_speed = constants.movement.max_speed(animal.species);
        match *ai_state {
            WildlifeAiState::Patrolling { dx, dy } => {
                // 260: shadow-fox orthogonal-axis avoidance.
                // - Magic channel: `WardCoverageMap` (Sight × Colony).
                // - Scent channel: `CatScentMap` (Scent × Colony).
                // Both fire independently; ward triggers the siege
                // roll, scent only reverses (cat scent isn't a thing
                // to siege).
                if is_shadow_fox {
                    let next = Position::new(pos.x() + dx, pos.y() + dy);
                    if ward_coverage.get(next.x(), next.y()) >= ward_avoid_threshold {
                        // For siege geometry we still need the ward's
                        // entity-level (x, y); pull the nearest from
                        // the snapshot so the EncirclingWard branch
                        // can orbit a concrete ward, not a grid cell.
                        let siege_anchor = ward_positions
                            .iter()
                            .min_by_key(|(wp, _)| next.tile_distance_squared(wp));
                        if let Some((wp, _radius)) = siege_anchor {
                            if rng.rng.random::<f32>() < c.ward_siege_chance {
                                *ai_state = WildlifeAiState::EncirclingWard {
                                    ward_x: wp.x(),
                                    ward_y: wp.y(),
                                    angle: 0.0,
                                    ticks: 0,
                                };
                                activation.record(Feature::WardSiegeStarted);
                            } else {
                                *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                            }
                            activation.record(Feature::ShadowFoxAvoidedWard);
                            continue;
                        }
                    }
                    if cat_scent.get(next.x(), next.y()) >= cat_scent_avoid_threshold {
                        *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                        activation.record(Feature::ShadowFoxAvoidedCatScent);
                        continue;
                    }
                }

                let next = Position::new(pos.x() + dx, pos.y() + dy);
                if map.in_bounds(next.x(), next.y())
                    && is_patrol_terrain(map.get(next.x(), next.y()).terrain, animal.species)
                {
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                } else {
                    // Reverse direction and try the other way (patrol
                    // terrain isn't required on the reverse tile —
                    // pre-140 parity; the integrator still refuses
                    // impassable ground).
                    let rev = Position::new(pos.x() - dx, pos.y() - dy);
                    if map.in_bounds(rev.x(), rev.y()) {
                        *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                        desired.0 = Some(crate::ai::steering::seek(pos.0, rev.0, species_speed));
                    }
                    // If neither works, stay put (cornered).
                }
            }
            WildlifeAiState::Circling {
                center_x,
                center_y,
                ref mut angle,
            } => {
                *angle += c.circling_angle_step;
                if *angle > std::f32::consts::TAU {
                    *angle -= std::f32::consts::TAU;
                }
                let radius = c.circling_radius;
                let target_x = center_x + (angle.cos() * radius) as i32;
                let target_y = center_y + (angle.sin() * radius) as i32;

                // Desire one heading-step toward the circle target.
                let dx = (target_x - pos.x()).signum();
                let dy = (target_y - pos.y()).signum();
                let next = Position::new(pos.x() + dx, pos.y() + dy);
                if map.in_bounds(next.x(), next.y())
                    && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                {
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                }
            }
            WildlifeAiState::Waiting => {
                // Ambush: don't move.
            }
            WildlifeAiState::Fleeing { dx, dy } => {
                let next = Position::new(pos.x() + dx, pos.y() + dy);
                if map.in_bounds(next.x(), next.y()) {
                    // Terrain-unchecked heading desire — pre-140 this
                    // arm WROTE the position unchecked (the fox-lake
                    // strand class); the integrator's passability +
                    // wall-slide now own collision, so a fleeing
                    // animal skirts water instead of entering it.
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                }
            }
            WildlifeAiState::EncirclingWard {
                ward_x,
                ward_y,
                ref mut angle,
                ref mut ticks,
            } => {
                *ticks += 1;

                // Check if ward still exists (not destroyed).
                let ward_alive = ward_positions
                    .iter()
                    .any(|(wp, _)| wp.x() == ward_x && wp.y() == ward_y);

                // Break siege if cat approaches or ward destroyed or timed out.
                // Phase 5a: shadow-fox sight channel with LoS check.
                let cat_nearby = cat_positions.iter().any(|cp| {
                    crate::systems::sensing::observer_sees_at_with_los(
                        crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
                        *pos,
                        &constants.sensory.shadow_fox,
                        *cp,
                        crate::components::SensorySignature::CAT,
                        c.siege_break_range,
                        &map,
                    )
                });
                if !ward_alive || *ticks >= c.ward_siege_max_ticks {
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                } else if cat_nearby {
                    // Aggression: siege provokes confrontation.
                    // 140 step 8/11 — Manhattan nearest-pick retired;
                    // tile-Euclidean² matches the `distance_to` metric.
                    if let Some(cat_pos) = cat_positions
                        .iter()
                        .min_by_key(|cp| pos.tile_distance_squared(cp))
                    {
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x(),
                            target_y: cat_pos.y(),
                        };
                    }
                } else {
                    // Orbit at ward edge + 1 tile.
                    *angle += c.circling_angle_step;
                    if *angle > std::f32::consts::TAU {
                        *angle -= std::f32::consts::TAU;
                    }
                    let orbit_radius = ward_positions
                        .iter()
                        .find(|(wp, _)| wp.x() == ward_x && wp.y() == ward_y)
                        .map(|(_, r)| *r + 1.0)
                        .unwrap_or(4.0);
                    let tx = ward_x + (angle.cos() * orbit_radius) as i32;
                    let ty = ward_y + (angle.sin() * orbit_radius) as i32;
                    let dx = (tx - pos.x()).signum();
                    let dy = (ty - pos.y()).signum();
                    let next = Position::new(pos.x() + dx, pos.y() + dy);
                    if map.in_bounds(next.x(), next.y())
                        && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                    {
                        desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                    }

                    // Deposit siege corruption at 3x normal rate.
                    if map.in_bounds(pos.x(), pos.y()) {
                        let tile = map.get_mut(pos.x(), pos.y());
                        tile.corruption = (tile.corruption + c.ward_siege_corruption_rate).min(1.0);
                    }
                }
            }

            WildlifeAiState::Stalking { target_x, target_y } => {
                // Shadow fox ward avoidance: cancel stalk if next step would
                // enter a ward-covered tile (260: was a hardcoded
                // `ward_positions` distance check; now reads the same
                // `WardCoverageMap` threshold as the patrol-step branch).
                if is_shadow_fox {
                    let dx = (target_x - pos.x()).signum();
                    let dy = (target_y - pos.y()).signum();
                    let next = Position::new(pos.x() + dx, pos.y() + dy);
                    if ward_coverage.get(next.x(), next.y()) >= ward_avoid_threshold {
                        *ai_state = WildlifeAiState::Patrolling { dx: -dx, dy: -dy };
                        activation.record(Feature::ShadowFoxAvoidedWard);
                        continue;
                    }
                }

                // Desire one heading-step toward the target cat.
                let dx = (target_x - pos.x()).signum();
                let dy = (target_y - pos.y()).signum();
                let next = Position::new(pos.x() + dx, pos.y() + dy);
                if map.in_bounds(next.x(), next.y())
                    && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                {
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                } else {
                    // Can't reach target, revert to patrolling.
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                }
            }
            // ---- Ticket 023 Phase B: motivation-driven states ----
            // These variants are only written by `shadowfox_motivation_tick`
            // and only ever appear on entities with `ShadowFoxDrives`.
            WildlifeAiState::Reconstituting { tile_x, tile_y } => {
                // Move one step toward the high-corruption recovery tile;
                // stay put once on it. Coherence recovery continues to
                // run through `shadowfox_coherence_tick`, which already
                // gives a multiplier for `tile_corruption > recovery_threshold`.
                if pos.x() == tile_x && pos.y() == tile_y {
                    // Already on target — hold position.
                } else {
                    let dx = (tile_x - pos.x()).signum();
                    let dy = (tile_y - pos.y()).signum();
                    let next = Position::new(pos.x() + dx, pos.y() + dy);
                    if map.in_bounds(next.x(), next.y())
                        && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                    {
                        desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                    }
                }
            }
            WildlifeAiState::Tending {
                ward_x,
                ward_y,
                ref mut angle,
            } => {
                // Orbit the ward's perimeter at the corruption-deposit
                // rate. Distinct from `EncirclingWard` siege (which uses
                // the much-faster `ward_siege_corruption_rate`); Tending
                // is the slow corruption-gardener pass that re-stamps
                // perimeter tiles cats have cleansed.
                *angle += c.circling_angle_step;
                if *angle > std::f32::consts::TAU {
                    *angle -= std::f32::consts::TAU;
                }
                let orbit_radius = ward_positions
                    .iter()
                    .find(|(wp, _)| wp.x() == ward_x && wp.y() == ward_y)
                    .map(|(_, r)| *r + 1.0)
                    .unwrap_or(4.0);
                let tx = ward_x + (angle.cos() * orbit_radius) as i32;
                let ty = ward_y + (angle.sin() * orbit_radius) as i32;
                let dx = (tx - pos.x()).signum();
                let dy = (ty - pos.y()).signum();
                let next = Position::new(pos.x() + dx, pos.y() + dy);
                if map.in_bounds(next.x(), next.y())
                    && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                {
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                }
                // The corruption deposit happens via the existing
                // shadow-fox-step deposit further down — keeps
                // Tending's per-tick rate consistent with patrol.
            }
            WildlifeAiState::Haunting {
                target_x,
                target_y,
                edge_distance,
                ticks: _,
            } => {
                // Pace at edge_distance from the target. If we're
                // closer than edge_distance, step away; if we're
                // farther, step toward. Phase B is detection-only;
                // Phase C wires the safety/mood drain when within
                // haunting_drain_radius.
                let dx_t = target_x - pos.x();
                let dy_t = target_y - pos.y();
                let dist = (dx_t.abs() + dy_t.abs()) as f32;
                let (step_dx, step_dy) = if dist < edge_distance {
                    // Too close — step directly away.
                    (-dx_t.signum(), -dy_t.signum())
                } else if dist > edge_distance {
                    // Too far — step toward.
                    (dx_t.signum(), dy_t.signum())
                } else {
                    // At the edge — orbit slowly (perpendicular step
                    // along the larger axis).
                    if dx_t.abs() >= dy_t.abs() {
                        (0, 1)
                    } else {
                        (1, 0)
                    }
                };
                let next = Position::new(pos.x() + step_dx, pos.y() + step_dy);
                if map.in_bounds(next.x(), next.y())
                    && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                {
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                }
            }
            WildlifeAiState::Seeding {
                frontier_x,
                frontier_y,
            } => {
                // Move one step toward the frontier tile, depositing
                // corruption via the existing shadow-fox-step deposit.
                let dx = (frontier_x - pos.x()).signum();
                let dy = (frontier_y - pos.y()).signum();
                let next = Position::new(pos.x() + dx, pos.y() + dy);
                if map.in_bounds(next.x(), next.y())
                    && map.get(next.x(), next.y()).terrain.is_wildlife_passable()
                {
                    desired.0 = Some(crate::ai::steering::seek(pos.0, next.0, species_speed));
                }
            }
            // ---- Ticket 310 S2: post-ambush retreat ----
            WildlifeAiState::Retreating { den_x, den_y } => {
                // SingleMinded home leg: `arrive` decelerates into the
                // den instead of overshooting; release to Patrolling
                // within the arrival radius. No ward/scent lookahead —
                // the den sits in corrupted territory and the fox is
                // leaving the colony, not probing it. (The
                // `predator_stalk_cats` ward-flee check still overrides
                // a retreat that crosses live ward coverage.)
                let den = Position::new(den_x, den_y);
                if pos.distance_to(&den) <= c.shadow_fox_retreat_arrival_radius {
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                } else {
                    desired.0 = Some(crate::ai::steering::arrive(
                        pos.0,
                        den.0,
                        species_speed,
                        c.shadow_fox_retreat_arrive_slow_radius,
                    ));
                }
            }
        }

        // ShadowFox spreads corruption to tiles it crosses.
        if is_shadow_fox && map.in_bounds(pos.x(), pos.y()) {
            let tile = map.get_mut(pos.x(), pos.y());
            tile.corruption = (tile.corruption + c.shadow_fox_corruption_deposit).min(1.0);
        }

        // Small random direction jitter for patrol creatures to avoid getting stuck.
        if matches!(*ai_state, WildlifeAiState::Patrolling { .. })
            && rng.rng.random::<f32>() < c.patrol_jitter_chance
        {
            let new_dx = rng.rng.random_range(-1i32..=1);
            let new_dy = rng.rng.random_range(-1i32..=1);
            if new_dx != 0 || new_dy != 0 {
                *ai_state = WildlifeAiState::Patrolling {
                    dx: new_dx,
                    dy: new_dy,
                };
            }
        }
    }
}

/// Returns true if the given terrain is suitable for patrolling by this species.
fn is_patrol_terrain(terrain: Terrain, species: WildSpecies) -> bool {
    match species {
        WildSpecies::Fox => matches!(
            terrain,
            Terrain::LightForest | Terrain::DenseForest | Terrain::Grass
        ),
        WildSpecies::Hawk => matches!(
            terrain,
            Terrain::Grass | Terrain::Sand | Terrain::LightForest
        ),
        WildSpecies::Snake => matches!(terrain, Terrain::Rock | Terrain::Mud | Terrain::Grass),
        WildSpecies::ShadowFox => matches!(
            terrain,
            Terrain::LightForest | Terrain::DenseForest | Terrain::Grass
        ),
    }
}

// ---------------------------------------------------------------------------
// shadowfox_coherence_tick — ticket 023 Phase A
// ---------------------------------------------------------------------------

/// Tick each shadow-fox's `coherence` drive: decay on clean ground,
/// recovery on corrupted ground, dissolution at zero.
///
/// The slow-environmental defeat path that pairs with combat banishment.
/// A colony that aggressively cleanses corruption can starve shadow-foxes
/// without ever fighting one — they dissolve back into the substrate they
/// rose from.
///
/// Coherence-zero dissolution emits `EventKind::ShadowFoxDissolved` and
/// records `Feature::ShadowFoxDissolved` (rare-legend; not on the never-fired
/// canary). The mythic-register narrative line uses the design-doc phrasing.
#[allow(clippy::too_many_arguments)]
pub fn shadowfox_coherence_tick(
    mut query: Query<(Entity, &Position, &WildAnimal, &mut ShadowFoxDrives)>,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
    mut narrative: ResMut<NarrativeLog>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
) {
    let c = &constants.wildlife;
    for (entity, pos, _animal, mut drives) in &mut query {
        drives.age_ticks = drives.age_ticks.saturating_add(1);

        let tile_corruption = if map.in_bounds(pos.x(), pos.y()) {
            map.get(pos.x(), pos.y()).corruption
        } else {
            // Off-map shadow-foxes are about to be despawned by
            // `cleanup_wildlife`; treat as clean ground so coherence
            // decays one final time rather than spuriously recovering.
            0.0
        };

        // Hysteresis band: tiles between `decay_threshold` and
        // `recovery_threshold` produce no net change — keeps shadow-foxes
        // patrolling moderate-corruption corridors from oscillating
        // across a single boundary every tick. Design doc §"Coherence
        // mechanics" calls this the "flicker band" (0.2 / 0.5 default).
        if tile_corruption >= c.shadow_fox_coherence_recovery_threshold {
            drives.coherence =
                (drives.coherence + c.shadow_fox_coherence_recovery_corrupt).min(1.0);
        } else if tile_corruption <= c.shadow_fox_coherence_decay_threshold {
            drives.coherence = (drives.coherence - c.shadow_fox_coherence_decay_clean).max(0.0);
        }

        if drives.coherence <= c.shadow_fox_coherence_dissolution_threshold {
            activation.record(Feature::ShadowFoxDissolved);
            narrative.push(
                time.tick,
                "The shadow-fox flickers once, twice, and is gone — the corruption could not hold it together.".to_string(),
                NarrativeTier::Nature,
            );
            if let Some(ref mut elog) = event_log {
                elog.push(
                    time.tick,
                    crate::resources::event_log::EventKind::ShadowFoxDissolved {
                        location: (pos.x(), pos.y()),
                        age_ticks: drives.age_ticks,
                        final_corruption: tile_corruption,
                    },
                );
            }
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// shadowfox_motivation_tick — ticket 023 Phase B
// ---------------------------------------------------------------------------

/// Re-elect each shadow-fox's `WildlifeAiState` from a softmax over four
/// drive pressures (Coherence, Resonance, Dread, Entropy). Fires every
/// `shadow_fox_motivation_tick_cadence` ticks (default 16) — between
/// elections the existing `wildlife_ai` movement loop drives the
/// shadow-fox according to its current state.
///
/// Phase B uses shallow pressure scoring (nearest-tile/cat heuristics);
/// Phase C deepens Dread to read cat mood/safety/ally counts and wires
/// the per-tick haunting drain. Stored drive fields on `ShadowFoxDrives`
/// reflect the most-recent election's pressures, surfacing the
/// motivation landscape in the focal-cat trace.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn shadowfox_motivation_tick(
    mut query: Query<
        (
            &Position,
            &mut WildlifeAiState,
            &mut ShadowFoxDrives,
            // 310 S3 — kill-site memory filters the hunger target;
            // Option so pre-S3 saves (no beliefs component) keep
            // electing unfiltered.
            Option<&crate::components::wildlife::ShadowFoxBeliefs>,
        ),
        Without<crate::components::wildlife::Carcass>,
    >,
    wards: Query<(&Ward, &Position), Without<WildAnimal>>,
    cats: Query<
        (
            &Position,
            &Mood,
            Option<&crate::components::prev_safety_deficit::PrevSafetyDeficit>,
        ),
        (
            With<Needs>,
            Without<Dead>,
            Without<PreyAnimal>,
            Without<WildAnimal>,
        ),
    >,
    map: Res<TileMap>,
    ward_coverage: Res<crate::resources::WardCoverageMap>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
    mut rng: ResMut<SimRng>,
    mut activation: ResMut<SystemActivation>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
) {
    let c = &constants.wildlife;
    let cadence = c.shadow_fox_motivation_tick_cadence.max(1);
    if !time.tick.is_multiple_of(cadence) {
        return;
    }

    let scan_radius = c.shadow_fox_motivation_scan_radius;
    let resonance_weight = c.shadow_fox_resonance_weight;
    let entropy_scale = c.shadow_fox_entropy_distance_scale;
    let temp = c.shadow_fox_motivation_softmax_temp.max(1e-3);
    let jitter = c.shadow_fox_motivation_jitter;
    let haunt_edge = c.shadow_fox_haunting_edge_distance;
    let isolation_radius = c.shadow_fox_dread_isolation_radius;
    let group_threshold = c.shadow_fox_dread_group_threshold;
    let group_suppression = c.shadow_fox_dread_group_suppression;

    // Pre-collect ward positions + repel radii once per cadence; reused
    // by every shadow-fox iteration below.
    let ward_anchors: Vec<(Position, f32)> = wards
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius()))
        .collect();
    // Ticket 023 Phase C — deep Dread reads each cat's mood + safety
    // deficit alongside position so the motivation tick can pick the
    // psychologically vulnerable target rather than just the closest.
    // The isolation factor below is computed per-candidate against the
    // position list (O(N²) — cheap because cap=2 shadow-foxes × ~12
    // cats × cadence=16).
    let cat_data: Vec<(Position, f32, f32)> = cats
        .iter()
        .map(|(p, mood, prev_safety)| {
            let safety_deficit = prev_safety.map(|s| s.0).unwrap_or(0.0).clamp(0.0, 1.0);
            (*p, mood.valence, safety_deficit)
        })
        .collect();
    let cat_anchors: Vec<Position> = cat_data.iter().map(|(p, _, _)| *p).collect();

    for (pos, mut state, mut drives, beliefs) in &mut query {
        // 310 S1 — satiation decays once per motivation cadence,
        // *before* the Stalking/EncirclingWard guard below so a
        // besieging or actively-hunting shadow-fox gets hungrier too.
        drives.satiation = (drives.satiation - c.shadow_fox_satiation_decay_per_cadence).max(0.0);

        // Ticket 023 Phase C: leave active Stalking + EncirclingWard
        // alone. Both states are pre-Phase-B chains driven by
        // `predator_stalk_cats` / `wildlife_ai`'s siege branch — they
        // run to natural completion (ambush + cooldown, or siege
        // timeout). Without this guard, the motivation tick at every
        // cadence overwrites Stalking, preventing the chain from
        // reaching the adjacent-cell combat threshold and suppressing
        // ShadowFoxAmbush / ShadowFoxBanished — the mythic-texture
        // canary the Phase C iteration is trying to restore.
        if matches!(
            *state,
            WildlifeAiState::Stalking { .. }
                | WildlifeAiState::EncirclingWard { .. }
                // 310 S2 — Retreating is SingleMinded: held until
                // `wildlife_ai` releases it at the den.
                | WildlifeAiState::Retreating { .. }
        ) {
            continue;
        }

        // ---- Coherence pressure (state-derived, no scan) ----
        let coherence_pressure = (1.0 - drives.coherence).max(0.0).powi(2);

        // ---- Resonance pressure: corrupt tiles adjacent to ward zones ----
        // Walks the scan-radius bounding box and counts cells where
        // corruption > recovery_threshold AND ward_coverage > 0. Cheap
        // O(scan_radius²) — at default 12 that's 625 cells per shadow-
        // fox per cadence.
        let mut nearest_threatened: Option<Position> = None;
        let mut nearest_threatened_dist = i32::MAX;
        let mut threatened_count: u32 = 0;
        let mut nearest_frontier: Option<Position> = None;
        let mut nearest_frontier_dist = i32::MAX;
        let mut best_corruption_tile: Option<Position> = None;
        let mut best_corruption_value: f32 = -1.0;
        let scan_radius_i = scan_radius.round() as i32;
        for dy in -scan_radius_i..=scan_radius_i {
            for dx in -scan_radius_i..=scan_radius_i {
                let tx = pos.x() + dx;
                let ty = pos.y() + dy;
                if !map.in_bounds(tx, ty) {
                    continue;
                }
                let tile_corruption = map.get(tx, ty).corruption;
                let dist = dx.abs() + dy.abs();

                // Reconstituting target: highest-corruption tile in scan.
                if tile_corruption > best_corruption_value {
                    best_corruption_value = tile_corruption;
                    best_corruption_tile = Some(Position::new(tx, ty));
                }

                // Resonance: corrupt tile under ward pressure.
                if tile_corruption > c.shadow_fox_coherence_recovery_threshold
                    && ward_coverage.get(tx, ty) > 0.0
                {
                    threatened_count += 1;
                    if dist < nearest_threatened_dist {
                        nearest_threatened_dist = dist;
                        nearest_threatened = Some(Position::new(tx, ty));
                    }
                }

                // Entropy: frontier tile (corrupt with a clean neighbor).
                if tile_corruption > c.shadow_fox_coherence_recovery_threshold {
                    let has_clean_neighbor =
                        [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(ndx, ndy)| {
                            let nx = tx + ndx;
                            let ny = ty + ndy;
                            map.in_bounds(nx, ny) && map.get(nx, ny).corruption < 0.05
                        });
                    if has_clean_neighbor && dist < nearest_frontier_dist {
                        nearest_frontier_dist = dist;
                        nearest_frontier = Some(Position::new(tx, ty));
                    }
                }
            }
        }
        let resonance_pressure = (threatened_count as f32 * resonance_weight).min(1.0);

        // ---- Dread pressure (Phase C: vulnerability targeting) ----
        // Per-cat score = `(0.5 - 0.5 * mood.valence) * safety_deficit * isolation_factor`,
        // where isolation_factor is 1.0 when the cat has < group_threshold
        // allies nearby and `group_suppression` (e.g. 0.2) otherwise. The
        // best (most vulnerable) candidate's score becomes the Dread
        // pressure, and that cat's position is the Haunting target.
        // Phase B's shallow nearest-cat heuristic is retired; Phase C
        // makes Dread distinguish "easy psychological prey" from "well-
        // defended cat" in the substrate's L2 trace.
        let mut best_target: Option<Position> = None;
        let mut dread_pressure: f32 = 0.0;
        // 310 S1 — hunger targets the *nearest* scanned cat regardless
        // of psychological vulnerability (Dread's criterion); tracked
        // on the same scan so the two drives read one perception pass.
        // 310 S3 — the kill-site consideration: cats near this fox's
        // remembered kill site are ineligible while the memory is
        // fresh (predators don't hunt fished-out ponds). Applied at
        // target *selection*, never at the movement layer; when the
        // filter excludes a cat that would otherwise have been the
        // choice, `Feature::ShadowFoxKillSiteAvoided` names it.
        let kill_site_filter = beliefs.and_then(|b| {
            if b.kill_site_fresh(time.tick, c.shadow_fox_kill_site_memory_ticks) {
                b.last_kill_site.map(|(kx, ky)| Position::new(kx, ky))
            } else {
                None
            }
        });
        let mut nearest_cat_any: Option<Position> = None;
        let mut nearest_cat_any_dist = f32::INFINITY;
        let mut nearest_fished_out_dist = f32::INFINITY;
        for (cat_pos, mood, safety_deficit) in cat_data.iter() {
            let dist = pos.distance_to(cat_pos);
            if dist > scan_radius {
                continue;
            }
            let fished_out = kill_site_filter
                .map(|ks| cat_pos.distance_to(&ks) <= c.shadow_fox_kill_site_avoid_radius)
                .unwrap_or(false);
            if fished_out {
                nearest_fished_out_dist = nearest_fished_out_dist.min(dist);
            } else if dist < nearest_cat_any_dist {
                nearest_cat_any_dist = dist;
                nearest_cat_any = Some(*cat_pos);
            }
            // Mood term: 0.0 when valence == +1, 1.0 when valence == -1.
            let mood_term = (0.5 - 0.5 * mood.clamp(-1.0, 1.0)).clamp(0.0, 1.0);
            // Isolation: count allies (other cats) within isolation_radius.
            let ally_count = cat_anchors
                .iter()
                .filter(|other| {
                    let od = cat_pos.distance_to(other);
                    od > 0.0 && od <= isolation_radius
                })
                .count() as u32;
            let isolation_factor = if ally_count >= group_threshold {
                group_suppression
            } else {
                1.0
            };
            let score = mood_term * safety_deficit * isolation_factor;
            if score > dread_pressure {
                dread_pressure = score;
                best_target = Some(*cat_pos);
            }
        }
        let nearest_cat = best_target;
        // 310 S3 — a strictly nearer candidate was passed over for
        // memory, not geometry (order-independent: compares the best
        // fished-out distance against the final chosen distance).
        let kill_site_excluded_nearer = nearest_fished_out_dist < nearest_cat_any_dist;

        // ---- Entropy pressure: inverse distance to nearest frontier ----
        let entropy_pressure = nearest_frontier
            .map(|fp| {
                let d = pos.distance_to(&fp).max(0.0);
                1.0 / (1.0 + entropy_scale * d)
            })
            .unwrap_or(0.0);

        // ---- Hunger pressure (310 S1): fifth drive, conditional ----
        // `(1 − satiation)²` shaped like the Coherence pressure; the
        // weight is the conditional-axis switch — at 0.0 the fifth
        // score (and its jitter draw) is skipped entirely, restoring
        // the four-drive softmax byte-exactly.
        let hunger_active = c.shadow_fox_hunger_drive_weight > 0.0;
        let hunger_pressure = if hunger_active {
            (1.0 - drives.satiation).max(0.0).powi(2) * c.shadow_fox_hunger_drive_weight
        } else {
            0.0
        };
        // Eligibility, not just weight: hunger stands for election only
        // when its own pressure clears the floor. The four 023 drives
        // are benign at near-zero pressure (their states walk somewhere
        // and pace), so softmax temperature spread electing one is
        // harmless churn — but hunger elects Stalking, and the first
        // S1 gate soak showed the spread electing it at satiation 0.98
        // (pressure ~2e-5) whenever *another* drive opened the floor,
        // producing sub-cooldown ambush waves on a single cat. A fed
        // predator may not stand for the hunt election at all.
        let hunger_eligible =
            hunger_active && hunger_pressure >= c.shadow_fox_motivation_min_pressure;

        // Store the latest pressures on the component for trace observability.
        drives.resonance = resonance_pressure;
        drives.dread = dread_pressure;
        drives.entropy = entropy_pressure;
        drives.age_ticks = drives.age_ticks.saturating_add(0); // no-op; tick advanced by coherence system

        // ---- Pressure floor: only transition when a drive is genuinely
        // pressured. Without this guard, Phase B's softmax monotonically
        // pulls shadow-foxes out of Patrolling and the existing
        // Stalking → Ambush → Banishment chain (mythic-texture canary)
        // never fires. Falls through to leave the current state alone.
        let max_pressure = coherence_pressure
            .max(resonance_pressure)
            .max(dread_pressure)
            .max(entropy_pressure)
            .max(hunger_pressure);
        if max_pressure < c.shadow_fox_motivation_min_pressure {
            continue;
        }

        // ---- Softmax with jitter ----
        // 310 S1 — the score list is 4 or 5 entries; index 4, when
        // present, is the hunger drive (eligibility-gated above).
        let mut scores = vec![
            coherence_pressure,
            resonance_pressure,
            dread_pressure,
            entropy_pressure,
        ];
        if hunger_eligible {
            scores.push(hunger_pressure);
        }
        for s in scores.iter_mut() {
            // Symmetric uniform jitter; clamp so noisy ties never go negative.
            *s = (*s + rng.rng.random::<f32>() * 2.0 * jitter - jitter).max(0.0);
        }
        let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut exps = vec![0.0f32; scores.len()];
        let mut sum = 0.0f32;
        for (i, &s) in scores.iter().enumerate() {
            exps[i] = ((s - max) / temp).exp();
            sum += exps[i];
        }
        let sample = rng.rng.random::<f32>() * sum;
        let mut cum = 0.0f32;
        let mut winner = 0usize;
        for (i, &e) in exps.iter().enumerate() {
            cum += e;
            if sample <= cum {
                winner = i;
                break;
            }
        }

        // ---- Apply winning state if it actually changes ----
        let next_state: Option<WildlifeAiState> = match winner {
            0 => best_corruption_tile.map(|t| WildlifeAiState::Reconstituting {
                tile_x: t.x(),
                tile_y: t.y(),
            }),
            1 => nearest_threatened
                .zip(ward_anchors.iter().min_by_key(|(wp, _)| {
                    nearest_threatened
                        .map(|t| t.tile_distance_squared(wp))
                        .unwrap_or(i32::MAX)
                }))
                .map(
                    |(_threatened_tile, (ward_pos, _))| WildlifeAiState::Tending {
                        ward_x: ward_pos.x(),
                        ward_y: ward_pos.y(),
                        angle: 0.0,
                    },
                ),
            2 => nearest_cat.map(|cp| WildlifeAiState::Haunting {
                target_x: cp.x(),
                target_y: cp.y(),
                edge_distance: haunt_edge,
                ticks: 0,
            }),
            3 => nearest_frontier.map(|fp| WildlifeAiState::Seeding {
                frontier_x: fp.x(),
                frontier_y: fp.y(),
            }),
            // 310 S1 — hunger elects a goal-directed hunt: Stalking
            // toward the nearest scanned cat. `predator_stalk_cats`
            // then drives the approach/ambush exactly as it does for
            // stalks its own 5%/tick roll initiates, and the guard at
            // the top of this loop leaves the hunt uninterrupted.
            4 => {
                // 310 S3 — the kill-site memory shaped this election.
                if kill_site_excluded_nearer {
                    activation.record(Feature::ShadowFoxKillSiteAvoided);
                }
                nearest_cat_any.map(|cp| WildlifeAiState::Stalking {
                    target_x: cp.x(),
                    target_y: cp.y(),
                })
            }
            _ => None,
        };

        // Only transition + emit when the chosen variant *kind* differs
        // from the current one. Re-entering the same kind is the no-op
        // case (continued pacing on the same drive).
        if let Some(new_state) = next_state {
            let changed = !same_motivation_kind(&state, &new_state);
            if changed {
                let feature = match new_state {
                    WildlifeAiState::Reconstituting { .. } => {
                        Some(Feature::ShadowFoxReconstitutingEntered)
                    }
                    WildlifeAiState::Tending { .. } => Some(Feature::ShadowFoxTendingEntered),
                    WildlifeAiState::Haunting { .. } => Some(Feature::ShadowFoxHauntingEntered),
                    WildlifeAiState::Seeding { .. } => Some(Feature::ShadowFoxSeedingEntered),
                    // 310 S1 — Stalking reaches this match only via the
                    // hunger arm (the guard above skips already-Stalking
                    // shadow-foxes before scoring).
                    WildlifeAiState::Stalking { .. } => Some(Feature::ShadowFoxHungerHuntEntered),
                    _ => None,
                };
                if let Some(f) = feature {
                    activation.record(f);
                }
                if let Some(ref mut elog) = event_log {
                    let kind = match new_state {
                        WildlifeAiState::Reconstituting { .. } => {
                            Some(crate::resources::event_log::EventKind::ShadowFoxReconstitutingEntered {
                                location: (pos.x(), pos.y()),
                                coherence: drives.coherence,
                            })
                        }
                        WildlifeAiState::Tending { ward_x, ward_y, .. } => {
                            Some(crate::resources::event_log::EventKind::ShadowFoxTendingEntered {
                                location: (pos.x(), pos.y()),
                                ward_location: (ward_x, ward_y),
                            })
                        }
                        WildlifeAiState::Haunting { target_x, target_y, .. } => {
                            Some(crate::resources::event_log::EventKind::ShadowFoxHauntingEntered {
                                location: (pos.x(), pos.y()),
                                target: (target_x, target_y),
                            })
                        }
                        WildlifeAiState::Seeding { frontier_x, frontier_y } => {
                            Some(crate::resources::event_log::EventKind::ShadowFoxSeedingEntered {
                                location: (pos.x(), pos.y()),
                                frontier: (frontier_x, frontier_y),
                            })
                        }
                        WildlifeAiState::Stalking { target_x, target_y } => {
                            Some(crate::resources::event_log::EventKind::ShadowFoxHungerHuntEntered {
                                location: (pos.x(), pos.y()),
                                target: (target_x, target_y),
                                satiation: drives.satiation,
                            })
                        }
                        _ => None,
                    };
                    if let Some(k) = kind {
                        elog.push(time.tick, k);
                    }
                }
                *state = new_state;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// shadowfox_haunting_drain — ticket 023 Phase C
// ---------------------------------------------------------------------------

/// Per-tick mood/safety drain for cats within `haunting_drain_radius` of
/// a shadow-fox in `WildlifeAiState::Haunting`. Also runs the haunting-
/// escalation timer: once `ticks` exceeds
/// `shadow_fox_haunting_escalation_ticks`, the haunt is promoted to
/// `WildlifeAiState::Stalking` (the existing pre-023 combat-approach
/// path), giving the cat a chance to flee or seek allies before
/// physical attack commits.
///
/// Phase C's job per the design doc: wire the psychological-pressure
/// drive so a Haunting shadow-fox can actually erode a cat's welfare
/// without combat. Pairs with the Phase C deep Dread targeting in
/// `shadowfox_motivation_tick`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn shadowfox_haunting_drain(
    mut shadowfoxes: Query<
        (&Position, &mut WildlifeAiState, &ShadowFoxDrives),
        Without<crate::components::wildlife::Carcass>,
    >,
    mut cats: Query<
        (&Position, &mut Needs, &mut Mood),
        (
            With<Needs>,
            Without<Dead>,
            Without<PreyAnimal>,
            Without<WildAnimal>,
        ),
    >,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
    mut activation: ResMut<SystemActivation>,
) {
    let c = &constants.wildlife;
    let drain_radius = c.shadow_fox_haunting_drain_radius;
    let mood_drain = c.shadow_fox_haunting_mood_drain;
    let safety_drain = c.shadow_fox_haunting_safety_drain;
    let feature_cadence = c.shadow_fox_haunting_feature_cadence.max(1);
    let escalation_ticks = c.shadow_fox_haunting_escalation_ticks;
    let haunt_edge = c.shadow_fox_haunting_edge_distance;

    for (fox_pos, mut state, drives) in &mut shadowfoxes {
        // Only operate on shadow-foxes currently in Haunting.
        let (target_x, target_y, current_ticks) = match *state {
            WildlifeAiState::Haunting {
                target_x,
                target_y,
                ticks,
                ..
            } => (target_x, target_y, ticks),
            _ => continue,
        };

        // Escalation check: enough cumulative haunting → promote to
        // Stalking. The existing pre-023 `predator_stalk_cats` system
        // will then handle the cat-vs-fox combat resolution.
        //
        // 310 S1 — satiation gates every physical-predation entry, and
        // this is the third one (legacy roll, hunger election,
        // escalation). Ungated, this path is a positive feedback loop:
        // an ambush tanks the victim's mood/safety, which is exactly
        // what Dread reads, so the motivation tick re-elects Haunting
        // and 30 ticks later the fox attacks again — the second S1
        // gate soak measured ~45-tick same-cat ambush trains (the
        // ambush execution itself never checks `ambush_cooldown`,
        // which only gates fresh stalk rolls). A fed shadow-fox keeps
        // haunting — the drain below still runs, "it watches, and
        // waits" — and the promotion fires once cadence decay brings
        // satiation back under the stalk threshold.
        if current_ticks >= escalation_ticks
            && drives.satiation < c.shadow_fox_stalk_satiation_threshold
        {
            *state = WildlifeAiState::Stalking { target_x, target_y };
            activation.record(Feature::ShadowFoxHauntingEscalated);
            continue;
        }

        // Increment tick counter. Re-borrow the variant via assignment
        // since `*state` is a mutable destructure target.
        if let WildlifeAiState::Haunting { ticks, .. } = &mut *state {
            *ticks = ticks.saturating_add(1);
        }

        // Apply drain to any cat within `drain_radius` of this shadow-
        // fox. The drain is positional, not target-bound: a haunting
        // shadow-fox in detection range of the entire colony will
        // pressure every nearby cat, not just the target. Consistent
        // with the design doc's "the cat cannot shake the feeling of
        // eyes in the dark" framing.
        let mut drained_any = false;
        for (cat_pos, mut needs, mut mood) in cats.iter_mut() {
            if fox_pos.distance_to(cat_pos) > drain_radius {
                continue;
            }
            needs.safety = (needs.safety - safety_drain).max(0.0);
            mood.valence = (mood.valence - mood_drain).max(-1.0);
            drained_any = true;

            // Suppress combat by maintaining the haunt-edge distance
            // contract: when the fox is at edge_distance (or closer
            // than it intends), don't escalate via this code path —
            // the per-tick escalation timer above is the only path to
            // combat from Haunting. Drain happens regardless of
            // edge-distance overlap.
            let _ = haunt_edge; // retained for future Phase D tuning
            let _ = target_x;
            let _ = target_y;
        }

        if drained_any && time.tick.is_multiple_of(feature_cadence) {
            activation.record(Feature::ShadowFoxHaunting);
        }
    }
}

/// True when two `WildlifeAiState` values are the *same Phase-B
/// motivation variant* (ignoring inner payload like ward angle).
/// Used by `shadowfox_motivation_tick` to suppress redundant Feature
/// emissions when re-electing the same drive.
fn same_motivation_kind(a: &WildlifeAiState, b: &WildlifeAiState) -> bool {
    matches!(
        (a, b),
        (
            WildlifeAiState::Reconstituting { .. },
            WildlifeAiState::Reconstituting { .. }
        ) | (
            WildlifeAiState::Tending { .. },
            WildlifeAiState::Tending { .. }
        ) | (
            WildlifeAiState::Haunting { .. },
            WildlifeAiState::Haunting { .. }
        ) | (
            WildlifeAiState::Seeding { .. },
            WildlifeAiState::Seeding { .. }
        )
    )
}

// ---------------------------------------------------------------------------
// predator_stalk_cats — foxes actively hunt nearby cats
// ---------------------------------------------------------------------------

/// Foxes within detection range of cats may switch to Stalking behavior.
/// A stalking fox that reaches an adjacent tile ambushes the nearest cat,
/// dealing damage and draining safety.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn predator_stalk_cats(
    mut wildlife: Query<
        (
            Entity,
            &mut WildAnimal,
            &Position,
            &mut WildlifeAiState,
            &mut Health,
            // 310 S1 — direct access replaces the `With` filter: the
            // stalk roll reads satiation (fed predators don't hunt)
            // and a landed ambush writes the satiation gain.
            &mut ShadowFoxDrives,
            // 310 S3 — spatial memory: den read for the retreat,
            // kill-site written on ambush + read by the stalk-target
            // filter. Option so pre-S3 saves keep hunting unfiltered.
            Option<&mut crate::components::wildlife::ShadowFoxBeliefs>,
        ),
        (Without<Dead>, Without<crate::components::wildlife::Carcass>),
    >,
    mut cats: Query<
        (
            Entity,
            &Position,
            &mut Health,
            &mut Needs,
            &mut Mood,
            &Name,
            &mut crate::components::CatBodyModel,
            &crate::components::equipment::WearableSlots,
        ),
        (Without<WildAnimal>, Without<Dead>),
    >,
    wards: Query<(&Ward, &Position), Without<WildAnimal>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    mut log: ResMut<NarrativeLog>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
    time: Res<TimeState>,
    mut activation: ResMut<SystemActivation>,
    mut body_part_writer: MessageWriter<crate::messages::body_part_injury::BodyPartInjury>,
    // 294: per-cat substrate replacement for the retired colony-shared
    // `RecentAmbushMap`. Emits `WitnessableEvent::PredatorAmbush`; each
    // cat within `WITNESS_RANGE` (10 Manhattan) lifts their
    // `LocationBeliefs[bucket(pos)].recency_of_threat_cue` in
    // `belief_integrator::apply_observation`.
    mut witnessable_writer: MessageWriter<crate::messages::witnessable_event::WitnessableEvent>,
    // 477 — focal-cat resolver-trace sink for ambush armor reduction.
    focal_trace: crate::resources::trace_log::FocalTraceParam,
) {
    let focal_sink = focal_trace.sink(time.tick);
    let c = &constants.wildlife;
    let ward_multiplier = constants.magic.shadow_fox_ward_repel_multiplier;

    // Snapshot ward positions (non-inverted, alive).
    let ward_positions: Vec<(Position, f32)> = wards
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius() * ward_multiplier))
        .collect();

    // Snapshot cat positions for stalking target selection.
    let cat_positions: Vec<(Entity, Position)> = cats
        .iter()
        .map(|(e, p, _, _, _, _, _, _)| (e, *p))
        .collect();

    for (predator_entity, mut animal, wl_pos, mut ai_state, _health, mut drives, mut beliefs) in
        &mut wildlife
    {
        // `&mut ShadowFoxDrives` access gates this loop to shadow
        // foxes only (regular foxes use fox_ai_decision; hawks/snakes
        // don't carry the drives substrate). Ticket 023 Phase A;
        // 310 S1 lifted the `With` filter to component access.

        // Tick down ambush cooldown.
        if animal.ambush_cooldown > 0 {
            animal.ambush_cooldown -= 1;
        }

        // 310 S3 — the kill-site consideration gates every target
        // selection in this system (selection layer, not movement
        // layer): the legacy roll's pool AND the active-stalk
        // retarget. Cats near this fox's fresh kill site are
        // ineligible.
        let kill_site_filter = beliefs.as_ref().and_then(|b| {
            if b.kill_site_fresh(time.tick, c.shadow_fox_kill_site_memory_ticks) {
                b.last_kill_site.map(|(kx, ky)| Position::new(kx, ky))
            } else {
                None
            }
        });

        // --- Ward avoidance: shadow foxes absolutely avoid wards ---
        let in_ward = ward_positions
            .iter()
            .any(|(wp, radius)| (wl_pos.distance_to(wp)) <= *radius);
        if in_ward {
            // Flee away from nearest ward.
            if let Some((ward_pos, _)) = ward_positions
                .iter()
                .min_by_key(|(wp, _)| wl_pos.tile_distance_squared(wp))
            {
                let away_dx = (wl_pos.x() - ward_pos.x()).signum();
                let away_dy = (wl_pos.y() - ward_pos.y()).signum();
                let dx = if away_dx != 0 { away_dx } else { 1 };
                let dy = if away_dy != 0 { away_dy } else { 0 };
                *ai_state = WildlifeAiState::Patrolling { dx, dy };
                activation.record(Feature::ShadowFoxAvoidedWard);
                continue;
            }
        }

        match *ai_state {
            WildlifeAiState::Patrolling { .. } | WildlifeAiState::Circling { .. } => {
                // Don't initiate new stalks during post-ambush cooldown.
                if animal.ambush_cooldown > 0 {
                    continue;
                }

                // 310 S1 — a fed shadow-fox doesn't hunt: skip the
                // legacy 5%/tick stalk roll while satiation holds at or
                // above the threshold. Cadence decay re-opens
                // eligibility; the hunger drive can also elect Stalking
                // directly through the motivation softmax.
                if drives.satiation >= c.shadow_fox_stalk_satiation_threshold {
                    continue;
                }

                // Find nearest cat within detection range, not inside a
                // ward. Phase 5a: shadow-fox sight channel with LoS.
                let visible = |cp: &Position| {
                    crate::systems::sensing::observer_sees_at_with_los(
                        crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
                        *wl_pos,
                        &constants.sensory.shadow_fox,
                        *cp,
                        crate::components::SensorySignature::CAT,
                        c.base_detection_range,
                        &map,
                    )
                };
                let unwarded = |cp: &Position| {
                    !ward_positions
                        .iter()
                        .any(|(wp, radius)| (cp.distance_to(wp)) <= *radius)
                };
                let nearest = cat_positions
                    .iter()
                    .filter(|(_, cp)| visible(cp))
                    .filter(|(_, cp)| unwarded(cp))
                    .filter(|(_, cp)| {
                        kill_site_filter
                            .map(|ks| cp.distance_to(&ks) > c.shadow_fox_kill_site_avoid_radius)
                            .unwrap_or(true)
                    })
                    .min_by_key(|(_, cp)| wl_pos.tile_distance_squared(cp));
                // Name the consideration when memory (not geometry)
                // emptied or reshaped the pool: some visible, unwarded
                // cat was excluded that is nearer than the survivor.
                if let Some(ks) = kill_site_filter {
                    let nearest_excluded = cat_positions
                        .iter()
                        .filter(|(_, cp)| visible(cp))
                        .filter(|(_, cp)| unwarded(cp))
                        .filter(|(_, cp)| {
                            cp.distance_to(&ks) <= c.shadow_fox_kill_site_avoid_radius
                        })
                        .map(|(_, cp)| wl_pos.tile_distance_squared(cp))
                        .min();
                    let survivor = nearest.map(|(_, cp)| wl_pos.tile_distance_squared(cp));
                    if nearest_excluded.is_some()
                        && (survivor.is_none() || nearest_excluded < survivor)
                    {
                        activation.record(Feature::ShadowFoxKillSiteAvoided);
                    }
                }

                if let Some((_, cat_pos)) = nearest {
                    // 5% chance per tick to begin stalking.
                    if rng.rng.random::<f32>() < 0.05 {
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x(),
                            target_y: cat_pos.y(),
                        };
                    }
                }
            }
            WildlifeAiState::Stalking { target_x, target_y } => {
                // Cancel stalk if target is inside a ward's radius.
                let target_pos = Position::new(target_x, target_y);
                let target_warded = ward_positions
                    .iter()
                    .any(|(wp, radius)| (target_pos.distance_to(wp)) <= *radius);
                if target_warded {
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                    activation.record(Feature::ShadowFoxAvoidedWard);
                    continue;
                }

                let dist = (wl_pos.x() - target_x).abs() + (wl_pos.y() - target_y).abs();

                if dist <= 1 {
                    // Ambush! Find the nearest cat at the target position.
                    let target_pos = Position::new(target_x, target_y);
                    if let Some((cat_entity, cat_pos)) = cat_positions
                        .iter()
                        .filter(|(_, cp)| cp.chebyshev_distance(&target_pos) <= 1)
                        .min_by_key(|(_, cp)| wl_pos.tile_distance_squared(cp))
                    {
                        let cat_pos = *cat_pos;
                        if let Ok((
                            _,
                            _,
                            mut cat_health,
                            mut needs,
                            mut mood,
                            name,
                            mut cat_body_model,
                            wearables,
                        )) = cats.get_mut(*cat_entity)
                        {
                            let tile_corruption = if map.in_bounds(wl_pos.x(), wl_pos.y()) {
                                map.get(wl_pos.x(), wl_pos.y()).corruption
                            } else {
                                0.0
                            };
                            let damage = animal.threat_power
                                * (1.0 + tile_corruption * c.corruption_threat_multiplier);
                            // 477 — armor reduces ambush damage. Reduce once
                            // for the health scalar; `damage_to_body_part`
                            // reduces internally for the body model + trace.
                            let em = crate::components::equipment_effects::equipment_modifiers_for(
                                wearables,
                                &constants.combat,
                            );
                            let reduced = crate::systems::combat::armor_reduced_damage(
                                damage,
                                crate::components::physical::InjurySource::ShadowFoxAmbush,
                                crate::components::body_zones::WoundKind::Normal,
                                &em,
                            );
                            cat_health.current = (cat_health.current - reduced).max(0.0);
                            // 095 Phase 1 — anatomical substrate is canonical.
                            // Legacy `Injury` record retired.
                            let _ = cat_pos; // injury_pos no longer needed
                            crate::systems::combat::damage_to_body_part(
                                *cat_entity,
                                &mut cat_body_model,
                                damage,
                                time.tick,
                                crate::components::physical::InjurySource::ShadowFoxAmbush,
                                &constants.combat,
                                &mut rng,
                                &mut body_part_writer,
                                &mut activation,
                                Some(&em),
                                focal_sink.as_ref(),
                            );
                            needs.safety = (needs.safety - c.threat_safety_drain).max(0.0);

                            let species_name = match animal.species {
                                WildSpecies::Fox => "fox",
                                WildSpecies::ShadowFox => "shadow-fox",
                                _ => "predator",
                            };
                            log.push(
                                time.tick,
                                format!(
                                    "A {species_name} lunges at {} from the undergrowth!",
                                    name.0
                                ),
                                NarrativeTier::Danger,
                            );
                            if let Some(ref mut elog) = event_log {
                                elog.push(
                                    time.tick,
                                    crate::resources::event_log::EventKind::Ambush {
                                        cat: name.0.clone(),
                                        predator_species: format!("{:?}", animal.species),
                                        location: (wl_pos.x(), wl_pos.y()),
                                        damage,
                                    },
                                );
                            }

                            // 294: emit the per-cat substrate event. Each
                            // cat within `WITNESS_RANGE` (10 Manhattan) of
                            // `*wl_pos` lifts their
                            // `LocationBeliefs[bucket(*wl_pos)].recency_of_threat_cue`
                            // in `belief_integrator::apply_observation`.
                            witnessable_writer.write(
                                crate::messages::witnessable_event::WitnessableEvent::PredatorAmbush {
                                    predator: predator_entity,
                                    victim: *cat_entity,
                                    position: *wl_pos,
                                    tick: time.tick,
                                },
                            );

                            mood.modifiers.push_back(
                                MoodModifier::new(
                                    c.threat_mood_penalty,
                                    c.threat_mood_ticks,
                                    "ambushed by predator",
                                )
                                .with_kind(MoodSource::Fear),
                            );
                        }

                        // Nearby cats witness the ambush — drain their safety.
                        for (witness_entity, witness_pos) in &cat_positions {
                            if *witness_entity == *cat_entity {
                                continue;
                            }
                            if wl_pos.distance_to(witness_pos) <= c.ambush_witness_range {
                                if let Ok((_, _, _, mut w_needs, mut w_mood, _, _, _)) =
                                    cats.get_mut(*witness_entity)
                                {
                                    w_needs.safety =
                                        (w_needs.safety - c.ambush_witness_safety_drain).max(0.0);
                                    w_mood.modifiers.push_back(
                                        MoodModifier::new(
                                            c.threat_mood_penalty * 0.5,
                                            c.threat_mood_ticks,
                                            "witnessed predator attack",
                                        )
                                        .with_kind(MoodSource::Fear),
                                    );
                                }
                            }
                        }
                    }
                    // After ambush, set the cooldown before the next stalk.
                    animal.ambush_cooldown = c.ambush_cooldown_ticks;
                    // 310 S1 — the kill feeds it: satiation rises past
                    // the stalk-suppression threshold, so the next hunt
                    // waits on cadence decay, not just the cooldown.
                    drives.satiation =
                        (drives.satiation + c.shadow_fox_satiation_gain_ambush).min(1.0);
                    // 310 S3 — remember the fished-out pond: the kill
                    // site is this fox's memory, read by both stalk
                    // target filters until it expires.
                    if let Some(b) = beliefs.as_mut() {
                        b.last_kill_site = Some((wl_pos.x(), wl_pos.y()));
                        b.last_kill_tick = time.tick;
                    }
                    // 310 S2 — a fed fox carries its kill home: retreat
                    // to the den instead of the legacy resume-patrol.
                    // SingleMinded — the motivation-tick guard holds it
                    // until `wildlife_ai` releases it on arrival. Den
                    // unknown (pre-S2 saves, scenario spawns) falls back
                    // to the legacy Patrolling reset.
                    *ai_state = match beliefs.as_ref().and_then(|b| b.den_position) {
                        Some((den_x, den_y)) => {
                            activation.record(Feature::ShadowFoxRetreatEntered);
                            if let Some(ref mut elog) = event_log {
                                elog.push(
                                    time.tick,
                                    crate::resources::event_log::EventKind::ShadowFoxRetreatEntered {
                                        location: (wl_pos.x(), wl_pos.y()),
                                        den: (den_x, den_y),
                                    },
                                );
                            }
                            WildlifeAiState::Retreating { den_x, den_y }
                        }
                        None => WildlifeAiState::Patrolling { dx: 1, dy: 0 },
                    };
                } else if (dist as f32) > c.base_detection_range * 2.0 {
                    // Target moved too far, give up.
                    *ai_state = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
                } else {
                    // Update target to nearest cat's current position.
                    // 310 S3 — the retarget is a selection too: without
                    // the kill-site filter here, a hunger election
                    // toward clean ground snapped back to the fished
                    // cluster one tick later (caught by the
                    // kill-site-avoidance scenario). Empty filtered
                    // pool → hold the committed target.
                    if let Some((_, cat_pos)) = cat_positions
                        .iter()
                        .filter(|(_, cp)| {
                            kill_site_filter
                                .map(|ks| cp.distance_to(&ks) > c.shadow_fox_kill_site_avoid_radius)
                                .unwrap_or(true)
                        })
                        .min_by_key(|(_, cp)| wl_pos.tile_distance_squared(cp))
                    {
                        *ai_state = WildlifeAiState::Stalking {
                            target_x: cat_pos.x(),
                            target_y: cat_pos.y(),
                        };
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        let mut map = TileMap::new(40, 30, Terrain::Grass);
        // Add some forest for foxes.
        for x in 0..10 {
            map.set(x, 0, Terrain::DenseForest);
            map.set(x, 29, Terrain::DenseForest);
        }
        // Add rock for snakes.
        for x in 30..40 {
            map.set(x, 0, Terrain::Rock);
            map.set(x, 29, Terrain::Rock);
        }
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        // 260: wildlife_ai now reads WardCoverageMap and CatScentMap
        // through the InfluenceMap registry; insert defaults so the
        // unit tests can exercise the no-ward / no-scent baseline.
        world.insert_resource(crate::resources::WardCoverageMap::default());
        world.insert_resource(crate::resources::ColonyDistrictMap::default());
        world.insert_resource(CatScentMap::default());

        let mut schedule = Schedule::default();
        // 140 step 11 — wildlife_ai writes desire; the integrator
        // moves. Chain them like the production schedule.
        schedule.add_systems((wildlife_ai, crate::systems::movement::integrate_velocities).chain());
        (world, schedule)
    }

    fn spawn_animal(
        world: &mut World,
        species: WildSpecies,
        pos: Position,
        ai_state: WildlifeAiState,
    ) -> Entity {
        // Ticket 138 — wildlife_ai now requires MovementBudget on
        // its query. In production an `OnAdd<WildAnimal>` observer
        // inserts the species default; tests bypass the observer
        // (they build a bare Schedule, not a full App) so we
        // inject the budget manually here.
        world
            .spawn((
                WildAnimal::new(species),
                pos,
                Health::default(),
                ai_state,
                crate::components::MovementBudget::for_species(
                    species,
                    &crate::resources::sim_constants::MovementConstants::default(),
                ),
                // 140 step 11 — the fluid-movement pair the observer
                // authors in production.
                crate::components::physical::Velocity::default(),
                crate::components::physical::DesiredVelocity::default(),
            ))
            .id()
    }

    #[test]
    fn fox_patrols_along_forest() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::Fox,
            Position::new(5, 15),
            WildlifeAiState::Patrolling { dx: 1, dy: 0 },
        );

        // 140 step 11 — desire-driven movement ramps at max_accel per
        // tick; give the accel ramp a few ticks to cross a tile edge.
        for _ in 0..4 {
            schedule.run(&mut world);
        }

        let pos = *world.get::<Position>(entity).unwrap();
        // Fox should have moved (either forward or jittered).
        assert!(
            pos.x() != 5 || pos.y() != 15,
            "fox should have moved from (5, 15)"
        );
    }

    #[test]
    fn snake_stays_still() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::Snake,
            Position::new(10, 10),
            WildlifeAiState::Waiting,
        );

        schedule.run(&mut world);

        let pos = *world.get::<Position>(entity).unwrap();
        assert_eq!(pos, Position::new(10, 10), "snake should not move");
    }

    #[test]
    fn hawk_circles_and_moves() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::Hawk,
            Position::new(20, 15),
            WildlifeAiState::Circling {
                center_x: 20,
                center_y: 15,
                angle: 0.0,
            },
        );

        // 140 step 11 — accel-ramp runway (see fox test).
        for _ in 0..4 {
            schedule.run(&mut world);
        }

        let pos = *world.get::<Position>(entity).unwrap();
        // Hawk should have moved from start (circling).
        assert!(
            pos.x() != 20 || pos.y() != 15,
            "hawk should have moved from (20, 15)"
        );
    }

    // ---- Ticket 023 Phase A: shadowfox_coherence_tick ----

    fn setup_coherence_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(TimeState::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(shadowfox_coherence_tick);
        (world, schedule)
    }

    fn spawn_shadowfox_at(world: &mut World, pos: Position, coherence: f32) -> Entity {
        world
            .spawn((
                WildAnimal::new(WildSpecies::ShadowFox),
                pos,
                Health::default(),
                WildlifeAiState::Patrolling { dx: 1, dy: 0 },
                ShadowFoxDrives {
                    coherence,
                    resonance: 0.0,
                    dread: 0.0,
                    entropy: 0.0,
                    age_ticks: 0,
                    origin_corruption: 0.9,
                    satiation: crate::resources::SimConstants::default()
                        .wildlife
                        .shadow_fox_satiation_at_spawn,
                },
            ))
            .id()
    }

    #[test]
    fn shadowfox_dissolves_on_clean_ground() {
        let (mut world, mut schedule) = setup_coherence_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        // Tile is plain Grass with corruption 0.0 — below the recovery
        // threshold (0.3), so coherence decays at `decay_clean` (0.002).
        // Dissolution at coherence <= 0.0 → ~500 ticks.

        let decay = world
            .resource::<crate::resources::SimConstants>()
            .wildlife
            .shadow_fox_coherence_decay_clean;
        let expected_dissolve_at = (1.0_f32 / decay).ceil() as u32;

        let mut ticks_until_despawn = 0_u32;
        for tick in 1..=(expected_dissolve_at + 5) {
            schedule.run(&mut world);
            if world.get::<ShadowFoxDrives>(entity).is_none() {
                ticks_until_despawn = tick;
                break;
            }
        }
        assert!(
            ticks_until_despawn > 0,
            "shadowfox should have dissolved within {} ticks",
            expected_dissolve_at + 5,
        );
        assert!(
            ticks_until_despawn.abs_diff(expected_dissolve_at) <= 1,
            "expected dissolution within ±1 of {} ticks, got {}",
            expected_dissolve_at,
            ticks_until_despawn,
        );
        // Feature recorded for the canary footer.
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::ShadowFoxDissolved)
                .copied()
                .unwrap_or(0)
                >= 1,
            "ShadowFoxDissolved feature should have been recorded on dissolution",
        );
    }

    #[test]
    fn shadowfox_recovers_on_corrupted_ground() {
        let (mut world, mut schedule) = setup_coherence_world();
        // Saturate the spawn tile with corruption so it sits well above
        // the recovery threshold (0.3).
        {
            let mut map = world.resource_mut::<TileMap>();
            map.get_mut(5, 5).corruption = 0.9;
        }
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 0.5);

        for _ in 0..10 {
            schedule.run(&mut world);
        }

        let drives = world.get::<ShadowFoxDrives>(entity).expect(
            "shadowfox should not have dissolved while sitting on heavily-corrupted ground",
        );
        assert!(
            drives.coherence > 0.5,
            "coherence should have recovered above 0.5; got {}",
            drives.coherence,
        );
        assert!(
            drives.coherence <= 1.0,
            "coherence must clamp at 1.0; got {}",
            drives.coherence,
        );
        assert!(drives.age_ticks >= 10, "age_ticks should have advanced");
    }

    // ---- Ticket 310 S1: satiation drive ----

    fn setup_motivation_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(40, 40, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        // TimeState::default() tick is a multiple of every cadence, so
        // the motivation tick runs on every schedule.run.
        world.insert_resource(TimeState::default());
        world.insert_resource(crate::resources::WardCoverageMap::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(shadowfox_motivation_tick);
        (world, schedule)
    }

    /// A cat as the motivation tick's cats query sees one: `Needs` for
    /// the `With` filter, `Mood` for the Dread read.
    fn spawn_plain_cat_at(world: &mut World, pos: Position) {
        world.spawn((pos, Mood::default(), Needs::default()));
    }

    #[test]
    fn satiation_decays_on_motivation_cadence() {
        let (mut world, mut schedule) = setup_motivation_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.5;

        schedule.run(&mut world);

        let decay = crate::resources::SimConstants::default()
            .wildlife
            .shadow_fox_satiation_decay_per_cadence;
        let satiation = world.get::<ShadowFoxDrives>(entity).unwrap().satiation;
        assert!(
            (satiation - (0.5 - decay)).abs() < 1e-6,
            "satiation should have decayed by exactly one cadence step; got {satiation}",
        );
    }

    #[test]
    fn hunger_drive_elects_stalking_when_starving() {
        let (mut world, mut schedule) = setup_motivation_world();
        // Deterministic election: no jitter, near-argmax temperature.
        {
            let mut constants = world.resource_mut::<crate::resources::SimConstants>();
            constants.wildlife.shadow_fox_motivation_jitter = 0.0;
            constants.wildlife.shadow_fox_motivation_softmax_temp = 0.001;
        }
        // Full coherence + clean grass map + defaulted cat mood/safety
        // → Coherence/Resonance/Dread/Entropy pressures are all 0.0;
        // hunger `(1 − 0)² × 0.10 = 0.10` is the only live drive and
        // sits above the 0.05 pressure floor.
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        spawn_plain_cat_at(&mut world, Position::new(8, 5));

        schedule.run(&mut world);

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(
                state,
                WildlifeAiState::Stalking {
                    target_x: 8,
                    target_y: 5
                }
            ),
            "starving shadow-fox should elect Stalking toward the nearest cat; got {state:?}",
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::ShadowFoxHungerHuntEntered)
                .copied()
                .unwrap_or(0)
                >= 1,
            "hunger-elected Stalking should record ShadowFoxHungerHuntEntered",
        );
    }

    #[test]
    fn hunger_axis_absent_when_zeroed() {
        let (mut world, mut schedule) = setup_motivation_world();
        // Conditional-axis escape hatch: weight 0.0 restores the
        // four-drive softmax — with every other pressure at 0.0 the
        // pressure floor declines to transition at all.
        {
            let mut constants = world.resource_mut::<crate::resources::SimConstants>();
            constants.wildlife.shadow_fox_motivation_jitter = 0.0;
            constants.wildlife.shadow_fox_hunger_drive_weight = 0.0;
        }
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        spawn_plain_cat_at(&mut world, Position::new(8, 5));

        schedule.run(&mut world);

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(state, WildlifeAiState::Patrolling { .. }),
            "zeroed hunger weight must leave the starving shadow-fox patrolling; got {state:?}",
        );
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::ShadowFoxHungerHuntEntered)
                .copied()
                .unwrap_or(0),
            0,
            "no hunger-hunt Feature may fire with the axis zeroed",
        );
    }

    #[test]
    fn hunger_below_floor_never_elected_under_softmax_spread() {
        // First S1 gate-soak regression: with another drive holding the
        // pressure floor open and a *wide* softmax temperature, the
        // near-zero hunger candidate must not be electable at all —
        // the gate is eligibility, not score.
        let (mut world, mut schedule) = setup_motivation_world();
        {
            let mut constants = world.resource_mut::<crate::resources::SimConstants>();
            constants.wildlife.shadow_fox_motivation_jitter = 0.0;
            // Wide temperature: without the eligibility gate a 5th
            // candidate at ~0 pressure wins ≈ 1/5 of elections.
            constants.wildlife.shadow_fox_motivation_softmax_temp = 10.0;
        }
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        // Nearly sated: hunger pressure (1 − 0.98)² × 0.10 ≈ 4e-6,
        // far below the 0.05 floor.
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.98;
        // A dread-pressured cat opens the floor: negative mood + full
        // safety deficit, isolated (no allies).
        world.spawn((
            Position::new(8, 5),
            Mood {
                valence: -1.0,
                ..Default::default()
            },
            Needs::default(),
            crate::components::prev_safety_deficit::PrevSafetyDeficit(1.0),
        ));

        for round in 0..100 {
            schedule.run(&mut world);
            let state = world.get::<WildlifeAiState>(entity).unwrap().clone();
            assert!(
                !matches!(state, WildlifeAiState::Stalking { .. }),
                "sub-floor hunger candidate elected Stalking on round {round}",
            );
            // Re-arm for the next election (Haunting is the expected
            // winner; the guard skips Stalking/EncirclingWard only).
            *world.get_mut::<WildlifeAiState>(entity).unwrap() =
                WildlifeAiState::Patrolling { dx: 1, dy: 0 };
            world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.98;
        }
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::ShadowFoxHungerHuntEntered)
                .copied()
                .unwrap_or(0),
            0,
            "no hunger-hunt election may fire below the pressure floor",
        );
    }

    fn setup_haunting_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(TimeState::default());
        world.insert_resource(SystemActivation::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(shadowfox_haunting_drain);
        (world, schedule)
    }

    /// Second-gate-soak regression: the Haunting → Stalking escalation
    /// is the third physical-predation entry and must respect the same
    /// satiation gate as the stalk roll and the hunger election —
    /// ungated it forms the ambush → dread → haunt → escalate → ambush
    /// feedback loop (~45-tick same-cat ambush trains).
    #[test]
    fn fed_haunting_fox_does_not_escalate() {
        let (mut world, mut schedule) = setup_haunting_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 1.0;
        *world.get_mut::<WildlifeAiState>(entity).unwrap() = WildlifeAiState::Haunting {
            target_x: 8,
            target_y: 5,
            edge_distance: 5.0,
            ticks: 10_000, // far past escalation_ticks (30)
        };

        for _ in 0..50 {
            schedule.run(&mut world);
        }

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(state, WildlifeAiState::Haunting { .. }),
            "fed shadow-fox must keep haunting, not escalate to Stalking; got {state:?}",
        );
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::ShadowFoxHauntingEscalated)
                .copied()
                .unwrap_or(0),
            0,
        );
    }

    #[test]
    fn hungry_haunting_fox_escalates() {
        let (mut world, mut schedule) = setup_haunting_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        *world.get_mut::<WildlifeAiState>(entity).unwrap() = WildlifeAiState::Haunting {
            target_x: 8,
            target_y: 5,
            edge_distance: 5.0,
            ticks: 10_000,
        };

        schedule.run(&mut world);

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(
                state,
                WildlifeAiState::Stalking {
                    target_x: 8,
                    target_y: 5
                }
            ),
            "hungry shadow-fox past the escalation threshold must promote to Stalking; got {state:?}",
        );
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::ShadowFoxHauntingEscalated)
                .copied()
                .unwrap_or(0),
            1,
        );
    }

    fn setup_stalk_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(40, 40, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(TimeState::default());
        world.init_resource::<bevy_ecs::message::Messages<
            crate::messages::body_part_injury::BodyPartInjury,
        >>();
        world.init_resource::<bevy_ecs::message::Messages<
            crate::messages::witnessable_event::WitnessableEvent,
        >>();

        let mut schedule = Schedule::default();
        schedule.add_systems(predator_stalk_cats);
        (world, schedule)
    }

    /// A cat as `predator_stalk_cats`' cats query sees one (full
    /// ambush-victim component set).
    fn spawn_ambushable_cat_at(world: &mut World, pos: Position) {
        world.spawn((
            pos,
            Health::default(),
            Needs::default(),
            Mood::default(),
            Name("Testcat".to_string()),
            crate::components::CatBodyModel::default(),
            crate::components::equipment::WearableSlots::default(),
        ));
    }

    #[test]
    fn fed_shadowfox_skips_stalk_roll() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 1.0;
        spawn_ambushable_cat_at(&mut world, Position::new(8, 5));

        // No motivation tick in this schedule, so satiation never
        // decays: the 5%/tick roll must stay suppressed for the full
        // window (un-suppressed, P(no stalk in 300 ticks) ≈ 2e-7).
        for _ in 0..300 {
            schedule.run(&mut world);
            let state = world.get::<WildlifeAiState>(entity).unwrap();
            assert!(
                !matches!(state, WildlifeAiState::Stalking { .. }),
                "fed shadow-fox (satiation 1.0) must never enter Stalking via the legacy roll",
            );
        }
    }

    #[test]
    fn hungry_shadowfox_still_stalks() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        spawn_ambushable_cat_at(&mut world, Position::new(8, 5));

        let mut stalked = false;
        for _ in 0..300 {
            schedule.run(&mut world);
            if matches!(
                world.get::<WildlifeAiState>(entity).unwrap(),
                WildlifeAiState::Stalking { .. }
            ) {
                stalked = true;
                break;
            }
        }
        assert!(
            stalked,
            "hungry shadow-fox must keep the legacy 5%/tick stalk roll",
        );
    }

    // ---- Ticket 310 S2: den + post-ambush retreat ----

    #[test]
    fn ambush_triggers_retreat_to_den() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.1;
        world
            .entity_mut(entity)
            .insert(crate::components::wildlife::ShadowFoxBeliefs {
                den_position: Some((2, 2)),
                last_kill_site: None,
                last_kill_tick: 0,
            });
        spawn_ambushable_cat_at(&mut world, Position::new(6, 5));
        *world.get_mut::<WildlifeAiState>(entity).unwrap() = WildlifeAiState::Stalking {
            target_x: 6,
            target_y: 5,
        };

        schedule.run(&mut world);

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(state, WildlifeAiState::Retreating { den_x: 2, den_y: 2 }),
            "post-ambush state must be Retreating toward the den; got {state:?}",
        );
        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::ShadowFoxRetreatEntered)
                .copied()
                .unwrap_or(0),
            1,
        );
    }

    #[test]
    fn ambush_without_den_falls_back_to_patrol() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.1;
        // den_position stays None (pre-S2 saves / bare scenario spawns).
        spawn_ambushable_cat_at(&mut world, Position::new(6, 5));
        *world.get_mut::<WildlifeAiState>(entity).unwrap() = WildlifeAiState::Stalking {
            target_x: 6,
            target_y: 5,
        };

        schedule.run(&mut world);

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(state, WildlifeAiState::Patrolling { .. }),
            "denless ambush must keep the legacy Patrolling reset; got {state:?}",
        );
    }

    #[test]
    fn retreating_fox_arrives_and_releases_to_patrol() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_animal(
            &mut world,
            WildSpecies::ShadowFox,
            Position::new(5, 15),
            WildlifeAiState::Retreating {
                den_x: 12,
                den_y: 15,
            },
        );

        let mut released_at = None;
        for tick in 0..120 {
            schedule.run(&mut world);
            if matches!(
                world.get::<WildlifeAiState>(entity).unwrap(),
                WildlifeAiState::Patrolling { .. }
            ) {
                released_at = Some(tick);
                break;
            }
        }
        let released_at =
            released_at.expect("retreating fox should reach the den and release to Patrolling");
        let pos = *world.get::<Position>(entity).unwrap();
        let arrival = crate::resources::SimConstants::default()
            .wildlife
            .shadow_fox_retreat_arrival_radius;
        assert!(
            pos.distance_to(&Position::new(12, 15)) <= arrival + 1.0,
            "release must happen at the den (got {:?} after tick {released_at})",
            pos,
        );
    }

    #[test]
    fn motivation_tick_holds_retreating_singleminded() {
        let (mut world, mut schedule) = setup_motivation_world();
        {
            let mut constants = world.resource_mut::<crate::resources::SimConstants>();
            constants.wildlife.shadow_fox_motivation_jitter = 0.0;
            constants.wildlife.shadow_fox_motivation_softmax_temp = 0.001;
        }
        // Starving fox with a cat in scan range: the hunger drive WOULD
        // elect Stalking — but Retreating is SingleMinded.
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        spawn_plain_cat_at(&mut world, Position::new(8, 5));
        *world.get_mut::<WildlifeAiState>(entity).unwrap() =
            WildlifeAiState::Retreating { den_x: 2, den_y: 2 };

        for _ in 0..10 {
            schedule.run(&mut world);
        }

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(state, WildlifeAiState::Retreating { .. }),
            "motivation tick must not interrupt a retreat; got {state:?}",
        );
    }

    // ---- Ticket 310 S3: kill-site memory ----

    fn insert_beliefs(
        world: &mut World,
        entity: Entity,
        den: Option<(i32, i32)>,
        kill_site: Option<(i32, i32)>,
        kill_tick: u64,
    ) {
        world
            .entity_mut(entity)
            .insert(crate::components::wildlife::ShadowFoxBeliefs {
                den_position: den,
                last_kill_site: kill_site,
                last_kill_tick: kill_tick,
            });
    }

    #[test]
    fn ambush_writes_kill_site_memory() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.1;
        insert_beliefs(&mut world, entity, Some((2, 2)), None, 0);
        spawn_ambushable_cat_at(&mut world, Position::new(6, 5));
        *world.get_mut::<WildlifeAiState>(entity).unwrap() = WildlifeAiState::Stalking {
            target_x: 6,
            target_y: 5,
        };

        schedule.run(&mut world);

        let beliefs = world
            .get::<crate::components::wildlife::ShadowFoxBeliefs>(entity)
            .unwrap();
        assert_eq!(
            beliefs.last_kill_site,
            Some((5, 5)),
            "a landed ambush must record the kill site",
        );
    }

    #[test]
    fn legacy_roll_skips_fished_out_cat() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        // Fresh kill memory right on top of the only visible cat.
        insert_beliefs(&mut world, entity, Some((2, 2)), Some((8, 5)), 0);
        spawn_ambushable_cat_at(&mut world, Position::new(8, 5));

        for _ in 0..300 {
            schedule.run(&mut world);
            let state = world.get::<WildlifeAiState>(entity).unwrap();
            assert!(
                !matches!(state, WildlifeAiState::Stalking { .. }),
                "the only cat sits in the fished-out radius — no stalk may start",
            );
        }
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::ShadowFoxKillSiteAvoided)
                .copied()
                .unwrap_or(0)
                >= 1,
            "the exclusion must be named, not silent",
        );
    }

    #[test]
    fn expired_kill_site_memory_frees_the_ground() {
        let (mut world, mut schedule) = setup_stalk_world();
        let memory = crate::resources::SimConstants::default()
            .wildlife
            .shadow_fox_kill_site_memory_ticks;
        // TimeState::default() tick 0; stamp the kill far enough in the
        // "past" via a tick beyond the window: set last_kill_tick = 0
        // and advance TimeState past the memory horizon.
        world.resource_mut::<TimeState>().tick = memory + 1;
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        insert_beliefs(&mut world, entity, Some((2, 2)), Some((8, 5)), 0);
        spawn_ambushable_cat_at(&mut world, Position::new(8, 5));

        let mut stalked = false;
        for _ in 0..300 {
            schedule.run(&mut world);
            if matches!(
                world.get::<WildlifeAiState>(entity).unwrap(),
                WildlifeAiState::Stalking { .. }
            ) {
                stalked = true;
                break;
            }
        }
        assert!(stalked, "expired memory must not gate the hunt");
    }

    #[test]
    fn hunger_election_prefers_ground_outside_kill_site() {
        let (mut world, mut schedule) = setup_motivation_world();
        {
            let mut constants = world.resource_mut::<crate::resources::SimConstants>();
            constants.wildlife.shadow_fox_motivation_jitter = 0.0;
            constants.wildlife.shadow_fox_motivation_softmax_temp = 0.001;
        }
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.0;
        // Nearer cat (8,5) sits in the fished-out radius of the kill
        // site; farther cat (5, 12) is clean ground.
        insert_beliefs(&mut world, entity, Some((2, 2)), Some((8, 5)), 0);
        spawn_plain_cat_at(&mut world, Position::new(8, 5));
        spawn_plain_cat_at(&mut world, Position::new(5, 12));

        schedule.run(&mut world);

        let state = world.get::<WildlifeAiState>(entity).unwrap();
        assert!(
            matches!(
                state,
                WildlifeAiState::Stalking {
                    target_x: 5,
                    target_y: 12
                }
            ),
            "hunger must hunt the clean ground, not the fished-out pond; got {state:?}",
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::ShadowFoxKillSiteAvoided)
                .copied()
                .unwrap_or(0)
                >= 1,
            "the reshaped choice must be named",
        );
    }

    #[test]
    fn ambush_feeds_satiation() {
        let (mut world, mut schedule) = setup_stalk_world();
        let entity = spawn_shadowfox_at(&mut world, Position::new(5, 5), 1.0);
        world.get_mut::<ShadowFoxDrives>(entity).unwrap().satiation = 0.1;
        // Adjacent target — the Stalking arm ambushes immediately.
        spawn_ambushable_cat_at(&mut world, Position::new(6, 5));
        *world.get_mut::<WildlifeAiState>(entity).unwrap() = WildlifeAiState::Stalking {
            target_x: 6,
            target_y: 5,
        };

        schedule.run(&mut world);

        let drives = world.get::<ShadowFoxDrives>(entity).unwrap();
        let expected = (0.1_f32
            + crate::resources::SimConstants::default()
                .wildlife
                .shadow_fox_satiation_gain_ambush)
            .min(1.0);
        assert!(
            (drives.satiation - expected).abs() < 1e-6,
            "ambush should add the satiation gain; got {}",
            drives.satiation,
        );
        let animal = world.get::<WildAnimal>(entity).unwrap();
        assert!(
            animal.ambush_cooldown > 0,
            "ambush must still set the legacy cooldown",
        );
        assert!(
            drives.satiation
                >= crate::resources::SimConstants::default()
                    .wildlife
                    .shadow_fox_stalk_satiation_threshold,
            "post-ambush satiation must sit above the stalk-suppression threshold",
        );
    }
}
