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
/// Stored as a `BTreeMap` (not `HashMap`) so `iter()` yields a stable,
/// process-independent order — ticket 502. `HashMap`'s per-process
/// `RandomState` made recipe iteration order differ across runs of the
/// same binary; `emit_have_item_row`'s winner scan breaks score ties by
/// first-seen, so byte-equal-scoring recipes (the remedy trio) flipped
/// between processes and broke soak-scale byte-identity. Same
/// determinism precedent as `Relationships` (relationships.rs).
#[derive(Resource, Default, Debug, Clone)]
pub struct RecipeRegistry {
    recipes: BTreeMap<RecipeId, Recipe>,
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

    /// 462: find the recipe whose output is `item`. Used by the
    /// `decompose_goal_have_item` HTN substrate helper to turn a
    /// `GoalKind::HaveItem(item)` Intention into a craft plan.
    ///
    /// Linear scan over registered recipes (≤50 today). If multiple
    /// recipes share an output kind (none today; flagged for revisit
    /// if a future Phase ≥2 recipe lands a second producer), this
    /// returns the match with the lexicographically-smallest id
    /// (deterministic — ticket 502).
    pub fn recipe_producing(&self, item: crate::components::items::ItemKind) -> Option<&Recipe> {
        self.iter().find(|r| r.output.item_kind == item)
    }

    /// Iterate recipes in ascending `RecipeId` order (stable across
    /// processes — ticket 502; callers may tie-break by first-seen).
    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
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

    /// Ticket 502 — iteration order must be ascending by RecipeId
    /// regardless of insertion order. Winner scans tie-break by
    /// first-seen; a process-dependent order here breaks cross-process
    /// determinism at the first byte-equal score tie.
    #[test]
    fn iter_yields_ascending_id_order_regardless_of_insertion_order() {
        let mut registry = RecipeRegistry::default();
        registry.insert(sample_recipe("z.last"));
        registry.insert(sample_recipe("a.first"));
        registry.insert(sample_recipe("m.middle"));
        let ids: Vec<&str> = registry.iter().map(|r| r.id.0).collect();
        assert_eq!(ids, vec!["a.first", "m.middle", "z.last"]);
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
