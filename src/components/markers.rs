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
// LifeStage sub-markers — Kitten decomposition (ticket 450)
// ---------------------------------------------------------------------------
//
// The monolithic `Kitten` stage (0–3 seasons) decomposes ethologically into
// three sub-stages with progressive capabilities. The sub-marker plus
// `Kitten` are co-resident on the same entity for the duration of each
// sub-stage; the three sub-markers themselves are mutually exclusive.
// Author: `growth.rs::update_life_stage_markers` from
// `KittenDependency.maturity` at thresholds 0.33 / 0.67 / 1.0.

/// Sub-stage 1: `Kitten ∧ maturity < 0.33` — newborn, motionless, eyes
/// closed. Co-authored with `Incapacitated` so every existing
/// `.forbid(Incapacitated::KEY)` filter (fetch / forage / hunt / mate /
/// cook / ward / mentor-target / mentor) already excludes them without
/// any per-DSE gate work. Reader: the HTN method `[BegForFood]`'s
/// `ApplicableWhen::Kitten ∧ ¬HasFoodInInventory` plus the
/// `MentorableAge` author (which excludes `NewbornKitten`).
#[derive(Component, Debug, Clone, Copy)]
pub struct NewbornKitten;
impl NewbornKitten {
    pub const KEY: &str = "NewbornKitten";
}

/// Sub-stage 2: `Kitten ∧ 0.33 ≤ maturity < 0.67` — eyes open, mobile,
/// can play / beg / sleep. `Incapacitated` is removed at the Stage 1 →
/// Stage 2 transition. Still excluded from foraging / hunting / mating
/// / mentoring by the `Kitten` marker on existing capability gates.
#[derive(Component, Debug, Clone, Copy)]
pub struct EyesOpenKitten;
impl EyesOpenKitten {
    pub const KEY: &str = "EyesOpenKitten";
}

/// Sub-stage 3: `Kitten ∧ 0.67 ≤ maturity < 1.0` — juvenile, the
/// "mentorable" phase. Co-authored with `MentorableAge`. `CanForage`
/// gate widens to include this stage (juvenile kittens can forage
/// alongside Young / Adult); `CanHunt` stays gated on Young/Adult
/// (Stage 3 kittens learn hunting by mentoring, not by hunting solo).
/// At maturity ≥ 1.0 the `KittenDependency` + `Kitten` markers retire
/// (existing `tick_kitten_growth` semantics).
#[derive(Component, Debug, Clone, Copy)]
pub struct JuvenileKitten;
impl JuvenileKitten {
    pub const KEY: &str = "JuvenileKitten";
}

/// Mentee-side eligibility gate: cat is old enough to absorb mentoring.
/// `JuvenileKitten ∨ Young ∨ Adult`. Newborn / Eyes-open kittens cannot
/// receive mentoring even though they're alive and present. Reader:
/// `src/ai/dses/mentor_target.rs`'s `.require(MentorableAge::KEY)`.
/// Author: `growth.rs::update_life_stage_markers` (co-authored with the
/// life-stage markers themselves so the marker shape stays consistent
/// across the same maturity read).
#[derive(Component, Debug, Clone, Copy)]
pub struct MentorableAge;
impl MentorableAge {
    pub const KEY: &str = "MentorableAge";
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

/// 035: corpse has been buried. Inserted on the deceased entity by
/// `goap.rs::resolve_goap_plans`'s post-loop drain immediately before
/// `commands.entity(...).despawn()`, so a freshly-buried corpse is
/// invisible to `update_target_existence_markers`'s
/// `HasUnburiedCorpse` author scan within the same tick. Defensive
/// against double-fire when two cats path-equally to the same corpse.
/// Read: `sensing.rs::update_target_existence_markers` filters dead
/// cats via `Without<Buried>`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Buried;
impl Buried {
    pub const KEY: &str = "Buried";
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

/// Ticket 084: combined ward-eligibility marker that expands `CanWard`
/// to cover cats who can reach a stashed thornbriar even without
/// currently carrying one. Fires when: `Adult ∧ ¬Injured ∧ (HasWardHerbs
/// ∨ HasStoredThornbriar)`. Reader: the `HerbcraftSetWard` DSE's
/// eligibility filter (replaces the `CanWard::KEY` require). Writer:
/// `capabilities.rs::update_capability_markers` (extended in Commit 2
/// of 084 to take a colony `HasStoredThornbriar` reference). GOAP then
/// composes either `[Travel → SetWard]` (carrying-path) or
/// `[Travel(Stores) → RetrieveHerbs(Thornbriar) → Travel → SetWard]`
/// (retrieve-path) based on which `CarryingIs` precondition holds.
#[derive(Component, Debug, Clone, Copy)]
pub struct CanWardFromSupply;
impl CanWardFromSupply {
    pub const KEY: &str = "CanWardFromSupply";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CanCook;
impl CanCook {
    pub const KEY: &str = "CanCook";
}

/// 367: per-cat capability — `Adult ∧ ¬Injured`, mirrors `CanCook`.
/// Gates `DryFoodDse`. Colony-scoped station availability stays on the
/// DSE eligibility filter so a "wants to dry but no rack" latent signal
/// could later flow into BuildPressure (paralleling the
/// `wants_cook_but_no_kitchen` pattern in `scoring.rs`).
#[derive(Component, Debug, Clone, Copy)]
pub struct CanDry;
impl CanDry {
    pub const KEY: &str = "CanDry";
}

/// 367: per-cat capability — `Adult ∧ ¬Injured`, mirrors `CanCook`.
/// Gates `SmokeMeatDse` (which loads a rack) and `TendSmokingRackDse`
/// (which advances per-rack progress one tend at a time).
#[derive(Component, Debug, Clone, Copy)]
pub struct CanSmoke;
impl CanSmoke {
    pub const KEY: &str = "CanSmoke";
}

/// 457: per-cat capability — `Adult ∧ ¬Injured`, mirrors `CanCook` /
/// `CanDry` / `CanSmoke`. Gates `CraftAtWorkshopDse` (the elect-side
/// pipeline that lets cats autonomously craft the 368 Phase 2 behavioral
/// tools — Grooming Brush, Play Bundle, Courtship Gift — and the
/// Polished Stone intermediate). Colony-scoped Workshop availability
/// stays on the DSE eligibility filter via `HasFunctionalWorkshop`.
#[derive(Component, Debug, Clone, Copy)]
pub struct CanCraft;
impl CanCraft {
    pub const KEY: &str = "CanCraft";
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

/// 450: per-cat — `inventory.has_food()` (any slot holds an item kind
/// classified as food, raw or cooked). Authored by
/// `items::update_inventory_markers`. Read by:
/// - the HTN method `[BegForFood]`'s `ApplicableWhen::Kitten ∧
///   ¬HasFoodInInventory` (a kitten with food doesn't beg);
/// - 429 Phase 2's `EatFromOwnInventoryDse` eligibility filter
///   (`.require(HasFoodInInventory::KEY).forbid(Incapacitated::KEY)`).
///
/// Distinct from the existing slot-kind markers (`HasRawFishInInventory`,
/// `HasRawMeatInInventory`, …) which gate preservation-chain DSEs on
/// specific raw inputs — this marker fires on *any* food, the way
/// "cat carries something it could eat" is the right perceptual axis
/// for the Eat-aspiration's method cascade and the eat-Sink DSE.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFoodInInventory;
impl HasFoodInInventory {
    pub const KEY: &str = "HasFoodInInventory";
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

/// Per-cat: this cat *believes* the colony's thornbriar reserve is at or
/// below `BeliefsConstants::low_ward_reserve_threshold`. Authored from
/// `ColonyReservesBelief` (not raw colony state) — reflects subjective
/// anticipation, so cats with no belief evidence don't fire the marker.
///
/// Writer: `items.rs::update_low_ward_reserve_markers` (ticket 308).
/// Reader: Herbcraft DSE consideration (ticket 309, blocks 308).
/// Allowlisted in `scripts/substrate_stubs.allowlist` with ticket 309
/// until the reader lands.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasLowWardReserve;
impl HasLowWardReserve {
    pub const KEY: &str = "HasLowWardReserve";
}

/// 231: per-cat marker indicating the cat has at least one empty
/// inventory slot (`!Inventory::is_full()`). Authored by
/// `items.rs::update_inventory_markers`.
///
/// Read on the substrate-path variant of the four pickup-class plan
/// actions (`PickUpItemFromGround` / `RetrieveRawFood` /
/// `RetrieveFoodForKitten` / `GatherHerb`). When absent, the planner's
/// substrate path fails its precondition and only the plan-path
/// variant (gated on `HasFreeSlotThisPlan(true)` after a DropItem-as-
/// prefix step) remains expandable — A* composes
/// `[DropItem, PickUp]` automatically when the cat is full. Mirrors
/// the ticket-096 Construct dual-branch precedent.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFreeSlot;
impl HasFreeSlot {
    pub const KEY: &str = "HasFreeSlot";
}

/// 367: per-cat — inventory contains at least one raw fish. Reader:
/// `DryFoodDse` eligibility filter. Writer: `items::update_inventory_markers`.
/// Distinct from `HasRawFoodInStores` (colony-scoped) — this is the
/// "I'm carrying drying-eligible food right now" personal signal.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasRawFishInInventory;
impl HasRawFishInInventory {
    pub const KEY: &str = "HasRawFishInInventory";
}

/// 367: per-cat — inventory contains at least one raw organ. Reader:
/// `DryFoodDse` eligibility filter (organ → Preserved Organ recipe).
/// Writer: `items::update_inventory_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasRawOrganInInventory;
impl HasRawOrganInInventory {
    pub const KEY: &str = "HasRawOrganInInventory";
}

/// 367: per-cat — inventory contains at least one raw meat
/// (`ItemKind::is_raw_meat()` — mammals + birds; fish goes through
/// drying). Reader: `SmokeMeatDse` eligibility filter. Writer:
/// `items::update_inventory_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasRawMeatInInventory;
impl HasRawMeatInInventory {
    pub const KEY: &str = "HasRawMeatInInventory";
}

/// 367: per-cat — inventory contains at least one fuel item (currently
/// `ItemKind::Wood`). Reader: `SmokeMeatDse` eligibility filter (the
/// load chain requires meat + fuel). Writer:
/// `items::update_inventory_markers`. Separate marker from
/// `HasMaterialsInInventory` because the semantic ("can I light a fire")
/// is narrower than "carries any build material"; Stone is a material
/// but not a fuel.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFuelInInventory;
impl HasFuelInInventory {
    pub const KEY: &str = "HasFuelInInventory";
}

/// 367: per-cat — inventory contains *something* that can go onto a
/// Drying Rack — either a Raw Fish (→ DriedFish recipe) or a Raw Organ
/// (→ PreservedOrgan recipe, also needs a herb but the resolver-side
/// pick handles that). Unified gate marker because `EligibilityFilter`
/// only supports AND across required markers, and the DSE needs an
/// OR-of-{fish, organ} signal. Sister marker to
/// `HasRawFoodInStores` (which OR's across many food kinds at the
/// colony layer for `CookDse`). Writer:
/// `items::update_inventory_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasDryableInInventory;
impl HasDryableInInventory {
    pub const KEY: &str = "HasDryableInInventory";
}

/// 367: per-cat — inventory satisfies the *full* Smoke Meat load
/// requirement (meat AND fuel). Conjunction marker because the load
/// resolver consumes both in one step; gating on just meat or just
/// fuel would let the DSE fire and the load step fail at runtime.
/// Writer: `items::update_inventory_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasSmokeableInInventory;
impl HasSmokeableInInventory {
    pub const KEY: &str = "HasSmokeableInInventory";
}

/// 235: per-cat marker — the cat's inventory contains at least one
/// build material (Wood/Stone/Moss/DriedGrass/Feather/ShadowBone).
/// Authored by `items.rs::update_inventory_markers` from
/// `inventory.has_any_material()`.
///
/// Reader lands with the 235-follow-on ticket that introduces the
/// central material pile destination + materials-deposit-prefix branch
/// in plan templates. Allowlisted in `scripts/substrate_stubs.allowlist`
/// under that follow-on id until the reader ships.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasMaterialsInInventory;
impl HasMaterialsInInventory {
    pub const KEY: &str = "HasMaterialsInInventory";
}

/// 235: per-cat marker — the cat's inventory contains at least one
/// curio (ShinyPebble/GlassShard/ColorfulShell). Authored by
/// `items.rs::update_inventory_markers` from
/// `inventory.has_any_curio()`.
///
/// Reader lands with ticket 16's Cache building (the curio sink). Curios
/// stay droppable-anywhere (v1 from 231) until that destination exists.
/// Allowlisted in `scripts/substrate_stubs.allowlist` under ticket 16
/// until the reader ships.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasCuriosInInventory;
impl HasCuriosInInventory {
    pub const KEY: &str = "HasCuriosInInventory";
}

/// 235: per-cat marker — at least one `Stores` building is within
/// `DispositionConstants::herb_stash_reachable_radius` Manhattan tiles
/// of this cat's position. Authored by
/// `goap.rs::herb_stash_accessible_for` in the per-cat MarkerSnapshot
/// loop (twin call sites: `evaluate_and_plan` and
/// `build_planner_markers`, both required for snapshot/planner-replay
/// parity).
///
/// Read on the deposit-prefix branch of pickup-class plan templates
/// (PickingUp / Cooking / Caretaking / Herbalism / Hunting). Combined
/// with `HasHerbsInInventory` + `CarryingIs(Herbs)` precondition + the
/// existing `ZoneIs(Stores)` constraint, A* composes
/// `[TravelTo(Stores), DepositHerbs(prefix), <goal-action>]` as an
/// alternative to `[DropItem, <goal-action>]` when the stash is
/// reachable, picking by cost. Far-from-stores cats fall back to the
/// DropItem prefix because the marker is false.
///
/// Sibling pattern to `MaterialsAvailable` (per-cat reachability author
/// at `goap.rs::materials_available_for`).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasHerbStashAccessible;
impl HasHerbStashAccessible {
    pub const KEY: &str = "HasHerbStashAccessible";
}

// ---------------------------------------------------------------------------
// Colony singleton
// ---------------------------------------------------------------------------

/// Marker for the single colony-state entity. Spawned exactly once
/// per simulation by `setup.rs::build_new_world` (production) and
/// `scenarios/env.rs::init_scenario_world_with` (scenario harness).
/// Colony-scoped markers below (ThornbriarAvailable,
/// HasFunctionalKitchen, …) attach to this entity. Authored each
/// FixedUpdate tick by the colony-marker chain
/// (`buildings::update_colony_building_markers`,
/// `magic::update_{herb_availability,ward_coverage,ward_siege}_markers`)
/// and cached into `WorldSnapshots::colony_markers` by
/// `world_snapshots::populate_world_snapshots`; `goap::evaluate_and_plan`
/// reads the cached bundle to populate `MarkerSnapshot`. Tickets 168, 433.
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

/// 367: colony — ≥1 functional, idle Drying Rack exists in the colony.
/// "Functional" = `Structure::effectiveness() > 0.0` (condition above
/// the 0.2 floor); "idle" = `DryingRackState.loaded.is_none()`. Reader:
/// `DryFoodDse` eligibility filter. Writer:
/// `buildings::update_colony_building_markers`. When all racks are
/// loaded, this drops to false and DryFood DSE shuts off — cats stop
/// trying to load racks that are already drying something.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFunctionalDryingRack;
impl HasFunctionalDryingRack {
    pub const KEY: &str = "HasFunctionalDryingRack";
}

/// 367: colony — ≥1 functional, idle Smoking Rack exists in the colony.
/// Same shape as `HasFunctionalDryingRack`. Reader: `SmokeMeatDse`
/// eligibility filter (the load chain). Writer:
/// `buildings::update_colony_building_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFunctionalSmokingRack;
impl HasFunctionalSmokingRack {
    pub const KEY: &str = "HasFunctionalSmokingRack";
}

/// 457: colony — ≥1 functional Workshop structure exists in the colony.
/// "Functional" = `Structure::effectiveness() > 0.0` (condition above
/// the damaged floor). Reader: `CraftAtWorkshopDse` eligibility filter.
/// Writer: `buildings::update_colony_building_markers`. Unlike the
/// preservation racks, Workshops don't carry a per-station load state —
/// any functional Workshop can host any of the six Phase 2 recipes, so
/// the marker is presence-only (no "idle" predicate layered on top).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFunctionalWorkshop;
impl HasFunctionalWorkshop {
    pub const KEY: &str = "HasFunctionalWorkshop";
}

/// 369: colony — ≥1 functional Tanning Frame exists in the colony.
/// Same presence-only shape as `HasFunctionalWorkshop` — Tanning
/// Frames host single-pass Phase 2b hide-craft recipes (HideBracers /
/// HidePlatedWrap) with no per-rack load state. Reader:
/// `CraftAtTanningFrameDse` eligibility filter. Writer:
/// `buildings::update_colony_building_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasFunctionalTanningFrame;
impl HasFunctionalTanningFrame {
    pub const KEY: &str = "HasFunctionalTanningFrame";
}

/// 367 follow-on: colony — ≥1 RawFish or RawOrgan item sits in any
/// `StoredItems` aggregate. Reader: composite per-cat marker
/// `HasDryableAccessible` populated in `goap::evaluate_and_plan`; the
/// composite is what `DryFoodDse` eligibility consults. Writer:
/// `buildings::update_colony_building_markers`.
///
/// Distinct from `HasRawFoodInStores`, which fires on *any* raw food
/// (including RawMouse / RawRat which the drying recipes don't accept).
/// Pre-existence of this marker is what lets a cat with an empty
/// inventory still elect `DryFood` — the planner builds a
/// `[RetrieveDryable, DryFood]` chain.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasDryableInStores;
impl HasDryableInStores {
    pub const KEY: &str = "HasDryableInStores";
}

/// 367 follow-on: per-cat composite — the cat could conceivably elect
/// `DryFood` this tick. Fires when EITHER the cat already carries a
/// dryable item (`HasDryableInInventory`) OR the cat has a free
/// inventory slot AND the colony has a dryable in `StoredItems`
/// (`HasFreeSlot && HasDryableInStores`). Reader: `DryFoodDse`
/// eligibility filter. Writer: `goap::evaluate_and_plan` via
/// `MarkerSnapshot::set_entity`.
///
/// Replaces `HasDryableInInventory` in the DSE eligibility list. Pre-
/// follow-on the narrow inventory marker gated the DSE; cats almost
/// never held raw fish / organ at score-time (deposit-at-stores drains
/// inventory on every hunt-return), so `DryFood` never fired even when
/// a functional rack existed and stores were full of fish.
/// `HasDryableInInventory` is still authored — runtime resolvers
/// (`resolve_load_drying_rack`) read it to know whether to consume the
/// cat's slot or run the retrieve step first.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasDryableAccessible;
impl HasDryableAccessible {
    pub const KEY: &str = "HasDryableAccessible";
}

/// 443: colony — ≥1 raw-meat item AND ≥1 fuel (Wood) item sit in any
/// `StoredItems`. Reader: per-cat composite `HasSmokeableAccessible`
/// in `goap::evaluate_and_plan`. Writer:
/// `buildings::update_colony_building_markers`.
///
/// Distinct from `HasRawFoodInStores` (fires on all raw food) and
/// `HasDryableInStores` (RawFish/RawOrgan only). Smoking requires
/// both meat AND fuel present together; the marker encodes the
/// conjunction so the per-cat composite can gate on a single
/// colony-level boolean.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasSmokeableInStores;
impl HasSmokeableInStores {
    pub const KEY: &str = "HasSmokeableInStores";
}

/// 443: per-cat composite — the cat could conceivably elect `SmokeMeat`
/// this tick. Fires when EITHER the cat already carries smokeable
/// inventory (`HasSmokeableInInventory`) OR has a free slot AND the
/// colony has smokeable meat + fuel in `StoredItems`
/// (`HasFreeSlot && HasSmokeableInStores`). Reader: `SmokeMeatDse`
/// eligibility filter. Writer: `goap::evaluate_and_plan` via
/// `MarkerSnapshot::set_entity`.
///
/// Mirrors `HasDryableAccessible` for the two-ingredient smoking chain.
/// `HasSmokeableInInventory` (both meat AND fuel) is still authored —
/// resolvers read it to short-circuit retrieve steps when the cat
/// already carries the needed items.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasSmokeableAccessible;
impl HasSmokeableAccessible {
    pub const KEY: &str = "HasSmokeableAccessible";
}

/// 457: per-cat — the cat carries ≥1 Phase 2 Workshop-recipe input
/// (Twig / Bristle / Fiber / Flower / Stone / Feather / PolishedStone).
/// Authored by `items::update_inventory_markers` mirroring the existing
/// `HasRawFishInInventory` / `HasFuelInInventory` rows. Reader:
/// `CraftAtWorkshopDse` eligibility filter.
///
/// Recipe-agnostic by design — any single Workshop input present in
/// inventory satisfies the marker; the resolver picks the specific
/// recipe at execute time. Mirrors the 367 inventory-marker shape
/// (`HasDryableInInventory` fires on any RawFish OR RawOrgan; the
/// drying resolver picks the specific raw input). A cat with Twig but
/// no Bristle still fires the marker — the L3 may elect Crafting, the
/// resolver finds no full recipe satisfied, returns Fail, and the cat
/// re-plans (substrate-honest: the per-recipe scoring lives at recipe-
/// variety, deferred per ticket scope).
///
/// Stores-side retrieve is intentionally NOT in scope for first-light.
/// Cats gather inputs via hunt (Bristle from `PreyByproductConstants`)
/// plus forage (`resolve_forage` drops Twig / Fiber / Flower at
/// `forage_ingredient_drop_chance = 0.10`) and craft when inputs are
/// already in hand. The plan template is single-step
/// `[CraftAtWorkshop]`, no `RetrieveCraftInput` leg.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasCraftInputInInventory;
impl HasCraftInputInInventory {
    pub const KEY: &str = "HasCraftInputInInventory";
}

/// 367: colony — ≥1 loaded Smoking Rack exists in the colony AND its
/// per-rack tend cooldown has elapsed (i.e. it's ready to be tended
/// right now). Reader: `TendSmokingRackDse` eligibility filter. Writer:
/// `buildings::update_colony_building_markers` (the writer evaluates
/// `current_tick - rack.last_tended_at_tick >=
/// crafting.smoking_tend_cooldown_ticks` per rack).
///
/// Distinct from "any loaded smoking rack" because we want the Tend DSE
/// to score zero when every rack is on cooldown — that's the
/// interleaving discipline (cats do something else for ~2 sim-hours
/// between tends).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasLoadedSmokingRackOffCooldown;
impl HasLoadedSmokingRackOffCooldown {
    pub const KEY: &str = "HasLoadedSmokingRackOffCooldown";
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

/// Ticket 084: ≥1 Thornbriar count exists in the colony's
/// `StoredHerbs` aggregate (summed across all Stores buildings).
/// Authored by `buildings.rs::update_colony_building_markers`.
/// Read by `HerbcraftSetWard`'s `CanWardFromSupply` eligibility gate
/// in Commit 2, and by `RetrieveHerbs(Thornbriar)` planner action
/// preconditions. Distinct from `ThornbriarAvailable` which gates on
/// *wild* harvestable thornbriar entities — this marker gates on
/// *stashed* thornbriar inside Stores.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasStoredThornbriar;
impl HasStoredThornbriar {
    pub const KEY: &str = "HasStoredThornbriar";
}

/// Ticket 084 Commit 3: colony-scoped chronicity marker — total
/// stashed thornbriar across all Stores has been *below*
/// `ScoringConstants::thornbriar_stash_low_threshold` for at least one
/// full `chronicity_window_ticks` window. Mirrors the 179 pattern
/// (`ColonyStoresChronicallyFull`): a slow-rolling latch that only
/// flips at window boundaries, filtering out single-tick transients
/// from gather/retrieve traffic.
///
/// Reader 1: the coordinator's `accumulate_build_pressure` Farming
/// gate (`coordination.rs:~1090`) — drives "we need to commit to a
/// Garden for thornbriar."
/// Reader 2: `FarmDse`'s `farm_herb_pressure` axis (replaces the
/// per-tick scalar with a marker consideration mirroring
/// `BuildDse::colony_stores_chronically_full`).
/// Writer: `buildings.rs::update_colony_building_markers` extended
/// with a `ThornbriarPressureTracker`-backed window latch.
#[derive(Component, Debug, Clone, Copy)]
pub struct ColonyThornbriarChronicallyLow;
impl ColonyThornbriarChronicallyLow {
    pub const KEY: &str = "ColonyThornbriarChronicallyLow";
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
impl HasConstructionSite {
    pub const KEY: &str = "HasConstructionSite";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HasDamagedBuilding;
impl HasDamagedBuilding {
    pub const KEY: &str = "HasDamagedBuilding";
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HasGarden;
impl HasGarden {
    pub const KEY: &str = "HasGarden";
}

/// 176: colony Stores have been refusing deposits at a chronic
/// rate over the trailing window. Author: `update_colony_storage_pressure`
/// in `src/systems/buildings.rs`. Read: Build DSE's score-bonus
/// consideration (lifts the colony toward "build another Stores")
/// and Coordinator's `assess_colony_needs` (queues a `Build` directive
/// of `StructureType::Stores` when set). Computed from a per-colony
/// sliding-window count of `Feature::DepositRejected` events scaled by
/// colony cat-count: when `rejected_per_cat_per_window` exceeds
/// `chronicity_threshold` the marker is inserted; otherwise removed.
#[derive(Component, Debug, Clone, Copy)]
pub struct ColonyStoresChronicallyFull;
impl ColonyStoresChronicallyFull {
    pub const KEY: &str = "ColonyStoresChronicallyFull";
}

/// 178: colony has at least one `StructureType::Midden` building.
/// Authored by `update_colony_building_markers` in `src/systems/buildings.rs`
/// (single pass: any Midden structure exists ⇒ insert; else remove).
/// Read by the Trashing DSE's `EligibilityFilter::require(HasMidden::KEY)`
/// — without it, the disposition is dormant and the cat falls back to
/// Discarding (which gates on `ColonyStoresChronicallyFull`).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasMidden;
impl HasMidden {
    pub const KEY: &str = "HasMidden";
}

/// Colony-scoped marker: ≥1 cat in the colony is a *care dependent* —
/// a creature who cannot self-provision and needs another cat to bring
/// it food. Read by both the Handing DSE's
/// `EligibilityFilter::require(HasDependentCat::KEY)` and (ticket 410)
/// the Caretake DSE's same requirement. Authored by
/// `update_colony_building_markers`.
///
/// **Narrative, not mechanic.** This marker says "a creature here needs
/// care," not "a slot exists to receive an item" — the latter
/// (`HasHandoffRecipient` pre-410) would equally apply to a
/// construction-kitty waiting on reeds, conflating distinct narratives.
/// Per the "mechanics are the narrative" design pillar.
///
/// **Current population:** any living `Kitten` (kittens cannot hunt and
/// depend on adults for food). The populator
/// (`src/systems/buildings.rs::update_colony_building_markers`) trivially
/// extends to other categories — incapacitated adults who cannot reach
/// food, other future dependents — as `kittens.is_empty() &&
/// other_dependents.is_empty()`. No consumer changes required when the
/// union grows.
///
/// **Colony-scope rather than per-cat:** any caregiver responds to any
/// dependent; the existence of *any* dependent in the colony enables
/// Caretake/Handing for *any* eligible cat. Actual recipient resolution
/// happens at dispatch time (`goap.rs::HandoffItem` picks the
/// hungriest-then-nearest from the kitten roster); per-cat picker is a
/// balance follow-on (ticket 192).
///
/// Ticket 188 authored the marker (then `HasHandoffRecipient`); ticket
/// 410 renamed and extended its consumer set.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasDependentCat;
impl HasDependentCat {
    pub const KEY: &str = "HasDependentCat";
}

/// Colony-scoped marker indicating ≥1 ground carcass (an `Item` with
/// `kind.is_food()` and `location == ItemLocation::OnGround`) exists
/// somewhere in the colony. Read by the PickingUp DSE's
/// `EligibilityFilter::require(HasGroundCarcass::KEY)`; authored by
/// `update_colony_building_markers`.
///
/// Today's source: engage_prey overflow at the kill tile when a cat's
/// inventory is full and it isn't self-eating
/// (`goap.rs::resolve_engage_prey`). Forward-compatible with future
/// carcass-as-container loot tables — child `Item` entities spawned at
/// a `Carcass` entity's tile will appear in the same query without any
/// further changes here.
///
/// **History.** Spec'd by 178 with the OnGround food-Item semantic;
/// 185 wired it incorrectly to `Carcass` *component* entities (which
/// `resolve_pick_up_from_ground` cannot consume — only `Item` entities
/// move through PickUp). Ticket 193 restored the spec'd semantic after
/// diagnosing 1367/10kt `PickingUp:GoalUnreachable` replans driven by
/// the marker/resolver mismatch in the post-185 canonical soak.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasGroundCarcass;
impl HasGroundCarcass {
    pub const KEY: &str = "HasGroundCarcass";
}

/// ≥1 other cat has a skill below 0.3 where this cat has the same
/// skill above 0.6 (per-cat relative predicate).
/// `aspirations.rs::update_mentoring_target_markers`.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasMentoringTarget;
impl HasMentoringTarget {
    pub const KEY: &str = "HasMentoringTarget";
}

/// 035: ≥1 unburied colony-mate corpse (entity with `Dead`, without
/// `Buried`) within `disposition.burial_sense_range` Manhattan tiles
/// of this cat. Authored by
/// `sensing.rs::update_target_existence_markers` in the same per-cat
/// pass that authors `HasSocialTarget` and `CarcassNearby`. Read by
/// `bury_dse`'s `EligibilityFilter::require(HasUnburiedCorpse::KEY)`
/// — when absent, the burial DSE is skipped and the corpse-target
/// picker is never called.
#[derive(Component, Debug, Clone, Copy)]
pub struct HasUnburiedCorpse;
impl HasUnburiedCorpse {
    pub const KEY: &str = "HasUnburiedCorpse";
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
/// Author: `growth.rs::update_kitten_cry_map` (ticket 161 merged the
/// authoring here from a separate Chain 2a system; the cry-map and
/// the marker share the same `&Needs` access and the same hunger
/// predicate, so co-locating them avoids adding a new schedule
/// conflict edge to Bevy's parallel scheduler).
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

/// **Active milestone-arc availability** (ticket 395). Cat has ≥1
/// living dependent kitten where this cat is `mother` or `father` AND
/// the kitten is in either:
/// - **Early arc window** `[0, teach_done_threshold)` — Wean / Teach
///   milestones still have eligibility, OR
/// - **Near-mature window** `[release_threshold, 1.0)` — Release is
///   pickable AND the kitten has not yet been symbolically released
///   (no `RearKittenReleased` marker).
///
/// Gates the `kitten_reared` reactive emit so the arc doesn't churn
/// during the long `[teach_done_threshold, release_threshold)` idle
/// gap (queen does Caretake-only there) or after symbolic Release
/// has fired for the kitten. Both parents pitch in — 395 retired the
/// 333/364 mother-only deferral on the picker too.
///
/// **§4.3 ordering hazard.** Same as [`Parent`]: a kitten's death
/// removes its parents' markers within the same tick. Don't infer
/// grief-parent status from `With<HasJuvenileDependent>` post-death.
///
/// Authoring: `growth.rs::update_parent_markers` (merged pass with
/// `Parent`).
#[derive(Component, Debug, Clone, Copy)]
pub struct HasJuvenileDependent;
impl HasJuvenileDependent {
    pub const KEY: &str = "HasJuvenileDependent";
}

/// **Symbolic Release fired** (ticket 395). Inserted by the
/// `rear_kitten` arc's Release drain when a parent (mother or father)
/// witnesses `Feature::KittenReleased` for this kitten. One-shot
/// semantics: the second parent's concurrent frame, on its next
/// dispatch, sees `released_by_arc=true` in the picker snapshot and
/// returns None → R11 Advance → frame pops without re-witnessing.
/// The marker also flips `HasJuvenileDependent` false on the parents'
/// side so the near-mature emit window stops firing for this kitten
/// even before natural maturation.
///
/// Persists on the kitten until despawn — survives natural
/// maturation alongside `BornInSim`.
///
/// Authoring: drain arm `KittenRearingAdvance::Release` in
/// `goap.rs`.
#[derive(Component, Debug, Clone, Copy)]
pub struct RearKittenReleased;
impl RearKittenReleased {
    pub const KEY: &str = "RearKittenReleased";
}

/// Cat was born during this simulation run (not a founding member).
/// Inserted once at the kitten-spawn site in `pregnancy.rs` alongside
/// `KittenDependency::new(...)`; never removed. Survives maturation
/// and persists until `cleanup_dead` despawns the entity.
///
/// **Why a born-once marker, not derived from `Age::born_tick`** —
/// founding cats also carry `born_tick` (set to `start_tick - age_ticks`
/// in `world_gen/colony.rs::generate_starting_cats`), and at the canonical
/// `start_tick = 0` they collapse to `born_tick = 0` indistinguishably from
/// in-sim-born cats. `KittenDependency` is removed at maturation so it can't
/// serve either. The marker is the canonical "born in this run" substrate.
///
/// **Consumer:** `colony_score.kittens_matured` increments on maturation
/// (`growth.rs::tick_kitten_growth`) and decrements on the death of a
/// matured in-sim-born cat (`death.rs::check_death`, gate
/// `With<BornInSim> + Without<KittenDependency>`). Ticket 166.
#[derive(Component, Debug, Clone, Copy)]
pub struct BornInSim;
impl BornInSim {
    pub const KEY: &str = "BornInSim";
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

/// Ward within fox detection radius — truthful per-tick scan: any
/// ward whose `repel_radius()` reaches the fox's tile. Authored by
/// `fox_spatial.rs::update_ward_detection_markers`. No DSE consumer
/// today; fox flee-from-wards behavior is a future ticket.
#[derive(Component, Debug, Clone, Copy)]
pub struct WardNearbyFox;
impl WardNearbyFox {
    pub const KEY: &str = "WardNearbyFox";
}

/// Mother fox at a den whose `cubs_present > 0`. Hybrid
/// event-driven + per-marker reconciliation
/// (`fox_spatial.rs::update_cub_marker`): `CubsBorn` events insert
/// the marker on the mother at the moment of spawn; a reconciliation
/// pass over flagged foxes removes the marker when the den's
/// cub count drops to 0 (cub maturation / cub death).
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

/// Fox has a home den. Authored event-driven by
/// `fox_spatial.rs::update_den_marker` from `DenClaimed` / `DenLost`
/// messages emitted in `wildlife.rs` (initial pair spawn, cub birth,
/// cub maturation, fox death).
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
        // 450 — kitten sub-stages + mentorable gate.
        assert_marker_queryable(NewbornKitten);
        assert_marker_queryable(EyesOpenKitten);
        assert_marker_queryable(JuvenileKitten);
        assert_marker_queryable(MentorableAge);
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
        assert_marker_queryable(Buried);
    }

    #[test]
    fn capability_markers_queryable() {
        assert_marker_queryable(CanHunt);
        assert_marker_queryable(CanForage);
        assert_marker_queryable(CanWard);
        assert_marker_queryable(CanWardFromSupply);
        assert_marker_queryable(CanCook);
        // 367 — preservation capabilities.
        assert_marker_queryable(CanDry);
        assert_marker_queryable(CanSmoke);
        // 457 — Workshop-craft capability.
        assert_marker_queryable(CanCraft);
    }

    #[test]
    fn inventory_markers_queryable() {
        assert_marker_queryable(HasHerbsInInventory);
        // 450 — generic food-in-inventory marker for the Eat method cascade.
        assert_marker_queryable(HasFoodInInventory);
        assert_marker_queryable(HasRemedyHerbs);
        assert_marker_queryable(HasWardHerbs);
        assert_marker_queryable(HasFunctionalKitchen);
        assert_marker_queryable(HasRawFoodInStores);
        assert_marker_queryable(HasStoredFood);
        assert_marker_queryable(ThornbriarAvailable);
        assert_marker_queryable(HasStoredThornbriar);
        assert_marker_queryable(ColonyThornbriarChronicallyLow);
        assert_marker_queryable(MaterialsAvailable);
        // 367 — preservation markers.
        assert_marker_queryable(HasRawFishInInventory);
        assert_marker_queryable(HasRawOrganInInventory);
        assert_marker_queryable(HasRawMeatInInventory);
        assert_marker_queryable(HasFuelInInventory);
        assert_marker_queryable(HasFunctionalDryingRack);
        assert_marker_queryable(HasFunctionalSmokingRack);
        assert_marker_queryable(HasLoadedSmokingRackOffCooldown);
        assert_marker_queryable(HasDryableInInventory);
        assert_marker_queryable(HasSmokeableInInventory);
        assert_marker_queryable(HasDryableInStores);
        assert_marker_queryable(HasDryableAccessible);
        // 443 — smoking chain accessibility markers.
        assert_marker_queryable(HasSmokeableInStores);
        assert_marker_queryable(HasSmokeableAccessible);
        // 457 — Workshop-craft station + input markers.
        assert_marker_queryable(HasFunctionalWorkshop);
        assert_marker_queryable(HasCraftInputInInventory);
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
        assert_marker_queryable(HasUnburiedCorpse);
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
        assert_marker_queryable(HasJuvenileDependent);
        assert_marker_queryable(RearKittenReleased);
        assert_marker_queryable(BornInSim);
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
