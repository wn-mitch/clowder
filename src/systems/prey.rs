use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::building::{StoredItems, Structure, StructureType};
use crate::components::items::Item;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::{
    DenRaided, FleeStrategy, PreyAiState, PreyAnimal, PreyConfig, PreyDen, PreyDensity, PreyKilled,
    PreyKind, PreyState,
};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeScale, TimeState};
use crate::species::SpeciesRegistry;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Whether a prey animal can move to the given tile (must be valid habitat).
fn can_move_to(x: i32, y: i32, habitat: &[Terrain], map: &TileMap) -> bool {
    map.in_bounds(x, y) && habitat.contains(&map.get(x, y).terrain)
}

/// Prey avoid moving onto heavily corrupted tiles.
fn can_move_to_prey(
    x: i32,
    y: i32,
    habitat: &[Terrain],
    map: &TileMap,
    corruption_threshold: f32,
) -> bool {
    can_move_to(x, y, habitat, map) && map.get(x, y).corruption < corruption_threshold
}

/// Try up to 50 random tiles to find one whose terrain is in `habitat`.
pub fn find_habitat_tile(habitat: &[Terrain], map: &TileMap, rng: &mut SimRng) -> Option<Position> {
    for _ in 0..50 {
        let x = rng.rng.random_range(0..map.width);
        let y = rng.rng.random_range(0..map.height);
        if habitat.contains(&map.get(x, y).terrain) {
            return Some(Position::new(x, y));
        }
    }
    None
}

/// Find a passable tile within `radius` of `center` that matches `habitat`.
fn find_nearby_habitat_tile(
    center: &Position,
    radius: i32,
    habitat: &[Terrain],
    map: &TileMap,
    rng: &mut SimRng,
) -> Option<Position> {
    for _ in 0..30 {
        let dx = rng.rng.random_range(-radius..=radius);
        let dy = rng.rng.random_range(-radius..=radius);
        let x = center.x + dx;
        let y = center.y + dy;
        if map.in_bounds(x, y) && habitat.contains(&map.get(x, y).terrain) {
            return Some(Position::new(x, y));
        }
    }
    None
}

/// Probabilistic detection: each tick, prey rolls to notice nearby cats.
/// Closer cats are more likely to be detected. `vigilance_mod` comes from
/// the Risk Allocation Hypothesis (U-shaped curve on predation pressure).
///
/// Phase 4 migration: the proximity gradient is computed via
/// `sensing::prey_cat_proximity`, routed through the unified `detect()`
/// on the prey's sight channel (Linear falloff). The Bernoulli gate
/// stays here; alertness and vigilance remain prey-state-dependent
/// factors outside the sensory model.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn try_detect_cat(
    pos: &Position,
    prey_kind: PreyKind,
    prey_profile: &crate::systems::sensing::SensoryProfile,
    alert_radius: i32,
    alertness: f32,
    vigilance_mod: f32,
    detection_base_chance: f32,
    alertness_base: f32,
    alertness_range: f32,
    cat_positions: &Query<(Entity, &Position), (With<Needs>, Without<Dead>, Without<PreyAnimal>)>,
    rng: &mut SimRng,
) -> Option<Entity> {
    for (entity, cat_pos) in cat_positions.iter() {
        let proximity = crate::systems::sensing::prey_cat_proximity(
            *pos,
            prey_kind,
            prey_profile,
            *cat_pos,
            alert_radius,
        );
        if proximity <= 0.0 {
            continue;
        }
        let alertness_mod = alertness_base + alertness * alertness_range;
        let detection_chance = detection_base_chance * proximity * alertness_mod * vigilance_mod;
        if rng.rng.random::<f32>() < detection_chance {
            return Some(entity);
        }
    }
    None
}

/// Risk Allocation Hypothesis: U-shaped vigilance curve on predation pressure.
/// Low pressure (safe) → relaxed → low vigilance.
/// Medium pressure → most vigilant.
/// High pressure (constant danger) → must forage despite risk → low vigilance.
fn vigilance_from_pressure(
    pressure: f32,
    center: f32,
    steepness: f32,
    baseline: f32,
    amplitude: f32,
) -> f32 {
    let x = (pressure - center) * steepness;
    baseline + amplitude * (-x * x).exp()
}

/// Bird instant-teleport: jump to a random habitat tile 5-8 tiles from threat.
fn bird_teleport(
    pos: &mut Mut<Position>,
    threat_pos: &Position,
    habitat: &[Terrain],
    map: &TileMap,
    rng: &mut SimRng,
    min_range: i32,
    max_range: i32,
) {
    for _ in 0..20 {
        let range = rng.rng.random_range(min_range..=max_range);
        let angle: f32 = rng.rng.random::<f32>() * std::f32::consts::TAU;
        let nx = threat_pos.x + (angle.cos() * range as f32) as i32;
        let ny = threat_pos.y + (angle.sin() * range as f32) as i32;
        if can_move_to(nx, ny, habitat, map) {
            pos.x = nx;
            pos.y = ny;
            return;
        }
    }
}

/// For cover-seeking species, find a forest tile roughly in the flee direction.
fn find_cover_direction(
    pos: &Position,
    threat_pos: &Position,
    map: &TileMap,
) -> Option<(i32, i32)> {
    let flee_dx = (pos.x - threat_pos.x).signum();
    let flee_dy = (pos.y - threat_pos.y).signum();

    let mut best: Option<(i32, i32, i32)> = None; // (dx, dy, distance)

    // Scan a 5-tile cone in the flee direction.
    for scan_dx in -2..=2i32 {
        for scan_dy in -2..=2i32 {
            let tx = pos.x + flee_dx + scan_dx;
            let ty = pos.y + flee_dy + scan_dy;
            if !map.in_bounds(tx, ty) {
                continue;
            }
            let terrain = map.get(tx, ty).terrain;
            if matches!(terrain, Terrain::LightForest | Terrain::DenseForest) {
                let dist = (tx - pos.x).abs() + (ty - pos.y).abs();
                if best.is_none() || dist < best.unwrap().2 {
                    let dx = (tx - pos.x).signum();
                    let dy = (ty - pos.y).signum();
                    if dx != 0 || dy != 0 {
                        best = Some((dx, dy, dist));
                    }
                }
            }
        }
    }

    best.map(|(dx, dy, _)| (dx, dy))
}

// ---------------------------------------------------------------------------
// prey_ai system
// ---------------------------------------------------------------------------

/// Advance the AI state machine for all living prey animals.
///
/// Species-differentiated: movement speed, alertness, flee strategy (Standard,
/// SeekCover, Teleport, Stationary), and freeze/alert durations all come from
/// the `PreyConfig` component.
#[allow(clippy::type_complexity)]
pub fn prey_ai(
    mut query: Query<
        (&PreyConfig, &mut PreyState, &mut Position),
        (With<PreyAnimal>, Without<Dead>),
    >,
    cat_positions: Query<(Entity, &Position), (With<Needs>, Without<Dead>, Without<PreyAnimal>)>,
    positions: Query<&Position, Without<PreyAnimal>>,
    dens: Query<(&PreyDen, &Position), Without<PreyAnimal>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
) {
    let p = &constants.prey;
    for (config, mut state, mut pos) in &mut query {
        // O(1) den lookup for predation pressure → vigilance.
        let (pressure, home_den_pos) = state
            .home_den
            .and_then(|e| dens.get(e).ok())
            .map(|(d, dp)| (d.predation_pressure, Some(*dp)))
            .unwrap_or((0.0, None));
        let vigilance_mod = vigilance_from_pressure(
            pressure,
            p.vigilance_center,
            p.vigilance_steepness,
            p.vigilance_baseline,
            p.vigilance_amplitude,
        );

        match state.ai_state {
            PreyAiState::Idle => {
                if let Some(threat) = try_detect_cat(
                    &pos,
                    config.kind,
                    constants
                        .sensory
                        .profile_for(crate::components::SensorySpecies::Prey(config.kind)),
                    config.alert_radius,
                    state.alertness,
                    vigilance_mod,
                    p.detection_base_chance,
                    p.alertness_base,
                    p.alertness_range,
                    &cat_positions,
                    &mut rng,
                ) {
                    if config.flee_strategy == FleeStrategy::Teleport {
                        let threat_pos = cat_positions.get(threat).map(|(_, p)| *p).unwrap_or(*pos);
                        bird_teleport(
                            &mut pos,
                            &threat_pos,
                            config.habitat,
                            &map,
                            &mut rng,
                            p.bird_teleport_min_range,
                            p.bird_teleport_max_range,
                        );
                        state.alertness = 0.0;
                    } else if config.freeze_ticks == 0 {
                        // Fish: no alert/flee.
                    } else {
                        state.ai_state = PreyAiState::Alert { threat, ticks: 0 };
                        continue;
                    }
                }

                state.alertness = (state.alertness + p.alertness_recovery).min(1.0);

                if rng.rng.random::<f32>() < p.grazing_wander_chance {
                    let dx = rng.rng.random_range(-1i32..=1);
                    let dy = rng.rng.random_range(-1i32..=1);
                    if dx != 0 || dy != 0 {
                        state.ai_state = PreyAiState::Grazing { dx, dy, ticks: 0 };
                    }
                }
            }

            PreyAiState::Grazing {
                mut dx,
                mut dy,
                ticks,
            } => {
                let new_ticks = ticks + 1;

                if let Some(threat) = try_detect_cat(
                    &pos,
                    config.kind,
                    constants
                        .sensory
                        .profile_for(crate::components::SensorySpecies::Prey(config.kind)),
                    config.alert_radius,
                    state.alertness,
                    vigilance_mod,
                    p.detection_base_chance,
                    p.alertness_base,
                    p.alertness_range,
                    &cat_positions,
                    &mut rng,
                ) {
                    if config.flee_strategy == FleeStrategy::Teleport {
                        let threat_pos = cat_positions.get(threat).map(|(_, p)| *p).unwrap_or(*pos);
                        bird_teleport(
                            &mut pos,
                            &threat_pos,
                            config.habitat,
                            &map,
                            &mut rng,
                            p.bird_teleport_min_range,
                            p.bird_teleport_max_range,
                        );
                        state.alertness = 0.0;
                    } else if config.freeze_ticks > 0 {
                        state.ai_state = PreyAiState::Alert { threat, ticks: 0 };
                        continue;
                    }
                }

                state.alertness = (state.alertness + p.alertness_recovery).min(1.0);

                if rng.rng.random::<f32>() < p.grazing_jitter_chance {
                    let jdx = rng.rng.random_range(-1i32..=1);
                    let jdy = rng.rng.random_range(-1i32..=1);
                    if jdx != 0 || jdy != 0 {
                        dx = jdx;
                        dy = jdy;
                    }
                }

                // Roaming limit: under pressure, stay close to home den.
                if let Some(den_pos) = home_den_pos {
                    let den_dist = pos.manhattan_distance(&den_pos);
                    let max_roam = if pressure > p.grazing_pressure_roam_threshold {
                        p.grazing_max_roam_pressured
                    } else {
                        p.grazing_max_roam_normal
                    };
                    if den_dist > max_roam {
                        dx = (den_pos.x - pos.x).signum();
                        dy = (den_pos.y - pos.y).signum();
                    }
                }

                if new_ticks % config.graze_cadence == 0 {
                    let nx = pos.x + dx;
                    let ny = pos.y + dy;
                    let corr_thresh = p.prey_corruption_avoidance;

                    if can_move_to_prey(nx, ny, config.habitat, &map, corr_thresh) {
                        pos.x = nx;
                        pos.y = ny;
                    } else {
                        dx = -dx;
                        dy = -dy;
                        let rx = pos.x + dx;
                        let ry = pos.y + dy;
                        if can_move_to_prey(rx, ry, config.habitat, &map, corr_thresh) {
                            pos.x = rx;
                            pos.y = ry;
                        }
                    }
                }

                if new_ticks >= p.grazing_max_ticks {
                    state.ai_state = PreyAiState::Idle;
                } else {
                    state.ai_state = PreyAiState::Grazing {
                        dx,
                        dy,
                        ticks: new_ticks,
                    };
                }
            }

            PreyAiState::Alert { threat, ticks } => {
                let new_ticks = ticks + 1;

                let threat_pos = cat_positions
                    .get(threat)
                    .map(|(_, p)| p)
                    .or_else(|_| positions.get(threat))
                    .ok();

                let still_near = threat_pos
                    .is_some_and(|tp| pos.manhattan_distance(tp) <= config.alert_radius + 2);

                if !still_near {
                    state.ai_state = PreyAiState::Idle;
                } else if new_ticks >= config.freeze_ticks {
                    // Compute flee direction toward home den (if any).
                    let toward =
                        home_den_pos.map(|dp| ((dp.x - pos.x).signum(), (dp.y - pos.y).signum()));
                    state.ai_state = PreyAiState::Fleeing {
                        from: threat,
                        toward,
                        ticks: 0,
                    };
                } else {
                    state.ai_state = PreyAiState::Alert {
                        threat,
                        ticks: new_ticks,
                    };
                }
            }

            PreyAiState::Fleeing {
                from,
                toward,
                ticks,
            } => {
                let new_ticks = ticks + 1;

                let threat_pos = cat_positions
                    .get(from)
                    .map(|(_, p)| p)
                    .or_else(|_| positions.get(from))
                    .ok();

                let should_stop = new_ticks >= config.flee_duration
                    || threat_pos.is_none()
                    || threat_pos
                        .map(|tp| pos.manhattan_distance(tp) > p.flee_stop_distance)
                        .unwrap_or(true);

                if should_stop {
                    state.alertness = 0.0;
                    state.ai_state = PreyAiState::Idle;
                    continue;
                }

                let tp = threat_pos.unwrap();

                match config.flee_strategy {
                    FleeStrategy::Standard => {
                        for _ in 0..config.flee_speed {
                            if let Some((dx, dy)) = toward {
                                // Flee toward home den.
                                let nx = pos.x + dx;
                                let ny = pos.y + dy;
                                if can_move_to(nx, ny, config.habitat, &map) {
                                    pos.x = nx;
                                    pos.y = ny;
                                } else {
                                    flee_step(&mut pos, tp, config.habitat, &map);
                                }
                            } else {
                                flee_step(&mut pos, tp, config.habitat, &map);
                            }
                        }
                    }
                    FleeStrategy::SeekCover => {
                        // Prefer den direction, then forest cover, then away-from-threat.
                        if let Some((dx, dy)) = toward {
                            let nx = pos.x + dx;
                            let ny = pos.y + dy;
                            if can_move_to(nx, ny, config.habitat, &map) {
                                pos.x = nx;
                                pos.y = ny;
                            } else if let Some((cdx, cdy)) = find_cover_direction(&pos, tp, &map) {
                                let nx = pos.x + cdx;
                                let ny = pos.y + cdy;
                                if can_move_to(nx, ny, config.habitat, &map) {
                                    pos.x = nx;
                                    pos.y = ny;
                                } else {
                                    flee_step(&mut pos, tp, config.habitat, &map);
                                }
                            } else {
                                flee_step(&mut pos, tp, config.habitat, &map);
                            }
                        } else if let Some((dx, dy)) = find_cover_direction(&pos, tp, &map) {
                            let nx = pos.x + dx;
                            let ny = pos.y + dy;
                            if can_move_to(nx, ny, config.habitat, &map) {
                                pos.x = nx;
                                pos.y = ny;
                            } else {
                                flee_step(&mut pos, tp, config.habitat, &map);
                            }
                        } else {
                            flee_step(&mut pos, tp, config.habitat, &map);
                        }
                    }
                    FleeStrategy::Teleport => {
                        let mut landed = false;
                        for _ in 0..20 {
                            let range = rng.rng.random_range(
                                p.bird_teleport_min_range..=p.bird_teleport_max_range,
                            );
                            let angle: f32 = rng.rng.random::<f32>() * std::f32::consts::TAU;
                            let nx = tp.x + (angle.cos() * range as f32) as i32;
                            let ny = tp.y + (angle.sin() * range as f32) as i32;
                            if can_move_to(nx, ny, config.habitat, &map) {
                                pos.x = nx;
                                pos.y = ny;
                                landed = true;
                                break;
                            }
                        }
                        if !landed {
                            for _ in 0..config.flee_speed {
                                flee_step(&mut pos, tp, config.habitat, &map);
                            }
                        }
                    }
                    FleeStrategy::Stationary => {
                        state.alertness = 0.0;
                        state.ai_state = PreyAiState::Idle;
                        continue;
                    }
                }

                state.ai_state = PreyAiState::Fleeing {
                    from,
                    toward,
                    ticks: new_ticks,
                };
            }
        }
    }
}

/// Move one tile away from threat, trying diagonal then cardinals.
fn flee_step(pos: &mut Mut<Position>, threat: &Position, habitat: &[Terrain], map: &TileMap) {
    let dx = (pos.x - threat.x).signum();
    let dy = (pos.y - threat.y).signum();
    let candidates = [
        (pos.x + dx, pos.y + dy),
        (pos.x + dx, pos.y),
        (pos.x, pos.y + dy),
    ];
    for (nx, ny) in candidates {
        if can_move_to(nx, ny, habitat, map) {
            pos.x = nx;
            pos.y = ny;
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// prey_scent_tick system (Phase 2B)
// ---------------------------------------------------------------------------

/// Live prey deposit scent onto `PreyScentMap` each tick; the whole
/// grid decays globally. Mirrors `fox_scent_tick` — scent becomes a
/// grid-addressable influence-map read rather than a point-to-point
/// wind-aware formula per (cat, prey) pair.
///
/// Per §5.6.3 row #1, scent propagation is eventually `wind+terrain`.
/// Phase 2B ships the simplest viable version (uniform deposit; no
/// directional plume, no terrain modulation on stamp) so cats get a
/// usable scent read this phase; directional stamping is a
/// follow-on balance pass.
pub fn prey_scent_tick(
    prey: Query<&Position, (With<PreyAnimal>, Without<Dead>)>,
    mut scent_map: ResMut<crate::resources::PreyScentMap>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let p = &constants.prey;
    // Global decay first — prior-tick deposits fade before this tick's
    // stamps land, matching FoxScentMap's ordering. Activity-trail
    // semantics (~1 in-game day to detect threshold), not territorial.
    scent_map.decay_all(p.scent_decay_rate.per_tick(&time_scale));
    for pos in &prey {
        scent_map.deposit(pos.x, pos.y, p.scent_deposit_per_tick);
    }
}

// ---------------------------------------------------------------------------
// prey_population system
// ---------------------------------------------------------------------------

/// Breed prey via dens (primary) and background breeding (secondary).
/// Applies seasonal modifiers from the species registry.
#[allow(clippy::too_many_arguments)]
pub fn prey_population(
    mut commands: Commands,
    configs: Query<&PreyConfig, With<PreyAnimal>>,
    mut dens: Query<(Entity, &mut PreyDen, &Position)>,
    registry: Res<SpeciesRegistry>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut prey_density: ResMut<PreyDensity>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let p = &constants.prey;
    let season = time.season(&config);

    // Count living prey per species.
    let mut counts: HashMap<PreyKind, usize> = HashMap::new();
    for prey_config in &configs {
        *counts.entry(prey_config.kind).or_insert(0) += 1;
    }

    // Update the density resource (read by pounce formula for density vulnerability).
    prey_density.0.clear();
    for profile in &registry.profiles {
        let profile = profile.as_ref();
        let kind = profile.kind();
        let pop = *counts.get(&kind).unwrap_or(&0);
        let cap = profile.population_cap();
        prey_density.0.insert(kind, pop as f32 / cap as f32);
    }

    let is_winter = matches!(season, crate::resources::time::Season::Winter);

    // --- Den-based spawning (primary) + refill ---
    for (den_entity, mut den, den_pos) in &mut dens {
        let profile = registry.find(den.kind);
        let pop = *counts.get(&den.kind).unwrap_or(&0);
        let cap = profile.population_cap();

        // Refill: dens regenerate spawns over time (prey rebuild nests).
        if den.spawns_remaining < den.capacity && !is_winter {
            let refill_chance = p.den_refill_base_chance * (den.capacity as f32 / 50.0);
            if rng.rng.random::<f32>() < refill_chance {
                den.spawns_remaining += 1;
            }
        }

        if den.spawns_remaining == 0 || pop >= cap {
            continue;
        }

        // Breeding suppression from predation pressure (ecology of fear).
        let fear_breeding_mod = 1.0 - den.predation_pressure * p.den_fear_breeding_suppression;

        // Corruption near the den further suppresses breeding.
        let den_corruption = if map.in_bounds(den_pos.x, den_pos.y) {
            map.get(den_pos.x, den_pos.y).corruption
        } else {
            0.0
        };
        let corruption_breeding_mod = if den_corruption > p.den_corruption_threshold {
            1.0 - den_corruption
        } else {
            1.0
        };

        if rng.rng.random::<f32>()
            < profile.den_spawn_rate() * fear_breeding_mod * corruption_breeding_mod
        {
            if let Some(spawn_pos) =
                find_nearby_habitat_tile(den_pos, 8, profile.habitat(), &map, &mut rng)
            {
                activation.record(Feature::PreyBred);
                let mut bundle = crate::components::prey::prey_bundle(profile);
                bundle.2.home_den = Some(den_entity); // Set home den on spawn.
                commands.spawn((bundle, spawn_pos));
                den.spawns_remaining -= 1;
                *counts.entry(den.kind).or_insert(0) += 1;
            }
        }
    }

    // --- Background breeding (secondary, 25% rate with seasonal modifier) ---
    for profile in &registry.profiles {
        let profile = profile.as_ref();
        let kind = profile.kind();
        let pop = *counts.get(&kind).unwrap_or(&0);
        let cap = profile.population_cap();

        let density_pressure = 1.0 - (pop as f32 / cap as f32);

        if density_pressure <= 0.0 {
            if rng.rng.random::<f32>() < 0.001 {
                log.push(
                    time.tick,
                    format!(
                        "The {} have overrun their territory.",
                        profile.plural_name()
                    ),
                    NarrativeTier::Nature,
                );
            }
            continue;
        }

        if density_pressure < 0.2 && rng.rng.random::<f32>() < 0.002 {
            log.push(
                time.tick,
                format!("The {} are growing restless.", profile.plural_name()),
                NarrativeTier::Nature,
            );
        }

        let seasonal_mod = profile.seasonal_breed_modifier(season);
        let breed_chance = profile.breed_rate()
            * p.background_breed_rate_multiplier
            * density_pressure
            * seasonal_mod;
        if rng.rng.random::<f32>() < breed_chance {
            if let Some(pos) = find_habitat_tile(profile.habitat(), &map, &mut rng) {
                activation.record(Feature::PreyBred);
                let bundle = crate::components::prey::prey_bundle(profile);
                commands.spawn((bundle, pos));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// prey_den_lifecycle system
// ---------------------------------------------------------------------------

/// Spawn new dens when prey populations are healthy and conditions are right.
/// Also decays predation_pressure on all existing dens each tick.
/// Decay predation pressure, track stress, and abandon dens under sustained
/// high pressure. New den formation is handled by `orphan_prey_adopt_or_found`.
pub fn prey_den_lifecycle(
    mut commands: Commands,
    mut existing_dens: Query<(Entity, &mut PreyDen, &Position)>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    map: Res<TileMap>,
    mut activation: ResMut<SystemActivation>,
) {
    let p = &constants.prey;
    for (entity, mut den, den_pos) in &mut existing_dens {
        // Decay predation pressure (half-life ~1400 ticks ≈ 1.4 days).
        den.predation_pressure *= p.den_predation_pressure_decay;

        // Corruption on the den tile counts as additional stress.
        let tile_corruption = if map.in_bounds(den_pos.x, den_pos.y) {
            map.get(den_pos.x, den_pos.y).corruption
        } else {
            0.0
        };

        // Track stress: sustained high pressure or corruption → abandonment.
        if den.predation_pressure > p.den_stress_high_threshold
            || tile_corruption > p.den_corruption_threshold
        {
            den.stressed_ticks += 1;
        } else if den.predation_pressure < p.den_stress_low_threshold {
            den.stressed_ticks = 0;
        }

        // Abandon after ~3 days of sustained high pressure.
        if u64::from(den.stressed_ticks) > p.den_abandon_stress_ticks {
            activation.record(Feature::PreyDenAbandoned);
            let name = den.den_name;
            let kind_name = match den.kind {
                PreyKind::Mouse => "mice",
                PreyKind::Rat => "rats",
                PreyKind::Rabbit => "rabbits",
                PreyKind::Fish => "fish",
                PreyKind::Bird => "birds",
            };
            log.push(
                time.tick,
                format!("The {kind_name} have abandoned their {name}."),
                NarrativeTier::Nature,
            );
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// update_den_pressure system (event-driven)
// ---------------------------------------------------------------------------

/// Updates den predation_pressure when prey are killed nearby.
/// Reads `PreyKilled` events — zero work on ticks with no kills.
pub fn update_den_pressure(
    mut dens: Query<(&mut PreyDen, &Position)>,
    mut kills: MessageReader<PreyKilled>,
    constants: Res<SimConstants>,
) {
    let p = &constants.prey;
    for kill in kills.read() {
        for (mut den, den_pos) in &mut dens {
            if den.kind == kill.kind
                && kill.position.manhattan_distance(den_pos) <= p.den_kill_pressure_range
            {
                den.predation_pressure =
                    (den.predation_pressure + p.den_kill_pressure_increment).min(1.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// apply_den_raids system (message-driven)
// ---------------------------------------------------------------------------

/// Processes `DenRaided` messages: reduces den spawns, spikes predation pressure.
pub fn apply_den_raids(
    mut dens: Query<&mut PreyDen>,
    mut raids: MessageReader<DenRaided>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let p = &constants.prey;
    for raid in raids.read() {
        if let Ok(mut den) = dens.get_mut(raid.den_entity) {
            activation.record(Feature::DenRaided);
            den.spawns_remaining = den.spawns_remaining.saturating_sub(raid.kills);
            den.predation_pressure =
                (den.predation_pressure + p.den_raid_pressure_increment).min(1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// orphan_prey_adopt_or_found system
// ---------------------------------------------------------------------------

/// Orphaned prey (home_den is None or stale) try to adopt a nearby den.
/// If no den within 25 tiles, tiny chance to found a new one.
#[allow(clippy::too_many_arguments)]
pub fn orphan_prey_adopt_or_found(
    mut commands: Commands,
    mut prey: Query<(&PreyConfig, &mut PreyState, &Position), With<PreyAnimal>>,
    dens: Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    time: Res<TimeState>,
    registry: Res<SpeciesRegistry>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let p = &constants.prey;
    for (config, mut state, pos) in &mut prey {
        // Only process orphans.
        let is_orphan = state.home_den.is_none_or(|e| dens.get(e).is_err());
        if !is_orphan {
            continue;
        }

        // Try to adopt: find nearest same-species den within range.
        let adoptable = dens
            .iter()
            .filter(|(_, d, dp)| {
                d.kind == config.kind
                    && pos.manhattan_distance(dp) <= p.den_orphan_adopt_range
                    && (d.spawns_remaining as f32)
                        < d.capacity as f32 * p.den_orphan_adopt_capacity_threshold
            })
            .min_by_key(|(_, _, dp)| pos.manhattan_distance(dp));

        if let Some((den_entity, _, _)) = adoptable {
            state.home_den = Some(den_entity);
            continue;
        }

        // No den to adopt — try to found (very rare).
        if rng.rng.random::<f32>() >= p.den_orphan_found_chance {
            continue;
        }

        // Must be on valid den habitat.
        let profile = registry.find(config.kind);
        if !profile
            .den_habitat()
            .contains(&map.get(pos.x, pos.y).terrain)
        {
            continue;
        }

        // Must be far from any same-species den.
        let too_close = dens.iter().any(|(_, d, dp)| {
            d.kind == config.kind && pos.manhattan_distance(dp) < p.den_orphan_min_spacing
        });
        if too_close {
            continue;
        }

        // Found a new den!
        activation.record(Feature::PreyDenFounded);
        let mut den = PreyDen::from_profile(profile);
        den.capacity /= 2; // Starts small.
        den.spawns_remaining = den.capacity;
        let den_entity = commands.spawn((den, Health::default(), *pos)).id();
        state.home_den = Some(den_entity);

        // Direction from map center for narrative flavor.
        let cx = map.width / 2;
        let cy = map.height / 2;
        let dir = if (pos.x - cx).abs() > (pos.y - cy).abs() {
            if pos.x > cx {
                "eastern"
            } else {
                "western"
            }
        } else if pos.y > cy {
            "southern"
        } else {
            "northern"
        };
        log.push(
            time.tick,
            format!(
                "A colony of {} has established a new {} in the {} wilds.",
                profile.name(),
                profile.den_name(),
                dir,
            ),
            NarrativeTier::Nature,
        );
    }
}

// ---------------------------------------------------------------------------
// prey_hunger system
// ---------------------------------------------------------------------------

/// Advance hunger for all prey; despawn any that starve.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn prey_hunger(
    mut commands: Commands,
    mut query: Query<
        (Entity, &PreyConfig, &mut PreyState, &mut Health, &Position),
        (With<PreyAnimal>, Without<Structure>),
    >,
    mut stores_query: Query<
        (Entity, &mut Structure, &Position, &mut StoredItems),
        Without<PreyAnimal>,
    >,
    cat_positions: Query<&Position, (With<Needs>, Without<Dead>, Without<PreyAnimal>)>,
    items_query: Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    registry: Res<SpeciesRegistry>,
    constants: Res<SimConstants>,
) {
    let p = &constants.prey;
    // Count population per species.
    let mut counts: HashMap<PreyKind, usize> = HashMap::new();
    for (_, cfg, _, _, _) in query.iter() {
        *counts.entry(cfg.kind).or_insert(0) += 1;
    }

    // Snapshot store positions and guarding state.
    let store_positions: Vec<(Entity, Position, bool)> = stores_query
        .iter()
        .filter(|(_, s, _, _)| s.kind == StructureType::Stores)
        .map(|(e, _, p, _)| {
            let guarded = cat_positions.iter().any(|cp| cp.manhattan_distance(p) <= 4);
            (e, *p, guarded)
        })
        .collect();

    for (entity, cfg, mut state, mut health, pos) in &mut query {
        let profile = registry.find(cfg.kind);
        let pop = *counts.get(&cfg.kind).unwrap_or(&0);
        let cap = profile.population_cap();

        // Base hunger increase.
        state.hunger += p.hunger_base_rate;

        // Overcrowding penalty above threshold.
        if pop as f32 > cap as f32 * p.overcrowding_threshold {
            state.hunger += p.overcrowding_hunger_extra;
        }

        // Mice and rats near stores raid them.
        let mut ate_from_stores = false;
        if matches!(cfg.kind, PreyKind::Mouse | PreyKind::Rat)
            && rng.rng.random::<f32>() < p.store_raid_chance
        {
            for &(store_entity, store_pos, guarded) in &store_positions {
                if guarded || pos.manhattan_distance(&store_pos) > p.store_raid_range {
                    continue;
                }
                if let Ok((_, mut structure, _, mut stored)) = stores_query.get_mut(store_entity) {
                    let food_entity = stored
                        .items
                        .iter()
                        .copied()
                        .find(|&e| items_query.get(e).is_ok_and(|i| i.kind.is_food()));
                    if let Some(food_entity) = food_entity {
                        stored.remove(food_entity);
                        commands.entity(food_entity).despawn();
                        state.hunger = (state.hunger - p.store_raid_hunger_relief).max(0.0);
                        structure.cleanliness =
                            (structure.cleanliness - p.store_raid_cleanliness_drain).max(0.0);
                        ate_from_stores = true;

                        if rng.rng.random::<f32>() < p.store_raid_narrative_chance {
                            log.push(
                                time.tick,
                                format!("A {} has gotten into the stores!", cfg.name),
                                NarrativeTier::Nature,
                            );
                        }
                        break;
                    }
                }
            }
        }

        if !ate_from_stores {
            state.hunger -= p.passive_hunger_relief;
        }
        state.hunger = state.hunger.clamp(0.0, 1.0);

        // Starvation drains health.
        if state.hunger > p.starvation_threshold {
            health.current -= p.starvation_health_drain;
        }

        if health.current <= 0.0 {
            if rng.rng.random::<f32>() < p.starvation_narrative_chance {
                log.push(
                    time.tick,
                    format!("A {} collapses from hunger.", cfg.name),
                    NarrativeTier::Nature,
                );
            }
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------
// spawn_initial_prey (world-gen helper, not a system)
// ---------------------------------------------------------------------------

/// Spawn initial prey population and dens during world generation.
pub fn spawn_initial_prey(world: &mut World) {
    let prey_constants = world.resource::<SimConstants>().prey.clone();
    let initial_den_counts: &[(PreyKind, usize)] = &[
        (PreyKind::Mouse, prey_constants.initial_den_count_mouse),
        (PreyKind::Rat, prey_constants.initial_den_count_rat),
        (PreyKind::Rabbit, prey_constants.initial_den_count_rabbit),
        (PreyKind::Fish, prey_constants.initial_den_count_fish),
        (PreyKind::Bird, prey_constants.initial_den_count_bird),
    ];

    // Snapshot terrain.
    let (map_width, map_height, terrain_snapshot): (i32, i32, Vec<Terrain>) = {
        let map = world.resource::<TileMap>();
        let snapshot = (0..map.height)
            .flat_map(|y| (0..map.width).map(move |x| (x, y)))
            .map(|(x, y)| map.get(x, y).terrain)
            .collect();
        (map.width, map.height, snapshot)
    };

    // Snapshot species data from the registry so we can release the borrow
    // before taking &mut for the RNG.
    struct SpeciesInfo {
        kind: PreyKind,
        den_count: usize,
        den_habitat: &'static [Terrain],
        prey_habitat: &'static [Terrain],
        prey_count: usize,
        den_template: PreyDen,
    }
    let species_info: Vec<SpeciesInfo> = {
        let registry = world.resource::<SpeciesRegistry>();
        initial_den_counts
            .iter()
            .map(|&(kind, den_count)| {
                let profile = registry.find(kind);
                SpeciesInfo {
                    kind,
                    den_count,
                    den_habitat: profile.den_habitat(),
                    prey_habitat: profile.habitat(),
                    prey_count: profile.population_cap() / 2,
                    den_template: PreyDen::from_profile(profile),
                }
            })
            .collect()
    };

    // Collect den and prey spawn data.
    let mut den_spawns: Vec<(PreyDen, Position)> = Vec::new();
    let mut prey_spawns: Vec<(PreyKind, Position)> = Vec::new();

    {
        let rng = &mut world.resource_mut::<SimRng>().rng;

        for info in &species_info {
            let den_habitat = info.den_habitat;
            let prey_habitat = info.prey_habitat;
            let prey_count = info.prey_count;

            // Spawn dens.
            let mut den_positions: Vec<Position> = Vec::new();
            for _ in 0..info.den_count {
                let mut attempts = 0;
                while attempts < 200 {
                    attempts += 1;
                    let x: i32 = rng.random_range(0..map_width);
                    let y: i32 = rng.random_range(0..map_height);
                    let terrain = terrain_snapshot[(y * map_width + x) as usize];
                    if !den_habitat.contains(&terrain) {
                        continue;
                    }
                    let pos = Position::new(x, y);
                    // Ensure spacing between dens of the same species.
                    let too_close = den_positions
                        .iter()
                        .any(|dp| pos.manhattan_distance(dp) < 15);
                    if too_close {
                        continue;
                    }
                    den_positions.push(pos);
                    den_spawns.push((info.den_template.clone(), pos));
                    break;
                }
            }

            // Spawn initial prey near dens.
            let mut spawned = 0;
            let mut attempts = 0;
            while spawned < prey_count && attempts < prey_count * 50 {
                attempts += 1;
                if den_positions.is_empty() {
                    // Fallback: random habitat tile if no dens placed.
                    let x: i32 = rng.random_range(0..map_width);
                    let y: i32 = rng.random_range(0..map_height);
                    let terrain = terrain_snapshot[(y * map_width + x) as usize];
                    if prey_habitat.contains(&terrain) {
                        prey_spawns.push((info.kind, Position::new(x, y)));
                        spawned += 1;
                    }
                } else {
                    // Spawn near a random den.
                    let den_idx = rng.random_range(0..den_positions.len());
                    let den_pos = den_positions[den_idx];
                    let dx = rng.random_range(-10..=10i32);
                    let dy = rng.random_range(-10..=10i32);
                    let x = (den_pos.x + dx).clamp(0, map_width - 1);
                    let y = (den_pos.y + dy).clamp(0, map_height - 1);
                    let terrain = terrain_snapshot[(y * map_width + x) as usize];
                    if prey_habitat.contains(&terrain) {
                        prey_spawns.push((info.kind, Position::new(x, y)));
                        spawned += 1;
                    }
                }
            }
        }
    }

    // Spawn den entities.
    for (den, pos) in den_spawns {
        world.spawn((den, Health::default(), pos));
    }

    // Snapshot configs for prey spawning (release registry borrow before spawning).
    let prey_bundles: Vec<(crate::components::prey::PreyBundle, Position)> = {
        let registry = world.resource::<SpeciesRegistry>();
        prey_spawns
            .into_iter()
            .map(|(kind, pos)| {
                let profile = registry.find(kind);
                (crate::components::prey::prey_bundle(profile), pos)
            })
            .collect()
    };
    for (bundle, pos) in prey_bundles {
        world.spawn((bundle, pos));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::map::Terrain;
    use bevy_ecs::schedule::Schedule;

    fn setup() -> (World, Schedule) {
        let mut world = World::new();
        let map = TileMap::new(20, 20, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: crate::resources::SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(crate::species::build_registry());
        world.insert_resource(PreyDensity::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(SystemActivation::default());
        let mut schedule = Schedule::default();
        schedule.add_systems((prey_population, prey_hunger).chain());
        (world, schedule)
    }

    fn spawn_prey_of(world: &mut World, kind: PreyKind, pos: Position) {
        let registry = world.resource::<SpeciesRegistry>();
        let profile = registry.find(kind);
        let bundle = crate::components::prey::prey_bundle(profile);
        world.spawn((bundle, pos));
    }

    #[test]
    fn prey_breed_when_below_cap() {
        let (mut world, mut schedule) = setup();

        // Spawn a den so breeding has a primary source.
        world.spawn((
            PreyDen::new(PreyKind::Mouse, 100),
            Health::default(),
            Position::new(10, 10),
        ));

        for i in 0..5i32 {
            spawn_prey_of(&mut world, PreyKind::Mouse, Position::new(i, 0));
        }

        for _ in 0..2000 {
            schedule.run(&mut world);
        }

        let count = world
            .query::<&PreyConfig>()
            .iter(&world)
            .filter(|c| c.kind == PreyKind::Mouse)
            .count();

        assert!(
            count > 5,
            "mice should have bred after 2000 ticks, got {count}"
        );
    }

    #[test]
    fn prey_do_not_exceed_cap() {
        let (mut world, mut schedule) = setup();

        for i in 0..80i32 {
            spawn_prey_of(&mut world, PreyKind::Mouse, Position::new(i % 20, i / 20));
        }

        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let count = world
            .query::<&PreyConfig>()
            .iter(&world)
            .filter(|c| c.kind == PreyKind::Mouse)
            .count();

        assert!(count <= 80, "mice at cap should not exceed 80, got {count}");
    }

    // -----------------------------------------------------------------------
    // prey_ai tests
    // -----------------------------------------------------------------------

    fn setup_ai() -> (World, Schedule) {
        let mut world = World::new();
        let map = TileMap::new(20, 20, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(crate::species::build_registry());
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(prey_ai);
        (world, schedule)
    }

    #[test]
    fn prey_grazes_and_moves() {
        let (mut world, mut schedule) = setup_ai();

        let start = Position::new(10, 10);
        let registry = world.resource::<SpeciesRegistry>();
        let profile = registry.find(PreyKind::Mouse);
        let config = profile.to_config();
        let mut state = PreyState::default();
        state.ai_state = PreyAiState::Grazing {
            dx: 1,
            dy: 0,
            ticks: 0,
        };
        world.spawn((PreyAnimal, config, state, Health::default(), start));

        for _ in 0..60 {
            schedule.run(&mut world);
        }

        let final_pos = *world
            .query::<&Position>()
            .iter(&world)
            .find(|p| p.x != 0 || p.y != 0) // skip any zero-pos entities
            .unwrap_or(&start);
        assert!(
            final_pos != start,
            "prey should have moved from {start:?} after 60 ticks of grazing, still at {final_pos:?}"
        );
    }

    #[test]
    fn prey_flees_from_threat() {
        let (mut world, mut schedule) = setup_ai();

        // Threat entity (not a cat, just a position source).
        let threat = world.spawn(Position::new(5, 5)).id();

        let start = Position::new(7, 7);
        let registry = world.resource::<SpeciesRegistry>();
        let profile = registry.find(PreyKind::Mouse);
        let config = profile.to_config();
        let mut state = PreyState::default();
        state.ai_state = PreyAiState::Fleeing {
            from: threat,
            toward: None,
            ticks: 0,
        };
        world.spawn((PreyAnimal, config, state, Health::default(), start));

        for _ in 0..10 {
            schedule.run(&mut world);
        }

        let final_pos = *world
            .query_filtered::<&Position, With<PreyAnimal>>()
            .single(&world)
            .unwrap();

        let threat_pos = Position::new(5, 5);
        let start_dist = start.manhattan_distance(&threat_pos);
        let end_dist = final_pos.manhattan_distance(&threat_pos);
        assert!(
            end_dist > start_dist,
            "prey should flee away from threat: start_dist={start_dist}, end_dist={end_dist}, final_pos={final_pos:?}"
        );
    }

    #[test]
    fn prey_alert_detects_nearby_cat() {
        let (mut world, mut schedule) = setup_ai();

        // Spawn a "cat" (needs Needs component for detection).
        world.spawn((Needs::default(), Health::default(), Position::new(10, 10)));

        // Spawn a rabbit (alert_radius=6) very close to the cat.
        let registry = world.resource::<SpeciesRegistry>();
        let profile = registry.find(PreyKind::Rabbit);
        let config = profile.to_config();
        let mut state = PreyState::default();
        state.alertness = 1.0; // Max alertness for reliable detection.
        world.spawn((
            PreyAnimal,
            config,
            state,
            Health::default(),
            Position::new(11, 10), // 1 tile away — high detection chance per tick
        ));

        // Run enough ticks for probabilistic detection to trigger.
        for _ in 0..30 {
            schedule.run(&mut world);
        }

        let prey_state = world
            .query_filtered::<&PreyState, With<PreyAnimal>>()
            .single(&world)
            .unwrap();

        // Should have transitioned from Idle → Alert → Fleeing.
        assert!(
            matches!(
                prey_state.ai_state,
                PreyAiState::Alert { .. } | PreyAiState::Fleeing { .. }
            ),
            "rabbit near cat should enter Alert or Fleeing, got {:?}",
            prey_state.ai_state,
        );
    }

    // -----------------------------------------------------------------------
    // Store raiding tests
    // -----------------------------------------------------------------------

    fn setup_hunger() -> (World, Schedule) {
        use crate::components::items::{Item, ItemKind, ItemLocation};

        let mut world = World::new();
        let map = TileMap::new(20, 20, Terrain::Grass);
        world.insert_resource(map);
        world.insert_resource(SimRng::new(42));
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: crate::resources::SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(crate::species::build_registry());
        world.insert_resource(SimConstants::default());

        // Stores building at (5, 5) with one food item.
        let stores_entity = world
            .spawn((
                Structure::new(StructureType::Stores),
                Position::new(5, 5),
                StoredItems::default(),
            ))
            .id();
        let food_entity = world
            .spawn(Item::new(
                ItemKind::RawMouse,
                0.5,
                ItemLocation::StoredIn(stores_entity),
            ))
            .id();
        world
            .entity_mut(stores_entity)
            .get_mut::<StoredItems>()
            .unwrap()
            .add(food_entity, StructureType::Stores);

        let mut schedule = Schedule::default();
        schedule.add_systems(prey_hunger);
        (world, schedule)
    }

    #[test]
    fn prey_raids_nearby_stores() {
        let (mut world, mut schedule) = setup_hunger();

        spawn_prey_of(&mut world, PreyKind::Mouse, Position::new(5, 6));

        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let stored = world.query::<&StoredItems>().single(&world).unwrap();
        assert!(
            stored.items.is_empty(),
            "mouse adjacent to stores should have eaten the food item within 100 ticks"
        );
    }

    #[test]
    fn fish_and_birds_do_not_raid() {
        let (mut world, mut schedule) = setup_hunger();

        spawn_prey_of(&mut world, PreyKind::Fish, Position::new(5, 6));
        spawn_prey_of(&mut world, PreyKind::Bird, Position::new(6, 5));

        schedule.run(&mut world);

        let stored = world.query::<&StoredItems>().single(&world).unwrap();
        assert_eq!(
            stored.items.len(),
            1,
            "fish and birds should not raid stores"
        );
    }

    #[test]
    fn cat_near_stores_deters_raiding() {
        let (mut world, mut schedule) = setup_hunger();

        spawn_prey_of(&mut world, PreyKind::Mouse, Position::new(5, 6));
        // Cat guarding stores.
        world.spawn((Needs::default(), Health::default(), Position::new(5, 4)));

        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let stored = world.query::<&StoredItems>().single(&world).unwrap();
        assert_eq!(
            stored.items.len(),
            1,
            "cat guarding stores should deter mouse from raiding"
        );
    }

    #[test]
    fn den_spawns_prey_nearby() {
        let (mut world, mut schedule) = setup();

        let den_pos = Position::new(10, 10);
        world.spawn((
            PreyDen::new(PreyKind::Mouse, 50),
            Health::default(),
            den_pos,
        ));

        for _ in 0..500 {
            schedule.run(&mut world);
        }

        let count = world
            .query::<&PreyConfig>()
            .iter(&world)
            .filter(|c| c.kind == PreyKind::Mouse)
            .count();

        assert!(
            count > 0,
            "den should have spawned at least one mouse after 500 ticks, got {count}"
        );
    }

    #[test]
    fn den_refills_after_depletion() {
        let (mut world, mut schedule) = setup();

        // Den with capacity 5 — will deplete quickly then refill.
        world.spawn((
            PreyDen::new(PreyKind::Mouse, 5),
            Health::default(),
            Position::new(10, 10),
        ));

        // Run enough ticks for the den to deplete and start refilling.
        for _ in 0..3000 {
            schedule.run(&mut world);
        }

        // Den should still exist (not despawned).
        let den_count = world.query::<&PreyDen>().iter(&world).count();
        assert_eq!(
            den_count, 1,
            "den should persist after depletion (refills naturally)"
        );
    }
}
