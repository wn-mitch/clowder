//! Preservation per-tick advancement (ticket 367 Phase 1b).
//!
//! Drying Racks are the only Phase 1b preservation station whose
//! progress advances *passively* — sun does the work, no per-cat
//! tend cycle. This module hosts the per-tick advancement system that
//! turns a loaded `DryingRackState` into a spawned `DriedFish` or
//! `PreservedOrgan` `Item` entity once `progress >= 1.0`.
//!
//! Smoking Racks deliberately have **no per-tick advancement** here —
//! their progress only ticks on discrete tend completions
//! (`resolve_tend_smoking_rack` in `src/steps/disposition/`). The
//! per-tick budget for this module is therefore bounded by the number
//! of *loaded* Drying Racks in the colony (typically 1-3 in a healthy
//! seed-42 soak), not by cat count.
//!
//! Per CLAUDE.md "Default to event-driven; justify per-tick": this is
//! load-bearing per-tick work — drying is a continuous physical
//! process under sun, no event source exists for "the rack has dried
//! another tick's worth of moisture out of the fish." The query is
//! filtered to racks with `state.loaded.is_some()` so idle racks cost
//! one branch-misprediction per tick rather than a full iteration.

use bevy::prelude::*;

use crate::components::building::{
    DryingLoad, DryingRackState, DryingRecipe, Structure, StructureType,
};
use crate::components::item_gate::sources::PreservationOutputSource;
use crate::components::item_gate::{ItemSource, SourceCtx, SourcePlacement};
use crate::components::items::ItemKind;
use crate::components::physical::Position;
use crate::components::recipe::{CraftedItem, RecipeId};
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::TimeState;
use crate::resources::weather::{Weather, WeatherState};
use crate::steps::disposition::preservation_output_quality;

/// Advance every loaded Drying Rack by one tick of progress under
/// Clear weather. When `progress >= 1.0`, spawns the recipe's output
/// `Item` entity on the ground at the rack's tile and resets the
/// rack to idle.
///
/// **Why Clear-weather-only:** the recipe is sun-drying; rain,
/// overcast, snow, fog, wind, or storm conditions either soak the
/// food (rain), block sunlight (overcast / fog), or freeze the cure
/// (snow / wind / storm). Mirrors the design doc's "preservation
/// outputs hold winter buffer calories *because they were made in
/// summer/autumn*" framing — the player can't dry food faster by
/// micromanaging.
///
/// **Why no per-tend handler here:** smoking advances only on tend
/// resolver completions (`resolve_tend_smoking_rack`), not per-tick.
/// Both pipelines are documented in `crafting.md` Phase 1b.
pub fn advance_preservation_drying(
    time: Res<TimeState>,
    weather: Res<WeatherState>,
    constants: Res<SimConstants>,
    mut racks: Query<(Entity, &Position, &Structure, &mut DryingRackState)>,
    mut commands: Commands,
    mut activation: Option<ResMut<SystemActivation>>,
) {
    // Non-clear weather pauses every loaded rack uniformly. The cat
    // who loaded earlier doesn't get penalised — progress stays
    // exactly where it was; the next Clear tick resumes from there.
    if weather.current != Weather::Clear {
        return;
    }

    // Two-pass shape: iterate-and-mutate to advance progress, then
    // spawn outputs after the borrow on `racks` has dropped. The
    // Vec is bounded by the number of loaded racks in this tick
    // (typically 0-3).
    let mut completions: Vec<DryingCompletion> = Vec::new();

    for (entity, rack_pos, structure, mut state) in racks.iter_mut() {
        if structure.kind != StructureType::DryingRack {
            continue;
        }
        // Effectiveness `0.0` means the rack is mid-construction or
        // condition-zero — sun can't dry through a half-built rack.
        if structure.effectiveness() == 0.0 {
            continue;
        }
        let Some(load) = state.loaded.clone() else {
            continue;
        };

        let total_ticks = recipe_total_ticks(&load, &constants);
        if total_ticks == 0 {
            // Defensive: a misconfigured SimConstants with zero
            // duration would otherwise advance progress by infinity
            // and spawn N outputs from one load. Skip and let the
            // operator notice the constant.
            continue;
        }

        state.progress = (state.progress + 1.0 / total_ticks as f32).min(1.0);

        if state.progress >= 1.0 {
            let output_kind = output_for(load.recipe);
            let recipe_id = recipe_id_for(load.recipe);
            let output_quality = preservation_output_quality(
                load.source_quality,
                load.crafter_skill,
                &constants.crafting,
            );

            completions.push(DryingCompletion {
                rack_entity: entity,
                pos: *rack_pos,
                output_kind,
                output_quality,
                output_modifiers: load.source_modifiers,
                recipe_id,
                recipe: load.recipe,
            });

            // Reset the rack to idle in the same tick — the load is
            // consumed, the output entity will spawn below.
            state.loaded = None;
            state.progress = 0.0;
        }
    }

    // Spawn outputs + record Features after the iter_mut borrow drops.
    // Each completion routes through `PreservationOutputSource` — an
    // `AlwaysGround` items-are-real Source. No actor is in scope here
    // (drying is sun-driven), so the SourceCtx carries `inventory: None`.
    // The trait's default body always picks the ground arm; the
    // `CraftedItem` provenance tag is attached to the spawned entity
    // after the trait dispatch.
    for c in &completions {
        let outcome = PreservationOutputSource {
            kind: c.output_kind,
            modifiers: c.output_modifiers,
            quality: c.output_quality,
        }
        .source(&mut SourceCtx {
            inventory: None,
            commands: &mut commands,
            default_position: c.pos,
        });
        if let Some(SourcePlacement::Ground { entity, .. }) = outcome.witness {
            commands.entity(entity).insert(CraftedItem {
                recipe: c.recipe_id,
                // No per-tick "crafter" — drying is sun-driven; the
                // loader's identity rode through `crafter_skill` into
                // the output's quality, which is the substrate-correct
                // place for provenance impact. A future ticket can
                // persist loader-entity onto `DryingLoad` if narrative
                // attribution is wanted ("Simba's first batch of
                // dried fish").
                crafter: None,
                crafted_at_tick: time.tick,
            });
        }
        outcome.record_if_witnessed(activation.as_deref_mut(), PreservationOutputSource::FEATURE);
        // Drying-rack output is `AlwaysGround` by construction, so
        // OverflowToGround would always fire here and inflate the
        // signal — skip it. The Negative anomaly canary stays
        // meaningful only on `InventoryFirst` Sources that overflow
        // because an actor's pouch was full.
    }
    if let Some(act) = activation.as_deref_mut() {
        for c in &completions {
            match c.recipe {
                DryingRecipe::DriedFish => act.record(Feature::FoodDried),
                DryingRecipe::PreservedOrgan => act.record(Feature::OrganPreserved),
            }
        }
    }
}

/// Captured at completion time so the second pass (entity spawn +
/// Feature record) doesn't re-borrow the `DryingRackState`.
struct DryingCompletion {
    /// Held for future debug-trace attribution.
    #[allow(dead_code)]
    rack_entity: Entity,
    pos: Position,
    output_kind: ItemKind,
    output_quality: f32,
    output_modifiers: crate::components::items::ItemModifiers,
    recipe_id: RecipeId,
    recipe: DryingRecipe,
}

fn recipe_total_ticks(load: &DryingLoad, constants: &SimConstants) -> u64 {
    match load.recipe {
        DryingRecipe::DriedFish => constants.crafting.drying_dried_fish_total_ticks,
        DryingRecipe::PreservedOrgan => constants.crafting.drying_preserved_organ_total_ticks,
    }
}

fn output_for(recipe: DryingRecipe) -> ItemKind {
    match recipe {
        DryingRecipe::DriedFish => ItemKind::DriedFish,
        DryingRecipe::PreservedOrgan => ItemKind::PreservedOrgan,
    }
}

fn recipe_id_for(recipe: DryingRecipe) -> RecipeId {
    match recipe {
        DryingRecipe::DriedFish => RecipeId("preserve.dried_fish"),
        DryingRecipe::PreservedOrgan => RecipeId("preserve.preserved_organ"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::{Item, ItemModifiers};
    use crate::resources::sim_constants::SimConstants;

    fn test_app() -> App {
        let mut app = App::new();
        app.insert_resource(TimeState::default());
        app.insert_resource(WeatherState {
            current: Weather::Clear,
            ..WeatherState::default()
        });
        app.insert_resource(SimConstants::default());
        app.insert_resource(SystemActivation::default());
        app.add_systems(Update, advance_preservation_drying);
        app
    }

    fn spawn_rack(app: &mut App, load: Option<DryingLoad>) -> Entity {
        app.world_mut()
            .spawn((
                Structure::new(StructureType::DryingRack),
                DryingRackState {
                    loaded: load,
                    progress: 0.0,
                },
                Position::new(5, 5),
            ))
            .id()
    }

    fn loaded_dried_fish() -> DryingLoad {
        DryingLoad {
            recipe: DryingRecipe::DriedFish,
            source_quality: 1.0,
            crafter_skill: 0.5,
            source_modifiers: ItemModifiers::default(),
        }
    }

    #[test]
    fn empty_rack_is_a_no_op() {
        let mut app = test_app();
        let rack = spawn_rack(&mut app, None);
        app.update();
        let state = app.world().entity(rack).get::<DryingRackState>().unwrap();
        assert!(state.loaded.is_none());
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn non_clear_weather_pauses_progress() {
        let mut app = test_app();
        app.world_mut().resource_mut::<WeatherState>().current = Weather::HeavyRain;
        let rack = spawn_rack(&mut app, Some(loaded_dried_fish()));
        app.update();
        let state = app.world().entity(rack).get::<DryingRackState>().unwrap();
        assert_eq!(state.progress, 0.0, "rain must not advance drying");
        assert!(state.loaded.is_some());
    }

    #[test]
    fn clear_weather_advances_progress_one_tick() {
        let mut app = test_app();
        let rack = spawn_rack(&mut app, Some(loaded_dried_fish()));
        let total_ticks = app
            .world()
            .resource::<SimConstants>()
            .crafting
            .drying_dried_fish_total_ticks;
        app.update();
        let state = app.world().entity(rack).get::<DryingRackState>().unwrap();
        let expected = 1.0 / total_ticks as f32;
        assert!(
            (state.progress - expected).abs() < 1e-6,
            "got {} expected {expected}",
            state.progress,
        );
    }

    #[test]
    fn completion_spawns_output_item_and_resets_rack() {
        let mut app = test_app();
        // Pre-set progress so the next tick crosses the threshold.
        let mut load = loaded_dried_fish();
        load.source_quality = 1.0;
        load.crafter_skill = 1.0;
        let rack = app
            .world_mut()
            .spawn((
                Structure::new(StructureType::DryingRack),
                DryingRackState {
                    loaded: Some(load),
                    progress: 1.0,
                },
                Position::new(5, 5),
            ))
            .id();
        app.update();
        let state = app.world().entity(rack).get::<DryingRackState>().unwrap();
        assert!(state.loaded.is_none(), "rack must reset after spawn");
        assert_eq!(state.progress, 0.0);

        // Output entity must exist at the rack's position.
        let mut found: Option<(ItemKind, f32)> = None;
        for (item, pos) in app
            .world_mut()
            .query::<(&Item, &Position)>()
            .iter(app.world())
        {
            if *pos == Position::new(5, 5) {
                found = Some((item.kind, item.quality));
                break;
            }
        }
        let (kind, quality) = found.expect("output item must spawn at rack tile");
        assert_eq!(kind, ItemKind::DriedFish);
        // Perfect input + perfect skill → quality 1.0 (clamped).
        assert!((quality - 1.0).abs() < 1e-6, "got quality {quality}");
    }

    #[test]
    fn completion_emits_food_dried_feature() {
        let mut app = test_app();
        let load = loaded_dried_fish();
        app.world_mut().spawn((
            Structure::new(StructureType::DryingRack),
            DryingRackState {
                loaded: Some(load),
                progress: 1.0,
            },
            Position::new(5, 5),
        ));
        app.update();
        let act = app.world().resource::<SystemActivation>();
        assert!(
            act.counts.get(&Feature::FoodDried).copied().unwrap_or(0) >= 1,
            "FoodDried must fire on completion: counts = {:?}",
            act.counts,
        );
    }

    #[test]
    fn preserved_organ_completion_emits_organ_feature() {
        let mut app = test_app();
        let load = DryingLoad {
            recipe: DryingRecipe::PreservedOrgan,
            source_quality: 1.0,
            crafter_skill: 0.5,
            source_modifiers: ItemModifiers::default(),
        };
        app.world_mut().spawn((
            Structure::new(StructureType::DryingRack),
            DryingRackState {
                loaded: Some(load),
                progress: 1.0,
            },
            Position::new(5, 5),
        ));
        app.update();
        let act = app.world().resource::<SystemActivation>();
        assert!(
            act.counts
                .get(&Feature::OrganPreserved)
                .copied()
                .unwrap_or(0)
                >= 1,
            "OrganPreserved must fire on completion",
        );
        // And FoodDried must NOT fire — the recipe variants are
        // distinct Feature emissions.
        assert_eq!(act.counts.get(&Feature::FoodDried).copied().unwrap_or(0), 0,);
    }
}
