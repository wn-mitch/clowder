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
impl Mentor {
    pub const KEY: &str = "Mentor";
}

/// Cat is the apprentice side of a `Training` relationship.
/// Authoring: as `Mentor`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Apprentice;
impl Apprentice {
    pub const KEY: &str = "Apprentice";
}

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
impl Injured {
    pub const KEY: &str = "Injured";
}

/// HP ratio at or below `DispositionConstants::critical_health_threshold`.
/// Authoring: `interoception::author_self_markers` — fires *before* the
/// disposition-layer critical-health interrupt at the same threshold so
/// DSE scoring can elect Flee or Rest before the interrupt's panic-fallback.
/// Ticket 087.
#[derive(Component, Debug, Clone, Copy)]
pub struct LowHealth;
impl LowHealth {
    pub const KEY: &str = "LowHealth";
}

/// At least one unhealed `InjuryKind::Severe` injury.
/// Authoring: `interoception::author_self_markers`. Ticket 087.
#[derive(Component, Debug, Clone, Copy)]
pub struct SevereInjury;
impl SevereInjury {
    pub const KEY: &str = "SevereInjury";
}

/// Composite body-distress: hunger, energy, thermal, or health deficit
/// above `DispositionConstants::body_distress_threshold`. The unified
/// "I am unwell" perception — analog of how external perception's
/// `HasThreatNearby` is a unified "I am in danger" signal across many
/// possible threats. Authoring: `interoception::author_self_markers`.
/// Ticket 087.
#[derive(Component, Debug, Clone, Copy)]
pub struct BodyDistressed;
impl BodyDistressed {
    pub const KEY: &str = "BodyDistressed";
}

/// Mean skill level across all six `Skills` fields below
/// `DispositionConstants::low_mastery_threshold`. The cat's
/// felt-competence is meaningfully low — drives future
/// "seek-mastery" / "pursue-practice" DSEs. Note: fires for
/// all freshly spawned cats (default mean ~0.07) and clears as
/// skills grow past the threshold. Authoring:
/// `interoception::author_self_markers`. Ticket 090.
#[derive(Component, Debug, Clone, Copy)]
pub struct LowMastery;
impl LowMastery {
    pub const KEY: &str = "LowMastery";
}

/// No active aspiration (`Aspirations::active.is_empty()` or no
/// `Aspirations` component). The cat has no directed striving —
/// drives future "adopt-aspiration" / "pursue-purpose" DSEs.
/// Authoring: `interoception::author_self_markers`. Ticket 090.
#[derive(Component, Debug, Clone, Copy)]
pub struct LackingPurpose;
impl LackingPurpose {
    pub const KEY: &str = "LackingPurpose";
}

/// Max of L4 deficits — `max(1 - respect, 1 - mastery)` exceeds
/// `DispositionConstants::esteem_distressed_threshold`. Parallels
/// `BodyDistressed` for the esteem tier: the unified "I feel
/// undervalued or incompetent" signal. Authoring:
/// `interoception::author_self_markers`. Ticket 090.
#[derive(Component, Debug, Clone, Copy)]
pub struct EsteemDistressed;
impl EsteemDistressed {
    pub const KEY: &str = "EsteemDistressed";
}

/// Cat is in an active combat step or hostile-adjacent.
/// `combat.rs::update_combat_marker`.
#[derive(Component, Debug, Clone, Copy)]
pub struct InCombat;
impl InCombat {
    pub const KEY: &str = "InCombat";
}

/// Tile under cat has corruption > threshold.
/// `magic.rs::update_corrupted_tile_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct OnCorruptedTile;
impl OnCorruptedTile {
    pub const KEY: &str = "OnCorruptedTile";
}

/// Tile under cat is `FairyRing` or `StandingStone`.
/// `sensing.rs::update_terrain_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct OnSpecialTerrain;
impl OnSpecialTerrain {
    pub const KEY: &str = "OnSpecialTerrain";
}

/// ≥1 wildlife hostile within species-attenuated detection range.
/// `sensing.rs::update_threat_proximity_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasThreatNearby;
impl HasThreatNearby {
    pub const KEY: &str = "HasThreatNearby";
}

/// Ticket 104 — Hide/Freeze DSE eligibility gate. Authored when the
/// cat has a threat in sight AND a low-cover tile within sprint range
/// (the "remain still and hope" predator-response valence is viable
/// here — fleeing is too risky, fighting unwinnable). **Phase 1: no
/// authoring system exists** — the marker is defined so the DSE can
/// gate against it, but never fires until a Phase-2/3 authoring system
/// lands alongside the 105 modifier's lift activation. With the marker
/// never authored, Hide is never eligible, so the DSE is dormant and
/// score-bit-identical to baseline.
#[derive(Component, Debug, Clone, Copy)]
pub struct HideEligible;
impl HideEligible {
    pub const KEY: &str = "HideEligible";
}

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
impl CanForage {
    pub const KEY: &str = "CanForage";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CanWard;
impl CanWard {
    pub const KEY: &str = "CanWard";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CanCook;
impl CanCook {
    pub const KEY: &str = "CanCook";
}

// ---------------------------------------------------------------------------
// Inventory markers (§4.3 Inventory — per-cat)
// ---------------------------------------------------------------------------

/// Authoring: `items.rs::update_inventory_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasHerbsInInventory;
impl HasHerbsInInventory {
    pub const KEY: &str = "HasHerbsInInventory";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HasRemedyHerbs;
impl HasRemedyHerbs {
    pub const KEY: &str = "HasRemedyHerbs";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HasWardHerbs;
impl HasWardHerbs {
    pub const KEY: &str = "HasWardHerbs";
}

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
impl ThornbriarAvailable {
    pub const KEY: &str = "ThornbriarAvailable";
}

/// Per-cat: the nearest reachable construction site has
/// `materials_complete()` true. Gates the substrate branch of the
/// `Construct` GOAP action — when set, the planner can plan
/// `[TravelTo(ConstructionSite), Construct]` directly without a
/// haul leg. Authored each tick from
/// `goap.rs::build_planner_markers` against
/// `ConstructionSite::materials_complete()`. Ticket 096.
#[derive(Component, Debug, Clone, Copy)]
pub struct MaterialsAvailable;
impl MaterialsAvailable {
    pub const KEY: &str = "MaterialsAvailable";
}

// ---------------------------------------------------------------------------
// TargetExistence markers (§4.3 TargetExistence — gates target-taking DSEs)
// ---------------------------------------------------------------------------

/// Broad-phase "is there anything worth scoring targets against?"
/// Authored by `sensing.rs::update_target_existence_markers`.

#[derive(Component, Debug, Clone, Copy)]
pub struct HasSocialTarget;
impl HasSocialTarget {
    pub const KEY: &str = "HasSocialTarget";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HasHerbsNearby;
impl HasHerbsNearby {
    pub const KEY: &str = "HasHerbsNearby";
}

/// Shared between cats and foxes via `With<Prey>` + distance.
#[derive(Component, Debug, Clone, Copy)]
pub struct PreyNearby;
impl PreyNearby {
    pub const KEY: &str = "PreyNearby";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CarcassNearby;
impl CarcassNearby {
    pub const KEY: &str = "CarcassNearby";
}

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
/// `aspirations.rs::update_mentoring_target_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasMentoringTarget;
impl HasMentoringTarget {
    pub const KEY: &str = "HasMentoringTarget";
}

/// Orientation-compatible partner with Partners+ bond exists.
/// `mating.rs::update_mate_eligibility_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasEligibleMate;
impl HasEligibleMate {
    pub const KEY: &str = "HasEligibleMate";
}

/// Cat has at least one living dependent kitten whose hunger has
/// dropped below `kitten_cry_hunger_threshold`. Authored each tick;
/// removed when no own kitten is hungry. The 0/1 substrate signal
/// that lets parents score Caretake even when their kitten is
/// outside the per-tick `CaretakeTargetDse` candidate pool (range
/// gate or hunger-cycle gate). Pairs with the `KittenCryMap` cell
/// sample at the cat's tile — the cry-map is the spatial-perception
/// channel; this marker is the kinship-channel substrate fact.
///
/// Author: `growth.rs::update_parent_hungry_kitten_markers`.
/// Read: `MarkerSnapshot.has(IsParentOfHungryKitten::KEY, entity)`
/// in `disposition.rs` / `goap.rs` populate sites; passed into
/// `caretake_target::resolve_caretake_target` as
/// `parent_marker_active` to enable the own-kitten-anywhere
/// fallback (ticket 158).
#[derive(Component, Debug, Clone, Copy)]
pub struct IsParentOfHungryKitten;
impl IsParentOfHungryKitten {
    pub const KEY: &str = "IsParentOfHungryKitten";
}

// ---------------------------------------------------------------------------
// Colony markers (§4.3 Colony)
// ---------------------------------------------------------------------------

/// Per-coordinator-cat, not on `ColonyState`:
/// `With<Coordinator> + DirectiveQueue.len() > 0`.
/// `coordination.rs::update_directive_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct IsCoordinatorWithDirectives;
impl IsCoordinatorWithDirectives {
    pub const KEY: &str = "IsCoordinatorWithDirectives";
}

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
impl WardsUnderSiege {
    pub const KEY: &str = "WardsUnderSiege";
}

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
impl Parent {
    pub const KEY: &str = "Parent";
}

// Note: `Fertility { phase, … }` is data-bearing (§7.M.7); lands in
// Phase 3c alongside the MateWithGoal DSE, not here.

// ---------------------------------------------------------------------------
// Fox-specific markers (§4.3 Fox-specific)
// ---------------------------------------------------------------------------

/// Authoring: `fox_spatial.rs::update_store_awareness_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct StoreVisible;
impl StoreVisible {
    pub const KEY: &str = "StoreVisible";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct StoreGuarded;
impl StoreGuarded {
    pub const KEY: &str = "StoreGuarded";
}

/// Cat within 5 tiles of fox's den AND cubs present.
/// `fox_spatial.rs::update_den_threat_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct CatThreateningDen;
impl CatThreateningDen {
    pub const KEY: &str = "CatThreateningDen";
}

/// Ward within fox detection radius (stubbed in
/// `FoxScoringContext.ward_nearby` today — promote to ECS marker).
/// `fox_spatial.rs::update_ward_detection_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct WardNearbyFox;
impl WardNearbyFox {
    pub const KEY: &str = "WardNearbyFox";
}

/// Fox has ≥1 cub at its den. Per-tick author scan today
/// (`fox_spatial.rs::update_cub_marker`); event-driven follow-on
/// (`CubsBorn` / on-despawn) deferred to a separate ticket.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasCubs;
impl HasCubs {
    pub const KEY: &str = "HasCubs";
}

/// `cub_satiation < 0.4`. `fox_spatial.rs::update_cub_hunger_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct CubsHungry;
impl CubsHungry {
    pub const KEY: &str = "CubsHungry";
}

/// Juvenile fox with no home den (dispersal eligibility).
/// `fox_spatial.rs::update_juvenile_dispersal_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct IsDispersingJuvenile;
impl IsDispersingJuvenile {
    pub const KEY: &str = "IsDispersingJuvenile";
}

/// Fox has a home den. Per-tick author scan today
/// (`fox_spatial.rs::update_den_marker`); event-driven follow-on
/// (`DenClaimed` / `DenLost`) deferred to a separate ticket.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasDen;
impl HasDen {
    pub const KEY: &str = "HasDen";
}

// ---------------------------------------------------------------------------
// §9.2 Faction overlay markers
// ---------------------------------------------------------------------------

/// Non-colony cat present on the map (Wandering Loner / Trader /
/// Scout per `docs/systems/trade.md`). Observer-Cat × target-Cat:
/// demote `Same` → `Neutral`. Authoritative-on-arrival: the trade
/// subsystem (Aspirational) inserts on spawn / removes on depart;
/// no per-tick author system.
#[derive(Component, Debug, Clone, Copy)]
pub struct Visitor;
impl Visitor {
    pub const KEY: &str = "Visitor";
}

/// Hostile-Loner variant. Observer-Cat × target-Cat: demote
/// `Same` → `Enemy`. Same authoritative-on-arrival lifecycle as
/// `Visitor`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HostileVisitor;
impl HostileVisitor {
    pub const KEY: &str = "HostileVisitor";
}

/// Cat exiled from the colony. Observer-Cat × target-Cat: demote
/// `Same` → `Enemy`. Inserted by `combat.rs::resolve_combat` when a
/// cat appears in the `pending_banishments` list (today's shadowfox
/// path despawns wildlife; the cat-on-cat branch tags rather than
/// despawns). The trigger that pushes a cat onto `pending_banishments`
/// is left to a future ticket.
#[derive(Component, Debug, Clone, Copy)]
pub struct Banished;
impl Banished {
    pub const KEY: &str = "Banished";
}

/// Fox or prey-species target befriended through repeated non-hostile
/// contact. Observer-Cat × target-Fox: upgrade `Predator` → `Ally`
/// (reciprocal on fox: `Prey` → `Ally`). Authored by
/// `social.rs::befriend_wildlife` from a cat ↔ wildlife familiarity
/// threshold.
#[derive(Component, Debug, Clone, Copy)]
pub struct BefriendedAlly;
impl BefriendedAlly {
    pub const KEY: &str = "BefriendedAlly";
}

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
        assert_marker_queryable(LowHealth);
        assert_marker_queryable(SevereInjury);
        assert_marker_queryable(BodyDistressed);
        assert_marker_queryable(LowMastery);
        assert_marker_queryable(LackingPurpose);
        assert_marker_queryable(EsteemDistressed);
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
        assert_marker_queryable(MaterialsAvailable);
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

    #[test]
    fn faction_overlay_marker_keys_unique() {
        let keys = [
            Visitor::KEY,
            HostileVisitor::KEY,
            Banished::KEY,
            BefriendedAlly::KEY,
        ];
        for k in keys {
            assert!(!k.is_empty(), "marker KEY must be non-empty");
        }
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(
                    keys[i], keys[j],
                    "§9.2 marker KEYs must be unique — collision between {} and {}",
                    keys[i], keys[j]
                );
            }
        }
    }

    #[test]
    fn l4_l5_self_perception_marker_keys_unique() {
        let keys = [
            LowHealth::KEY,
            SevereInjury::KEY,
            BodyDistressed::KEY,
            LowMastery::KEY,
            LackingPurpose::KEY,
            EsteemDistressed::KEY,
        ];
        for k in keys {
            assert!(!k.is_empty(), "marker KEY must be non-empty");
        }
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(
                    keys[i], keys[j],
                    "ticket 087/090 self-perception marker KEYs must be unique — collision between {} and {}",
                    keys[i], keys[j]
                );
            }
        }
    }
}
