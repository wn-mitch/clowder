use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::ai::pathfinding::step_toward;
use crate::components::building::StructureType;
use crate::components::coordination::{ActiveDirective, PendingDelivery};
use crate::components::identity::{Gender, Name, Orientation};
use crate::components::items::{Item, ItemKind, ItemLocation, quality_label};
use crate::components::mental::{Memory, MemoryEntry, MemoryType, Mood, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::components::prey::PreyAnimal;
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::time::TimeState;
use crate::systems::social::{are_orientation_compatible, value_compatibility_delta};

// ---------------------------------------------------------------------------
// Deferred social effects
// ---------------------------------------------------------------------------

/// Effects queued during the main iteration loop and applied after it ends.
/// This avoids the need to mutably borrow two entities simultaneously.
struct SocialDelta {
    entity: Entity,
    social_delta: f32,
    warmth_delta: f32,
}

/// Records an interaction pair for value-compatibility and romantic processing
/// after the main loop.
struct InteractionPair {
    a: Entity,
    b: Entity,
}

/// A memory to be transmitted from one cat to another via social interaction.
struct MemoryTransmission {
    receiver: Entity,
    entry: MemoryEntry,
}

/// Significance weight for memory transmission probability by event type.
fn significance_weight(event_type: MemoryType) -> f32 {
    match event_type {
        MemoryType::ThreatSeen => 0.8,
        MemoryType::Death => 0.7,
        MemoryType::MagicEvent => 0.6,
        MemoryType::ResourceFound => 0.5,
        MemoryType::Injury => 0.4,
        MemoryType::SocialEvent => 0.3,
    }
}

/// Deferred skill growth for an apprentice being mentored.
struct MentorEffect {
    apprentice: Entity,
    mentor_skills: Skills,
}

/// Check whether two optional locations are approximately the same (~5 tiles).
fn approx_location_match(a: &Option<Position>, b: &Option<Position>) -> bool {
    match (a, b) {
        (Some(pa), Some(pb)) => pa.manhattan_distance(pb) <= 5,
        (None, None) => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// resolve_actions system
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Forage helpers
// ---------------------------------------------------------------------------

fn pick_forage_item(terrain: &crate::resources::map::Terrain, rng: &mut SimRng) -> ItemKind {
    use crate::resources::map::Terrain;
    let roll: f32 = rng.rng.random();
    match terrain {
        Terrain::LightForest | Terrain::DenseForest => {
            if roll < 0.3 { ItemKind::Berries }
            else if roll < 0.5 { ItemKind::Nuts }
            else if roll < 0.7 { ItemKind::Mushroom }
            else if roll < 0.85 { ItemKind::Moss }
            else { ItemKind::DriedGrass }
        }
        Terrain::Grass => {
            if roll < 0.35 { ItemKind::Roots }
            else if roll < 0.55 { ItemKind::WildOnion }
            else if roll < 0.70 { ItemKind::Mushroom }
            else if roll < 0.95 { ItemKind::Berries }
            else { ItemKind::DriedGrass }
        }
        _ => ItemKind::Roots, // Fallback for other terrains
    }
}

// ---------------------------------------------------------------------------
// Main action resolution
// ---------------------------------------------------------------------------

/// - Then applies the per-tick effect for the action that is now in progress.
/// - Movement actions call `step_toward` each tick to close on the target.
/// - Hunt/Forage store food items in Stores buildings and grow skills.
/// - Socialize/Groom effects on the *target* cat are deferred and applied after
///   the main iteration loop to avoid borrow conflicts.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_actions(
    mut query: Query<(
        Entity,
        &mut CurrentAction,
        &mut Needs,
        &mut Position,
        &mut Skills,
        &mut Memory,
        &mut Mood,
        &mut crate::components::magic::Inventory,
        Option<&crate::components::aspirations::Preferences>,
        &Name,
    ), (Without<Dead>, Without<crate::components::task_chain::TaskChain>)>,
    traits_query: Query<(&Personality, &Gender, &Orientation)>,
    pending_delivery_query: Query<&PendingDelivery>,
    active_directive_query: Query<&ActiveDirective>,
    building_positions: Query<(&crate::components::building::Structure, &Position), Without<CurrentAction>>,
    prey_query: Query<(Entity, &PreyAnimal, &Position), Without<CurrentAction>>,
    mut stores_query: Query<
        (Entity, &mut crate::components::building::StoredItems, &crate::components::building::Structure, &Position),
        (Without<CurrentAction>, Without<Dead>),
    >,
    items_for_eating: Query<&Item>,
    map: Res<TileMap>,
    mut rng: ResMut<SimRng>,
    time: Res<TimeState>,
    config: Res<crate::resources::time::SimConfig>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<NarrativeLog>,
    mut commands: Commands,
) {

    let season = time.season(&config);

    // Snapshot Hearth positions for memory transmission proximity bonus.
    let hearth_positions: Vec<Position> = building_positions
        .iter()
        .filter(|(s, _)| {
            s.kind == crate::components::building::StructureType::Hearth
                && s.effectiveness() > 0.0
        })
        .map(|(_, p)| *p)
        .collect();

    let mut social_deltas: Vec<SocialDelta> = Vec::new();
    let mut interaction_pairs: Vec<InteractionPair> = Vec::new();
    let mut coordination_deliveries: Vec<(Entity, Entity)> = Vec::new();
    let mut mentor_effects: Vec<MentorEffect> = Vec::new();

    for (entity, mut current, mut needs, mut pos, mut skills, mut memory, mut mood, mut inventory, preferences, cat_name) in &mut query {
        if current.ticks_remaining == 0 {
            continue;
        }

        current.ticks_remaining -= 1;

        // Preference mood effects on action completion.
        if current.ticks_remaining == 0 {
            if let Some(prefs) = preferences {
                match prefs.get(current.action) {
                    Some(crate::components::aspirations::Preference::Like) => {
                        mood.modifiers.push_back(MoodModifier {
                            amount: 0.05,
                            ticks_remaining: 30,
                            source: format!("enjoyed {:?}", current.action),
                        });
                    }
                    Some(crate::components::aspirations::Preference::Dislike) => {
                        mood.modifiers.push_back(MoodModifier {
                            amount: -0.05,
                            ticks_remaining: 30,
                            source: "hated doing that".to_string(),
                        });
                    }
                    None => {}
                }
            }
        }

        match current.action {
            Action::Eat => {
                // Walk to the targeted Stores building, then consume one item.
                if let Some(target) = current.target_position {
                    if pos.manhattan_distance(&target) > 1 {
                        // Still walking to the store.
                        if let Some(next) = step_toward(&pos, &target, &map) {
                            *pos = next;
                        }
                    } else if let Some(store_entity) = current.target_entity {
                        // Adjacent to store — eat one item and finish.
                        let mut ate_kind: Option<ItemKind> = None;
                        if let Ok((_, mut stored, _, _)) = stores_query.get_mut(store_entity) {
                            let food_item = stored.items.iter()
                                .copied()
                                .find(|&item_e| {
                                    items_for_eating.get(item_e)
                                        .is_ok_and(|item| item.kind.is_food())
                                });
                            if let Some(item_entity) = food_item {
                                if let Ok(item) = items_for_eating.get(item_entity) {
                                    let food_val = item.kind.food_value();
                                    needs.hunger = (needs.hunger + food_val).min(1.0);
                                    ate_kind = Some(item.kind);
                                }
                                stored.remove(item_entity);
                                commands.entity(item_entity).despawn();
                            }
                        }
                        if let Some(kind) = ate_kind {
                            if rng.rng.random_range(0u32..3) == 0 {
                                let cat = &cat_name.0;
                                let item_name = kind.name();
                                log.push(
                                    time.tick,
                                    format!("{cat} eats {item_name} from the stores."),
                                    NarrativeTier::Micro,
                                );
                            }
                        }
                        // Done eating — end action regardless of whether food was found.
                        current.ticks_remaining = 0;
                    }
                }
            }
            Action::Sleep => {
                needs.energy = (needs.energy + 0.02).min(1.0);
                needs.warmth = (needs.warmth + 0.01).min(1.0);
            }
            Action::Hunt => {
                // Check if carrying prey back to stores (post-kill carry phase).
                let carrying_food = inventory.slots.iter().any(|s| {
                    matches!(s, crate::components::magic::ItemSlot::Item(k) if k.is_food())
                });

                if carrying_food {
                    // Carry phase: walk toward stores to deposit.
                    if let Some(target) = current.target_position {
                        if pos.manhattan_distance(&target) > 1 {
                            if let Some(next) = step_toward(&pos, &target, &map) {
                                *pos = next;
                            }
                        } else if let Some(store_entity) = current.target_entity {
                            // Adjacent to stores — deposit the food item.
                            let deposited = if let Some(idx) = inventory.slots.iter().position(|s| {
                                matches!(s, crate::components::magic::ItemSlot::Item(k) if k.is_food())
                            }) {
                                let kind = match &inventory.slots[idx] {
                                    crate::components::magic::ItemSlot::Item(k) => *k,
                                    _ => unreachable!(),
                                };
                                inventory.slots.swap_remove(idx);
                                let quality = (0.3 + skills.hunting * 0.4).clamp(0.0, 1.0);
                                let item_entity = commands
                                    .spawn(Item::new(kind, quality, ItemLocation::StoredIn(store_entity)))
                                    .id();
                                if let Ok((_, mut stored, _, _)) = stores_query.get_mut(store_entity) {
                                    stored.add(item_entity, StructureType::Stores);
                                }
                                let cat = &cat_name.0;
                                let text = format!("{cat} drops {} at the colony stores without a word.", kind.name());
                                log.push(time.tick, text, NarrativeTier::Action);
                                true
                            } else {
                                false
                            };

                            if deposited || current.ticks_remaining == 0 {
                                current.ticks_remaining = 0;
                            }
                        }
                    }
                    // If ticks expire before reaching stores, drop item on ground.
                    if current.ticks_remaining == 0 && carrying_food {
                        if let Some(idx) = inventory.slots.iter().position(|s| {
                            matches!(s, crate::components::magic::ItemSlot::Item(k) if k.is_food())
                        }) {
                            let kind = match &inventory.slots[idx] {
                                crate::components::magic::ItemSlot::Item(k) => *k,
                                _ => unreachable!(),
                            };
                            inventory.slots.swap_remove(idx);
                            let quality = (0.3 + skills.hunting * 0.4).clamp(0.0, 1.0);
                            commands.spawn(Item::new(kind, quality, ItemLocation::OnGround));
                        }
                    }
                } else {
                    // Hunt phase: move toward hunting ground.
                    if let Some(target) = current.target_position {
                        if pos.manhattan_distance(&target) > 1 {
                            if let Some(next) = step_toward(&pos, &target, &map) {
                                *pos = next;
                            }
                        }
                    }

                    // On last tick: resolve the hunt.
                    if current.ticks_remaining == 0 {
                        let success = rng.rng.random::<f32>() < 0.40 + skills.hunting * 0.55;
                        if success {
                            // Find nearest prey within 5 tiles.
                            let nearest_prey = prey_query.iter()
                                .filter(|(_, _, prey_pos)| pos.manhattan_distance(prey_pos) <= 5)
                                .min_by_key(|(_, _, prey_pos)| pos.manhattan_distance(prey_pos))
                                .map(|(prey_entity, prey, _)| (prey_entity, prey.species));

                            if let Some((prey_entity, species)) = nearest_prey {
                                let item_kind = species.item_kind();

                                // Despawn the prey entity.
                                commands.entity(prey_entity).despawn();

                                // Skilled hunters butcher more usable portions from a kill.
                                let item_count = if skills.hunting > 0.4 { 2 } else { 1 };
                                for _ in 0..item_count {
                                    inventory.add_item(item_kind);
                                }

                                // Remember this as a good hunting spot.
                                memory.remember(MemoryEntry {
                                    event_type: MemoryType::ResourceFound,
                                    location: current.target_position,
                                    involved: vec![],
                                    tick: time.tick,
                                    strength: 1.0,
                                    firsthand: true,
                                });
                                mood.modifiers.push_back(MoodModifier {
                                    amount: 0.1,
                                    ticks_remaining: 30,
                                    source: format!("caught a {}", species.name()),
                                });

                                // Narrative for the catch.
                                let prey_name = species.name();
                                let cat = &cat_name.0;
                                let quality = (0.3 + skills.hunting * 0.4).clamp(0.0, 1.0);
                                let text = if let Some(ql) = quality_label(quality) {
                                    format!("{cat} catches a {ql} {prey_name}.")
                                } else {
                                    format!("{cat} brings down prey with a quick snap.")
                                };
                                log.push(time.tick, text, NarrativeTier::Action);

                                // Switch to carry phase: target the nearest Stores.
                                let cat_pos = *pos;
                                let nearest_store = stores_query
                                    .iter()
                                    .filter(|(_, _, s, _)| s.kind == StructureType::Stores)
                                    .min_by_key(|(_, _, _, sp)| cat_pos.manhattan_distance(sp));

                                if let Some((se, _, _, sp)) = nearest_store {
                                    current.target_position = Some(*sp);
                                    current.target_entity = Some(se);
                                    current.ticks_remaining = 15; // time to walk to stores
                                }
                                // If no stores, item stays in inventory and drops on ground
                                // when ticks expire (handled by the carry phase above).
                            } else {
                                // Skill check passed but no prey nearby.
                                mood.modifiers.push_back(MoodModifier {
                                    amount: -0.05,
                                    ticks_remaining: 20,
                                    source: "hunt found nothing".to_string(),
                                });
                            }
                        } else {
                            mood.modifiers.push_back(MoodModifier {
                                amount: -0.05,
                                ticks_remaining: 20,
                                source: "failed hunt".to_string(),
                            });
                        }
                        // Skill growth on every attempt.
                        skills.hunting += skills.growth_rate() * 0.02;
                    }
                }
            }
            Action::Forage => {
                // Move toward foraging terrain.
                if let Some(target) = current.target_position {
                    if pos.manhattan_distance(&target) > 1 {
                        if let Some(next) = step_toward(&pos, &target, &map) {
                            *pos = next;
                        }
                    } else {
                        // At destination: check terrain yield.
                        let mut yielded = false;
                        if map.in_bounds(pos.x, pos.y) {
                            let tile = map.get(pos.x, pos.y);
                            let yield_amount = tile.terrain.foraging_yield()
                                * (0.15 + skills.foraging * 0.6)
                                * season.foraging_multiplier();
                            if yield_amount > 0.0 {
                                yielded = true;
                            }
                        }

                        // On last tick at a productive spot: spawn item, remember + mood boost.
                        if yielded && current.ticks_remaining == 0 {
                            memory.remember(MemoryEntry {
                                event_type: MemoryType::ResourceFound,
                                location: current.target_position,
                                involved: vec![],
                                tick: time.tick,
                                strength: 0.8,
                                firsthand: true,
                            });
                            mood.modifiers.push_back(MoodModifier {
                                amount: 0.05,
                                ticks_remaining: 15,
                                source: "good foraging".to_string(),
                            });

                            // Spawn a foraged item in the nearest Stores building.
                            if map.in_bounds(pos.x, pos.y) {
                                let tile = map.get(pos.x, pos.y);
                                let forage_kind = pick_forage_item(&tile.terrain, &mut rng);
                                let quality = (0.3 + skills.foraging * 0.3).clamp(0.0, 1.0);

                                let cat_pos = *pos;
                                let nearest_store = stores_query
                                    .iter()
                                    .filter(|(_, _, s, _)| s.kind == StructureType::Stores)
                                    .min_by_key(|(_, _, _, sp)| cat_pos.manhattan_distance(sp))
                                    .map(|(e, _, _, _)| e);

                                // Skilled foragers gather more from the same spot.
                                let item_count = if skills.foraging > 0.4 { 2 } else { 1 };

                                if let Some(store_entity) = nearest_store {
                                    for _ in 0..item_count {
                                        let item_entity = commands
                                            .spawn(Item::new(forage_kind, quality, ItemLocation::StoredIn(store_entity)))
                                            .id();
                                        if let Ok((_, mut stored, _, _)) = stores_query.get_mut(store_entity) {
                                            stored.add(item_entity, StructureType::Stores);
                                        }
                                    }
                                } else {
                                    for _ in 0..item_count {
                                        commands.spawn(Item::new(forage_kind, quality, ItemLocation::OnGround));
                                    }
                                }

                                // Outcome narrative: what was gathered.
                                let cat = &cat_name.0;
                                let item_name = forage_kind.name();
                                log.push(
                                    time.tick,
                                    format!("{cat} gathers {item_name} and adds them to the stores."),
                                    NarrativeTier::Micro,
                                );
                            }
                        }

                        // Skill growth each tick spent foraging.
                        skills.foraging += skills.growth_rate() * 0.01;
                    }
                }
            }
            Action::Wander => {
                if let Some(target) = current.target_position {
                    if let Some(next) = step_toward(&pos, &target, &map) {
                        *pos = next;
                    } else {
                        // Stuck or arrived — end early so AI re-evaluates.
                        current.ticks_remaining = 0;
                    }
                }
            }
            Action::Idle => {
                // Micro-drift: idle cats fidget ±1 tile occasionally.
                if rng.rng.random::<f32>() < 0.3 {
                    let dx = rng.rng.random_range(-1i32..=1);
                    let dy = rng.rng.random_range(-1i32..=1);
                    let nx = pos.x + dx;
                    let ny = pos.y + dy;
                    if map.in_bounds(nx, ny) && map.get(nx, ny).terrain.is_passable() {
                        *pos = Position::new(nx, ny);
                    }
                }
            }
            Action::Socialize => {
                if let Some(target_pos) = current.target_position {
                    if pos.manhattan_distance(&target_pos) > 1 {
                        if let Some(next) = step_toward(&pos, &target_pos, &map) {
                            *pos = next;
                        }
                    } else if let Some(target_entity) = current.target_entity {
                        // Adjacent: restore social and build relationship.
                        needs.social = (needs.social + 0.03).min(1.0);
                        social_deltas.push(SocialDelta {
                            entity: target_entity,
                            social_delta: 0.03,
                            warmth_delta: 0.0,
                        });
                        relationships.modify_fondness(entity, target_entity, 0.005);
                        relationships.modify_familiarity(entity, target_entity, 0.003);
                        relationships.get_or_insert(entity, target_entity).last_interaction = time.tick;
                        interaction_pairs.push(InteractionPair {
                            a: entity,
                            b: target_entity,
                        });
                    }
                }
            }
            Action::Groom => {
                if let Some(target_entity) = current.target_entity {
                    if let Some(target_pos) = current.target_position {
                        if pos.manhattan_distance(&target_pos) > 1 {
                            if let Some(next) = step_toward(&pos, &target_pos, &map) {
                                *pos = next;
                            }
                        } else {
                            needs.social = (needs.social + 0.02).min(1.0);
                            social_deltas.push(SocialDelta {
                                entity: target_entity,
                                social_delta: 0.02,
                                warmth_delta: 0.02,
                            });
                            relationships.modify_fondness(entity, target_entity, 0.008);
                            relationships.modify_familiarity(entity, target_entity, 0.003);
                            relationships.get_or_insert(entity, target_entity).last_interaction = time.tick;
                            interaction_pairs.push(InteractionPair {
                                a: entity,
                                b: target_entity,
                            });
                        }
                    }
                } else {
                    needs.warmth = (needs.warmth + 0.02).min(1.0);
                }
            }
            Action::Explore => {
                // Move toward distant target.
                if let Some(target) = current.target_position {
                    if pos.manhattan_distance(&target) > 1 {
                        if let Some(next) = step_toward(&pos, &target, &map) {
                            *pos = next;
                        }
                    } else {
                        // Arrived: check for interesting terrain.
                        if map.in_bounds(pos.x, pos.y) {
                            let tile = map.get(pos.x, pos.y);
                            if matches!(
                                tile.terrain,
                                crate::resources::map::Terrain::FairyRing
                                    | crate::resources::map::Terrain::StandingStone
                                    | crate::resources::map::Terrain::AncientRuin
                                    | crate::resources::map::Terrain::DeepPool
                            ) {
                                memory.remember(MemoryEntry {
                                    event_type: MemoryType::ResourceFound,
                                    location: Some(*pos),
                                    involved: vec![],
                                    tick: time.tick,
                                    strength: 0.8,
                                    firsthand: true,
                                });
                            }
                        }
                    }
                }
            }
            Action::Flee => {
                // Move away from nearest threat (target_position is the flee destination).
                if let Some(target) = current.target_position {
                    if let Some(next) = step_toward(&pos, &target, &map) {
                        *pos = next;
                    }
                }
            }
            Action::Fight => {
                // Movement toward threat handled here; damage handled by combat system.
                if let Some(target) = current.target_position {
                    if pos.manhattan_distance(&target) > 1 {
                        if let Some(next) = step_toward(&pos, &target, &map) {
                            *pos = next;
                        }
                    }
                }
            }
            Action::Patrol => {
                // Walk colony perimeter, restoring safety.
                if let Some(target) = current.target_position {
                    if pos.manhattan_distance(&target) > 1 {
                        if let Some(next) = step_toward(&pos, &target, &map) {
                            *pos = next;
                        }
                    }
                }
                needs.safety = (needs.safety + 0.005).min(1.0);
            }
            // Build, Farm, Herbcraft, and PracticeMagic are driven by the TaskChain system.
            Action::Build | Action::Farm | Action::Herbcraft | Action::PracticeMagic => {}
            Action::Mentor => {
                if let Some(target_entity) = current.target_entity {
                    if let Some(target_pos) = current.target_position {
                        if pos.manhattan_distance(&target_pos) > 1 {
                            if let Some(next) = step_toward(&pos, &target_pos, &map) {
                                *pos = next;
                            }
                        } else {
                            // Adjacent: teaching in progress.
                            needs.mastery = (needs.mastery + 0.02).min(1.0);
                            needs.social = (needs.social + 0.01).min(1.0);
                            relationships.modify_fondness(entity, target_entity, 0.005);
                            relationships.modify_familiarity(entity, target_entity, 0.003);
                            relationships.get_or_insert(entity, target_entity).last_interaction = time.tick;
                            mentor_effects.push(MentorEffect {
                                apprentice: target_entity,
                                mentor_skills: skills.clone(),
                            });
                            interaction_pairs.push(InteractionPair {
                                a: entity,
                                b: target_entity,
                            });
                        }
                    }
                }
            }
            // Coordinate: move toward target cat, deliver directive when adjacent.
            // Delivery is handled via deferred effects below.
            Action::Coordinate => {
                if let Some(target) = current.target_position {
                    if pos.manhattan_distance(&target) > 1 {
                        if let Some(next) = step_toward(&pos, &target, &map) {
                            *pos = next;
                        }
                    }
                    if pos.manhattan_distance(&target) <= 1 {
                        // Adjacent to target — delivery happens, action complete.
                        if let Some(target_entity) = current.target_entity {
                            coordination_deliveries.push((entity, target_entity));
                        }
                        current.ticks_remaining = 0;
                    }
                }
            }
        }
    }

    // Apply deferred social effects to target entities.
    for delta in &social_deltas {
        if let Ok((_, _, mut needs, _, _, _, _, _, _, _)) = query.get_mut(delta.entity) {
            needs.social = (needs.social + delta.social_delta).min(1.0);
            needs.warmth = (needs.warmth + delta.warmth_delta).min(1.0);
        }
    }

    // Apply value compatibility and romantic progression for interaction pairs.
    for pair in &interaction_pairs {
        if let (Ok((pers_a, gender_a, orient_a)), Ok((pers_b, gender_b, orient_b))) =
            (traits_query.get(pair.a), traits_query.get(pair.b))
        {
            // Value compatibility: same-side values build fondness, divergent hurt.
            let compat = value_compatibility_delta(
                pers_a.loyalty, pers_a.tradition, pers_a.compassion, pers_a.pride, pers_a.independence,
                pers_b.loyalty, pers_b.tradition, pers_b.compassion, pers_b.pride, pers_b.independence,
            );
            if compat.abs() > f32::EPSILON {
                relationships.modify_fondness(pair.a, pair.b, compat);
            }

            // Romantic progression for orientation-compatible cats.
            if are_orientation_compatible(*gender_a, *orient_a, *gender_b, *orient_b) {
                if let Some(rel) = relationships.get(pair.a, pair.b) {
                    if rel.fondness > 0.4 && rel.familiarity > 0.3 {
                        relationships.modify_romantic(pair.a, pair.b, 0.002);
                    }
                }
            }
        }
    }

    // Memory transmission: cats share memories during social interactions.
    // Collected separately from interaction_pairs to keep read/write phases distinct.
    let mut memory_transmissions: Vec<MemoryTransmission> = Vec::new();

    for pair in &interaction_pairs {
        let fondness = relationships.get(pair.a, pair.b).map_or(0.0, |r| r.fondness);

        // Check Hearth proximity for campfire-stories bonus.
        let near_hearth = if let (Ok((_, _, _, pos_a, _, _, _, _, _, _)), Ok((_, _, _, pos_b, _, _, _, _, _, _))) =
            (query.get(pair.a), query.get(pair.b))
        {
            hearth_positions.iter().any(|hp| {
                pos_a.manhattan_distance(hp) <= 3 || pos_b.manhattan_distance(hp) <= 3
            })
        } else {
            false
        };
        let hearth_mult = if near_hearth { 1.5 } else { 1.0 };

        // Read both cats' memories (immutable borrows via query.get).
        if let (Ok((_, _, _, _, _, mem_a, _, _, _, _)), Ok((_, _, _, _, _, mem_b, _, _, _, _))) =
            (query.get(pair.a), query.get(pair.b))
        {
            // A → B transmission.
            for entry in &mem_a.events {
                let prob = entry.strength
                    * (fondness + 1.0) / 2.0
                    * significance_weight(entry.event_type)
                    * hearth_mult;
                if rng.rng.random::<f32>() < prob {
                    let already_known = mem_b.events.iter().any(|e| {
                        e.event_type == entry.event_type
                            && approx_location_match(&e.location, &entry.location)
                    });
                    if !already_known {
                        memory_transmissions.push(MemoryTransmission {
                            receiver: pair.b,
                            entry: MemoryEntry {
                                event_type: entry.event_type,
                                location: entry.location,
                                involved: entry.involved.clone(),
                                tick: entry.tick,
                                strength: entry.strength * 0.5,
                                firsthand: false,
                            },
                        });
                    }
                }
            }

            // B → A transmission.
            for entry in &mem_b.events {
                let prob = entry.strength
                    * (fondness + 1.0) / 2.0
                    * significance_weight(entry.event_type)
                    * hearth_mult;
                if rng.rng.random::<f32>() < prob {
                    let already_known = mem_a.events.iter().any(|e| {
                        e.event_type == entry.event_type
                            && approx_location_match(&e.location, &entry.location)
                    });
                    if !already_known {
                        memory_transmissions.push(MemoryTransmission {
                            receiver: pair.a,
                            entry: MemoryEntry {
                                event_type: entry.event_type,
                                location: entry.location,
                                involved: entry.involved.clone(),
                                tick: entry.tick,
                                strength: entry.strength * 0.5,
                                firsthand: false,
                            },
                        });
                    }
                }
            }
        }
    }

    // Apply memory transmissions.
    for tx in memory_transmissions {
        if let Ok((_, _, _, _, _, mut mem, _, _, _, _)) = query.get_mut(tx.receiver) {
            mem.remember(tx.entry);
        }
    }

    // Apply mentor effects: grow apprentice's weakest skill that the mentor excels at.
    for effect in &mentor_effects {
        if let Ok((_, _, _, _, mut app_skills, _, _, _, _, _)) = query.get_mut(effect.apprentice) {
            let pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, app_skills.hunting),
                (effect.mentor_skills.foraging, app_skills.foraging),
                (effect.mentor_skills.herbcraft, app_skills.herbcraft),
                (effect.mentor_skills.building, app_skills.building),
                (effect.mentor_skills.combat, app_skills.combat),
                (effect.mentor_skills.magic, app_skills.magic),
            ];
            // Find the skill with the largest teachable gap (mentor > 0.6, apprentice < 0.3).
            if let Some((idx, _)) = pairs.iter().enumerate()
                .filter(|(_, (m, a))| *m > 0.6 && *a < 0.3)
                .max_by(|(_, (a_m, a_a)), (_, (b_m, b_a))| {
                    let gap_a = a_m - a_a;
                    let gap_b = b_m - b_a;
                    gap_a.partial_cmp(&gap_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| (i, ()))
            {
                let growth = app_skills.growth_rate() * 0.04; // 2x normal
                match idx {
                    0 => app_skills.hunting += growth,
                    1 => app_skills.foraging += growth,
                    2 => app_skills.herbcraft += growth,
                    3 => app_skills.building += growth,
                    4 => app_skills.combat += growth,
                    5 => app_skills.magic += growth,
                    _ => {}
                }
            }
        }
    }

    // Process coordination deliveries: coordinator delivers directive to target cat.
    for &(coordinator_entity, target_entity) in &coordination_deliveries {
        let delivery = match pending_delivery_query.get(coordinator_entity) {
            Ok(d) => d.0.clone(),
            Err(_) => continue,
        };

        // Compute coordinator's social weight for the bonus calculation.
        // We read the coordinator's memory from the main query.
        let coord_sw = if let Ok((_, _, _, _, _, coord_memory, _, _, _, _)) = query.get(coordinator_entity) {
            crate::systems::coordination::social_weight(
                coordinator_entity,
                &relationships,
                coord_memory,
            )
        } else {
            0.0
        };

        // Competing coordinators: if target already has a directive, keep the one
        // from the coordinator the target likes more.
        if let Ok(existing) = active_directive_query.get(target_entity) {
            let existing_fondness = relationships
                .get(target_entity, existing.coordinator)
                .map_or(0.0, |r| r.fondness);
            let new_fondness = relationships
                .get(target_entity, coordinator_entity)
                .map_or(0.0, |r| r.fondness);
            if new_fondness <= existing_fondness {
                // Target prefers the existing coordinator — skip.
                commands.entity(coordinator_entity).remove::<PendingDelivery>();
                continue;
            }
        }

        commands.entity(target_entity).insert(ActiveDirective {
            kind: delivery.kind,
            priority: delivery.priority,
            coordinator: coordinator_entity,
            coordinator_social_weight: coord_sw,
            delivered_tick: time.tick,
        });
        commands.entity(coordinator_entity).remove::<PendingDelivery>();

        // Small social bump for the coordinator (fulfilling their role).
        if let Ok((_, _, mut needs, _, _, _, _, _, _, _)) = query.get_mut(coordinator_entity) {
            needs.social = (needs.social + 0.05).min(1.0);
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

    use crate::components::building::{StoredItems, Structure};
    use crate::components::items::{Item, ItemKind, ItemLocation};
    use crate::components::mental::{Memory, Mood};
    use crate::components::skills::Skills;
    use crate::resources::map::{Terrain, TileMap};
    use crate::resources::time::TimeState;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TileMap::new(20, 20, Terrain::Grass));
        world.insert_resource(SimRng::new(42));
        world.insert_resource(TimeState::default());
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(Relationships::default());
        world.insert_resource(crate::resources::narrative::NarrativeLog::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_actions);
        (world, schedule)
    }

    /// Spawn a Stores building at the given position with the given food items.
    fn spawn_stores_with_food(world: &mut World, pos: Position, food_items: &[ItemKind]) -> Entity {
        let store = world
            .spawn((
                Structure::new(StructureType::Stores),
                StoredItems::default(),
                pos,
            ))
            .id();

        for &kind in food_items {
            let item_entity = world
                .spawn(Item::new(kind, 1.0, ItemLocation::StoredIn(store)))
                .id();
            world
                .entity_mut(store)
                .get_mut::<StoredItems>()
                .unwrap()
                .add(item_entity, StructureType::Stores);
        }

        store
    }

    fn spawn_cat(
        world: &mut World,
        action: Action,
        ticks: u64,
        target: Option<Position>,
        needs: Needs,
        pos: Position,
    ) -> Entity {
        spawn_cat_with_target_entity(world, action, ticks, target, None, needs, pos)
    }

    fn spawn_cat_with_target_entity(
        world: &mut World,
        action: Action,
        ticks: u64,
        target_pos: Option<Position>,
        target_entity: Option<Entity>,
        needs: Needs,
        pos: Position,
    ) -> Entity {
        world
            .spawn((
                CurrentAction {
                    action,
                    ticks_remaining: ticks,
                    target_position: target_pos,
                    target_entity,
                    last_scores: Vec::new(),
                },
                needs,
                pos,
                Skills::default(),
                Memory::default(),
                Mood::default(),
                crate::components::magic::Inventory::default(),
                Name("Test Cat".to_string()),
            ))
            .id()
    }

    /// Eating should increase hunger by consuming a food item from a Stores building.
    #[test]
    fn eating_restores_hunger_and_consumes_food() {
        let (mut world, mut schedule) = setup_world();

        let store_pos = Position::new(5, 5);
        let store = spawn_stores_with_food(
            &mut world,
            store_pos,
            &[ItemKind::RawMouse, ItemKind::RawFish],
        );

        let mut needs = Needs::default();
        needs.hunger = 0.5;

        // Cat is adjacent to the store and targeting it.
        let entity = spawn_cat_with_target_entity(
            &mut world, Action::Eat, 3, Some(store_pos), Some(store),
            needs, Position::new(5, 6),
        );

        let items_before = world.get::<StoredItems>(store).unwrap().items.len();
        schedule.run(&mut world);
        let items_after = world.get::<StoredItems>(store).unwrap().items.len();

        let n = world.get::<Needs>(entity).unwrap();
        assert!(
            n.hunger > 0.5,
            "hunger should increase after eating; got {}",
            n.hunger
        );
        assert!(
            items_after < items_before,
            "stored items should decrease; before={items_before}, after={items_after}"
        );
    }

    /// Eating stops when no food items are available.
    #[test]
    fn eating_stops_when_stores_empty() {
        let (mut world, mut schedule) = setup_world();

        let store_pos = Position::new(5, 5);
        let store = spawn_stores_with_food(&mut world, store_pos, &[]);

        let mut needs = Needs::default();
        needs.hunger = 0.5;

        // Cat adjacent to empty store.
        let entity = spawn_cat_with_target_entity(
            &mut world, Action::Eat, 5, Some(store_pos), Some(store),
            needs, Position::new(5, 6),
        );

        schedule.run(&mut world);

        let ca = world.get::<CurrentAction>(entity).unwrap();
        assert_eq!(
            ca.ticks_remaining, 0,
            "eating should stop when no food items available"
        );
    }

    /// Eating should not push hunger above 1.0.
    #[test]
    fn eating_clamps_hunger_at_one() {
        let (mut world, mut schedule) = setup_world();

        let store_pos = Position::new(5, 5);
        // RawRat has food_value 0.4, hunger starts at 0.99 — should clamp to 1.0.
        let store = spawn_stores_with_food(&mut world, store_pos, &[ItemKind::RawRat]);

        let mut needs = Needs::default();
        needs.hunger = 0.99;

        let entity = spawn_cat_with_target_entity(
            &mut world, Action::Eat, 2, Some(store_pos), Some(store),
            needs, Position::new(5, 6),
        );

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, 1.0, "hunger should clamp at 1.0; got {}", n.hunger);
    }

    /// Sleeping should restore energy and warmth each tick.
    #[test]
    fn sleeping_restores_energy_and_warmth() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.energy = 0.5;
        needs.warmth = 0.5;

        let entity = spawn_cat(
            &mut world,
            Action::Sleep,
            5,
            None,
            needs,
            Position::new(5, 5),
        );

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert!(
            (n.energy - 0.52).abs() < 1e-5,
            "energy should be ~0.52; got {}",
            n.energy
        );
        assert!(
            (n.warmth - 0.51).abs() < 1e-5,
            "warmth should be ~0.51; got {}",
            n.warmth
        );
    }

    /// Wandering with a target should move the cat each tick.
    #[test]
    fn wandering_moves_cat_toward_target() {
        let (mut world, mut schedule) = setup_world();

        let start = Position::new(0, 0);
        let target = Position::new(5, 5);

        let entity = spawn_cat(
            &mut world,
            Action::Wander,
            10,
            Some(target),
            Needs::default(),
            start,
        );

        schedule.run(&mut world);

        let pos = *world.get::<Position>(entity).unwrap();
        let before_dist = start.manhattan_distance(&target);
        let after_dist = pos.manhattan_distance(&target);
        assert!(
            after_dist < before_dist,
            "cat should have moved closer to target; before={before_dist}, after={after_dist}"
        );
    }

    /// Idle action should have no effect on needs.
    #[test]
    fn idle_has_no_effect() {
        let (mut world, mut schedule) = setup_world();

        let needs = Needs::default();
        let hunger_before = needs.hunger;
        let energy_before = needs.energy;

        let entity = spawn_cat(
            &mut world,
            Action::Idle,
            3,
            None,
            needs,
            Position::new(5, 5),
        );

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert_eq!(n.hunger, hunger_before, "idle should not change hunger");
        assert_eq!(n.energy, energy_before, "idle should not change energy");
    }

    /// ticks_remaining is decremented each run.
    #[test]
    fn ticks_remaining_decrements() {
        let (mut world, mut schedule) = setup_world();

        let entity = spawn_cat(
            &mut world,
            Action::Idle,
            5,
            None,
            Needs::default(),
            Position::new(5, 5),
        );

        schedule.run(&mut world);
        let after_one = world.get::<CurrentAction>(entity).unwrap().ticks_remaining;
        assert_eq!(after_one, 4);

        schedule.run(&mut world);
        let after_two = world.get::<CurrentAction>(entity).unwrap().ticks_remaining;
        assert_eq!(after_two, 3);
    }

    /// An entity with ticks_remaining == 0 should not be affected.
    #[test]
    fn zero_ticks_remaining_is_skipped() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.hunger = 0.5;

        let entity = spawn_cat(
            &mut world,
            Action::Eat,
            0,
            None,
            needs,
            Position::new(5, 5),
        );

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        // The cat's hunger should not change since ticks_remaining is 0.
        assert_eq!(n.hunger, 0.5, "zero-tick action should not modify needs");
    }

    /// Hunting grows the hunting skill on the last tick.
    #[test]
    fn hunting_grows_skill() {
        let (mut world, mut schedule) = setup_world();

        // Set up a map with some forest.
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        map.set(5, 5, Terrain::DenseForest);
        world.insert_resource(map);

        let entity = spawn_cat(
            &mut world,
            Action::Hunt,
            1, // last tick: resolves hunt
            Some(Position::new(5, 5)),
            Needs::default(),
            Position::new(5, 5),
        );

        let skill_before = world.get::<Skills>(entity).unwrap().hunting;
        schedule.run(&mut world);
        let skill_after = world.get::<Skills>(entity).unwrap().hunting;

        assert!(
            skill_after > skill_before,
            "hunting skill should grow; before={skill_before}, after={skill_after}"
        );
    }

    /// Self-grooming should restore warmth.
    #[test]
    fn self_groom_restores_warmth() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.warmth = 0.5;

        let entity = world.spawn((
            CurrentAction {
                action: Action::Groom,
                ticks_remaining: 3,
                target_position: None,
                target_entity: None,
                last_scores: Vec::new(),
            },
            needs,
            Position::new(5, 5),
            Skills::default(),
            Memory::default(),
            Mood::default(),
            crate::components::magic::Inventory::default(),
            Name("Groomer".to_string()),
        )).id();

        schedule.run(&mut world);

        let n = world.get::<Needs>(entity).unwrap();
        assert!(
            (n.warmth - 0.52).abs() < 1e-5,
            "warmth should be ~0.52 after self-groom; got {}",
            n.warmth
        );
    }

    /// Socializing with an adjacent target should restore social need.
    #[test]
    fn socializing_restores_social_need() {
        let (mut world, mut schedule) = setup_world();

        let mut needs_a = Needs::default();
        needs_a.social = 0.5;
        let mut needs_b = Needs::default();
        needs_b.social = 0.5;

        let cat_b = world.spawn((
            CurrentAction::default(),
            needs_b,
            Position::new(5, 6),
            Skills::default(),
            Memory::default(),
            Mood::default(),
            crate::components::magic::Inventory::default(),
            Name("Target".to_string()),
        )).id();

        let _cat_a = world.spawn((
            CurrentAction {
                action: Action::Socialize,
                ticks_remaining: 3,
                target_position: Some(Position::new(5, 6)),
                target_entity: Some(cat_b),
                last_scores: Vec::new(),
            },
            needs_a,
            Position::new(5, 5),
            Skills::default(),
            Memory::default(),
            Mood::default(),
            crate::components::magic::Inventory::default(),
            Name("Socializer".to_string()),
        )).id();

        schedule.run(&mut world);

        let n_b = world.get::<Needs>(cat_b).unwrap();
        assert!(
            n_b.social > 0.5,
            "target cat's social should increase; got {}",
            n_b.social
        );
    }

    /// Socializing builds fondness in the Relationships resource.
    #[test]
    fn socializing_builds_fondness() {
        let (mut world, mut schedule) = setup_world();

        let cat_b = world.spawn((
            CurrentAction::default(),
            Needs::default(),
            Position::new(5, 6),
            Skills::default(),
            Memory::default(),
            Mood::default(),
            crate::components::magic::Inventory::default(),
            Name("Target".to_string()),
        )).id();

        let cat_a = world.spawn((
            CurrentAction {
                action: Action::Socialize,
                ticks_remaining: 3,
                target_position: Some(Position::new(5, 6)),
                target_entity: Some(cat_b),
                last_scores: Vec::new(),
            },
            Needs::default(),
            Position::new(5, 5),
            Skills::default(),
            Memory::default(),
            Mood::default(),
            crate::components::magic::Inventory::default(),
            Name("Socializer".to_string()),
        )).id();

        // Init relationship with known fondness.
        {
            let mut rels = world.resource_mut::<Relationships>();
            let rel = rels.get_or_insert(cat_a, cat_b);
            rel.fondness = 0.0;
            rel.familiarity = 0.0;
        }

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        let rel = rels.get(cat_a, cat_b).unwrap();
        assert!(
            rel.fondness > 0.0,
            "fondness should increase after socializing; got {}",
            rel.fondness
        );
        assert!(
            rel.familiarity > 0.0,
            "familiarity should increase after socializing; got {}",
            rel.familiarity
        );
    }
}
