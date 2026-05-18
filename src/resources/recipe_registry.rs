// Recipe registry (ticket 365 — 016 Phase 1a).
//
// Single source of truth for crafting recipe data. Populated
// at startup by `crate::plugins::simulation::populate_recipe_registry`,
// mirroring the `DseRegistry` / `MethodRegistry` /
// `InfluenceMapRegistry` populate pattern.
//
// Read by per-discipline resolvers (`resolve_prepare_remedy`,
// `resolve_set_ward`, …) when they need recipe inputs / duration
// / destination; read by HTN methods when they emit craft
// intentions citing `RecipeId`. The registry never carries
// per-cat or per-frame state — it's static data plumbed once.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::components::recipe::{Recipe, RecipeId};

/// Catalog of every recipe the simulation knows about.
#[derive(Resource, Default, Debug, Clone)]
pub struct RecipeRegistry {
    recipes: HashMap<RecipeId, Recipe>,
}

impl RecipeRegistry {
    /// Register a recipe. Panics on duplicate ids — recipe ids
    /// are stable identifiers and silent overwrites would mask
    /// merge conflicts between contributors editing the populate
    /// function.
    pub fn insert(&mut self, recipe: Recipe) {
        let id = recipe.id;
        if self.recipes.insert(id, recipe).is_some() {
            panic!("duplicate RecipeId registered: {id:?}");
        }
    }

    pub fn get(&self, id: RecipeId) -> Option<&Recipe> {
        self.recipes.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::items::ItemKind;
    use crate::components::recipe::{
        DisciplineKind, ItemDestination, RecipeDuration, RecipeInput, RecipeOutput,
        StationRequirement,
    };

    fn sample_recipe(id: &'static str) -> Recipe {
        Recipe {
            id: RecipeId(id),
            discipline: DisciplineKind::Herbalism,
            inputs: vec![RecipeInput {
                kind: ItemKind::HerbHealingMoss,
                count: 1,
            }],
            station: StationRequirement::Workshop,
            duration: RecipeDuration::Fixed { ticks: 10 },
            output: RecipeOutput {
                item_kind: ItemKind::HerbHealingMoss,
                destination: ItemDestination::Inventory,
            },
        }
    }

    #[test]
    fn empty_registry_has_no_recipes() {
        let registry = RecipeRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get(RecipeId("anything")).is_none());
    }

    #[test]
    fn insert_and_lookup_round_trip() {
        let mut registry = RecipeRegistry::default();
        registry.insert(sample_recipe("test.one"));
        registry.insert(sample_recipe("test.two"));
        assert_eq!(registry.len(), 2);
        assert!(registry.get(RecipeId("test.one")).is_some());
        assert!(registry.get(RecipeId("test.two")).is_some());
        assert!(registry.get(RecipeId("missing")).is_none());
    }

    #[test]
    #[should_panic(expected = "duplicate RecipeId registered")]
    fn duplicate_id_panics() {
        let mut registry = RecipeRegistry::default();
        registry.insert(sample_recipe("dup"));
        registry.insert(sample_recipe("dup"));
    }
}
