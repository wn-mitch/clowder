use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::{Action, CurrentAction};
use crate::components::identity::{Age, Appearance, Gender, Name};
use crate::components::mental::Mood;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::resources::map::TileMap;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::narrative_templates::{
    resolve_variables, MoodBucket, TemplateContext, TemplateRegistry, VariableContext,
};
use crate::resources::rng::SimRng;
use crate::resources::time::{DayPhase, Season, SimConfig, TimeState};
use crate::resources::weather::WeatherState;

// ---------------------------------------------------------------------------
// generate_narrative system
// ---------------------------------------------------------------------------

/// Emit a narrative line for each cat that is on the last tick of its current
/// action (`ticks_remaining == 1`).
///
/// When a [`TemplateRegistry`] resource is present, uses the condition-matching
/// template engine. Otherwise falls back to hardcoded strings.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn generate_narrative(
    query: Query<
        (
            &Name,
            &CurrentAction,
            &Needs,
            &Personality,
            &Mood,
            &Gender,
            &Age,
            &Appearance,
            &Position,
        ),
        Without<Dead>,
    >,
    names: Query<&Name>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    weather: Res<WeatherState>,
    registry: Option<Res<TemplateRegistry>>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
    let tick = time.tick;
    let day_phase = DayPhase::from_tick(tick, &config);
    let season = Season::from_tick(tick, &config);

    for (name, current, needs, personality, mood, gender, age, appearance, pos) in &query {
        if current.ticks_remaining != 1 {
            continue;
        }

        let has_target = current.target_entity.is_some();

        // Rate-limit idle narration: only 1 in 5 completions.
        if current.action == Action::Idle {
            let roll: u32 = rng.rng.random_range(0..5);
            if roll != 0 {
                continue;
            }
        }

        // Hunt/Forage emit event-specific narrative (scent, catch, fail)
        // directly from resolve_disposition_chains. Skip them here to avoid
        // duplicate ticker-tape entries.
        if matches!(current.action, Action::Hunt | Action::Forage) {
            continue;
        }

        // Rate-limit routine narration.
        match current.action {
            Action::Eat | Action::Sleep => {
                let roll: u32 = rng.rng.random_range(0..3);
                if roll != 0 {
                    continue;
                }
            }
            // 158: rate-limit applies to *self*-grooming (the high-
            // frequency thermal-care variant). Allogrooming
            // (`GroomOther`) is rarer and has its own narration line
            // below.
            Action::GroomSelf if !has_target => {
                let roll: u32 = rng.rng.random_range(0..2);
                if roll != 0 {
                    continue;
                }
            }
            _ => {}
        }

        let cat = &name.0;
        let other_name: Option<String> = current
            .target_entity
            .and_then(|e| names.get(e).ok())
            .map(|n| n.0.clone());

        let terrain = if map.in_bounds(pos.x, pos.y) {
            map.get(pos.x, pos.y).terrain
        } else {
            crate::resources::map::Terrain::Grass
        };

        // Try the template engine first.
        if let Some(ref reg) = registry {
            let ctx = TemplateContext {
                action: current.action,
                day_phase,
                season,
                weather: weather.current,
                mood_bucket: MoodBucket::from_valence(mood.valence),
                life_stage: age.stage(tick, config.ticks_per_season),
                has_target,
                terrain,
                event: None,
            };

            if let Some(template) = reg.select(&ctx, personality, needs, &mut rng.rng) {
                let tier = template.tier;
                let var_ctx = VariableContext {
                    name: cat,
                    gender: *gender,
                    weather: weather.current,
                    day_phase,
                    season,
                    life_stage: age.stage(tick, config.ticks_per_season),
                    fur_color: &appearance.fur_color,
                    other: other_name.as_deref(),
                    prey: None,
                    item: None,
                    item_singular: None,
                    quality: None,
                };
                let text = resolve_variables(&template.text, &var_ctx);
                log.push(tick, text, tier);
                continue;
            }
        }

        // Fallback: hardcoded templates (for backwards compatibility / empty registry).
        let (text, tier) = match current.action {
            Action::Eat => {
                let options = [
                    format!("{cat} eats from the stores."),
                    format!("{cat} has a quick meal."),
                    format!("{cat} chews thoughtfully."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Sleep => {
                let options = [
                    format!("{cat} curls up and sleeps."),
                    format!("{cat} naps in a quiet corner."),
                    format!("{cat} dozes off."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Hunt => {
                let options = [
                    format!("{cat} catches a vole in one clean strike."),
                    format!("{cat} drags back a field mouse."),
                    format!("{cat} stalks through the undergrowth but comes back empty-pawed."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Forage => {
                let options = [
                    format!("{cat} gathers a pawful of seeds."),
                    format!("{cat} finds late-season berries."),
                    format!("{cat} digs up edible roots."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Wander => {
                let options = [
                    format!("{cat} wanders about."),
                    format!("{cat} explores nearby."),
                    format!("{cat} stretches and strolls."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Socialize => {
                let options = [
                    format!("{cat} chats with a companion."),
                    format!("{cat} shares a moment with a friend."),
                    format!("{cat} sits close to another cat."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::GroomSelf => {
                let options = [
                    format!("{cat} grooms carefully."),
                    format!("{cat} smooths down ruffled fur."),
                    format!("{cat} licks a paw clean."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Micro)
            }

            Action::GroomOther => {
                let options = [
                    format!("{cat} grooms a companion's coat."),
                    format!("{cat} returns a friend's affectionate licks."),
                    format!("{cat} settles in to groom another cat."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Explore => {
                let options = [
                    format!("{cat} ventures into unfamiliar ground."),
                    format!("{cat} scouts the perimeter."),
                    format!("{cat} discovers something interesting."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Idle => {
                let text = if needs.hunger < 0.3 {
                    format!("{cat}'s stomach growls.")
                } else if needs.energy < 0.3 {
                    format!("{cat} yawns widely.")
                } else {
                    let options = [
                        format!("{cat} sits quietly."),
                        format!("{cat} grooms a paw."),
                        format!("{cat} watches the sky."),
                    ];
                    let idx = rng.rng.random_range(0..options.len());
                    options[idx].clone()
                };
                (text, NarrativeTier::Micro)
            }

            Action::Flee => {
                let options = [
                    format!("{cat} bolts toward the den, ears flat."),
                    format!("{cat} retreats from the undergrowth."),
                    format!("{cat} dashes to safety."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Fight => {
                // Combat narrative is event-driven from the combat system.
                // Action-completion narrative is minimal.
                (
                    format!("{cat} disengages from the fight."),
                    NarrativeTier::Action,
                )
            }

            Action::Patrol => {
                let options = [
                    format!("{cat} patrols the colony perimeter."),
                    format!("{cat} keeps watch along the edge of camp."),
                    format!("{cat} scans the tree line for movement."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Build => {
                let options = [
                    format!("{cat} hammers a beam into place."),
                    format!("{cat} hauls stones to the construction site."),
                    format!("{cat} patches a crack in the wall with careful paws."),
                    format!("{cat} packs mud between the timbers."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Farm => {
                let options = [
                    format!("{cat} tends the garden rows."),
                    format!("{cat} turns the soil with patient paws."),
                    format!("{cat} coaxes a seedling out of the earth."),
                    format!("{cat} gathers ripe herbs from the garden."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            // 155: Herbcraft fanned to 3 sub-actions; each draws from
            // the same Herbalism prose pool. PracticeMagic fanned to 6
            // sub-actions; same Witchcraft pool.
            Action::HerbcraftGather
            | Action::HerbcraftRemedy
            | Action::HerbcraftSetWard => {
                let options = [
                    format!("{cat} carefully harvests herbs from the undergrowth."),
                    format!("{cat} grinds herbs into a poultice with practiced paws."),
                    format!("{cat} weaves a thornward at the colony's edge."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::MagicScry
            | Action::MagicDurableWard
            | Action::MagicCleanse
            | Action::MagicColonyCleanse
            | Action::MagicHarvest
            | Action::MagicCommune => {
                let options = [
                    format!("{cat}'s eyes go distant, seeing something far away..."),
                    format!("{cat} presses paws against the earth, whispering old words."),
                    format!("{cat} sits before the standing stone, lost in meditation."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Significant)
            }

            Action::Coordinate => {
                let other = other_name.as_deref().unwrap_or("a companion");
                let options = [
                    format!("{cat} approaches {other} with purpose."),
                    format!("{cat} nudges {other} toward a task."),
                    format!("{cat} assigns {other} a duty with a firm look."),
                    format!("{cat} confers with {other} in low tones."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Mentor => {
                let other = other_name.as_deref().unwrap_or("a younger cat");
                let options = [
                    format!("{cat} teaches {other} a new trick."),
                    format!("{cat} shows {other} the finer points of the craft."),
                    format!("{cat} guides {other} with patient paws."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Mate => {
                let other = other_name.as_deref().unwrap_or("their partner");
                (
                    format!("{cat} and {other} share a tender moment."),
                    NarrativeTier::Significant,
                )
            }

            Action::Caretake => (
                format!("{cat} tends to a hungry kitten."),
                NarrativeTier::Action,
            ),

            Action::Cook => {
                let options = [
                    format!("{cat} tends a pot of something savoury at the hearth."),
                    format!("{cat} turns food over hot coals."),
                    format!("{cat} coaxes flavour out of a simple meal."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Hide => {
                // Ticket 104 — Hide/Freeze valence. Phase 1 ships
                // dormant (HideEligible marker never authored), so
                // this arm is unreachable at runtime today; lands the
                // narrative anchor so once lift activation wakes the
                // DSE up, the ticker has prose ready.
                let options = [
                    format!("{cat} flattens against the ground, holding still."),
                    format!("{cat} freezes in place, ears pinned back."),
                    format!("{cat} crouches low, breathing shallowly."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            // 176: inventory-disposal narratives. The DSEs ship with
            // default-zero scoring weights so these arms are reached
            // only after balance-tuning lifts the saturation surfaces;
            // landing the prose now keeps the ticker ready.
            Action::Drop => {
                let options = [
                    format!("{cat} drops a surplus catch where they stand."),
                    format!("{cat} sets down what they were carrying."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Trash => {
                let options = [
                    format!("{cat} carries refuse over to the midden."),
                    format!("{cat} leaves an unwanted item at the colony's discard pile."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::Handoff => {
                let options = [
                    format!("{cat} passes a catch to a colony-mate."),
                    format!("{cat} hands off what they were carrying."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }

            Action::PickUp => {
                let options = [
                    format!("{cat} picks up an item from the ground."),
                    format!("{cat} stoops to gather a ground item into their inventory."),
                ];
                let idx = rng.rng.random_range(0..options.len());
                (options[idx].clone(), NarrativeTier::Action)
            }
        };

        log.push(tick, text, tier);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    use crate::components::identity::{Age, Appearance, Gender, Orientation, Species};
    use crate::components::mental::{Memory, Mood};
    use crate::components::personality::Personality;
    use crate::components::physical::{DeathCause, Health};
    use crate::components::skills::{Corruption, MagicAffinity, Skills, Training};
    use crate::resources::narrative::NarrativeLog;
    use crate::resources::rng::SimRng;
    use crate::resources::time::{SimConfig, TimeState};
    use crate::resources::weather::WeatherState;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(TimeState::default());
        world.insert_resource(SimConfig::default());
        world.insert_resource(WeatherState::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SimRng::new(42));
        world.insert_resource(TileMap::new(20, 20, crate::resources::map::Terrain::Grass));
        // No TemplateRegistry — tests exercise the fallback path.
        let mut schedule = Schedule::default();
        schedule.add_systems(generate_narrative);
        (world, schedule)
    }

    fn test_personality() -> Personality {
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;
        Personality::random(&mut ChaCha8Rng::seed_from_u64(0))
    }

    fn test_appearance() -> Appearance {
        Appearance {
            fur_color: "tabby".to_string(),
            pattern: "striped".to_string(),
            eye_color: "green".to_string(),
            distinguishing_marks: vec![],
        }
    }

    /// Spawn a cat with the full component set needed by generate_narrative.
    fn spawn_cat(world: &mut World, name: &str, action: Action, ticks_remaining: u64) -> Entity {
        spawn_cat_with_needs(world, name, action, ticks_remaining, Needs::default())
    }

    fn spawn_cat_with_needs(
        world: &mut World,
        name: &str,
        action: Action,
        ticks_remaining: u64,
        needs: Needs,
    ) -> Entity {
        world
            .spawn((
                (
                    Name(name.to_string()),
                    Species,
                    Age { born_tick: 0 },
                    Gender::Queen,
                    Orientation::Bisexual,
                    test_personality(),
                    test_appearance(),
                ),
                (
                    crate::components::physical::Position::new(5, 5),
                    Health::default(),
                    needs,
                    Mood::default(),
                    Memory::default(),
                    Skills::default(),
                    MagicAffinity(0.0),
                    Corruption(0.0),
                    Training::default(),
                    CurrentAction {
                        action,
                        ticks_remaining,
                        target_position: None,
                        target_entity: None,
                        last_scores: Vec::new(),
                    },
                ),
            ))
            .id()
    }

    /// An action with ticks_remaining == 1 should produce a log entry.
    #[test]
    fn narrates_on_last_tick() {
        let (mut world, mut schedule) = setup_world();
        spawn_cat(&mut world, "Mochi", Action::Eat, 1);

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        assert_eq!(log.entries.len(), 1, "should have one entry");
        assert!(
            log.entries[0].text.contains("Mochi"),
            "entry should mention the cat's name"
        );
        assert_eq!(log.entries[0].tier, NarrativeTier::Action);
    }

    /// An action with ticks_remaining != 1 should not produce a log entry.
    #[test]
    fn does_not_narrate_mid_action() {
        let (mut world, mut schedule) = setup_world();
        spawn_cat(&mut world, "Pepper", Action::Sleep, 10);

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        assert!(log.entries.is_empty(), "should not narrate mid-action");
    }

    /// Idle narration uses Micro tier (not Action).
    #[test]
    fn idle_uses_micro_tier() {
        let (mut world, mut schedule) = setup_world();
        spawn_cat(&mut world, "Dusk", Action::Idle, 1);

        // Run 20 ticks to give rate-limit at least one chance to pass.
        for _ in 0..20 {
            schedule.run(&mut world);
        }

        let log = world.resource::<NarrativeLog>();
        let idle_entries: Vec<_> = log
            .entries
            .iter()
            .filter(|e| e.tier == NarrativeTier::Micro)
            .collect();

        assert!(
            !idle_entries.is_empty(),
            "at least one Micro-tier idle entry should appear in 20 ticks"
        );
    }

    /// Hungry cat idle text mentions the stomach.
    #[test]
    fn hungry_idle_mentions_stomach() {
        let (mut world, mut schedule) = setup_world();

        let mut needs = Needs::default();
        needs.hunger = 0.1; // below 0.3 threshold

        for _ in 0..10 {
            spawn_cat_with_needs(&mut world, "Pip", Action::Idle, 1, needs.clone());
        }

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        let stomach_entries: Vec<_> = log
            .entries
            .iter()
            .filter(|e| e.text.contains("stomach"))
            .collect();

        assert!(
            !stomach_entries.is_empty(),
            "at least one 'stomach growls' entry expected for hungry cats"
        );
    }

    /// Dead cats should not produce narrative entries.
    #[test]
    fn dead_cat_does_not_narrate() {
        let (mut world, mut schedule) = setup_world();
        let entity = spawn_cat(&mut world, "Ghost", Action::Eat, 1);
        world.entity_mut(entity).insert(Dead {
            tick: 0,
            cause: DeathCause::Starvation,
        });

        schedule.run(&mut world);

        let log = world.resource::<NarrativeLog>();
        assert!(
            log.entries.is_empty(),
            "dead cat should not produce narrative; got {:?}",
            log.entries
        );
    }
}
