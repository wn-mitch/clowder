// Recipe substrate (ticket 365 — 016 Phase 1a).
//
// Typed data layer shared by every crafting discipline. Bespoke
// resolvers (`resolve_prepare_remedy`, `resolve_set_ward`,
// `resolve_cook`, etc.) read recipe data from `RecipeRegistry`;
// HTN methods cite recipes by `RecipeId` when emitting craft
// intentions. The data layer is unified; the execution layer
// stays bespoke per discipline (cooking flavor composition,
// ward misfire rolls, herbcraft skill growth all live on their
// own resolvers).
//
// Not a Component — these types are pure data carried by the
// `RecipeRegistry` resource. The marker that *tags* a produced
// item as a `CraftedItem` lives in `components::markers`.

use bevy_ecs::prelude::*;

use crate::ai::aspirations::SkillKind;
use crate::components::items::ItemKind;

/// Stable identifier for a recipe. Stringly-typed so registry
/// entries are greppable across module boundaries; the registry
/// keys on this for `O(1)` lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct RecipeId(pub &'static str);

/// What a recipe needs as input.
///
/// `ItemKind` is the canonical item identity (already covers
/// every herb, every raw food, every material). A future
/// "consumes any-of-{kinds}" shape (e.g. "any fuel") would
/// extend this with a new variant rather than adding a flag
/// field — keep the enum closed at the call site so resolvers
/// pattern-match exhaustively.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RecipeInput {
    pub kind: ItemKind,
    pub count: u32,
}

/// Where the produced item lands.
///
/// Three destinations cover every Phase 1–5 output in
/// `docs/systems/crafting.md`:
///   - `Inventory`     — carried by the crafter (remedy, food, gifts).
///   - `EquippedSlot`  — donned via `slot-inventory.md` (wearables, Phase 3).
///   - `WorldPosition` — placed at a tile (wards, decorations, Phase 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ItemDestination {
    Inventory,
    /// Phase 3 wearables. `SlotKind` doesn't exist yet
    /// (`slot-inventory.md` ships with Phase 3); for now the
    /// destination is opaque — resolvers that produce wearables
    /// will pattern-match `EquippedSlot` and route to the
    /// slot-inventory API once it lands.
    EquippedSlot,
    /// Spawned as a world entity at the crafter's position (or
    /// at a recipe-chosen tile). Wards and Phase 4 decorations.
    WorldPosition,
}

/// What station the recipe needs nearby (eligibility precondition).
///
/// The bespoke DSE / planner already gates station presence
/// today via per-discipline markers (`CanCook`, `CanWard`,
/// `NearestKitchen`, etc.). This field is metadata — it lets
/// future tooling answer "which recipes need a Workshop?"
/// without grepping per-discipline scoring code. Resolvers do
/// not currently consult it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StationRequirement {
    /// No station needed (e.g. ward setting at perimeter, flint
    /// blade on open ground).
    None,
    /// Workshop — current remedy_prep + planned Phase 2/3 venue.
    Workshop,
    /// Kitchen — Cooking discipline.
    Kitchen,
    /// Drying Rack — Phase 1b preservation.
    DryingRack,
    /// Smoking Rack — Phase 1b preservation.
    SmokingRack,
    /// Tanning Frame — Phase 2b hide work (extends Drying Rack).
    TanningFrame,
}

/// How long the recipe takes to execute.
///
/// Tick budgets are stored as raw ticks; the per-discipline
/// resolver converts via `TimeScale` if it cares about
/// seasonal speed. Most recipes have a single budget; remedy
/// preparation has a faster path when the crafter is at a
/// Workshop (10t vs 15t), so the enum carries both shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RecipeDuration {
    Fixed {
        ticks: u64,
    },
    AtStationFaster {
        default_ticks: u64,
        at_station_ticks: u64,
    },
}

/// Discipline that owns the recipe. Cosmetic / filterable
/// metadata — every recipe belongs to exactly one discipline,
/// which maps 1:1 with the `DispositionKind` split landed in
/// tickets 155/172.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DisciplineKind {
    Herbalism,
    Witchcraft,
    Cooking,
    /// 367 Phase 1b — preservation. Drying Rack + Smoking Rack
    /// pipelines. Maps 1-to-many onto the three new dispositions
    /// (`DryingFood`, `SmokingMeat`, `TendingSmokingRack`) because
    /// preservation is one discipline but three execution shapes
    /// (load-and-leave drying, load-then-tend smoking, tend-cycles).
    Preservation,
    // Future disciplines (Phase 2/3/4/5): FiberWeaving, BoneShellCraft,
    // HidePeltWork, PigmentMark, StonecraftCairn, AdornmentSetting.
}

/// What a recipe produces. Always a real item — the crafting
/// pillar "items are real" precludes a "virtual output" variant.
/// `destination` tells the resolver where to put it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RecipeOutput {
    pub item_kind: ItemKind,
    pub destination: ItemDestination,
}

/// A recipe — typed data the registry holds, the HTN method
/// cites by id, and the per-discipline resolver reads at
/// runtime.
///
/// Not deserializable: recipes are authored in Rust code via
/// `populate_recipe_registry`, never loaded from external data.
/// `RecipeId` carries `&'static str` keys, which can't round-trip
/// through serde without leaking.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Recipe {
    pub id: RecipeId,
    pub discipline: DisciplineKind,
    pub inputs: Vec<RecipeInput>,
    pub station: StationRequirement,
    pub duration: RecipeDuration,
    pub output: RecipeOutput,
    /// OSRS-style skill gate (366 — 016 Phase 5 precursor).
    ///
    /// `None` = no skill requirement (Phase ≤4 recipes; e.g. the 365-era
    /// herbcraft remedies). `Some((skill, level))` = at least one cat
    /// in the colony must clear `level` on the named axis for the
    /// recipe to be unlocked. Read by
    /// [`crate::resources::recipe_registry::RecipeRegistry::is_phase5_unlocked`].
    ///
    /// 372 lands the first Phase 5 recipes (Generational Tapestry,
    /// Shrine-Cairn, Bone-Lattice Lantern, Pigment-Deepened Textile)
    /// with `Some(...)` gates on the matching mastery axes. Until
    /// then no recipe carries a `Some` value and the predicate
    /// returns false.
    pub skill_gate: Option<(SkillKind, f32)>,
}

/// Provenance metadata attached to every item produced by a
/// crafting recipe. Carries the recipe id, the crafter, and the
/// tick the item was made. Phase 1a attaches this to spawned
/// `Ward` entities; Phase 1b+ attaches it to inventory items
/// (preservation outputs) and to placed decorations.
///
/// Per `docs/systems/crafting.md` "items are not stat sticks":
/// this Component carries narrative/identity data ONLY — no
/// generic numeric modifier fields. Effects live on action
/// resolvers keyed to item identity, never as bolted-on
/// numeric bonuses here.
///
/// Not deserializable for the same reason as [`Recipe`] —
/// `RecipeId` carries `&'static str` keys. `CraftedItem` is
/// attached at spawn time by the producing resolver; if it ever
/// needs to round-trip through `persistence::save_world`, the
/// crafter (`Option<Entity>`) wouldn't survive anyway (entity
/// handles aren't stable across save/load).
#[derive(Component, Debug, Clone, PartialEq, serde::Serialize)]
pub struct CraftedItem {
    pub recipe: RecipeId,
    /// The cat that crafted the item.
    pub crafter: Option<Entity>,
    pub crafted_at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_id_is_value_typed() {
        let a = RecipeId("remedy.healing_poultice");
        let b = RecipeId("remedy.healing_poultice");
        assert_eq!(a, b);
    }

    #[test]
    fn recipe_output_inventory_vs_world() {
        let inv = RecipeOutput {
            item_kind: ItemKind::HerbHealingMoss,
            destination: ItemDestination::Inventory,
        };
        let world = RecipeOutput {
            item_kind: ItemKind::HerbThornbriar,
            destination: ItemDestination::WorldPosition,
        };
        assert_ne!(inv.destination, world.destination);
    }

    #[test]
    fn recipe_duration_at_station_holds_both_budgets() {
        let dur = RecipeDuration::AtStationFaster {
            default_ticks: 15,
            at_station_ticks: 10,
        };
        match dur {
            RecipeDuration::AtStationFaster {
                default_ticks,
                at_station_ticks,
            } => {
                assert!(at_station_ticks < default_ticks);
            }
            RecipeDuration::Fixed { .. } => unreachable!(),
        }
    }
}
