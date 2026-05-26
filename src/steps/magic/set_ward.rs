use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::magic::{Inventory, MisfireEffectKind, Ward, WardKind};
use crate::components::mental::Mood;
use crate::components::physical::{Health, Position};
use crate::components::recipe::CraftedItem;
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::messages::misfire_effect::MisfireEffect;
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::sim_constants::{CombatConstants, MagicConstants};
use crate::resources::time::TimeScale;
use crate::steps::StepResult;

/// # GOAP step resolver: `SetWard`
///
/// **Real-world effect** — on first tick, rolls a misfire check;
/// on completion, consumes a Thornbriar herb from inventory and
/// spawns a `Ward` entity at the actor's position with a
/// `CraftedItem` provenance Component (ticket 365 — 016 Phase 1a:
/// wards are crafted-item world entities, first member of the
/// WorldPosition-destination recipe family that Phase 4
/// decorations extend). Grows magic skill.
///
/// **Plan-level preconditions** — emitted by the magic planner
/// for ward-placement DSEs. 084 Commit 2 added a retrieve-from-
/// stash branch to the `HerbcraftSetWard` template — when the
/// chain routes through `RetrieveHerbs(Thornbriar)` first, the
/// planner-level guarantee is `CarryingIs(Herbs)` by the time
/// `SetWard` executes (same precondition the gather-from-wild
/// branch produces). The Fail path below therefore becomes
/// correctly unreachable on the retrieve-path (planner-side
/// `CarryingIs(Herbs)` precondition + cat-side `add_herb` succeeded
/// in the resolver). It stays as defense-in-depth in case
/// runtime-state divergence ever puts a cat at SetWard without a
/// thornbriar (e.g. concurrent drop event).
///
/// **Runtime preconditions** — herb consumption may fail; Fail
/// on `inventory.take_herb(Thornbriar)` miss or misfire fizzle.
///
/// **Witness** — returns plain `StepResult`. Predates the
/// `StepOutcome<W>` convention. Advance is witness-equivalent
/// (ward entity only spawns on success path).
///
/// **Feature emission** — caller records `Feature::WardPlaced`
/// (Positive) on Advance at `src/systems/goap.rs:2259,2292` and
/// `src/systems/magic.rs:719`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_set_ward(
    ticks: u64,
    entity: Entity,
    kind: WardKind,
    cat_name: &str,
    inventory: &mut Inventory,
    magic_aff: &MagicAffinity,
    skills: &mut Skills,
    mood: &mut Mood,
    corruption: &mut Corruption,
    health: &mut Health,
    pos: &Position,
    rng: &mut impl Rng,
    commands: &mut Commands,
    log: &mut NarrativeLog,
    event_log: Option<&mut EventLog>,
    misfire_writer: &mut MessageWriter<MisfireEffect>,
    tick: u64,
    m: &MagicConstants,
    combat: &CombatConstants,
    time_scale: &TimeScale,
    // 301: Path A (`true`) when the cat was carrying out an
    // `ActiveDirective::SetWard` whose target the coordinator chose
    // via `compute_ward_placement`; Path B (`false`) when the cat
    // self-picked `HerbcraftSetWard` and is planting at its current
    // position. Routed into the `WardPlaced` event so post-soak
    // validation can isolate the directive-driven subset that the
    // ticket-301 structural change actually shifts.
    via_directive: bool,
    crafter: Option<Entity>,
) -> StepResult {
    if ticks >= m.set_ward_duration.ticks(time_scale) {
        // Consume thornbriar if setting a thornward.
        if kind == WardKind::Thornward
            && !inventory.take_herb(crate::components::magic::HerbKind::Thornbriar)
        {
            return StepResult::Fail("no thornbriar for ward".into());
        }

        // Check for misfire on magical actions.
        if kind == WardKind::DurableWard {
            if let Some(misfire) =
                crate::systems::magic::check_misfire(magic_aff.0, skills.magic, rng, m)
            {
                crate::systems::magic::apply_misfire(
                    entity,
                    misfire,
                    cat_name,
                    mood,
                    corruption,
                    health,
                    pos,
                    commands,
                    log,
                    misfire_writer,
                    tick,
                    m,
                    combat,
                    time_scale,
                );
                if matches!(misfire, MisfireEffectKind::Fizzle) {
                    return StepResult::Fail("misfire: fizzle".into());
                }
                if matches!(misfire, MisfireEffectKind::InvertedWard) {
                    // Spawn inverted ward instead. Carries CraftedItem
                    // even though the misfire path produced an inverted
                    // outcome — the cat still performed the craft work,
                    // and provenance attribution matters for the
                    // narrative layer.
                    commands.spawn((
                        Ward::inverted_at(kind),
                        Position::new(pos.x, pos.y),
                        CraftedItem {
                            recipe: ward_recipe_id(kind),
                            crafter,
                            crafted_at_tick: tick,
                        },
                    ));
                    return StepResult::Advance;
                }
            }
        }

        // Spawn the ward entity. Thornward decay rate is configurable.
        let mut ward = match kind {
            WardKind::Thornward => Ward::thornward(),
            WardKind::DurableWard => Ward::durable(),
        };
        if kind == WardKind::Thornward {
            ward.decay_rate = m.thornward_decay_rate.per_tick(time_scale);
        }
        let spawn_strength = ward.strength;
        commands.spawn((
            ward,
            Position::new(pos.x, pos.y),
            CraftedItem {
                recipe: ward_recipe_id(kind),
                crafter,
                crafted_at_tick: tick,
            },
        ));
        if let Some(elog) = event_log {
            elog.push(
                tick,
                EventKind::WardPlaced {
                    cat: cat_name.to_string(),
                    ward_kind: format!("{kind:?}"),
                    location: (pos.x, pos.y),
                    strength: spawn_strength,
                    via_directive,
                },
            );
        }
        skills.herbcraft += skills.growth_rate() * m.herbcraft_ward_skill_growth;
        // Magic-affinity cats absorb magic practice from any ward work.
        // This gives gifted-but-untrained cats a natural progression path:
        // they work herbcraft wards alongside the rest of the colony, and
        // their magic skill climbs until durable wards become viable. Cats
        // without affinity gain herbcraft only, as intended.
        if magic_aff.0 > 0.2 || kind == WardKind::DurableWard {
            skills.magic += skills.growth_rate() * m.magic_ward_skill_growth;
        }
        let text = match kind {
            WardKind::Thornward => {
                let variants = [
                    format!("{cat_name} traces thornbriar sigils into the earth. A ward stands."),
                    format!("{cat_name} weaves thornbriar into a warding sigil."),
                    format!("{cat_name} presses thornbriar into the soil — the air tightens."),
                ];
                variants[rng.random_range(0..variants.len())].clone()
            }
            WardKind::DurableWard => {
                let variants = [
                    format!("{cat_name} chants the old words. A durable ward takes root."),
                    format!("{cat_name} sets a deep ward, felt more than seen."),
                ];
                variants[rng.random_range(0..variants.len())].clone()
            }
        };
        log.push(tick, text, NarrativeTier::Significant);
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}

/// Recipe id for a ward kind (ticket 365 — 016 Phase 1a). One
/// recipe per WardKind. Mirrors `RemedyKind::recipe_id`. Used by
/// `resolve_set_ward` to attach `CraftedItem` provenance and by
/// `populate_recipe_registry` to register the catalog entries.
pub fn ward_recipe_id(kind: WardKind) -> crate::components::recipe::RecipeId {
    use crate::components::recipe::RecipeId;
    match kind {
        WardKind::Thornward => RecipeId("ward.thornward"),
        WardKind::DurableWard => RecipeId("ward.durable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::magic::HerbKind;
    use crate::components::recipe::CraftedItem;
    use crate::resources::sim_constants::SimConstants;
    use bevy_ecs::system::SystemState;
    use rand::SeedableRng;

    fn time_scale() -> TimeScale {
        TimeScale::from_config(&crate::resources::time::SimConfig::default(), 16.6667)
    }

    #[test]
    fn thornward_spawn_carries_crafted_item_provenance() {
        let mut world = World::new();
        world.init_resource::<bevy_ecs::message::Messages<MisfireEffect>>();
        let mut state: SystemState<(Commands, MessageWriter<MisfireEffect>)> =
            SystemState::new(&mut world);

        let constants = SimConstants::default();
        let m = &constants.magic;
        let combat = &constants.combat;
        let ts = time_scale();

        let mut inventory = Inventory::default();
        inventory.add_herb(HerbKind::Thornbriar);
        let mut skills = Skills::default();
        let mut mood = Mood::default();
        let mut corruption = Corruption(0.0);
        let mut health = Health::default();
        let pos = Position::new(3, 4);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let magic_aff = MagicAffinity(0.0);
        let mut log = NarrativeLog::default();
        let entity = world.spawn_empty().id();
        let crafter = world.spawn_empty().id();

        let required = m.set_ward_duration.ticks(&ts);
        let (mut commands, mut misfire_writer) = state.get_mut(&mut world);
        let result = resolve_set_ward(
            required,
            entity,
            WardKind::Thornward,
            "Sage",
            &mut inventory,
            &magic_aff,
            &mut skills,
            &mut mood,
            &mut corruption,
            &mut health,
            &pos,
            &mut rng,
            &mut commands,
            &mut log,
            None, // no event log
            &mut misfire_writer,
            500, // tick
            m,
            combat,
            &ts,
            false,         // via_directive
            Some(crafter), // 365: crafter provenance
        );
        state.apply(&mut world);

        assert!(matches!(result, StepResult::Advance));
        assert!(!inventory.has_herb(HerbKind::Thornbriar));

        let mut found = 0_u32;
        let mut q = world.query::<(&Ward, &Position, &CraftedItem)>();
        for (ward, ward_pos, ci) in q.iter(&world) {
            assert_eq!(ward.kind, WardKind::Thornward);
            assert_eq!(ward_pos.x, pos.x);
            assert_eq!(ward_pos.y, pos.y);
            assert_eq!(ci.recipe, ward_recipe_id(WardKind::Thornward));
            assert_eq!(ci.crafter, Some(crafter));
            assert_eq!(ci.crafted_at_tick, 500);
            found += 1;
        }
        assert_eq!(found, 1, "exactly one ward spawned with CraftedItem");
    }
}
