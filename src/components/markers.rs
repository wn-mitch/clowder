//! Context-tag marker components — §4 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Mark's "context tags" are categorical filters: a DSE is either
//! eligible to score (all required tags present, no forbidden tags
//! present) or skipped entirely. Clowder's collapse (§4 prose): **Mark
//! context tags + Bevy ECS components + our current `ScoringContext`
//! booleans are the same concept in three vocabularies.** All three
//! become ECS marker components inserted/removed by per-tick systems;
//! DSE eligibility becomes `Query<With<A>, Without<B>>` — a first-class
//! ECS operation instead of a per-tick `if` statement.
//!
//! **Phase 3a scope:** define the marker *structs* only. The authoring
//! systems that insert/remove them live per §4.6's roster and land in
//! Phase 3d's gap-fill (the refactor plan pairs roster gap-fill with
//! the faction matrix landing). DSEs in Phase 3c consume these markers
//! via `EligibilityFilter::require("MarkerName")` against the marker
//! lookup registry.
//!
//! **Markers not defined here** — already exist in the tree; keep
//! them at their current home to avoid churning existing consumers:
//!
//! - `Species` (`identity.rs:17`) — to be renamed to `Cat` under the
//!   §4.3 Species category in a later pass (query-disjointness win).
//! - `PreyAnimal` (`prey.rs:130`) — as `Species` above; proposed rename
//!   to `Prey`.
//! - `Coordinator` (`coordination.rs:14`) — already a ZST marker.
//! - `Pregnant` (`pregnancy.rs:17`) — data-bearing component; serves
//!   marker duty via `With<Pregnant>`.
//! - `Dead` (`death.rs:72`) — data-bearing component with marker usage.
//! - `FateAssigned` (`fate.rs:49`) — already a ZST.
//! - `AspirationsInitialized` (`aspirations.rs:139`) — already a ZST.
//!
//! **Deferred to Phase 3c** (ship with the consumers that need them):
//!
//! - `Fertility { phase, cycle_offset, post_partum_remaining_ticks }`
//!   — §7.M.7 lifecycle; data-bearing, authored by a new
//!   `src/systems/fertility.rs`. Lands with `MateWithGoal` DSE.
//!
//! **Species renames** (`Fox`, `Hawk`, `Snake`, `ShadowFox` as ZSTs
//! alongside the current `WildAnimal.species` enum) are also
//! deferred — query-disjointness is a separate, cross-cutting port
//! per §4.3's "Partial → Built" row set.

use bevy_ecs::prelude::*;

// ---------------------------------------------------------------------------
// Role markers (§4.3 Role)
// ---------------------------------------------------------------------------

/// Cat is the mentor side of a `Training { mentor, apprentice }`
/// relationship. Authoring: `aspirations.rs::update_training_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Mentor;

/// Cat is the apprentice side of a `Training` relationship.
/// Authoring: as `Mentor`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Apprentice;

// ---------------------------------------------------------------------------
// LifeStage markers (§4.3 LifeStage — replace Age::stage() hot call)
// ---------------------------------------------------------------------------

/// `Age::stage() == Kitten` (0–3 seasons). Authoring:
/// `growth.rs::update_life_stage_markers` — one marker mutually
/// exclusive per cat.
#[derive(Component, Debug, Clone, Copy)]
pub struct Kitten;
impl Kitten {
    pub const KEY: &str = "Kitten";
}

/// `Age::stage() == Young` (4–11 seasons).
#[derive(Component, Debug, Clone, Copy)]
pub struct Young;
impl Young {
    pub const KEY: &str = "Young";
}

/// `Age::stage() == Adult` (12–59 seasons).
#[derive(Component, Debug, Clone, Copy)]
pub struct Adult;
impl Adult {
    pub const KEY: &str = "Adult";
}

/// `Age::stage() == Elder` (60+ seasons).
#[derive(Component, Debug, Clone, Copy)]
pub struct Elder;
impl Elder {
    pub const KEY: &str = "Elder";
}

// ---------------------------------------------------------------------------
// State markers (§4.3 State)
// ---------------------------------------------------------------------------

/// Severe unhealed injury — downed.
/// `systems::incapacitation::update_incapacitation`. Used as the
/// eligibility gate that retires the §2.3 incapacitated branch:
/// `Q<_, With<Incapacitated>>` picks the narrow DSE set (Eat, Sleep,
/// Idle); every other DSE uses `Without<Incapacitated>`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Incapacitated;
impl Incapacitated {
    pub const KEY: &str = "Incapacitated";
}

/// Any injury present — weaker than `Incapacitated`.
/// `needs.rs::update_injury_marker`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Injured;

/// Cat is in an active combat step or hostile-adjacent.
/// `combat.rs::update_combat_marker`.
#[derive(Component, Debug, Clone, Copy)]
pub struct InCombat;

/// Tile under cat has corruption > threshold.
/// `magic.rs::update_corrupted_tile_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct OnCorruptedTile;

/// Tile under cat is `FairyRing` or `StandingStone`.
/// `sensing.rs::update_terrain_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct OnSpecialTerrain;

/// ≥1 wildlife hostile within species-attenuated detection range.
/// `sensing.rs::update_threat_proximity_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasThreatNearby;

// ---------------------------------------------------------------------------
// Capability markers (§4.3 Capability — derived per-tick from parent tags)
// ---------------------------------------------------------------------------

/// Authoring for all four: `src/ai/capabilities.rs::update_capability_markers`
/// (new file in Phase 3d). Predicates are conjunctions over life-stage,
/// injury state, inventory, and nearby-tile checks — see §4.3 rows.
#[derive(Component, Debug, Clone, Copy)]
pub struct CanHunt;
impl CanHunt {
    pub const KEY: &str = "CanHunt";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CanForage;

#[derive(Component, Debug, Clone, Copy)]
pub struct CanWard;

#[derive(Component, Debug, Clone, Copy)]
pub struct CanCook;

// ---------------------------------------------------------------------------
// Inventory markers (§4.3 Inventory — per-cat)
// ---------------------------------------------------------------------------

/// Authoring: `items.rs::update_inventory_markers`
/// (with `Changed<Inventory>` filter for per-tick cost).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasHerbsInInventory;

#[derive(Component, Debug, Clone, Copy)]
pub struct HasRemedyHerbs;

#[derive(Component, Debug, Clone, Copy)]
pub struct HasWardHerbs;

// ---------------------------------------------------------------------------
// Colony singleton
// ---------------------------------------------------------------------------

/// Marker for the single colony-state entity. Phase 3a introduces the
/// type; the spawn path attaches exactly one entity with this marker
/// in Phase 3d. Colony-scoped markers below (ThornbriarAvailable,
/// HasFunctionalKitchen, …) attach to this entity so DSE queries
/// joining cat + colony state use `(cat_q, colony_q.single())`.
#[derive(Component, Debug, Clone, Copy)]
pub struct ColonyState;

// ---------------------------------------------------------------------------
// Inventory markers — colony-scoped (§4.3 Inventory on ColonyState)
// ---------------------------------------------------------------------------

/// Authoring: `buildings.rs::update_colony_building_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFunctionalKitchen;
impl HasFunctionalKitchen {
    pub const KEY: &str = "HasFunctionalKitchen";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HasRawFoodInStores;
impl HasRawFoodInStores {
    pub const KEY: &str = "HasRawFoodInStores";
}

/// Colony stores carry ≥1 food item (raw or cooked). Gates `Eat`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasStoredFood;
impl HasStoredFood {
    pub const KEY: &str = "HasStoredFood";
}

/// ≥1 harvestable Thornbriar exists in the world.
/// `magic.rs::update_herb_availability_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct ThornbriarAvailable;

// ---------------------------------------------------------------------------
// TargetExistence markers (§4.3 TargetExistence — gates target-taking DSEs)
// ---------------------------------------------------------------------------

/// Broad-phase "is there anything worth scoring targets against?"
/// Authored by `sensing.rs::update_target_existence_markers`.

#[derive(Component, Debug, Clone, Copy)]
pub struct HasSocialTarget;

#[derive(Component, Debug, Clone, Copy)]
pub struct HasHerbsNearby;

/// Shared between cats and foxes via `With<Prey>` + distance.
#[derive(Component, Debug, Clone, Copy)]
pub struct PreyNearby;

#[derive(Component, Debug, Clone, Copy)]
pub struct CarcassNearby;

/// `buildings.rs::update_colony_building_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasConstructionSite;

#[derive(Component, Debug, Clone, Copy)]
pub struct HasDamagedBuilding;

#[derive(Component, Debug, Clone, Copy)]
pub struct HasGarden;
impl HasGarden {
    pub const KEY: &str = "HasGarden";
}

/// ≥1 other cat has a skill below 0.3 where this cat has the same
/// skill above 0.6 (per-cat relative predicate).
/// `aspirations.rs::update_mentoring_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasMentoringTarget;

/// Orientation-compatible partner with Partners+ bond exists.
/// `mating.rs::update_mate_eligibility_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasEligibleMate;

/// Cat is the parent side of a `KittenDependency` whose kitten's
/// hunger exceeds threshold.
/// `growth.rs::update_parent_hungry_kitten_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct IsParentOfHungryKitten;

// ---------------------------------------------------------------------------
// Colony markers (§4.3 Colony)
// ---------------------------------------------------------------------------

/// Per-coordinator-cat, not on `ColonyState`:
/// `With<Coordinator> + DirectiveQueue.len() > 0`.
/// `coordination.rs::update_directive_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct IsCoordinatorWithDirectives;

/// Colony ward coverage: no wards OR average strength < 0.3.
/// `magic.rs::update_ward_coverage_markers`. Attaches to `ColonyState`.
#[derive(Component, Debug, Clone, Copy)]
pub struct WardStrengthLow;
impl WardStrengthLow {
    pub const KEY: &str = "WardStrengthLow";
}

/// Any colony ward has `WildlifeAiState::EncirclingWard` adjacent.
/// `magic.rs::update_ward_siege_marker`. Attaches to `ColonyState`.
#[derive(Component, Debug, Clone, Copy)]
pub struct WardsUnderSiege;

// ---------------------------------------------------------------------------
// Reproduction markers (§4.3 Reproduction)
// ---------------------------------------------------------------------------

/// **Active parenthood** (not lifetime identity) — cat has ≥1 living
/// entity with `KittenDependency.mother == self` or `…father == self`.
/// Removed when the last dependent kitten matures or dies. See §4.3
/// prose on the ordering hazard: grief consumers MUST NOT infer
/// grief-parent status from `With<Parent>` on survivors post-death.
/// The canonical parent-at-time-of-death channel is the future
/// `CatDied.survivors_by_relationship` event payload.
///
/// Authoring: `growth.rs::update_parent_markers` (new). Insert/remove
/// in a single tick pass over `Query<&KittenDependency>`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Parent;

// Note: `Fertility { phase, … }` is data-bearing (§7.M.7); lands in
// Phase 3c alongside the MateWithGoal DSE, not here.

// ---------------------------------------------------------------------------
// Fox-specific markers (§4.3 Fox-specific)
// ---------------------------------------------------------------------------

/// Authoring: `fox_spatial.rs::update_store_awareness_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct StoreVisible;

#[derive(Component, Debug, Clone, Copy)]
pub struct StoreGuarded;

/// Cat within 5 tiles of fox's den AND cubs present.
/// `fox_spatial.rs::update_den_threat_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct CatThreateningDen;

/// Ward within fox detection radius (stubbed in
/// `FoxScoringContext.ward_nearby` today — promote to ECS marker).
/// `fox_spatial.rs::update_ward_detection_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct WardNearbyFox;

/// Fox has ≥1 cub at its den. Event-driven (`CubsBorn` +
/// on-despawn cleanup).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasCubs;

/// `cub_satiation < 0.4`. `fox_goap.rs::update_cub_hunger_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct CubsHungry;

/// Juvenile fox with no home den (dispersal eligibility).
/// `fox_goap.rs::update_juvenile_dispersal_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct IsDispersingJuvenile;

/// Fox has a home den. Event-driven (`DenClaimed` / `DenLost`).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasDen;

// ---------------------------------------------------------------------------
// §9.2 Faction overlay markers
// ---------------------------------------------------------------------------

/// Non-colony cat present on the map (Wandering Loner / Trader /
/// Scout per `docs/systems/trade.md`). Observer-Cat × target-Cat:
/// demote `Same` → `Neutral`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Visitor;

/// Hostile-Loner variant. Observer-Cat × target-Cat: demote
/// `Same` → `Enemy`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HostileVisitor;

/// Cat exiled from the colony. Observer-Cat × target-Cat: demote
/// `Same` → `Enemy`. See §9.2 — today's `combat.rs::pending_banishments`
/// path is shadowfox-only; extending to cat-on-cat lands in Phase 3d.
#[derive(Component, Debug, Clone, Copy)]
pub struct Banished;

/// Fox or prey-species target befriended through repeated non-hostile
/// contact. Observer-Cat × target-Fox: upgrade `Predator` → `Ally`
/// (reciprocal on fox: `Prey` → `Ally`). `social.rs::befriend_wildlife`.
#[derive(Component, Debug, Clone, Copy)]
pub struct BefriendedAlly;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! Markers are ZSTs carrying no runtime state. These tests exist
    //! to catch accidental deletion / typo regressions — if a marker's
    //! name changes and the change isn't cross-referenced against the
    //! `EligibilityFilter::require("name")` strings in each DSE,
    //! eligibility silently fails.
    //!
    //! The test strategy: insert each marker onto a fresh entity, then
    //! query `With<Marker>` and confirm the entity comes back. This
    //! validates that every marker derives `Component` correctly.

    use super::*;

    fn assert_marker_queryable<M: Component + Copy>(marker: M) {
        let mut world = World::new();
        let entity = world.spawn(marker).id();
        let mut q = world.query_filtered::<Entity, With<M>>();
        let collected: Vec<Entity> = q.iter(&world).collect();
        assert_eq!(collected, vec![entity]);
    }

    #[test]
    fn role_markers_queryable() {
        assert_marker_queryable(Mentor);
        assert_marker_queryable(Apprentice);
    }

    #[test]
    fn life_stage_markers_queryable() {
        assert_marker_queryable(Kitten);
        assert_marker_queryable(Young);
        assert_marker_queryable(Adult);
        assert_marker_queryable(Elder);
    }

    #[test]
    fn state_markers_queryable() {
        assert_marker_queryable(Incapacitated);
        assert_marker_queryable(Injured);
        assert_marker_queryable(InCombat);
        assert_marker_queryable(OnCorruptedTile);
        assert_marker_queryable(OnSpecialTerrain);
        assert_marker_queryable(HasThreatNearby);
    }

    #[test]
    fn capability_markers_queryable() {
        assert_marker_queryable(CanHunt);
        assert_marker_queryable(CanForage);
        assert_marker_queryable(CanWard);
        assert_marker_queryable(CanCook);
    }

    #[test]
    fn inventory_markers_queryable() {
        assert_marker_queryable(HasHerbsInInventory);
        assert_marker_queryable(HasRemedyHerbs);
        assert_marker_queryable(HasWardHerbs);
        assert_marker_queryable(HasFunctionalKitchen);
        assert_marker_queryable(HasRawFoodInStores);
        assert_marker_queryable(HasStoredFood);
        assert_marker_queryable(ThornbriarAvailable);
    }

    #[test]
    fn target_existence_markers_queryable() {
        assert_marker_queryable(HasSocialTarget);
        assert_marker_queryable(HasHerbsNearby);
        assert_marker_queryable(PreyNearby);
        assert_marker_queryable(CarcassNearby);
        assert_marker_queryable(HasConstructionSite);
        assert_marker_queryable(HasDamagedBuilding);
        assert_marker_queryable(HasGarden);
        assert_marker_queryable(HasMentoringTarget);
        assert_marker_queryable(HasEligibleMate);
        assert_marker_queryable(IsParentOfHungryKitten);
    }

    #[test]
    fn colony_markers_queryable() {
        assert_marker_queryable(ColonyState);
        assert_marker_queryable(IsCoordinatorWithDirectives);
        assert_marker_queryable(WardStrengthLow);
        assert_marker_queryable(WardsUnderSiege);
    }

    #[test]
    fn reproduction_markers_queryable() {
        assert_marker_queryable(Parent);
    }

    #[test]
    fn fox_specific_markers_queryable() {
        assert_marker_queryable(StoreVisible);
        assert_marker_queryable(StoreGuarded);
        assert_marker_queryable(CatThreateningDen);
        assert_marker_queryable(WardNearbyFox);
        assert_marker_queryable(HasCubs);
        assert_marker_queryable(CubsHungry);
        assert_marker_queryable(IsDispersingJuvenile);
        assert_marker_queryable(HasDen);
    }

    #[test]
    fn faction_overlay_markers_queryable() {
        assert_marker_queryable(Visitor);
        assert_marker_queryable(HostileVisitor);
        assert_marker_queryable(Banished);
        assert_marker_queryable(BefriendedAlly);
    }
}
