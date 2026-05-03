use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::CurrentAction;
use crate::components::identity::Name;
use crate::components::magic::{
    FlavorPlant, GrowthStage, Harvestable, Herb, Inventory, MisfireEffect, RemedyEffect,
    RemedyKind, Seasonal, Ward, WardKind,
};
use crate::components::mental::{Memory, Mood, MoodModifier, MoodSource};
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::components::task_chain::{StepKind, StepStatus, TaskChain};
use crate::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{MagicConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{Season, SimConfig, TimeScale, TimeState};

// ---------------------------------------------------------------------------
// §4 colony-scoped ward marker predicate
// ---------------------------------------------------------------------------

/// True when the colony has no wards at all, or the average ward strength
/// is below `threshold`. Called from both scoring systems to populate the
/// `MarkerSnapshot` without duplicating predicate logic.
pub fn is_ward_strength_low<'a>(wards: impl Iterator<Item = &'a Ward>, threshold: f32) -> bool {
    let mut count = 0u32;
    let mut sum = 0.0f32;
    for ward in wards {
        count += 1;
        sum += ward.strength;
    }
    count == 0 || (sum / count as f32) < threshold
}

/// True when at least one harvestable Thornbriar exists in the world.
/// Mirrors the inline `herb_query.iter().any(|...kind == Thornbriar)`
/// scans previously living in `disposition::evaluate_dispositions` and
/// `goap::evaluate_and_plan`. Authors the `ThornbriarAvailable` colony
/// marker via the caller's `MarkerSnapshot::set_colony` site.
pub fn is_thornbriar_available<'a>(mut herbs: impl Iterator<Item = &'a Herb>) -> bool {
    herbs.any(|h| h.kind == crate::components::magic::HerbKind::Thornbriar)
}

/// True when at least one shadow fox is in the `EncirclingWard` AI
/// state — i.e. some colony ward is actively under siege. Mirrors the
/// inline `wildlife_ai_query.iter().any(...)` scan previously living in
/// `goap::evaluate_and_plan`. Authors the `WardsUnderSiege` colony
/// marker via the caller's `MarkerSnapshot::set_colony` site.
pub fn is_any_ward_under_siege<'a>(
    mut wildlife_ai: impl Iterator<Item = &'a WildlifeAiState>,
) -> bool {
    wildlife_ai.any(|s| matches!(s, WildlifeAiState::EncirclingWard { .. }))
}

// ---------------------------------------------------------------------------
// CorruptionPushback — message emitted by positive colony events
// ---------------------------------------------------------------------------

/// Emitted by births, bonds, socializing, etc. to reduce local corruption.
#[derive(Message, Debug, Clone)]
pub struct CorruptionPushback {
    pub position: Position,
    pub radius: i32,
    pub amount: f32,
}

// ---------------------------------------------------------------------------
// corruption_spread
// ---------------------------------------------------------------------------

/// Every 10 ticks, tiles with corruption > 0.3 bleed a fraction of their
/// corruption into the 4-adjacent neighbours.
pub fn corruption_spread(
    mut map: ResMut<TileMap>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    mut log: ResMut<NarrativeLog>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let m = &constants.magic;
    if !m.corruption_spread_cadence.fires_at(time.tick, &time_scale) {
        return;
    }
    let corruption_spread_rate = m.corruption_spread_rate.per_tick(&time_scale);

    // Snapshot corrupted tiles so we don't double-count within one pass.
    let mut sources: Vec<(i32, i32, f32)> = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let c = map.get(x, y).corruption;
            if c > m.corruption_spread_threshold {
                sources.push((x, y, c));
            }
        }
    }

    let mut new_tiles_corrupted = 0u32;
    let deltas: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    for (sx, sy, corruption) in sources {
        let spread = corruption * corruption_spread_rate;
        for (dx, dy) in &deltas {
            let nx = sx + dx;
            let ny = sy + dy;
            if map.in_bounds(nx, ny) {
                let tile = map.get_mut(nx, ny);
                let was_clean = tile.corruption < m.corruption_new_tile_threshold;
                tile.corruption = (tile.corruption + spread).min(1.0);
                if was_clean && tile.corruption >= m.corruption_new_tile_threshold {
                    new_tiles_corrupted += 1;
                }
            }
        }
    }

    // Narrate when corruption reaches new ground.
    if new_tiles_corrupted > 0 {
        activation.record(Feature::CorruptionSpread);
        log.push(
            time.tick,
            "Dark tendrils creep across the ground. The corruption spreads.".to_string(),
            NarrativeTier::Action,
        );
    }
}

// ---------------------------------------------------------------------------
// ward_decay
// ---------------------------------------------------------------------------

/// Each tick, every ward loses strength. Wards on WardPost tiles decay at half
/// speed. Wards that hit zero strength are despawned.
#[allow(clippy::too_many_arguments)]
pub fn ward_decay(
    mut wards: Query<(Entity, &mut Ward, &Position)>,
    shadow_foxes: Query<(&WildlifeAiState, &Position), With<WildAnimal>>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut commands: Commands,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
) {
    let m = &constants.magic;
    let c = &constants.wildlife;
    let mut any_decayed = false;
    for (entity, mut ward, pos) in &mut wards {
        let on_ward_post =
            map.in_bounds(pos.x, pos.y) && map.get(pos.x, pos.y).terrain == Terrain::WardPost;

        let mut effective_decay = if on_ward_post {
            ward.decay_rate * m.ward_post_decay_multiplier
        } else {
            ward.decay_rate
        };

        // Shadow foxes encircling this ward erode it in waves, not in a
        // countdown. A single fox carries most of the pressure; each extra
        // fox contributes sub-linearly (sqrt scaling) so compound siege is
        // sustained, not doubled. The colony's correct response stays the
        // same either way — drive the foxes off — instead of depending on
        // how many are present. Under the 2-fox population cap this is the
        // whole curve; the shape still generalizes if the cap lifts.
        let siege_count = shadow_foxes
            .iter()
            .filter(|(ai, _)| {
                matches!(ai, WildlifeAiState::EncirclingWard { ward_x, ward_y, .. }
                if *ward_x == pos.x && *ward_y == pos.y)
            })
            .count();
        let siege_pressure = (siege_count as f32).sqrt();
        effective_decay += siege_pressure * c.ward_siege_decay_bonus;

        ward.strength -= effective_decay;
        any_decayed = true;

        if ward.strength <= 0.0 {
            commands.entity(entity).despawn();
            activation.record(Feature::WardDespawned);
            let text = if siege_count > 0 {
                "A ward's thornbriar tangle crumbles — shadow-foxes pressed it too hard."
            } else {
                "A ward's thornbriar tangle crumbles back to dust."
            };
            log.push(time.tick, text.to_string(), NarrativeTier::Nature);
            if let Some(ref mut elog) = event_log {
                elog.push(
                    time.tick,
                    crate::resources::event_log::EventKind::WardDespawned {
                        ward_kind: format!("{:?}", ward.kind),
                        location: (pos.x, pos.y),
                        sieged: siege_count > 0,
                    },
                );
            }
        }
    }
    if any_decayed {
        activation.record(Feature::WardDecay);
    }
}

// ---------------------------------------------------------------------------
// update_ward_coverage_map
// ---------------------------------------------------------------------------

/// Rebuild the `WardCoverageMap` from current `Ward` entities each tick.
///
/// Coverage is a derived property of live wards (strength × repel
/// radius), not a memory like fox-scent — recomputing from scratch is
/// simpler than maintaining event-driven decay parity with the
/// continuous `Ward::strength` decay applied above in `ward_decay`.
/// Cost is bounded: ~10 wards × ~25-bucket stamp area = ~250 writes
/// per tick. Runs after `ward_decay` so newly-despawned wards drop out
/// of coverage on the same tick.
///
/// Consumed by ward-placement DSEs (ticket 045) for anti-clustering:
/// candidate tiles with high `ward_coverage` value are already
/// covered, so a new ward there is redundant.
pub fn update_ward_coverage_map(
    wards: Query<(&Ward, &Position)>,
    mut coverage: ResMut<crate::resources::WardCoverageMap>,
) {
    coverage.clear();
    for (ward, pos) in &wards {
        if ward.inverted {
            continue;
        }
        coverage.stamp_ward(pos.x, pos.y, ward.strength, ward.repel_radius());
    }
}

// ---------------------------------------------------------------------------
// apply_remedy_effects
// ---------------------------------------------------------------------------

/// Tick active remedy buffs. Healing and energy tonics apply each tick; mood
/// tonic pushes a single modifier on the first tick only.
pub fn apply_remedy_effects(
    mut query: Query<(
        Entity,
        &mut RemedyEffect,
        &mut Health,
        &mut Needs,
        &mut Mood,
    )>,
    mut commands: Commands,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
) {
    let m = &constants.magic;
    let healing_poultice_rate = m.healing_poultice_rate.per_tick(&time_scale);
    let energy_tonic_rate = m.energy_tonic_rate.per_tick(&time_scale);
    let mood_tonic_ticks = m.mood_tonic_duration.ticks(&time_scale);
    for (entity, mut remedy, mut health, mut needs, mut mood) in &mut query {
        activation.record(Feature::RemedyApplied);
        match remedy.kind {
            RemedyKind::HealingPoultice => {
                health.current = (health.current + healing_poultice_rate).min(health.max);
            }
            RemedyKind::EnergyTonic => {
                needs.energy = (needs.energy + energy_tonic_rate).min(1.0);
            }
            RemedyKind::MoodTonic => {
                // Only on the first tick of application.
                if remedy.ticks_remaining == remedy.kind.duration() {
                    mood.modifiers.push_back(
                        MoodModifier::new(m.mood_tonic_bonus, mood_tonic_ticks, "herbal remedy")
                            .with_kind(MoodSource::Physical),
                    );
                }
            }
        }

        remedy.ticks_remaining = remedy.ticks_remaining.saturating_sub(1);
        if remedy.ticks_remaining == 0 {
            commands.entity(entity).remove::<RemedyEffect>();
        }
    }
}

// ---------------------------------------------------------------------------
// personal_corruption_effects
// ---------------------------------------------------------------------------

/// High personal corruption causes mood drops and erratic behaviour.
pub fn personal_corruption_effects(
    mut cats: Query<(&Corruption, &mut Mood, Option<&mut CurrentAction>)>,
    _relationships: Res<Relationships>,
    mut rng: ResMut<SimRng>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
) {
    let m = &constants.magic;
    let personal_corruption_mood_ticks = m.personal_corruption_mood_duration.ticks(&time_scale);
    // TODO: corruption > 0.5 should also decay fondness toward all known cats
    // in Relationships. This requires mutable access to Relationships plus
    // multi-entity iteration (needs to know *which* cats this cat knows),
    // which conflicts with the current borrow structure. Defer to a dedicated
    // system or event-based approach.

    for (corruption, mut mood, current_action) in &mut cats {
        if corruption.0 > m.personal_corruption_mood_threshold
            && rng.rng.random::<f32>() < m.personal_corruption_mood_chance
        {
            activation.record(Feature::PersonalCorruptionEffect);
            mood.modifiers.push_back(
                MoodModifier::new(
                    m.personal_corruption_mood_penalty,
                    personal_corruption_mood_ticks,
                    "corruption",
                )
                .with_kind(MoodSource::Magic),
            );
        }

        if corruption.0 > m.personal_corruption_erratic_threshold
            && rng.rng.random::<f32>() < m.personal_corruption_erratic_chance
        {
            if let Some(mut action) = current_action {
                action.ticks_remaining = 0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// corruption_tile_effects
// ---------------------------------------------------------------------------

/// Cats standing on corrupted ground receive a mood penalty and (at high
/// corruption) health drain. Herbs on heavily corrupted tiles become twisted.
pub fn corruption_tile_effects(
    mut cats: Query<(&Position, &mut Mood, &mut Health), Without<Dead>>,
    map: Res<TileMap>,
    mut herbs: Query<(Entity, &Position, &mut Herb, Option<&Harvestable>)>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
    mut activation: ResMut<SystemActivation>,
    mut commands: Commands,
) {
    let m = &constants.magic;
    let corruption_health_drain = m.corruption_health_drain.per_tick(&time_scale);
    let corruption_tile_mood_ticks = m.corruption_tile_mood_duration.ticks(&time_scale);
    for (pos, mut mood, mut health) in &mut cats {
        if !map.in_bounds(pos.x, pos.y) {
            continue;
        }
        let corruption = map.get(pos.x, pos.y).corruption;
        if corruption > m.corruption_tile_mood_threshold {
            let already_has = mood
                .modifiers
                .iter()
                .any(|md| md.source == "corrupted ground");
            if !already_has {
                activation.record(Feature::CorruptionTileEffect);
                mood.modifiers.push_back(
                    MoodModifier::new(
                        -m.corruption_tile_mood_threshold * corruption,
                        corruption_tile_mood_ticks,
                        "corrupted ground",
                    )
                    .with_kind(MoodSource::Magic),
                );
            }
        }
        // Health drain on heavily corrupted tiles.
        if corruption > m.corruption_health_drain_threshold {
            health.current = (health.current - corruption_health_drain).max(0.0);
            activation.record(Feature::CorruptionHealthDrain);
        }
    }

    for (entity, pos, mut herb, harvestable) in &mut herbs {
        if !map.in_bounds(pos.x, pos.y) {
            continue;
        }
        let corruption = map.get(pos.x, pos.y).corruption;
        if corruption > m.corruption_twisted_herb_threshold {
            herb.twisted = true;
        }
        // High corruption suppresses harvestability entirely.
        if corruption > m.herb_suppression_threshold && harvestable.is_some() {
            commands.entity(entity).remove::<Harvestable>();
            activation.record(Feature::HerbSuppressed);
        }
    }
}

// ---------------------------------------------------------------------------
// update_corrupted_tile_markers system (§4.2 OnCorruptedTile)
// ---------------------------------------------------------------------------

/// Author the `OnCorruptedTile` ZST on living cats whose current tile has
/// `corruption > corrupted_tile_threshold`; remove it otherwise.
///
/// **Predicate** — `tile.corruption > constants.disposition
/// .corrupted_tile_threshold`. Bit-for-bit mirror of the inline
/// `on_corrupted_tile` computations in
/// `goap.rs::evaluate_and_plan` and
/// `disposition.rs::evaluate_dispositions`. The threshold lives in
/// `DispositionConstants` (not `MagicConstants`) for historical
/// reasons — both inline call sites read it from the same field, and
/// this author is consistent.
///
/// **Out-of-bounds positions** — if the cat's `Position` is outside
/// the map (which shouldn't happen in practice but the inline
/// predicates handle it) the predicate evaluates to `false` and the
/// marker is removed if present.
///
/// **Ordering** — registered in Chain 2a alongside other §4 marker
/// authors, before the GOAP scoring pipeline runs, so the
/// `MarkerSnapshot` population in `evaluate_dispositions` and
/// `evaluate_and_plan` sees the freshly-authored ZST.
///
/// **Lifecycle** — `Dead` cats are filtered out so no marker is
/// authored on corpses during the narrative grace-period window
/// before `cleanup_dead`.
///
/// **Non-goal:** this author does **not** unblock the Cleanse
/// dormancy — magic_cleanse needs the cat to *path to* a corrupted
/// tile, which is a spatial-routing problem documented in
/// `docs/open-work/tickets/014-phase-4-follow-ons.md` lines
/// 124–136. Wiring the marker pays down the §4 catalog without
/// promising a Cleanse fix.
pub fn update_corrupted_tile_markers(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            &Position,
            Has<crate::components::markers::OnCorruptedTile>,
        ),
        Without<Dead>,
    >,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
) {
    use crate::components::markers::OnCorruptedTile;
    let threshold = constants.disposition.corrupted_tile_threshold;
    for (entity, pos, has_marker) in cats.iter() {
        let on_corrupted =
            map.in_bounds(pos.x, pos.y) && map.get(pos.x, pos.y).corruption > threshold;
        match (on_corrupted, has_marker) {
            (true, false) => {
                commands.entity(entity).insert(OnCorruptedTile);
            }
            (false, true) => {
                commands.entity(entity).remove::<OnCorruptedTile>();
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// update_corruption_landmarks — territory centroid cache for §L2.10.7
// ---------------------------------------------------------------------------

/// Recompute the per-tick intensity-weighted centroid of corruption
/// across the colony map. Read by `ColonyCleanseDse` through
/// [`crate::ai::considerations::LandmarkAnchor::TerritoryCorruptionCentroid`]
/// — caching avoids re-scanning the 120×90 grid 50× per scoring tick.
///
/// Ordering — runs after `corruption_spread` / `corruption_tile_effects`
/// so the centroid reflects this tick's post-spread state, with at most
/// one tick of lag for AI consumers reading it next frame.
pub fn update_corruption_landmarks(
    map: Res<TileMap>,
    mut landmarks: ResMut<crate::resources::CorruptionLandmarks>,
) {
    let width = map.width;
    let height = map.height;
    landmarks.recompute(width, height, |x, y| map.get(x, y).corruption);
}

// ---------------------------------------------------------------------------
// herb_seasonal_check
// ---------------------------------------------------------------------------

/// On season transitions, add or remove the `Harvestable` marker on herbs
/// depending on whether the current season is in the herb's available list.
pub fn herb_seasonal_check(
    mut query: Query<(Entity, &mut Herb, &Seasonal, Option<&Harvestable>)>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
) {
    if !time.tick.is_multiple_of(config.ticks_per_season) {
        return;
    }

    let current_season = Season::from_tick(time.tick, &config);

    for (entity, mut herb, seasonal, harvestable) in &mut query {
        let in_season = seasonal.available.contains(&current_season);

        if in_season && !herb.twisted && harvestable.is_none() {
            activation.record(Feature::HerbSeasonalCheck);
            commands.entity(entity).insert(Harvestable);
        } else if !in_season && harvestable.is_some() {
            activation.record(Feature::HerbSeasonalCheck);
            commands.entity(entity).remove::<Harvestable>();
            // Reset visual growth stage when season ends.
            herb.growth_stage = GrowthStage::Sprout;
        }
    }
}

// ---------------------------------------------------------------------------
// advance_herb_growth
// ---------------------------------------------------------------------------

/// Every `herb_growth_cadence` firing, advance the growth stage of in-season herbs.
/// Plants start as Sprout and grow toward Blossom while their season is active.
pub fn advance_herb_growth(
    mut herbs: Query<&mut Herb, With<Harvestable>>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let cadence = constants.magic.herb_growth_cadence;
    if !cadence.fires_at(time.tick, &time_scale) {
        return;
    }

    for mut herb in &mut herbs {
        if let Some(next) = herb.growth_stage.next() {
            herb.growth_stage = next;
            activation.record(Feature::HerbSeasonalCheck);
        }
    }
}

// ---------------------------------------------------------------------------
// update_herb_location_map
// ---------------------------------------------------------------------------

/// Re-stamp the [`HerbLocationMap`](crate::resources::HerbLocationMap)
/// from live `Harvestable` herb entities.
///
/// §5.6.3 row #8 — sight × neutral. Each plant paints a per-kind
/// linear-falloff disc of radius
/// `constants.influence_maps.herb_location_sense_range` weighted by
/// growth stage (`Sprout=0.25` → `Blossom=1.0`). Twisted herbs are
/// skipped — they aren't valid harvest targets and their spatial
/// signature shouldn't pull cats in.
///
/// Re-stamped every tick (matches the four ticket-006 colony-faction
/// maps). The "event-driven on harvest" propagation called for in
/// §5.6.3 falls out for free: `gather_herb` despawns the herb entity
/// and the next tick's re-stamp picks up the absence.
pub fn update_herb_location_map(
    herbs: Query<(&Herb, &Position), With<Harvestable>>,
    mut map: ResMut<crate::resources::HerbLocationMap>,
    constants: Res<SimConstants>,
) {
    let sense_range = constants.influence_maps.herb_location_sense_range;
    map.clear();
    for (herb, pos) in &herbs {
        if herb.twisted {
            continue;
        }
        let strength = crate::resources::growth_stage_strength(herb.growth_stage);
        map.stamp(herb.kind, pos.x, pos.y, strength, sense_range);
    }
}

// ---------------------------------------------------------------------------
// advance_flavor_growth
// ---------------------------------------------------------------------------

/// Advance growth stage for seasonal flavor plants (Sunflower, Rose).
/// Rock decorations have no Seasonal component and are skipped automatically.
pub fn advance_flavor_growth(
    mut plants: Query<&mut FlavorPlant, With<Seasonal>>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    constants: Res<SimConstants>,
) {
    let cadence = constants.magic.herb_growth_cadence;
    if !cadence.fires_at(time.tick, &time_scale) {
        return;
    }

    for mut plant in &mut plants {
        if plant.kind.is_seasonal() {
            if let Some(next) = plant.growth_stage.next() {
                plant.growth_stage = next;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// herb_regrowth — periodically respawn depleted herbs
// ---------------------------------------------------------------------------

/// Every `herb_regrowth_cadence` firing, check Thornbriar population and
/// attempt to spawn a replacement on a random eligible tile if below cap.
/// Prevents permanent thornbriar depletion from making wards impossible.
#[allow(clippy::too_many_arguments)]
pub fn herb_regrowth(
    herbs: Query<&Herb>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
) {
    use crate::components::magic::HerbKind;

    let m = &constants.magic;
    if !m.herb_regrowth_cadence.fires_at(time.tick, &time_scale) {
        return;
    }

    let current_season = time.season(&config);
    let thornbriar_count = herbs
        .iter()
        .filter(|h| h.kind == HerbKind::Thornbriar)
        .count() as u32;
    if thornbriar_count >= m.thornbriar_regrowth_cap {
        return;
    }

    if rng.rng.random::<f32>() >= m.herb_regrowth_chance {
        return;
    }

    // Collect eligible tiles: forest terrain, forest-edge for thornbriar.
    let mut candidates: Vec<(i32, i32)> = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let terrain = map.get(x, y).terrain;
            if !HerbKind::Thornbriar.spawn_terrains().contains(&terrain) {
                continue;
            }
            if !crate::world_gen::herbs::is_forest_edge(x, y, &map) {
                continue;
            }
            candidates.push((x, y));
        }
    }

    if candidates.is_empty() {
        return;
    }

    let idx = rng.rng.random_range(0..candidates.len());
    let (x, y) = candidates[idx];
    let available = HerbKind::Thornbriar.available_seasons().to_vec();
    let in_season = available.contains(&current_season);

    let mut ec = commands.spawn((
        Herb {
            kind: HerbKind::Thornbriar,
            growth_stage: GrowthStage::Sprout,
            magical: map.get(x, y).mystery > 0.5,
            twisted: false,
        },
        Position::new(x, y),
        Seasonal { available },
    ));
    if in_season {
        ec.insert(Harvestable);
    }
    activation.record(Feature::HerbSeasonalCheck);
}

// ---------------------------------------------------------------------------
// spawn_shadow_fox_from_corruption
// ---------------------------------------------------------------------------

/// Heavily corrupted tiles (> 0.7) may spontaneously spawn shadow-foxes, up to
/// a population cap of 2.
#[allow(clippy::too_many_arguments)]
pub fn spawn_shadow_fox_from_corruption(
    map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
    wildlife: Query<&WildAnimal>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    mut commands: Commands,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut event_log: Option<ResMut<crate::resources::event_log::EventLog>>,
) {
    let m = &constants.magic;
    if !m.shadow_fox_spawn_cadence.fires_at(time.tick, &time_scale) {
        return;
    }

    let shadow_fox_count = wildlife
        .iter()
        .filter(|a| a.species == WildSpecies::ShadowFox)
        .count();

    if shadow_fox_count >= m.shadow_fox_population_cap {
        return;
    }

    for y in 0..map.height {
        for x in 0..map.width {
            if map.get(x, y).corruption > m.shadow_fox_corruption_threshold
                && rng.rng.random::<f32>() < m.shadow_fox_spawn_chance
            {
                activation.record(Feature::ShadowFoxSpawn);
                let corruption_at_spawn = map.get(x, y).corruption;
                commands.spawn((
                    WildAnimal::new(WildSpecies::ShadowFox),
                    Position::new(x, y),
                    WildlifeAiState::Patrolling { dx: 1, dy: 0 },
                    Health {
                        current: 1.0,
                        max: 1.0,
                        injuries: Vec::new(),
                        total_starvation_damage: 0.0,
                    },
                    crate::components::SensorySpecies::Wild(WildSpecies::ShadowFox),
                    crate::components::SensorySignature::WILDLIFE,
                ));
                if let Some(ref mut elog) = event_log {
                    elog.push(
                        time.tick,
                        crate::resources::event_log::EventKind::ShadowFoxSpawn {
                            location: (x, y),
                            corruption: corruption_at_spawn,
                        },
                    );
                }
                // Cap at 1 spawn per check.
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// resolve_magic_task_chains
// ---------------------------------------------------------------------------

/// Ticks magic-related TaskChain steps (GatherHerb, PrepareRemedy, ApplyRemedy,
/// SetWard, Scry, CleanseCorruption, SpiritCommunion).
///
/// Runs after `resolve_task_chains` in the schedule. Handles disjoint StepKind
/// variants so the two systems don't conflict.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_magic_task_chains(
    mut cats: Query<
        (
            Entity,
            &mut TaskChain,
            &mut CurrentAction,
            &mut Position,
            &mut Skills,
            &mut Inventory,
            &mut Mood,
            &mut Memory,
            &MagicAffinity,
            &mut Corruption,
            &mut Health,
            &Name,
        ),
        (Without<Dead>, Without<Herb>),
    >,
    herb_entities: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    alive_check: Query<(), Without<Dead>>,
    mut map: ResMut<TileMap>,
    mut rng: ResMut<SimRng>,
    mut log: ResMut<NarrativeLog>,
    mut relationships: ResMut<Relationships>,
    time: Res<TimeState>,
    time_scale: Res<TimeScale>,
    mut commands: Commands,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
) {
    let m = &constants.magic;
    // Collect workshop positions for speed bonus detection.
    // (We can't query buildings here due to borrow conflicts, so we check terrain.)
    let workshop_tiles_exist = true; // simplified — workshop bonus checked via terrain

    let mut chains_to_remove: Vec<Entity> = Vec::new();
    // Deferred fondness changes for gratitude mechanic.
    let mut gratitude: Vec<(Entity, Entity, f32)> = Vec::new();

    for (
        cat_entity,
        mut chain,
        mut current,
        mut pos,
        mut skills,
        mut inventory,
        mut mood,
        mut memory,
        magic_aff,
        mut corruption,
        mut health,
        name,
    ) in &mut cats
    {
        let Some(step) = chain.current_mut() else {
            chains_to_remove.push(cat_entity);
            current.ticks_remaining = 0;
            continue;
        };

        // Only handle magic steps.
        let is_magic_step = matches!(
            step.kind,
            StepKind::GatherHerb
                | StepKind::PrepareRemedy { .. }
                | StepKind::ApplyRemedy { .. }
                | StepKind::SetWard { .. }
                | StepKind::Scry
                | StepKind::CleanseCorruption
                | StepKind::SpiritCommunion
        );
        if !is_magic_step {
            continue;
        }

        // Ensure step is in progress.
        if matches!(step.status, StepStatus::Pending) {
            step.status = StepStatus::InProgress { ticks_elapsed: 0 };
        }

        let ticks = match &mut step.status {
            StepStatus::InProgress { ticks_elapsed } => {
                *ticks_elapsed += 1;
                *ticks_elapsed
            }
            _ => continue,
        };

        // Workshop proximity check via terrain.
        let at_workshop =
            map.in_bounds(pos.x, pos.y) && map.get(pos.x, pos.y).terrain == Terrain::Workshop;
        let _ = workshop_tiles_exist;

        // Extract step data before the match to avoid borrow conflicts
        // between step (borrowed from chain) and chain mutations.
        let step_target_entity = step.target_entity;
        let step_target_position = step.target_position;

        use crate::steps::StepResult;
        let apply = |result: StepResult, chain: &mut TaskChain| match result {
            StepResult::Continue => {}
            StepResult::Advance => {
                chain.advance();
            }
            StepResult::Fail(reason) => {
                chain.fail_current(reason);
            }
        };

        match step.kind.clone() {
            StepKind::GatherHerb => {
                apply(
                    crate::steps::magic::resolve_gather_herb(
                        ticks,
                        step_target_entity,
                        &mut inventory,
                        &mut skills,
                        &herb_entities,
                        &mut commands,
                        m,
                        &time_scale,
                    ),
                    &mut chain,
                );
            }

            StepKind::PrepareRemedy { remedy } => {
                apply(
                    crate::steps::magic::resolve_prepare_remedy(
                        ticks,
                        remedy,
                        at_workshop,
                        &mut inventory,
                        &mut skills,
                        m,
                        &time_scale,
                    ),
                    &mut chain,
                );
            }

            StepKind::ApplyRemedy { remedy } => {
                let patient_alive = step_target_entity
                    .map(|e| alive_check.get(e).is_ok())
                    .unwrap_or(false);
                let cached = &mut step.cached_path;
                let (result, grat) = crate::steps::magic::resolve_apply_remedy(
                    remedy,
                    cat_entity,
                    step_target_position,
                    step_target_entity,
                    patient_alive,
                    cached,
                    &mut pos,
                    &mut skills,
                    &map,
                    &mut commands,
                    &mut log,
                    time.tick,
                    m,
                );
                if let Some(g) = grat {
                    gratitude.push(g);
                }
                apply(result, &mut chain);
            }

            StepKind::SetWard { kind } => {
                apply(
                    crate::steps::magic::resolve_set_ward(
                        ticks,
                        kind,
                        &name.0,
                        &mut inventory,
                        magic_aff,
                        &mut skills,
                        &mut mood,
                        &mut corruption,
                        &mut health,
                        &pos,
                        &mut rng.rng,
                        &mut commands,
                        &mut log,
                        None,
                        time.tick,
                        m,
                        &constants.combat,
                        &time_scale,
                    ),
                    &mut chain,
                );
            }

            StepKind::Scry => {
                apply(
                    crate::steps::magic::resolve_scry(
                        ticks,
                        &name.0,
                        magic_aff,
                        &mut skills,
                        &mut memory,
                        &mut mood,
                        &mut corruption,
                        &mut health,
                        &pos,
                        &map,
                        &mut rng.rng,
                        &mut commands,
                        &mut log,
                        time.tick,
                        m,
                        &constants.combat,
                        &time_scale,
                    ),
                    &mut chain,
                );
            }

            StepKind::CleanseCorruption => {
                apply(
                    crate::steps::magic::resolve_cleanse_corruption(
                        ticks,
                        &name.0,
                        magic_aff,
                        &mut skills,
                        &mut corruption,
                        &mut mood,
                        &mut health,
                        &pos,
                        &mut map,
                        &mut rng.rng,
                        &mut commands,
                        &mut log,
                        time.tick,
                        m,
                        &constants.combat,
                        &time_scale,
                    ),
                    &mut chain,
                );
            }

            StepKind::SpiritCommunion => {
                apply(
                    crate::steps::magic::resolve_spirit_communion(
                        ticks,
                        &name.0,
                        magic_aff,
                        &mut skills,
                        &mut mood,
                        &mut corruption,
                        &mut health,
                        &pos,
                        &mut rng.rng,
                        &mut commands,
                        &mut log,
                        time.tick,
                        &mut activation,
                        m,
                        &constants.combat,
                        &time_scale,
                    ),
                    &mut chain,
                );
            }

            // Non-magic steps handled by resolve_task_chains.
            _ => {}
        }

        // Sync CurrentAction targets from whatever step is now active.
        chain.sync_targets(&mut current);

        if chain.is_complete() {
            chains_to_remove.push(cat_entity);
            current.ticks_remaining = 0;
        }
    }

    for entity in chains_to_remove {
        commands.entity(entity).remove::<TaskChain>();
    }

    // Apply deferred gratitude fondness changes.
    for (healed, healer, amount) in gratitude {
        relationships.modify_fondness(healed, healer, amount);
    }
}

/// Apply a misfire effect to the caster.
#[allow(clippy::too_many_arguments)]
pub fn apply_misfire(
    effect: MisfireEffect,
    cat_name: &str,
    mood: &mut Mood,
    corruption: &mut Corruption,
    health: &mut Health,
    pos: &Position,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    tick: u64,
    m: &MagicConstants,
    combat: &crate::resources::sim_constants::CombatConstants,
    time_scale: &TimeScale,
) {
    match effect {
        MisfireEffect::Fizzle => {
            mood.modifiers.push_back(
                MoodModifier::new(
                    m.misfire_fizzle_mood_penalty,
                    m.misfire_fizzle_mood_duration.ticks(time_scale),
                    "embarrassment",
                )
                .with_kind(MoodSource::Pride),
            );
            log.push(
                tick,
                format!("{cat_name} concentrates... and nothing happens."),
                NarrativeTier::Significant,
            );
        }
        MisfireEffect::CorruptionBacksplash => {
            corruption.0 = (corruption.0 + m.misfire_corruption_backsplash_amount).min(1.0);
            log.push(
                tick,
                format!("Dark energy surges back into {cat_name}!"),
                NarrativeTier::Significant,
            );
        }
        MisfireEffect::InvertedWard => {
            // Spawned by the caller — just log here.
            log.push(
                tick,
                "The ward twists, its light turning sickly...".to_string(),
                NarrativeTier::Significant,
            );
        }
        MisfireEffect::WoundTransfer => {
            // Minor wound: apply_injury handles HP penalty + injury record.
            // Use the negligible threshold + epsilon so a Minor injury is
            // always created regardless of the combat damage thresholds.
            let synthetic_damage = combat.injury_negligible_threshold + 0.001;
            crate::systems::combat::apply_injury(
                health,
                synthetic_damage,
                tick,
                crate::components::physical::InjurySource::MagicMisfire,
                *pos,
                combat,
            );
            log.push(
                tick,
                format!("{cat_name} gasps as a wound appears on their own flank."),
                NarrativeTier::Significant,
            );
        }
        MisfireEffect::LocationReveal => {
            // Create a MagicEvent memory that wildlife systems can check.
            log.push(
                tick,
                "Something in the darkness turns its head...".to_string(),
                NarrativeTier::Significant,
            );
            // The inverted ward spawned at the caster's location acts as a beacon.
            commands.spawn((
                Ward::inverted_at(WardKind::Thornward),
                Position::new(pos.x, pos.y),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// check_misfire (helper, not a system)
// ---------------------------------------------------------------------------

/// Determine whether a magic attempt misfires, based on the gap between
/// affinity and skill. Returns `None` when the attempt succeeds cleanly.
pub fn check_misfire(
    affinity: f32,
    skill: f32,
    rng: &mut impl Rng,
    m: &MagicConstants,
) -> Option<MisfireEffect> {
    if skill >= affinity * m.misfire_skill_safe_ratio {
        return None;
    }
    let chance = (affinity - skill) * m.misfire_chance_scale;
    if rng.random::<f32>() >= chance {
        return None;
    }
    let roll: f32 = rng.random();
    Some(match roll {
        r if r < m.misfire_fizzle_threshold => MisfireEffect::Fizzle,
        r if r < m.misfire_corruption_backsplash_threshold => MisfireEffect::CorruptionBacksplash,
        r if r < m.misfire_inverted_ward_threshold => MisfireEffect::InvertedWard,
        r if r < m.misfire_wound_transfer_threshold => MisfireEffect::WoundTransfer,
        _ => MisfireEffect::LocationReveal,
    })
}

// ---------------------------------------------------------------------------
// apply_corruption_pushback — positive colony events reduce local corruption
// ---------------------------------------------------------------------------

pub fn apply_corruption_pushback(
    mut messages: MessageReader<CorruptionPushback>,
    mut map: ResMut<TileMap>,
    mut activation: ResMut<SystemActivation>,
) {
    for msg in messages.read() {
        activation.record(Feature::CorruptionPushback);
        for dy in -msg.radius..=msg.radius {
            for dx in -msg.radius..=msg.radius {
                if dx.abs() + dy.abs() > msg.radius {
                    continue; // Manhattan distance
                }
                let tx = msg.position.x + dx;
                let ty = msg.position.y + dy;
                if map.in_bounds(tx, ty) {
                    let tile = map.get_mut(tx, ty);
                    tile.corruption = (tile.corruption - msg.amount).max(0.0);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::time::SimSpeed;
    use bevy_ecs::schedule::Schedule;

    fn test_time_scale() -> TimeScale {
        TimeScale::from_config(&SimConfig::default(), 16.6667)
    }

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(TimeState {
            tick: 0,
            paused: false,
            speed: SimSpeed::Normal,
        });
        world.insert_resource(SimConfig::default());
        world.insert_resource(test_time_scale());
        world.insert_resource(TileMap::new(10, 10, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(Relationships::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(SystemActivation::default());
        world
    }

    // -----------------------------------------------------------------------
    // check_misfire
    // -----------------------------------------------------------------------

    #[test]
    fn misfire_no_misfire_when_skilled() {
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let m = &SimConstants::default().magic;
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        // skill (0.5) >= affinity (0.5) * 0.8 = 0.4 → always None
        for _ in 0..100 {
            assert!(check_misfire(0.5, 0.5, &mut rng, m).is_none());
        }
    }

    #[test]
    fn misfire_high_chance_when_unskilled() {
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let m = &SimConstants::default().magic;
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut some_count = 0;
        for _ in 0..200 {
            if check_misfire(0.9, 0.1, &mut rng, m).is_some() {
                some_count += 1;
            }
        }
        // chance = (0.9 - 0.1) * 0.5 = 0.4 → ~40% should misfire
        assert!(
            some_count > 40,
            "expected many misfires with unskilled cat, got {some_count}/200"
        );
    }

    // -----------------------------------------------------------------------
    // ward_decay
    // -----------------------------------------------------------------------

    #[test]
    fn ward_decay_removes_at_zero() {
        let mut world = test_world();

        let ward_entity = world
            .spawn((
                Ward {
                    kind: crate::components::magic::WardKind::Thornward,
                    strength: 0.01,
                    decay_rate: 0.02,
                    inverted: false,
                },
                Position::new(0, 0),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(ward_decay);
        schedule.run(&mut world);

        // After one tick: strength = 0.01 - 0.02 = -0.01 → despawned
        assert!(
            world.get_entity(ward_entity).is_err(),
            "ward should be despawned after strength drops to zero"
        );
    }

    // -----------------------------------------------------------------------
    // corruption_spread
    // -----------------------------------------------------------------------

    #[test]
    fn corruption_spreads_to_adjacent() {
        let mut world = test_world();

        // Set tick to a multiple of 10 so the system runs.
        world.resource_mut::<TimeState>().tick = 10;

        // Set one tile to corruption 0.5 (above 0.3 threshold).
        world.resource_mut::<TileMap>().get_mut(5, 5).corruption = 0.5;

        let mut schedule = Schedule::default();
        schedule.add_systems(corruption_spread);
        schedule.run(&mut world);

        let map = world.resource::<TileMap>();
        let expected_spread = 0.5 * 0.0001;

        // 4-adjacent tiles should have gained corruption.
        for (nx, ny) in [(5, 4), (5, 6), (4, 5), (6, 5)] {
            let c = map.get(nx, ny).corruption;
            assert!(
                (c - expected_spread).abs() < 1e-6,
                "tile ({nx},{ny}) should have corruption {expected_spread}, got {c}"
            );
        }

        // Diagonal should be unaffected.
        let diag = map.get(6, 6).corruption;
        assert!(
            diag.abs() < 1e-6,
            "diagonal tile should be unaffected, got {diag}"
        );
    }

    // -----------------------------------------------------------------------
    // apply_remedy_effects
    // -----------------------------------------------------------------------

    #[test]
    fn remedy_heals_over_time() {
        let mut world = test_world();

        let cat = world
            .spawn((
                Health {
                    current: 0.5,
                    max: 1.0,
                    injuries: Vec::new(),
                    total_starvation_damage: 0.0,
                },
                Needs::default(),
                Mood::default(),
                RemedyEffect {
                    kind: RemedyKind::HealingPoultice,
                    ticks_remaining: RemedyKind::HealingPoultice.duration(),
                    healer: None,
                },
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_remedy_effects);
        schedule.run(&mut world);

        let health = world.get::<Health>(cat).unwrap();
        assert!(
            (health.current - 0.508).abs() < 1e-6,
            "health should increase by healing_poultice_rate per tick, got {}",
            health.current
        );
    }

    // --- is_ward_strength_low ---

    #[test]
    fn no_wards_is_low() {
        assert!(is_ward_strength_low(std::iter::empty(), 0.3));
    }

    #[test]
    fn strong_wards_not_low() {
        let ward = Ward {
            kind: WardKind::Thornward,
            strength: 0.8,
            decay_rate: 0.001,
            inverted: false,
        };
        assert!(!is_ward_strength_low(std::iter::once(&ward), 0.3));
    }

    #[test]
    fn weak_wards_is_low() {
        let ward = Ward {
            kind: WardKind::Thornward,
            strength: 0.1,
            decay_rate: 0.001,
            inverted: false,
        };
        assert!(is_ward_strength_low(std::iter::once(&ward), 0.3));
    }

    #[test]
    fn mixed_wards_average_above_threshold() {
        let strong = Ward {
            kind: WardKind::Thornward,
            strength: 0.5,
            decay_rate: 0.001,
            inverted: false,
        };
        let weak = Ward {
            kind: WardKind::Thornward,
            strength: 0.2,
            decay_rate: 0.001,
            inverted: false,
        };
        // avg = 0.35, threshold 0.3 → NOT low
        assert!(!is_ward_strength_low([&strong, &weak].into_iter(), 0.3));
    }

    // --- is_thornbriar_available (Ticket 014 Magic colony batch) ---

    fn herb(kind: crate::components::magic::HerbKind) -> Herb {
        Herb {
            kind,
            growth_stage: GrowthStage::Blossom,
            magical: false,
            twisted: false,
        }
    }

    #[test]
    fn no_herbs_thornbriar_unavailable() {
        assert!(!is_thornbriar_available(std::iter::empty()));
    }

    #[test]
    fn only_other_herbs_thornbriar_unavailable() {
        let h = herb(crate::components::magic::HerbKind::HealingMoss);
        assert!(!is_thornbriar_available(std::iter::once(&h)));
    }

    #[test]
    fn one_thornbriar_makes_available() {
        let h = herb(crate::components::magic::HerbKind::Thornbriar);
        assert!(is_thornbriar_available(std::iter::once(&h)));
    }

    #[test]
    fn mixed_herbs_with_thornbriar_available() {
        let moss = herb(crate::components::magic::HerbKind::HealingMoss);
        let briar = herb(crate::components::magic::HerbKind::Thornbriar);
        assert!(is_thornbriar_available([&moss, &briar].into_iter()));
    }

    // --- is_any_ward_under_siege (Ticket 014 Magic colony batch) ---

    #[test]
    fn no_wildlife_no_siege() {
        assert!(!is_any_ward_under_siege(std::iter::empty()));
    }

    #[test]
    fn idle_wildlife_no_siege() {
        let s = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
        assert!(!is_any_ward_under_siege(std::iter::once(&s)));
    }

    #[test]
    fn encircling_ward_yields_siege() {
        let s = WildlifeAiState::EncirclingWard {
            ward_x: 0,
            ward_y: 0,
            angle: 0.0,
            ticks: 5,
        };
        assert!(is_any_ward_under_siege(std::iter::once(&s)));
    }

    #[test]
    fn one_encircling_among_many_yields_siege() {
        let idle = WildlifeAiState::Patrolling { dx: 1, dy: 0 };
        let encircle = WildlifeAiState::EncirclingWard {
            ward_x: 0,
            ward_y: 0,
            angle: 0.0,
            ticks: 5,
        };
        assert!(is_any_ward_under_siege([&idle, &encircle].into_iter()));
    }

    // -----------------------------------------------------------------------
    // update_corrupted_tile_markers tests (§4.2 OnCorruptedTile)
    // -----------------------------------------------------------------------

    fn corrupted_tile_setup() -> (World, Schedule, f32) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(10, 10, Terrain::Grass));
        world.insert_resource(SimConstants::default());
        let threshold = world
            .resource::<SimConstants>()
            .disposition
            .corrupted_tile_threshold;
        let mut schedule = Schedule::default();
        schedule.add_systems(update_corrupted_tile_markers);
        (world, schedule, threshold)
    }

    fn has_on_corrupted_tile(world: &World, entity: Entity) -> bool {
        world
            .get::<crate::components::markers::OnCorruptedTile>(entity)
            .is_some()
    }

    fn set_corruption(world: &mut World, x: i32, y: i32, value: f32) {
        world.resource_mut::<TileMap>().get_mut(x, y).corruption = value;
    }

    #[test]
    fn corruption_above_threshold_inserts_marker() {
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        set_corruption(&mut world, 5, 5, threshold + 0.05);
        let cat = world.spawn(Position { x: 5, y: 5 }).id();
        schedule.run(&mut world);
        assert!(
            has_on_corrupted_tile(&world, cat),
            "tile corruption above threshold should insert marker"
        );
    }

    #[test]
    fn corruption_below_threshold_no_marker() {
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        set_corruption(&mut world, 5, 5, threshold - 0.01);
        let cat = world.spawn(Position { x: 5, y: 5 }).id();
        schedule.run(&mut world);
        assert!(
            !has_on_corrupted_tile(&world, cat),
            "tile corruption below threshold should not insert marker"
        );
    }

    #[test]
    fn corruption_at_threshold_no_marker() {
        // Predicate is strict `>`, mirroring the inline computations
        // in disposition.rs and goap.rs.
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        set_corruption(&mut world, 5, 5, threshold);
        let cat = world.spawn(Position { x: 5, y: 5 }).id();
        schedule.run(&mut world);
        assert!(
            !has_on_corrupted_tile(&world, cat),
            "tile corruption equal to threshold should not insert marker (strict gt)"
        );
    }

    #[test]
    fn position_change_crosses_threshold_boundary() {
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        set_corruption(&mut world, 5, 5, threshold + 0.1);
        let cat = world.spawn(Position { x: 1, y: 1 }).id();
        schedule.run(&mut world);
        assert!(!has_on_corrupted_tile(&world, cat));

        world.get_mut::<Position>(cat).unwrap().x = 5;
        world.get_mut::<Position>(cat).unwrap().y = 5;
        schedule.run(&mut world);
        assert!(
            has_on_corrupted_tile(&world, cat),
            "moving onto a corrupted tile should insert marker"
        );

        world.get_mut::<Position>(cat).unwrap().x = 1;
        world.get_mut::<Position>(cat).unwrap().y = 1;
        schedule.run(&mut world);
        assert!(
            !has_on_corrupted_tile(&world, cat),
            "moving off a corrupted tile should remove marker"
        );
    }

    #[test]
    fn corruption_mutation_crosses_threshold_boundary() {
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        let cat = world.spawn(Position { x: 5, y: 5 }).id();
        schedule.run(&mut world);
        assert!(!has_on_corrupted_tile(&world, cat));

        set_corruption(&mut world, 5, 5, threshold + 0.2);
        schedule.run(&mut world);
        assert!(
            has_on_corrupted_tile(&world, cat),
            "rising tile corruption should insert marker"
        );

        set_corruption(&mut world, 5, 5, 0.0);
        schedule.run(&mut world);
        assert!(
            !has_on_corrupted_tile(&world, cat),
            "cleansed tile should remove marker"
        );
    }

    #[test]
    fn dead_cats_are_skipped_corrupted_tile() {
        use crate::components::physical::DeathCause;
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        set_corruption(&mut world, 5, 5, threshold + 0.1);
        let cat = world
            .spawn((
                Position { x: 5, y: 5 },
                Dead {
                    tick: 0,
                    cause: DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(
            !has_on_corrupted_tile(&world, cat),
            "dead cats should not receive marker even on heavily corrupted tile"
        );
    }

    #[test]
    fn multiple_cats_at_different_tiles_independent() {
        let (mut world, mut schedule, threshold) = corrupted_tile_setup();
        set_corruption(&mut world, 5, 5, threshold + 0.1);
        let on_corrupt = world.spawn(Position { x: 5, y: 5 }).id();
        let off_corrupt = world.spawn(Position { x: 1, y: 1 }).id();
        schedule.run(&mut world);
        assert!(has_on_corrupted_tile(&world, on_corrupt));
        assert!(!has_on_corrupted_tile(&world, off_corrupt));
    }
}
