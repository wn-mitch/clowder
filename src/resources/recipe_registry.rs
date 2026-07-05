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

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::components::recipe::{Recipe, RecipeId};
use crate::components::skills::Skills;

/// Catalog of every recipe the simulation knows about.
///
/// Iteration order is **registration order** (the order
/// `populate_recipe_registry` inserts) — ticket 502. This order is
/// load-bearing: `emit_have_item_row`'s winner scan and
/// [`Self::recipe_producing`]'s first-match both resolve byte-equal
/// score ties toward the earlier-registered recipe, so registration
/// order IS the author-curated tie-break priority (and
/// `recipe_producing`'s doc always promised "first-registered match").
/// The pre-502 `HashMap` storage silently broke that promise: its
/// per-process `RandomState` iteration order flipped ties between
/// runs of the same binary (observed: the seed-42 canonical soak
/// forked at elapsed tick 3750 on a warriors-kit aspiration tie and
/// again at the first remedy craft). Keyed lookup goes through a
/// `BTreeMap` index. Same determinism doctrine as `Relationships`
/// (relationships.rs).
#[derive(Resource, Default, Debug, Clone)]
pub struct RecipeRegistry {
    /// Recipes in registration order — the tie-break priority surface.
    recipes: Vec<Recipe>,
    /// `RecipeId` → index into `recipes` for keyed lookup.
    index: BTreeMap<RecipeId, usize>,
}

impl RecipeRegistry {
    /// Register a recipe. Panics on duplicate ids — recipe ids
    /// are stable identifiers and silent overwrites would mask
    /// merge conflicts between contributors editing the populate
    /// function. Registration position doubles as tie-break
    /// priority (see type-level doc) — insert order in
    /// `populate_recipe_registry` is a design surface, not
    /// happenstance.
    pub fn insert(&mut self, recipe: Recipe) {
        let id = recipe.id;
        if self.index.insert(id, self.recipes.len()).is_some() {
            panic!("duplicate RecipeId registered: {id:?}");
        }
        self.recipes.push(recipe);
    }

    pub fn get(&self, id: RecipeId) -> Option<&Recipe> {
        self.index.get(&id).map(|&i| &self.recipes[i])
    }

    /// 462: find the recipe whose output is `item`. Used by the
    /// `decompose_goal_have_item` HTN substrate helper to turn a
    /// `GoalKind::HaveItem(item)` Intention into a craft plan.
    ///
    /// Linear scan over registered recipes (≤50 today). If multiple
    /// recipes share an output kind (the two ward recipes share a
    /// placeholder output today, but wards are never HaveItem lookup
    /// targets), this returns the first-registered match — now
    /// actually true (502); the pre-502 HashMap made it per-process
    /// arbitrary despite this doc's promise.
    pub fn recipe_producing(&self, item: crate::components::items::ItemKind) -> Option<&Recipe> {
        self.iter().find(|r| r.output.item_kind == item)
    }

    /// Iterate recipes in registration order (stable across processes
    /// — ticket 502). First-seen-wins consumers inherit registration
    /// order as their tie-break priority.
    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.iter()
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    /// Phase 5 gating predicate (366 — 016 Phase 5 precursor).
    ///
    /// Walks every recipe carrying a `skill_gate` and returns true
    /// iff at least one such recipe is currently unlocked by the
    /// colony — i.e. at least one cat's matching skill axis clears
    /// the recipe's threshold. OSRS-style: each recipe declares its
    /// own min-level on a typed `SkillKind` axis.
    ///
    /// 366 land: predicate is shape-correct but returns false
    /// because no recipe carries a `Some(skill_gate)` yet. 372 lands
    /// the first Phase 5 recipes (Generational Tapestry, Shrine-Cairn,
    /// Bone-Lattice Lantern, Pigment-Deepened Textile) plus the
    /// craft-action step resolvers that grow the matching skill
    /// axes (`weaving` / `bone_shaping` / `hidework` / `pigment` /
    /// `cairn`); the predicate flips naturally as cats grind past
    /// the recipe thresholds.
    ///
    /// Adoption of a mastery arc does NOT alone unlock recipes —
    /// the cat must clear the recipe's skill threshold, which only
    /// becomes possible once 372 wires the action-side skill growth.
    pub fn is_phase5_unlocked<'a>(
        &self,
        colony_skills: impl IntoIterator<Item = &'a Skills>,
    ) -> bool {
        let cats: Vec<&Skills> = colony_skills.into_iter().collect();
        self.iter().any(|recipe| {
            let Some((skill, threshold)) = recipe.skill_gate else {
                return false;
            };
            cats.iter().any(|s| skill.value(s) >= threshold)
        })
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
            skill_gate: None,
            is_warriors_kit: false,
            discipline_skill_affinity: None,
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

    /// Ticket 502 — iteration order must be registration order,
    /// independent of id ordering and (unlike the pre-502 HashMap) of
    /// per-process hash state. Winner scans tie-break by first-seen;
    /// registration order is the author-curated tie-break priority.
    #[test]
    fn iter_yields_registration_order() {
        let mut registry = RecipeRegistry::default();
        registry.insert(sample_recipe("z.last"));
        registry.insert(sample_recipe("a.first"));
        registry.insert(sample_recipe("m.middle"));
        let ids: Vec<&str> = registry.iter().map(|r| r.id.0).collect();
        assert_eq!(ids, vec!["z.last", "a.first", "m.middle"]);
        // Keyed lookup unaffected by position.
        assert_eq!(registry.get(RecipeId("a.first")).unwrap().id.0, "a.first");
        assert_eq!(registry.get(RecipeId("m.middle")).unwrap().id.0, "m.middle");
    }

    // -----------------------------------------------------------------
    // is_phase5_unlocked (366 — 016 Phase 5 precursor)
    // -----------------------------------------------------------------

    use crate::ai::aspirations::SkillKind;

    fn gated_recipe(id: &'static str, skill: SkillKind, level: f32) -> Recipe {
        let mut r = sample_recipe(id);
        r.skill_gate = Some((skill, level));
        r
    }

    #[test]
    fn is_phase5_unlocked_false_when_no_skill_gated_recipes() {
        // 365-era recipes carry `skill_gate: None`; the predicate
        // returns false regardless of how skilled the colony is.
        let mut registry = RecipeRegistry::default();
        registry.insert(sample_recipe("herbcraft.healing_poultice"));
        let mut paragon = Skills::default();
        paragon.weaving = 5.0;
        paragon.bone_shaping = 5.0;
        assert!(!registry.is_phase5_unlocked([&paragon]));
    }

    #[test]
    fn is_phase5_unlocked_false_when_skill_below_threshold() {
        let mut registry = RecipeRegistry::default();
        registry.insert(gated_recipe(
            "phase5.test_tapestry",
            SkillKind::Weaving,
            1.0,
        ));
        let mut novice = Skills::default();
        novice.weaving = 0.5;
        assert!(!registry.is_phase5_unlocked([&novice]));
    }

    #[test]
    fn is_phase5_unlocked_true_when_some_cat_clears() {
        let mut registry = RecipeRegistry::default();
        registry.insert(gated_recipe(
            "phase5.test_tapestry",
            SkillKind::Weaving,
            1.0,
        ));
        let mut novice = Skills::default();
        novice.weaving = 0.5;
        let mut master = Skills::default();
        master.weaving = 1.5;
        assert!(registry.is_phase5_unlocked([&novice, &master]));
    }

    #[test]
    fn is_phase5_unlocked_checks_named_axis_not_total() {
        // The predicate must read `SkillKind::Weaving` specifically;
        // a cat with high `bone_shaping` does not unlock a weaving-
        // gated recipe.
        let mut registry = RecipeRegistry::default();
        registry.insert(gated_recipe(
            "phase5.test_tapestry",
            SkillKind::Weaving,
            1.0,
        ));
        let mut bone_master = Skills::default();
        bone_master.bone_shaping = 2.0;
        assert!(!registry.is_phase5_unlocked([&bone_master]));
    }
}
